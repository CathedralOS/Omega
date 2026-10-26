//! The emitted shared library must survive the actual platform loader and call
//! the original C provider. The C driver knows only the fixed two-pointer ABI.

use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use machine_emission::native_foreign_bridge::{
    NativeForeignBridgeRequest, emit_native_foreign_bridge,
};
use std::{path::PathBuf, process::Command};
use target::{ForeignLocatorCandidate, ObjectFormat, TargetProfile, normalize_foreign_locator};

fn successful(command: &mut Command) {
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{command:?}: {:?}\n{}\n{}",
        result.status,
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn emitted_adapters_load_and_share_the_real_provider_registration() {
    let profile = TargetProfile::host();
    let target = profile.native_target();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "omega-foreign-image-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture = repository
        .ancestors()
        .nth(4)
        .unwrap()
        .join("samples/cli/facts/foreign_retained_shared_borrow/fixture");
    let compiler = std::env::var_os("CC").unwrap_or_else(|| {
        if cfg!(windows) {
            "clang".into()
        } else {
            "cc".into()
        }
    });
    let (library, extension) = match target.object_format {
        ObjectFormat::MachO => (directory.join("libomega_retention_fixture.dylib"), "dylib"),
        ObjectFormat::Elf => (directory.join("libomega_retention_fixture.so"), "so"),
        ObjectFormat::Coff => (directory.join("omega_retention_fixture.dll"), "dll"),
    };
    let mut compile = Command::new(&compiler);
    compile.args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]);
    match target.object_format {
        ObjectFormat::MachO => {
            compile
                .arg("-dynamiclib")
                .arg(format!("-Wl,-install_name,{}", library.display()));
        }
        ObjectFormat::Elf => {
            compile.args(["-shared", "-fPIC"]).arg(format!(
                "-Wl,--version-script={}",
                fixture.join("provider.map").display()
            ));
        }
        ObjectFormat::Coff => {
            compile.arg("-shared");
        }
    }
    successful(
        compile
            .arg(fixture.join("provider.c"))
            .arg("-o")
            .arg(&library),
    );
    let executable = directory.join(if cfg!(windows) {
        "driver.exe"
    } else {
        "driver"
    });
    let driver = directory.join("driver.c");
    std::fs::write(&driver, include_str!("foreign_adapter.c")).unwrap();
    let mut compile = Command::new(&compiler);
    compile
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(driver)
        .arg("-o")
        .arg(&executable);
    if target.object_format == ObjectFormat::Elf {
        compile.arg("-ldl");
    }
    successful(&mut compile);
    let word = ValueShape::integer(8, 8);
    let pointer = ValueShape::borrowed_reference(8, 8);
    let mut run = Command::new(executable);
    run.current_dir(&directory);
    for (index, (function, parameters, result)) in [
        ("retention_acquire", vec![], Some(word)),
        ("retention_register", vec![word, pointer], Some(word)),
        ("retention_complete", vec![word, pointer], Some(word)),
        ("retention_release", vec![word], None),
    ]
    .into_iter()
    .enumerate()
    {
        let locator = match target.object_format {
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
                library: library
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .as_bytes()
                    .to_vec(),
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
        let adapter = image_emission::emit_native_foreign_adapter(
            bridge,
            normalize_foreign_locator(locator, profile).unwrap(),
            [index as u8 + 1; 32],
        )
        .unwrap();
        let path = directory.join(format!("adapter-{index}.{extension}"));
        std::fs::write(&path, adapter.container().contents()).unwrap();
        run.arg(path).arg(adapter.export());
    }
    successful(&mut run);
    let output = run
        .env("OMEGA_FOREIGN_REPEAT_COMPLETION", "1")
        .output()
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        let signal = match target.architecture {
            target::Architecture::Aarch64 => 5, // C __builtin_trap: BRK / SIGTRAP
            target::Architecture::X86_64 => 4,  // UD2 / SIGILL
        };
        assert_eq!(
            output.status.signal(),
            Some(signal),
            "actual C lifecycle trap: {output:?}"
        );
    }
    #[cfg(windows)]
    assert_eq!(
        output.status.code().map(|code| code as u32),
        Some(0xc000001d),
        "actual C illegal-instruction trap: {output:?}"
    );
    std::fs::remove_dir_all(directory).unwrap();
}
