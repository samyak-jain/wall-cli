use std::{path::PathBuf, sync::Arc};

use crate::{cache::WallpaperCache, wallpaper::WallpaperData, WallpaperImage};
use anyhow::{bail, Ok};
use image::{io::Reader as ImageReader, ImageBuffer};
use image_transitions::cross_fade;
use rayon::{
    iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
    slice::ParallelSlice,
};
use tracing::{debug, error, info};

// Resize all images in the wallpaper_directory into the provided resolution in parallel
#[tracing::instrument]
pub async fn resize_images(
    wallpapers: Arc<WallpaperData>,
    wallpaper_dir: &PathBuf,
    dest_dir: &PathBuf,
    resolution: (u32, u32),
) -> anyhow::Result<()> {
    let image_names = wallpapers.get_all();

    image_names
        .par_iter()
        .try_for_each(|image_name| -> anyhow::Result<()> {
            let image_path = wallpaper_dir.join(image_name);
            let dest_image_path = dest_dir.join(image_name);

            if dest_image_path.exists() {
                return Ok(());
            }

            debug!(image_name = image_name, "resizing image");

            let image = ImageReader::open(image_path)?.decode()?;
            let new_image = image.resize_to_fill(
                resolution.0,
                resolution.1,
                image::imageops::FilterType::Lanczos3,
            );

            new_image.save(dest_image_path)?;

            Ok(())
        })?;

    Ok(())
}

// Generate intermediate images for the cross fade animation and store it in the cache folder
#[tracing::instrument]
pub async fn generate_intermediate_wallpapers(
    wallpaper_windows: &WallpaperCache,
    wallpaper_dir: &PathBuf,
    cache_dir: &PathBuf,
    iterations: usize,
    resolution: (u32, u32),
) -> anyhow::Result<()> {
    wallpaper_windows
        .par_iter()
        .try_for_each(|(first_wallpaper, second_wallpaper)| {
            let cache_path = cache_dir.join(format!(
                "{}_{}",
                PathBuf::from(first_wallpaper)
                    .file_stem()
                    .ok_or(anyhow::anyhow!("cannot get file stem for wallpaper path"))?
                    .to_string_lossy(),
                PathBuf::from(second_wallpaper)
                    .file_stem()
                    .ok_or(anyhow::anyhow!("cannot get file stem for wallpaper path"))?
                    .to_string_lossy()
            ));

            if cache_path.exists() {
                return Ok(());
            }

            info!(
                generate_directory = cache_path.to_string_lossy().into_owned(),
                "generating wallpapers"
            );

            let first_file_path = wallpaper_dir.join(first_wallpaper);
            let second_file_path = wallpaper_dir.join(second_wallpaper);

            let first_image = ImageReader::open(&first_file_path)?.decode()?.into_rgb8();
            let second_image = ImageReader::open(&second_file_path)?.decode()?.into_rgb8();

            let width = resolution.0;
            let height = resolution.1;

            if first_image.dimensions() != resolution {
                error!(
                    image_resolution = format!("{:#?}", first_image.dimensions()),
                    file_path = first_file_path.to_string_lossy().into_owned(),
                    "dimension mismatch"
                );
                bail!("first image resolution not matching");
            }

            if second_image.dimensions() != resolution {
                error!(
                    image_resolution = format!("{:#?}", second_image.dimensions()),
                    file_path = second_file_path.to_string_lossy().into_owned(),
                    "dimension mismatch"
                );
                bail!("first image resolution not matching");
            }

            let first_image_raw = first_image.as_flat_samples();
            let second_image_raw = second_image.as_flat_samples();

            let output_buffer = cross_fade(
                &first_image_raw.samples,
                &second_image_raw.samples,
                iterations,
            )?;
            let split_buffer = output_buffer.par_chunks_exact(first_image_raw.samples.len());

            std::fs::create_dir(&cache_path)?;

            split_buffer
                .map(|raw_output| -> WallpaperImage {
                    ImageBuffer::from_raw(width, height, raw_output)
                        .expect("container not large enough")
                })
                .enumerate()
                .try_for_each(|(index, image)| -> anyhow::Result<()> {
                    let file_path = cache_path.join(format!("{}.png", index));
                    debug!(
                        file = file_path.to_string_lossy().into_owned(),
                        "saving cross fade file"
                    );
                    image.save(file_path)?;
                    Ok(())
                })?;

            Ok(())
        })?;

    Ok(())
}
