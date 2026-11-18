//! Install completed, content-identified adapters in the explicitly authorized native host.
#![forbid(unsafe_code)]

use bounded_process::{BoundedProcessChild, BoundedProcessLimits, BoundedProcessPrepared};
use calling_conventions::PublishedForeignAdapterIdentity;
use native_provider_runtime::{NativeProviderCommand, NativeProviderError, NativeProviderInstallation, NativeProviderProcess, transport};
use product_publication::{NativeForeignAdapterImage, TerminalRuntimeProvisions};
use std::{cell::RefCell, io, process::Command, rc::Rc, sync::{Arc, atomic::{AtomicU64, Ordering}}, time::Duration};
use transport::{Duplex, Packet};

pub fn for_provisions(command: Command, provisions: Option<&TerminalRuntimeProvisions>) -> NativeProviderCommand {
    let roster = provisions.into_iter().flat_map(|runtime| runtime.published_native_adapters()).cloned().collect();
    NativeProviderCommand::new(command, move |command, timeout| launch(command, timeout, roster))
}

fn invalid(reason: &'static str) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, reason) }

struct PublishedAdapterInstallation {
    channel: Rc<RefCell<Box<dyn Duplex>>>,
    roster: Vec<Arc<NativeForeignAdapterImage>>,
}

impl NativeProviderInstallation for PublishedAdapterInstallation {
    fn install(&mut self, adapter: &PublishedForeignAdapterIdentity, ordinal: u64, storage: &[u8]) -> io::Result<Vec<u8>> {
        let image = lookup_adapter(&self.roster, adapter)?;
        install_image(self.channel.borrow_mut().as_mut(), image, ordinal, storage)
    }
}

fn lookup_adapter<'roster>(
    roster: &'roster [Arc<NativeForeignAdapterImage>],
    requested: &PublishedForeignAdapterIdentity,
) -> io::Result<&'roster NativeForeignAdapterImage> {
    roster.iter().find(|candidate| {
        candidate.application() == &requested.application
            && candidate.locator() == &requested.locator
            && candidate.bridge().request() == &requested.bridge
            && candidate.image_digest() == &requested.image_digest
    }).map(AsRef::as_ref).ok_or_else(|| invalid("native adapter does not belong to this sponsor's published roster"))
}

fn install_image(mut channel: impl io::Read + io::Write, image: &NativeForeignAdapterImage, ordinal: u64, storage: &[u8]) -> io::Result<Vec<u8>> {
    let mut packet = Packet::new(1);
    packet.number(ordinal);
    packet.text(image.export())?;
    packet.bytes(image.container().contents())?;
    packet.bytes(image.image_digest())?;
    packet.append(storage);
    transport::exchange(&mut channel, packet.as_bytes())
}

    fn launch(command: Command, timeout: Duration, roster: Vec<Arc<NativeForeignAdapterImage>>) -> Result<NativeProviderProcess, NativeProviderError> {
    let (child, socket, directory) = spawn_worker(command, timeout)?;
    let channel: Rc<RefCell<Box<dyn Duplex>>> = Rc::new(RefCell::new(Box::new(socket)));
    let installation = PublishedAdapterInstallation { channel: Rc::clone(&channel), roster };
    NativeProviderProcess::from_sponsor(child, channel, Box::new(installation), directory, timeout)
}

