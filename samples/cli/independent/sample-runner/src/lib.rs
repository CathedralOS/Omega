//! Capture actual numerical native and Terminal output from the same product.
//! This uses normal package preparation and preserves its admission checks.
//! Review modes use this exact executable identity; run never accepts decisions.
//!
//! Every independent sample runs the identical product: package preparation,
//! the retained Terminal artifact, the published native product and the
//! per-input captures. Only the sample's own identity, whether it has reviewed
//! package decisions to refresh, and the evidence layout its validation reads
//! back differ, so each sample declares one [`Sample`] roster and shares this
//! harness. The numerical oracle, its frozen expected output and its tolerance
//! stay with the sample that owns them; this harness captures and never
//! compares.
use omega_build::{
    BuildRequest, BuildSession, BuildTargets, NativeDestination, ProjectProduct,
    execute_package_command,
};
use package_manager::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageCommandStatus,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionStatus, TerminalRuntimeRequest,
};

/// The evidence layout a sample's `validate.py` reads back.
///
/// Captured bytes are the same in every layout; only where they land and which
/// attestations accompany them differ, because each validation reads its own
/// already-frozen one.
#[derive(Clone, Copy)]
pub enum Evidence {
    /// `terminal.stdout`, `terminal.stderr` and `terminal.status` beside every
    /// named input, then the published product's `native.*` beside it too.
    Adjacent,
    /// One directory holding the `--terminal` bundle: the retained
    /// `application.terminal` and its digest, this producer's digest, the
    /// published native executable with its digest and path, a `sources.tsv`
    /// of the declared sources, and a `case exit input stdout stderr` row in
    /// `records.tsv` beside each case's captured stdout and stderr.
    Terminal {
        /// Sample-relative sources that `sources.tsv` attests by digest.
        sources: &'static [&'static str],
    },
    /// One directory holding the `--captured` bundle: a `case exit` row in
    /// `records.tsv` beside each case's captured stdout and stderr.
    Captured,
    /// One directory holding the numbered bundle: a headerless `case exit` row
    /// in `records.tsv` beside each case's `NN.stdout` and `NN.stderr`, where
    /// `NN` is the case's position in the declared input directory rather than
    /// its file name. `validate.py --route terminal` reads this.
    Numbered,
    /// One directory holding the named bundle: a `NAME.stdout`, `NAME.stderr`
    /// and `NAME.exit` for every case a names file lists, one per line.
    /// `validate.py --captures` reads this.
    Named,
}

impl Evidence {
    /// Whether captures land as one bundle in the output directory rather than
    /// beside each named input.
    const fn bundled(self) -> bool {
        !matches!(self, Self::Adjacent)
    }

    /// Whether the bundle tabulates its cases in one `records.tsv`.
    const fn tabulated(self) -> bool {
        matches!(
            self,
            Self::Terminal { .. } | Self::Captured | Self::Numbered
        )
    }

    /// The `records.tsv` header, absent when the reading validator expects the
    /// rows alone.
    const fn header(self) -> &'static str {
        match self {
            Self::Terminal { .. } => "case\texit\tinput_sha256\tstdout_sha256\tstderr_sha256\n",
            Self::Captured => "case\texit\n",
            Self::Adjacent | Self::Numbered | Self::Named => "",
        }
    }
}

/// One resolved producer invocation: the sample directory, the entry source to
/// build, where the bundle lands and which cases the layout names.
struct Invocation {
    sample_directory: PathBuf,
    root: PathBuf,
    directory: PathBuf,
    inputs: Vec<PathBuf>,
}

