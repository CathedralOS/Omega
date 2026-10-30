//! The real native C strtol operation over a bounded private host snapshot.
//! Returned pointer arithmetic stays entirely in that allocation. The caller
//! receives an offset, never a guest address or lifetime/access authority.

#[cfg(all(unix, target_pointer_width = "64"))]
unsafe extern "C" {
    fn strtol(
        input: *const std::ffi::c_char,
        end: *mut *mut std::ffi::c_char,
        base: std::ffi::c_int,
    ) -> std::ffi::c_long;
}

pub fn parse_host_long(bytes: &[u8], base: i32) -> Option<(i64, usize)> {
    if bytes.last() != Some(&0) || !(base == 0 || (2..=36).contains(&base)) {
        return None;
    }
    #[cfg(all(unix, target_pointer_width = "64"))]
    {
        let mut end = std::ptr::null_mut();
        // SAFETY: input is a finite immutable allocation ending with NUL, so
        // strtol never reads beyond it. The output is one host pointer word.
        // C long is 64 bits on the admitted Unix64 ABIs. No guest memory or
        // guest pointer participates in this operation.
        let value = unsafe { strtol(bytes.as_ptr().cast(), &mut end, base) };
        let offset = end.addr().checked_sub(bytes.as_ptr().addr())?;
        if offset >= bytes.len() {
            return None;
        }
        Some((value, offset))
    }
    #[cfg(not(all(unix, target_pointer_width = "64")))]
    {
        let _ = base;
        None
    }
}
