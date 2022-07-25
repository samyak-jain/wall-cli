use clap::Parser;
use filesystem::{read_directory, watch_dir_changes};
use wallpaper::update_wallpapers;
use wallpaper::WallpaperData;

use std::sync::Arc;
use std::time::Duration;
use std::{env, path::PathBuf};

use tokio::time;

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

    #[clap(short, long, value_parser, default_value_t = 144)]
    fps: u16,

    #[clap(short, long, value_parser, default_value_t = 5)]
    transition_time: u8,
}

type StreamEvent = notify::Result<notify::event::Event>;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let data_dir = args
        .data_dir
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| env::current_dir().expect("could not get current working directory"));

    let cache_dir = args
        .cache_dir
        .or_else(dirs::cache_dir)
        .expect("could not get cache dir");

    let wallpapers = Arc::new(
        read_directory(&args.wallpaper_dir)
            .await
            .expect("could not read the wallpaper directory"),
    );
    let shared_wallpapers = wallpapers.clone();

    let wallpaper_dir_changes = watch_dir_changes(&args.wallpaper_dir)
        .await
        .expect("could not watch for changes in the wallpaper directoy");

    let mut interval = time::interval(Duration::from_secs(args.transition_time.into()));
    let mut update_task = tokio::spawn(async move {
        update_wallpapers(shared_wallpapers, wallpaper_dir_changes)
            .await
            .expect("could not update wallpaper")
    });

    loop {
        tokio::select! {
            _ = &mut update_task => {}
            _ = interval.tick() => {
            }
        }
    }
}