/// Resolve one sample's producer arguments into the cases its layout reads.
fn invocation(evidence: Evidence, arguments: &[String]) -> Invocation {
    match evidence {
        Evidence::Adjacent => {
            assert!(
                arguments.len() >= 4,
                "usage: runner ROOT.omg OUTPUT_DIRECTORY INPUT_FILE..."
            );
            let root = PathBuf::from(&arguments[1]);
            Invocation {
                sample_directory: sample_directory(&root),
                root,
                directory: PathBuf::from(&arguments[2]),
                inputs: arguments.iter().skip(3).map(PathBuf::from).collect(),
            }
        }
        Evidence::Terminal { .. } | Evidence::Captured => {
            assert_eq!(
                arguments.len(),
                3,
                "usage: runner ROOT.omg|SAMPLE_DIRECTORY OUTPUT_DIRECTORY (a bundle names its cases by data/*.stdin)"
            );
            let (root, sample_directory) = source(&PathBuf::from(&arguments[1]));
            Invocation {
                inputs: stdin_files(&sample_directory.join("data")),
                sample_directory,
                root,
                directory: PathBuf::from(&arguments[2]),
            }
        }
        Evidence::Numbered => {
            assert_eq!(
                arguments.len(),
                4,
                "usage: runner SAMPLE_DIRECTORY INPUT_DIRECTORY OUTPUT_DIRECTORY"
            );
            let sample_directory = PathBuf::from(&arguments[1]);
            let inputs = stdin_files(&PathBuf::from(&arguments[2]));
            assert!(
                !inputs.is_empty(),
                "a numbered bundle names at least one case"
            );
            Invocation {
                root: sample_directory.join("main.omg"),
                directory: PathBuf::from(&arguments[3]),
                inputs,
                sample_directory,
            }
        }
        Evidence::Named => {
            assert_eq!(
                arguments.len(),
                4,
                "usage: runner SAMPLE_DIRECTORY NAMES_FILE OUTPUT_DIRECTORY"
            );
            let sample_directory = PathBuf::from(&arguments[1]);
            let inputs = named(&sample_directory, &PathBuf::from(&arguments[2]));
            assert!(!inputs.is_empty(), "a named bundle names at least one case");
            Invocation {
                root: sample_directory.join("main.omg"),
                directory: PathBuf::from(&arguments[3]),
                inputs,
                sample_directory,
            }
        }
    }
}

/// One sample's roster entry for the shared capture harness.
#[derive(Clone, Copy)]
pub struct Sample {
    /// Cargo package name of the sample's validation crate. It names the
    /// producer in harness diagnostics; the published binary keeps that name.
    pub package: &'static str,
    /// Whether the sample has reviewed package decisions to refresh, which is
    /// the only per-sample difference between capture producers.
    pub review: bool,
    /// The evidence layout this sample's validation reads back.
    pub evidence: Evidence,
}

/// Run one sample's capture producer on the current process arguments.
///
/// `runner ROOT.omg OUTPUT_DIRECTORY INPUT_FILE...` builds the product from
/// the sample's source root and captures actual output for every input.
/// A bundled sample instead takes `runner ROOT.omg OUTPUT_DIRECTORY` alone, or
/// the sample directory holding that `main.omg`, and names its cases by
/// `data/*.stdin`, the matrix its validation froze. A numbered bundle takes
/// `runner SAMPLE_DIRECTORY INPUT_DIRECTORY OUTPUT_DIRECTORY` and names its
/// cases by position; a named bundle takes `runner SAMPLE_DIRECTORY NAMES_FILE
/// OUTPUT_DIRECTORY` and names its cases from that file.
/// `runner --review DIRECTORY` and `runner --resume-review DIRECTORY` refresh
/// normal package review for samples that declare [`Sample::review`].
pub fn main(sample: Sample) {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run(sample))
        .unwrap()
        .join()
        .unwrap();
}

