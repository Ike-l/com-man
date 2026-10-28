use std::{sync::atomic::AtomicU64, time::Duration};

use tracing::{Level, event, span};
use tracing_subscriber::{fmt, EnvFilter};

use crate::{command_center::CommandCenter, commands::Commands, get_memory_usage::get_memory_usage};

pub mod commands;
pub mod slash_commands;
pub mod command_center;
pub mod get_user_id;
pub mod get_memory_usage;

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

                let author = msg.author.display_name().to_string();
                self.command_center.execute(command, is_admin, ctx, msg).await;
                event!(Level::DEBUG, author = author, is_admin = is_admin, memory_usage = get_memory_usage(), "Memory Usage (MB)");
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
        event!(Level::DEBUG, memory_usage = get_memory_usage(), "Memory Usage (MB)");
    }
}

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() {
    let _profiler = dhat::Profiler::new_heap();
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
    
    tokio::select! {
        result = client.start() => {
            if let Err(err) = result {
                event!(Level::ERROR, error = ?err, "Client Error");
            }
        },

        _ = tokio::signal::ctrl_c() => {
            event!(Level::WARN, "Ctrl+C Received");
        }
    };
}

