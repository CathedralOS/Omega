/*
We run the isolated provider worker loop (`run_worker_if_requested`, `Worker`):
load exact adapter images, attach shared original homes, and serve serialized
calls.

Code and mappings remain resident inside `ManuallyDrop<Worker>` until
`std::process::exit`, including across native `atexit` and TLS callbacks.
On Linux (`__errno_location`), macOS (`__error`) and Windows
(`GetLastError`/`SetLastError`), we save and restore guest thread error state
immediately around native adapter entry so worker IPC and loader housekeeping
never clobber guest observations.
*/

use crate::{
    channel,
    native_code::{LoadedFrame, NativeValue},
    provider_references::occurrence_index,
    shared_backing::SharedBacking,
    wire::{self, BorrowGeometry, Cursor, Geometry, Packet, invalid},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::OpenOptions,
    io::{self, Write},
    mem::ManuallyDrop,
    path::{Path, PathBuf},
    time::Duration,
};

pub const CHANNEL_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_CHANNEL";
pub const PARENT_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_PARENT";
pub const ROOT_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_ROOT";
pub const TIMEOUT_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_TIMEOUT_MS";

struct Adapter {
    entry: LoadedFrame,
    parameters: Vec<Geometry>,
    result: Option<Geometry>,
}

struct ProviderReference {
    adapter: u64,
    address: usize,
    extent: usize,
}

struct GuestErrorSlot {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    errno: *mut libc::c_int,
}

pub(crate) struct Worker {
    adapters: Vec<Adapter>, // Retained through process teardown so atexit/TLS callbacks never see unloaded code.
    pub(crate) homes: BTreeMap<u64, SharedBacking>,
    provider_references: Vec<Option<ProviderReference>>,
    directory: PathBuf,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    guest_errno: libc::c_int, // Restored into the live POSIX errno TLS slot around native entry.
    #[cfg(windows)]
    guest_last_error: u32, // Restored via `SetLastError` immediately around native entry.
}

/// The sponsor's handshake, read out of this process's environment.
pub(crate) struct Sponsorship {
    pub(crate) endpoint: OsString,
    #[cfg(windows)]
    pub(crate) parent: Option<OsString>,
    pub(crate) root: Option<OsString>,
    pub(crate) timeout: Option<OsString>,
}

/// Reads the sponsor's handshake and removes it from the environment.
///
/// Foreign code running in the worker may start processes, and they inherit
/// our environment. If one of them is `omega` (or any host that calls
/// `run_worker_if_requested`), a leftover `CHANNEL_ENV` would turn it into a
/// broken worker that dials a channel it was never given, then exits 1 instead
/// of running its command. So the handshake is consumed exactly once, here.
pub(crate) fn take_sponsorship() -> Option<Sponsorship> {
    let endpoint = std::env::var_os(CHANNEL_ENV)?;
    let sponsorship = Sponsorship {
        endpoint,
        #[cfg(windows)]
        parent: std::env::var_os(PARENT_ENV),
        root: std::env::var_os(ROOT_ENV),
        timeout: std::env::var_os(TIMEOUT_ENV),
    };
    for name in [CHANNEL_ENV, PARENT_ENV, ROOT_ENV, TIMEOUT_ENV] {
        // SAFETY: hosts call this as the first statement of `main`, before
        // they start any thread, so nothing can read the environment
        // concurrently. Under the libtest harness the only other thread is
        // the idle test runner, which does not touch the environment.
        unsafe { std::env::remove_var(name) };
    }
    Some(sponsorship)
}

