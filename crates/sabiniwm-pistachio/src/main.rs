#[allow(unused_imports)]
#[macro_use]
extern crate maplit;

use big_s::S;
use itertools::Itertools;
use sabiniwm::action::{self, Action, ActionFnI};
use sabiniwm::config::{ConfigDelegateUnstableI, XkbConfig};
use sabiniwm::input::{KeySeqSerde, Keymap, ModMask};
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
        let xkb_config = smithay::input::keyboard::XkbConfig {
            layout: "custom",
            ..Default::default()
        };
        XkbConfig {
            xkb_config,
            repeat_delay: 200,
            repeat_rate: 60,
        }
    }
}

fn main() -> eyre::Result<()> {
    tracing_init()?;
    color_eyre::install()?;

    let workspace_tags = (0..=9)
        .map(|i| WorkspaceTag(format!("{}", i)))
        .collect_vec();

    let meta_keys = if should_use_udev() {
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
        kbd("H-x H-q") => action::ActionQuitSabiniwm.into_action(),
        kbd("H-x H-2") => action::ActionChangeVt(2).into_action(),

        kbd("H-x H-t") => Action::spawn("alacritty"),
        kbd("H-x H-e") => Action::spawn("emacs"),
        kbd("H-x H-b") => Action::spawn("firefox"),

        kbd("H-space") => LayoutMessageSelect::Next.into(),
        // Toggle Full
        kbd("H-f") => LayoutMessageToggle.into(),

        kbd("H-d") => action::ActionWorkspaceFocusNonEmpty::Prev.into_action(),
        kbd("H-h") => action::ActionMoveFocus::Prev.into_action(),
        kbd("H-t") => action::ActionMoveFocus::Next.into_action(),
        kbd("H-n") => action::ActionWorkspaceFocusNonEmpty::Next.into_action(),
        kbd("H-D") => action::ActionWindowMoveToWorkspace::Prev.into_action(),
        kbd("H-H") => action::ActionWindowSwap::Prev.into_action(),
        kbd("H-T") => action::ActionWindowSwap::Next.into_action(),
        kbd("H-N") => action::ActionWindowMoveToWorkspace::Next.into_action(),
        kbd("H-o") => action::ActionWorkspaceFocusNonEmpty::Prev.into_action(),
        kbd("H-e") => action::ActionMoveFocus::Prev.into_action(),
        kbd("H-u") => action::ActionMoveFocus::Next.into_action(),
        kbd("H-i") => action::ActionWorkspaceFocusNonEmpty::Next.into_action(),
        kbd("H-O") => action::ActionWindowMoveToWorkspace::Prev.into_action(),
        kbd("H-E") => action::ActionWindowSwap::Prev.into_action(),
        kbd("H-U") => action::ActionWindowSwap::Next.into_action(),
        kbd("H-I") => action::ActionWindowMoveToWorkspace::Next.into_action(),

        kbd("H-v") => action::ActionWorkspaceFocus::Next.into_action(),
        kbd("H-b") => action::ActionWorkspaceFocus::Prev.into_action(),

        kbd("H-k") => (action::ActionWindowKill {}).into_action(),
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
    let keymap = Keymap::new(keymap);

    let config_delegate = Box::new(Config);

    SabiniwmState::run(config_delegate, workspace_tags, keymap)?;

    Ok(())
}
