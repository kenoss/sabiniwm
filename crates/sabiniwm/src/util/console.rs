//! Getting the user back to their console when startup goes wrong.
//!
//! With the udev backend, `LibSeatSession` puts the VT into `KD_GRAPHICS` mode before anything
//! else happens. From that point on a failure is invisible: the screen is black, nothing that is
//! written to the console can be read, and VT switching is disabled. If we then hang, the machine
//! is stuck until it is power cycled.

use std::os::fd::AsRawFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Puts the console back into text mode.
///
/// The session manager restores the VT when the session's controller goes away, but that only
/// helps if we actually manage to exit. Do it ourselves so that bailing out always hands the
/// console back.
pub(crate) fn restore_text_mode() {
    // `<linux/kd.h>` and `<linux/vt.h>`.
    const KDSETMODE: libc::c_ulong = 0x4b3a;
    const KD_TEXT: libc::c_ulong = 0x00;
    const VT_SETMODE: libc::c_ulong = 0x5602;
    const VT_AUTO: libc::c_char = 0x00;

    #[repr(C)]
    struct VtMode {
        mode: libc::c_char,
        waitv: libc::c_char,
        relsig: libc::c_short,
        acqsig: libc::c_short,
        frsig: libc::c_short,
    }

    let Ok(tty) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    else {
        return;
    };

    // Give up the `VT_PROCESS` switching handshake first, otherwise nobody can switch away from
    // this VT anymore.
    let mode = VtMode {
        mode: VT_AUTO,
        waitv: 0,
        relsig: 0,
        acqsig: 0,
        frsig: 0,
    };
    // SAFETY: The fd is open for the duration of the calls and `VtMode` matches `struct vt_mode`.
    // Both ioctls are best-effort; there is nothing sensible to do if they fail.
    unsafe {
        libc::ioctl(tty.as_raw_fd(), VT_SETMODE as _, &mode);
        libc::ioctl(tty.as_raw_fd(), KDSETMODE as _, KD_TEXT);
    }
}

/// Aborts the process if the initialization does not finish in time.
///
/// A hang during initialization is indistinguishable from a crash for the user, except that it
/// cannot be recovered from. Give up and restore the console instead.
pub(crate) struct InitWatchdog {
    finished: Arc<AtomicBool>,
}

impl InitWatchdog {
    pub fn start(timeout: Duration) -> Self {
        let finished = Arc::new(AtomicBool::new(false));

        if !timeout.is_zero() {
            let finished = finished.clone();
            let result = std::thread::Builder::new()
                .name("init-watchdog".to_owned())
                .spawn(move || {
                    let deadline = Instant::now() + timeout;
                    loop {
                        if finished.load(Ordering::SeqCst) {
                            return;
                        }
                        let Some(remaining) = deadline.checked_duration_since(Instant::now())
                        else {
                            break;
                        };
                        std::thread::sleep(remaining.min(Duration::from_millis(100)));
                    }

                    error!(
                        "initialization did not finish in {:?}; restoring the console and aborting",
                        timeout
                    );
                    restore_text_mode();
                    // Not `panic!()`: the thread we want to stop is another one, and it is
                    // presumably blocked.
                    std::process::exit(1);
                });
            if let Err(err) = result {
                warn!(?err, "Failed to spawn the init watchdog");
            }
        }

        Self { finished }
    }

    pub fn finish(self) {
        // `Drop` does the work.
    }
}

impl Drop for InitWatchdog {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::SeqCst);
    }
}