pub fn run_worker_if_requested() {
    let Some(sponsorship) = take_sponsorship() else {
        return;
    };
    let result = (|| {
        configure_failure_reporting()?;
        let endpoint = sponsorship
            .endpoint
            .into_string()
            .map_err(|_| invalid("native worker endpoint is not text"))?;
        let timeout: u64 = sponsorship
            .timeout
            .and_then(|timeout| timeout.into_string().ok())
            .ok_or_else(|| invalid("native worker has no sponsor deadline"))?
            .parse()
            .map_err(|_| invalid("native worker sponsor deadline is malformed"))?;
        if timeout == 0 || timeout > 3_600_000 {
            return Err(invalid("native worker sponsor deadline is out of range"));
        }
        let timeout = Duration::from_millis(timeout);
        let directory = sponsorship
            .root
            .ok_or_else(|| invalid("native worker has no private root"))?;
        #[cfg(unix)]
        let mut stream = channel::receive(
            endpoint
                .parse()
                .map_err(|_| invalid("native worker descriptor is malformed"))?,
            timeout,
        )?;
        #[cfg(windows)]
        let mut stream = channel::receive(
            &endpoint,
            sponsorship
                .parent
                .and_then(|parent| parent.into_string().ok())
                .ok_or_else(|| invalid("native worker has no parent identity"))?
                .parse()
                .map_err(|_| invalid("native worker parent identity is malformed"))?,
            timeout,
        )?;
        let mut worker = ManuallyDrop::new(Worker {
            adapters: Vec::new(),
            homes: BTreeMap::new(),
            provider_references: Vec::new(),
            directory: directory.into(),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            guest_errno: 0,
            #[cfg(windows)]
            guest_last_error: 0,
        });
        loop {
            let packet = wire::receive(&mut stream)?;
            let mut cursor = Cursor::new(&packet);
            let tag = cursor.byte()?;
            if tag == 4 {
                cursor.finish()?;
                let mut response = Packet::new(0);
                response.bytes(&[])?;
                wire::send(&mut stream, &response.0)?;
                return Ok(());
            }
            let result = match tag {
                1 => worker.install(cursor),
                2 => worker.attach(cursor),
                3 => worker.call(cursor),
                5 => worker.pointer_link(cursor, true),
                6 => worker.pointer_link(cursor, false),
                7 => worker.returned_pointer_matches(cursor),
                8 => worker.read_provider_reference(cursor),
                9 => worker.release_provider_reference(cursor),
                _ => Err(invalid("unknown native worker request")),
            };
            match result {
                Ok(result) => {
                    let mut packet = Packet::new(0);
                    packet.bytes(&result)?;
                    wire::send(&mut stream, &packet.0)?;
                }
                Err(error) => {
                    let mut packet = Packet::new(1);
                    packet.text(&error.to_string())?;
                    wire::send(&mut stream, &packet.0)?;
                    return Err(error);
                }
            }
        }
    })();
    std::process::exit(if result.is_ok() { 0 } else { 1 });
}

