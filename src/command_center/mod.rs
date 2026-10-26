use std::{pin::Pin, sync::{atomic::{AtomicU64, Ordering}, Arc}, time::Duration};
use std::future::Future;

use crate::commands::Commands;
use serenity::futures::StreamExt;

pub struct CommandCenter {
    pub vote_threshold: AtomicU64,
    pub vote_duration: serenity::prelude::Mutex<Duration>
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
                if let Err(err) = Self::vote(
                    ctx, 
                    msg, 
                    message,
                    threshold,
                    duration
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
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let emoji = serenity::all::ReactionType::Unicode("👍".to_string());

        let vote_msg = msg.channel_id.say(
            &ctx.http,
            format!(
                "Initiating vote to execute: `{message}`! React with `{}` to execute (Needs {} more votes in {}s).",
                emoji,
                threshold - 1,
                duration.as_secs()
            ),
        ).await?;

        vote_msg.react(&ctx.http, emoji.clone()).await?;

        let collector = vote_msg
            .await_reactions(&ctx.shard)
            .timeout(duration)
            .filter(move |reaction| reaction.emoji == emoji);

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

    async fn pong(channel_id: &serenity::all::ChannelId, cache_http: impl serenity::http::CacheHttp) {
        Self::echo(channel_id, cache_http, "Pong!").await;
    }

    async fn echo(channel_id: &serenity::all::ChannelId, cache_http: impl serenity::http::CacheHttp, message: impl Into<String>) {
        if let Err(err) = channel_id.say(&cache_http, message).await {
            println!("Error Sending Message: {err:?}");
        }
    }
}