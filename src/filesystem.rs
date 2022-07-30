use crate::{StreamEvent, WallpaperData};
use anyhow::bail;
use notify::{RecommendedWatcher, Watcher};
use std::{collections::BTreeSet, path::PathBuf};
use tokio::sync::mpsc::unbounded_channel;
use tokio_stream::{
    wrappers::{ReadDirStream, UnboundedReceiverStream},
    StreamExt,
};

pub async fn read_directory(directory: &PathBuf) -> anyhow::Result<WallpaperData> {
    if !directory.is_dir() {
        bail!("wallpaper_dir is supposed to be a directory")
    }

    let mut wallpapers = BTreeSet::new();
    let mut file_list = ReadDirStream::new(tokio::fs::read_dir(directory).await?);

    while let Some(Ok(file)) = file_list.next().await {
        if file.file_type().await?.is_dir() {
            continue;
        }

        wallpapers.insert(
            file.file_name().into_string().map_err(|err| {
                anyhow::anyhow!("could not covert file name to string: {:?}", err)
            })?,
        );
    }

    Ok(WallpaperData {
        wallpapers: std::sync::Mutex::new(wallpapers),
    })
}

pub async fn read_images_from_directory(
    directory: &PathBuf,
) -> anyhow::Result<impl Iterator<Item = PathBuf>> {
    if !directory.is_dir() {
        bail!("invalid directory: {}, given", directory.display());
    }

    let image_list = ReadDirStream::new(tokio::fs::read_dir(directory).await?)
        .filter_map(|entry| Some(entry.ok()?.path()))
        .collect::<Vec<_>>()
        .await;

    Ok(image_list.into_iter())
}

pub async fn watch_dir_changes(
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
