use std::{path::PathBuf, sync::Arc};

use crate::{
    cache::{save_cache, WallpaperCache},
    WallpaperData,
};
use image::{io::Reader as ImageReader, ImageBuffer};
use image_transitions::cross_fade;

pub async fn generate_intermediate_wallpapers(
    wallpapers: Arc<WallpaperData>,
    wallpaper_dir: &PathBuf,
    iterations: usize,
) -> anyhow::Result<()> {
    let wallpaper_windows = WallpaperCache::new(wallpapers);

    for (first_wallpaper, second_wallpaper) in wallpaper_windows.iter() {
        let mut folder_path = wallpaper_dir.clone();
        folder_path.push(format!("{}_{}", first_wallpaper, second_wallpaper));

        if folder_path.exists() {
            continue;
        }

        let mut first_file_path = wallpaper_dir.clone();
        first_file_path.push(first_wallpaper);

        let mut second_file_path = wallpaper_dir.clone();
        second_file_path.push(second_wallpaper);

        let first_image = ImageReader::open(first_file_path)?.decode()?.into_rgb16();
        let second_image = ImageReader::open(second_file_path)?.decode()?.into_rgb16();

        let width = first_image.width();
        let height = first_image.height();

        let first_image_raw = first_image.into_raw();
        let second_image_raw = second_image.into_raw();

        let output_buffer = cross_fade(&first_image_raw, &second_image_raw, iterations)?;
        let split_buffer = output_buffer.chunks_exact(first_image_raw.len());

        tokio::fs::create_dir(&folder_path).await?;

        let images = split_buffer
            .map(|raw_output| {
                // TODO: possible remove this allocation
                ImageBuffer::from_raw(width, height, Vec::from(raw_output))
                    .ok_or(anyhow::anyhow!("image buffer not big enough"))
                    .unwrap()
            })
            .collect::<Vec<_>>();

        save_cache(folder_path, images).await?;
    }

    Ok(())
}
