use crate::StreamEvent;
use anyhow::bail;
use notify::{RecommendedWatcher, Watcher};
use rand::prelude::SliceRandom;
use std::path::PathBuf;
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::{wrappers::ReadDirStream, StreamExt};
use tracing::{debug, info};

#[tracing::instrument]
pub async fn read_directory(
    directory: &PathBuf,
    resolution: (u32, u32),
) -> anyhow::Result<Vec<PathBuf>> {
    if !directory.is_dir() {
        bail!("wallpaper_dir is supposed to be a directory")
    }

    info!(
        wallpaper_dir = directory.to_string_lossy().into_owned(),
        "reading wallpapers"
    );

    let mut file_list = ReadDirStream::new(tokio::fs::read_dir(directory).await?);
    let mut files = Vec::new();

    while let Some(Ok(file)) = file_list.next().await {
        if file.file_type().await?.is_dir() {
            continue;
        }

        let path = file.path();

        let image_dimensions = image::io::Reader::open(&path)?.into_dimensions()?;
        if image_dimensions < resolution {
            info!(
                file_path = format!("{:#?}", path),
                dimensions = format!("{:#?}", image_dimensions),
                "skipping file because dimensions too small"
            );
            continue;
        }

        files.push(path);
    }

    let mut rng = rand::thread_rng();
    files.shuffle(&mut rng);

    debug!(
        wallpaper_list = format!("{:#?}", files),
        "list of wallpapers read from the directory"
    );

    Ok(files)
}

pub fn watch_dir_changes(
    directory: &PathBuf,
) -> anyhow::Result<tokio::sync::mpsc::UnboundedReceiver<StreamEvent>> {
    if !directory.is_dir() {
        bail!("wallpaper_dir is supposed to be a directory")
    }

    let (tx, rx) = unbounded_channel();

    let mut watcher = RecommendedWatcher::new(move |res| {
        tx.send(res).expect("channel is closed");
    })?;
    watcher.watch(&directory, notify::RecursiveMode::Recursive)?;

    Ok(rx)
}

pub async fn validate_directory(
    path: Option<PathBuf>,
    alternative: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    let directory = path
        .or(alternative.and_then(|alt_dir| Some(alt_dir.join("wall-cli"))))
        .ok_or(anyhow::anyhow!("cannot get directory"))?;

    if !directory.exists() {
        tokio::fs::create_dir(&directory).await?;
    }

    Ok(directory)
}
