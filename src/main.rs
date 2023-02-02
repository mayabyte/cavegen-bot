use caveripper::{
    parse_seed,
    sublevel::Sublevel,
    layout::Layout,
    render::{
        render_caveinfo,
        LayoutRenderOptions,
        CaveinfoRenderOptions,
        save_image,
        render_layout
    },
    query::{Query, QUERY_HELP, find_matching_layouts_parallel},
    assets::AssetManager
};
use log::{LevelFilter, info};
use poise::{
    Framework,
    FrameworkOptions,
    serenity_prelude::{
        GatewayIntents,
        AttachmentType, self, FutureExt
    },
    command,
    FrameworkBuilder,
    PrefixFrameworkOptions,
    samples::{register_application_commands_buttons}, Event, FrameworkContext, BoxFuture
};
use rayon::{ThreadPoolBuilder, prelude::{IntoParallelIterator, ParallelIterator}};
use simple_logger::SimpleLogger;
use tokio::task::spawn_blocking;
use std::{
    path::PathBuf,
    convert::{TryInto, TryFrom},
    time::{Duration, Instant},
    collections::HashSet
};

const COMMAND_TIMEOUT_S: u64 = 15;

struct Data {}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .with_module_level("tracing", LevelFilter::Off)
        .with_module_level("serenity", LevelFilter::Off)
        .init()?;

    let token = tokio::fs::read_to_string("discord_token.txt").await?
        .trim().to_string();

    let framework: FrameworkBuilder<Data, Error> = Framework::builder()
        .options(FrameworkOptions {
            commands: vec![
                cavegen_register(),
                pspspsps(),
                cavegen(),
                caveinfo(),
                cavegen_query_help(),
                caveinfo_text(),
                cavesearch(),
                cavestats(),
            ],
            prefix_options: PrefixFrameworkOptions {
                prefix: Some("!".to_string()),
                ..Default::default()
            },
            event_handler,
            ..Default::default()
        })
        .token(token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .setup(move |_ctx, _ready, _framework| {
            Box::pin(async move { Ok(Data {}) })
        });

    ThreadPoolBuilder::new().build_global()?;
    AssetManager::init_global("caveripper_assets", ".")?;

    info!("Cavegen Bot started.");
    framework.run().await?;

    info!("Cavegen Bot shutting down.");
    Ok(())
}

fn event_handler<'a>(
    ctx: &'a serenity_prelude::Context,
    event: &'a Event<'a>,
    _framework_context: FrameworkContext<'a, Data, Error>,
    _data: &'a Data
) -> BoxFuture<'a, Result<(), Error>>
{
    async move {
        match event {
            Event::ReactionAdd { add_reaction } => {
                // Allow deletion of messages via a special reaction
                if add_reaction.emoji.unicode_eq("❌") {
                    let message = add_reaction.message(ctx.http.clone()).await?;
                    if add_reaction.user_id.zip(message.interaction.map(|itx| itx.user.id)).map(|(a, b)| a == b).unwrap_or(false) {
                        info!("Deleting message {} due to reaction by user", message.id);
                        add_reaction.message(ctx.http.clone()).await?.delete(ctx).await?;
                    }
                }
                Ok(())
            },
            _ => Ok(())
        }
    }.boxed()
}

