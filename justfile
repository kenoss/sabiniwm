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
Name=NAME
Comment=A tiling wayland compositor, influenced xmonad
Exec=EXEC
Type=Application
'''

export TEMPLATE_LAUNCH := '''
#!/usr/bin/env bash

RUST_LOG=info RUST_BACKTRACE=1 SABINIWM_XKB_CONFIG='{"layout": "custom", "repeat_delay": 200, "repeat_rate": 60}' BIN_PATH
'''

install-sessions-head:
  just install-session head-debug 'false'
  just install-session head-release 'true'

install-sessions-stable:
  just install-session stable-release 'true'

install-session LABEL IS_RELEASE:
  just install-session-aux 'sabiniwm-{{LABEL}}' 'target/session/{{LABEL}}' {{IS_RELEASE}}

install-session-aux NAME DIR IS_RELEASE:
  # Remove first to prevent 'Text file busy': https://stackoverflow.com/questions/16764946/what-generates-the-text-file-busy-message-in-unix
  rm -rf '{{DIR}}'
  mkdir -p '{{DIR}}'
  if [ '{{IS_RELEASE}}' = 'true' ]; then \
      cargo build --release; \
      cp -a target/release/sabiniwm-pistachio '{{DIR}}/'; \
  else \
      cargo build; \
      cp -a target/debug/sabiniwm-pistachio '{{DIR}}/'; \
  fi
  echo "$TEMPLATE_SESSION" | sed "s/NAME/{{NAME}}/; s|EXEC|$(pwd)/{{DIR}}/launch|" > '{{DIR}}/{{NAME}}.desktop'
  echo "$TEMPLATE_LAUNCH" | sed "s|BIN_PATH|$(pwd)/{{DIR}}/sabiniwm-pistachio|" > '{{DIR}}/launch'
  chmod +x '{{DIR}}/launch'
  sudo install -m 644 '{{DIR}}/{{NAME}}.desktop' /usr/share/wayland-sessions/
