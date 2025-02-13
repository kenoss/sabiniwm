use crate::backend::BackendI;
use crate::config::ConfigDelegateUnstableI;
use crate::envvar::EnvVar;
use crate::pointer::{PointerElement, CLEAR_COLOR};
use crate::render::{CustomRenderElement, OutputRenderElement, SurfaceDmabufFeedback};
use crate::render_loop::RenderLoop;
use crate::state::{DndIcon, InnerState, SabiniwmState, SabiniwmStateWithConcreteBackend};
use crate::util::EventHandler;
use crate::view::window::WindowRenderElement;
use crate::wl_global::WlGlobal;
use eyre::WrapErr;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::format::FormatSet;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::compositor::DrmCompositor;
use smithay::backend::drm::{
    CreateDrmNodeError, DrmAccessError, DrmDevice, DrmDeviceFd, DrmError, DrmEvent,
    DrmEventMetadata, DrmNode, DrmSurface, GbmBufferedSurface, NodeType,
};
use smithay::backend::egl::context::ContextPriority;
use smithay::backend::egl::{self, EGLDevice, EGLDisplay};
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::damage::{Error as OutputDamageTrackerError, OutputDamageTracker};
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::backend::renderer::element::{AsRenderElements, RenderElement, RenderElementStates};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::backend::renderer::multigpu::{GpuManager, MultiRenderer};
use smithay::backend::renderer::sync::SyncPoint;
#[cfg(feature = "egl")]
use smithay::backend::renderer::ImportEgl;
use smithay::backend::renderer::{
    Bind, Color32F, DebugFlags, ExportMem, ImportAll, ImportDma, ImportMem, ImportMemWl, Offscreen,
    Renderer,
};
use smithay::backend::session::libseat::{self, LibSeatSession};
use smithay::backend::session::{Event as SessionEvent, Session};
use smithay::backend::udev::UdevEvent;
use smithay::backend::SwapBuffersError;
use smithay::delegate_drm_lease;
use smithay::desktop::space::{Space, SurfaceTree};
use smithay::desktop::utils::OutputPresentationFeedback;
use smithay::input::pointer::{CursorImageAttributes, CursorImageStatus};
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use smithay::reexports::drm::control::{connector, crtc, Device};
use smithay::reexports::drm::Device as _;
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::wayland_protocols::wp::linux_dmabuf::zv1::server::zwp_linux_dmabuf_feedback_v1;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::{input as libinput, wayland_server};
use smithay::utils::{DeviceFd, IsAlive, Logical, Physical, Point, Rectangle, Scale, Transform};
use smithay::wayland::compositor;
use smithay::wayland::dmabuf::{DmabufFeedbackBuilder, DmabufGlobal, DmabufState};
use smithay::wayland::drm_lease::{
    DrmLease, DrmLeaseBuilder, DrmLeaseHandler, DrmLeaseRequest, DrmLeaseState, LeaseRejected,
};
use smithay::wayland::drm_syncobj::{supports_syncobj_eventfd, DrmSyncobjHandler, DrmSyncobjState};
use smithay_drm_extras::drm_scanner::{DrmScanEvent, DrmScanner};
use std::collections::hash_map::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

// we cannot simply pick the first supported format of the intersection of *all* formats, because:
// - we do not want something like Abgr4444, which looses color information, if something better is available
// - some formats might perform terribly
// - we might need some work-arounds, if one supports modifiers, but the other does not
//
// So lets just pick `ARGB2101010` (10-bit) or `ARGB8888` (8-bit) for now, they are widely supported.
const SUPPORTED_FORMATS: &[Fourcc] = &[
    Fourcc::Abgr2101010,
    Fourcc::Argb2101010,
    Fourcc::Abgr8888,
    Fourcc::Argb8888,
];
const SUPPORTED_FORMATS_8BIT_ONLY: &[Fourcc] = &[Fourcc::Abgr8888, Fourcc::Argb8888];

type UdevRenderer<'a> = MultiRenderer<
    'a,
    'a,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
    GbmGlesBackend<GlesRenderer, DrmDeviceFd>,
>;

#[derive(Debug, PartialEq)]
struct UdevOutputId {
    primary_node: DrmNode,
    crtc: crtc::Handle,
}

pub(crate) struct UdevBackend {
    session: LibSeatSession,
    dmabuf_state: Option<(DmabufState, DmabufGlobal)>,
    syncobj_state: Option<DrmSyncobjState>,
    selected_render_node: DrmNode,
    gpus: GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    backends: HashMap<DrmNode, BackendData>,
    pointer_images: Vec<(xcursor::parser::Image, MemoryRenderBuffer)>,
    pointer_element: PointerElement,
    pointer_image: crate::cursor::Cursor,
    debug_flags: DebugFlags,

    // Input
    libinput_context: libinput::Libinput,
    input_devices: HashSet<libinput::Device>,
}

impl UdevBackend {
    pub fn new(
        envvar: &EnvVar,
        loop_handle: LoopHandle<'static, SabiniwmState>,
    ) -> eyre::Result<Self> {
        /*
         * Initialize session
         */
        let (session, notifier) = LibSeatSession::new().wrap_err("initialize session")?;

        /*
         * Initialize the compositor
         */
        let device_node_path = if let Some(path) = &envvar.sabiniwm.drm_device_node {
            path.clone()
        } else {
            smithay::backend::udev::primary_gpu(session.seat())
                .wrap_err("get primary GPU")?
                .ok_or_else(|| eyre::eyre!("GPU not found"))?
        };
        let device_node = DrmNode::from_path(device_node_path.clone()).wrap_err_with(|| {
            format!(
                "open DRM device node: path = {}",
                device_node_path.display()
            )
        })?;
        let selected_render_node = if device_node.ty() == NodeType::Render {
            device_node
        } else {
            device_node
                .node_with_type(NodeType::Render)
                .ok_or_else(|| {
                    eyre::eyre!(
                        "no corresponding render node for: path = {}",
                        dev_path_or_na(&device_node)
                    )
                })?
                .wrap_err_with(|| {
                    format!(
                        "get render node for: path = {}",
                        dev_path_or_na(&device_node)
                    )
                })?
        };
        info!(
            "Using {} as render node.",
            dev_path_or_na(&selected_render_node)
        );

        let gpus = GpuManager::new(GbmGlesBackend::with_context_priority(ContextPriority::High))?;

        let mut libinput_context =
            libinput::Libinput::new_with_udev(LibinputSessionInterface::from(session.clone()));
        let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
        loop_handle
            .insert_source(libinput_backend, |event, _, state| {
                state.handle_event(event)
            })
            .map_err(|e| eyre::eyre!("{}", e))?;
        libinput_context
            .udev_assign_seat(&session.seat())
            .map_err(|e| eyre::eyre!("{:?}", e))?;

        loop_handle
            .insert_source(notifier, move |event, _, state| {
                state.as_udev_mut().handle_event(event)
            })
            .map_err(|e| eyre::eyre!("{}", e))?;

        Ok(UdevBackend {
            dmabuf_state: None,
            syncobj_state: None,
            session,
            selected_render_node,
            gpus,
            backends: HashMap::new(),
            pointer_image: crate::cursor::Cursor::load(),
            pointer_images: Vec::new(),
            pointer_element: PointerElement::default(),
            debug_flags: DebugFlags::empty(),
            libinput_context,
            input_devices: HashSet::new(),
        })
    }
}

