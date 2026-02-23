use std::path::Path;

use tokio::fs;
use uuid::Uuid;

use super::{Error, Trace};

pub async fn load_all_from_directory() -> Result<Vec<Trace>, Error> {
    let traces_dir = crate::TRACES_DIR.get().unwrap();
    let mut traces = Vec::new();

    let mut dir = fs::read_dir(traces_dir).await?;
    while let Some(entry) = dir.next_entry().await? {
        traces.push(load_from_file(entry.path()).await?);
    }

    Ok(traces)
}

pub async fn load_from_directory(trace_id: Uuid) -> Result<Trace, Error> {
    let traces_dir = crate::TRACES_DIR.get().unwrap();
    let file_path = traces_dir.join(format!("{}.json", trace_id.hyphenated()));
    load_from_file(&file_path).await
}

pub async fn save_to_directory(trace: &Trace) -> Result<(), Error> {
    let traces_dir = crate::TRACES_DIR.get().unwrap();
    let file_path = traces_dir.join(format!("{}.json", trace.metadata.id.hyphenated()));
    save_to_file(&file_path, trace).await
}

pub async fn load_from_file<P: AsRef<Path>>(file_path: P) -> Result<Trace, Error> {
    let text = fs::read(file_path).await?;
    let trace = serde_json::from_slice(&text)?;
    Ok(trace)
}

pub async fn save_to_file<P: AsRef<Path>>(file_path: P, trace: &Trace) -> Result<(), Error> {
    fs::write(&file_path, serde_json::to_string_pretty(trace)?).await?;
    Ok(())
}
