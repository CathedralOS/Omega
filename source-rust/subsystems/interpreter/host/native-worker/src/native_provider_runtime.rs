/*
We sponsor an isolated provider process (`NativeProviderProcess`) and its
immutable file-backed native homes (`NativeHomeId`).

The host sponsor resolves each requested adapter identity against its published
images and installs it on a private channel. The process owns the resulting
slots and mapped ranges for one worker incarnation. The interpreter correlates
normal calls with actual claims, content guarantees, and retained loans.

When creating a shared home, we insert the parent's physical `SharedBacking`
before lending the mapping to the worker so a rejected attach cannot free a
mapping already opened by the worker. On exchange failure, we briefly poll
`try_wait` so a real native trap's exit status is observed before forced
process cleanup.
*/

use crate::{
    provider_references::{NativeResultStorage, ProviderReferenceResult},
    shared_backing::SharedBacking,
    wire::{self, BorrowGeometry, Geometry, Packet, invalid},
    worker,
};
use bounded_process::BoundedProcessChild;
use calling_conventions::PublishedForeignAdapterIdentity;
use calling_conventions::{ValueClass, ValueShape};
use std::{
    collections::BTreeMap,
    io,
    path::Path,
    process::{Command, ExitStatus},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeAdapterId {
    pub(crate) era: u64,
    pub(crate) ordinal: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeHomeId {
    pub(crate) era: u64,
    pub(crate) ordinal: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeParameterStorage {
    Value,
    OriginalReference { byte_size: usize, alignment: usize },
    OriginalByteView,
}

pub enum NativeArgument<'a> {
    Value(&'a [u8]),
    Borrowed {
        home: NativeHomeId,
        offset: usize,
    },
    ByteView {
        home: NativeHomeId,
        offset: usize,
        length: usize,
    },
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

/// Framed, non-installing worker operation issued by the runtime for this
/// session. Only the host that owns the transport can submit it.
pub struct NativeProviderOperation<'a>(&'a [u8]);
impl NativeProviderOperation<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        self.0
    }
}

/// Non-executable parameter and result geometry, checked against the ABI by
/// the runtime before the host installs its matching published adapter.
pub struct NativeAdapterStorage<'a>(&'a [u8]);
impl NativeAdapterStorage<'_> {
    pub fn as_bytes(&self) -> &[u8] {
        self.0
    }
}

/// A live worker session supplied by the explicit native sponsor. The
/// interpreter has no handle to the worker's private installation channel.
pub trait NativeProviderSession {
    fn home_directory(&self) -> &Path;
    /// Release the sponsor's newly created mappings directory only after this
    /// worker has retired. A custom sponsor owns its own storage cleanup.
    fn release_home_directory(&mut self);

    fn install(
        &mut self,
        adapter: &PublishedForeignAdapterIdentity,
        ordinal: u64,
        storage: NativeAdapterStorage<'_>,
    ) -> io::Result<Vec<u8>>;
    fn exchange(&mut self, operation: NativeProviderOperation<'_>) -> io::Result<Vec<u8>>;
}

/// A command and its explicitly supplied native host. The interpreter starts
/// it only when an admitted call actually needs a provider.
pub struct NativeProviderCommand {
    command: Command,
    start: Box<
        dyn FnOnce(Command, Duration) -> Result<NativeProviderProcess, NativeProviderError> + Send,
    >,
}
impl NativeProviderCommand {
    pub fn new(
        command: Command,
        start: impl FnOnce(Command, Duration) -> Result<NativeProviderProcess, NativeProviderError>
        + Send
        + 'static,
    ) -> Self {
        Self {
            command,
            start: Box::new(start),
        }
    }
    pub fn current_dir(&mut self, directory: &Path) {
        self.command.current_dir(directory);
    }