impl smithay::wayland::buffer::BufferHandler for UdevBackend {
    fn buffer_destroyed(&mut self, _buffer: &wayland_server::protocol::wl_buffer::WlBuffer) {}
}

impl crate::backend::DmabufHandlerDelegate for UdevBackend {
    fn dmabuf_state(&mut self) -> &mut smithay::wayland::dmabuf::DmabufState {
        &mut self.dmabuf_state.as_mut().unwrap().0
    }

    fn dmabuf_imported(
        &mut self,
        _global: &smithay::wayland::dmabuf::DmabufGlobal,
        dmabuf: smithay::backend::allocator::dmabuf::Dmabuf,
    ) -> bool {
        let ret = self
            .gpus
            .single_renderer(&self.selected_render_node)
            .and_then(|mut renderer| renderer.import_dmabuf(&dmabuf, None))
            .is_ok();
        if ret {
            dmabuf.set_node(self.selected_render_node);
        }
        ret
    }
}

impl BackendI for UdevBackend {
    fn init(&mut self, inner: &mut InnerState) -> eyre::Result<()> {
        /*
         * Initialize the udev backend
         */
        let udev_backend = smithay::backend::udev::UdevBackend::new(&inner.seat_name)?;

        for (device_id, path) in udev_backend.device_list() {
            if let Err(err) = DrmNode::from_dev_id(device_id)
                .map_err(DeviceAddError::DrmNode)
                .and_then(|node| {
                    let mut as_udev_mut = SabiniwmStateWithConcreteBackend {
                        backend: self,
                        inner,
                    };
                    as_udev_mut.device_added(node, path)
                })
            {
                error!("Skipping device {device_id}: {err}");
            }
        }

        inner.shm_state.update_formats(
            self.gpus
                .single_renderer(&self.selected_render_node)
                .unwrap()
                .shm_formats(),
        );

        #[cfg_attr(not(feature = "egl"), allow(unused_mut))]
        let mut renderer = self
            .gpus
            .single_renderer(&self.selected_render_node)
            .unwrap();

        #[cfg(feature = "egl")]
        {
            info!(
                ?self.selected_render_node,
                "Trying to initialize EGL Hardware Acceleration",
            );
            match renderer.bind_wl_display(&inner.display_handle) {
                Ok(_) => info!("EGL hardware-acceleration enabled"),
                Err(err) => info!(?err, "Failed to initialize EGL hardware-acceleration"),
            }
        }

        // init dmabuf support with format list from selected render node
        let dmabuf_formats = renderer.dmabuf_formats();
        let default_feedback =
            DmabufFeedbackBuilder::new(self.selected_render_node.dev_id(), dmabuf_formats)
                .build()?;
        let mut dmabuf_state = DmabufState::new();
        let global = dmabuf_state.create_global_with_default_feedback::<SabiniwmState>(
            &inner.display_handle,
            &default_feedback,
        );
        self.dmabuf_state = Some((dmabuf_state, global));

        let gpus = &mut self.gpus;
        for backend in self.backends.values_mut() {
            // Update the per drm surface dmabuf feedback
            for surface_data in backend.surfaces.values_mut() {
                surface_data.dmabuf_feedback = surface_data.dmabuf_feedback.take().or_else(|| {
                    get_surface_dmabuf_feedback(
                        self.selected_render_node,
                        surface_data.render_node,
                        gpus,
                        &surface_data.compositor,
                    )
                });
            }
        }

        // Expose syncobj protocol if supported by primary GPU
        if let Some(primary_node) = self
            .selected_render_node
            .node_with_type(NodeType::Primary)
            .and_then(|x| x.ok())
        {
            if let Some(backend) = self.backends.get(&primary_node) {
                let import_device = backend.drm.device_fd().clone();
                if supports_syncobj_eventfd(&import_device) {
                    let syncobj_state =
                        DrmSyncobjState::new::<SabiniwmState>(&inner.display_handle, import_device);
                    self.syncobj_state = Some(syncobj_state);
                }
            }
        }

        inner
            .loop_handle
            .insert_source(udev_backend, |event, _, state| {
                state.as_udev_mut().handle_event(event)
            })
            .map_err(|e| eyre::eyre!("{}", e))?;

        Ok(())
    }

    fn has_relative_motion(&self) -> bool {
        true
    }

    fn has_gesture(&self) -> bool {
        true
    }

    fn seat_name(&self) -> String {
        self.session.seat()
    }

    fn early_import(&mut self, surface: &wayland_server::protocol::wl_surface::WlSurface) {
        if let Err(err) = self.gpus.early_import(self.selected_render_node, surface) {
            warn!("Early buffer import failed: {}", err);
        }
    }

    fn update_led_state(&mut self, led_state: smithay::input::keyboard::LedState) {
        let keyboards = self
            .input_devices
            .iter()
            .filter(|device| device.has_capability(libinput::DeviceCapability::Keyboard))
            .cloned();
        for mut keyboard in keyboards {
            keyboard.led_update(led_state.into());
        }
    }

    fn change_vt(&mut self, vt: i32) {
        if let Err(e) = self.session.change_vt(vt) {
            warn!("changing VT failed: {e}");
        }
    }
}

impl DrmLeaseHandler for SabiniwmState {
    fn drm_lease_state(&mut self, node: DrmNode) -> &mut DrmLeaseState {
        self.backend_udev_mut()
            .backends
            .get_mut(&node)
            .unwrap()
            .leasing_global
            .as_mut()
            .unwrap()
    }

