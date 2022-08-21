use clap::Parser;
use filesystem::{read_directory, validate_directory, watch_dir_changes};
use gstreamer::{BufferList, MessageView};
use tracing::metadata::LevelFilter;
use tracing::{error, info};

use std::path::PathBuf;

use std::time::Duration;

use tokio::time;

use crate::{
    processing::{generate_intermediate_wallpapers, resize_images},
    video::Pipeline,
    window::XConnection,
};

mod filesystem;
mod processing;
mod video;
mod window;

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
    transition_duration_seconds: u8,

    #[clap(short, long, value_parser, default_value_t = 60)]
    seconds_between_transition: u8,

    #[clap(long, value_enum, default_value_t = LogMode::File)]
    log_mode: LogMode,

    #[clap(long, value_enum, default_value_t = LogLevel::Info)]
    level: LogLevel,
}

type StreamEvent = notify::Result<notify::event::Event>;

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

    let wallpapers = read_directory(&args.wallpaper_dir, resolution)
        .await
        .expect("could not read the wallpaper directory");

    info!("loading gstreamer");
    gstreamer::init().expect("unable to init gstreamer");

    info!("starting X server connection");
    let connection = XConnection::new();

    let mut wallpaper_dir_changes = watch_dir_changes(&args.wallpaper_dir)
        .expect("could not watch for changes in the wallpaper directoy");

    info!("started watching wallpaper directory");

    let iterations = args.fps * u16::from(args.transition_duration_seconds);
    info!(
        iterations = iterations,
        "calculated number of itermediate wallpapers for each pair"
    );

    let window_id = connection.create_window().expect("could not create window");
    let mut pipeline = Pipeline::new(window_id, resolution, args.fps)
        .expect("could not create gstreamer pipeline");

    resize_images(&wallpapers, &resized_image_dir, resolution).expect("unable to resize images");

    let mut wallpapers = read_directory(&resized_image_dir, resolution)
        .await
        .expect("could not read the wallpaper directory");

    let mut interval = time::interval(Duration::from_secs(args.seconds_between_transition.into()));
    let mut current_wallpaper_index = 0;

    loop {
        tokio::select! {
            Some(_) = wallpaper_dir_changes.recv() => {
                resize_images(&wallpapers, &resized_image_dir, resolution).expect("unable to resize images");
                wallpapers = read_directory(&resized_image_dir, resolution).await.expect("could not read the wallpaper directory");
            }
            _ = interval.tick() => {
                info!("changing wallpaper");

                let (mut intermediate_buffer, image_len) = generate_intermediate_wallpapers(
                    &wallpapers[current_wallpaper_index % wallpapers.len()],
                    &wallpapers[(current_wallpaper_index + 1) % wallpapers.len()],
                    iterations,
                    resolution,
                )
                .expect("could not generate intermediate buffer");

                let mut pipeline_buffer = Vec::new();
                for mut buf in intermediate_buffer.chunks_exact_mut(image_len) {
                    let mut buffer = gstreamer::buffer::Buffer::with_size(buf.len()).unwrap();
                    let buffer_mut = buffer.get_mut().unwrap();
                    let mut vframe = gstreamer_video::VideoFrameRef::from_buffer_ref_writable(buffer_mut, &pipeline.video_info).unwrap();
                    vframe.plane_data_mut(0).unwrap().swap_with_slice(&mut buf);
                    drop(vframe);
                    drop(buffer_mut);
                    pipeline_buffer.push(buffer);
                }


                pipeline.push_frames(BufferList::from_iter(pipeline_buffer)).expect("could not push frames to pipeline");
                current_wallpaper_index += 1;
            }
            Some(msg) = pipeline.events() => {
                match msg.view() {
                    MessageView::Eos(..) => {},
                    MessageView::Error(err) => {
                        error!(error = err.error().to_string(), "gstreamer error");
                        panic!("gstreamer error");
                    },
                    _ => {}
                }
            }
        }
    }
}
