use std::{path::PathBuf, sync::Arc};

use crate::{
    cache::{save_cache, WallpaperCache},
    wallpaper::WallpaperData,
};
use image::{
    codecs::png::PngEncoder, io::Reader as ImageReader, load_from_memory, ImageBuffer,
    ImageEncoder, RgbImage,
};
use image_transitions::cross_fade;

pub async fn resize_images(
    wallpapers: Arc<WallpaperData>,
    wallpaper_dir: &PathBuf,
    data_dir: &PathBuf,
    resolution: (u32, u32),
) -> anyhow::Result<()> {
    let image_names = wallpapers.get_all();
    for image_name in image_names {
        let mut image_path = wallpaper_dir.clone();
        image_path.push(&image_name);

        let mut dest_image_path = data_dir.clone();
        dest_image_path.push(image_name);

        if dest_image_path.exists() {
            continue;
        }

        let image = ImageReader::open(image_path)?.decode()?;
        let new_image = image.resize_to_fill(
            resolution.0,
            resolution.1,
            image::imageops::FilterType::Lanczos3,
        );

        new_image.save(dest_image_path)?;
    }

    Ok(())
}

pub async fn generate_intermediate_wallpapers(
    wallpaper_windows: &WallpaperCache,
    wallpaper_dir: &PathBuf,
    cache_dir: &PathBuf,
    iterations: usize,
) -> anyhow::Result<()> {
    for (first_wallpaper, second_wallpaper) in wallpaper_windows.iter() {
        let mut cache_path = cache_dir.clone();
        cache_path.push(format!("{}_{}", first_wallpaper, second_wallpaper));

        if cache_path.exists() {
            continue;
        }

        let mut first_file_path = wallpaper_dir.clone();
        first_file_path.push(first_wallpaper);

        dbg!(first_file_path.display());

        let mut second_file_path = wallpaper_dir.clone();
        second_file_path.push(second_wallpaper);

        dbg!(second_file_path.display());

        let first_image = ImageReader::open(first_file_path)?.decode()?.into_rgb8();
        let second_image = ImageReader::open(second_file_path)?.decode()?.into_rgb8();

        let width = first_image.width();
        let height = first_image.height();

        dbg!(width, height);

        let first_image_raw = first_image.as_flat_samples();
        let second_image_raw = second_image.as_flat_samples();

        let output_buffer = cross_fade(
            &first_image_raw.samples,
            &second_image_raw.samples,
            iterations,
        )?;
        let split_buffer = output_buffer.chunks_exact(first_image_raw.samples.len());

        tokio::fs::create_dir(&cache_path).await?;

        let intermediate_image = split_buffer.map(|raw_output| {
            ImageBuffer::from_raw(width, height, raw_output).expect("container not large enough")
            // let img = RgbImage::new(width, height);
            // let mut img_flat_buffer = img.as_flat_samples();
            // img_flat_buffer.samples = raw_output;
            // // img_flat_buffer.try_into_buffer::<image::Rgb<u8>>().unwrap();
            // img
        });

        save_cache(cache_path, intermediate_image).await?;

        // let images = split_buffer.for_each(|raw_output| {
        //     // TODO: possible remove this allocation
        //     // load_from_memory(&filebuffer).unwrap().to_rgba8()
        // });
        //
        //save_cache(cache_path, images).await?;
    }

    Ok(())
}
