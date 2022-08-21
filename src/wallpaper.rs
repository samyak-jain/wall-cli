use std::path::PathBuf;

use rand::prelude::SliceRandom;
use tracing::info;

use crate::StreamEvent;

#[tracing::instrument]
pub async fn handle_event(wallpapers: &mut Vec<PathBuf>, event: StreamEvent) -> anyhow::Result<()> {
    let event_data = event?;
    dbg!(event_data.clone());
    let paths = event_data.paths;

    match event_data.kind {
        notify::EventKind::Create(notify::event::CreateKind::File) => {
            wallpapers.extend(paths);
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Name(
            notify::event::RenameMode::Both,
        )) => {
            let old_path = paths.get(0).ok_or(anyhow::anyhow!("paths is empty"))?;
            let new_path = paths.get(1).ok_or(anyhow::anyhow!(
                "new path for old path {:#?} is not there",
                old_path
            ))?;

            info!(
                old_path = old_path.to_string_lossy().into_owned(),
                new_path = new_path.to_string_lossy().into_owned(),
                "renaming"
            );

            let position = wallpapers.iter().position(|item| item == old_path);
            if let Some(position) = position {
                wallpapers[position] = new_path.to_path_buf();
                tokio::fs::remove_file(old_path).await?;
            } else {
                return Ok(());
            }
        }
        notify::EventKind::Remove(notify::event::RemoveKind::File) => {
            paths.iter().try_for_each(|path| -> Option<()> {
                wallpapers.swap_remove(wallpapers.iter().position(|item| item == path)?);
                Some(())
            });
        }
        _ => {}
    };

    let mut rng = rand::thread_rng();
    wallpapers.shuffle(&mut rng);

    Ok(())
}
