use std::path::PathBuf;

use crate::WallpaperData;
use image::{io::Reader as ImageReader, ImageBuffer, Rgba};
use image_transitions::cross_fade;

pub async fn generate_intermediate_wallpapers(
    wallpapers: WallpaperData,
    wallpaper_dir: PathBuf,
    iterations: usize,
) -> anyhow::Result<()> {
    let wallpaper_list = wallpapers.get_all();
    let mut wallpaper_windows = wallpaper_list.windows(2);

    while let Some([first_wallpaper, second_wallpaper]) = wallpaper_windows.next() {
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

        for (index, raw_output) in split_buffer.enumerate() {
            let output_image: ImageBuffer<Rgba<u16>, &[u16]> =
                ImageBuffer::from_raw(width, height, raw_output)
                    .ok_or(anyhow::anyhow!("image buffer container not big enough"))?;

            let mut output_path = folder_path.clone();
            output_path.push(index.to_string());

            output_image.save(output_path)?;
        }
    }

    Ok(())
}
