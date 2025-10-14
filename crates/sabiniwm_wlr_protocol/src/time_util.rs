use std::time::Duration;

/// Decomposes `Duration` to `(tv_sec_hi, tv_sec_lo, tv_nsec)`.
pub(crate) fn duration_into_parts(time: Duration) -> (u32, u32, u32) {
    let tv_sec_hi = (time.as_secs() >> 32) as u32;
    let tv_sec_lo = (time.as_secs() & 0xFFFFFFFF) as u32;
    let tv_nsec = time.subsec_nanos();
    (tv_sec_hi, tv_sec_lo, tv_nsec)
}
