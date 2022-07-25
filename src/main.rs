use cache::WallpaperCache;
use clap::Parser;
use filesystem::{read_directory, watch_dir_changes};
use image::ImageBuffer;
use image::Rgba;
use wallpaper::update_wallpapers;
use wallpaper::WallpaperData;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::time;

use crate::processing::generate_intermediate_wallpapers;

mod cache;
mod filesystem;
mod processing;
mod wallpaper;

#[derive(Parser, Debug)]
struct Args {
    #[clap(value_parser)]
    wallpaper_dir: PathBuf,

    #[clap(short, long, value_parser)]
    cache_dir: Option<PathBuf>,

    #[clap(short, long, value_parser, default_value_t = 144)]
    fps: u16,

    #[clap(short, long, value_parser, default_value_t = 5)]
    transition_time: u8,
}

type StreamEvent = notify::Result<notify::event::Event>;
type WallpaperImage = ImageBuffer<Rgba<u16>, Vec<u16>>;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let cache_dir = args
        .cache_dir
        .or_else(dirs::cache_dir)
        .expect("could not get cache dir");

    let wallpapers = Arc::new(
        read_directory(&args.wallpaper_dir)
            .await
            .expect("could not read the wallpaper directory"),
    );
    let wallpapers_to_update = wallpapers.clone();

    let wallpaper_dir_changes = watch_dir_changes(&args.wallpaper_dir)
        .await
        .expect("could not watch for changes in the wallpaper directoy");

    let mut interval = time::interval(Duration::from_secs(args.transition_time.into()));
    let mut update_task = tokio::spawn(async move {
        update_wallpapers(wallpapers_to_update, wallpaper_dir_changes)
            .await
            .expect("could not update wallpaper")
    });

    let iterations = args.fps * u16::from(args.transition_time);

    let mut wallpapers_to_generate = wallpapers.clone();
    generate_intermediate_wallpapers(wallpapers_to_generate, &cache_dir, iterations as usize)
        .await
        .expect("could not generate intermediate wallpapers");

    let wallpaper_cache = WallpaperCache::new(wallpapers.clone());

    loop {
        wallpapers_to_generate = wallpapers.clone();

        tokio::select! {
            _ = &mut update_task => {
                generate_intermediate_wallpapers(wallpapers_to_generate, &cache_dir, iterations as usize).await.expect("could not generate intermediate wallpapers");
            }
            _ = interval.tick() => {
                wallpaper_cache.set_wallpapers(&args.wallpaper_dir, args.fps).await.expect("could not set wallpapers");
            }
        }
    }
}
