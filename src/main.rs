use cache::WallpaperCache;
use clap::Parser;
use filesystem::{read_directory, validate_directory, watch_dir_changes};
use image::ImageBuffer;
use tracing::metadata::LevelFilter;
use tracing::{error, info};
use wallpaper::handle_event;
use wallpaper::WallpaperData;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::time;

use crate::processing::generate_intermediate_wallpapers;
use crate::processing::resize_images;

mod cache;
mod filesystem;
mod processing;
mod wallpaper;

#[derive(clap::ValueEnum, Clone, Debug)]
enum LogMode {
    Stdout,
    File,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum LogLevel {
    Debug,
    Info,
}

impl Into<LevelFilter> for LogLevel {
    fn into(self) -> LevelFilter {
        match self {
            LogLevel::Debug => LevelFilter::DEBUG,
            LogLevel::Info => LevelFilter::INFO,
        }
    }
}

#[derive(Parser, Debug)]
struct Args {
    #[clap(value_parser)]
    wallpaper_dir: PathBuf,

    #[clap(short, long, value_parser)]
    cache_dir: Option<PathBuf>,

    #[clap(short, long, value_parser)]
    log_dir: Option<PathBuf>,

    #[clap(short, long, value_parser)]
    resolution: String,

    #[clap(short, long, value_parser, default_value_t = 144)]
    fps: u16,

    #[clap(short, long, value_parser, default_value_t = 5)]
    transition_time: u8,

    #[clap(long, value_enum, default_value_t = LogMode::File)]
    log_mode: LogMode,

    #[clap(long, value_enum, default_value_t = LogLevel::Info)]
    level: LogLevel,
}

type StreamEvent = notify::Result<notify::event::Event>;
type WallpaperImage<'a> = ImageBuffer<image::Rgb<u8>, &'a [u8]>;

#[tokio::main]
#[tracing::instrument]
async fn main() {
    let args = Args::parse();

    let log_dir = validate_directory(args.log_dir, dirs::state_dir())
        .await
        .unwrap();

    match args.log_mode {
        LogMode::Stdout => tracing_subscriber::fmt().with_max_level(args.level).init(),
        LogMode::File => {
            let file_appender = tracing_appender::rolling::daily(&log_dir, "wall-cli.log");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
            tracing_subscriber::fmt()
                .with_max_level(args.level)
                .with_writer(non_blocking)
                .init()
        }
    };

    let cache_dir = validate_directory(args.cache_dir, dirs::cache_dir())
        .await
        .unwrap();

    let generate_image_dir = validate_directory(Some(cache_dir.join("generated")), None)
        .await
        .unwrap();
    let resized_image_dir = validate_directory(Some(cache_dir.join("resized")), None)
        .await
        .unwrap();

    let split_resolution = args.resolution.split("x").collect::<Vec<_>>();
    let resolution = (
        split_resolution
            .get(0)
            .expect("resolution width not provided")
            .parse::<u32>()
            .unwrap(),
        split_resolution
            .get(1)
            .expect("resolution height not provided")
            .parse::<u32>()
            .unwrap(),
    );

    info!(
        log_dir = log_dir.to_string_lossy().into_owned(),
        cache_dir = cache_dir.to_string_lossy().into_owned(),
        wallpaper_dir = args.wallpaper_dir.to_string_lossy().into_owned(),
        "config variables"
    );

    let wallpapers = Arc::new(
        read_directory(&args.wallpaper_dir, resolution)
            .await
            .expect("could not read the wallpaper directory"),
    );

    let mut wallpaper_dir_changes = watch_dir_changes(&args.wallpaper_dir)
        .await
        .expect("could not watch for changes in the wallpaper directoy");

    info!("started watching wallpaper directory");

    let mut interval = time::interval(Duration::from_secs(args.transition_time.into()));

    let iterations = args.fps * u16::from(args.transition_time);
    info!(
        iterations = iterations,
        "calculated number of itermediate wallpapers for each pair"
    );

    let wallpaper_cache = WallpaperCache::new(wallpapers.clone());

    resize_images(
        wallpapers.clone(),
        &args.wallpaper_dir,
        &resized_image_dir,
        resolution,
    )
    .await
    .expect("unable to resize images");

    // if let Err(error) = generate_intermediate_wallpapers(
    //     &wallpaper_cache,
    //     &resized_image_dir,
    //     &generate_image_dir,
    //     iterations as usize,
    //     resolution,
    // )
    // .await
    // {
    //     let stderror: &(dyn std::error::Error) = error.as_ref();
    //     error!(
    //         error = stderror,
    //         "unable to generate intermediate wallpapers"
    //     );
    // };

    loop {
        tokio::select! {
            Some(event_result) = wallpaper_dir_changes.recv() => {
                handle_event(wallpapers.clone(), event_result).await.expect("could not handle file event");
                resize_images(wallpapers.clone(), &args.wallpaper_dir, &resized_image_dir, resolution).await.expect("unable to resize images");
                if let Err(error) = generate_intermediate_wallpapers(&wallpaper_cache, &resized_image_dir, &generate_image_dir, iterations as usize, resolution).await {
                    let stderror: &(dyn std::error::Error) = error.as_ref();
                    error!(error = stderror, "unable to generate intermediate wallpapers");
                };
            }
            _ = interval.tick() => {
                if let Err(error) = wallpaper_cache.set_wallpapers(&generate_image_dir, args.fps).await {
                    let stderror: &(dyn std::error::Error) = error.as_ref();
                    error!(error = stderror, "unable to set wallpaper");
                };
            }
        }
    }
}
