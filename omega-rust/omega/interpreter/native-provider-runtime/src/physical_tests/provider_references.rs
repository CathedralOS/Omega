//! Real provider-cell reads test physical identity and bounded transport. The
//! host above this API still owes every semantic access and lifetime contract.

use super::{Command, TIMEOUT, native_calls::compile};
use crate::{
    NativeAdapterId, NativeArgument, NativeProviderProcess, NativeProviderReference,
    NativeResultStorage, wire::Packet,
};
use calling_conventions::{
    CallSignature, CallingPolicy, NativeForeignBridgeRequest, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use image_fixtures::emit_native_foreign_bridge;
use std::path::{Path, PathBuf};
use target::{ForeignLocatorCandidate, ObjectFormat, TargetProfile, normalize_foreign_locator};

const CELL: NativeResultStorage = NativeResultStorage::SharedProviderReference {
    byte_size: 4,
    alignment: 4,
};

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "omega-provider-reference-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut fixture = Self {
            library: PathBuf::new(),
            directory,
        };
        fixture.library = fixture.compile_library("cell");
        fixture
    }

    fn compile_library(&self, name: &str) -> PathBuf {
        let target = TargetProfile::host().native_target();
        let extension = match target.object_format {
            ObjectFormat::Elf => "so",
            ObjectFormat::MachO => "dylib",
            ObjectFormat::Coff => "dll",
        };
        let library = self.directory.join(format!("{name}.{extension}"));
        let script = self.directory.join("provider.map");
        std::fs::write(
            &script,
            "REFERENCE_1.0 { global: provider_reference_*; local: *; };\n",
        )
        .unwrap();
        let compiler = std::env::var_os("CC")
            .unwrap_or_else(|| if cfg!(windows) { "clang" } else { "cc" }.into());
        let mut command = Command::new(compiler);
        command.args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]);
        match target.object_format {
            ObjectFormat::Elf => {
                command
                    .args(["-shared", "-fPIC"])
                    .arg(format!("-Wl,--version-script={}", script.display()));
            }
            ObjectFormat::MachO => {
                command
                    .arg("-dynamiclib")
                    .arg(format!("-Wl,-install_name,{}", library.display()));
            }
            ObjectFormat::Coff => {
                command.arg("-shared");
            }
        }
        compile(
            command
                .arg(
                    Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("src/physical_tests/provider_references.c"),
                )
                .arg("-o")
                .arg(&library),
        );
        library
    }

    fn launch(&self) -> NativeProviderProcess {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "physical_tests::internal_native_provider_worker",
            "--nocapture",
        ]);
        #[cfg(windows)]
        {
            let mut paths = vec![self.directory.clone()];
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            command.env("PATH", std::env::join_paths(paths).unwrap());
        }
        NativeProviderProcess::launch(command, TIMEOUT).unwrap()
    }

    fn product(
        &self,
        library: &Path,
        symbol: &str,
        version: &[u8],
        parameters: Vec<ValueShape>,
        result: Option<ValueShape>,
    ) -> native_artifact::NativeForeignAdapterImage {
        let profile = TargetProfile::host();
        let target = profile.native_target();
        let candidate = match target.object_format {
            ObjectFormat::Elf => ForeignLocatorCandidate::ElfVersioned {
                object: library.to_str().unwrap().as_bytes().to_vec(),
                symbol: symbol.as_bytes().to_vec(),
                version: version.to_vec(),
            },
            ObjectFormat::MachO => ForeignLocatorCandidate::MachODylibSymbol {
                install_name: library.to_str().unwrap().as_bytes().to_vec(),
                symbol: format!("_{symbol}").into_bytes(),
            },
            ObjectFormat::Coff => ForeignLocatorCandidate::PeByName {
                library: library
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .as_bytes()
                    .to_vec(),
                export: symbol.as_bytes().to_vec(),
            },
        };
        let foreign = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::native_for_target(target),
            &CallSignature {
                parameters: parameters.clone(),
                result,
            },
        )
        .unwrap();
        let bridge = emit_native_foreign_bridge(NativeForeignBridgeRequest {
            target,
            foreign,
            scalar_parameters: vec![None; parameters.len()],
        })
        .unwrap();
        let locator = normalize_foreign_locator(candidate, profile).unwrap();
        let imports = image_fixtures::encode_locator_import(&locator);
        image_emission::emit_native_foreign_adapter(bridge, locator, [37; 32], imports)
        .unwrap()
    }

    fn install(
        &self,
        process: &mut NativeProviderProcess,
        symbol: &str,
        storage: NativeResultStorage,
    ) -> NativeAdapterId {
        let product = self.product(
            &self.library,
            symbol,
            b"REFERENCE_1.0",
            Vec::new(),
            Some(ValueShape::integer(8, 8)),
        );
        process.install_with_result(&product, &[], storage).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn read_cell(process: &mut NativeProviderProcess, reference: NativeProviderReference) -> i32 {
    let mut bytes = [0xa5; 8];
    process
        .read_provider_reference(reference, 0, &mut bytes[2..6])
        .unwrap();
    assert_eq!(&bytes[..2], &[0xa5; 2]);
    assert_eq!(&bytes[6..], &[0xa5; 2]);
    i32::from_le_bytes(bytes[2..6].try_into().unwrap())
}

#[test]
fn retained_provider_cells_keep_exact_occurrences_and_bounded_reads() {
    let fixture = Fixture::new();
    let mut process = fixture.launch();
    let cell = fixture.install(&mut process, "provider_reference_cell", CELL);
    let other = fixture.install(&mut process, "provider_reference_cell", CELL);
    let change = fixture.install(
        &mut process,
        "provider_reference_change",
        NativeResultStorage::Value,
    );
    let guards = fixture.install(
        &mut process,
        "provider_reference_guards",
        NativeResultStorage::Value,
    );
    assert!(process.call(cell, &[]).is_err());
    assert!(process.call_provider_reference(change, &[]).is_err());
    let first = process.call_provider_reference(cell, &[]).unwrap();
    assert_eq!(read_cell(&mut process, first), 37);
    for invalid in [
        NativeProviderReference {
            era: first.era.wrapping_add(1),
            ..first
        },
        NativeProviderReference {
            adapter: other.ordinal,
            ..first
        },
        NativeProviderReference {
            occurrence: 0,
            ..first
        },
        NativeProviderReference {
            occurrence: u64::MAX,
            ..first
        },
    ] {
        assert!(
            process
                .read_provider_reference(invalid, 0, &mut [0; 4])
                .is_err()
        );
        assert!(process.release_provider_reference(invalid).is_err());
    }
    for (offset, size) in [(0, 0), (0, 5), (4, 1), (usize::MAX, 1)] {
        assert!(
            process
                .read_provider_reference(first, offset, &mut vec![0; size])
                .is_err()
        );
    }
    let mut middle = [0; 2];
    process
        .read_provider_reference(first, 1, &mut middle)
        .unwrap();
    assert_eq!(middle, 37_i32.to_le_bytes()[1..3]);
    let second = process.call_provider_reference(cell, &[]).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        u64::from_le_bytes(process.call(change, &[]).unwrap().try_into().unwrap()),
        91
    );
    // Both occurrences still read the actual provider cell, not saved bytes.
    assert_eq!(read_cell(&mut process, first), 91);
    assert_eq!(read_cell(&mut process, second), 91);
    assert_eq!(
        u64::from_le_bytes(process.call(guards, &[]).unwrap().try_into().unwrap()),
        0x1357_9bdf_2468_ace0
    );
    process.release_provider_reference(first).unwrap();
    assert!(
        process
            .read_provider_reference(first, 0, &mut [0; 4])
            .is_err()
    );
    assert!(process.release_provider_reference(first).is_err());
    assert_eq!(read_cell(&mut process, second), 91);
    let third = process.call_provider_reference(cell, &[]).unwrap();
    assert!(third.occurrence > second.occurrence);
    process.release_provider_reference(second).unwrap();
    // Physical process retirement also ends a handle that was still live.
    process.close().unwrap();
    let mut next = fixture.launch();
    fixture.install(&mut next, "provider_reference_cell", CELL);
    let error = next
        .read_provider_reference(third, 0, &mut [0; 4])
        .unwrap_err();
    assert!(error.reason.contains("different incarnation"), "{error}");
    next.close().unwrap();
}

