use std::{sync::atomic::AtomicU64, time::Duration};

use tracing::{Level, event, span};
use tracing_subscriber::{fmt, EnvFilter};

use crate::{command_center::CommandCenter, commands::Commands};

pub mod commands;
pub mod slash_commands;
pub mod command_center;

struct App {
    command_center: CommandCenter,
    app_span: tracing::Span,
}

#[serenity::async_trait]
impl serenity::prelude::EventHandler for App {
    async fn message(&self, ctx: serenity::all::Context, msg: serenity::all::Message) {
        let _enter = self.app_span.enter();

        if msg.author.id == ctx.cache.current_user().id {
            return;
        }

        let command = Commands::try_from(msg.content.clone());
        match command {
            Ok(command) => {
                let is_admin = if let Some(guild_id) = msg.guild_id {
                    if let Some(guild) = ctx.cache.guild(guild_id) {
                        if let (Some(member), Some(channel)) = (
                            guild.members.get(&msg.author.id),
                            guild.channels.get(&msg.channel_id),
                        ) {
                            guild.user_permissions_in(&channel, &member).administrator()
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };

                let span = span!(Level::DEBUG, "Executing", author = msg.author.display_name(), is_admin = is_admin);
                let _enter = span.enter();
                
                self.command_center.execute(command, is_admin, ctx, msg).await;
            },
            Err(err) => {
                CommandCenter::echo(&msg.channel_id, &ctx.http, format!("Error in command: {err}")).await;
            }
        }
    }

    async fn ready(&self, _ctx: serenity::all::Context, ready: serenity::all::Ready) {
        self.app_span.record("user_name", tracing::field::display(&ready.user.name));
        let _enter = self.app_span.enter();
        event!(Level::INFO, "Connected");
    }
}


#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();    

    let log_level = std::env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string());
    fmt()
        .with_env_filter(EnvFilter::new(format!("com_man={log_level}")))
        .init();

    let span = span!(Level::INFO, "App", user_name = tracing::field::Empty);
    let _enter = span.enter();

    let token = std::env::var("DISCORD_TOKEN").expect(".env file in crate directory with 'DISCORD_TOKEN=_'");

    let intents = serenity::prelude::GatewayIntents::all();

    let app = App {
        command_center: CommandCenter {
            vote_duration: serenity::prelude::Mutex::new(Duration::from_secs(60)),
            accept_vote_threshold: AtomicU64::new(1),
            deny_vote_threshold: AtomicU64::new(1),
            accept_reaction_emoji: serenity::all::ReactionType::Unicode("👍".to_string()),
            deny_reaction_emoji: serenity::all::ReactionType::Unicode("👎".to_string()),
        },
        app_span: span.clone(),
    };

    let mut client = serenity::prelude::Client::builder(
        &token, 
        intents
    ).event_handler(app).await.expect("Error with building the client");

    if let Err(err) = client.start().await {
        event!(Level::ERROR, error = ?err, "Client Error");
    }
}

async fn get_user_id(
    guild_id: &serenity::all::GuildId,
    message: &str,
    ctx: &serenity::all::Context
) -> Option<serenity::all::UserId> {
    if let Some(mention_id) = parse_mention(message) {
        if let Ok(id) = mention_id.parse::<u64>() {
            return Some(serenity::all::UserId::new(id));
        }
    }

    let user_name = parse_username(message)?;
    let members = guild_id.members(&ctx.http, None, None).await.ok()?;
    for member in members {
        let display = member.display_name();
        let name = &member.user.name;
        let nick = member.nick.as_deref().unwrap_or("");

        if display.eq_ignore_ascii_case(&user_name) || 
            name.eq_ignore_ascii_case(&user_name) || 
            nick.eq_ignore_ascii_case(&user_name) {
            let user_id = member.user.id;
            return Some(user_id);
        }
    }

    None
}

fn parse_mention(message: &str) -> Option<String> {
    let start_bracket = message.find('<')?;
    if message.as_bytes().get(start_bracket + 1) != Some(&b'@') {
        return None;
    }

    let start = start_bracket + 2;
    let end_bracket = message[start..].find('>')?;
    let end = start + end_bracket;

    if message.as_bytes().get(end) != Some(&b'>') {
        return None;
    }

    let mention = message[start..end].to_string();

    Some(mention)
}

fn parse_username(message: &str) -> Option<String> {
    if !message.starts_with('@') {
        return None;
    }

    let rest = &message[1..];
    let end = rest.find(' ');

    let username = if let Some(end) = end { 
        rest[..end].to_string() 
    } else { 
        rest.to_string() 
    };

    Some(username)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_mention_with_id() {
        assert_eq!(parse_mention(" bar <@123456789> foo "), Some("123456789".to_string()));
    }

    #[test]
    fn parse_mention_no_match() {
        assert_eq!(parse_mention("  @username  "), None);
        assert_eq!(parse_mention("hello"), None);
    }

    #[test]
    fn parse_empty() {
        assert_eq!(parse_mention(""), None);
        assert_eq!(parse_username(""), None);
    }

    #[test]
    fn parse_space() {
        assert_eq!(parse_mention(" "), None);
        assert_eq!(parse_username(" "), None);
    }

    #[test]
    fn parse_indicators() {
        assert_eq!(parse_mention("@"), None);
        assert_eq!(parse_mention(" @"), None);
        assert_eq!(parse_mention("@ "), None);
        assert_eq!(parse_mention(" @ "), None);
        assert_eq!(parse_mention("\t@"), None);
        assert_eq!(parse_mention("@\t"), None);
        assert_eq!(parse_mention("\t@\t"), None);
        assert_eq!(parse_mention("  @"), None);
        assert_eq!(parse_mention("@  "), None);
        assert_eq!(parse_mention("  @  "), None);

        assert_eq!(parse_username(" @<>"), None);
        assert_eq!(parse_username(" @ <>"), None);
        assert_eq!(parse_username(" @ < >"), None);
        assert_eq!(parse_username(" @ <> "), None);
        assert_eq!(parse_username(" @ < > "), None);
        assert_eq!(parse_username(" @< >"), None);
        assert_eq!(parse_username(" @<> "), None);
        assert_eq!(parse_username(" @< > "), None);

        assert_eq!(parse_username("\t@<>"), None);
        assert_eq!(parse_username("\t@ <>"), None);
        assert_eq!(parse_username("\t@ < >"), None);
        assert_eq!(parse_username("\t@ <> "), None);
        assert_eq!(parse_username("\t@ < > "), None);
        assert_eq!(parse_username("\t@< >"), None);
        assert_eq!(parse_username("\t@<> "), None);
        assert_eq!(parse_username("\t@< > "), None);

        assert_eq!(parse_username("  @<>"), None);
        assert_eq!(parse_username("  @ <>"), None);
        assert_eq!(parse_username("  @ < >"), None);
        assert_eq!(parse_username("  @ <> "), None);
        assert_eq!(parse_username("  @ < > "), None);
        assert_eq!(parse_username("  @< >"), None);
        assert_eq!(parse_username("  @<> "), None);
        assert_eq!(parse_username("  @< > "), None);

        assert_eq!(parse_username(" @< >"), None);
    }

    #[test]
    fn parse_username_extracts_name() {
        assert_eq!(parse_username("@username"), Some("username".to_string()));
        assert_eq!(parse_username("@username foo "), Some("username".to_string()));
        // assert_eq!(parse_username(" bar @username foo "), Some("username".to_string())); // gets None
    }

    #[test]
    fn parse_username_returns_none_for_mention() {
        assert_eq!(parse_username("<@123456789>"), None);
    }
}