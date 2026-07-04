use std::path::PathBuf;

use omega_compiler::{CompileOptions, compile};
use omega_core::allocations::CountingAllocator;

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator::system();

fn main() {
    // `omega refresh-samples [samples-dir]`: compile every sample main.omg
    // under samples/, in place, in parallel, so each sample folder holds a
    // current, runnable build/omega-program.exe. Cross-platform (no shell
    // script) and links this binary's own compiler -- though the binary itself
    // still needs a rebuild after compiler changes (apps/omega-cli is its own
    // workspace).
    let mut raw_arguments = std::env::args_os().skip(1);
    if raw_arguments
        .next()
        .is_some_and(|first| first == "refresh-samples")
    {
        let samples_root = raw_arguments
            .next()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("samples"));
        refresh_samples(&samples_root);
    }

    let Some(arguments) = parse_arguments() else {
        eprintln!(
            "usage: omega [--check] [--build-dir <dir>] [--target <name>] <root.omg>\n       omega refresh-samples [samples-dir]"
        );
        std::process::exit(2);
    };

    let options = CompileOptions {
        build_dir: arguments.build_dir,
        root_path: arguments.root_path,
        target_name: arguments.target_name,
        write_output: !arguments.check_only,
    };

    match compile(options) {
        Ok(report) => {
            println!("{}", report.summary());
        }
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }

            std::process::exit(1);
        }
    };
}

/// Compile every sample `main.omg` under `<samples_root>` into its own `build/` directory,
/// fanned across the machine's cores (each sample owns a distinct build dir, so
/// the parallel compiles never collide). Prints a summary and exits.
fn refresh_samples(samples_root: &std::path::Path) -> ! {
    let mut mains = Vec::new();
    if let Err(error) = collect_sample_mains(samples_root, &mut mains) {
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
                    match compile(CompileOptions {
                        root_path: main_path.clone(),
                        build_dir: Some(build_dir),
                        target_name: None,
                        write_output: true,
                    }) {
                        Ok(_) => {
                            built.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        }
                        Err(diagnostics) => {
                            let first = diagnostics
                                .first()
                                .map(|diagnostic| diagnostic.to_string())
                                .unwrap_or_else(|| "unknown error".to_owned());
                            failures
                                .lock()
                                .unwrap()
                                .push(format!("{}: {first}", main_path.display()));
                        }
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

fn collect_sample_mains(
    directory: &std::path::Path,
    mains: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    if directory.join("main.omg").is_file() {
        mains.push(directory.join("main.omg"));
        return Ok(());
    }

    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|name| name == "build") {
            continue;
        }
        collect_sample_mains(&path, mains)?;
    }

    Ok(())
}

struct CliArguments {
    build_dir: Option<PathBuf>,
    check_only: bool,
    root_path: PathBuf,
    target_name: Option<String>,
}

fn parse_arguments() -> Option<CliArguments> {
    let mut build_dir = None;
    let mut check_only = false;
    let mut root_path = None;
    let mut target_name = None;
    let mut arguments = std::env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--check" {
            check_only = true;
            continue;
        }

        if argument == "--build-dir" {
            build_dir = arguments.next().map(PathBuf::from);
            build_dir.as_ref()?;
            continue;
        }

        if argument == "--target" {
            target_name = arguments
                .next()
                .and_then(|target_name| target_name.into_string().ok());
            target_name.as_ref()?;
            continue;
        }

        if root_path.is_some() {
            return None;
        }

        root_path = Some(PathBuf::from(argument));
    }

    Some(CliArguments {
        build_dir,
        check_only,
        root_path: root_path?,
        target_name,
    })
}