    pub fn start(self, timeout: Duration) -> Result<NativeProviderProcess, NativeProviderError> {
        (self.start)(self.command, timeout)
    }
}

pub struct NativeProviderProcess {
    child: Option<BoundedProcessChild>,
    session: Box<dyn NativeProviderSession>,
    pub(crate) homes: BTreeMap<u64, SharedBacking>,
    pub(crate) era: u64,
    pub(crate) adapters: Vec<NativeResultStorage>,
    pub(crate) provider_references: Vec<Option<ProviderReferenceResult>>,
    timeout: Duration,
    pub(crate) faulted: bool,
}

impl NativeProviderProcess {
    /// Adopt a worker created by its sponsor. Installation stays with that
    /// sponsor. The requester can only name an adapter in the published roster.
    pub fn from_sponsor(
        child: BoundedProcessChild,
        mut session: Box<dyn NativeProviderSession>,
        timeout: Duration,
    ) -> Result<Self, NativeProviderError> {
        static NEXT_ERA: AtomicU64 = AtomicU64::new(1);
        let era = if timeout.is_zero() || timeout > Duration::from_secs(3600) {
            Err(invalid("native sponsor deadline is out of range"))
        } else {
            NEXT_ERA
                .try_update(Ordering::Relaxed, Ordering::Relaxed, |era| {
                    era.checked_add(1)
                })
                .map_err(|_| invalid("native provider occurrence identities exhausted"))
        };
        let era = match era {
            Ok(era) => era,
            Err(error) => {
                drop(child);
                session.release_home_directory();
                return Err(error.into());
            }
        };
        Ok(Self {
            child: Some(child),
            session,
            homes: BTreeMap::new(),
            era,
            adapters: Vec::new(),
            provider_references: Vec::new(),
            timeout,
            faulted: false,
        })
    }

    /// Result storage comes from the host's complete admitted boundary
    /// contract. An adapter's pointer-shaped ABI alone grants no referent.
    pub fn install_published_with_result(
        &mut self,
        adapter: &PublishedForeignAdapterIdentity,
        storage: &[NativeParameterStorage],
        result_storage: NativeResultStorage,
    ) -> Result<NativeAdapterId, NativeProviderError> {
        self.install_admitted(
            adapter,
            storage,
            result_storage,
            |this, ordinal, storage| {
                this.session
                    .install(adapter, ordinal, NativeAdapterStorage(storage))
            },
        )
    }

    fn install_admitted(
        &mut self,
        adapter: &PublishedForeignAdapterIdentity,
        storage: &[NativeParameterStorage],
        result_storage: NativeResultStorage,
        install: impl FnOnce(&mut Self, u64, &[u8]) -> io::Result<Vec<u8>>,
    ) -> Result<NativeAdapterId, NativeProviderError> {
        if adapter.bridge.target != target::NativeTarget::host() {
            return Err(invalid("native frame product targets another host").into());
        }
        let plan = &adapter.bridge.foreign.plan().call;
        if storage.len() != plan.parameters.len() {
            return Err(invalid("native parameter storage has an incomplete telescope").into());
        }
        let ordinal = u64::try_from(self.adapters.len())
            .map_err(|_| invalid("native adapter occurrence identities exhausted"))?;
        let mut packet = Packet(Vec::new());
        packet.number(plan.parameters.len() as u64);
        for (parameter, storage) in plan.parameters.iter().zip(storage) {
            geometry(parameter.shape, *storage)?.write(&mut packet);
        }
        match &plan.result {
            None if result_storage == NativeResultStorage::Value => packet.byte(0),
            None => return Err(invalid("provider reference result has no result carrier").into()),
            Some(result) => {
                packet.byte(1);
                match result_storage {
                    NativeResultStorage::Value => {
                        geometry(result.shape, NativeParameterStorage::Value)?
                    }
                    NativeResultStorage::SharedProviderReference {
                        byte_size,
                        alignment,
                    } if result.shape == ValueShape::integer(8, 8) => geometry(
                        result.shape,
                        NativeParameterStorage::OriginalReference {
                            byte_size,
                            alignment,
                        },
                    )?,
                    _ => {
                        return Err(invalid(
                            "provider reference result is not its exact physical pointer transport",
                        )
                        .into());
                    }
                }
                .write(&mut packet);
            }
        }
        if self.faulted || self.child.is_none() {
            return Err(invalid("native provider incarnation is closed or faulted").into());
        }
        let result = install(self, ordinal, &packet.0).and_then(|acknowledgment| {
            if acknowledgment.is_empty() {
                Ok(acknowledgment)
            } else {
                Err(invalid(
                    "native adapter installation returned a malformed acknowledgment",
                ))
            }
        });
        self.finish_exchange(result)?;
        let id = NativeAdapterId {
            era: self.era,
            ordinal,
        };
        self.adapters.push(result_storage);
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
        let path = worker::home_path(self.session.home_directory(), ordinal);
        let home = SharedBacking::create(&path, extent)?;
        if let Err(error) = home.pointer(0, extent, alignment) {
            drop(home);
            std::fs::remove_file(path)?;
            return Err(error.into());
        }
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
        if self.installed_result_storage(adapter)? != NativeResultStorage::Value {
            return Err(
                invalid("provider reference completion requires its typed result handle").into(),
            );
        }
        let packet = self.call_packet(adapter, arguments)?;
        self.exchange(packet)
    }

