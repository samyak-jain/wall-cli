use std::path::PathBuf;

use anyhow::{bail, Ok};
use image::io::Reader as ImageReader;
use image_transitions::cross_fade;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use tracing::{debug, error, info};

// Resize all images in the wallpaper_directory into the provided resolution in parallel
pub fn resize_images(
    wallpapers: &Vec<PathBuf>,
    dest_dir: &PathBuf,
    resolution: (u32, u32),
) -> anyhow::Result<()> {
    info!("starting to resize image");

    wallpapers
        .par_iter()
        .try_for_each(|image_path| -> anyhow::Result<()> {
            let image_name = image_path.file_name().ok_or(anyhow::anyhow!(
                "could not get file name for file: {:#?}",
                image_path
            ))?;

            let dest_image_path = dest_dir.join(image_name);
            if dest_image_path.exists() {
                debug!(
                    image = dest_image_path.to_string_lossy().into_owned(),
                    "image already resized"
                );
                return Ok(());
            }

            info!(
                image_name = image_name.to_string_lossy().into_owned(),
                "resizing image"
            );

            let image = ImageReader::open(image_path)?.decode()?;
            let new_image = image.resize_to_fill(
                resolution.0,
                resolution.1,
                image::imageops::FilterType::Lanczos3,
            );

            new_image.save(dest_image_path)?;

            Ok(())
        })?;

    info!("images resized");

    Ok(())
}

// Generate intermediate images for the cross fade animation and store it in the cache folder
pub fn generate_intermediate_wallpapers(
    first_wallpaper: &PathBuf,
    second_wallpaper: &PathBuf,
    iterations: u16,
    resolution: (u32, u32),
) -> anyhow::Result<(Vec<u8>, usize)> {
    let first_image = ImageReader::open(&first_wallpaper)?.decode()?.into_rgb8();
    let second_image = ImageReader::open(&second_wallpaper)?.decode()?.into_rgb8();

    // let width = resolution.0;
    // let height = resolution.1;

    if first_image.dimensions() != resolution {
        error!(
            image_resolution = format!("{:#?}", first_image.dimensions()),
            file_path = first_wallpaper.to_string_lossy().into_owned(),
            "dimension mismatch"
        );
        bail!("first image resolution not matching");
    }

    if second_image.dimensions() != resolution {
        error!(
            image_resolution = format!("{:#?}", second_image.dimensions()),
            file_path = second_wallpaper.to_string_lossy().into_owned(),
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
        image_transitions::GridStride::Default,
    )?;

    Ok((output_buffer, first_image_raw.samples.len()))

    // let split_buffer = output_buffer
    //     .par_chunks_exact(first_image_raw.samples.len())
    //     .collect::<Vec<_>>();

    // let boxed_buffer = split_buffer.into_boxed_slice();
    // let leaked_buffer = Box::leak(boxed_buffer);

    // let streamer_buffer = BufferList::from_iter(
    //     leaked_buffer
    //         .iter()
    //         .map(|buf| gstreamer::buffer::Buffer::from_slice(buf)),
    // );

    // // mem::forget(split_buffer);

    // Ok(streamer_buffer)
}
