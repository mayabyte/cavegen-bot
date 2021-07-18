#![feature(entry_insert)]

mod cavegen;
mod cooldown;
mod validators;

use cavegen::{clean_output_dir, invoke_cavegen, normalize_sublevel_id};
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
use std::{collections::HashMap, error::Error, path::PathBuf, sync::Arc, time::SystemTime};
use validators::seed_valid;

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
#[commands(cavegen, caveinfo)]
struct General;

#[group]
#[only_in(dms)]
#[commands(cavegen, caveinfo)]
struct Dms;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

pub struct CooldownTimer;
impl TypeMapKey for CooldownTimer {
    type Value = Arc<RwLock<HashMap<String, SystemTime>>>;
}

#[hook]
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    let can_run = check_cooldown(command_name, ctx, msg).await;

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
async fn cavegen(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    const ERROR_MESSAGE: &str =
        ":x: Usage: `!cavegen <sublevel> <seed>`.\nExample: `!cavegen SCx-7 0x1234ABCD`.";

    // Validate the arguments
    if args.len() != 2 {
        msg.channel_id.say(&ctx.http, ERROR_MESSAGE).await?;
        return Err("!cavegen requires 2 arguments.".into());
    }

    let sublevel: String = if let Some(sublevel) = normalize_sublevel_id(&args.single::<String>()?) {
        sublevel
    } else {
        msg.channel_id.say(&ctx.http, "Unknown or invalid sublevel.").await?;
        return Err("Unknown or invalid sublevel.".into());
    };

    let seed: String = args.single()?;
    if !seed_valid(&seed) {
        msg.channel_id.say(&ctx.http, "Invalid seed.").await?;
        return Err("Invalid seed.".into());
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

    // Update the cooldown timer.
    update_cooldown("cavegen", ctx, msg).await;

    Ok(())
}

#[command]
async fn caveinfo(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    const ERROR_MESSAGE: &str = ":x: Usage: `!caveinfo <sublevel> [-251]`.\nExample: `!cavegen SCx-7`.";

    // Validate the arguments
    if args.len() != 1 {
        msg.channel_id.say(&ctx.http, ERROR_MESSAGE).await?;
        return Err("!caveinfo requires only 1 argument.".into());
    }

    let sublevel: String = if let Some(sublevel) = normalize_sublevel_id(&args.single::<String>()?) {
        sublevel
    } else {
        msg.channel_id.say(&ctx.http, "Unknown or invalid sublevel.").await?;
        return Err("Unknown or invalid sublevel.".into());
    };

    // Now that we know the arguments are good, invoke Cavegen with them
    invoke_cavegen(&format!("cave {} -caveInfoReport", &sublevel))
        .await
        .unwrap();

    // Send the resultant picture to Discord
    let output_filename: PathBuf = format!("./CaveGen/output/!caveinfo/{}.png", &sublevel).into();
    msg.channel_id
        .send_files(&ctx.http, vec![&output_filename], |m| {
            m.content(format!("Caveinfo for {}", sublevel))
        })
        .await?;

    // Clean up after ourselves
    clean_output_dir().await;

    // Update the cooldown timer.
    update_cooldown("caveinfo", ctx, msg).await;

    Ok(())
}
