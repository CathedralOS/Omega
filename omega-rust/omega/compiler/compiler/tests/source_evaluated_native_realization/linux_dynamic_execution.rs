//! Native execution of a source-produced normalized foreign call on the
//! linux_x86_64 host: the emitted dynamic ELF must load through the system
//! interpreter and drive its libc import for real.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::os::unix::fs::PermissionsExt;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::process::Command;

use super::{Fixture, realize_linux_dynamic};
use native_realization as native;

#[test]
fn boundary_requirement_executes_its_foreign_call_on_linux_x64() {
    let fixture = Fixture::new_linux_boundary_requirement_named("linux-dynamic-exec");
    let artifact = realize_linux_dynamic(fixture.compile_terminal(), 0x6578_6563_0001);
    let native::RequestedNativeArtifact::DynamicElf(candidate) = artifact else {
        panic!("the boundary requirement must realize a dynamic ELF candidate")
    };
    candidate
        .validate()
        .expect("dynamic native candidate independently replays");
    assert_eq!(candidate.target(), target::NativeTarget::linux_x64());
    assert_eq!(candidate.image().output().final_image_imports, 1);
    let image_bytes = candidate.image().output().bytes.clone();
    assert!(image_bytes.starts_with(b"\x7fELF"));

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        // Write the emitted image and execute it unwrapped: the dynamic loader
        // resolves the versioned libc import, so the observed exit status is
        // the foreign call's own transported argument.
        let path = fixture.root.join("boundary-requirement");
        std::fs::write(&path, &image_bytes).expect("write emitted ELF image");
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&path, permissions).unwrap();
        let output = Command::new(&path)
            .output()
            .expect("the emitted ELF must load on the linux_x86_64 host");
        assert!(
            output.status.code() == Some(70),
            "foreign exit call must report its argument, got {:?} stdout {:?} stderr {:?}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
