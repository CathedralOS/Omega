//! Real C registrations cross the persistent worker's independently mapped
//! original homes; a real stale-token trap must fault only that incarnation.
use super::*;
use crate::{NativeArgument, NativeProviderProcess};
use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use machine_emission::native_foreign_bridge::{
    NativeForeignBridgeRequest, emit_native_foreign_bridge,
};
use target::{ForeignLocatorCandidate, ObjectFormat, TargetProfile, normalize_foreign_locator};

fn compile(command: &mut Command) {
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
    let fixture = manifest
        .ancestors()
        .nth(4)
        .unwrap()
        .join("samples/cli/facts/foreign_retained_shared_borrow/fixture");
    let profile = TargetProfile::host();
    let target = profile.native_target();
    let name = match target.object_format {
        ObjectFormat::MachO => "libomega_retention_fixture.dylib",
        ObjectFormat::Elf => "libomega_retention_fixture.so",
        ObjectFormat::Coff => "omega_retention_fixture.dll",
    };
    let library = directory.join(name);
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
            command.args(["-shared", "-fPIC"]).arg(format!(
                "-Wl,--version-script={}",
                fixture.join("provider.map").display()
            ));
        }
        ObjectFormat::Coff => {
            command.arg("-shared");
        }
    }
    compile(
        command
            .arg(fixture.join("provider.c"))
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
    let mut process = NativeProviderProcess::launch(command, TIMEOUT).unwrap();
    let word = ValueShape::integer(8, 8);
    let pointer = ValueShape::borrowed_reference(8, 8);
    let mut installed = Vec::new();
    let mut products = Vec::new();
    for (index, (function, parameters, result)) in [
        ("retention_acquire", vec![], Some(word)),
        ("retention_register", vec![word, pointer], Some(word)),
        ("retention_complete", vec![word, pointer], Some(word)),
        ("retention_release", vec![word], None),
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
        let bridge = emit_native_foreign_bridge(NativeForeignBridgeRequest {
            target,
            scalar_parameters: vec![None; parameters.len()],
            foreign: evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::native_for_target(target),
                &CallSignature { parameters, result },
            )
            .unwrap(),
        })
        .unwrap();
        let product = image_emission::emit_native_foreign_adapter(
            bridge,
            normalize_foreign_locator(candidate, profile).unwrap(),
            [index as u8 + 1; 32],
        )
        .unwrap();
        installed.push(process.install(&product).unwrap());
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
    // Following completion, update the same original home and re-register it.
    // Reusing a staged registration snapshot would return the old value here.
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
    #[cfg(windows)]
    {
        let mut paths = vec![directory.clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        command.env("PATH", std::env::join_paths(paths).unwrap());
    }
    let mut fresh = NativeProviderProcess::launch(command, TIMEOUT).unwrap();
    let acquire = fresh.install(&products[0]).unwrap();
    let release = fresh.install(&products[3]).unwrap();
    let home = fresh.create_home(8, 8).unwrap();
    assert_ne!(home, original);
    assert!(fresh.call(installed[0], &[]).is_err());
    assert!(fresh.read_home(original, 0, &mut bytes).is_err());
    let capacity = fresh.call(acquire, &[]).unwrap();
    fresh
        .call(release, &[NativeArgument::Value(&capacity)])
        .unwrap();
    fresh.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}