    fn lease_request(
        &mut self,
        node: DrmNode,
        request: DrmLeaseRequest,
    ) -> Result<DrmLeaseBuilder, LeaseRejected> {
        let backend = self
            .backend_udev()
            .backends
            .get(&node)
            .ok_or(LeaseRejected::default())?;

        let mut builder = DrmLeaseBuilder::new(&backend.drm);
        for conn in request.connectors {
            if let Some((_, crtc)) = backend
                .non_desktop_connectors
                .iter()
                .find(|(handle, _)| *handle == conn)
            {
                builder.add_connector(conn);
                builder.add_crtc(*crtc);
                let planes = backend
                    .drm
                    .planes(crtc)
                    .map_err(LeaseRejected::with_cause)?;
                let (primary_plane, primary_plane_claim) = planes
                    .primary
                    .iter()
                    .find_map(|plane| {
                        backend
                            .drm
                            .claim_plane(plane.handle, *crtc)
                            .map(|claim| (plane, claim))
                    })
                    .ok_or_else(LeaseRejected::default)?;
                builder.add_plane(primary_plane.handle, primary_plane_claim);
                if let Some((cursor, claim)) = planes.cursor.iter().find_map(|plane| {
                    backend
                        .drm
                        .claim_plane(plane.handle, *crtc)
                        .map(|claim| (plane, claim))
                }) {
                    builder.add_plane(cursor.handle, claim);
                }
            } else {
                warn!(
                    ?conn,
                    "Lease requested for desktop connector, denying request"
                );
                return Err(LeaseRejected::default());
            }
        }

        Ok(builder)
    }

    fn new_active_lease(&mut self, node: DrmNode, lease: DrmLease) {
        let backend = self.backend_udev_mut().backends.get_mut(&node).unwrap();
        backend.active_leases.push(lease);
    }

    fn lease_destroyed(&mut self, node: DrmNode, lease: u32) {
        let backend = self.backend_udev_mut().backends.get_mut(&node).unwrap();
        backend.active_leases.retain(|l| l.id() != lease);
    }
}

delegate_drm_lease!(SabiniwmState);

impl DrmSyncobjHandler for SabiniwmState {
    fn drm_syncobj_state(&mut self) -> &mut DrmSyncobjState {
        self.as_udev_mut().backend.syncobj_state.as_mut().unwrap()
    }
}

smithay::delegate_drm_syncobj!(SabiniwmState);

#[derive(Debug, Default, serde::Deserialize)]
pub(crate) enum SurfaceCompositionPolicy {
    UseGbmBufferedSurface,
    #[default]
    UseDrmCompositor,
}

type RenderSurface =
    GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, Option<OutputPresentationFeedback>>;

type GbmDrmCompositor = DrmCompositor<
    GbmAllocator<DrmDeviceFd>,
    GbmDevice<DrmDeviceFd>,
    Option<OutputPresentationFeedback>,
    DrmDeviceFd,
>;

enum SurfaceComposition {
    Surface {
        surface: RenderSurface,
        damage_tracker: OutputDamageTracker,
        debug_flags: DebugFlags,
    },
    Compositor(GbmDrmCompositor),
}

struct SurfaceCompositorRenderResult<'a> {
    rendered: bool,
    states: RenderElementStates,
    sync: Option<SyncPoint>,
    damage: Option<&'a Vec<Rectangle<i32, Physical>>>,
}

impl SurfaceComposition {
    fn frame_submitted(
        &mut self,
    ) -> Result<Option<Option<OutputPresentationFeedback>>, SwapBuffersError> {
        match self {
            SurfaceComposition::Compositor(c) => {
                c.frame_submitted().map_err(Into::<SwapBuffersError>::into)
            }
            SurfaceComposition::Surface { surface, .. } => surface
                .frame_submitted()
                .map_err(Into::<SwapBuffersError>::into),
        }
    }

    fn format(&self) -> smithay::reexports::gbm::Format {
        match self {
            SurfaceComposition::Compositor(c) => c.format(),
            SurfaceComposition::Surface { surface, .. } => surface.format(),
        }
    }

    fn surface(&self) -> &DrmSurface {
        match self {
            SurfaceComposition::Compositor(c) => c.surface(),
            SurfaceComposition::Surface { surface, .. } => surface.surface(),
        }
    }

    fn reset_buffers(&mut self) {
        match self {
            SurfaceComposition::Compositor(c) => c.reset_buffers(),
            SurfaceComposition::Surface { surface, .. } => surface.reset_buffers(),
        }
    }

    fn reset_state(&mut self) -> Result<(), SwapBuffersError> {
        match self {
            SurfaceComposition::Compositor(c) => {
                c.reset_state().map_err(Into::<SwapBuffersError>::into)
            }
            SurfaceComposition::Surface { surface, .. } => surface
                .surface()
                .reset_state()
                .map_err(Into::<SwapBuffersError>::into),
        }
    }

    fn queue_frame(
        &mut self,
        sync: Option<SyncPoint>,
        damage: Option<Vec<Rectangle<i32, Physical>>>,
        user_data: Option<OutputPresentationFeedback>,
    ) -> Result<(), SwapBuffersError> {
        match self {
            SurfaceComposition::Surface { surface, .. } => surface
                .queue_buffer(sync, damage, user_data)
                .map_err(Into::<SwapBuffersError>::into),
            SurfaceComposition::Compositor(c) => c
                .queue_frame(user_data)
                .map_err(Into::<SwapBuffersError>::into),
        }
    }

    fn render_frame<R, E, Target>(
        &mut self,
        renderer: &mut R,
        elements: &[E],
        clear_color: Color32F,
        frame_mode: smithay::backend::drm::compositor::FrameMode,
    ) -> Result<SurfaceCompositorRenderResult<'_>, SwapBuffersError>
    where
        R: Renderer + Bind<Dmabuf> + Bind<Target> + Offscreen<Target> + ExportMem,
        <R as Renderer>::TextureId: 'static,
        <R as Renderer>::Error: Into<SwapBuffersError> + Send + Sync + 'static,
        E: RenderElement<R>,
    {
        match self {
            SurfaceComposition::Surface {
                surface,
                damage_tracker,
                debug_flags,
            } => {
                let (dmabuf, age) = surface
                    .next_buffer()
                    .map_err(Into::<SwapBuffersError>::into)?;
                renderer
                    .bind(dmabuf)
                    .map_err(Into::<SwapBuffersError>::into)?;
                let current_debug_flags = renderer.debug_flags();
                renderer.set_debug_flags(*debug_flags);
                let res = damage_tracker
                    .render_output(renderer, age.into(), elements, clear_color)
                    .map(|res| {
                        let rendered = res.damage.is_some();
                        SurfaceCompositorRenderResult {
                            rendered,
                            damage: res.damage,
                            states: res.states,
                            sync: rendered.then_some(res.sync),
                        }
                    })
                    .map_err(|err| match err {
                        OutputDamageTrackerError::Rendering(err) => err.into(),
                        _ => unreachable!(),
                    });
                renderer.set_debug_flags(current_debug_flags);
                res
            }
            SurfaceComposition::Compositor(compositor) => compositor
                .render_frame(renderer, elements, clear_color, frame_mode)
                .map(|render_frame_result| SurfaceCompositorRenderResult {
                    rendered: !render_frame_result.is_empty,
                    damage: None,
                    states: render_frame_result.states,
                    sync: None,
                })
                .map_err(|err| match err {
                    smithay::backend::drm::compositor::RenderFrameError::PrepareFrame(err) => {
                        err.into()
                    }
                    smithay::backend::drm::compositor::RenderFrameError::RenderFrame(
                        OutputDamageTrackerError::Rendering(err),
                    ) => err.into(),
                    _ => unreachable!(),
                }),
        }
    }
}

