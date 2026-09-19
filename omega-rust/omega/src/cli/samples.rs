use compiler::CompileOptions;
use omega::compilation::{CompileProjectRequest, compile_project};
use std::path::{Path, PathBuf};
use target::TargetProfile;

/// Compile every sample `main.omg` under `samples_root` into its own build
/// directory for the exact host target. Each worker owns a distinct output.
pub(crate) fn refresh(samples_root: &Path) -> ! {
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
    // Refresh compiles each sample for the compiler host's exact deployment
    // profile; a host with no catalogued profile cannot produce them, so it
    // reports the absence instead of panicking inside `TargetProfile::host`.
    let Some(host) = TargetProfile::host_if_supported() else {
        eprintln!(
            "refresh-samples builds every sample for the compiler host's deployment \
             profile; this host has no catalogued Omega deployment profile"
        );
        std::process::exit(2);
    };
    let target_name = host.target_name().to_owned();

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
                    let options = CompileOptions {
                        root_path: main_path.clone(),
                        build_dir: Some(build_dir),
                        target_name: Some(target_name.clone()),
                    };
                    // Samples use the ordinary package/build, trust-admission,
                    // and native-publication route. In particular, acquisition
                    // alone cannot supply a dependency's generated source, and
                    // refreshing a sample never accepts trust on its behalf.
                    let mut request = CompileProjectRequest::new(options);
                    request.require_package_project = true;
                    match compile_project(request) {
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
