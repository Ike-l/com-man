use std::{sync::atomic::AtomicU64, time::Duration};

use crate::{command_center::CommandCenter, commands::{CommandError, Commands}};

pub mod commands;
pub mod slash_commands;
pub mod command_center;

struct App {
    command_center: CommandCenter
}

#[serenity::async_trait]
impl serenity::prelude::EventHandler for App {
    async fn message(&self, ctx: serenity::all::Context, msg: serenity::all::Message) {
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
            vote_threshold: AtomicU64::new(1),
            reaction_emoji: serenity::all::ReactionType::Unicode("👍".to_string())
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

async fn get_user_id(
    guild_id: &serenity::all::GuildId,
    message: &str,
    ctx: &serenity::all::Context
) -> Option<serenity::all::UserId> {
    let start = message.find('@')? + 1;
    let end = message[start..].find(' ').unwrap_or(message.len() - 1) - 1;
    
    let starts =  message.find('<').is_some_and(|index| index + 2 == start);
    let ends = message[start..].find('>').is_some_and(|index| start + index - 1 == end);

    if starts && ends {
        let id = &message[start..=end];
        let id = id.parse::<u64>().ok()?;
        let user_id = serenity::all::UserId::new(id);
        return Some(user_id);
    }

    let user_name = &message[start..end + 1];
    let members = guild_id.members(&ctx.http, None, None).await.ok()?;
    for member in members {
        let display = member.display_name();
        let name = &member.user.name;
        let nick = member.nick.as_deref().unwrap_or("");

        if display.eq_ignore_ascii_case(user_name) || 
            name.eq_ignore_ascii_case(user_name) || 
            nick.eq_ignore_ascii_case(user_name) {
            let user_id = member.user.id;
            return Some(user_id);
        }
    }

    None
}