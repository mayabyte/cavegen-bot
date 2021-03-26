use lazy_static::lazy_static;
use maplit::hashmap;
use regex::Regex;
use serenity::{
    async_trait,
    framework::{
        standard::{
            macros::{command, group},
            Args, CommandResult,
        },
        StandardFramework,
    },
    model::{channel::Message, prelude::Ready},
    prelude::*,
    utils::MessageBuilder,
    Client,
};
use std::{collections::HashMap, error::Error, path::PathBuf};
use tokio::process::Command;

lazy_static! {
    static ref CAVES: HashMap<&'static str, u16> = hashmap! {
        "EC" => 2,
        "SCx" => 9,
        "FC" => 8,
        "HoB" => 5,
        "WFG" => 5,
        "SH" => 7,
        "BK" => 7,
        "CoS" => 5,
        "GK" => 6,
        "SR" => 7,
        "SC" => 5,
        "CoC" => 10,
        "HoH" => 15,
        "DD" => 14
    };
    static ref HEX: Regex = Regex::new(r"0x[0-9A-F]{8}").unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let token = env!("DISCORD_TOKEN");
    let mut client = Client::builder(token)
        .framework(
            StandardFramework::new()
                .configure(|config| config.prefix("!"))
                .group(&GENERAL_GROUP),
        )
        .event_handler(Handler)
        .await?;
    client.start().await?;

    Ok(())
}

#[group]
#[commands(cavegen)]
struct General;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

static CAVEGEN_USAGE: &'static str = ":x: Usage: `!cavegen <sublevel> <seed>`.";
#[command]
async fn cavegen(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    // Validate the arguments: expects '!cavegen <sublevel> <seed>'
    if args.len() != 2 {
        msg.channel_id.say(&ctx.http, CAVEGEN_USAGE).await?;
        return Err("!cavegen requires 2 arguments.".into());
    }

    let mut error_message = MessageBuilder::new();
    let sublevel: String = args.single()?;
    let seed: String = args.single()?;

    if !sublevel_valid(&sublevel) {
        error_message.push_line("Error: couldn't parse sublevel. Make sure there's a dash between the cave and the floor, like 'SCx-6'.");
    }
    if !seed_valid(&seed) {
        error_message.push_line("Error: couldn't parse seed. Make sure it starts with '0x' and has exactly 8 characters from 0-9 and A-F afterwards.");
    }

    let error_message = error_message.build();
    if error_message.len() > 0 {
        msg.channel_id.say(&ctx.http, error_message).await?;
        return Err("Invalid input to !cavegen".into());
    }

    // Now that we know the arguments are good, invoke Cavegen with them
    Command::new("java")
        .current_dir("./CaveGen")
        .arg("-jar")
        .arg("CaveGen.jar")
        .arg("cave")
        .arg(&sublevel)
        .arg("-seed")
        .arg(&seed)
        .spawn()?
        .wait()
        .await?;

    // Send the resultant picture to Discord
    let output_filename: PathBuf =
        format!("./CaveGen/output/{}/{}.png", &sublevel, &seed[2..]).into();
    msg.channel_id
        .send_files(&ctx.http, vec![&output_filename], |m| {
            m.content(format!("{} {}", sublevel, seed))
        })
        .await?;

    Ok(())
}

fn sublevel_valid(sublevel: &str) -> bool {
    if let Some((cave, level)) = sublevel.split_once('-') {
        CAVES
            .get(cave)
            .and_then(|max_floors| Some(level.parse::<u16>().ok()? <= *max_floors))
            .unwrap_or(false)
    } else {
        false
    }
}

fn seed_valid(seed: &str) -> bool {
    HEX.is_match(seed) && seed.len() == 10
}
