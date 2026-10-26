use std::{sync::atomic::{AtomicU64, Ordering}, time::Duration};

use crate::{commands::Commands, get_user_id, slash_commands::SlashCommand};
use serenity::futures::StreamExt;
use tracing::{Level, event, span};


pub struct CommandCenter {
    pub accept_vote_threshold: AtomicU64,
    pub deny_vote_threshold: AtomicU64,
    pub vote_duration: serenity::prelude::Mutex<Duration>,
    pub accept_reaction_emoji: serenity::all::ReactionType,
    pub deny_reaction_emoji: serenity::all::ReactionType,
}

impl CommandCenter {
    pub async fn execute(&self, command: Commands, is_admin: bool, ctx: serenity::all::Context, msg: serenity::all::Message) {
        match command {
            Commands::Ping => {
                Self::pong(&msg.channel_id, &ctx.http).await;
            },
            Commands::Echo(message) => {
                let accept_threshold = self.accept_vote_threshold.load(Ordering::Relaxed);
                let deny_threshold = self.deny_vote_threshold.load(Ordering::Relaxed);
                let duration = self.vote_duration.lock().await.clone();
                let accept_reaction_emoji = self.accept_reaction_emoji.clone();
                let deny_reaction_emoji = self.deny_reaction_emoji.clone();
                let blocklist = if let Some(guild_id) = &msg.guild_id {
                    let mut blocklist = Vec::new();
                    for word in message.split_ascii_whitespace() {
                        if let Some(user_id) = get_user_id(guild_id, word, &ctx).await {
                            blocklist.push(user_id);
                        }
                    }
                    blocklist
                } else {
                    vec![]
                };

                Self::vote(
                    ctx, 
                    msg, 
                    message, 
                    accept_threshold, 
                    deny_threshold,
                    duration, 
                    accept_reaction_emoji,
                    deny_reaction_emoji,
                    blocklist,

                ).await;
            },
            Commands::List => {
                let message = Commands::list();
                Self::echo(&msg.channel_id, ctx.http, message).await;
            },
            Commands::SetDuration(duration) => {
                if is_admin {
                    let mut current_duration = self.vote_duration.lock().await;
                    *current_duration = duration;
                    Self::echo(&msg.channel_id, ctx.http, format!("Successfully set duration to {}s", duration.as_secs())).await;
                } else {
                    Self::echo(&msg.channel_id, ctx.http, "You need admin for this command").await;
                }
            },
            Commands::SetAcceptVoteThreshold(threshold) => {
                if is_admin {
                    self.accept_vote_threshold.store(threshold, Ordering::Relaxed);
                    Self::echo(&msg.channel_id, ctx.http, format!("Successfully set accept threshold to {threshold}")).await;
                } else {
                    Self::echo(&msg.channel_id, ctx.http, "You need admin for this command").await;
                }
            },
            Commands::SetDenyVoteThreshold(threshold) => {
                if is_admin {
                    self.deny_vote_threshold.store(threshold, Ordering::Relaxed);
                    Self::echo(&msg.channel_id, ctx.http, format!("Successfully set deny threshold to {threshold}")).await;
                } else {
                    Self::echo(&msg.channel_id, ctx.http, "You need admin for this command").await;
                }
            },
        }
    }

    async fn vote(
        ctx: serenity::all::Context, 
        msg: serenity::all::Message,
        message: String,
        accept_threshold: u64,
        deny_threshold: u64,
        duration: Duration,
        accept_reaction_emoji: serenity::all::ReactionType,
        deny_reaction_emoji: serenity::all::ReactionType,
        blocklist: Vec<serenity::all::UserId>,
    ) {
        let Some(vote_msg) = Self::echo(&msg.channel_id, &ctx.http, format!(
            "Initiating vote to execute: `{message}`.\nReact with `{}` to `accept`.\nReact with `{}` to `deny`.\nNeeds {} more `accepts`.\nNeeds {} more `denies`.\nExpires in {}s.",
            accept_reaction_emoji,
            deny_reaction_emoji,
            accept_threshold,
            deny_threshold,
            duration.as_secs()
        )).await else { return };
        
        let react_result = vote_msg.react(&ctx.http, accept_reaction_emoji.clone()).await;
        if let Err(_) = react_result {
            let _ = Self::echo(&msg.channel_id, &ctx.http, "Failed to react `accept` to my message");
        }
        let accept_threshold = accept_threshold + 1;

        let react_result = vote_msg.react(&ctx.http, deny_reaction_emoji.clone()).await;
        if let Err(_) = react_result {
            let _ = Self::echo(&msg.channel_id, &ctx.http, "Failed to react `deny` to my message");
        }
        let deny_threshold = deny_threshold + 1;

        let collector = vote_msg
            .await_reactions(&ctx.shard)
            .timeout(duration)
            .filter({
                let accept_reaction_emoji = accept_reaction_emoji.clone();
                let deny_reaction_emoji = deny_reaction_emoji.clone();
                move |reaction| reaction.emoji == accept_reaction_emoji || reaction.emoji == deny_reaction_emoji
            });

        let mut reaction_stream = collector.stream();
        let mut current_accepts: u64 = 0;
        let mut current_denies: u64 = 0;

        while let Some(reaction) = reaction_stream.next().await {
            if reaction.emoji == accept_reaction_emoji {
                if let Ok(users) = vote_msg
                    .channel_id
                    .reaction_users(&ctx.http, vote_msg.id, reaction.emoji.clone(), None, None)
                    .await
                {
                    current_accepts = users
                        .iter()
                        .filter(|u| !blocklist.contains(&u.id))
                        .count() as u64;
                }
            }

            if reaction.emoji == deny_reaction_emoji {
                if let Ok(users) = vote_msg
                    .channel_id
                    .reaction_users(&ctx.http, vote_msg.id, reaction.emoji.clone(), None, None)
                    .await
                {
                    current_denies = users
                        .iter()
                        .filter(|u| !blocklist.contains(&u.id))
                        .count() as u64;
                }
            }

            if current_denies >= deny_threshold {
                break;
            }
        }

        if current_denies >= deny_threshold {
            Self::echo(&msg.channel_id, &ctx.http, format!("Vote denied for: `{message}`")).await;
            return;
        }

        if current_accepts >= accept_threshold {
            Self::echo(&msg.channel_id, &ctx.http, format!("Vote accepted. Executing: `{message}`")).await;
            
            let trimmed = message.trim();
            if trimmed.starts_with('/') {
                Self::execute_slash_command(&ctx, &msg, &trimmed).await;
            } else {
                Self::echo(&msg.channel_id, &ctx.http, message).await;
            }
        } else {
            Self::echo(&msg.channel_id, &ctx.http, format!("Vote failed for `{message}`. Not enough votes in {}s", duration.as_secs())).await;
        }
    }