impl Worker {
    fn install(&mut self, mut cursor: Cursor<'_>) -> io::Result<Vec<u8>> {
        let ordinal = cursor.usize()?;
        if ordinal != self.adapters.len() {
            return Err(invalid("native adapter occurrence is stale or repeated"));
        }
        let export = cursor.text()?;
        let bytes = cursor.bytes()?;
        let digest = cursor.bytes()?;
        if Sha256::digest(bytes).as_slice() != digest {
            return Err(invalid(
                "native adapter bytes differ from their admitted digest",
            ));
        }
        let count = cursor.usize()?;
        if count > cursor.remaining() / 17 {
            return Err(invalid("native adapter parameter telescope is truncated"));
        }
        let mut parameters = Vec::with_capacity(count);
        for _ in 0..count {
            parameters.push(Geometry::read(&mut cursor)?);
        }
        let result = match cursor.byte()? {
            0 => None,
            1 => Some(Geometry::read(&mut cursor)?),
            _ => return Err(invalid("native adapter result marker is malformed")),
        };
        if result.is_some_and(|result| {
            matches!(
                result.referent,
                Some(BorrowGeometry::ByteView) | Some(BorrowGeometry::Fixed { extent: 0, .. })
            )
        }) {
            return Err(invalid(
                "provider reference result needs nonempty fixed referent geometry",
            ));
        }
        cursor.finish()?;
        let extension = match target::NativeTarget::host().object_format {
            target::ObjectFormat::Elf => "so",
            target::ObjectFormat::MachO => "dylib",
            target::ObjectFormat::Coff => "dll",
        };
        let path = self
            .directory
            .join(format!("adapter-{ordinal}.{extension}"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(bytes)?;
        drop(file);
        let entry = unsafe { LoadedFrame::load(&path, export) }?;
        self.adapters.push(Adapter {
            entry,
            parameters,
            result,
        });
        Ok(Vec::new())
    }

    fn attach(&mut self, mut cursor: Cursor<'_>) -> io::Result<Vec<u8>> {
        let id = cursor.number()?;
        let extent = cursor.usize()?;
        cursor.finish()?;
        if id != self.homes.len() as u64 + 1 {
            return Err(invalid("native home occurrence is stale or repeated"));
        }
        if extent == 0 || extent > wire::MAXIMUM_PACKET_BYTES {
            return Err(invalid("native home exceeds the sponsor's mapping budget"));
        }
        let home = SharedBacking::attach(&home_path(&self.directory, id), extent)?;
        self.homes.insert(id, home);
        Ok(Vec::new())
    }

    fn call(&mut self, mut cursor: Cursor<'_>) -> io::Result<Vec<u8>> {
        let ordinal = cursor.usize()?;
        let adapter = self
            .adapters
            .get(ordinal)
            .ok_or_else(|| invalid("native call names no installed adapter"))?;
        let count = cursor.usize()?;
        if count != adapter.parameters.len() {
            return Err(invalid("native call has a different parameter telescope"));
        }
        let mut inputs = Vec::with_capacity(count);
        for geometry in &adapter.parameters {
            let mut value = NativeValue::zeroed(geometry.extent, geometry.alignment)?;
            match (cursor.byte()?, geometry.referent) {
                (0, None) => value.initialize(cursor.bytes()?)?,
                (1, Some(BorrowGeometry::Fixed { extent, alignment })) => {
                    let id = cursor.number()?;
                    let offset = cursor.usize()?;
                    let home = self
                        .homes
                        .get(&id)
                        .ok_or_else(|| invalid("native loan names no mapped original home"))?;
                    let pointer = home.pointer(offset, extent, alignment)?;
                    value.initialize(&(pointer as u64).to_le_bytes())?;
                }
                (2, Some(BorrowGeometry::ByteView)) => {
                    let id = cursor.number()?;
                    let offset = cursor.usize()?;
                    let length = cursor.usize()?;
                    let home = self
                        .homes
                        .get(&id)
                        .ok_or_else(|| invalid("native byte view names no original home"))?;
                    let pointer = home.pointer(offset, length, 1)?;
                    let mut pair = [0; 16];
                    pair[..8].copy_from_slice(&(pointer as u64).to_le_bytes());
                    pair[8..].copy_from_slice(&(length as u64).to_le_bytes());
                    value.initialize(&pair)?;
                }
                _ => {
                    return Err(invalid(
                        "native value and mapped-loan carriers do not match",
                    ));
                }
            }
            inputs.push(value);
        }
        cursor.finish()?;
        let result_geometry = adapter.result;
        let output = result_geometry
            .map(|geometry| NativeValue::zeroed(geometry.extent, geometry.alignment))
            .transpose()?;
        let pointers = inputs
            .iter()
            .map(|value| value.pointer().cast_const())
            .collect::<Vec<_>>();
        let result_pointer = output
            .as_ref()
            .map_or(std::ptr::null_mut(), NativeValue::pointer);
        let error_slot = self.restore_guest_error();
        unsafe { adapter.entry.call(&pointers, result_pointer) };
        self.capture_guest_error(error_slot);
        let returned = output.as_ref().map_or_else(Vec::new, NativeValue::bytes);
        match result_geometry.and_then(|geometry| geometry.referent) {
            Some(BorrowGeometry::Fixed { extent, alignment }) => {
                self.capture_provider_reference(ordinal as u64, &returned, extent, alignment)
            }
            Some(BorrowGeometry::ByteView) => Err(invalid(
                "provider byte-view result has no admitted read protocol",
            )),
            None => Ok(returned),
        }
    }

    /// Invoke only after framing, metadata work and allocations. The next
    /// operation must observe guest state or enter the native adapter.
    fn restore_guest_error(&self) -> GuestErrorSlot {
        #[cfg(target_os = "macos")]
        let errno = unsafe { libc::__error() };
        #[cfg(target_os = "linux")]
        let errno = unsafe { libc::__errno_location() };
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        unsafe {
            *errno = self.guest_errno;
        }
        #[cfg(windows)]
        unsafe {
            windows_sys::Win32::Foundation::SetLastError(self.guest_last_error);
        }
        GuestErrorSlot {
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            errno,
        }
    }

    fn capture_guest_error(&mut self, _slot: GuestErrorSlot) {
        #[cfg(windows)]
        {
            self.guest_last_error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            self.guest_errno = unsafe { *_slot.errno };
        }
    }

    fn capture_provider_reference(
        &mut self,
        adapter: u64,
        returned: &[u8],
        extent: usize,
        alignment: usize,
    ) -> io::Result<Vec<u8>> {
        let bytes: [u8; 8] = returned
            .try_into()
            .map_err(|_| invalid("provider reference result is not one pointer word"))?;
        let address = usize::try_from(u64::from_le_bytes(bytes))
            .map_err(|_| invalid("provider reference address exceeds the worker address model"))?;
        if address == 0
            || !address.is_multiple_of(alignment)
            || address.checked_add(extent).is_none()
        {
            return Err(invalid(
                "provider reference result is null, misaligned or overflowing",
            ));
        }
        let occurrence = self
            .provider_references
            .len()
            .checked_add(1)
            .and_then(|ordinal| u64::try_from(ordinal).ok())
            .ok_or_else(|| invalid("provider reference occurrence identities exhausted"))?;
        self.provider_references.push(Some(ProviderReference {
            adapter,
            address,
            extent,
        }));
        let mut result = Packet(Vec::with_capacity(16));
        result.number(adapter);
        result.number(occurrence);
        Ok(result.0)
    }

    fn read_provider_reference(&mut self, mut cursor: Cursor<'_>) -> io::Result<Vec<u8>> {
        let adapter = cursor.number()?;
        let occurrence = cursor.number()?;
        let offset = cursor.usize()?;
        let length = cursor.usize()?;
        cursor.finish()?;
        let reference = self
            .provider_references
            .get(occurrence_index(occurrence)?)
            .and_then(Option::as_ref)
            .filter(|reference| reference.adapter == adapter)
            .ok_or_else(|| invalid("provider read names no exact live result occurrence"))?;
        if length == 0
            || length > wire::MAXIMUM_PACKET_BYTES
            || offset
                .checked_add(length)
                .is_none_or(|end| end > reference.extent)
        {
            return Err(invalid(
                "provider reference read exceeds its admitted referent",
            ));
        }
        let pointer = std::ptr::with_exposed_provenance::<u8>(reference.address + offset);
        let mut output = vec![0; length];
        self.restore_guest_error();
        // SAFETY: the exact admitted provider result establishes this live
        // fixed referent. Its address never comes from a read request, and
        // the complete range was checked above. A provider fault remains
        // contained in this worker incarnation; no scalar is fabricated.
        unsafe { std::ptr::copy_nonoverlapping(pointer, output.as_mut_ptr(), length) };
        Ok(output)
    }

    fn release_provider_reference(&mut self, mut cursor: Cursor<'_>) -> io::Result<Vec<u8>> {
        let adapter = cursor.number()?;
        let occurrence = cursor.number()?;
        cursor.finish()?;
        self.provider_references
            .get(occurrence_index(occurrence)?)
            .and_then(Option::as_ref)
            .filter(|reference| reference.adapter == adapter)
            .ok_or_else(|| invalid("provider release names no exact live result occurrence"))?;
        self.provider_references[occurrence_index(occurrence)?] = None;
        Ok(Vec::new())
    }
}

pub(crate) fn home_path(directory: &Path, id: u64) -> PathBuf {
    directory.join(format!("home-{id}"))
}

fn configure_failure_reporting() -> io::Result<()> {
    #[cfg(unix)]
    {
        let limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limit) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
        };
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
    Ok(())
}
