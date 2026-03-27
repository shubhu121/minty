use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub type SharedTextEmbedding = Arc<std::sync::Mutex<TextEmbedding>>;

pub async fn load_shared_text_embedding(
    model_name: EmbeddingModel,
    cache_dir: Option<PathBuf>,
) -> Result<SharedTextEmbedding, String> {
    let model = load_text_embedding(model_name, cache_dir).await?;
    Ok(Arc::new(std::sync::Mutex::new(model)))
}

async fn load_text_embedding(
    model_name: EmbeddingModel,
    cache_dir: Option<PathBuf>,
) -> Result<TextEmbedding, String> {
    if let Some(dir) = cache_dir.as_ref() {
        fs::create_dir_all(dir).map_err(|e| {
            format!(
                "failed to create fastembed cache dir {}: {e}",
                dir.display()
            )
        })?;
        std::env::set_var("FASTEMBED_CACHE_PATH", dir);
    }

    match try_load_text_embedding(&model_name, cache_dir.clone()).await {
        Ok(model) => Ok(model),
        Err(first_error) => {
            let Some(dir) = cache_dir.as_ref() else {
                return Err(first_error);
            };

            eprintln!(
                "[FastEmbed] Model init failed, clearing cache at {} and retrying: {}",
                dir.display(),
                first_error
            );
            clear_fastembed_cache(dir).map_err(|e| {
                format!(
                    "{first_error}; additionally failed to clear fastembed cache {}: {e}",
                    dir.display()
                )
            })?;

            try_load_text_embedding(&model_name, cache_dir)
                .await
                .map_err(|retry_error| {
                    format!(
                        "{retry_error} (after clearing a broken cache; initial error: {first_error})"
                    )
                })
        }
    }
}

async fn try_load_text_embedding(
    model_name: &EmbeddingModel,
    cache_dir: Option<PathBuf>,
) -> Result<TextEmbedding, String> {
    let model_name = model_name.clone();

    tokio::task::spawn_blocking(move || {
        let options = if let Some(dir) = cache_dir {
            InitOptions::new(model_name).with_cache_dir(dir)
        } else {
            InitOptions::new(model_name)
        };

        TextEmbedding::try_new(options).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("model init panicked: {e}"))?
}

fn clear_fastembed_cache(cache_dir: &Path) -> std::io::Result<()> {
    if !cache_dir.exists() {
        return Ok(());
    }

    for entry in fs::read_dir(cache_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }

    Ok(())
}