    async fn execute_slash_command(ctx: &serenity::all::Context, msg: &serenity::all::Message, message: &str) {
        let Some(guild_id) = msg.guild_id else { 
            Self::echo(&msg.channel_id, &ctx.http, "Failed to get guild id").await;
            return 
        };
        let result = SlashCommand::try_from(
            message,
            &guild_id,
            ctx
        ).await;

        let slash_command = match result {
            Ok(slash_command) => slash_command,
            Err(err) => {
                Self::echo(&msg.channel_id, &ctx.http, format!("Failed to derive slash command with error: {err}")).await;
                return;
            },
        };

        match slash_command {
            SlashCommand::DM((user_id, reason)) => {
                let err = user_id.dm(&ctx.http, serenity::all::CreateMessage::new().content(reason)).await;
                if let Err(err) = err {
                    Self::echo(&msg.channel_id, &ctx.http, format!("Failed to DM with error: {err:?}")).await;
                }
            },
            SlashCommand::Kick(user_id) => {
                let err = guild_id.kick(&ctx.http, user_id).await;
                if let Err(err) = err {
                    Self::echo(&msg.channel_id, &ctx.http, format!("Failed to Kick with error: {err:?}")).await;
                }
            },
            SlashCommand::Ban(user_id) => {
                let err = guild_id.ban(&ctx.http, user_id, 0).await;
                if let Err(err) = err {
                    Self::echo(&msg.channel_id, &ctx.http, format!("Failed to Ban with error: {err:?}")).await;
                }
            },
            SlashCommand::BanWithReason((user_id, reason)) => {
                let err = guild_id.ban_with_reason(&ctx.http, user_id, 0, reason).await;
                if let Err(err) = err {
                    Self::echo(&msg.channel_id, &ctx.http, format!("Failed to Ban with error: {err:?}")).await;
                }
            }
            SlashCommand::Disconnect(user_id) => {
                let err = guild_id.disconnect_member(&ctx.http, user_id).await;
                if let Err(err) = err {
                    Self::echo(&msg.channel_id, &ctx.http, format!("Failed to Disconnect with error: {err:?}")).await;
                }
            },
            SlashCommand::Timeout((user_id, timestamp)) => {
                let Ok(mut member) = guild_id.member(&ctx.http, user_id).await else { return };
                let err = member.disable_communication_until_datetime(&ctx.http, timestamp).await;
                if let Err(err) = err {
                    Self::echo(&msg.channel_id, &ctx.http, format!("Failed to Timeout with error: {err:?}")).await;
                }
            }
        }
    }

    async fn pong(channel_id: &serenity::all::ChannelId, cache_http: impl serenity::http::CacheHttp) {
        Self::echo(channel_id, cache_http, "Pong!").await;
    }

    pub async fn echo(channel_id: &serenity::all::ChannelId, cache_http: impl serenity::http::CacheHttp, message: impl Into<String>) -> Option<serenity::all::Message> {
        let message = message.into();
        let span = span!(Level::TRACE, "Echo", message = message.clone());
        let _enter = span.enter();

        event!(Level::TRACE, "Echoing");

        let result = channel_id.say(&cache_http, message.clone()).await;
        
        match result {
            Ok(message) => return Some(message),
            Err(err) => {
                event!(Level::ERROR, error = ?err, "Error Sending Message");
                return None
            }
        }
    }
}