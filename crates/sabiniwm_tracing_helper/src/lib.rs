pub mod debug;

use std::path::PathBuf;

/// Returns the path of the log file to use with the udev backend.
///
/// With the udev backend there is nowhere to print to: the VT is in `KD_GRAPHICS` mode as soon as
/// the session is taken, so anything written to the console is lost. The log file is the only
/// record of what happened, and recovering from a failed startup usually means power cycling the
/// machine, so it must not live on a tmpfs.
///
/// Overridable with `SABINIWM_LOG_FILE`.
pub fn log_file_path() -> eyre::Result<PathBuf> {
    if let Some(path) = std::env::var_os("SABINIWM_LOG_FILE") {
        return Ok(PathBuf::from(path));
    }

    let state_home = match std::env::var_os("XDG_STATE_HOME") {
        Some(x) => PathBuf::from(x),
        None => {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| eyre::eyre!("neither XDG_STATE_HOME nor HOME is set"))?;
            PathBuf::from(home).join(".local/state")
        }
    };
    let dir = state_home.join("sabiniwm");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("sabiniwm.log"))
}

/// Filters out span context for event logging
pub struct NoSpanContextFilter;

impl<S> tracing_subscriber::layer::Filter<S> for NoSpanContextFilter
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn enabled(
        &self,
        metadata: &tracing_core::Metadata<'_>,
        _cx: &tracing_subscriber::layer::Context<'_, S>,
    ) -> bool {
        !metadata.is_span()
    }
}
