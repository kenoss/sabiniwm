mod props {
    use smithay::utils::{Logical, Rectangle};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Thickness {
        pub top: u32,
        pub right: u32,
        pub bottom: u32,
        pub left: u32,
    }

    impl From<u32> for Thickness {
        fn from(x: u32) -> Self {
            Self {
                top: x,
                right: x,
                bottom: x,
                left: x,
            }
        }
    }

    impl From<(u32, u32)> for Thickness {
        fn from((y, x): (u32, u32)) -> Self {
            Self {
                top: y,
                right: x,
                bottom: y,
                left: x,
            }
        }
    }

    impl From<(u32, u32, u32, u32)> for Thickness {
        fn from((top, right, bottom, left): (u32, u32, u32, u32)) -> Self {
            Self {
                top,
                right,
                bottom,
                left,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Rgba {
        pub r: u8,
        pub g: u8,
        pub b: u8,
        pub a: u8,
    }

    impl Rgba {
        pub fn from_rgba(hex: u32) -> Self {
            let r = (hex >> 24) as u8;
            let g = (hex >> 16) as u8;
            let b = (hex >> 8) as u8;
            let a = hex as u8;
            Self { r, g, b, a }
        }

        pub fn from_rgb(hex: u32) -> Self {
            assert_eq!(hex >> 24, 0);
            let r = (hex >> 16) as u8;
            let g = (hex >> 8) as u8;
            let b = hex as u8;
            let a = 0xff;
            Self { r, g, b, a }
        }

        pub fn to_f32_array(&self) -> [f32; 4] {
            fn convert(x: u8) -> f32 {
                x as f32 / 0xff as f32
            }

            [
                convert(self.r),
                convert(self.g),
                convert(self.b),
                convert(self.a),
            ]
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Border {
        pub dim: Thickness,
        pub active_rgba: Rgba,
        pub inactive_rgba: Rgba,
    }

    #[derive(Debug, Clone)]
    pub struct WindowProps {
        pub geometry: Rectangle<i32, Logical>,
        pub border: Border,
    }
}

#[allow(clippy::module_inception)]
mod window {
    use super::props::*;
    use crate::model::grid_geometry::RectangleExt;
    use crate::util::Id;
    use itertools::Itertools;
    use smithay::backend::renderer::element::solid::SolidColorBuffer;
    use smithay::desktop::space::SpaceElement;
    use smithay::reexports::wayland_server;
    use smithay::utils::{IsAlive, Logical, Physical, Point, Rectangle, Scale, Size};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    struct Ssd {
        // top, right, bottom, left
        borders: [SolidColorBuffer; 4],
        // Relative locations from top_left: top, right, bottom, left.
        relative_locs: [Point<i32, Logical>; 4],
    }

    impl Ssd {
        fn new() -> Self {
            Self {
                borders: [
                    SolidColorBuffer::default(),
                    SolidColorBuffer::default(),
                    SolidColorBuffer::default(),
                    SolidColorBuffer::default(),
                ],
                relative_locs: [(0, 0).into(); 4],
            }
        }
    }

    // Note that `SpaceElement` almost necessarily requires `Clone + PartialEq` because, for example, for
    // `Space::map_element()`. And some methods is called with `&self` while it should have `&mut self`, e.g.
    // `SpaceElement::set_activate()`. So, we wrap `WindowInner`.
    #[derive(Clone)]
    pub struct Window {
        id: Id<Window>,
        inner: Arc<Mutex<WindowInner>>,
        swindow: smithay::desktop::Window,
    }

    struct WindowInner {
        props: WindowProps,
        ssd: Option<Ssd>,
    }

    impl PartialEq for Window {
        fn eq(&self, other: &Self) -> bool {
            debug_assert_eq!(self.id == other.id, self.swindow == other.swindow);

            self.id == other.id
        }
    }

    impl Eq for Window {}

    impl Window {
        pub fn new(swindow: smithay::desktop::Window) -> Self {
            let geometry = swindow.geometry();
            let border = Border {
                dim: 1.into(),
                active_rgba: Rgba::from_rgba(0x00000000),
                inactive_rgba: Rgba::from_rgba(0x00000000),
            };
            let inner = WindowInner {
                props: WindowProps { geometry, border },
                ssd: Some(Ssd::new()),
            };
            let inner = Arc::new(Mutex::new(inner));
            Self {
                id: Id::new(),
                inner,
                swindow,
            }
        }

        pub fn id(&self) -> Id<Window> {
            self.id
        }

        // TODO: Remove.
        pub fn smithay_window(&self) -> &smithay::desktop::Window {
            &self.swindow
        }

        pub fn toplevel(&self) -> Option<&smithay::wayland::shell::xdg::ToplevelSurface> {
            self.swindow.toplevel()
        }

        pub fn on_commit(&self) {
            self.swindow.on_commit();
        }

        pub fn surface_under<P: Into<Point<f64, Logical>>>(
            &self,
            point: P,
            surface_type: smithay::desktop::WindowSurfaceType,
        ) -> Option<(
            wayland_server::protocol::wl_surface::WlSurface,
            Point<i32, Logical>,
        )> {
            let point = point.into();
            self.swindow.surface_under(point, surface_type)
        }

        pub fn send_frame<T, F>(
            &self,
            output: &smithay::output::Output,
            time: T,
            throttle: Option<Duration>,
            primary_scan_out_output: F,
        ) where
            T: Into<Duration>,
            F: FnMut(
                    &wayland_server::protocol::wl_surface::WlSurface,
                    &smithay::wayland::compositor::SurfaceData,
                ) -> Option<smithay::output::Output>
                + Copy,
        {
            let time = time.into();
            self.swindow
                .send_frame(output, time, throttle, primary_scan_out_output)
        }

        pub fn set_props(&mut self, props: WindowProps) {
            self.inner.lock().unwrap().props = props;
            self.update_ssd()
        }

        fn update_ssd(&mut self) {
            use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

            let Some(surface) = self.swindow.toplevel() else {
                return;
            };
            let activated = surface
                .with_pending_state(|state| state.states.contains(xdg_toplevel::State::Activated));

            self.inner.lock().unwrap().update_ssd(activated);
        }

        fn update_ssd_nonmut(&self) {
            use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;

            let Some(surface) = self.swindow.toplevel() else {
                return;
            };
            let activated = surface
                .with_pending_state(|state| state.states.contains(xdg_toplevel::State::Activated));

            let inner = self.inner.clone();
            inner.lock().unwrap().update_ssd(activated);
        }
    }

    impl IsAlive for Window {
        fn alive(&self) -> bool {
            self.swindow.alive()
        }
    }

    impl SpaceElement for Window {
        fn geometry(&self) -> Rectangle<i32, Logical> {
            let props = &self.inner.lock().unwrap().props;
            let mut geometry = props.geometry;
            // Ad-hoc: Use of geometry/bbox in smithay is problematic.
            //
            // In smithay, actual geometry/bbox is `space_element.geometry/bbox()` shifted with location of space element, which is
            // updated with `Space::map_element()`. See `InnerEleemnt::geometry()` and `InnerElement::bbox()`. So, we should keep
            // `geometry/bbox.loc` to (0, 0).
            geometry.loc = Point::default();

            geometry
        }

        fn bbox(&self) -> Rectangle<i32, Logical> {
            let props = &self.inner.lock().unwrap().props;
            let mut bbox = props.geometry.inflate(props.border.dim.clone());
            // Ditto.
            bbox.loc = Point::default();

            bbox
        }

        fn is_in_input_region(&self, point: &Point<f64, Logical>) -> bool {
            self.swindow.is_in_input_region(point)
        }

        fn z_index(&self) -> u8 {
            0
        }

        fn set_activate(&self, activated: bool) {
            self.swindow.set_activate(activated);
            self.update_ssd_nonmut();
        }

        fn output_enter(&self, output: &smithay::output::Output, overlap: Rectangle<i32, Logical>) {
            self.swindow.output_enter(output, overlap);
        }

        fn output_leave(&self, output: &smithay::output::Output) {
            self.swindow.output_leave(output);
        }

        fn refresh(&self) {
            self.swindow.refresh();
        }
    }

    impl WindowInner {
        fn update_ssd(&mut self, activated: bool) {
            let border = &self.props.border.clone();
            if let Some(ref mut ssd) = &mut self.ssd {
                let bbox: Size<i32, Logical> = (
                    self.props.geometry.size.w + (border.dim.left + border.dim.right) as i32,
                    self.props.geometry.size.h + (border.dim.top + border.dim.bottom) as i32,
                )
                    .into();
                let rgba = if activated {
                    &border.active_rgba
                } else {
                    &border.inactive_rgba
                };
                let color = rgba.to_f32_array();

                let size_top: Size<i32, Logical> = (bbox.w, border.dim.top as i32).into();
                ssd.borders[0].update(size_top, color);
                ssd.relative_locs[0] = (0, 0).into();

                let size_right: Size<i32, Logical> = (border.dim.right as i32, bbox.h).into();
                ssd.borders[1].update(size_right, color);
                ssd.relative_locs[1] = (bbox.w - border.dim.right as i32, 0).into();

                let size_bottom: Size<i32, Logical> = (bbox.w, border.dim.bottom as i32).into();
                ssd.borders[2].update(size_bottom, color);
                ssd.relative_locs[2] = (0, bbox.h - border.dim.bottom as i32).into();

                let size_left: Size<i32, Logical> = (border.dim.left as i32, bbox.h).into();
                ssd.borders[3].update(size_left, color);
                ssd.relative_locs[3] = (0, 0).into();
            }
        }
    }

    pub(crate) mod as_render_elements {
        use super::*;
        use smithay::backend::renderer::element::solid::SolidColorRenderElement;
        use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
        use smithay::backend::renderer::element::AsRenderElements;
        use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};

        #[derive(derive_more::From)]
        #[thin_delegate::register]
        pub enum WindowRenderElement<R>
        where
            R: Renderer,
        {
            Window(WaylandSurfaceRenderElement<R>),
            Decoration(SolidColorRenderElement),
        }

        #[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
        impl<R> smithay::backend::renderer::element::Element for WindowRenderElement<R>
        where
            R: smithay::backend::renderer::Renderer,
            <R as smithay::backend::renderer::RendererSuper>::TextureId: 'static,
            R: ImportAll + ImportMem,
        {
        }

        #[thin_delegate::fill_delegate(external_trait_def = crate::external_trait_def::smithay::backend::renderer::element)]
        impl<R> smithay::backend::renderer::element::RenderElement<R> for WindowRenderElement<R>
        where
            R: smithay::backend::renderer::Renderer,
            <R as smithay::backend::renderer::RendererSuper>::TextureId: 'static,
            R: ImportAll + ImportMem,
        {
        }

        impl<R> AsRenderElements<R> for Window
        where
            R: Renderer + ImportAll + ImportMem,
            R::TextureId: Texture + Clone + 'static,
        {
            type RenderElement = WindowRenderElement<R>;

            fn render_elements<C>(
                &self,
                renderer: &mut R,
                location: Point<i32, Physical>,
                scale: Scale<f64>,
                alpha: f32,
            ) -> Vec<C>
            where
                C: From<Self::RenderElement>,
            {
                let mut ret = AsRenderElements::render_elements(
                    &self.swindow,
                    renderer,
                    location,
                    scale,
                    alpha,
                )
                .into_iter()
                .map(C::from)
                .collect_vec();

                let inner = self.inner.lock().unwrap();
                if let Some(ssd) = &inner.ssd {
                    let mut left_top = location;
                    let border = &inner.props.border;
                    left_top.x -= border.dim.left as i32;
                    left_top.y -= border.dim.top as i32;

                    for i in 0..4 {
                        let rloc = &ssd.relative_locs[i];
                        let rloc: Point<f64, Logical> = (rloc.x as f64, rloc.y as f64).into();
                        let loc = left_top + rloc.to_physical_precise_round(scale);

                        ret.push(
                            WindowRenderElement::Decoration(SolidColorRenderElement::from_buffer(
                                &ssd.borders[i],
                                loc,
                                scale,
                                alpha,
                                smithay::backend::renderer::element::Kind::Unspecified,
                            ))
                            .into(),
                        )
                    }
                }

                ret
            }
        }
    }
}

pub(crate) use props::*;
pub(crate) use window::as_render_elements::*;
pub(crate) use window::*;