#[test]
fn invalid_provider_results_and_read_faults_remain_worker_local() {
    let fixture = Fixture::new();
    let mut survivor = fixture.launch();
    let cell = fixture.install(&mut survivor, "provider_reference_cell", CELL);
    let live = survivor.call_provider_reference(cell, &[]).unwrap();
    for symbol in ["provider_reference_null", "provider_reference_misaligned"] {
        let mut process = fixture.launch();
        let adapter = fixture.install(&mut process, symbol, CELL);
        let error = process.call_provider_reference(adapter, &[]).unwrap_err();
        assert!(
            error.reason.contains("null, misaligned or overflowing"),
            "{error}"
        );
        assert!(process.call_provider_reference(adapter, &[]).is_err());
        assert_eq!(read_cell(&mut survivor, live), 37);
    }
    let mut process = fixture.launch();
    let adapter = fixture.install(&mut process, "provider_reference_inaccessible", CELL);
    let inaccessible = process.call_provider_reference(adapter, &[]).unwrap();
    let mut bytes = [0xa5; 4];
    let error = process
        .read_provider_reference(inaccessible, 0, &mut bytes)
        .unwrap_err();
    let status = error
        .status
        .expect("the actual provider read fault stays observable");
    assert!(!status.success());
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert!(matches!(
            status.signal(),
            Some(libc::SIGSEGV | libc::SIGBUS)
        ));
    }
    #[cfg(windows)]
    assert_eq!(status.code().map(|value| value as u32), Some(0xc0000005));
    assert_eq!(bytes, [0xa5; 4]);
    assert!(
        process
            .read_provider_reference(inaccessible, 0, &mut bytes)
            .is_err()
    );
    assert!(process.release_provider_reference(inaccessible).is_err());
    assert_eq!(read_cell(&mut survivor, live), 37);
    survivor.release_provider_reference(live).unwrap();
    survivor.close().unwrap();
}

