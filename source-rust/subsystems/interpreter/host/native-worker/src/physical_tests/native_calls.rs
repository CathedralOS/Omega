/*
We test real foreign C registrations across the persistent worker's
independently mapped original homes, verifying that updating an original home
after completion is observed on subsequent registrations, that a stale-token
trap faults only that incarnation without replay, and that POSIX guest `errno`
survives worker IPC and dynamic-library constructor housekeeping.
*/

use super::*;
use crate::NativeArgument;
use crate::native_provider_runtime::test_session::TestProviderProcess;
use calling_conventions::{
    CallSignature, CallingPolicy, NativeForeignBridgeRequest, ValueShape,
    evaluate_ordinary_boundary_entry_plan,
};
use native_compiler::test_support::compile_runtime_foreign_adapter;
use target::{ForeignLocatorCandidate, ObjectFormat, TargetProfile, normalize_foreign_locator};

pub(super) fn compile(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {:?}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn real_provider_retains_original_mapped_roots_and_faults_without_replay() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omega-worker-retention-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let provider = manifest
        .ancestors()
        .map(|dir| dir.join("samples/cli/facts/foreign_retained_shared_borrow/fixture/provider.c"))
        .find(|path| path.is_file())
        .expect("shared retention provider fixture");
    let profile = TargetProfile::host();
    let target = profile.native_target();
    let name = match target.object_format {
        ObjectFormat::MachO => "libomega_retention_fixture.dylib",
        ObjectFormat::Elf => "libomega_retention_fixture.so",
        ObjectFormat::Coff => "omega_retention_fixture.dll",
    };
    let library = directory.join(name);
    let version_script = directory.join("provider.map");
    std::fs::write(
        &version_script,
        "RETAIN_1.0 { global: retention_*; local: *; };\n",
    )
    .unwrap();
    let compiler = std::env::var_os("CC").unwrap_or_else(|| {
        if cfg!(windows) {
            "clang".into()
        } else {
            "cc".into()
        }
    });
    let mut command = Command::new(compiler);
    command.args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]);
    match target.object_format {
        ObjectFormat::MachO => {
            command
                .arg("-dynamiclib")
                .arg(format!("-Wl,-install_name,{}", library.display()));
        }
        ObjectFormat::Elf => {
            command
                .args(["-shared", "-fPIC"])
                .arg(format!("-Wl,--version-script={}", version_script.display()));
        }
        ObjectFormat::Coff => {
            command.arg("-shared");
        }
    }
    compile(
        command
            .arg(provider)
            .arg(manifest.join("src/physical_tests/process_exit.c"))
            .arg("-o")
            .arg(&library),
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "--exact",
        "physical_tests::internal_native_provider_worker",
        "--nocapture",
    ]);
    #[cfg(windows)]
    {
        let mut paths = vec![directory.clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        command.env("PATH", std::env::join_paths(paths).unwrap());
    }
    let mut process = TestProviderProcess::launch(command, TIMEOUT).unwrap();
    let word = ValueShape::integer(8, 8);
    let pointer = ValueShape::borrowed_reference(8, 8);
    let mut installed = Vec::new();
    let mut products = Vec::new();
    for (index, (function, parameters, result)) in [
        ("retention_acquire", vec![], Some(word)),
        ("retention_register", vec![word, pointer], Some(word)),
        ("retention_complete", vec![word, pointer], Some(word)),
        ("retention_release", vec![word], None),
        ("retention_exit_register", vec![pointer], None),
    ]
    .into_iter()
    .enumerate()
    {
        let candidate = match target.object_format {
            ObjectFormat::MachO => ForeignLocatorCandidate::MachODylibSymbol {
                install_name: library.to_str().unwrap().as_bytes().to_vec(),
                symbol: format!("_{function}").into_bytes(),
            },
            ObjectFormat::Elf => ForeignLocatorCandidate::ElfVersioned {
                object: library.to_str().unwrap().as_bytes().to_vec(),
                symbol: function.as_bytes().to_vec(),
                version: b"RETAIN_1.0".to_vec(),
            },
            ObjectFormat::Coff => ForeignLocatorCandidate::PeByName {
                library: name.as_bytes().to_vec(),
                export: function.as_bytes().to_vec(),
            },
        };
        let request = NativeForeignBridgeRequest {
            target,
            scalar_parameters: vec![None; parameters.len()],
            foreign: evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature { parameters, result },
            )
            .unwrap(),
        };
        let locator = normalize_foreign_locator(candidate, profile).unwrap();
        let product = image_emission::emit_native_foreign_adapter(
            compile_runtime_foreign_adapter(request, locator, [index as u8 + 1; 32]).unwrap(),
        )
        .unwrap();
        installed.push(
            process
                .install(&product, &parameter_storage(&product))
                .unwrap(),
        );
        products.push(product);
    }
    let original = process.create_home(8, 8).unwrap();
    let observed = process.create_home(8, 8).unwrap();
    process
        .write_home(original, 0, &37_u64.to_le_bytes())
        .unwrap();
    let capacity = process.call(installed[0], &[]).unwrap();
    let pending = process
        .call(
            installed[1],
            &[
                NativeArgument::Value(&capacity),
                NativeArgument::Borrowed {
                    home: original,
                    offset: 0,
                },
            ],
        )
        .unwrap();
    let returned = process
        .call(
            installed[2],
            &[
                NativeArgument::Value(&pending),
                NativeArgument::Borrowed {
                    home: observed,
                    offset: 0,
                },
            ],
        )
        .unwrap();
    let mut bytes = [0; 8];
    process.read_home(observed, 0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 37);
    assert_eq!(capacity, returned);
    process
        .write_home(original, 0, &91_u64.to_le_bytes())
        .unwrap();
    let next = process
        .call(
            installed[1],
            &[
                NativeArgument::Value(&returned),
                NativeArgument::Borrowed {
                    home: original,
                    offset: 0,
                },
            ],
        )
        .unwrap();
    assert_ne!(pending, next);
    let returned = process
        .call(
            installed[2],
            &[
                NativeArgument::Value(&next),
                NativeArgument::Borrowed {
                    home: observed,
                    offset: 0,
                },
            ],
        )
        .unwrap();
    process.read_home(observed, 0, &mut bytes).unwrap();
    assert_eq!(u64::from_le_bytes(bytes), 91);
    assert_eq!(capacity, returned);
    assert!(
        process
            .call(installed[3], &[NativeArgument::Value(&returned)])
            .unwrap()
            .is_empty()
    );
    let trap = process
        .call(
            installed[2],
            &[
                NativeArgument::Value(&pending),
                NativeArgument::Borrowed {
                    home: observed,
                    offset: 0,
                },
            ],
        )
        .unwrap_err();
    let status = trap
        .status
        .expect("the original C trap must remain observable");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            status.signal(),
            Some(match target.architecture {
                target::Architecture::Aarch64 => 5,
                target::Architecture::X86_64 => 4,
            })
        );
    }
    #[cfg(windows)]
    assert_eq!(status.code().map(|code| code as u32), Some(0xc000001d));
    assert!(process.call(installed[0], &[]).is_err());
    assert!(process.read_home(original, 0, &mut bytes).is_err());
    drop(process);
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "--exact",
        "physical_tests::internal_native_provider_worker",
        "--nocapture",
    ]);
    let exit_witness = directory.join("exit-value.bin");
    command.env("OMEGA_NATIVE_EXIT_WITNESS", &exit_witness);
    #[cfg(windows)]
    {
        let mut paths = vec![directory.clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        command.env("PATH", std::env::join_paths(paths).unwrap());
    }
    let mut fresh = TestProviderProcess::launch(command, TIMEOUT).unwrap();
    let acquire = fresh
        .install(&products[0], &parameter_storage(&products[0]))
        .unwrap();
    let release = fresh
        .install(&products[3], &parameter_storage(&products[3]))
        .unwrap();
    let retain_until_exit = fresh
        .install(&products[4], &parameter_storage(&products[4]))
        .unwrap();
    let home = fresh.create_home(8, 8).unwrap();
    assert_ne!(home, original);
    assert!(fresh.call(installed[0], &[]).is_err());
    assert!(fresh.read_home(original, 0, &mut bytes).is_err());
    let capacity = fresh.call(acquire, &[]).unwrap();
    fresh
        .call(release, &[NativeArgument::Value(&capacity)])
        .unwrap();
    fresh.write_home(home, 0, &91_u64.to_le_bytes()).unwrap();
    fresh
        .call(
            retain_until_exit,
            &[NativeArgument::Borrowed { home, offset: 0 }],
        )
        .unwrap();
    fresh.close().unwrap();
    assert_eq!(std::fs::read(exit_witness).unwrap(), 91_u64.to_le_bytes());
    std::fs::remove_dir_all(directory).unwrap();
}