fn run(sample: Sample) {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments
        .get(1)
        .is_some_and(|argument| argument == "--review" || argument == "--resume-review")
    {
        assert_eq!(
            arguments.len(),
            3,
            "review requires exactly a sample directory"
        );
        assert!(sample.review, "{} has no review mode", sample.package);
        review(&arguments[2], arguments[1] == "--resume-review");
        return;
    }
    let Invocation {
        sample_directory,
        root,
        directory,
        inputs,
    } = invocation(sample.evidence, &arguments);
    fs::create_dir_all(&directory).unwrap();
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let product_directory = directory.join("product");
    let outcomes = omega_build::build(BuildRequest {
        root_path: root,
        build_directory: Some(product_directory.clone()),
        targets: BuildTargets::Single(profile),
        product: ProjectProduct::Native {
            destination: NativeDestination::Retained,
            private_entry_ram: Vec::new(),
        },
        timings: false,
        offline: false,
        verify: false,
        // Preparation previously unwrapped its optional package project.
        require_package_project: true,
        optimization_rollback: Default::default(),
        build_snapshot: None,
    })
    .unwrap();
    let [outcome] = outcomes
        .try_into()
        .map_err(|_| "one named target produces exactly one Build outcome")
        .unwrap();
    let report = outcome.result.unwrap().into_report().unwrap().report;
    let artifact = report
        .retained_native_program()
        .map(|program| program.psi_artifact())
        .unwrap();
    let mut records = sample.evidence.header().to_owned();
    for (ordinal, input_path) in inputs.iter().enumerate() {
        let observation = terminal_runtime::execute(TerminalRuntimeRequest {
            entry: None,
            native_provider: None,
            x86_flags: terminal_runtime::capture_host_x86_flags("linux_x86_64")
                .expect("host flags capture"),
            memory_ordering:
                terminal_runtime::TerminalMemoryOrderingEnvironment::private_sequential(
                    "linux_x86_64",
                ),
            artifact,
            target: Some("linux_x86_64"),
            provisions: report.runtime_provisions(),
            provider_proposal: None,
            fuel_allowance: 500_000_000,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(fs::File::open(input_path).unwrap())),
            clock: None,
        })
        .unwrap();
        if sample.evidence.bundled() {
            let name = match sample.evidence {
                Evidence::Numbered => format!("{ordinal:02}"),
                _ => case_name(input_path).to_owned(),
            };
            let exit = exit_code(&observation.status, &name);
            fs::write(
                directory.join(format!("{name}.stdout")),
                &observation.stdout,
            )
            .unwrap();
            fs::write(
                directory.join(format!("{name}.stderr")),
                &observation.stderr,
            )
            .unwrap();
            if let Evidence::Named = sample.evidence {
                fs::write(directory.join(format!("{name}.exit")), exit.to_string()).unwrap();
            } else {
                records.push_str(&format!("{name}\t{exit}"));
                if let Evidence::Terminal { .. } = sample.evidence {
                    records.push_str(&format!(
                        "\t{}\t{}\t{}",
                        digest(&fs::read(input_path).unwrap()),
                        digest(&observation.stdout),
                        digest(&observation.stderr),
                    ));
                }
                records.push('\n');
            }
            println!("observed Terminal {name} exit {exit}");
        } else {
            let case_directory = input_path.parent().unwrap();
            fs::write(case_directory.join("terminal.stdout"), &observation.stdout).unwrap();
            fs::write(case_directory.join("terminal.stderr"), &observation.stderr).unwrap();
            fs::write(
                case_directory.join("terminal.status"),
                format!("{:?}", observation.status),
            )
            .unwrap();
            println!(
                "{} Terminal={:?}",
                case_directory.display(),
                observation.status
            );
        }
    }
    if sample.evidence.tabulated() {
        fs::write(directory.join("records.tsv"), &records).unwrap();
    }
    match sample.evidence {
        Evidence::Captured | Evidence::Numbered | Evidence::Named => {}
        Evidence::Adjacent => {
            let (_, executable) =
                omega_build::publication::publish_native_artifact(report, &product_directory)
                    .unwrap();
            for input_path in &inputs {
                let case_directory = input_path.parent().unwrap();
                let native = Command::new(&executable)
                    .stdin(Stdio::from(fs::File::open(input_path).unwrap()))
                    .output()
                    .unwrap();
                fs::write(case_directory.join("native.stdout"), native.stdout).unwrap();
                fs::write(case_directory.join("native.stderr"), native.stderr).unwrap();
                fs::write(
                    case_directory.join("native.status"),
                    native.status.code().unwrap().to_string(),
                )
                .unwrap();
                println!("{} native={}", case_directory.display(), native.status);
            }
        }
        Evidence::Terminal { sources } => {
            let terminal = artifact.to_bytes();
            fs::write(directory.join("application.terminal"), &terminal).unwrap();
            fs::write(directory.join("terminal.sha256"), digest(&terminal)).unwrap();
            let mut table = String::new();
            for name in sources.iter().copied() {
                table.push_str(&format!(
                    "{name}\t{}\n",
                    digest(&fs::read(sample_directory.join(name)).unwrap())
                ));
            }
            fs::write(directory.join("sources.tsv"), table).unwrap();
            fs::write(
                directory.join("producer.sha256"),
                digest(&fs::read(std::env::current_exe().unwrap()).unwrap()),
            )
            .unwrap();
            let (_, executable) =
                omega_build::publication::publish_native_artifact(report, &product_directory)
                    .unwrap();
            fs::write(
                directory.join("native.sha256"),
                digest(&fs::read(&executable).unwrap()),
            )
            .unwrap();
            fs::write(
                directory.join("native-path.txt"),
                executable.display().to_string(),
            )
            .unwrap();
            println!("retained native executable {}", executable.display());
        }
    }
}

