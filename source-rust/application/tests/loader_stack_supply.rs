//! A program that writes to the console runs its Omega frames on the loader's
//! stack. On Windows a local record holding a 2 MB array used to compile and
//! then die with STATUS_STACK_OVERFLOW, because the image always reserved
//! 1 MiB. The image now reserves enough for the checked frames. macOS gives
//! the main thread a fixed 8 MiB, so a larger frame there is refused at build
//! time with its call path.
//!
//! The spec rule: `wiki/spec/build/entry_roots.md`, the WCSU paragraph
//! (insufficient fixed supply reports the responsible call path), with
//! `wiki/spec/resources/storage.md`, "Stack demand and backing".

#[path = "support/project.rs"]
mod project;
mod support;

/// Build `main` for `target` with a local record of `bytes` bytes and a
/// console write, and return the exit status, everything printed and the
/// staged project.
fn build(
    name: &str,
    target: &str,
    bytes: u32,
) -> (
    Option<i32>,
    String,
    platform_custody::temporary_directory::TemporaryDirectory,
) {
    let library = support::repository_root().join("source/library/system");
    let library = library.to_string_lossy().replace('\\', "/");
    let application = name.replace('-', "_");
    let build = format!(
        "use main.omg as app;
use {{omega}}/build/dependency.omg;
use {{omega}}/language/core/process_authority.omg as streams;
machine build(builder: @mutable BuildPlan) {{
    builder.product = BuildProduct::Application {{ name: \"{application}\" }};
    let allowed: [BoundaryServiceReference; 1] = [Build::service::<|streams|::StandardOutput>(builder)];
    let system: DependencyDescription = |dependency|::from_source(Source::Path {{ location: \"{library}\" }});
    _ = |dependency|::add(builder, system, @allowed[..]);
    Build::bind(builder, {target}::ProgramEntry, |app|::main);
}}
"
    );
    let main = format!(
        "use {{omega}}/language/core/process_authority.omg;
use {{system}}/console.omg;
data Buffers {{
    input: [u8; {bytes}];
    count: u64;
}}
public machine main() reaches |process_authority|::StandardOutput {{
    let mutable buffers: Buffers;
    buffers.count = 3u64;
    buffers.input[{last}] = 7u8;
    |console|::Console::write_line(\"ok\");
}}
",
        last = bytes - 1
    );
    let scratch = project::stage(name, &[("build.omg", &build), ("main.omg", &main)]);
    let output = project::omega(
        scratch.path(),
        &["--target", target, "--build-dir", "out", "build.omg"],
    );
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output.status.code(), report, scratch)
}

/// The PE32+ optional header's `SizeOfStackReserve`.
fn stack_reserve(image: &[u8]) -> u64 {
    let header = u32::from_le_bytes(image[0x3c..0x40].try_into().unwrap()) as usize;
    let field = header + 24 + 72;
    u64::from_le_bytes(image[field..field + 8].try_into().unwrap())
}

#[test]
fn a_windows_image_reserves_the_stack_its_frames_need() {
    for (bytes, reserve) in [(2_000_000, 2_097_152), (512_000, 1_048_576)] {
        let (status, report, scratch) = build("loader-stack-reserve", "windows_x86_64", bytes);
        assert_eq!(status, Some(0), "{report}");
        let image = std::fs::read(scratch.path().join("out/omega-program.exe")).unwrap();
        assert_eq!(stack_reserve(&image), reserve, "{report}");
    }
}

#[test]
fn a_frame_larger_than_the_macos_main_thread_stack_is_refused() {
    let (status, report, _) = build("loader-stack-overflow", "macos_arm64", 9_000_000);
    assert_ne!(status, Some(0), "{report}");
    assert!(
        report.contains("stack frames on call path machine ")
            && report.contains("the macos_arm64 loader stack supplies 8388608 bytes"),
        "{report}"
    );
}
