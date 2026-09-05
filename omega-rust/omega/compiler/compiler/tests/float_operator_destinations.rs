use compiler::compile_to_checked;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-float-destinations-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).expect("create test project");
        Self(path)
    }

    fn write(&self, source: &str) -> PathBuf {
        let path = self.0.join("main.omg");
        fs::write(&path, source).expect("write test source");
        path
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test project");
    }
}

#[test]
fn compiler_rejects_binary_float_argument_format_changes() {
    let project = Project::new();
    for (source, destination) in [("f64", "f32"), ("f32", "f64")] {
        let path = project.write(&format!(
            "machine take(value: {destination}) {{}} machine run() {{ take(1.0{source} + 2.0{source}); }}"
        ));
        let diagnostics = match compile_to_checked(&path, None) {
            Err(diagnostics) => diagnostics,
            Ok(_) => panic!("incompatible argument must reject"),
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("explicit conversion")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn compiler_accepts_matching_binary_float_arguments() {
    let project = Project::new();
    for format in ["f32", "f64"] {
        let path = project.write(&format!(
            "machine take(value: {format}) {{}} machine run() {{ take(1.0{format} + 2.0{format}); }}"
        ));
        compile_to_checked(&path, None).expect("matching argument must check");
    }
}