fn spawn_worker(command: Command, timeout: Duration) -> Result<(BoundedProcessChild, impl io::Read + io::Write + 'static, std::path::PathBuf), io::Error> {
        if timeout.is_zero() || timeout > Duration::from_secs(3600) {
            return Err(invalid("native sponsor deadline is out of range"));
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
        let builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        let builder = {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = builder;
            builder.mode(0o700);
            builder
        };
        builder.create(&directory)?;
        let launched = (|| {
            let private = transport::prepare(timeout)?;
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
                .env(transport::ROOT_ENV, &directory)
                .env(transport::PARENT_ENV, std::process::id().to_string())
                .env(transport::TIMEOUT_ENV, timeout.as_millis().max(1).to_string());
            #[cfg(unix)]
            let stream = {
                let descriptor = prepared.retain_descriptor(private.child)?;
                prepared.env(transport::CHANNEL_ENV, descriptor.to_string());
                private.parent
            };
            #[cfg(windows)]
            prepared.env(transport::CHANNEL_ENV, &private.name);
            let mut child = BoundedProcessChild::spawn(prepared)?;
            #[cfg(windows)]
            let stream = private.accept(child.id())?;
            #[cfg(unix)]
            let _ = &mut child;
            Ok::<_, io::Error>((child, Box::new(stream) as Box<dyn Duplex>))
        })();
        match launched {
            Ok((child, channel)) => Ok((child, channel, directory)),
            Err(error) => {
                let _ = std::fs::remove_dir_all(directory);
                Err(error)
            }
        }
    }



#[cfg(feature = "test-support")]
pub mod test_support {
    use super::*;
    /// Start a fresh worker for tests that deliberately exercise the IPC protocol.
    pub fn spawn(command: Command, timeout: Duration) -> io::Result<(BoundedProcessChild, impl io::Read + io::Write + 'static, std::path::PathBuf)> {
        spawn_worker(command, timeout)
    }

    pub fn install(channel: impl io::Read + io::Write, product: &NativeForeignAdapterImage, ordinal: u64, storage: &[u8]) -> io::Result<Vec<u8>> {
        install_image(channel, product, ordinal, storage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{CallSignature, CallingPolicy, NativeForeignBridgeRequest, ValueShape, evaluate_ordinary_boundary_entry_plan};
    use target::{ForeignLocatorCandidate, TargetProfile, normalize_foreign_locator};

    #[test]
    fn sponsor_lookup_requires_the_actual_product_and_complete_abi() {
        let profile = TargetProfile::LinuxX64;
        let target = profile.native_target();
        let locate = |symbol: &[u8]| normalize_foreign_locator(ForeignLocatorCandidate::ElfVersioned {
            object: b"libc.so.6".to_vec(), symbol: symbol.to_vec(), version: b"GLIBC_2.2.5".to_vec(),
        }, profile).unwrap();
        let plan = |parameters| evaluate_ordinary_boundary_entry_plan(CallingPolicy::native_for_target(target), &CallSignature {
            parameters, result: Some(ValueShape::integer(4, 4)),
        }).unwrap();
        let bridge = image_fixtures::emit_native_foreign_bridge(NativeForeignBridgeRequest {
            target, foreign: plan(Vec::new()), scalar_parameters: Vec::new(),
        }).unwrap();
        let locator = locate(b"getpid");
        let imports = image_fixtures::encode_locator_import(&locator);
        let image = image_emission::emit_native_foreign_adapter(bridge, locator, [1; 32], imports).unwrap();
        let roster = [Arc::new(image)];
        let exact = roster[0].runtime_identity();
        assert!(std::ptr::eq(lookup_adapter(&roster, &exact).unwrap(), roster[0].as_ref()));
        assert!(lookup_adapter(&[], &exact).is_err());
        let mut differing = exact.clone();
        differing.image_digest[3] ^= 1;
        assert!(lookup_adapter(&roster, &differing).is_err());
        differing = exact.clone();
        differing.application[3] ^= 1;
        assert!(lookup_adapter(&roster, &differing).is_err());
        differing = exact.clone();
        differing.locator = locate(b"getuid");
        assert!(lookup_adapter(&roster, &differing).is_err());
        differing = exact.clone();
        differing.bridge.target = TargetProfile::MacosX64.native_target();
        assert!(lookup_adapter(&roster, &differing).is_err());
        differing = exact.clone();
        differing.bridge.foreign = plan(vec![ValueShape::integer(8, 8)]);
        assert!(lookup_adapter(&roster, &differing).is_err());
        differing = exact;
        differing.bridge.scalar_parameters.push(None);
        assert!(lookup_adapter(&roster, &differing).is_err());
    }
}