struct SurfaceData {
    primary_node: DrmNode,
    render_node: DrmNode,
    // Holds not to `drop()`.
    #[allow(unused)]
    wl_output_global: WlGlobal<SabiniwmState, WlOutput>,
    compositor: SurfaceComposition,
    dmabuf_feedback: Option<SurfaceDmabufFeedback>,
    // Note that a render loop is run per CRTC. This might be not good with multiple displays.
    // Possible solution would be running only one render loop (or one per GPU) with highest refresh
    // rate.
    //
    // TODO: Investigate and support it.
    render_loop: RenderLoop<SabiniwmState>,
}

struct BackendData {
    surfaces: HashMap<crtc::Handle, SurfaceData>,
    non_desktop_connectors: Vec<(connector::Handle, crtc::Handle)>,
    leasing_global: Option<DrmLeaseState>,
    active_leases: Vec<DrmLease>,
    gbm: GbmDevice<DrmDeviceFd>,
    drm: DrmDevice,
    drm_scanner: DrmScanner,
    render_node: DrmNode,
    registration_token: RegistrationToken,
}

#[derive(Debug, thiserror::Error)]
enum DeviceAddError {
    #[error("Failed to open device using libseat: {0}")]
    DeviceOpen(libseat::Error),
    #[error("Failed to initialize drm device: {0}")]
    DrmDevice(DrmError),
    #[error("Failed to initialize gbm device: {0}")]
    GbmDevice(std::io::Error),
    #[error("Failed to access drm node: {0}")]
    DrmNode(CreateDrmNodeError),
    #[error("Failed to add device to GpuManager: {0}")]
    AddNode(egl::Error),
}

fn get_surface_dmabuf_feedback(
    selected_render_node: DrmNode,
    render_node: DrmNode,
    gpus: &mut GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>,
    composition: &SurfaceComposition,
) -> Option<SurfaceDmabufFeedback> {
    let primary_formats = gpus
        .single_renderer(&selected_render_node)
        .ok()?
        .dmabuf_formats();

    let render_formats = gpus.single_renderer(&render_node).ok()?.dmabuf_formats();

    let all_render_formats = primary_formats
        .iter()
        .chain(render_formats.iter())
        .copied()
        .collect::<FormatSet>();

    let surface = composition.surface();
    let planes = surface.planes().clone();

    // We limit the scan-out tranche to formats we can also render from
    // so that there is always a fallback render path available in case
    // the supplied buffer can not be scanned out directly
    let planes_formats = surface
        .plane_info()
        .formats
        .iter()
        .copied()
        .chain(planes.overlay.into_iter().flat_map(|p| p.formats))
        .collect::<FormatSet>()
        .intersection(&all_render_formats)
        .copied()
        .collect::<FormatSet>();

    let builder = DmabufFeedbackBuilder::new(selected_render_node.dev_id(), primary_formats);
    let render_feedback = builder
        .clone()
        .add_preference_tranche(render_node.dev_id(), None, render_formats.clone())
        .build()
        .unwrap();

    let scanout_feedback = builder
        .add_preference_tranche(
            surface.device_fd().dev_id().unwrap(),
            Some(zwp_linux_dmabuf_feedback_v1::TrancheFlags::Scanout),
            planes_formats,
        )
        .add_preference_tranche(render_node.dev_id(), None, render_formats)
        .build()
        .unwrap();

    Some(SurfaceDmabufFeedback {
        render_feedback,
        scanout_feedback,
    })
}

impl SabiniwmState {
    fn as_udev_mut(&mut self) -> SabiniwmStateWithConcreteBackend<'_, UdevBackend> {
        SabiniwmStateWithConcreteBackend {
            backend: self.backend.as_udev_mut(),
            inner: &mut self.inner,
        }
    }

    fn backend_udev(&self) -> &UdevBackend {
        self.backend.as_udev()
    }

    fn backend_udev_mut(&mut self) -> &mut UdevBackend {
        self.backend.as_udev_mut()
    }
}

