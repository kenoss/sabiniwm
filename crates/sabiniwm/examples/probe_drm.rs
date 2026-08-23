//! Diagnostic tool for the udev backend's device selection and renderer setup.
//!
//! The udev backend takes over the VT (via `LibSeatSession`) before it does anything else, so a
//! failure during its initialization leaves nothing but a black screen: the error message is
//! printed onto a console that is in `KD_GRAPHICS` mode and is lost. This example runs everything
//! the udev backend does up to, but not including, the modeset, so that it can be run from inside
//! another running compositor and its output can actually be read.
//!
//! Notable differences from the real backend:
//!
//! - Devices are opened directly instead of via libseat, because only one session controller is
//!   allowed at a time and the compositor we are running under already holds it.
//! - `DrmDevice` is not constructed. Dropping it tries to restore the pre-existing KMS state, which
//!   would be a bad thing to do while another compositor is driving the display. The checks
//!   `DrmDevice::new()` performs (`resource_handles()`, `plane_handles()`) are done directly here.
//! - No `DrmOutput` is initialized; that requires DRM master.
//!
//! ```shell
//! $ cargo run -p sabiniwm --example probe_drm
//! ```

use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::drm::{DrmDeviceFd, DrmNode, NodeType};
use smithay::backend::egl::context::ContextPriority;
use smithay::backend::egl::{EGLDevice, EGLDisplay};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::multigpu::GpuManager;
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::backend::renderer::{ImportDma, ImportMemWl};
use smithay::reexports::drm;
use smithay::reexports::drm::control::Device as ControlDevice;
use smithay::reexports::drm::{ClientCapability, Device as _};
use smithay::utils::DeviceFd;
use smithay::wayland::drm_syncobj::supports_syncobj_eventfd;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};

// Kept in sync with `crate::backend::udev`.
const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];

/// A bare DRM device, used only for the read-only queries below.
struct Card(std::fs::File);

impl Card {
    fn open(path: &Path) -> std::io::Result<Self> {
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .map(Card)
    }
}

impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl drm::Device for Card {}
impl ControlDevice for Card {}

fn main() -> eyre::Result<()> {
    let seat = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "seat0".to_owned());

    println!("== device selection (seat = {seat})");
    let all_gpus = smithay::backend::udev::all_gpus(&seat)?;
    println!("  all_gpus()               = {all_gpus:?}");
    println!(
        "  primary_gpu()            = {:?}",
        smithay::backend::udev::primary_gpu(&seat)?
    );
    println!(
        "  SABINIWM_DRM_DEVICE_NODE = {:?}",
        std::env::var("SABINIWM_DRM_DEVICE_NODE").ok()
    );

    for path in &all_gpus {
        println!();
        if let Err(err) = probe(path) {
            println!("  ERROR: {err:?}");
        }
    }

    Ok(())
}

