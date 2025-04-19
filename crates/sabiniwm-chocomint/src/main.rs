// This is a QWERTY version of sabiniwm-pistachio.
// This is loosely updated. Last update is 2025-04-20.

#[allow(unused_imports)]
#[macro_use]
extern crate tracing;

#[allow(unused_imports)]
#[macro_use]
extern crate maplit;

use big_s::S;
use sabiniwm::action::{self, Action, ActionFnI};
use sabiniwm::config::{ConfigDelegateUnstableI, XkbConfig};
use sabiniwm::input::{KeySeqSerde, Keymap, ModMask};
#[allow(unused)]
use sabiniwm::reexports::smithay;
use sabiniwm::view::predefined::{LayoutMessageSelect, LayoutMessageToggle};
use sabiniwm::view::stackset::WorkspaceTag;
use sabiniwm::SabiniwmState;

fn should_use_udev() -> bool {
    matches!(
        std::env::var("DISPLAY"),
        Err(std::env::VarError::NotPresent)
    ) && matches!(
        std::env::var("WAYLAND_DISPLAY"),
        Err(std::env::VarError::NotPresent)
    )
}

fn tracing_init() -> eyre::Result<()> {
    use time::macros::format_description;
    use time::UtcOffset;
    use tracing_subscriber::fmt::time::OffsetTime;
    use tracing_subscriber::EnvFilter;

    match std::env::var("RUST_LOG") {
        Err(std::env::VarError::NotPresent) => {}
        _ => {
            let offset = UtcOffset::current_local_offset().unwrap();
            let timer = OffsetTime::new(
                offset,
                format_description!("[hour]:[minute]:[second].[subsecond digits:3]"),
            );

            let fmt = tracing_subscriber::fmt()
                .with_env_filter(EnvFilter::from_default_env())
                .with_timer(timer)
                .with_line_number(true)
                .with_ansi(true);

            if should_use_udev() {
                let log_file =
                    std::io::LineWriter::new(std::fs::File::create("/tmp/sabiniwm.log")?);

                fmt.with_writer(std::sync::Mutex::new(log_file)).init();
            } else {
                fmt.init();
            }
        }
    }

    Ok(())
}

struct Config;