impl SabiniwmStateWithConcreteBackend<'_, UdevBackend> {
    fn device_added(&mut self, node: DrmNode, path: &Path) -> Result<(), DeviceAddError> {
        assert_eq!(node.ty(), NodeType::Primary);

        // Try to open the device
        let fd = self
            .backend
            .session
            .open(
                path,
                OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
            )
            .map_err(DeviceAddError::DeviceOpen)?;

        let fd = DrmDeviceFd::new(DeviceFd::from(fd));

        let (drm, notifier) =
            DrmDevice::new(fd.clone(), true).map_err(DeviceAddError::DrmDevice)?;
        let gbm = GbmDevice::new(fd).map_err(DeviceAddError::GbmDevice)?;

        let registration_token = self
            .inner
            .loop_handle
            .insert_source(notifier, move |event, metadata, state| match event {
                DrmEvent::VBlank(crtc) => {
                    state.as_udev_mut().on_vblank(node, crtc, metadata);
                }
                DrmEvent::Error(error) => {
                    error!("{:?}", error);
                }
            })
            .unwrap();

        let render_node =
            EGLDevice::device_for_display(&unsafe { EGLDisplay::new(gbm.clone()).unwrap() })
                .ok()
                .and_then(|x| x.try_get_render_node().ok().flatten())
                .unwrap_or(node);

        self.backend
            .gpus
            .as_mut()
            .add_node(render_node, gbm.clone())
            .map_err(DeviceAddError::AddNode)?;

        // FIXME
        self.backend.backends.insert(
            node,
            BackendData {
                registration_token,
                gbm,
                drm,
                drm_scanner: DrmScanner::new(),
                non_desktop_connectors: Vec::new(),
                render_node,
                surfaces: HashMap::new(),
                leasing_global: DrmLeaseState::new::<SabiniwmState>(
                    &self.inner.display_handle,
                    &node,
                )
                .map_err(|err| {
                    // TODO replace with inspect_err, once stable
                    warn!(?err, "Failed to initialize drm lease global for: {}", node);
                    err
                })
                .ok(),
                active_leases: Vec::new(),
            },
        );

        self.device_changed(node);

        Ok(())
    }

    fn connector_connected(
        &mut self,
        node: DrmNode,
        connector: connector::Info,
        crtc: crtc::Handle,
    ) {
        assert_eq!(node.ty(), NodeType::Primary);

        let mut aux = || -> eyre::Result<()> {
            let device = self.backend.backends.get_mut(&node).ok_or_else(|| {
                eyre::eyre!(
                    "BackendData not found for: path = {}",
                    dev_path_or_na(&node)
                )
            })?;

            let mut renderer = self
                .backend
                .gpus
                .single_renderer(&device.render_node)
                .unwrap();
            let render_formats = renderer
                .as_mut()
                .egl_context()
                .dmabuf_render_formats()
                .clone();

            let output_name = format!(
                "{}-{}",
                connector.interface().as_str(),
                connector.interface_id()
            );
            info!(?crtc, "Trying to setup connector {}", output_name);

            let non_desktop = device
                .drm
                .get_properties(connector.handle())
                .ok()
                .and_then(|props| {
                    let (info, value) = props
                        .into_iter()
                        .filter_map(|(handle, value)| {
                            let info = device.drm.get_property(handle).ok()?;

                            Some((info, value))
                        })
                        .find(|(info, _)| info.name().to_str() == Ok("non-desktop"))?;

                    info.value_type().convert_value(value).as_boolean()
                })
                .unwrap_or(false);

            let display_info =
                smithay_drm_extras::display_info::for_connector(&device.drm, connector.handle());
            let make = display_info
                .as_ref()
                .and_then(|info| info.make())
                .unwrap_or_else(|| "Unknown".into());
            let model = display_info
                .as_ref()
                .and_then(|info| info.model())
                .unwrap_or_else(|| "Unknown".into());

            if non_desktop {
                info!(
                    "Connector {} is non-desktop, setting up for leasing",
                    output_name
                );
                device
                    .non_desktop_connectors
                    .push((connector.handle(), crtc));
                if let Some(lease_state) = device.leasing_global.as_mut() {
                    lease_state.add_connector::<SabiniwmState>(
                        connector.handle(),
                        output_name,
                        format!("{} {}", make, model),
                    );
                }
            } else {
                let (phys_w, phys_h) = connector.size().unwrap_or((0, 0));
                let output = smithay::output::Output::new(
                    output_name,
                    smithay::output::PhysicalProperties {
                        size: (phys_w as i32, phys_h as i32).into(),
                        subpixel: connector.subpixel().into(),
                        make,
                        model,
                    },
                );
                let wl_output_global = WlGlobal::<SabiniwmState, WlOutput>::new(
                    output.create_global::<SabiniwmState>(&self.inner.display_handle.clone()),
                    self.inner.display_handle.clone(),
                );

                let x = self.inner.space.outputs().fold(0, |acc, o| {
                    acc + self.inner.space.output_geometry(o).unwrap().size.w
                });
                let position = (x, 0).into();

                let (mode, scale) = self
                    .inner
                    .config_delegate
                    .select_mode_and_scale_on_connecter_added(&connector);
                output.set_preferred(mode.into());
                output.change_current_state(Some(mode.into()), None, Some(scale), Some(position));
                self.inner.space.map_output(&output, position);
                let size = self.inner.space.output_geometry(&output)
                    .unwrap(/* Space::map_output() and Output::change_current_state() is called. */)
                    .size;
                self.inner.view.resize_output(size, &mut self.inner.space);
                self.inner.on_output_added(&output);

                output.user_data().insert_if_missing(|| UdevOutputId {
                    primary_node: node,
                    crtc,
                });

                let allocator = GbmAllocator::new(
                    device.gbm.clone(),
                    GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
                );

                let color_formats = if self.inner.envvar.sabiniwm.disable_10bit {
                    SUPPORTED_FORMATS_8BIT_ONLY
                } else {
                    SUPPORTED_FORMATS
                };

                let surface = device
                    .drm
                    .create_surface(crtc, mode, &[connector.handle()])
                    .wrap_err("create drm surface")?;
                let compositor = match &self.inner.envvar.sabiniwm.surface_composition_policy {
                    SurfaceCompositionPolicy::UseGbmBufferedSurface => {
                        let gbm_surface = GbmBufferedSurface::new(
                            surface,
                            allocator,
                            color_formats,
                            render_formats,
                        )
                        .wrap_err("create rendering surface")?;
                        SurfaceComposition::Surface {
                            surface: gbm_surface,
                            damage_tracker: OutputDamageTracker::from_output(&output),
                            debug_flags: self.backend.debug_flags,
                        }
                    }
                    SurfaceCompositionPolicy::UseDrmCompositor => {
                        let driver = device.drm.get_driver().wrap_err("query drm driver")?;
                        let mut planes = surface.planes().clone();

                        // Using an overlay plane on a nvidia card breaks
                        if driver
                            .name()
                            .to_string_lossy()
                            .to_lowercase()
                            .contains("nvidia")
                            || driver
                                .description()
                                .to_string_lossy()
                                .to_lowercase()
                                .contains("nvidia")
                        {
                            planes.overlay = vec![];
                        }

                        let mut compositor = DrmCompositor::new(
                            &output,
                            surface,
                            Some(planes),
                            allocator,
                            device.gbm.clone(),
                            color_formats.iter().copied(),
                            render_formats,
                            device.drm.cursor_size(),
                            Some(device.gbm.clone()),
                        )
                        .wrap_err("DrmCompositor::new()")?;
                        compositor.set_debug_flags(self.backend.debug_flags);
                        SurfaceComposition::Compositor(compositor)
                    }
                };

                let dmabuf_feedback = get_surface_dmabuf_feedback(
                    self.backend.selected_render_node,
                    device.render_node,
                    &mut self.backend.gpus,
                    &compositor,
                );

                let mut render_loop =
                    RenderLoop::new(self.inner.loop_handle.clone(), &output, move |state| {
                        state.as_udev_mut().render(node, Some(crtc));
                    });
                render_loop.start();

                let surface = SurfaceData {
                    primary_node: node,
                    render_node: device.render_node,
                    wl_output_global,
                    compositor,
                    dmabuf_feedback,
                    render_loop,
                };

                device.surfaces.insert(crtc, surface);

                self.schedule_initial_render(node, crtc, self.inner.loop_handle.clone());
            }

            Ok(())
        };

        match aux() {
            Ok(()) => {}
            Err(e) => {
                error!("{:?}", e);
            }
        }
    }

    fn connector_disconnected(
        &mut self,
        node: DrmNode,
        connector: connector::Info,
        crtc: crtc::Handle,
    ) {
        assert_eq!(node.ty(), NodeType::Primary);

        let Some(device) = self.backend.backends.get_mut(&node) else {
            return;
        };

        if let Some(pos) = device
            .non_desktop_connectors
            .iter()
            .position(|(handle, _)| *handle == connector.handle())
        {
            let _ = device.non_desktop_connectors.remove(pos);
            if let Some(leasing_state) = device.leasing_global.as_mut() {
                leasing_state.withdraw_connector(connector.handle());
            }
        } else {
            device.surfaces.remove(&crtc);

            let output = self
                .inner
                .space
                .outputs()
                .find(|o| {
                    o.user_data()
                        .get::<UdevOutputId>()
                        .map(|id| id.primary_node == node && id.crtc == crtc)
                        .unwrap_or(false)
                })
                .cloned();

            if let Some(output) = output {
                self.inner.space.unmap_output(&output);
            }
        }
    }

    fn device_changed(&mut self, node: DrmNode) {
        assert_eq!(node.ty(), NodeType::Primary);

        let Some(device) = self.backend.backends.get_mut(&node) else {
            return;
        };

        let scan_result = match device.drm_scanner.scan_connectors(&device.drm) {
            Ok(scan_result) => scan_result,
            Err(err) => {
                warn!(?err, "Failed to scan connectors");
                return;
            }
        };
        for event in scan_result {
            match event {
                DrmScanEvent::Connected { connector, crtc } => {
                    if let Some(crtc) = crtc {
                        self.connector_connected(node, connector, crtc);
                    }
                }
                DrmScanEvent::Disconnected { connector, crtc } => {
                    if let Some(crtc) = crtc {
                        self.connector_disconnected(node, connector, crtc);
                    }
                }
            }
        }
    }

    fn device_removed(&mut self, node: DrmNode) {
        assert_eq!(node.ty(), NodeType::Primary);

        let crtcs = {
            let Some(device) = self.backend.backends.get_mut(&node) else {
                return;
            };

            let crtcs: Vec<_> = device
                .drm_scanner
                .crtcs()
                .map(|(info, crtc)| (info.clone(), crtc))
                .collect();
            crtcs
        };

        for (connector, crtc) in crtcs {
            self.connector_disconnected(node, connector, crtc);
        }

        debug!("Surfaces dropped");

        // drop the backends on this side
        if let Some(mut backend_inner) = self.backend.backends.remove(&node) {
            if let Some(mut leasing_global) = backend_inner.leasing_global.take() {
                leasing_global.disable_global::<SabiniwmState>();
            }

            self.backend
                .gpus
                .as_mut()
                .remove_node(&backend_inner.render_node);

            self.inner
                .loop_handle
                .remove(backend_inner.registration_token);

            debug!("Dropping device");
        }
    }

    fn on_vblank(
        &mut self,
        node: DrmNode,
        crtc: crtc::Handle,
        metadata: &mut Option<DrmEventMetadata>,
    ) {
        assert_eq!(node.ty(), NodeType::Primary);

        let Some(backend) = self.backend.backends.get_mut(&node) else {
            error!("Trying to finish frame on non-existent backend {}", node);
            return;
        };

        let Some(surface) = backend.surfaces.get_mut(&crtc) else {
            error!("Trying to finish frame on non-existent crtc {:?}", crtc);
            return;
        };

        let output = if let Some(output) = self.inner.space.outputs().find(|o| {
            o.user_data().get::<UdevOutputId>()
                == Some(&UdevOutputId {
                    primary_node: surface.primary_node,
                    crtc,
                })
        }) {
            output.clone()
        } else {
            // somehow we got called with an invalid output
            return;
        };

        let should_schedule_render = match surface
            .compositor
            .frame_submitted()
            .map_err(Into::<SwapBuffersError>::into)
        {
            Ok(user_data) => {
                if let Some(mut feedback) = user_data.flatten() {
                    let tp = metadata.as_ref().and_then(|metadata| match metadata.time {
                        smithay::backend::drm::DrmEventTime::Monotonic(tp) => Some(tp),
                        smithay::backend::drm::DrmEventTime::Realtime(_) => None,
                    });
                    let seq = metadata
                        .as_ref()
                        .map(|metadata| metadata.sequence)
                        .unwrap_or(0);

                    let (clock, flags) = if let Some(tp) = tp {
                        (
                            tp.into(),
                            wp_presentation_feedback::Kind::Vsync
                                | wp_presentation_feedback::Kind::HwClock
                                | wp_presentation_feedback::Kind::HwCompletion,
                        )
                    } else {
                        (
                            self.inner.clock.now(),
                            wp_presentation_feedback::Kind::Vsync,
                        )
                    };

                    use smithay::wayland::presentation::Refresh;
                    feedback.presented(
                        clock,
                        output
                            .current_mode()
                            .map(|mode| {
                                Refresh::fixed(Duration::from_secs_f64(
                                    1_000f64 / mode.refresh as f64,
                                ))
                            })
                            .unwrap_or(Refresh::Unknown),
                        seq as u64,
                        flags,
                    );
                }

                true
            }
            Err(err) => {
                warn!("Error during rendering: {:?}", err);
                match err {
                    SwapBuffersError::AlreadySwapped => true,
                    SwapBuffersError::TemporaryFailure(err) => match err.downcast_ref::<DrmError>()
                    {
                        // If the device has been deactivated do not reschedule, this will be
                        // done by session resume.
                        Some(DrmError::DeviceInactive) => false,
                        Some(DrmError::Access(DrmAccessError { source, .. }))
                            if source.kind() == std::io::ErrorKind::PermissionDenied =>
                        {
                            true
                        }
                        _ => false,
                    },
                    SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
                }
            }
        };

        if should_schedule_render {
            surface.render_loop.on_vblank();
        }
    }

    // If crtc is `Some()`, render it, else render all crtcs
    fn render(&mut self, node: DrmNode, crtc: Option<crtc::Handle>) {
        let Some(backend) = self.backend.backends.get_mut(&node) else {
            error!("Trying to render on non-existent backend {}", node);
            return;
        };

        if let Some(crtc) = crtc {
            self.render_surface(node, crtc);
        } else {
            let crtcs: Vec<_> = backend.surfaces.keys().copied().collect();
            for crtc in crtcs {
                self.render_surface(node, crtc);
            }
        };
    }

    fn render_surface(&mut self, node: DrmNode, crtc: crtc::Handle) {
        let Some(device) = self.backend.backends.get_mut(&node) else {
            return;
        };

        let Some(surface) = device.surfaces.get_mut(&crtc) else {
            return;
        };

        let output = if let Some(output) = self.inner.space.outputs().find(|o| {
            o.user_data().get::<UdevOutputId>()
                == Some(&UdevOutputId {
                    primary_node: surface.primary_node,
                    crtc,
                })
        }) {
            output.clone()
        } else {
            // somehow we got called with an invalid output
            return;
        };

        // TODO get scale from the rendersurface when supporting HiDPI
        let frame = self
            .backend
            .pointer_image
            .get_image(1 /*scale*/, self.inner.clock.now().into());

        let render_node = surface.render_node;
        let selected_render_node = self.backend.selected_render_node;
        let mut renderer = if selected_render_node == render_node {
            self.backend.gpus.single_renderer(&render_node)
        } else {
            let format = surface.compositor.format();
            self.backend
                .gpus
                .renderer(&selected_render_node, &render_node, format)
        }
        .unwrap();

        let pointer_images = &mut self.backend.pointer_images;
        let pointer_image = pointer_images
            .iter()
            .find_map(|(image, texture)| {
                if image == &frame {
                    Some(texture.clone())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                let buffer = MemoryRenderBuffer::from_slice(
                    &frame.pixels_rgba,
                    Fourcc::Argb8888,
                    (frame.width as i32, frame.height as i32),
                    1,
                    Transform::Normal,
                    None,
                );
                pointer_images.push((frame, buffer.clone()));
                buffer
            });

        let additional_elements = make_additional_elements(
            &mut renderer,
            &self.inner.space,
            &output,
            self.inner.pointer.current_location(),
            &pointer_image,
            &mut self.backend.pointer_element,
            &self.inner.dnd_icon,
            &mut self.inner.cursor_status,
        );
        let (elements, clear_color) =
            self.inner
                .make_output_elements(&mut renderer, &output, additional_elements);
        let result = self.inner.render_surface_data(
            surface,
            &mut renderer,
            &output,
            elements,
            clear_color,
            self.inner.frame_mode(),
        );
        let should_reschedule_render = match &result {
            Ok(has_rendered) => !has_rendered,
            Err(err) => {
                warn!("Error during rendering: {:?}", err);
                match err {
                    SwapBuffersError::AlreadySwapped => false,
                    SwapBuffersError::TemporaryFailure(err) => match err.downcast_ref::<DrmError>()
                    {
                        Some(DrmError::DeviceInactive) => true,
                        Some(DrmError::Access(DrmAccessError { source, .. })) => {
                            source.kind() == std::io::ErrorKind::PermissionDenied
                        }
                        _ => false,
                    },
                    SwapBuffersError::ContextLost(err) => match err.downcast_ref::<DrmError>() {
                        Some(DrmError::TestFailed(_)) => {
                            // reset the complete state, disabling all connectors and planes in case we hit a test failed
                            // most likely we hit this after a tty switch when a foreign master changed CRTC <-> connector bindings
                            // and we run in a mismatch
                            device
                                .drm
                                .reset_state()
                                .expect("failed to reset drm device");
                            true
                        }
                        _ => panic!("Rendering loop lost: {}", err),
                    },
                }
            }
        };

        // TODO: Check that this is reasonable for the above `Err` case.
        surface
            .render_loop
            .on_render_frame(should_reschedule_render);
    }

    fn schedule_initial_render(
        &mut self,
        node: DrmNode,
        crtc: crtc::Handle,
        evt_handle: LoopHandle<'static, SabiniwmState>,
    ) {
        let Some(device) = self.backend.backends.get_mut(&node) else {
            return;
        };

        let Some(surface) = device.surfaces.get_mut(&crtc) else {
            return;
        };

        let node = surface.render_node;
        let result = {
            let mut renderer = self.backend.gpus.single_renderer(&node).unwrap();
            initial_render(surface, &mut renderer, self.inner.frame_mode())
        };

        if let Err(err) = result {
            match err {
                SwapBuffersError::AlreadySwapped => {}
                SwapBuffersError::TemporaryFailure(err) => {
                    // TODO dont reschedule after 3(?) retries
                    warn!("Failed to submit page_flip: {}", err);
                    let handle = evt_handle.clone();
                    evt_handle.insert_idle(move |state| {
                        state
                            .as_udev_mut()
                            .schedule_initial_render(node, crtc, handle)
                    });
                }
                SwapBuffersError::ContextLost(err) => panic!("Rendering loop lost: {}", err),
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn make_additional_elements<R>(
    renderer: &mut R,
    space: &Space<crate::view::window::Window>,
    output: &smithay::output::Output,
    pointer_location: Point<f64, Logical>,
    pointer_image: &MemoryRenderBuffer,
    pointer_element: &mut PointerElement,
    dnd_icon: &Option<DndIcon>,
    cursor_status: &mut CursorImageStatus,
) -> Vec<CustomRenderElement<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    let output_geometry = space.output_geometry(output).unwrap();
    let scale = Scale::from(output.current_scale().fractional_scale());

    let mut elements: Vec<CustomRenderElement<_>> = Vec::new();

    if output_geometry.to_f64().contains(pointer_location) {
        let cursor_hotspot = if let CursorImageStatus::Surface(surface) = cursor_status {
            compositor::with_states(surface, |states| {
                states
                    .data_map
                    .get::<Mutex<CursorImageAttributes>>()
                    .unwrap()
                    .lock()
                    .unwrap()
                    .hotspot
            })
        } else {
            (0, 0).into()
        };
        let cursor_pos = pointer_location - output_geometry.loc.to_f64();

        // set cursor
        pointer_element.set_buffer(pointer_image.clone());

        // draw the cursor as relevant
        {
            // reset the cursor if the surface is no longer alive
            let should_reset = if let CursorImageStatus::Surface(surface) = cursor_status {
                !surface.alive()
            } else {
                false
            };
            if should_reset {
                *cursor_status = CursorImageStatus::default_named();
            }

            pointer_element.set_status(cursor_status.clone());

            let cursor_lefttop_pos = (cursor_pos - cursor_hotspot.to_f64())
                .to_physical(scale)
                .to_i32_round();
            elements.extend(pointer_element.render_elements(
                renderer,
                cursor_lefttop_pos,
                scale,
                1.0,
            ));
        }

        // draw the dnd icon if applicable
        if let Some(dnd_icon) = dnd_icon.as_ref() {
            let dnd_icon_pos = (cursor_pos + dnd_icon.offset.to_f64())
                .to_physical(scale)
                .to_i32_round();
            if dnd_icon.surface.alive() {
                elements.extend(
                    SurfaceTree::from_surface(&dnd_icon.surface).render_elements(
                        renderer,
                        dnd_icon_pos,
                        scale,
                        1.0,
                    ),
                );
            }
        }
    }

    elements
}

impl InnerState {
    fn frame_mode(&self) -> smithay::backend::drm::compositor::FrameMode {
        use smithay::backend::drm::compositor::FrameMode;

        if self.envvar.sabiniwm.enable_direct_scanout {
            FrameMode::ALL
        } else {
            FrameMode::COMPOSITE
        }
    }

    fn render_surface_data<R>(
        &self,
        surface: &mut SurfaceData,
        renderer: &mut R,
        output: &smithay::output::Output,
        elements: Vec<OutputRenderElement<R, WindowRenderElement<R>>>,
        clear_color: Color32F,
        frame_mode: smithay::backend::drm::compositor::FrameMode,
    ) -> Result<bool, SwapBuffersError>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + 'static,
        R: ExportMem + Offscreen<GlesTexture> + smithay::backend::renderer::Bind<Dmabuf>,
        <R as Renderer>::Error: Into<smithay::backend::SwapBuffersError> + Send + Sync + 'static,
    {
        let SurfaceCompositorRenderResult {
            rendered,
            states,
            sync,
            damage,
        } = surface.compositor.render_frame::<_, _, GlesTexture>(
            renderer,
            &elements,
            clear_color,
            frame_mode,
        )?;

        self.post_repaint(
            output,
            &states,
            surface.dmabuf_feedback.clone(),
            self.clock.now().into(),
        );

        if rendered {
            let output_presentation_feedback = self.take_presentation_feedback(output, &states);
            let damage = damage.cloned();
            surface
                .compositor
                .queue_frame(sync, damage, Some(output_presentation_feedback))
                .map_err(Into::<SwapBuffersError>::into)?;
        }

        Ok(rendered)
    }
}

fn initial_render(
    surface: &mut SurfaceData,
    renderer: &mut UdevRenderer<'_>,
    frame_mode: smithay::backend::drm::compositor::FrameMode,
) -> Result<(), SwapBuffersError> {
    surface
        .compositor
        .render_frame::<_, CustomRenderElement<_>, GlesTexture>(
            renderer,
            &[],
            CLEAR_COLOR,
            frame_mode,
        )?;
    surface.compositor.queue_frame(None, None, None)?;
    surface.compositor.reset_buffers();

    Ok(())
}

/// Gets path of DRM node. Returns "N/A" if it's unavailable.
fn dev_path_or_na(node: &DrmNode) -> String {
    match node.dev_path() {
        Some(path) => format!("{}", path.display()),
        None => "N/A".to_string(),
    }
}

impl EventHandler<UdevEvent> for SabiniwmStateWithConcreteBackend<'_, UdevBackend> {
    fn handle_event(&mut self, event: UdevEvent) {
        match event {
            UdevEvent::Added { device_id, path } => {
                let mut aux = || {
                    let node = DrmNode::from_dev_id(device_id).map_err(DeviceAddError::DrmNode)?;
                    assert_eq!(node.ty(), NodeType::Primary);

                    self.device_added(node, &path)
                };
                match aux() {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Skipping to add device: device_id = {device_id}, error = {e}");
                    }
                }
            }
            UdevEvent::Changed { device_id } => {
                let Ok(node) = DrmNode::from_dev_id(device_id) else {
                    return;
                };
                assert_eq!(node.ty(), NodeType::Primary);

                self.device_changed(node);
            }
            UdevEvent::Removed { device_id } => {
                let Ok(node) = DrmNode::from_dev_id(device_id) else {
                    return;
                };
                assert_eq!(node.ty(), NodeType::Primary);

                self.device_removed(node);
            }
        }
    }
}

impl EventHandler<InputEvent<LibinputInputBackend>> for SabiniwmState {
    fn handle_event(&mut self, event: InputEvent<LibinputInputBackend>) {
        match event {
            InputEvent::DeviceAdded { mut device } => {
                info!("InputEvent::DeviceAdded:{:?}", LibinputDeviceInfo(&device));

                self.as_udev_mut()
                    .backend
                    .input_devices
                    .insert(device.clone());

                if device.has_capability(libinput::DeviceCapability::Keyboard) {
                    if let Some(led_state) = self
                        .inner
                        .seat
                        .get_keyboard()
                        .map(|keyboard| keyboard.led_state())
                    {
                        device.led_update(led_state.into());
                    }
                }

                if device.has_capability(libinput::DeviceCapability::Pointer) {
                    let _ = device.config_send_events_set_mode(libinput::SendEventsMode::ENABLED);
                }

                if device.has_capability(libinput::DeviceCapability::Touch) {
                    let _ = device.config_send_events_set_mode(libinput::SendEventsMode::ENABLED);
                }

                self.inner
                    .config_delegate
                    .config_input_device_on_added(&mut device);
            }
            InputEvent::DeviceRemoved { device } => {
                info!(
                    "InputEvent::DeviceRemoved:{:?}",
                    LibinputDeviceInfo(&device)
                );

                self.as_udev_mut().backend.input_devices.remove(&device);
            }
            _ => {
                self.process_input_event(event);
            }
        }
    }
}

struct LibinputDeviceInfo<'a>(&'a libinput::Device);

