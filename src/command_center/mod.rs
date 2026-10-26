use std::{sync::atomic::{AtomicU64, Ordering}, time::Duration};

use crate::{slash_commands::SlashCommand, commands::Commands};
use serenity::futures::StreamExt;
use tracing::{Level, event, span};


pub struct CommandCenter {
    pub vote_threshold: AtomicU64,
    pub vote_duration: serenity::prelude::Mutex<Duration>,
    pub reaction_emoji: serenity::all::ReactionType,
}

impl CommandCenter {
    pub async fn execute(&self, command: Commands, is_admin: bool, ctx: serenity::all::Context, msg: serenity::all::Message) {
        match command {
            Commands::Ping => {
                Self::pong(&msg.channel_id, &ctx.http).await;
            },
            Commands::Echo(message) => {
                let threshold = self.vote_threshold.load(Ordering::Relaxed) + 1;
                let duration = self.vote_duration.lock().await.clone();
                let reaction_emoji = self.reaction_emoji.clone();
                Self::vote(ctx, msg, message, threshold, duration, reaction_emoji).await;
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
            Commands::SetVoteThreshold(threshold) => {
                if is_admin {
                    self.vote_threshold.store(threshold, Ordering::Relaxed);
                    Self::echo(&msg.channel_id, ctx.http, format!("Successfully set threshold to {threshold}")).await;
                } else {
                    Self::echo(&msg.channel_id, ctx.http, "You need admin for this command").await;
                }
            }
        }
    }

    async fn vote(
        ctx: serenity::all::Context, 
        msg: serenity::all::Message,
        message: String,
        threshold: u64,
        duration: Duration,
        reaction_emoji: serenity::all::ReactionType,
    ) {
        let Some(vote_msg) = Self::echo(&msg.channel_id, &ctx.http, format!(
            "Initiating vote to execute: `{message}`! React with `{}` to execute (Needs {} more votes in {}s).",
            reaction_emoji,
            threshold - 1,
            duration.as_secs()
        )).await else { return };

        let react_result = vote_msg.react(&ctx.http, reaction_emoji.clone()).await;
        if let Err(_) = react_result {
            let _ = Self::echo(&msg.channel_id, &ctx.http, "Failed to react to my message");
        }

        let collector = vote_msg
            .await_reactions(&ctx.shard)
            .timeout(duration)
            .filter(move |reaction| reaction.emoji == reaction_emoji);

        let mut reaction_stream = collector.stream();
        let mut current_votes: u64 = 0;

        while let Some(reaction) = reaction_stream.next().await {
            if let Ok(updated_msg) = vote_msg.channel_id.message(&ctx.http, vote_msg.id).await {
                if let Some(reaction_count) = updated_msg
                    .reactions
                    .iter()
                    .find(|r| r.reaction_type == reaction.emoji)
                {
                    current_votes = reaction_count.count;
                    if current_votes >= threshold {
                        break;
                    }
                }
            }
        }

        if current_votes >= threshold {
            Self::echo(&msg.channel_id, &ctx.http, format!("Vote passed! Executing `{message}`")).await;
            
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