fn parameter_storage(
    product: &image_emission::NativeForeignAdapterImage,
) -> Vec<crate::NativeParameterStorage> {
    product
        .bridge()
        .request()
        .foreign
        .plan()
        .call
        .parameters
        .iter()
        .map(|parameter| {
            if parameter.shape.class == calling_conventions::ValueClass::BorrowedReference {
                crate::NativeParameterStorage::OriginalReference {
                    byte_size: parameter.shape.byte_size as usize,
                    alignment: usize::from(parameter.shape.alignment),
                }
            } else {
                crate::NativeParameterStorage::Value
            }
        })
        .collect()
}

#[cfg(target_os = "macos")]
#[test]
fn darwin_guest_errno_survives_worker_housekeeping() {
    guest_errno_survives_worker_housekeeping();
}

#[cfg(target_os = "linux")]
#[test]
fn linux_guest_errno_survives_worker_housekeeping() {
    guest_errno_survives_worker_housekeeping();
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn guest_errno_survives_worker_housekeeping() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("omega-worker-errno-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/physical_tests/errno.c");
    let profile = TargetProfile::host();
    let target = profile.native_target();
    let version_script = directory.join("errno.map");
    std::fs::write(
        &version_script,
        "ERRNO_1.0 { global: errno_*; local: *; };\n",
    )
    .unwrap();
    let mut libraries = Vec::new();
    for image_ordinal in 0..2 {
        let extension = match target.object_format {
            ObjectFormat::MachO => "dylib",
            ObjectFormat::Elf => "so",
            ObjectFormat::Coff => unreachable!("errno control targets POSIX hosts"),
        };
        let library = directory.join(format!("liberrno-{image_ordinal}.{extension}"));
        let mut command = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()));
        command.args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]);
        match target.object_format {
            ObjectFormat::MachO => {
                command
                    .arg("-dynamiclib")
                    .arg(format!("-Wl,-install_name,{}", library.display()));
            }
            ObjectFormat::Elf => {
                command
                    .args(["-shared", "-fPIC"])
                    .arg(format!("-Wl,--version-script={}", version_script.display()));
            }
            ObjectFormat::Coff => unreachable!("errno control targets POSIX hosts"),
        }
        compile(command.arg(&fixture).arg("-o").arg(&library));
        libraries.push(library);
    }
    let system = match target.object_format {
        ObjectFormat::MachO => "/usr/lib/libSystem.B.dylib",
        ObjectFormat::Elf => "libc.so.6",
        ObjectFormat::Coff => unreachable!("errno control targets POSIX hosts"),
    };
    let product = |library: &str, symbol: &str, parameters: Vec<ValueShape>, result, tag| {
        let request = NativeForeignBridgeRequest {
            target,
            scalar_parameters: vec![None; parameters.len()],
            foreign: evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature { parameters, result },
            )
            .unwrap(),
        };
        let candidate = match target.object_format {
            ObjectFormat::MachO => ForeignLocatorCandidate::MachODylibSymbol {
                install_name: library.as_bytes().to_vec(),
                symbol: format!("_{symbol}").into_bytes(),
            },
            ObjectFormat::Elf => ForeignLocatorCandidate::ElfVersioned {
                object: library.as_bytes().to_vec(),
                symbol: symbol.as_bytes().to_vec(),
                version: if library == system {
                    if profile == TargetProfile::LinuxArm64 {
                        b"GLIBC_2.17".to_vec()
                    } else {
                        b"GLIBC_2.2.5".to_vec()
                    }
                } else {
                    b"ERRNO_1.0".to_vec()
                },
            },
            ObjectFormat::Coff => unreachable!("errno control targets POSIX hosts"),
        };
        let locator = normalize_foreign_locator(candidate, profile).unwrap();
        image_emission::emit_native_foreign_adapter(
            compile_runtime_foreign_adapter(request, locator, [tag; 32]).unwrap(),
        )
        .unwrap()
    };
    let word = ValueShape::integer(8, 8);
    let integer = ValueShape::integer(4, 4);
    let errno_symbol = if cfg!(target_os = "macos") {
        "__error"
    } else {
        "__errno_location"
    };
    let products = [
        product(system, "close", vec![integer], Some(integer), 1),
        product(system, errno_symbol, vec![], Some(word), 2),
        product(
            system,
            "memcpy",
            vec![ValueShape::borrowed_reference(4, 4), word, word],
            Some(word),
            3,
        ),
        product(system, "getpid", vec![], Some(integer), 4),
        product(
            libraries[0].to_str().unwrap(),
            "errno_success_changes",
            vec![],
            Some(word),
            5,
        ),
        product(
            libraries[1].to_str().unwrap(),
            "errno_success_changes",
            vec![],
            Some(word),
            6,
        ),
    ];
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args([
        "--exact",
        "physical_tests::internal_native_provider_worker",
        "--nocapture",
    ]);
    let mut process = TestProviderProcess::launch(command, TIMEOUT).unwrap();
    let mut installed = Vec::new();
    for adapter in &products[..5] {
        installed.push(
            process
                .install(adapter, &parameter_storage(adapter))
                .unwrap(),
        );
    }
    let view_product = product(
        libraries[0].to_str().unwrap(),
        "errno_copy_view",
        vec![ValueShape::integer(16, 8)],
        Some(word),
        7,
    );
    let view_adapter = process
        .install(
            &view_product,
            &[crate::NativeParameterStorage::OriginalByteView],
        )
        .unwrap();
    let view_home = process.create_home(8, 1).unwrap();
    process
        .write_home(view_home, 0, &[23, 23, 0, 0, 0, 0, 37, 37])
        .unwrap();
    let observed = process.create_home(8, 4).unwrap();
    process
        .write_home(observed, 4, &0x1234_5678_u32.to_le_bytes())
        .unwrap();
    let errno_pointer = process.call(installed[1], &[]).unwrap();
    assert_ne!(
        u64::from_le_bytes(errno_pointer.clone().try_into().unwrap()),
        0
    );
    let copy_errno = |process: &mut TestProviderProcess, expected: libc::c_int| {
        process
            .call(
                installed[2],
                &[
                    NativeArgument::Borrowed {
                        home: observed,
                        offset: 0,
                    },
                    NativeArgument::Value(&errno_pointer),
                    NativeArgument::Value(&4_u64.to_le_bytes()),
                ],
            )
            .unwrap();
        let mut bytes = [0; 8];
        process.read_home(observed, 0, &mut bytes).unwrap();
        assert_eq!(i32::from_le_bytes(bytes[..4].try_into().unwrap()), expected);
        assert_eq!(
            u32::from_le_bytes(bytes[4..].try_into().unwrap()),
            0x1234_5678
        );
        let copied = process
            .call(
                view_adapter,
                &[NativeArgument::ByteView {
                    home: view_home,
                    offset: 2,
                    length: 4,
                }],
            )
            .unwrap();
        assert_eq!(u64::from_le_bytes(copied.try_into().unwrap()), 4);
        process.read_home(view_home, 0, &mut bytes).unwrap();
        assert_eq!(&bytes[..2], &[23, 23]);
        assert_eq!(
            i32::from_le_bytes(bytes[2..6].try_into().unwrap()),
            expected
        );
        assert_eq!(&bytes[6..], &[37, 37]);
    };
    copy_errno(&mut process, 0);
    let failed = process
        .call(
            installed[0],
            &[NativeArgument::Value(&(-1_i32).to_le_bytes())],
        )
        .unwrap();
    assert_eq!(i32::from_le_bytes(failed.try_into().unwrap()), -1);
    process
        .install(&products[5], &parameter_storage(&products[5]))
        .unwrap();
    let another_home = process.create_home(4096, 8).unwrap();
    process.write_home(another_home, 0, &[91]).unwrap();
    assert_eq!(process.call(installed[1], &[]).unwrap(), errno_pointer);
    copy_errno(&mut process, libc::EBADF);
    let pid = process.call(installed[3], &[]).unwrap();
    assert!(i32::from_le_bytes(pid.try_into().unwrap()) > 0);
    copy_errno(&mut process, libc::EBADF);
    let successful = process.call(installed[4], &[]).unwrap();
    assert_eq!(u64::from_le_bytes(successful.try_into().unwrap()), 37);
    copy_errno(&mut process, libc::ENOENT);
    process.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
