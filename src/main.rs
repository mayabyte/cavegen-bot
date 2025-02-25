#![feature(deadline_api)]

use caveripper::{
    assets::{fs_asset_manager::FsAssetManager, AssetManager},
    layout::Layout,
    parse_seed,
    query::{find_matching_layouts_parallel, Query, StructuralQuery},
    render::{
        render_caveinfo, render_layout, save_image, CaveinfoRenderOptions, LayoutRenderOptions,
        RenderHelper,
    },
    sublevel::Sublevel,
};
use log::{info, LevelFilter};
use poise::{
    command,
    samples::register_application_commands_buttons,
    serenity_prelude::{self, AttachmentType, FutureExt, GatewayIntents, GuildId, UserId},
    BoxFuture, Event, Framework, FrameworkBuilder, FrameworkContext, FrameworkOptions,
    PrefixFrameworkOptions,
};
use rayon::{
    prelude::{IntoParallelIterator, ParallelIterator},
    ThreadPoolBuilder,
};
use simple_logger::SimpleLogger;
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::task::spawn_blocking;

const COMMAND_TIMEOUT_S: u64 = 15;

struct Data {
    mgr: &'static FsAssetManager,
    render_helper: &'static RenderHelper<'static, FsAssetManager>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .with_module_level("tracing", LevelFilter::Off)
        .with_module_level("serenity", LevelFilter::Off)
        .init()?;

    let token = tokio::fs::read_to_string("discord_token.txt")
        .await?
        .trim()
        .to_string();

    // The asset manager is leaked because it will live for the entire duration of
    // the bot's runtime and dealing with both async and multithreading for a non-
    // static reference is a huge pain.
    let asset_manager: &'static FsAssetManager = Box::leak(Box::new(FsAssetManager::init()?));
    let render_helper: &'static RenderHelper<FsAssetManager> =
        Box::leak(Box::new(RenderHelper::new(asset_manager)));

    let mut owners = HashSet::new();
    owners.insert(UserId(94645807839457280));

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
            owners,
            command_check: Some(debug_version_check),
            ..Default::default()
        })
        .token(token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .setup(move |_ctx, _ready, _framework| {
            Box::pin(async move {
                Ok(Data {
                    mgr: asset_manager,
                    render_helper,
                })
            })
        });

    ThreadPoolBuilder::new().build_global()?;

    info!("Cavegen Bot started.");
    framework.run().await?;

    info!("Cavegen Bot shutting down.");
    Ok(())
}

fn debug_version_check<'a>(ctx: Context<'_>) -> BoxFuture<'_, Result<bool, Error>> {
    async move {
        let is_debug_server = ctx
            .guild_id()
            .map(|id| id == GuildId(824767720234942536))
            .unwrap_or(false);
        Ok(!(cfg!(debug_assertions) ^ is_debug_server))
    }
    .boxed()
}

