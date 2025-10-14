## Protocol

For screencopy, two primary protocols are available:

- [`ext-image-copy-capture-v1`](https://wayland.app/protocols/ext-image-copy-capture-v1)
- [`wlr-screencopy-unstable-v1`](https://wayland.app/protocols/wlr-screencopy-unstable-v1)

We use `wlr-screencopy-unstable-v1` in the short term. Reasons:

1. The ecosystem is not yet mature.
   1-1. A compositor-agnostic XDG Portal backend for `ext-image-copy-capture` does not exist yet.
        In contrast, `wlr-screencopy` has `xdg-desktop-portal-wlr`. COSMIC has its own backend, but
        it appears to be specific to COSMIC.
   1-2. grim doesn't support `ext-image-copy-capture` yet.
2. `wlr-screencopy` is easier to implement.
3. smithay seems to have a plan [[smithay#781](https://github.com/Smithay/smithay/issues/781)] to
   implement `ext-image-copy-capture-v1`.

## About `TransformBehavior`

This section describes issues around transform in screencopy and the choices made for sabiniwm.

Note that `zwlr_screencopy_v1` doesn't designate what coordinates and transform we should use. Our
implementation provides three options: `TransformBehavior`.

### Limitation

Note that we currently use `smithay::desktop::space::space_render_elements()` to get
`RenderElements` for e.g. windows and borders. This lacks the capability to use an optional scale;
it internally uses `output.current_scale().fractional_scale()`. To use `NoTransform`, we need to use
a scale of 1.0. Consequently, sabiniwm currently cannot utilize `NoTransform`.

### Clients

Client behaviors also vary.

Wayland compositors expose scale and transform:

- scale: [wl_output::scale](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_output)
- transform: [wl_output::geometry](https://wayland.freedesktop.org/docs/html/apa.html#protocol-spec-wl_output)

So, clients may or may not apply scale and transform themselves.

Here are some examples.

#### [grim](https://github.com/emersion/grim)

grim expects `TransformBehavior::ScaleActualTerminal`.

- It retrieves the transform and attempts to invert it to produce a "natural" image, as perceived by
  a human.
  https://github.com/emersion/grim/blob/v1.4.0/render.c#L184-L186
- It scales the given image if it's different from the physical geometry.
  https://github.com/emersion/grim/blob/v1.4.0/render.c#L181-L183
- Note also that it only uses
  [`zwlr_screencopy_manager_v1::capture_output`](https://wayland.app/protocols/wlr-screencopy-unstable-v1#zwlr_screencopy_manager_v1:request:capture_output),
  not
  [`zwlr_screencopy_manager_v1::capture_output_region`](https://wayland.app/protocols/wlr-screencopy-unstable-v1#zwlr_screencopy_manager_v1:request:capture_output_region),
  even with the option `-g`. It performs clipping internally.

Therefore, using `TransformBehavior::NoTransform/Scale/ScaleActual` would yield incorrect results.

Note also that grim has a bug for `Transform::Flipped90/Flipped270`. See
#grim-transform-invert-wrong in note/issue-transform.md.

#### waymirror ([libwayshot](https://github.com/waycrate/wayshot))

waymirror expects `TransformBehavior::NoTransform`.

- It doesn't check scale or transform; it gets an image, and draws the dmabuf as-is.
  https://github.com/waycrate/wayshot/blob/7d38071115e0f5ad66c0457baa511c9c4f46fd1b/libwayshot/examples/waymirror.rs#L139-L147

Therefore, using `TransformBehavior::Scale/ScaleActual/ScaleActualTerminal` would yield incorrect
results.

#### [wf-recorder](https://github.com/ammen99/wf-recorder)

wf-recorder expects `TransformBehavior::NoTransform`.

#### [wl-screenrec](https://github.com/russelltg/wl-screenrec)

wl-screenrec looks to expect `TransformBehavior::ScaleActualTerminal`.

- <https://github.com/russelltg/wl-screenrec/blob/c75ad3b6b5e571875dae836b17fbed08983b035f/src/main.rs#L848>
- <https://github.com/russelltg/wl-screenrec/blob/c75ad3b6b5e571875dae836b17fbed08983b035f/src/transform.rs#L69>

(The author doesn't have an environment that supports VA-API. Not tested.)

### sabiniwm's choice

- Provides `TransformBehavior` for switching our policy in the future.
- Employs `TransformBehavior::ScaleActualTerminal` as grim expects it.
  - This choice is influenced by the fact that grim's author is also a wlroots developer,
    suggesting it might represent a de facto standard or common expectation.

### Additional resources

See also <https://gitlab.freedesktop.org/wlroots/wlr-protocols/-/issues/66>.
