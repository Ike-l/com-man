use std::{pin::Pin, sync::{atomic::{AtomicU64, Ordering}, Arc}, time::Duration};
use std::future::Future;

use crate::commands::Commands;
use serenity::futures::StreamExt;

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
        let lower = message.to_lowercase();
        if lower.starts_with("/timeout") {
            Self::execute_timeout(ctx, msg, message).await;
            return;
        } else if lower.starts_with("/kick") {
            Self::execute_kick(ctx, msg, message).await;
            return;
        } else if lower.starts_with("/ban") {
            Self::execute_ban(ctx, msg, message).await;
            return;
        } else if lower.starts_with("/mute") {
            Self::execute_timeout(ctx, msg, message).await; 
            return;
        }
        
        Self::echo(&msg.channel_id, &ctx.http, message).await;
    }

    fn parse_mention(message: &str) -> Option<serenity::all::UserId> {
        let mention_start = message.find("<@")?;
        let id_start = mention_start + 2;
        let id_end = message[id_start..]
            .find('>')
            .map(|e| id_start + e)
            .unwrap_or(message.len());
        let id_str = if message.as_bytes().get(id_start) == Some(&b'!') {
            &message[id_start + 1..id_end]
        } else {
            &message[id_start..id_end]
        };
        let id = id_str.parse::<u64>().ok()?;
        Some(serenity::all::UserId::new(id))
    }

    async fn require_guild(
        ctx: &serenity::all::Context,
        msg: &serenity::all::Message,
        server_only_msg: &str,
    ) -> Option<serenity::all::GuildId> {
        match msg.guild_id {
            Some(id) => Some(id),
            None => {
                Self::echo(&msg.channel_id, &ctx.http, server_only_msg).await;
                None
            }
        }
    }

    async fn execute_moderation(
        guild_id: serenity::all::GuildId,
        user_id: serenity::all::UserId,
        ctx: &serenity::all::Context,
        msg: &serenity::all::Message,
        confirm_msg: impl Fn(serenity::all::UserId) -> String,
        action: impl FnOnce(serenity::all::GuildId, serenity::all::UserId, Arc<serenity::http::Http>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + 'static,
    ) {
        let http = ctx.http.clone();
        tokio::spawn(async move {
            action(guild_id, user_id, http).await;
        });
        Self::echo(&msg.channel_id, &ctx.http, confirm_msg(user_id)).await;
    }

    async fn execute_kick(ctx: &serenity::all::Context, msg: &serenity::all::Message, message: &str) {
        let Some(guild_id) = Self::require_guild(ctx, msg, "Kick can only be used in a server").await else { return };
        let Some(user_id) = Self::parse_mention(message) else {
            Self::echo(&msg.channel_id, &ctx.http, "Could not find user to kick").await;
            return;
        };
        Self::execute_moderation(
            guild_id,
            user_id,
            ctx,
            msg,
            |user_id| format!("Kicked <@{user_id}>"),
            |guild_id, user_id, http| Box::pin(async move {
                if let Err(e) = guild_id.kick(&http, user_id).await {
                    println!("Error kicking member: {:?}", e);
                }
            }),
        )
        .await;
    }

    async fn execute_ban(ctx: &serenity::all::Context, msg: &serenity::all::Message, message: &str) {
        let Some(guild_id) = Self::require_guild(ctx, msg, "Ban can only be used in a server").await else { return };
        let Some(user_id) = Self::parse_mention(message) else {
            Self::echo(&msg.channel_id, &ctx.http, "Could not find user to ban").await;
            return;
        };
        Self::execute_moderation(
            guild_id,
            user_id,
            ctx,
            msg,
            |user_id| format!("Banned <@{user_id}>"),
            |guild_id, user_id, http| Box::pin(async move {
                if let Err(e) = guild_id.ban(&http, user_id, 0).await {
                    println!("Error banning member: {:?}", e);
                }
            }),
        )
        .await;
    }

    async fn execute_timeout(ctx: &serenity::all::Context, msg: &serenity::all::Message, message: &str) {
        let Some(guild_id) = Self::require_guild(ctx, msg, "Timeout can only be used in a server").await else { return };

        let mut user_id = Self::parse_mention(message);

        // Fall back to looking up member by username / nickname / display name
        if user_id.is_none() {
            if let Some(at_pos) = message.find('@') {
                let username_start = at_pos + 1;
                let username_end = message[username_start..]
                    .find(|c: char| c.is_whitespace() || c == '>' || c == '<')
                    .map(|e| username_start + e)
                    .unwrap_or(message.len());
                let username = &message[username_start..username_end];
                if !username.is_empty() {
                    if let Ok(members) = guild_id.members(&ctx.http, None, None).await {
                        for member in members {
                            let display = member.display_name();
                            let name = &member.user.name;
                            let nick = member.nick.as_deref().unwrap_or("");
                            if display.eq_ignore_ascii_case(username)
                                || name.eq_ignore_ascii_case(username)
                                || nick.eq_ignore_ascii_case(username)
                            {
                                user_id = Some(member.user.id);
                                break;
                            }
                        }
                    }
                }
            }
        }

        let Some(user_id) = user_id else {
            Self::echo(&msg.channel_id, &ctx.http, "Could not find user to timeout").await;
            return;
        };

        // Parse duration - look for duration: or just numbers with mins/hours/seconds
        let mut duration_secs: u64 = 300; // default 5 min
        if let Some(dur_start) = message.to_lowercase().find("duration:") {
            let rest = &message[dur_start + 9..];
            // Try to parse like "5 mins", "5m", "300", etc.
            let words: Vec<&str> = rest.split_whitespace().collect();
            if !words.is_empty() {
                // Try first word as number
                if let Ok(num) = words[0].parse::<u64>() {
                    if words.len() > 1 {
                        let unit = words[1].to_lowercase();
                        if unit.contains("min") {
                            duration_secs = num * 60;
                        } else if unit.contains("hour") || unit.contains("h") {
                            duration_secs = num * 3600;
                        } else if unit.contains("sec") || unit == "s" {
                            duration_secs = num;
                        } else if unit.contains("day") {
                            duration_secs = num * 86400;
                        } else {
                            duration_secs = num;
                        }
                    } else {
                        duration_secs = num;
                    }
                }
            }
        } else {
            // Try to find patterns like "5 mins", "5m"
            let lower = message.to_lowercase();
            // Look for number followed by time unit
            let re_patterns = ["mins?", "minutes?", "hours?", "h ", "seconds?", "secs?", "s ", "days?"];
            for pattern in &re_patterns {
                if let Some(idx) = lower.find(pattern) {
                    // Look backwards for number
                    let before = &lower[..idx];
                    // Find last number
                    let mut num_start = idx;
                    let bytes = before.as_bytes();
                    let mut i = bytes.len() as i32 - 1;
                    while i >= 0 && (bytes[i as usize].is_ascii_digit() || bytes[i as usize] == b' ') {
                        if bytes[i as usize].is_ascii_digit() {
                            num_start = i as usize;
                        } else if bytes[i as usize] == b' ' {
                            break;
                        }
                        i -= 1;
                    }
                    if let Ok(num) = before[num_start..].trim().parse::<u64>() {
                        if pattern.starts_with("min") {
                            duration_secs = num * 60;
                            break;
                        } else if pattern.starts_with("hour") || pattern.starts_with("h ") {
                            duration_secs = num * 3600;
                            break;
                        } else if pattern.starts_with("sec") || pattern.starts_with("s ") {
                            duration_secs = num;
                            break;
                        } else if pattern.starts_with("day") {
                            duration_secs = num * 86400;
                            break;
                        }
                    }
                }
            }
        }

        // Get reason
        let mut reason = "No reason provided".to_string();
        if let Some(reason_start) = message.to_lowercase().find("reason:") {
            reason = message[reason_start + 7..].trim().to_string();
            // Remove duration part if it's in the reason
            if let Some(dur) = reason.to_lowercase().find("duration:") {
                reason = reason[..dur].trim().to_string();
            }
        }

        // Apply timeout
        use chrono::{Utc, Duration as ChronoDuration};
        let until = Utc::now() + ChronoDuration::seconds(duration_secs as i64);

        let mins = duration_secs / 60;
        let secs = duration_secs % 60;
        let duration_str = if mins > 0 && secs == 0 {
            format!("{} min{}", mins, if mins == 1 { "" } else { "s" })
        } else if mins > 0 {
            format!("{}m {}s", mins, secs)
        } else {
            format!("{}s", secs)
        };

        let confirm = move |user_id: serenity::all::UserId| {
            format!("Timed out <@{user_id}> for {duration_str} (reason: {reason})")
        };
        Self::execute_moderation(
            guild_id,
            user_id,
            ctx,
            msg,
            confirm,
            move |guild_id, user_id, http| Box::pin(async move {
                let mut member = match guild_id.member(&http, user_id).await {
                    Ok(m) => m,
                    Err(e) => {
                        println!("Error getting member: {:?}", e);
                        return;
                    }
                };
                if let Err(e) = member.disable_communication_until_datetime(&http, until.into()).await {
                    println!("Error timing out member: {:?}", e);
                }
            }),
        )
        .await;
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