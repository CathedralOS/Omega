//! Read the executing host's OS clock without introducing a modeled epoch.
//! The receiving runtime independently admits the target, selector and authority.

pub fn read_host_clock_nanoseconds(selector: u32) -> std::io::Result<u64> {
    #[cfg(unix)]
    {
        let mut reading = std::mem::MaybeUninit::<libc::timespec>::uninit();
        // clock_gettime initializes the complete output on success. The
        // selector is an OS word, and the output has its native ABI alignment.
        if unsafe { libc::clock_gettime(selector as libc::clockid_t, reading.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let reading = unsafe { reading.assume_init() };
        let seconds = u64::try_from(reading.tv_sec)
            .map_err(|_| std::io::Error::other("host clock precedes its nonnegative epoch"))?;
        let nanoseconds = u64::try_from(reading.tv_nsec)
            .map_err(|_| std::io::Error::other("host clock returned negative nanoseconds"))?;
        if nanoseconds >= 1_000_000_000 {
            return Err(std::io::Error::other(
                "host clock returned an invalid timespec",
            ));
        }
        seconds
            .checked_mul(1_000_000_000)
            .and_then(|value| value.checked_add(nanoseconds))
            .ok_or_else(|| std::io::Error::other("host clock exceeds u64 nanoseconds"))
    }
    #[cfg(not(unix))]
    {
        let _ = selector;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "this host has no admitted POSIX clock observation",
        ))
    }
}
