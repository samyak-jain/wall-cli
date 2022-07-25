use std::{path::PathBuf, sync::Arc};

use anyhow::bail;

use crate::{
    filesystem::read_images_from_directory,
    wallpaper::{WallpaperData, WallpaperSetter},
    WallpaperImage,
};

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
                            .iter()
                            .next()
                            .expect("cannot find element in wallpaper window")
                            .clone(),
                        window
                            .iter()
                            .next()
                            .expect("cannot find element in wallpaper window")
                            .clone(),
                    )
                })
                .collect(),
        }
    }

    pub async fn set_wallpapers(&self, wallpaper_dir: &PathBuf, fps: u16) -> anyhow::Result<()> {
        for pair in &self.wallpaper_pairs {
            let folder_name = format!("{}_{}", pair.0, pair.1);
            let mut folder_path = wallpaper_dir.clone();
            folder_path.push(folder_name);

            if !folder_path.exists() {
                bail!("cache folder {} does not exist", folder_path.display());
            }

            let wallpaper_setter = WallpaperSetter::new();
            wallpaper_setter
                .set_many(read_images_from_directory(&folder_path).await?, fps)
                .await?;
        }

        Ok(())
    }
}

pub async fn save_cache(folder_path: PathBuf, images: Vec<WallpaperImage>) -> anyhow::Result<()> {
    if !folder_path.exists() {
        tokio::fs::create_dir(&folder_path).await?;
    }

    for (index, file) in images.iter().enumerate() {
        let mut file_path = folder_path.clone();
        // TODO: How to handle file extensions?
        file_path.push(index.to_string());

        file.save(file_path)?;
    }

    Ok(())
}
