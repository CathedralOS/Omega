//! Sponsor one actual provider process and its immutable native homes.
//!
//! The process is a physical failure/lifetime boundary, not a semantic fallback.
//! Each installed slot consumes a produced exact ABI image. Mapped ranges stay
//! owned independently in both processes; only the interpreter can correlate
//! normal calls with actual claims, content guarantees and retained loans.

use crate::{
    channel,
    shared_backing::SharedBacking,
    wire::{self, Cursor, Duplex, Geometry, Packet, invalid},
    worker,
};
use bounded_process::{BoundedProcessChild, BoundedProcessLimits, BoundedProcessPrepared};
use calling_conventions::{ValueClass, ValueShape};
use native_artifact::NativeForeignAdapterImage;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    process::{Command, ExitStatus},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeAdapterId {
    era: u64,
    ordinal: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHomeId {
    era: u64,
    ordinal: u64,
}

pub enum NativeArgument<'a> {
    Value(&'a [u8]),
    Borrowed { home: NativeHomeId, offset: usize },
}

#[derive(Debug)]
pub struct NativeProviderError {
    pub reason: String,
    pub status: Option<ExitStatus>,
}
impl std::fmt::Display for NativeProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native provider: {}", self.reason)?;
        if let Some(status) = self.status {
            write!(f, " ({status})")?;
        }
        Ok(())
    }
}
impl std::error::Error for NativeProviderError {}
impl From<io::Error> for NativeProviderError {
    fn from(error: io::Error) -> Self {
        Self {
            reason: error.to_string(),
            status: None,
        }
    }
}

pub struct NativeProviderProcess {
    child: Option<BoundedProcessChild>,
    channel: Box<dyn Duplex>,
    homes: BTreeMap<u64, SharedBacking>,
    directory: PathBuf,
    era: u64,
    adapters: u64,
    timeout: Duration,
    faulted: bool,
}

