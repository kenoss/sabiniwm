use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct EnvVar {
    /// Environment variables Without prefix.
    pub generic: EnvVarGeneric,
    /// Environment variables prefixed with `SABINIWM_`
    pub sabiniwm: EnvVarSabiniwm,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct EnvVarGeneric {
    pub display: Option<String>,
    pub wayland_display: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub(crate) struct EnvVarSabiniwm {
    /// Prevent auto detection and use designated DRM device node.
    ///
    /// Both primary node (e.g. /dev/dri/card0) and render node (e.g. /dev/dri/renderD128) are
    /// available. Sabiniwm infers corresponding primary/render nodes.
    pub drm_device_node: Option<PathBuf>,
    /// Give up if the initialization takes longer than this many seconds. `0` disables it.
    ///
    /// See [`crate::util::console`] for why this exists.
    #[serde(default = "default_init_timeout_sec")]
    pub init_timeout_sec: u64,
    #[serde(default = "default_bool::<false>")]
    pub disable_10bit: bool,
    #[serde(default = "default_bool::<true>")]
    pub enable_direct_scanout: bool,
}

impl EnvVarSabiniwm {
    pub fn init_timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.init_timeout_sec)
    }
}

const fn default_init_timeout_sec() -> u64 {
    20
}

// https://github.com/serde-rs/serde/issues/1030
// TODO(https://github.com/serde-rs/serde/issues/368): Use literal once default literals is supported.
const fn default_bool<const V: bool>() -> bool {
    V
}

impl EnvVar {
    pub fn load() -> eyre::Result<Self> {
        Ok(Self {
            generic: envy::from_env()?,
            sabiniwm: envy::prefixed("SABINIWM_").from_env()?,
        })
    }
}
