#![feature(entry_insert)]

mod validators;
mod cavegen;

use serenity::{
    async_trait,
    framework::{
        standard::{
            macros::{command, group, hook},
            Args, CommandResult,
        },
        StandardFramework,
    },
    model::{channel::Message, prelude::Ready},
    prelude::*,
    utils::MessageBuilder,
    Client,
};
use std::{collections::HashMap, error::Error, path::PathBuf, sync::Arc, time::SystemTime};
use validators::{sublevel_valid, seed_valid};
use cavegen::{invoke_cavegen, clean_output_dir};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = env!("DISCORD_TOKEN");
    let mut client = Client::builder(token)
        .framework(
            StandardFramework::new()
                .configure(|config| config.prefix("!"))
                .group(&GENERAL_GROUP)
                .before(before),
        )
        .event_handler(Handler)
        .await?;

    {
        let mut data = client.data.write().await;
        data.insert::<CooldownTimer>(Arc::new(RwLock::new(HashMap::default())));
    }

    client.start().await?;

    Ok(())
}

#[group]
#[allowed_roles("Runner", "Score Attacker", "Discord Mod")]
#[commands(cavegen)]
struct General;

#[group]
#[only_in(dms)]
#[commands(cavegen)]
struct Dms;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

struct CooldownTimer;
impl TypeMapKey for CooldownTimer {
    type Value = Arc<RwLock<HashMap<String, SystemTime>>>;
}

#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    println!("Running command '{}' invoked by '{}'", command_name, msg.author.tag());

    let cooldown_lock = {
        let data_read = ctx.data.read().await;
        data_read.get::<CooldownTimer>().expect("Expected CooldownTimer in TypeMap.").clone()
    };

    {
        // Check when this user last used Cavegen
        let cooldowns = cooldown_lock.read().await;
        if let Some(last_time) = cooldowns.get(&msg.author.tag()) {
            if last_time.elapsed().unwrap().as_secs() < 600u64 {
                msg.channel_id.say(&ctx.http,
                    "You're using Cavegen too often :( \
                    Please wait a little while before trying again."
                ).await.expect("Couldn't send error message");
                return false;
            }
        }

        // Check when the last overall invocation of Cavegen was
        if let Some(last_use) = cooldowns.values().max() {
            if last_use.elapsed().unwrap().as_secs() < 120u64 {
                msg.channel_id.say(&ctx.http,
                    "Cavegen was used too recently! Please wait \
                    a little while before trying again."
                ).await.expect("Couldn't send error message");
                return false;
            }
        }
    }

    // Update the cooldown timer
    {
        let mut cooldowns = cooldown_lock.write().await;
        cooldowns.entry(msg.author.tag()).insert(SystemTime::now());
    }

    true
}

#[command]
async fn cavegen(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Validate the arguments: expects '!cavegen <sublevel> <seed>'
    if args.len() != 2 {
        msg.channel_id.say(&ctx.http, ":x: Usage: `!cavegen <sublevel> <seed>`.").await?;
        return Err("!cavegen requires 2 arguments.".into());
    }

    let mut error_message = MessageBuilder::new();
    let sublevel: String = args.single()?;
    let seed: String = args.single()?;

    if !sublevel_valid(&sublevel) {
        error_message.push_line(
            "Error: couldn't parse sublevel. Make sure there's a \
            dash between the cave and the floor, like 'SCx-6'."
        );
    }
    if !seed_valid(&seed) {
        error_message.push_line(
            "Error: couldn't parse seed. Make sure it starts with \
            '0x' and has exactly 8 characters from 0-9 and A-F afterwards."
        );
    }

    let error_message = error_message.build();
    if error_message.len() > 0 {
        msg.channel_id.say(&ctx.http, error_message).await?;
        return Err("Invalid input to !cavegen".into());
    }

    // Now that we know the arguments are good, invoke Cavegen with them
    invoke_cavegen(&format!("cave {} -seed {}", &sublevel, &seed)).await?;

    // Send the resultant picture to Discord
    let output_filename: PathBuf =
        format!("./CaveGen/output/{}/{}.png", &sublevel, &seed[2..]).into();
    msg.channel_id
        .send_files(&ctx.http, vec![&output_filename], |m| {
            m.content(format!("{} {}", sublevel, seed))
        })
        .await?;

    // Clean up after ourselves
    clean_output_dir().await;

    Ok(())
}
