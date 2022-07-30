use cache::WallpaperCache;
use clap::Parser;
use filesystem::{read_directory, watch_dir_changes};
use image::ImageBuffer;
use image::Rgba;
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

#[derive(Parser, Debug)]
struct Args {
    #[clap(value_parser)]
    wallpaper_dir: PathBuf,

    #[clap(short, long, value_parser)]
    data_dir: Option<PathBuf>,

    #[clap(short, long, value_parser)]
    cache_dir: Option<PathBuf>,

    #[clap(short, long, value_parser)]
    resolution: String,

    #[clap(short, long, value_parser, default_value_t = 144)]
    fps: u16,

    #[clap(short, long, value_parser, default_value_t = 5)]
    transition_time: u8,
}

type StreamEvent = notify::Result<notify::event::Event>;
type WallpaperImage<'a> = ImageBuffer<image::Rgb<u8>, &'a [u8]>;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let cache_dir = args
        .cache_dir
        .or_else(dirs::cache_dir)
        .expect("could not get cache dir");

    let data_dir = args
        .data_dir
        .or_else(dirs::data_dir)
        .expect("could not get cache dir");

    let split_resolution = args.resolution.split("x").collect::<Vec<_>>();
    let resolution = (
        split_resolution.get(0).unwrap().parse::<u32>().unwrap(),
        split_resolution.get(1).unwrap().parse::<u32>().unwrap(),
    );

    let wallpapers = Arc::new(
        read_directory(&args.wallpaper_dir)
            .await
            .expect("could not read the wallpaper directory"),
    );
    dbg!(wallpapers.clone());
    // let wallpapers_to_update = wallpapers.clone();

    let mut wallpaper_dir_changes = watch_dir_changes(&args.wallpaper_dir)
        .await
        .expect("could not watch for changes in the wallpaper directoy");

    let mut interval = time::interval(Duration::from_secs(args.transition_time.into()));
    // let mut update_task = tokio::spawn(async move {
    //     handle_event(wallpapers_to_update, wallpaper_dir_changes)
    //         .await
    //         .expect("could not update wallpaper")
    // });

    let iterations = args.fps * u16::from(args.transition_time);

    let wallpaper_cache = WallpaperCache::new(wallpapers.clone());

    dbg!(wallpaper_cache.clone());

    resize_images(
        wallpapers.clone(),
        &args.wallpaper_dir,
        &data_dir,
        resolution,
    )
    .await
    .expect("unable to resize images");

    dbg!("resized");

    generate_intermediate_wallpapers(&wallpaper_cache, &data_dir, &cache_dir, iterations as usize)
        .await
        .expect("could not generate intermediate wallpapers");

    dbg!("generated");

    loop {
        tokio::select! {
            Some(event_result) = wallpaper_dir_changes.recv() => {
                handle_event(wallpapers.clone(), event_result).await.unwrap();
                generate_intermediate_wallpapers(&wallpaper_cache, &data_dir, &cache_dir, iterations as usize).await.expect("could not generate intermediate wallpapers");
            }
            _ = interval.tick() => {
                wallpaper_cache.set_wallpapers(&cache_dir, args.fps).await.expect("could not set wallpapers");
            }
        }
    }
}