/// Generates a sublevel layout image.
#[command(slash_command, user_cooldown = 3)]
async fn cavegen(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `SH-4`, `\"Dream Den 10\"`"] sublevel: String,
    #[description = "8-digit hexadecimal number. Not case sensitive. '0x' is optional."] seed: String,
    #[description = "Draw circles indicating gauge activation range."] #[flag] draw_gauge_range: bool,
    #[description = "Draw map unit grid lines."] #[flag] draw_grid: bool,
    #[description = "Draw score numbers for each map unit."] #[flag] draw_score: bool,
    #[description = "Draw carrying waypoints."] #[flag] draw_waypoints: bool,
) -> Result<(), Error>
{
    info!("Received command `cavegen {sublevel} {seed} {draw_gauge_range} {draw_grid}` from user {}", ctx.author());
    ctx.defer().await?; // Errors will only be visible to the command author

    let sublevel: Sublevel = sublevel.as_str().try_into()?;
    let caveinfo = AssetManager::get_caveinfo(&sublevel)?;
    let seed = parse_seed(&seed)?;

    // Append a random number to the filename to prevent race conditions
    // when the same command is invoked multiple times in rapid succession.
    let uuid: u32 = rand::random();
    let filename = PathBuf::from(format!("output/{}_{}_{:#010X}_{}.png", caveinfo.cave_cfg.game, sublevel.short_name(), seed, uuid));

    // A sub scope is necessary because Layout currently does not implement
    // Send due to use of Rc.
    let layout_image = {
        let layout = Layout::generate(seed, caveinfo);
        render_layout(
            &layout,
            LayoutRenderOptions {
                quickglance: true,
                draw_gauge_range,
                draw_grid,
                draw_score,
                draw_waypoints,
                draw_paths: false, // TODO when this is implemented in Caveripper
            }
        )
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
#[command(slash_command, user_cooldown = 3)]
async fn caveinfo(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `\"Dream Den 10\"`, `ch-cos2`, `newyear:sk1`"] sublevel: String,
) -> Result<(), Error>
{
    info!("Received command `caveinfo {sublevel}` from user {}", ctx.author());
    ctx.defer().await?; // Errors will only be visible to the command author

    let sublevel: Sublevel = sublevel.as_str().try_into()?;
    let caveinfo = AssetManager::get_caveinfo(&sublevel)?;

    // Append a random number to the filename to prevent race conditions
    // when the same command is invoked multiple times in rapid succession.
    let uuid: u32 = rand::random();
    let filename = PathBuf::from(format!("output/{}_{}_caveinfo_{}.png", caveinfo.cave_cfg.game, sublevel.short_name(), uuid));
    let _ = tokio::fs::create_dir("output").await;  // Ensure output directory exists.
    save_image(
        &render_caveinfo(
            caveinfo,
            CaveinfoRenderOptions {
                draw_treasure_info: true,
                draw_waypoint_distances: true,
                draw_waypoints: true,
            }
        )?,
        &filename
    )?;

    ctx.send(|b| {
        b.attachment(AttachmentType::Path(&filename))
    }).await?;

    // Clean up afterwards
    tokio::fs::remove_file(&filename).await?;

    Ok(())
}

/// Shows a text-only caveinfo. Only visible to you.
#[command(slash_command, user_cooldown = 1)]
async fn caveinfo_text(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `\"Dream Den 10\"`, `ch-cos2`, `newyear:sk1`"] sublevel: String,
) -> Result<(), Error>
{
    info!("Received command `caveinfo_text {sublevel}` from user {}", ctx.author());
    ctx.defer_ephemeral().await?;

    let sublevel: Sublevel = sublevel.as_str().try_into()?;
    let caveinfo = AssetManager::get_caveinfo(&sublevel)?;
    ctx.say(format!("{caveinfo}")).await?;

    Ok(())
}

/// Shows a detailed help message for Caveripper's query language. Only visible to you.
#[command(slash_command, user_cooldown = 1)]
async fn cavegen_query_help(ctx: Context<'_>) -> Result<(), Error> {
    info!("Received command `cavegen_query_help` from user {}", ctx.author());
    ctx.defer_ephemeral().await?;
    ctx.say(QUERY_HELP).await?;
    Ok(())
}

/// Search for a layout matching a condition
#[command(slash_command, user_cooldown = 5)]
async fn cavesearch(
    ctx: Context<'_>,
    #[description = "A query string. See Caveripper for details."] query: String,
    #[description = "Search sequentially from the given seed. Searches random seeds if not provided."] start_from: Option<String>,
) -> Result<(), Error>
{
    info!("Received command `cavesearch {query}` from user {}", ctx.author());
    ctx.defer().await?; // Errors will only be visible to the command author

    let query = Query::try_from(query.trim_matches('"'))?;
    let start_from = if let Some(s) = start_from {
        Some(parse_seed(&s)?)
    }
    else { None };

    // Apply the query clauses in sequence, using the result of the previous one's
    // search as the seed source for the following one.
    let query2 = query.clone();
    let result_recv = spawn_blocking(
        move || find_matching_layouts_parallel(
            &query2,
            Some(Instant::now() + Duration::from_secs(COMMAND_TIMEOUT_S)),
            None,
            start_from,
            None
        )
    ).await?;

    if let Ok(seed) = result_recv.recv_timeout(Duration::from_secs(COMMAND_TIMEOUT_S)) {
        let sublevels_in_query: HashSet<&Sublevel> = query.clauses.iter()
            .map(|clause| &clause.sublevel)
            .collect();

        let uuid: u32 = rand::random();  // Collision prevention
        let mut filenames = Vec::new();
        for sublevel in sublevels_in_query.iter() {
            let caveinfo = AssetManager::get_caveinfo(sublevel)?;
            let layout_image = {
                let layout = Layout::generate(seed, caveinfo);
                render_layout(
                    &layout,
                    LayoutRenderOptions {
                        quickglance: true,
                        ..Default::default()
                    }
                )
            }?;
            let filename = PathBuf::from(format!("output/{}_{}_{:#010X}_{}.png", caveinfo.cave_cfg.game, sublevel.short_name(), seed, uuid));
            let _ = tokio::fs::create_dir("output").await;  // Ensure output directory exists.
            save_image(&layout_image, &filename)?;
            filenames.push(filename);
        }

        ctx.send(|b| {
            let mut content = format!("`{seed:#010X}`");
            for (file, sublevel) in filenames.iter().zip(sublevels_in_query.iter()) {
                content.push_str(&format!(" - {}", sublevel.long_name()));
                b.attachment(AttachmentType::Path(file));
            }
            b.content(content);
            b
        }).await?;

        // Clean up afterwards
        for file in filenames.iter() {
            tokio::fs::remove_file(file).await?;
        }
    }
    else {
        ctx.say(format!("Couldn't find matching seed in 10s for query \"{query}\".")).await?;
    }

    Ok(())
}

/// Finds the percentage of seeds that match the given query
#[command(slash_command, user_cooldown = 5)]
async fn cavestats(
    ctx: Context<'_>,
    #[description = "A query string. See Caveripper for details."] #[rest] query: String,
) -> Result<(), Error>
{
    info!("Received command `cavestats {query}` from user {}", ctx.author());
    ctx.defer().await?;  // Errors will only be visible to the command author

    let query = Query::try_from(query.trim_matches('"'))?;
    let num_to_search = 100_000;

    let query2 = query.clone();
    let num_matched = spawn_blocking(move ||
        (0..num_to_search).into_par_iter()
            .filter(|_| {
                let seed: u32 = rand::random();
                query2.matches(seed)
            })
            .count()
    ).await?;

    let percent_matched = (num_matched as f64 / num_to_search as f64) * 100.0;
    ctx.say(format!("**{percent_matched:.03}%** ({num_matched}/{num_to_search}) of layouts match \"{query}\"")).await?;

    Ok(())
}

/// Test command
#[command(slash_command, hide_in_help)]
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
