use caveripper::{parse_seed, sublevel::Sublevel, layout::{render::{render_caveinfo, RenderOptions, save_image, render_layout}, Layout}};
use poise::{Framework, FrameworkOptions, serenity_prelude::{GatewayIntents, AttachmentType}, command, FrameworkBuilder, PrefixFrameworkOptions, samples::register_application_commands_buttons};
use std::{path::PathBuf, convert::TryInto};

struct Data {}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let token = tokio::fs::read_to_string("discord_token.txt").await?.trim().to_string();

    let framework: FrameworkBuilder<_, Error> = Framework::builder()
        .options(FrameworkOptions {
            commands: vec![cavegen_register(), pspspsps(), cavegen(), caveinfo()],
            prefix_options: PrefixFrameworkOptions {
                prefix: Some("!".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .token(token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .user_data_setup(move |_ctx, _ready, _framework| Box::pin(async move { Ok(Data {}) }));

    framework.run().await?;

    Ok(())
}

/// Generates a sublevel layout image.
#[command(prefix_command, slash_command, user_cooldown = 3)]
async fn cavegen(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `SH-4`, `\"Dream Den 10\"`"] sublevel: String,
    #[description = "8-digit hexadecimal number. Not case sensitive. '0x' is optional."] seed: String,
) -> Result<(), Error> 
{
    let sublevel: Sublevel = sublevel.as_str().try_into()?;
    let caveinfo = caveripper::assets::ASSETS.get_caveinfo(&sublevel)?;
    let seed = if seed.eq_ignore_ascii_case("random") {
        rand::random()
    } else {
        parse_seed(&seed)?
    };

    // Append a random number to the filename to prevent race conditions
    // when the same command is invoked multiple times in rapid succession.
    let uuid: u32 = rand::random(); 
    let filename = PathBuf::from(format!("output/{}_{:#010X}_{}.png", sublevel.short_name(), seed, uuid));

    // A sub scope is necessary because Layout currently does not implement
    // Send due to use of Rc.
    let layout_image = {
        let layout = Layout::generate(seed, &caveinfo);
        render_layout(&layout, &RenderOptions {
            quickglance: true,
            ..Default::default()
        })
    }?;

    let _ = tokio::fs::create_dir("output").await;  // Ensure output directory exists.
    save_image(&layout_image, &filename)?;

    ctx.send(|b| {
        b
            .content(format!("{} - `{:#010X}`", sublevel.long_name(), seed))
            .attachment(AttachmentType::Path(&filename))
    }).await?;

    // Clean up afterwards
    tokio::fs::remove_file(&filename).await?;

    Ok(())
}

/// Shows a Caveinfo image.
#[command(prefix_command, slash_command, user_cooldown = 3)]
async fn caveinfo(
    ctx: Context<'_>, 
    #[description = "A sublevel specifier. Examples: `scx1`, `SH-4`, `\"Dream Den 10\"`"] sublevel: String
) -> Result<(), Error> 
{
    let sublevel: Sublevel = sublevel.as_str().try_into()?;
    let caveinfo = caveripper::assets::ASSETS.get_caveinfo(&sublevel)?;

    // Append a random number to the filename to prevent race conditions
    // when the same command is invoked multiple times in rapid succession.
    let uuid: u32 = rand::random(); 
    let filename = PathBuf::from(format!("output/{}_caveinfo_{}.png", sublevel.short_name(), uuid));
    let _ = tokio::fs::create_dir("output").await;  // Ensure output directory exists.
    save_image(&render_caveinfo(&caveinfo, RenderOptions::default())?, &filename)?;

    ctx.send(|b| {
        b.attachment(AttachmentType::Path(&filename))
    }).await?;

    // Clean up afterwards
    tokio::fs::remove_file(&filename).await?;

    Ok(())
}

/// Test command
#[command(prefix_command, slash_command, hide_in_help)]
async fn pspspsps(ctx: Context<'_>) -> Result<(), Error> {
    if &format!("{}#{}", ctx.author().name, ctx.author().discriminator) == "chemical#7290" {
        let path = PathBuf::from("./assets/fast_gbb.gif");
        ctx.send(|b| {
            b.attachment(AttachmentType::Path(&path))
        }).await?;
    }
    Ok(())
}

/// Must be run to register slash commands.
/// Only usable by bot owner, but admin check is put in place for safety anyway.
#[command(prefix_command, required_permissions = "ADMINISTRATOR", hide_in_help)]
async fn cavegen_register(ctx: Context<'_>) -> Result<(), Error> {
    Ok(register_application_commands_buttons(ctx).await?)
}
