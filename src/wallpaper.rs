use core::fmt;
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::time;
use tracing::info;
use wall::xlib::Xlib;

use crate::StreamEvent;

#[derive(Debug)]
pub struct WallpaperData {
    pub wallpapers: Mutex<BTreeSet<String>>,
}

impl WallpaperData {
    fn insert(&self, value: String) {
        let mut unlocked_wallpapers = self.wallpapers.lock().expect("unable to get mutex lock");
        unlocked_wallpapers.insert(value);
    }

    fn remove(&self, value: &String) {
        let mut unlocked_wallpapers = self.wallpapers.lock().expect("unable to get mutex lock");
        unlocked_wallpapers.remove(value);
    }

    pub fn get_all(&self) -> Vec<String> {
        let unlocked_wallpapers = self.wallpapers.lock().expect("unable to get mutex lock");
        unlocked_wallpapers.iter().cloned().collect()
    }
}

#[tracing::instrument]
pub async fn handle_event(
    wallpapers: Arc<WallpaperData>,
    event: StreamEvent,
) -> anyhow::Result<()> {
    let event_data = event?;
    dbg!(event_data.clone());
    let paths = event_data.paths;

    let get_name = |path: Option<&PathBuf>, path_type: &str| -> anyhow::Result<String> {
        Ok(path
            .ok_or(anyhow::anyhow!("no {} available", path_type))?
            .file_name()
            .ok_or(anyhow::anyhow!("no filename available for {}", path_type))?
            .to_string_lossy()
            .into_owned())
    };

    match event_data.kind {
        notify::EventKind::Create(notify::event::CreateKind::File) => {
            paths
                .into_iter()
                .filter_map(|entry| get_name(Some(&entry), "add path").ok())
                .for_each(|name| {
                    info!(path = name, "new file added");
                    wallpapers.insert(name);
                });
            return Ok(());
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Name(
            notify::event::RenameMode::Both,
        )) => {
            let old_path = get_name(paths.get(0), "old path")?;
            let new_path = get_name(paths.get(1), "new path")?;

            info!(old_path = old_path, new_path = new_path, "renaming");

            wallpapers.remove(&old_path);
            tokio::fs::remove_file(&paths[0]).await?;

            wallpapers.insert(new_path);

            return Ok(());
        }
        notify::EventKind::Remove(notify::event::RemoveKind::File) => {
            paths
                .into_iter()
                .filter_map(|entry| {
                    info!(file = entry.to_string_lossy().into_owned(), "removing file");
                    std::fs::remove_file(&entry).ok()?;
                    get_name(Some(&entry), "remove path").ok()
                })
                .for_each(|name| {
                    wallpapers.remove(&name);
                });
            return Ok(());
        }
        _ => {}
    };

    Ok(())
}

pub struct WallpaperSetter(Xlib);

impl fmt::Debug for WallpaperSetter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wallpaper setter with Xlib client")
    }
}

impl WallpaperSetter {
    pub fn new() -> Self {
        Self(Xlib::new().expect("could not create xlib client"))
    }

    pub fn set(&self, path: PathBuf) -> anyhow::Result<()> {
        self.0.set(path, None)?;
        Ok(())
    }

    #[tracing::instrument]
    pub async fn set_many(
        &self,
        paths: impl Iterator<Item = PathBuf> + std::fmt::Debug,
        fps: u16,
    ) -> anyhow::Result<()> {
        // TODO: calculate time taken to set the wallpaper and subtract it from timeout
        let timeout_in_milliseconds = (1000f32 / fps as f32).floor() as u64;
        let mut interval = time::interval(Duration::from_millis(timeout_in_milliseconds));

        info!("setting new wallpapers");

        for path in paths {
            interval.tick().await;
            self.set(path)?;
        }

        Ok(())
    }
}
