#[path = "../cargo_outputs.rs"]
mod cargo_outputs;

use cargo_outputs::CargoOutputs;
use std::ffi::OsStr;
use std::fs;

#[test]
fn unmarked_target_excludes_cargo_profiles_and_keeps_neighboring_source() {
    let directory = std::env::temp_dir().join(format!(
        "omega-compiler-identity-target-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    ));
    fs::create_dir(&directory).unwrap();
    let target = "example-host";
    let host_profile = directory.join("debug");
    let target_profile = directory.join(target).join("custom");
    for profile in [&host_profile, &target_profile] {
        fs::create_dir_all(profile.join("build")).unwrap();
        fs::write(profile.join(".cargo-lock"), b"").unwrap();
    }
    let source = directory.join("source.rs");
    fs::write(&source, "fn retained() {}").unwrap();
    let out = target_profile.join("build/compiler-identity/hash/out");
    let outputs = CargoOutputs::new(&out, None, Some(OsStr::new(target)));
    assert!(outputs.contains(&target_profile));
    assert!(outputs.contains(&host_profile));
    assert!(outputs.contains(&directory.join(".rustc_info.json")));
    assert!(!outputs.contains(&directory));
    assert!(!outputs.contains(&source));
    fs::remove_dir_all(directory).unwrap();
}
