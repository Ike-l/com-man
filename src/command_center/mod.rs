use std::{sync::atomic::{AtomicU64, Ordering}, time::Duration};

use crate::{slash_commands::SlashCommand, commands::Commands};
use serenity::{futures::StreamExt, prelude::CacheHttp};


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
                if let Err(err) = Self::vote(
                    ctx, 
                    msg, 
                    message,
                    threshold,
                    duration,
                    reaction_emoji
                ).await {
                    println!("Error Sending Message: {err:?}");
                }
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
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let vote_msg = msg.channel_id.say(
            &ctx.http,
            format!(
                "Initiating vote to execute: `{message}`! React with `{}` to execute (Needs {} more votes in {}s).",
                reaction_emoji,
                threshold - 1,
                duration.as_secs()
            ),
        ).await?;

        vote_msg.react(&ctx.http, reaction_emoji.clone()).await?;

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
            msg.channel_id
                .say(&ctx.http, format!("Vote passed! Executing `{message}`"))
                .await?;
            
            let trimmed = message.trim();
            if trimmed.starts_with('/') {
                Self::execute_slash_command(&ctx, &msg, &trimmed).await;
            } else {
                Self::echo(&msg.channel_id, &ctx.http, message).await;
            }
        } else {
            msg.channel_id
                .say(&ctx.http, format!("Vote failed for `{message}`. Not enough votes in {}s", duration.as_secs()))
                .await?;
        }

        Ok(())
    }

    async fn execute_slash_command(ctx: &serenity::all::Context, msg: &serenity::all::Message, message: &str) {
        let Some(guild_id) = msg.guild_id else { return };
        let Ok(slash_command) = SlashCommand::try_from(
            message,
            &guild_id,
            ctx
        ).await else { return };

        match slash_command {
            SlashCommand::Kick(user_id) => {
                let _ = guild_id.kick(ctx.http(), user_id).await;
            },
            SlashCommand::Ban(user_id) => {
                let _ = guild_id.ban(ctx.http(), user_id, 0).await;
            },
            SlashCommand::Disconnect(user_id) => {
                let _ = guild_id.disconnect_member(ctx.http(), user_id).await;
            },
            SlashCommand::Timeout((user_id, timestamp)) => {
                let Ok(mut member) = guild_id.member(ctx.http(), user_id).await else { return };
                let _ = member.disable_communication_until_datetime(ctx.http(), timestamp).await;
            }
        }
    }

    async fn pong(channel_id: &serenity::all::ChannelId, cache_http: impl serenity::http::CacheHttp) {
        Self::echo(channel_id, cache_http, "Pong!").await;
    }

    async fn echo(channel_id: &serenity::all::ChannelId, cache_http: impl serenity::http::CacheHttp, message: impl Into<String>) {
        if let Err(err) = channel_id.say(&cache_http, message).await {
            println!("Error Sending Message: {err:?}");
        }
    }
}