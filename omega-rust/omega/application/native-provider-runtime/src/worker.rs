//! One isolated provider incarnation: load exact adapter images, map original
//! homes, then serve serialized calls. Code and mappings remain resident until
//! actual process exit, including native process-exit and TLS callbacks.
//! A failed call or decoder closes the incarnation; it is never replayed.

use crate::{
    channel,
    native_code::{LoadedFrame, NativeValue},
    shared_backing::SharedBacking,
    wire::{self, Cursor, Geometry, Packet, invalid},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::{self, Write},
    mem::ManuallyDrop,
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) const CHANNEL_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_CHANNEL";
pub(crate) const PARENT_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_PARENT";
pub(crate) const ROOT_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_ROOT";
pub(crate) const TIMEOUT_ENV: &str = "OMEGA_INTERNAL_NATIVE_PROVIDER_TIMEOUT_MS";

struct Adapter {
    entry: LoadedFrame,
    parameters: Vec<Geometry>,
    result: Option<Geometry>,
}
pub(crate) struct Worker {
    // A Pending never silently moves into a reloaded library's state. The
    // whole worker is retained through native process teardown, not dropped
    // before atexit/TLS callbacks can use retained pointers or code.
    adapters: Vec<Adapter>,
    pub(crate) homes: BTreeMap<u64, SharedBacking>,
    directory: PathBuf,
}

/// Internal executable entry used by an embedding sponsor's actual bounded
/// child. Absence is an ordinary CLI/test invocation. The private endpoint,
/// exact parent identity and owned directory come from the launcher; standard
/// streams carry no control requests and cannot forge a successful response.
pub fn run_worker_if_requested() {
    let Some(endpoint) = std::env::var_os(CHANNEL_ENV) else {
        return;
    };
    let result = (|| {
        configure_failure_reporting()?;
        let endpoint = endpoint
            .into_string()
            .map_err(|_| invalid("native worker endpoint is not text"))?;
        let timeout: u64 = std::env::var(TIMEOUT_ENV)
            .map_err(|_| invalid("native worker has no sponsor deadline"))?
            .parse()
            .map_err(|_| invalid("native worker sponsor deadline is malformed"))?;
        if timeout == 0 || timeout > 3_600_000 {
            return Err(invalid("native worker sponsor deadline is out of range"));
        }
        let timeout = Duration::from_millis(timeout);
        let directory = std::env::var_os(ROOT_ENV)
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
            std::env::var(PARENT_ENV)
                .map_err(|_| invalid("native worker has no parent identity"))?
                .parse()
                .map_err(|_| invalid("native worker parent identity is malformed"))?,
            timeout,
        )?;
        let mut worker = ManuallyDrop::new(Worker {
            adapters: Vec::new(),
            homes: BTreeMap::new(),
            directory: directory.into(),
        });
        loop {
            let packet = wire::receive(&mut stream)?;
            let mut cursor = Cursor::new(&packet);
            let tag = cursor.byte()?;
            if tag == 4 {
                cursor.finish()?;
                // Acknowledge the retirement request, then exit the dedicated
                // process with every native library and home still resident.
                // The sponsor must observe actual exit before retiring its
                // mappings. This is never a semantic completion receipt.
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
    // Never return into an arbitrary embedding runtime after loading native
    // code. process::exit runs native exit callbacks while the intentionally
    // retained Worker remains intact; the OS reclaims its mappings afterward.
    // Errors likewise cannot unmap pointers already retained by a provider.
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
        if result.is_some_and(|result| result.referent.is_some()) {
            return Err(invalid(
                "native borrowed result needs an exact returned-home correspondence",
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
        // The authenticated parent supplies a compiler-produced fixed-frame
        // product. Its complete bytes/hash and export are retained unchanged.
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
                (1, Some((extent, alignment))) => {
                    let id = cursor.number()?;
                    let offset = cursor.usize()?;
                    let home = self
                        .homes
                        .get(&id)
                        .ok_or_else(|| invalid("native loan names no mapped original home"))?;
                    let pointer = home.pointer(offset, extent, alignment)?;
                    value.initialize(&(pointer as u64).to_le_bytes())?;
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
        let output = adapter
            .result
            .map(|geometry| NativeValue::zeroed(geometry.extent, geometry.alignment))
            .transpose()?;
        let pointers = inputs
            .iter()
            .map(|value| value.pointer().cast_const())
            .collect::<Vec<_>>();
        // Complete argument extents and pointer/home ranges have been joined
        // above. The selected provider's behavior remains the admitted trusted
        // premise; actual native traps kill only this worker incarnation.
        unsafe {
            adapter.entry.call(
                &pointers,
                output
                    .as_ref()
                    .map_or(std::ptr::null_mut(), NativeValue::pointer),
            );
        }
        Ok(output.as_ref().map_or_else(Vec::new, NativeValue::bytes))
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
