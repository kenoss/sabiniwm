pub(crate) mod smithay {
    pub(crate) mod backend {
        pub(crate) mod renderer {
            #[thin_delegate::external_trait_def(with_uses = true)]
            pub(crate) mod element {
                use smithay::backend::renderer::element::{Id, Kind, UnderlyingStorage};
                use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
                use smithay::backend::renderer::Renderer;
                use smithay::utils::{
                    Buffer as BufferCoords, Physical, Point, Rectangle, Scale, Transform,
                };

                #[thin_delegate::register]
                pub trait Element {
                    /// Get the unique id of this element
                    fn id(&self) -> &Id;
                    /// Get the current commit position of this element
                    fn current_commit(&self) -> CommitCounter;
                    /// Get the location relative to the output
                    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
                        self.geometry(scale).loc
                    }
                    /// Get the src of the underlying buffer
                    fn src(&self) -> Rectangle<f64, BufferCoords>;
                    /// Get the transform of the underlying buffer
                    fn transform(&self) -> Transform {
                        Transform::Normal
                    }
                    /// Get the geometry relative to the output
                    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical>;
                    /// Get the damage since the provided commit relative to the element
                    fn damage_since(
                        &self,
                        scale: Scale<f64>,
                        commit: Option<CommitCounter>,
                    ) -> DamageSet<i32, Physical> {
                        if commit != Some(self.current_commit()) {
                            DamageSet::from_slice(&[Rectangle::from_size(
                                self.geometry(scale).size,
                            )])
                        } else {
                            DamageSet::default()
                        }
                    }
                    /// Get the opaque regions of the element relative to the element
                    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
                        OpaqueRegions::default()
                    }
                    /// Returns an alpha value the element should be drawn with regardless of any
                    /// already encoded alpha in it's underlying representation.
                    fn alpha(&self) -> f32 {
                        1.0
                    }
                    /// Returns the [`Kind`] for this element
                    fn kind(&self) -> Kind {
                        Kind::default()
                    }
                }

                #[thin_delegate::register]
                pub trait RenderElement<R: Renderer>: Element {
                    /// Draw this element
                    fn draw(
                        &self,
                        frame: &mut R::Frame<'_, '_>,
                        src: Rectangle<f64, BufferCoords>,
                        dst: Rectangle<i32, Physical>,
                        damage: &[Rectangle<i32, Physical>],
                        opaque_regions: &[Rectangle<i32, Physical>],
                    ) -> Result<(), R::Error>;

                    /// Get the underlying storage of this element, may be used to optimize rendering (eg. drm planes)
                    #[inline]
                    fn underlying_storage(
                        &self,
                        renderer: &mut R,
                    ) -> Option<UnderlyingStorage<'_>> {
                        let _ = renderer;
                        None
                    }
                }
            }
        }
    }

    pub(crate) mod input {
        #[thin_delegate::external_trait_def(with_uses = true)]
        pub(crate) mod keyboard {
            use ::smithay::backend::input::KeyState;
            use ::smithay::input::keyboard::{KeysymHandle, ModifiersState};
            use ::smithay::input::SeatHandler;
            use ::smithay::utils::Serial;

            #[thin_delegate::register]
            pub trait KeyboardTarget<D>: IsAlive + PartialEq + Clone + fmt::Debug + Send
            where
                D: SeatHandler,
            {
                /// Keyboard focus of a given seat was assigned to this handler
                fn enter(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    keys: Vec<KeysymHandle<'_>>,
                    serial: Serial,
                );
                /// The keyboard focus of a given seat left this handler
                fn leave(&self, seat: &Seat<D>, data: &mut D, serial: Serial);
                /// A key was pressed on a keyboard from a given seat
                fn key(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    key: KeysymHandle<'_>,
                    state: KeyState,
                    serial: Serial,
                    time: u32,
                );
                /// Hold modifiers were changed on a keyboard from a given seat
                fn modifiers(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    modifiers: ModifiersState,
                    serial: Serial,
                );
            }
        }

        #[thin_delegate::external_trait_def(with_uses = true)]
        pub(crate) mod pointer {
            use ::smithay::input::pointer::{
                AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
                GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
                GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent, MotionEvent,
                RelativeMotionEvent,
            };
            use ::smithay::input::SeatHandler;
            use ::smithay::utils::{IsAlive, Serial};

            #[thin_delegate::register]
            pub trait PointerTarget<D>: IsAlive + PartialEq + Clone + fmt::Debug + Send
            where
                D: SeatHandler,
            {
                /// A pointer of a given seat entered this handler
                fn enter(&self, seat: &Seat<D>, data: &mut D, event: &MotionEvent);
                /// A pointer of a given seat moved over this handler
                fn motion(&self, seat: &Seat<D>, data: &mut D, event: &MotionEvent);
                /// A pointer of a given seat that provides relative motion moved over this handler
                fn relative_motion(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &RelativeMotionEvent,
                );
                /// A pointer of a given seat clicked a button
                fn button(&self, seat: &Seat<D>, data: &mut D, event: &ButtonEvent);
                /// A pointer of a given seat scrolled on an axis
                fn axis(&self, seat: &Seat<D>, data: &mut D, frame: AxisFrame);
                /// End of a pointer frame
                fn frame(&self, seat: &Seat<D>, data: &mut D);
                /// A pointer of a given seat started a swipe gesture
                fn gesture_swipe_begin(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GestureSwipeBeginEvent,
                );
                /// A pointer of a given seat updated a swipe gesture
                fn gesture_swipe_update(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GestureSwipeUpdateEvent,
                );
                /// A pointer of a given seat ended a swipe gesture
                fn gesture_swipe_end(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GestureSwipeEndEvent,
                );
                /// A pointer of a given seat started a pinch gesture
                fn gesture_pinch_begin(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GesturePinchBeginEvent,
                );
                /// A pointer of a given seat updated a pinch gesture
                fn gesture_pinch_update(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GesturePinchUpdateEvent,
                );
                /// A pointer of a given seat ended a pinch gesture
                fn gesture_pinch_end(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GesturePinchEndEvent,
                );
                /// A pointer of a given seat started a hold gesture
                fn gesture_hold_begin(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GestureHoldBeginEvent,
                );
                /// A pointer of a given seat ended a hold gesture
                fn gesture_hold_end(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &GestureHoldEndEvent,
                );
                /// A pointer of a given seat left this handler
                fn leave(&self, seat: &Seat<D>, data: &mut D, serial: Serial, time: u32);
                /// A pointer of a given seat moved from another handler to this handler
                fn replace(
                    &self,
                    replaced: <D as SeatHandler>::PointerFocus,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &MotionEvent,
                ) {
                    PointerTarget::<D>::leave(&replaced, seat, data, event.serial, event.time);
                    PointerTarget::<D>::enter(self, seat, data, event);
                }
            }
        }

        #[thin_delegate::external_trait_def(with_uses = true)]
        pub(crate) mod touch {
            use ::smithay::input::touch::{
                DownEvent, MotionEvent, OrientationEvent, ShapeEvent, UpEvent,
            };
            use ::smithay::input::SeatHandler;
            use ::smithay::utils::Serial;

            #[thin_delegate::register]
            pub trait TouchTarget<D>: IsAlive + PartialEq + Clone + fmt::Debug + Send
            where
                D: SeatHandler,
            {
                /// A new touch point has appeared on the target.
                ///
                /// This touch point is assigned a unique ID. Future events from this touch point reference this ID.
                /// The ID ceases to be valid after a touch up event and may be reused in the future.
                fn down(&self, seat: &Seat<D>, data: &mut D, event: &DownEvent, seq: Serial);

                /// The touch point has disappeared.
                ///
                /// No further events will be sent for this touch point and the touch point's ID
                /// is released and may be reused in a future touch down event.
                fn up(&self, seat: &Seat<D>, data: &mut D, event: &UpEvent, seq: Serial);

                /// A touch point has changed coordinates.
                fn motion(&self, seat: &Seat<D>, data: &mut D, event: &MotionEvent, seq: Serial);

                /// Indicates the end of a set of events that logically belong together.
                fn frame(&self, seat: &Seat<D>, data: &mut D, seq: Serial);

                /// Touch session cancelled.
                ///
                /// Touch cancellation applies to all touch points currently active on this target.
                /// The client is responsible for finalizing the touch points, future touch points on
                /// this target may reuse the touch point ID.
                fn cancel(&self, seat: &Seat<D>, data: &mut D, seq: Serial);

                /// Sent when a touch point has changed its shape.
                ///
                /// A touch point shape is approximated by an ellipse through the major and minor axis length.
                /// The major axis length describes the longer diameter of the ellipse, while the minor axis
                /// length describes the shorter diameter. Major and minor are orthogonal and both are specified
                /// in surface-local coordinates. The center of the ellipse is always at the touch point location
                /// as reported by [`TouchTarget::down`] or [`TouchTarget::motion`].
                fn shape(&self, seat: &Seat<D>, data: &mut D, event: &ShapeEvent, seq: Serial);

                /// Sent when a touch point has changed its orientation.
                ///
                /// The orientation describes the clockwise angle of a touch point's major axis to the positive surface
                /// y-axis and is normalized to the -180 to +180 degree range. The granularity of orientation depends
                /// on the touch device, some devices only support binary rotation values between 0 and 90 degrees.
                fn orientation(
                    &self,
                    seat: &Seat<D>,
                    data: &mut D,
                    event: &OrientationEvent,
                    seq: Serial,
                );
            }
        }
    }

    #[thin_delegate::external_trait_def]
    pub(crate) mod utils {
        #[thin_delegate::register]
        pub trait IsAlive {
            /// Check if object is alive
            fn alive(&self) -> bool;
        }
    }

    pub(crate) mod wayland {
        #[thin_delegate::external_trait_def(with_uses = true)]
        pub(crate) mod buffer {
            use smithay::reexports::wayland_server::protocol::wl_buffer;

            #[thin_delegate::register]
            pub trait BufferHandler {
                /// Called when the client has destroyed the buffer.
                ///
                /// At this point the buffer is no longer usable by Smithay.
                fn buffer_destroyed(&mut self, buffer: &wl_buffer::WlBuffer);
            }
        }

        #[thin_delegate::external_trait_def(with_uses = true)]
        pub(crate) mod seet {
            use smithay::reexports::wayland_server::backend::ObjectId;
            use smithay::reexports::wayland_server::protocol::wl_surface;

            #[thin_delegate::register]
            pub trait WaylandFocus {
                /// Returns the underlying wl_surface, if any.
                ///
                /// *Note*: This has to return `Some`, if `same_client_as` can return true
                /// for any provided `ObjectId`
                fn wl_surface(&self) -> Option<Cow<'_, wl_surface::WlSurface>>;
                /// Returns true, if the underlying wayland object originates from
                /// the same client connection as the provided `ObjectId`.
                ///
                /// *Must* return false, if there is not underlying wayland object.
                fn same_client_as(&self, object_id: &ObjectId) -> bool {
                    self.wl_surface()
                        .map(|s| s.id().same_client_as(object_id))
                        .unwrap_or(false)
                }
            }
        }
    }
}