    pub(crate) fn installed_result_storage(
        &self,
        adapter: NativeAdapterId,
    ) -> Result<NativeResultStorage, NativeProviderError> {
        if adapter.era != self.era {
            return Err(
                invalid("native adapter belongs to a different provider incarnation").into(),
            );
        }
        self.adapters
            .get(
                usize::try_from(adapter.ordinal)
                    .map_err(|_| invalid("native adapter ordinal overflows"))?,
            )
            .copied()
            .ok_or_else(|| invalid("native call names no installed adapter").into())
    }

    pub(crate) fn call_packet(
        &self,
        adapter: NativeAdapterId,
        arguments: &[NativeArgument<'_>],
    ) -> Result<Packet, NativeProviderError> {
        self.installed_result_storage(adapter)?;
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
                NativeArgument::ByteView {
                    home,
                    offset,
                    length,
                } => {
                    if home.era != self.era || !self.homes.contains_key(&home.ordinal) {
                        return Err(invalid(
                            "native byte view belongs to another or absent original home",
                        )
                        .into());
                    }
                    packet.byte(2);
                    packet.number(home.ordinal);
                    packet.number(*offset as u64);
                    packet.number(*length as u64);
                }
            }
        }
        Ok(packet)
    }

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
                    "native provider did not exit after retirement request acknowledgement",
                )
                .into());
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        self.retire_process()?;
        if !status.success() {
            return Err(NativeProviderError {
                reason: "native provider failed during physical process retirement".into(),
                status: Some(status),
            });
        }
        Ok(())
    }

    pub(crate) fn exchange(&mut self, packet: Packet) -> Result<Vec<u8>, NativeProviderError> {
        if self.faulted || self.child.is_none() {
            return Err(invalid("native provider incarnation is closed or faulted").into());
        }
        let exchange = self.session.exchange(NativeProviderOperation(&packet.0));
        self.finish_exchange(exchange)
    }

    fn finish_exchange(
        &mut self,
        exchange: io::Result<Vec<u8>>,
    ) -> Result<Vec<u8>, NativeProviderError> {
        exchange.map_err(|error| {
            self.faulted = true;
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
        if self.retire_process().is_ok() {
            self.homes.clear();
            self.session.release_home_directory();
        }
    }
}

#[cfg(test)]
pub(crate) mod test_session;

fn geometry(shape: ValueShape, storage: NativeParameterStorage) -> io::Result<Geometry> {
    match storage {
        NativeParameterStorage::OriginalReference {
            byte_size,
            alignment,
        } if byte_size != 0
            && alignment.is_power_of_two()
            && (shape == ValueShape::integer(8, 8)
                || (shape.class == ValueClass::BorrowedReference
                    && shape.byte_size as usize == byte_size
                    && usize::from(shape.alignment) == alignment)) =>
        {
            Ok(Geometry {
                extent: 8,
                alignment: 8,
                referent: Some(BorrowGeometry::Fixed {
                    extent: byte_size,
                    alignment,
                }),
            })
        }
        NativeParameterStorage::OriginalByteView if shape == ValueShape::integer(16, 8) => {
            Ok(Geometry {
                extent: 16,
                alignment: 8,
                referent: Some(BorrowGeometry::ByteView),
            })
        }
        NativeParameterStorage::Value
            if !matches!(
                shape.class,
                ValueClass::BorrowedReference | ValueClass::BorrowedScalarReference
            ) =>
        {
            Ok(Geometry {
                extent: shape.byte_size as usize,
                alignment: usize::from(shape.alignment),
                referent: None,
            })
        }
        _ => Err(invalid(
            "native parameter storage disagrees with its exact physical value transport",
        )),
    }
}
