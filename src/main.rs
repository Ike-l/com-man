use serenity::{all::{Message, Ready}, async_trait, prelude::{Client, Context, EventHandler, GatewayIntents}};


struct App;

#[async_trait]
impl EventHandler for App {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(err) = msg.channel_id.say(&ctx.http, "Pong!").await {
                println!("Error Sending Message: {err:?}");
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}


#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let token = std::env::var("DISCORD_TOKEN").expect(".env file in crate directory with 'DISCORD_TOKEN=_'");

    let intents = GatewayIntents::all();

    let mut client = Client::builder(
        &token, 
        intents
    ).event_handler(App).await.expect("Error with building the client");

    if let Err(err) = client.start().await {
        println!("Client Error: {err:?}");
    }
}