fn event_handler<'a>(
    ctx: &'a serenity_prelude::Context,
    event: &'a Event<'a>,
    _framework_context: FrameworkContext<'a, Data, Error>,
    _data: &'a Data,
) -> BoxFuture<'a, Result<(), Error>> {
    async move {
        match event {
            Event::ReactionAdd { add_reaction } => {
                // Allow deletion of messages via a special reaction
                if add_reaction.emoji.unicode_eq("❌") {
                    let message = add_reaction.message(ctx.http.clone()).await?;
                    if add_reaction
                        .user_id
                        .zip(message.interaction.map(|itx| itx.user.id))
                        .map(|(a, b)| a == b)
                        .unwrap_or(false)
                    {
                        info!("Deleting message {} due to reaction by user", message.id);
                        add_reaction
                            .message(ctx.http.clone())
                            .await?
                            .delete(ctx)
                            .await?;
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    .boxed()
}

/// Generates a sublevel layout image.
#[command(slash_command, user_cooldown = 3)]
async fn cavegen(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `SH-4`, `\"Dream Den 10\"`"] sublevel: String,
    #[description = "8-digit hexadecimal number. Not case sensitive. '0x' is optional."]
    seed: String,
    #[description = "Draw circles indicating gauge activation range."]
    #[flag]
    draw_gauge_range: bool,
    #[description = "Draw map unit grid lines."]
    #[flag]
    draw_grid: bool,
    #[description = "Draw score numbers for each map unit."]
    #[flag]
    draw_score: bool,
    #[description = "Draw carrying waypoints."]
    #[flag]
    draw_waypoints: bool,
) -> Result<(), Error> {
    info!(
        "Received command `cavegen {sublevel} {seed} {draw_gauge_range} {draw_grid}` from user {}",
        ctx.author()
    );
    ctx.defer().await?; // Errors will only be visible to the command author

    let mgr = ctx.data().mgr;
    let sublevel = Sublevel::try_from_str(sublevel.as_str(), mgr)?;
    let caveinfo = mgr.load_caveinfo(&sublevel)?;
    let seed = parse_seed(&seed)?;

    // Append a random number to the filename to prevent race conditions
    // when the same command is invoked multiple times in rapid succession.
    let uuid: u32 = rand::random();
    let filename = PathBuf::from(format!(
        "output/{}_{}_{:#010X}_{}.png",
        caveinfo.cave_cfg.game,
        sublevel.short_name(),
        seed,
        uuid
    ));

    // A sub scope is necessary because Layout currently does not implement
    // Send due to use of Rc.
    let layout_image = {
        let layout = Layout::generate(seed, caveinfo);
        render_layout(
            &layout,
            &ctx.data().render_helper,
            LayoutRenderOptions {
                quickglance: true,
                draw_gauge_range,
                draw_grid,
                draw_score,
                draw_waypoints,
                draw_comedown_square: false, // TODO
            },
        )
    }?;

    let _ = tokio::fs::create_dir("output").await; // Ensure output directory exists.
    save_image(&layout_image, &filename)?;

    ctx.send(|b| {
        b.content(format!("{} - `{:#010X}`", sublevel.long_name(), seed))
            .attachment(AttachmentType::Path(&filename))
    })
    .await?;

    // Clean up afterwards
    tokio::fs::remove_file(&filename).await?;

    Ok(())
}

/// Shows a Caveinfo image.
#[command(slash_command, user_cooldown = 3)]
async fn caveinfo(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `\"Dream Den 10\"`, `ch-cos2`, `newyear:sk1`"]
    sublevel: String,
    #[description = "Whether to draw caps and hallways in the unit list. By default they're omitted."]
    #[flag]
    show_halls_and_caps: bool,
) -> Result<(), Error> {
    info!(
        "Received command `caveinfo {sublevel}` from user {}",
        ctx.author()
    );
    ctx.defer().await?; // Errors will only be visible to the command author

    let mgr = ctx.data().mgr;
    let sublevel = Sublevel::try_from_str(sublevel.as_str(), mgr)?;
    let caveinfo = mgr.load_caveinfo(&sublevel)?;

    // Append a random number to the filename to prevent race conditions
    // when the same command is invoked multiple times in rapid succession.
    let uuid: u32 = rand::random();
    let filename = PathBuf::from(format!(
        "output/{}_{}_caveinfo_{}.png",
        caveinfo.cave_cfg.game,
        sublevel.short_name(),
        uuid
    ));
    let _ = tokio::fs::create_dir("output").await; // Ensure output directory exists.
    save_image(
        &render_caveinfo(
            caveinfo,
            &ctx.data().render_helper,
            CaveinfoRenderOptions {
                draw_treasure_info: true,
                draw_waypoint_distances: true,
                draw_waypoints: true,
                hide_small_units: !show_halls_and_caps,
            },
        )?,
        &filename,
    )?;

    ctx.send(|b| b.attachment(AttachmentType::Path(&filename)))
        .await?;

    // Clean up afterwards
    tokio::fs::remove_file(&filename).await?;

    Ok(())
}

/// Shows a text-only caveinfo. Only visible to you.
#[command(slash_command, user_cooldown = 1)]
async fn caveinfo_text(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `\"Dream Den 10\"`, `ch-cos2`, `newyear:sk1`"]
    sublevel: String,
) -> Result<(), Error> {
    info!(
        "Received command `caveinfo_text {sublevel}` from user {}",
        ctx.author()
    );
    ctx.defer_ephemeral().await?;

    let mgr = ctx.data().mgr;
    let sublevel = Sublevel::try_from_str(sublevel.as_str(), mgr)?;
    let caveinfo = mgr.load_caveinfo(&sublevel)?;
    ctx.say(format!("{caveinfo}")).await?;

    Ok(())
}

/// Shows a detailed help message for Caveripper's query language. Only visible to you.
#[command(slash_command, user_cooldown = 1)]
async fn cavegen_query_help(ctx: Context<'_>) -> Result<(), Error> {
    info!(
        "Received command `cavegen_query_help` from user {}",
        ctx.author()
    );
    ctx.defer_ephemeral().await?;
    ctx.say(
        "See https://github.com/mayabyte/caveripper/blob/main/QUERY.md for the query usage guide.",
    )
    .await?;
    Ok(())
}

/// Search for a layout matching a condition
#[command(slash_command, user_cooldown = 3)]
async fn cavesearch(
    ctx: Context<'_>,
    #[description = "A query string. See Caveripper for details."] query: String,
) -> Result<(), Error> {
    info!(
        "Received command `cavesearch {query}` from user {}",
        ctx.author()
    );
    ctx.defer().await?; // Errors will only be visible to the command author

    let mgr = ctx.data().mgr;
    let query = StructuralQuery::try_parse(query.trim_matches('"'), mgr)?;
    let deadline = Instant::now() + Duration::from_secs(COMMAND_TIMEOUT_S);

    // Apply the query clauses in sequence, using the result of the previous one's
    // search as the seed source for the following one.
    let query2 = query.clone();
    let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();

    spawn_blocking(move || {
        find_matching_layouts_parallel(
            &query2,
            mgr,
            Some(deadline),
            Some(1),
            Some(|| {}),
            |seed| {
                let _ = send.send(seed);
            },
        );
    })
    .await?;

    // `send` is moved into the above closure and dropped when `find_matching_layouts_parallel`
    // finishes, so this recv call will return None at that point due to the channel closing.
    if let Some(seed) = recv.recv().await {
        let sublevels_in_query: HashSet<&Sublevel> = query
            .clauses
            .iter()
            .map(|clause| &clause.sublevel)
            .collect();

        let uuid: u32 = rand::random(); // Collision prevention
        let mut filenames = Vec::new();
        for sublevel in sublevels_in_query.iter() {
            let caveinfo = mgr.load_caveinfo(sublevel)?;
            let layout_image = {
                let layout = Layout::generate(seed, caveinfo);
                render_layout(
                    &layout,
                    &ctx.data().render_helper,
                    LayoutRenderOptions {
                        quickglance: true,
                        ..Default::default()
                    },
                )
            }?;
            let filename = PathBuf::from(format!(
                "output/{}_{}_{:#010X}_{}.png",
                caveinfo.cave_cfg.game,
                sublevel.short_name(),
                seed,
                uuid
            ));
            let _ = tokio::fs::create_dir("output").await; // Ensure output directory exists.
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
        })
        .await?;

        // Clean up afterwards
        for file in filenames.iter() {
            tokio::fs::remove_file(file).await?;
        }
    } else {
        ctx.say(format!("Couldn't find any seeds matching \"{query}\"."))
            .await?;
    }

    Ok(())
}

/// Finds the percentage of seeds that match the given query
#[command(slash_command, user_cooldown = 3)]
async fn cavestats(
    ctx: Context<'_>,
    #[description = "A query string. See Caveripper for details."]
    #[rest]
    query: String,
) -> Result<(), Error> {
    info!(
        "Received command `cavestats {query}` from user {}",
        ctx.author()
    );
    ctx.defer().await?; // Errors will only be visible to the command author

    let mgr = ctx.data().mgr;
    let query = StructuralQuery::try_parse(query.trim_matches('"'), mgr)?;
    let num_to_search = 100_000;

    let query2 = query.clone();
    let num_matched = spawn_blocking(move || {
        (0..num_to_search)
            .into_par_iter()
            .filter(|_| {
                let seed: u32 = rand::random();
                query2.matches(seed, mgr)
            })
            .count()
    })
    .await?;

    let percent_matched = (num_matched as f64 / num_to_search as f64) * 100.0;
    ctx.say(format!(
        "**{percent_matched:.03}%** of layouts match \"{query}\""
    ))
    .await?;

    Ok(())
}

/// Test command
#[command(slash_command, hide_in_help)]
async fn pspspsps(ctx: Context<'_>) -> Result<(), Error> {
    if &format!("{}#{}", ctx.author().name, ctx.author().discriminator) == "chemical#7290" {
        let path = PathBuf::from("./assets/fast_gbb.gif");
        ctx.send(|b| b.attachment(AttachmentType::Path(&path)))
            .await?;
    }
    Ok(())
}

/// Must be run to register slash commands.
/// Only usable by bot owner, but admin check is put in place for safety anyway.
#[command(prefix_command, required_permissions = "ADMINISTRATOR", hide_in_help)]
async fn cavegen_register(ctx: Context<'_>) -> Result<(), Error> {
    if cfg!(debug_assertions) {
        ctx.say("Cavegen Bot DEBUG").await?;
    } else {
        ctx.say("Cavegen Bot PRODUCTION").await?;
    }
    Ok(register_application_commands_buttons(ctx).await?)
}