impl ConfigDelegateUnstableI for Config {
    fn get_xkb_config(&self) -> XkbConfig<'_> {
        let xkb_config = Default::default();
        XkbConfig {
            xkb_config,
            repeat_delay: 200,
            repeat_rate: 60,
        }
    }

    fn make_workspace_tags(&self) -> Vec<WorkspaceTag> {
        (0..=9).map(|i| WorkspaceTag(format!("{}", i))).collect()
    }

    fn get_modmask(&self, is_udev_backend: bool) -> sabiniwm::input::ModMask {
        use sabiniwm::input::ModMask;

        if is_udev_backend {
            ModMask::MOD5
        } else {
            ModMask::MOD4
        }
    }

    fn make_keymap(&self, is_udev_backend: bool) -> Keymap<Action> {
        let workspace_tags = self.make_workspace_tags();

        let meta_keys = if is_udev_backend {
            hashmap! {
                S("C") => ModMask::CONTROL,
                S("M") => ModMask::MOD1,
                S("s") => ModMask::MOD4,
                S("H") => ModMask::MOD5,
            }
        } else {
            hashmap! {
                S("C") => ModMask::CONTROL,
                S("M") => ModMask::MOD1,
                // Hyper uses Mod5 in my environment. Use Mod4 for development with winit.
                S("H") => ModMask::MOD4,
            }
        };
        let keyseq_serde = KeySeqSerde::new(meta_keys);
        let kbd = |s| keyseq_serde.kbd(s).unwrap();
        let mut keymap = hashmap! {
            kbd("H-b H-q") => action::ActionQuitSabiniwm.into_action(),
            kbd("H-b H-2") => action::ActionChangeVt(2).into_action(),

            kbd("H-b H-t") => Action::spawn("alacritty"),
            kbd("H-b H-e") => Action::spawn("emacs"),
            kbd("H-b H-b") => Action::spawn("firefox"),

            kbd("H-space") => LayoutMessageSelect::Next.into(),
            // Toggle Full
            kbd("H-b H-f") => LayoutMessageToggle.into(),

            kbd("H-h") => action::ActionWorkspaceFocusNonEmpty::Prev.into_action(),
            kbd("H-k") => action::ActionMoveFocus::Prev.into_action(),
            kbd("H-j") => action::ActionMoveFocus::Next.into_action(),
            kbd("H-l") => action::ActionWorkspaceFocusNonEmpty::Next.into_action(),
            kbd("H-H") => action::ActionWindowMoveToWorkspace::Prev.into_action(),
            kbd("H-K") => action::ActionWindowSwap::Prev.into_action(),
            kbd("H-J") => action::ActionWindowSwap::Next.into_action(),
            kbd("H-L") => action::ActionWindowMoveToWorkspace::Next.into_action(),
            kbd("H-s") => action::ActionWorkspaceFocusNonEmpty::Prev.into_action(),
            kbd("H-d") => action::ActionMoveFocus::Prev.into_action(),
            kbd("H-f") => action::ActionMoveFocus::Next.into_action(),
            kbd("H-g") => action::ActionWorkspaceFocusNonEmpty::Next.into_action(),
            kbd("H-S") => action::ActionWindowMoveToWorkspace::Prev.into_action(),
            kbd("H-D") => action::ActionWindowSwap::Prev.into_action(),
            kbd("H-F") => action::ActionWindowSwap::Next.into_action(),
            kbd("H-G") => action::ActionWindowMoveToWorkspace::Next.into_action(),

            kbd("H-greater") => action::ActionWorkspaceFocus::Next.into_action(),
            kbd("H-n") => action::ActionWorkspaceFocus::Prev.into_action(),

            kbd("H-b H-k") => (action::ActionWindowKill {}).into_action(),

            kbd("H-o") => (action::ActionWindowFloat {}).into_action(),
            kbd("H-p") => (action::ActionWindowSink {}).into_action(),
        };
        keymap.extend(workspace_tags.iter().cloned().enumerate().map(|(i, tag)| {
            (
                // TODO: Fix lifetime issue and use `kbd`.
                keyseq_serde.kbd(&format!("H-{i}")).unwrap(),
                action::ActionWorkspaceFocus::WithTag(tag).into_action(),
            )
        }));
        const SHIFTED: &[char] = &[')', '!', '@', '#', '$', '%', '^', '&', '*', '('];
        fn keysym_str(c: char) -> &'static str {
            match c {
                '!' => "exclam",
                '@' => "at",
                '#' => "numbersign",
                '$' => "dollar",
                '%' => "percent",
                '^' => "asciicircum",
                '&' => "ampersand",
                '*' => "asterisk",
                '(' => "parenleft",
                ')' => "parenright",
                _ => unreachable!(),
            }
        }
        keymap.extend(workspace_tags.iter().cloned().enumerate().map(|(i, tag)| {
            (
                // TODO: Fix lifetime issue and use `kbd`.
                keyseq_serde
                    .kbd(&format!("H-{}", keysym_str(SHIFTED[i])))
                    .unwrap(),
                action::ActionWithSavedFocus(
                    action::ActionWindowMoveToWorkspace::WithTag(tag).into_action(),
                )
                .into_action(),
            )
        }));

        Keymap::new(keymap)
    }

    fn get_border_for_float_window(&self) -> sabiniwm::view::window::Border {
        use sabiniwm::view::window::{Border, Rgba};

        Border {
            dim: 2.into(),
            active_rgba: Rgba::from_rgba(0x556b2fff),
            inactive_rgba: Rgba::from_rgba(0x202020ff),
        }
    }

    fn on_lid_closed(&self) {
        info!("Config::on_lid_closed()");

        fn get_xdg_config_home() -> Option<String> {
            use std::collections::HashMap;

            let envvars = std::env::vars().collect::<HashMap<_, _>>();
            let path = match envvars.get("XDG_CONFIG_HOME") {
                Some(path) => path.clone(),
                None => match envvars.get("HOME") {
                    Some(path) => "$HOME/.config".replace("$HOME", path),
                    None => return None,
                },
            };
            Some(path)
        }

        fn spawn_script() -> Option<()> {
            const SCRIPT_PATH: &str = "$XDG_CONFIG_HOME/sabiniwm/on_lid_closed";

            let xdg_config_home = get_xdg_config_home()?;
            let path = SCRIPT_PATH.replace("$XDG_CONFIG_HOME", &xdg_config_home);

            info!("Config::on_lid_closed(): exec {path}");
            std::process::Command::new(path).spawn().ok()?;

            Some(())
        }

        match spawn_script() {
            Some(_) => {}
            // For example, script was not found or not executable.
            // Execute swaylock by default.
            None => {
                info!("Config::on_lid_closed(): exec default hook");
                let _ = std::process::Command::new("swaylock")
                    .args([
                        "--color",
                        "101010",
                        "--show-keyboard-layout",
                        "--disable-caps-lock-text",
                    ])
                    .spawn();
                let _ = std::process::Command::new("systemctl")
                    .args(["suspend"])
                    .spawn();
            }
        }
    }
}

fn main() -> eyre::Result<()> {
    tracing_init()?;
    color_eyre::install()?;

    let config_delegate = Box::new(Config);
    SabiniwmState::run(config_delegate)?;

    Ok(())
}
