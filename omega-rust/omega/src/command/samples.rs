use super::output;
use compiler::{
    ArtifactEmissionPolicy, CompileOptions, CompileRequest, RequestedCompileProduct, compile,
};
use std::path::{Path, PathBuf};

/// Compile every sample `main.omg` under `samples_root` into its own build
/// directory for the exact host target. Each worker owns a distinct output.
pub(super) fn refresh(samples_root: &Path) -> ! {
    let mut mains = Vec::new();
    if let Err(error) = collect_mains(samples_root, &mut mains) {
        eprintln!(
            "cannot read samples dir {}: {error}",
            samples_root.display()
        );
        std::process::exit(2);
    }
    mains.sort();
    let total = mains.len();
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .min(total.max(1));
    let target_name = target::TargetProfile::host().target_name().to_owned();

    let queue = std::sync::Mutex::new(mains);
    let failures = std::sync::Mutex::new(Vec::<String>::new());
    let built = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let Some(main_path) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    let build_dir = main_path
                        .parent()
                        .expect("main.omg has a sample directory")
                        .join("build");
                    let prepared =
                        match package_manager::operations::prepare_local_project(&main_path) {
                            Ok(Some(prepared)) => prepared,
                            Ok(None) => {
                                failures.lock().unwrap().push(format!(
                                    "{}: sample project has no sibling build.omg",
                                    main_path.display()
                                ));
                                continue;
                            }
                            Err(error) => {
                                failures
                                    .lock()
                                    .unwrap()
                                    .push(format!("{}: {error}", main_path.display()));
                                continue;
                            }
                        };
                    let (prepared_entry, package_inputs) = prepared.into_parts();
                    let options = CompileOptions {
                        root_path: prepared_entry,
                        build_dir: Some(build_dir.clone()),
                        target_name: Some(target_name.clone()),
                    };
                    let request = CompileRequest::new(options)
                        .with_package_inputs(package_inputs)
                        .with_requested_product(RequestedCompileProduct::NativeArtifact)
                        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly);
                    let result = match compile(request) {
                        Ok(report) => output::publish_native_artifact(report, &build_dir),
                        Err(diagnostics) => Err(diagnostics
                            .first()
                            .map(ToString::to_string)
                            .unwrap_or_else(|| "unknown compilation error".to_owned())),
                    };
                    match result {
                        Ok(_) => {
                            built.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        Err(message) => failures
                            .lock()
                            .unwrap()
                            .push(format!("{}: {message}", main_path.display())),
                    }
                }
            });
        }
    });

    let failures = failures.into_inner().unwrap();
    println!(
        "{} of {total} samples built across {workers} threads",
        built.load(std::sync::atomic::Ordering::SeqCst)
    );
    if failures.is_empty() {
        std::process::exit(0);
    }
    for failure in &failures {
        eprintln!("FAILED {failure}");
    }
    std::process::exit(1);
}

fn collect_mains(directory: &Path, mains: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if directory.join("main.omg").is_file() {
        mains.push(directory.join("main.omg"));
        return Ok(());
    }

    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if !path.is_dir() || path.file_name().is_some_and(|name| name == "build") {
            continue;
        }
        collect_mains(&path, mains)?;
    }
    Ok(())
}
