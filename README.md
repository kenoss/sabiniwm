# sabiniwm

A tiling Wayland compositor, influenced by xmonad

## Goals and non-goals

### Goal

- Code over configuration: extensible/configurable, like xmonad
- Minimal and clear runtime dependencies (No dependency on, e.g., Haskell.)
- Simple, clean, and maintainable code

### Non-goals

- Excessive use of animations
  - Not a priority. Small use of animations might be supported in the future.
- File-based configuration
  - Users can implement it if they need it (like [spacemacs](https://github.com/syl20bnr/spacemacs)
    for Emacs). Please publish a crate if you implement it.
- Batteries included (default configuration)
  - The author believes that there are no good default values for configuration. Users must
    configure it themselves.

## Status

Alpha; not stable.

In the short-term, you shouldn't expect API stability.

The author, kenoss@, is using it on Asahi Linux, M2 MacBook Air (main machine).

## Roadmap

### Milestone 1

kenoss@ is 80%-ish satisfied with daily use of it on a private machine, a MacBook Air.

- [x] Fundamental udev support (Touchpad, scaling for HiDPI displays)
- [x] Fundamental tiling features
- [x] Layouts (Tall, Full, Select, Toggle, margins, borders)
- [x] Floating windows
- [x] Manage hook
- [x] Session lock (`ext-session-lock-v1`)
- [x] Screenshot/screencast

### Milestone 2

kenoss@ is 80%-ish satisfied with daily use of it on a corporate machine.

- [x] External (wireless) mouse
  - [ ] (I need to look into the
        [[random disconnection issue](https://www.reddit.com/r/archlinux/comments/apnesg/usb_mouse_randomly_disconnecting/)]).
- [x] Authentication dialog with security keys (Floating windows + manage hooks)
- [ ] xrandr (Multiple outputs, external displays)
- [ ] Notification
- [x] Screencast with Chrome
- [ ] IME

## Comparison

### [xmonad](https://xmonad.org/)

xmonad is useful, mature, and the source of ideas for sabiniwm. But it lacks Wayland support, and
has never supported it, as discussed in
[[issue 1](https://github.com/xmonad/xmonad/issues/38)]
[[issue 2](https://github.com/xmonad/xmonad/issues/193)].

### [niri](https://github.com/YaLTeR/niri)

niri is a beautiful and feature-rich tiling Wayland compositor. But it's not xmonad-like.

I recommend you try it if you are not seeking xmonad alternatives.
sabiniwm aims in the opposite direction.

### [waymonad](https://github.com/waymonad/waymonad)

The project does not appear active
[[issue](https://github.com/waymonad/waymonad/issues/44#issuecomment-1665417483)].

### Other tiling Wayland compositors

Many of them are written in C/C++. They are not easy to read and write.

## Dependencies

See [.github/workflows/ci.yaml](.github/workflows/ci.yaml).

## Getting started

No documentation is available.

You can start by running and modifying `sabiniwm_chocomint`/`sabiniwm_pistachio`.

```shell
$ cargo run -- --bin sabiniwm_chocomint
$ cargo run -- --bin sabiniwm_pistachio # If you are a dvorak user.
```

## How to develop

See [justfile](./justfile). For example,

```shell
$ # build
$ cargo build
$ # run
$ cargo run
$ # check
$ just check-strict
$ # watch
$ cargo watch -c -s 'cargo build && just check-strict'
$ # install to /usr/share/wayland-sessions/
$ just install-sessions-head
```

See also the [tatarajo](https://github.com/kenoss/tatarajo) educational course.

## How to run with udev backend

### Run

You can run it with udev backend in the following ways:

- From TTY (i.e., with the display manager turned off): Just `cargo run` works.
- From display manager: Use `just install-sessions-stable` and select `sabiniwm`.

## Configuration

sabiniwm is like xmonad.

- xmonad is a framework/library to program your own WM. You don't write a configuration, you write a
  Haskell program.
- sabiniwm is a library to program your own Wayland compositor. You don't write a configuration, you
  write a Rust program.

See `sabiniwm_chocomint`/`sabiniwm_pistachio` crates. For example, `sabiniwm_pistachio` (bin crate)
uses `sabiniwm` (lib crate). It configures the behavior by implementing/providing `Config`, a set of
callbacks:

```rust
struct Config { ... }

impl ConfigDelegateUnstableI for Config { ... }

fn main() -> eyre::Result<()> {
    let config_delegate = Box::new(Config::new(perfetto_toggle_handle));
    SabiniwmState::run(config_delegate)?;

    Ok(())
}
```

Another mechanism for adding functionality is `Action`. An `Action` is an executable command that
can be triggered to perform a specific task. For example, sabiniwm has predefined actions for window
management in `sabiniwm::action::*`, and external crates can define their own, such as
`sabiniwm_tracing_helper::debug::ActionTraceToggle`. This provides a flexible way to define and
expose new behaviors.

If you feel there are not enough extension points, please file an issue.

## Set up environment

### screenshot/screencast

See [note/setup-xdg-desktop-portal.md](note/setup-xdg-desktop-portal.md).

## TODO

- smithay `d4780c5`: Consider adding `#[inline]` for derived operations.
- smithay `f5aeb51`: Consider adding `#[inline]` for common operations.

## License

This repository is distributed under the terms of both the MIT license and the
Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.
