use big_s::S;
use std::fmt;
use std::sync::OnceLock;
use std::thread::ThreadId;

/// Set panic hook, which log a backtrace with `tracing` macro.
pub(crate) fn set_hook() {
    static ROOT_CAUSE_THREAD_ID: OnceLock<ThreadId> = OnceLock::new();

    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let mut initialized = false;
        let root_cause_thread_id = ROOT_CAUSE_THREAD_ID.get_or_init(|| {
            initialized = true;
            std::thread::current().id()
        });
        if *root_cause_thread_id == std::thread::current().id() {
            let maybe_unwinding = if initialized {
                ""
            } else {
                " (maybe panic in unwinding)"
            };
            error!(
                r#"panic hook: thread "{}" panicked{}
backtrace:
{:?}
"#,
                std::thread::current().name().unwrap_or("anonymous"),
                maybe_unwinding,
                BacktraceAltFormatter(backtrace::Backtrace::new()),
            );
        }
        original_hook(panic_info);
    }));
}

// Helper to show the backtrace of actual cause of panic hook.
struct BacktraceAltFormatter(backtrace::Backtrace);

// Mimicked implementation of backtrace::Backtrace.
impl fmt::Debug for BacktraceAltFormatter {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        use backtrace::{BacktraceFmt, BytesOrWideString, PrintFmt};

        let full = fmt.alternate();
        let (frames, style) = if full {
            (self.0.frames(), PrintFmt::Full)
        } else {
            let mut i = 0;
            for (j, frame) in self.0.frames().iter().enumerate() {
                for symbol in frame.symbols() {
                    let name = symbol.name().map(|x| format!("{:?}", x)).unwrap_or(S(""));
                    if name.starts_with("core::panicking::panic::") {
                        i = j + 1;
                        break;
                    }
                }
            }

            (&self.0.frames()[i..], PrintFmt::Short)
        };

        // When printing paths we try to strip the cwd if it exists, otherwise
        // we just print the path as-is. Note that we also only do this for the
        // short format, because if it's full we presumably want to print
        // everything.
        let cwd = std::env::current_dir();
        let mut print_path = move |fmt: &mut fmt::Formatter<'_>, path: BytesOrWideString<'_>| {
            let path = path.into_path_buf();
            if !full
                && let Ok(cwd) = &cwd
                && let Ok(suffix) = path.strip_prefix(cwd)
            {
                return fmt::Display::fmt(&suffix.display(), fmt);
            }
            fmt::Display::fmt(&path.display(), fmt)
        };

        let mut f = BacktraceFmt::new(fmt, style, &mut print_path);
        f.add_context()?;
        for frame in frames {
            f.frame().backtrace_frame(frame)?;
        }
        f.finish()?;
        Ok(())
    }
}
