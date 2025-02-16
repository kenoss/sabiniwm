check:
  cargo build && cargo clippy && cargo fmt -- --check

check-strict:
  export CARGO_TARGET_DIR=target/check-strict RUSTFLAGS='-D warnings'; just check

check-warn:
  export CARGO_TARGET_DIR=target/check-strict RUSTFLAGS='-D warnings'; clear; cargo build --color always |& head -n 32

run *ARGS:
  cargo run {{ARGS}}

test *ARGS:
  cargo test {{ARGS}}

export TEMPLATE_SESSION := '''
[Desktop Entry]
Name=sabiniwm
Comment=A tiling wayland compositor, influenced xmonad
Exec=EXEC
Type=Application
'''

export TEMPLATE_LAUNCH := '''
#!/usr/bin/env bash

RUST_LOG=info RUST_BACKTRACE=1 SABINIWM_XKB_CONFIG='{"layout": "custom", "repeat_delay": 200, "repeat_rate": 60}' BIN_PATH
'''

install-session-dev:
  cargo build --release
  mkdir -p target/session
  echo "$TEMPLATE_SESSION" | sed "s|EXEC|$(pwd)/target/session/launch|" > target/session/sabiniwm.desktop
  echo "$TEMPLATE_LAUNCH" | sed "s|BIN_PATH|$(pwd)/target/release/sabiniwm-pistachio|" > target/session/launch
  chmod +x $(pwd)/target/session/launch
  sudo install -m 644 target/session/sabiniwm.desktop /usr/share/wayland-sessions/

install-session-dev-debug:
  cargo build
  mkdir -p target/session
  echo "$TEMPLATE_SESSION" | sed "s|EXEC|$(pwd)/target/session/launch|" > target/session/sabiniwm.desktop
  echo "$TEMPLATE_LAUNCH" | sed "s|BIN_PATH|$(pwd)/target/debug/sabiniwm-pistachio|" > target/session/launch
  chmod +x $(pwd)/target/session/launch
  sudo install -m 644 target/session/sabiniwm.desktop /usr/share/wayland-sessions/
