//! A Windows program that writes to the console runs its Omega frames on the
//! loader's stack, and the image reserves 1 MiB of it. A local record holding
//! a 2 MB array used to compile and then die with STATUS_STACK_OVERFLOW. The
//! build now refuses it and names the call path, and a 512 KB record still
//! builds.
//!
//! The spec rule: `wiki/spec/build/entry_roots.md`, the WCSU paragraph
//! (insufficient fixed supply reports the responsible call path), with
//! `wiki/spec/resources/storage.md`, "Stack demand and backing".

#[path = "support/project.rs"]
mod project;
mod support;

/// Build `main` for windows_x86_64 with a local record of `bytes` bytes and
/// a console write, and return the exit status and everything printed.
fn build(name: &str, bytes: u32) -> (Option<i32>, String) {
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
    Build::bind(builder, windows_x86_64::ProgramEntry, |app|::main);
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
        &[
            "--target",
            "windows_x86_64",
            "--build-dir",
            "out",
            "build.omg",
        ],
    );
    let report = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("{report}");
    (output.status.code(), report)
}

#[test]
fn a_frame_larger_than_the_windows_loader_stack_is_refused() {
    let (status, report) = build("loader-stack-overflow", 2_000_000);
    assert_ne!(status, Some(0), "{report}");
    assert!(
        report.contains("stack frames on call path machine ")
            && report.contains("the windows_x86_64 loader stack supplies 1048576 bytes"),
        "{report}"
    );
}

#[test]
fn a_frame_within_the_windows_loader_stack_builds() {
    let (status, report) = build("loader-stack-fits", 512_000);
    assert_eq!(status, Some(0), "{report}");
    assert!(report.contains("published native output"), "{report}");
}