impl std::fmt::Debug for LibinputDeviceInfo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut capabilities = vec![];
        for c in [
            libinput::DeviceCapability::Keyboard,
            libinput::DeviceCapability::Pointer,
            libinput::DeviceCapability::Touch,
            libinput::DeviceCapability::TabletTool,
            libinput::DeviceCapability::TabletPad,
            libinput::DeviceCapability::Gesture,
            libinput::DeviceCapability::Switch,
        ] {
            if self.0.has_capability(c) {
                capabilities.push(c);
            }
        }

        f.debug_struct("")
            .field("sysname", &self.0.sysname())
            .field("name", &self.0.name())
            .field("path", &smithay::backend::input::Device::syspath(self.0))
            .field("capabilities", &capabilities)
            .finish()
    }
}

impl EventHandler<smithay::backend::session::Event>
    for SabiniwmStateWithConcreteBackend<'_, UdevBackend>
{
    fn handle_event(&mut self, event: smithay::backend::session::Event) {
        match event {
            SessionEvent::PauseSession => {
                self.backend.libinput_context.suspend();
                info!("pausing session");

                for backend in self.backend.backends.values_mut() {
                    backend.drm.pause();
                    backend.active_leases.clear();
                    if let Some(lease_global) = backend.leasing_global.as_mut() {
                        lease_global.suspend();
                    }

                    for surface in backend.surfaces.values_mut() {
                        surface.render_loop.stop();
                    }
                }
            }
            SessionEvent::ActivateSession => {
                info!("resuming session");

                if let Err(err) = self.backend.libinput_context.resume() {
                    error!("Failed to resume libinput context: {:?}", err);
                }
                for backend in self.backend.backends.values_mut() {
                    // if we do not care about flicking (caused by modesetting) we could just
                    // pass true for disable connectors here. this would make sure our drm
                    // device is in a known state (all connectors and planes disabled).
                    // but for demonstration we choose a more optimistic path by leaving the
                    // state as is and assume it will just work. If this assumption fails
                    // we will try to reset the state when trying to queue a frame.
                    backend
                        .drm
                        .activate(false)
                        .expect("failed to activate drm backend");
                    if let Some(lease_global) = backend.leasing_global.as_mut() {
                        lease_global.resume::<SabiniwmState>();
                    }
                    for surface in backend.surfaces.values_mut() {
                        if let Err(err) = surface.compositor.reset_state() {
                            warn!("Failed to reset drm surface state: {}", err);
                        }

                        surface.render_loop.start();
                    }
                }
            }
        }
    }
}