fn probe(path: &Path) -> eyre::Result<()> {
    println!("== {}", path.display());

    let node = DrmNode::from_path(path)?;
    println!("  node                     = {:?} {}", node.ty(), node);
    println!("  driver (sysfs)           = {:?}", sysfs_driver(&node));

    // This is how `UdevBackend::new()` derives `selected_render_node` from the primary node. It is
    // `None` for a display-only device, which is exactly what happens on a split display/render
    // SoC such as Apple silicon.
    println!(
        "  node_with_type(Render)   = {:?}",
        node.node_with_type(NodeType::Render).and_then(Result::ok)
    );

    let card = Card::open(path)?;

    // `DrmDevice::new()` fails here for a render-only device: `drmModeGetResources` returns
    // `EOPNOTSUPP` when the driver does not have `DRIVER_MODESET`.
    let resources = match card.resource_handles() {
        Ok(resources) => resources,
        Err(err) => {
            println!(
                "  resource_handles()       = Err({err}) -- not a KMS device, DrmDevice::new() would fail here"
            );
            return Ok(());
        }
    };
    println!(
        "  resource_handles()       = {} connectors, {} crtcs",
        resources.connectors().len(),
        resources.crtcs().len()
    );

    for handle in resources.connectors() {
        // Don't force a probe: the compositor we are running under owns this connector.
        let info = card.get_connector(*handle, false)?;
        let preferred = info
            .modes()
            .iter()
            .find(|mode| {
                mode.mode_type()
                    .contains(drm::control::ModeTypeFlags::PREFERRED)
            })
            .or_else(|| info.modes().first());
        println!(
            "    {}-{} {:?} preferred_mode = {:?}",
            info.interface().as_str(),
            info.interface_id(),
            info.state(),
            preferred.map(|mode| (mode.size(), mode.vrefresh())),
        );
    }

    let plane_formats = primary_plane_formats(&card, &resources)?;

    let fd = DrmDeviceFd::new(DeviceFd::from(OwnedFd::from(card.0)));
    let gbm = GbmDevice::new(fd)?;
    // SAFETY: `gbm` outlives the display and everything derived from it.
    let display = unsafe { EGLDisplay::new(gbm.clone()) }?;
    let device = EGLDevice::device_for_display(&display)?;
    println!(
        "  EGL drm_device_path      = {:?}",
        device.drm_device_path()
    );
    println!(
        "  EGL render_device_path   = {:?}",
        device.render_device_path()
    );

    // This is what `device_added()` uses. Unlike `node_with_type()` above it resolves correctly
    // even for a display-only device, because Mesa opens the render node under the hood.
    let render_node = device.try_get_render_node()?;
    println!("  try_get_render_node()    = {render_node:?}");
    let Some(render_node) = render_node else {
        return Ok(());
    };

    // `linux-drm-syncobj-v1` needs the device we render on, not the one we scan out on.
    println!(
        "  syncobj eventfd          = {} on {}, {} on {}",
        supports_syncobj_eventfd(&card_fd(path)?),
        path.display(),
        supports_syncobj_eventfd(&card_fd(&render_node.dev_path().unwrap_or_default())?),
        dev_path_or_na(&render_node),
    );

    let mut gpus = GpuManager::new(
        GbmGlesBackend::<GlesRenderer, DrmDeviceFd>::with_context_priority(ContextPriority::High),
    )?;
    gpus.as_mut().add_node(render_node, gbm)?;
    let mut renderer = gpus.single_renderer(&render_node)?;
    println!("  single_renderer()        = ok");
    println!(
        "  shm_formats()            = {} formats",
        renderer.shm_formats().count()
    );
    println!(
        "  dmabuf_formats()         = {} format/modifier pairs",
        renderer.dmabuf_formats().iter().count()
    );

    // Roughly what `DrmOutputManager::initialize_output()` negotiates: a format the renderer can
    // render into and the primary plane can scan out.
    let render_formats = renderer
        .as_mut()
        .egl_context()
        .dmabuf_render_formats()
        .clone();
    println!("  candidate scanout formats:");
    for fourcc in SUPPORTED_FORMATS {
        println!(
            "    {fourcc:?}: renderable = {}, on primary plane = {}",
            render_formats.iter().any(|format| format.code == *fourcc),
            plane_formats.contains(&(*fourcc as u32)),
        );
    }

    Ok(())
}

fn card_fd(path: &Path) -> eyre::Result<DrmDeviceFd> {
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    Ok(DrmDeviceFd::new(DeviceFd::from(OwnedFd::from(file))))
}

fn dev_path_or_na(node: &DrmNode) -> String {
    match node.dev_path() {
        Some(path) => format!("{}", path.display()),
        None => "N/A".to_owned(),
    }
}

fn primary_plane_formats(
    card: &Card,
    resources: &drm::control::ResourceHandles,
) -> eyre::Result<Vec<u32>> {
    // Primary and cursor planes are hidden unless universal planes are enabled.
    card.set_client_capability(ClientCapability::UniversalPlanes, true)?;

    let Some(crtc) = resources.crtcs().first() else {
        return Ok(Vec::new());
    };
    let mut formats = Vec::new();
    for handle in card.plane_handles()? {
        let info = card.get_plane(handle)?;
        if !resources.filter_crtcs(info.possible_crtcs()).contains(crtc) {
            continue;
        }
        formats.extend_from_slice(info.formats());
    }
    Ok(formats)
}

fn sysfs_driver(node: &DrmNode) -> Option<PathBuf> {
    let path = format!(
        "/sys/dev/char/{}:{}/device/driver",
        node.major(),
        node.minor()
    );
    std::fs::read_link(path)
        .ok()
        .and_then(|path| path.file_name().map(PathBuf::from))
}
