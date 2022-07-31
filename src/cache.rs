use std::{path::PathBuf, sync::Arc};

use anyhow::bail;
use tracing::info;

use crate::{
    filesystem::read_images_from_directory,
    wallpaper::{WallpaperData, WallpaperSetter},
};

#[derive(Debug)]
pub struct WallpaperCache {
    wallpaper_pairs: Vec<(String, String)>,
}

impl std::ops::Deref for WallpaperCache {
    type Target = Vec<(String, String)>;

    fn deref(&self) -> &Self::Target {
        &self.wallpaper_pairs
    }
}

impl WallpaperCache {
    pub fn new(wallpapers: Arc<WallpaperData>) -> Self {
        let wallpaper_list = wallpapers.get_all();
        let wallpaper_windows = wallpaper_list.windows(2);

        Self {
            wallpaper_pairs: wallpaper_windows
                .map(|window| {
                    (
                        window
                            .get(0)
                            .expect("cannot find element in wallpaper window")
                            .clone(),
                        window
                            .get(1)
                            .expect("cannot find element in wallpaper window")
                            .clone(),
                    )
                })
                .collect(),
        }
    }

    #[tracing::instrument]
    pub async fn set_wallpapers(&self, cache_dir: &PathBuf, fps: u16) -> anyhow::Result<()> {
        for pair in &self.wallpaper_pairs {
            let folder_path = cache_dir.join(format!(
                "{}_{}",
                PathBuf::from(&pair.0)
                    .file_stem()
                    .ok_or(anyhow::anyhow!("cannot get file stem for wallpaper path"))?
                    .to_string_lossy(),
                PathBuf::from(&pair.1)
                    .file_stem()
                    .ok_or(anyhow::anyhow!("cannot get file stem for wallpaper path"))?
                    .to_string_lossy()
            ));

            if !folder_path.exists() {
                bail!("cache folder {} does not exist", folder_path.display());
            }

            info!(
                image_dir = folder_path.to_string_lossy().into_owned(),
                "setting wallpaper from image directory"
            );

            let wallpaper_setter = WallpaperSetter::new();
            wallpaper_setter
                .set_many(read_images_from_directory(&folder_path).await?, fps)
                .await?;
        }

        Ok(())
    }
}