/// Every `*.stdin` case in name order, which is the matrix a captured
/// validation froze and reads back.
fn stdin_files(data_directory: &Path) -> Vec<PathBuf> {
    let mut inputs: Vec<PathBuf> = fs::read_dir(data_directory)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "stdin")
        })
        .collect();
    inputs.sort();
    inputs
}

/// Every case a names file lists, one name per line, as its own `data`
/// fixture.
fn named(sample_directory: &Path, names: &Path) -> Vec<PathBuf> {
    fs::read_to_string(names)
        .unwrap()
        .lines()
        .filter(|name| !name.is_empty())
        .map(|name| sample_directory.join("data").join(format!("{name}.stdin")))
        .collect()
}

/// The sample directory a source root belongs to.
fn sample_directory(root: &Path) -> PathBuf {
    root.parent()
        .expect("the source root names a directory")
        .to_path_buf()
}

/// The entry source to build and the sample directory holding it, from either
/// the entry `main.omg` or the sample directory an observer names.
fn source(declared: &Path) -> (PathBuf, PathBuf) {
    if declared.is_dir() {
        (declared.join("main.omg"), declared.to_path_buf())
    } else {
        (declared.to_path_buf(), sample_directory(declared))
    }
}

/// A case's name, which also names its captures inside a bundle.
fn case_name(input_path: &Path) -> &str {
    let name = input_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("a case input names a case");
    assert!(
        name.bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "unexpected case name {name}"
    );
    name
}

/// The integer exit one captured case reports, which a records row carries.
fn exit_code(status: &TerminalExecutionStatus, name: &str) -> i32 {
    let TerminalExecutionStatus::ExternalCompletion(completion) = status else {
        panic!("{name}: expected the app's explicit Console exit, observed {status:?}");
    };
    completion.return_code
}

/// Lowercase SHA-256 over exactly these bytes, as every digest file spells it.
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

// Package identities include the actual executable: refresh normal checked
// review with this producer instead of overriding or weakening that identity.
fn review(directory: &str, resume: bool) {
    let root = PathBuf::from(directory).canonicalize().unwrap();
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let command = if resume {
        PackageCommand::Resume {
            kind: PackageCommandKind::Update,
        }
    } else {
        PackageCommand::Update {
            packages: Vec::new(),
            revision: None,
        }
    };
    let options = PackageCommandOptions {
        project_root: root.clone(),
        targets: if resume { Vec::new() } else { vec![profile] },
        offline: true,
        build_inputs: None,
    };
    let mut session = BuildSession::create(&root.join("build/package-manager")).unwrap();
    let result = execute_package_command(command, options, &mut session);
    let outcome = session.dispose(result).unwrap();
    print!("{}", outcome.report);
    for path in outcome.review_paths {
        println!("review: {}", path.display());
    }
    if outcome.status == PackageCommandStatus::ReviewRequired {
        std::process::exit(3);
    }
}
