use caveripper::{
    parse_seed, 
    sublevel::Sublevel, 
    layout::{
        render::{
            render_caveinfo, 
            LayoutRenderOptions,
            CaveinfoRenderOptions, 
            save_image, 
            render_layout
        }, Layout
    }, 
    query::Query, 
    search::find_matching_layouts_parallel, 
    assets::AssetManager
};
use log::{LevelFilter, info};
use poise::{
    Framework, 
    FrameworkOptions, 
    serenity_prelude::{
        GatewayIntents, 
        AttachmentType
    }, 
    command, 
    FrameworkBuilder, 
    PrefixFrameworkOptions, 
    samples::{register_application_commands_buttons}
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

    let framework: FrameworkBuilder<_, Error> = Framework::builder()
        .options(FrameworkOptions {
            commands: vec![
                cavegen_register(), 
                pspspsps(), 
                cavegen(), 
                caveinfo(), 
                cavesearch(),
                cavestats(),
            ],
            prefix_options: PrefixFrameworkOptions {
                prefix: Some("!".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .token(token)
        .intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
        .user_data_setup(move |_ctx, _ready, _framework| Box::pin(async move { Ok(Data {}) }));

    ThreadPoolBuilder::new().num_threads(16).build_global()?;
    AssetManager::init_global("caveripper_assets", ".")?;

    info!("Cavegen Bot started.");
    framework.run().await?;

    info!("Cavegen Bot shutting down.");
    Ok(())
}

/// Generates a sublevel layout image.
#[command(slash_command, user_cooldown = 3)]
async fn cavegen(
    ctx: Context<'_>,
    #[description = "A sublevel specifier. Examples: `scx1`, `SH-4`, `\"Dream Den 10\"`"] sublevel: String,
    #[description = "8-digit hexadecimal number. Not case sensitive. '0x' is optional."] seed: String,
    #[description = "Draw circles indicating gauge activation range."] #[flag] draw_gauge_range: bool,
    #[description = "Draw map unit grid lines."] #[flag] draw_grid: bool,
) -> Result<(), Error> 
{
    info!("Received command `cavegen {sublevel} {seed} {draw_gauge_range} {draw_grid}` from user {}", ctx.author());

    let sublevel: Sublevel = sublevel.as_str().try_into()?;
    let caveinfo = AssetManager::get_caveinfo(&sublevel)?;
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
        let layout = Layout::generate(seed, caveinfo);
        render_layout(
            &layout, 
            LayoutRenderOptions {
                quickglance: true,
                draw_gauge_range,
                draw_grid,
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

    let sublevel: Sublevel = sublevel.as_str().try_into()?;
    let caveinfo = AssetManager::get_caveinfo(&sublevel)?;

    // Append a random number to the filename to prevent race conditions
    // when the same command is invoked multiple times in rapid succession.
    let uuid: u32 = rand::random(); 
    let filename = PathBuf::from(format!("output/{}_caveinfo_{}.png", sublevel.short_name(), uuid));
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

/// Search for a layout matching a condition
#[command(prefix_command, user_cooldown = 10, broadcast_typing)]
async fn cavesearch(
    ctx: Context<'_>,
    #[description = "A query string. See Caveripper for details."] #[rest] query: String,
) -> Result<(), Error>
{
    info!("Received command `cavesearch {query}` from user {}", ctx.author());

    let query = Query::try_from(query.trim_matches('"'))?;

    // Apply the query clauses in sequence, using the result of the previous one's
    // search as the seed source for the following one.
    let query2 = query.clone();
    let result_recv = spawn_blocking(move || find_matching_layouts_parallel(&query2, Some(Instant::now() + Duration::from_secs(10)), None, None)).await?;

    if let Ok(seed) = result_recv.recv_timeout(Duration::from_secs(10)) {
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
            let filename = PathBuf::from(format!("output/{}_{:#010X}_{}.png", sublevel.short_name(), seed, uuid));
            let _ = tokio::fs::create_dir("output").await;  // Ensure output directory exists.
            save_image(&layout_image, &filename)?;
            filenames.push(filename);
        }

        ctx.say(format!("Seed `{:#010X}` matches query \"{}\".", seed, query)).await?;
        for (file, sublevel) in filenames.iter().zip(sublevels_in_query.iter()) {
            ctx.send(|b| {
                b.content(format!("{} - `{:#010X}`", sublevel.long_name(), seed));
                b.attachment(AttachmentType::Path(file));
                b
            }).await?;
        }

        // Clean up afterwards
        for file in filenames.iter() {
            tokio::fs::remove_file(file).await?;
        }
    }
    else {
        ctx.say(format!("Couldn't find matching seed in 10s for query \"{}\".", query)).await?;
    }

    Ok(())
}

/// Finds the percentage of seeds that match the given query
#[command(prefix_command, user_cooldown = 10, broadcast_typing)]
async fn cavestats(
    ctx: Context<'_>,
    #[description = "A query string. See Caveripper for details."] #[rest] query: String,
) -> Result<(), Error> 
{
    info!("Received command `cavestats {query}` from user {}", ctx.author());

    let query = Query::try_from(query.trim_matches('"'))?;
    let num_to_search = 10_000;

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
    ctx.say(format!("**{percent_matched:.03}%** ({num_matched}/{num_to_search}) of checked seeds matched the condition \"{query}\"")).await?;

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