impl NativeProviderProcess {
    /// The executable must call `run_worker_if_requested` before ordinary
    /// dispatch. This private sponsor provides native storage and no standard
    /// stream capability; callers must admit the actual boundary's effects.
    pub fn launch(command: Command, timeout: Duration) -> Result<Self, NativeProviderError> {
        if timeout.is_zero() || timeout > Duration::from_secs(3600) {
            return Err(invalid("native sponsor deadline is out of range").into());
        }
        static NEXT_ERA: AtomicU64 = AtomicU64::new(1);
        let era = NEXT_ERA
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |era| {
                era.checked_add(1)
            })
            .map_err(|_| invalid("native provider occurrence identities exhausted"))?;
        let instant = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "omega-native-provider-{}-{instant}-{era}",
            std::process::id()
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory)?;
        let launched = (|| {
            let private = channel::prepare(timeout)?;
            let mut prepared = BoundedProcessPrepared::new(
                command,
                BoundedProcessLimits::new(
                    120,
                    8 * 1024 * 1024 * 1024,
                    64 * 1024 * 1024,
                    128,
                    16,
                    1024 * 1024 * 1024,
                    2 * 1024 * 1024 * 1024,
                ),
                "isolated native provider",
            )?;
            prepared.stdin_null().stdout_null().stderr_null();
            prepared
                .env(worker::ROOT_ENV, &directory)
                .env(worker::PARENT_ENV, std::process::id().to_string())
                .env(worker::TIMEOUT_ENV, timeout.as_millis().max(1).to_string());
            #[cfg(unix)]
            let stream = {
                let descriptor = prepared.retain_descriptor(private.child)?;
                prepared.env(worker::CHANNEL_ENV, descriptor.to_string());
                private.parent
            };
            #[cfg(windows)]
            prepared.env(worker::CHANNEL_ENV, &private.name);
            let mut child = BoundedProcessChild::spawn(prepared)?;
            #[cfg(windows)]
            let stream = private.accept(child.id())?;
            #[cfg(unix)]
            let _ = &mut child;
            Ok::<_, io::Error>((child, Box::new(stream) as Box<dyn Duplex>))
        })();
        match launched {
            Ok((child, channel)) => Ok(Self {
                child: Some(child),
                channel,
                homes: BTreeMap::new(),
                directory,
                era,
                adapters: 0,
                timeout,
                faulted: false,
            }),
            Err(error) => {
                let _ = std::fs::remove_dir_all(directory);
                Err(error.into())
            }
        }
    }

    pub fn install(
        &mut self,
        adapter: &NativeForeignAdapterImage,
    ) -> Result<NativeAdapterId, NativeProviderError> {
        if adapter.bridge().request().target != target::NativeTarget::host() {
            return Err(invalid("native frame product targets another host").into());
        }
        let plan = &adapter.bridge().request().foreign.plan().call;
        let mut packet = Packet::new(1);
        packet.number(self.adapters);
        packet.text(adapter.export())?;
        packet.bytes(adapter.container().contents())?;
        packet.bytes(&Sha256::digest(adapter.container().contents()))?;
        packet.number(plan.parameters.len() as u64);
        for parameter in &plan.parameters {
            geometry(parameter.shape)?.write(&mut packet);
        }
        match &plan.result {
            None => packet.byte(0),
            Some(result) => {
                packet.byte(1);
                geometry(result.shape)?.write(&mut packet);
            }
        }
        self.exchange(packet)?;
        let id = NativeAdapterId {
            era: self.era,
            ordinal: self.adapters,
        };
        self.adapters = self
            .adapters
            .checked_add(1)
            .ok_or_else(|| invalid("native adapter occurrence identities exhausted"))?;
        Ok(id)
    }

    pub fn create_home(
        &mut self,
        extent: usize,
        alignment: usize,
    ) -> Result<NativeHomeId, NativeProviderError> {
        if extent == 0 || extent > wire::MAXIMUM_PACKET_BYTES || !alignment.is_power_of_two() {
            return Err(invalid("native home exceeds the sponsor's mapping budget").into());
        }
        let ordinal = (self.homes.len() as u64)
            .checked_add(1)
            .ok_or_else(|| invalid("native home occurrence identities exhausted"))?;
        let path = worker::home_path(&self.directory, ordinal);
        let home = SharedBacking::create(&path, extent)?;
        if let Err(error) = home.pointer(0, extent, alignment) {
            drop(home);
            std::fs::remove_file(path)?;
            return Err(error.into());
        }
        // Publish the parent's physical custodian before lending the mapping.
        // A rejected attach cannot free a mapping already opened by the worker.
        self.homes.insert(ordinal, home);
        let mut packet = Packet::new(2);
        packet.number(ordinal);
        packet.number(extent as u64);
        self.exchange(packet)?;
        Ok(NativeHomeId {
            era: self.era,
            ordinal,
        })
    }

    pub fn read_home(
        &self,
        home: NativeHomeId,
        offset: usize,
        output: &mut [u8],
    ) -> Result<(), NativeProviderError> {
        if self.faulted || home.era != self.era {
            return Err(invalid(
                "native home belongs to a faulted or different provider incarnation",
            )
            .into());
        }
        self.homes
            .get(&home.ordinal)
            .ok_or_else(|| invalid("native home is absent"))?
            .read(offset, output)?;
        Ok(())
    }
    pub fn write_home(
        &mut self,
        home: NativeHomeId,
        offset: usize,
        input: &[u8],
    ) -> Result<(), NativeProviderError> {
        if self.faulted || home.era != self.era {
            return Err(invalid(
                "native home belongs to a faulted or different provider incarnation",
            )
            .into());
        }
        self.homes
            .get_mut(&home.ordinal)
            .ok_or_else(|| invalid("native home is absent"))?
            .write(offset, input)?;
        Ok(())
    }

    pub fn call(
        &mut self,
        adapter: NativeAdapterId,
        arguments: &[NativeArgument<'_>],
    ) -> Result<Vec<u8>, NativeProviderError> {
        if adapter.era != self.era {
            return Err(
                invalid("native adapter belongs to a different provider incarnation").into(),
            );
        }
        let mut packet = Packet::new(3);
        packet.number(adapter.ordinal);
        packet.number(arguments.len() as u64);
        for argument in arguments {
            match argument {
                NativeArgument::Value(value) => {
                    packet.byte(0);
                    packet.bytes(value)?;
                }
                NativeArgument::Borrowed { home, offset } => {
                    if home.era != self.era || !self.homes.contains_key(&home.ordinal) {
                        return Err(invalid(
                            "native loan belongs to a different or absent original home",
                        )
                        .into());
                    }
                    packet.byte(1);
                    packet.number(home.ordinal);
                    packet.number(*offset as u64);
                }
            }
        }
        self.exchange(packet)
    }

    /// Retire the physical provider incarnation after native destructors have
    /// acknowledged closing while their mappings remain live. This is never
    /// a normal completion or release receipt for a guest ownership claim.
    pub fn close(mut self) -> Result<(), NativeProviderError> {
        self.exchange(Packet::new(4))?;
        let deadline = Instant::now() + self.timeout;
        let status = loop {
            let child = self
                .child
                .as_mut()
                .ok_or_else(|| invalid("native provider is already closed"))?;
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "native provider did not exit after physical retirement",
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        self.retire_process()?;
        if !status.success() {
            return Err(NativeProviderError {
                reason: "native provider failed after physical retirement acknowledgement".into(),
                status: Some(status),
            });
        }
        Ok(())
    }

    fn exchange(&mut self, packet: Packet) -> Result<Vec<u8>, NativeProviderError> {
        if self.faulted || self.child.is_none() {
            return Err(invalid("native provider incarnation is closed or faulted").into());
        }
        let exchange = (|| {
            wire::send(self.channel.as_mut(), &packet.0)?;
            let response = wire::receive(self.channel.as_mut())?;
            let mut cursor = Cursor::new(&response);
            match cursor.byte()? {
                0 => {
                    let result = cursor.bytes()?.to_vec();
                    cursor.finish()?;
                    Ok(result)
                }
                1 => Err(io::Error::other(cursor.text()?.to_owned())),
                _ => Err(invalid("native worker response is malformed")),
            }
        })();
        exchange.map_err(|error| {
            self.faulted = true;
            // A real trap can close the pipe slightly before its exit is
            // reapable. Observe that original status before forced cleanup.
            let deadline = Instant::now() + self.timeout.min(Duration::from_millis(100));
            let mut status = None;
            while let Some(child) = &mut self.child {
                status = child.try_wait().ok().flatten();
                if status.is_some() || Instant::now() >= deadline {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let cleanup = self.retire_process();
            let mut reason = error.to_string();
            if let Err(error) = cleanup {
                reason.push_str(&format!("; physical custody quarantined: {error}"));
            }
            NativeProviderError { reason, status }
        })
    }

    fn retire_process(&mut self) -> io::Result<()> {
        let Some(child) = &mut self.child else {
            return Ok(());
        };
        child.terminate()?;
        let deadline = Instant::now() + self.timeout;
        while child.try_wait()?.is_none() {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "native provider process was not reaped",
                ));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        self.child.take().unwrap().finish()?;
        Ok(())
    }
}

impl Drop for NativeProviderProcess {
    fn drop(&mut self) {
        // Abrupt private-domain abandonment is never a normal C completion.
        // If cleanup refuses, leave the nonrecycled files for quarantine. A
        // surviving worker owns its own mapping references, so parent Drop
        // cannot invalidate any retained pointer in that address space.
        if self.retire_process().is_ok() {
            self.homes.clear();
            let _ = std::fs::remove_dir_all(&self.directory);
        }
    }
}

fn geometry(shape: ValueShape) -> io::Result<Geometry> {
    match shape.class {
        ValueClass::BorrowedReference => Ok(Geometry {
            extent: 8,
            alignment: 8,
            referent: Some((shape.byte_size as usize, usize::from(shape.alignment))),
        }),
        ValueClass::BorrowedScalarReference => Err(invalid(
            "native scalar descriptors require their exact admitted scalar backing codec",
        )),
        _ => Ok(Geometry {
            extent: shape.byte_size as usize,
            alignment: usize::from(shape.alignment),
            referent: None,
        }),
    }
}
