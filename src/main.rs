use std::{sync::atomic::AtomicU64, time::Duration};

use crate::{command_center::CommandCenter, commands::{CommandError, Commands}};

pub mod commands;
pub mod command_center;

struct App {
    command_center: CommandCenter
}

#[serenity::async_trait]
impl serenity::prelude::EventHandler for App {
    async fn message(&self, ctx: serenity::all::Context, msg: serenity::all::Message) {
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
                
                self.command_center.execute(command, is_admin, ctx, msg).await;
            },
            Err(err) => {
                match err {
                    CommandError::NoPrefix => println!("Command has no Prefix"),
                    CommandError::NotFound => println!("Command Not Found"),
                    CommandError::ParsingDuration => println!("Failed to parse given duration"),
                    CommandError::ParsingThreshold => println!("Failed to parse given threshold"),
                }
            }
        }
    }

    async fn ready(&self, _: serenity::all::Context, ready: serenity::all::Ready) {
        println!("{} is connected!", ready.user.name);
    }
}


#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let token = std::env::var("DISCORD_TOKEN").expect(".env file in crate directory with 'DISCORD_TOKEN=_'");

    let intents = serenity::prelude::GatewayIntents::all();

    let app = App {
        command_center: CommandCenter {
            vote_duration: serenity::prelude::Mutex::new(Duration::from_secs(60)),
            vote_threshold: AtomicU64::new(5),
        }
    };

    let mut client = serenity::prelude::Client::builder(
        &token, 
        intents
    ).event_handler(app).await.expect("Error with building the client");

    if let Err(err) = client.start().await {
        println!("Client Error: {err:?}");
    }
}