#[test]
fn worker_rejects_provider_reference_coordinates_ranges_and_framing() {
    let fixture = Fixture::new();
    for case in 0..9 {
        let mut process = fixture.launch();
        let adapter = fixture.install(&mut process, "provider_reference_cell", CELL);
        let other = fixture.install(&mut process, "provider_reference_cell", CELL);
        let handle = process.call_provider_reference(adapter, &[]).unwrap();
        let mut packet = Packet::new(if case == 6 { 9 } else { 8 });
        packet.number(if case == 0 {
            other.ordinal
        } else {
            adapter.ordinal
        });
        packet.number(match case {
            1 => 0,
            2 | 6 => handle.occurrence + 1,
            _ => handle.occurrence,
        });
        if case != 6 {
            packet.number(match case {
                3 => 4,
                4 => u64::MAX,
                _ => 0,
            });
            packet.number(if case == 5 { 0 } else { 4 });
        }
        if case == 7 {
            packet.0.pop();
        }
        if case == 8 {
            packet.byte(0);
        }
        let error = process.exchange(packet).unwrap_err();
        let expected = match case {
            0 | 2 => "provider read names no exact live result occurrence",
            1 => "provider reference occurrence exceeds its dense namespace",
            3..=5 => "provider reference read exceeds its admitted referent",
            6 => "provider release names no exact live result occurrence",
            7 => "worker message is truncated",
            8 => "worker message has trailing bytes",
            _ => unreachable!(),
        };
        assert_eq!(error.reason, expected, "case{case}: {error}");
        assert!(
            process
                .read_provider_reference(handle, 0, &mut [0; 4])
                .is_err()
        );
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn provider_errno_handle_reads_guest_tls_after_loader_housekeeping() {
    let fixture = Fixture::new();
    let another_library = fixture.compile_library("housekeeping");
    let mut process = fixture.launch();
    let system = Path::new(if cfg!(target_os = "macos") {
        "/usr/lib/libSystem.B.dylib"
    } else {
        "libc.so.6"
    });
    let version = if TargetProfile::host() == TargetProfile::LinuxArm64 {
        b"GLIBC_2.17".as_slice()
    } else {
        b"GLIBC_2.2.5".as_slice()
    };
    let pointer = fixture.product(
        system,
        if cfg!(target_os = "macos") {
            "__error"
        } else {
            "__errno_location"
        },
        version,
        Vec::new(),
        Some(ValueShape::integer(8, 8)),
    );
    let errno = process.install_with_result(&pointer, &[], CELL).unwrap();
    let reference = process.call_provider_reference(errno, &[]).unwrap();
    assert_eq!(read_cell(&mut process, reference), 0);
    let close = fixture.product(
        system,
        "close",
        version,
        vec![ValueShape::integer(4, 4)],
        Some(ValueShape::integer(4, 4)),
    );
    let close = process
        .install(&close, &[crate::NativeParameterStorage::Value])
        .unwrap();
    assert_eq!(
        i32::from_le_bytes(
            process
                .call(close, &[NativeArgument::Value(&(-1_i32).to_le_bytes())])
                .unwrap()
                .try_into()
                .unwrap()
        ),
        -1
    );
    let housekeeping = fixture.product(
        &another_library,
        "provider_reference_success_changes_errno",
        b"REFERENCE_1.0",
        Vec::new(),
        Some(ValueShape::integer(8, 8)),
    );
    let success = process.install(&housekeeping, &[]).unwrap();
    let home = process.create_home(4096, 8).unwrap();
    process.write_home(home, 0, &[91]).unwrap();
    assert_eq!(read_cell(&mut process, reference), libc::EBADF);
    assert_eq!(
        u64::from_le_bytes(process.call(success, &[]).unwrap().try_into().unwrap()),
        37
    );
    assert_eq!(read_cell(&mut process, reference), libc::ENOENT);
    process.release_provider_reference(reference).unwrap();
    process.close().unwrap();
}
