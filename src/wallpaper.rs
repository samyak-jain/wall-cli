use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::time;
use tokio_stream::StreamExt;
use wall::xlib::Xlib;

use crate::StreamEvent;

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

pub async fn update_wallpapers(
    wallpapers: Arc<WallpaperData>,
    events: impl tokio_stream::Stream<Item = StreamEvent>,
) -> anyhow::Result<()> {
    // Stream needs to be Unpin
    pin_utils::pin_mut!(events);

    while let Some(event) = events.next().await {
        let event_data = event?;
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
            notify::EventKind::Create(notify::event::CreateKind::File) => paths
                .into_iter()
                .filter_map(|entry| get_name(Some(&entry), "add path").ok())
                .for_each(|name| {
                    wallpapers.insert(name);
                }),
            notify::EventKind::Modify(notify::event::ModifyKind::Name(
                notify::event::RenameMode::Both,
            )) => {
                let old_path = get_name(paths.get(0), "old path")?;
                let new_path = get_name(paths.get(1), "new path")?;

                wallpapers.remove(&old_path);
                wallpapers.insert(new_path);
            }
            notify::EventKind::Remove(notify::event::RemoveKind::File) => paths
                .into_iter()
                .filter_map(|entry| get_name(Some(&entry), "remove path").ok())
                .for_each(|name| {
                    wallpapers.remove(&name);
                }),
            _ => return Ok(()),
        };
    }

    Ok(())
}

pub struct WallpaperSetter(Xlib);

impl WallpaperSetter {
    pub fn new() -> Self {
        Self(Xlib::new().expect("could not create xlib client"))
    }

    pub fn set(&self, path: PathBuf) -> anyhow::Result<()> {
        self.0.set(path, None)?;
        Ok(())
    }

    pub async fn set_many(
        &self,
        paths: impl Iterator<Item = PathBuf>,
        fps: usize,
    ) -> anyhow::Result<()> {
        let timeout_in_milliseconds = (1000f32 / fps as f32).floor() as u64;
        let mut interval = time::interval(Duration::from_millis(timeout_in_milliseconds));

        for path in paths {
            interval.tick().await;
            self.set(path)?;
        }

        Ok(())
    }
}