# sabiniwm

A tiling Wayland compositor, influenced by xmonad

## Goal and non goal

### Goal

- Code over configuration: extensible/configurable like xmonad
- Minimal and clear runtime dependency (No dependency to, e.g. Haskell.)
- Simple, clean and mentainable code

### Non goal

- Excessive use of animation
  - Not a priority. Small use of animation might be supported in the future.
- Configuration with file
  - A user can implement it if they need it. Please publish a crate if you implemented it.
- Battery included (default configuration)
  - The author believes that there is no good default values for configuration. Users must configure by theirselves.

## Status

Alpha, not stable.

In the short-term, you shouldn't expect API stability.

The author kenoss@ is using it on Asahi Linux, M2 Macbook Air (main machine).

## Roadmap

### Milestone 1

kenoss@ is 80%-ish satisfied daily use of it on private machine, Macbook Air.

- [x] Fundamental udev support (Touchpad, Scaling for HiDPI display)
- [x] Fundamental features of tiling
- [x] Layouts (Tall, Full, Select, Toggle, margin, border)
- [ ] Floating windows
- [ ] Screenshot/screencast
- [ ] etc.

### Milestone 2

kenoss@ is 80%-ish satisfied daily use of it on coop machine.

- [ ] External (wireless) mouse (I have to look into [[random disconnection issue](https://www.reddit.com/r/archlinux/comments/apnesg/usb_mouse_randomly_disconnecting/)].)
- [ ] Authentication dialog with security keys (Floating windows + manage hooks?)
- [ ] xrandr (Multiple outputs, external displays)
- [ ] Notification
- [ ] Screencast with Chrome
- [ ] IME
- [ ] etc.

## Comparison

### [xmonad](https://xmonad.org/)

xmonad is useful, matured, and the source of ideas of sabiniwm. But it lacks Wayland support, and never supports
[[issue 1](https://github.com/xmonad/xmonad/issues/38)][[issue 2](https://github.com/xmonad/xmonad/issues/193)].

### [niri](https://github.com/YaLTeR/niri)

niri is beautiful and feature-rich tiliing Wayland compositor. But it's not xmonad-like.

I recommend you to try it if you are not seeking xmonad alternatives.
sabiniwm aims at opposite direction.

### [waymonad](https://github.com/waymonad/waymonad)

The project looks not active [[issue](https://github.com/waymonad/waymonad/issues/44#issuecomment-1665417483)].

### Other tiling Wayland compositors

Lots of them are written in C/C++. Not easy to read and write.

## Getting started

No document is available.

You can start with running and modifying `sabiniwm-chocomint`/`sabiniwm-pistachio`.

```shell
$ cargo run -- --bin sabiniwm-chocomint
$ cargo run -- --bin sabiniwm-pistachio # If you are a dvorak user.
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
$ just install-session-dev
```

See also [tatarajo](https://github.com/kenoss/tatarajo) educational course.

## How to run with udev backend

### Dependencies

See [.github/workflows/ci.yaml](.github/workflows/ci.yaml).

### Run

You can run it with udev backend in the following ways:

- From TTY (i.e., turning off display manager): Just `cargo run` works.
- From display manager: Use `just install-session-dev` and select `sabiniwm`.

## License

This repository is distributed under the terms of both the MIT license and the
Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT), and
[COPYRIGHT](COPYRIGHT) for details.
