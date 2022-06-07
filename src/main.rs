#![feature(entry_insert)]

mod args;
mod cavegen;
mod cooldown;
mod settings;

use cavegen::{cleanup_output, run_cavegen, run_caveinfo};
use cooldown::{check_cooldown, update_cooldown};
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
    Client,
};
use settings::NUM_TRACKED_COOLDOWNS;
use std::{error::Error, path::PathBuf, sync::Arc, time::SystemTime};

use crate::args::extract_standard_args;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = include_str!("../discord_token.txt");
    let mut client = Client::builder(token)
        .framework(
            StandardFramework::new()
                .configure(|config| {
                    config.prefix("!")
                    .on_mention(Some(824753467133657089.into())) // Respond to commands that @ the bot
                })
                .group(&GENERAL_GROUP)
                .before(before),
        )
        .event_handler(Handler)
        .await?;

    {
        let mut data = client.data.write().await;
        data.insert::<CooldownTimer>(Arc::new(RwLock::new(
            [SystemTime::UNIX_EPOCH; NUM_TRACKED_COOLDOWNS],
        )));
    }

    client.start().await?;

    Ok(())
}

#[group]
#[commands(cavegen, caveinfo, pspspsps)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

/// Cooldown timer for commands that call into Cavegen.
/// Prevents spam and avoids overloading the host machine.
pub struct CooldownTimer;
impl TypeMapKey for CooldownTimer {
    type Value = Arc<RwLock<[SystemTime; NUM_TRACKED_COOLDOWNS]>>;
}

#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    let can_run = check_cooldown(ctx, msg).await;

    if can_run {
        println!(
            "Received command '{}' invoked by '{}'",
            command_name,
            msg.author.tag()
        );
    } else {
        msg.channel_id
            .say(
                &ctx.http,
                format!(
                    ":x: !{} was run too recently. Please wait a while before trying again!",
                    command_name
                ),
            )
            .await
            .unwrap();
    }

    can_run
}

#[command]
async fn cavegen(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = extract_standard_args(args);

    if args.get("help").is_some() {
        msg.channel_id.say(
            &ctx.http,
            "**Usage: `!cavegen <cave specifier> <seed> [optional arguments]`.**\n\
            Cave specifiers can be sublevels: \"SCx3\", \"BK4\", etc., challenge mode sublevels: \"CH3-1\" (the dash is required), \
            or the word \"colossal\" to generate a CC layout.\n\
            Seeds must start with `0x`: `0x1234abcd`.\n\
            Include `+score` in your message to draw score related info.\n\
            Include `+paths` in your message to draw treasure carry paths.\n\
            Include `+jp` in your message to change to JP treasures. PAL doesn't work currently.\n\
            Include `+251` in your message to generate Pikmin 251 caves.\n\
            Include `+newyear` in your message to generate Pikmin 2: New Year caves.\n\
            Include `+pretty` in your message to draw 'pretty' layout images."
        ).await?;
        return Ok(());
    }

    match run_cavegen(&args).await {
        Ok(output_file) => {
            msg.channel_id
                .send_files(&ctx.http, vec![&output_file], |m| {
                    m.content(format!(
                        "{} {}",
                        args.get("cave").unwrap(),
                        args.get("seed").unwrap()
                    ))
                })
                .await?;
            update_cooldown(ctx).await;
            cleanup_output(&output_file).await;
        }
        Err(err) => {
            msg.channel_id.say(&ctx.http, err.to_string()).await?;
            eprintln!("{:#?}", err);
        }
    }

    Ok(())
}

#[command]
async fn caveinfo(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let args = extract_standard_args(args);

    if args.get("help").is_some() {
        msg.channel_id.say(
            &ctx.http,
            "**Usage: `!caveinfo <cave specifier> [optional arguments]`.**\n\
            Cave specifiers can be sublevels: \"SCx3\", \"BK4\", etc., or challenge mode sublevels: \"CH3-1\" (the dash is required).\n\
            Waypoints and spawn points are drawn by default.\n\
            Include `+jp` in your message to change to JP treasures. PAL doesn't work currently.\n\
            Include `+251` in your message to show info for Pikmin 251 caves.\n\
            Include `+newyear` in your message to show info for Pikmin 2: New Year caves."
        ).await?;
        return Ok(());
    }

    match run_caveinfo(&args).await {
        Ok(output_file) => {
            msg.channel_id
                .send_files(&ctx.http, vec![&output_file], |m| {
                    m.content(format!("Caveinfo for {}", args.get("cave").unwrap()))
                })
                .await?;
            update_cooldown(ctx).await;
            cleanup_output(&output_file).await;
        }
        Err(err) => {
            msg.channel_id.say(&ctx.http, err.to_string()).await?;
            eprintln!("{:#?}", err);
        }
    }

    Ok(())
}

#[command]
async fn pspspsps(ctx: &Context, msg: &Message, _args: Args) -> CommandResult {
    if &format!("{}#{}", msg.author.name, msg.author.discriminator) == "chemical#7290" {
        msg.channel_id.send_files(&ctx.http, vec![&PathBuf::from("./assets/fast_gbb.gif")], |m| m).await?;
    }
    Ok(())
}