//! Capture actual numerical native and Terminal output from the same product.
//! This uses normal package preparation and preserves its admission checks.
//! Review modes use this exact executable identity; run never accepts decisions.
//!
//! Every independent sample runs the identical product: package preparation,
//! the retained Terminal artifact, the published native product and the
//! per-input captures. Only the sample's own identity and the declarations it
//! makes differ, so each sample states one [`Sample`] roster and shares this
//! harness: whether it has reviewed package decisions to refresh
//! ([`Sample::review`]), the ceilings its native child already ran under
//! ([`Sample::caps`]), where plain captures land ([`Sample::evidence`]) and the
//! cases whose frozen expectations it still compares ([`Sample::frozen`]). The
//! numerical oracle, its frozen output and its tolerance stay with the sample
//! that owns them: plain capture never compares, while `--evidence` replays a
//! sample's frozen cases and fails on the first wrong expectation.
use bounded_process::{
    BoundedCaptureBudget, BoundedProcessInput, BoundedProcessPrepared, run_bounded_process,
};
pub use bounded_process::{BoundedCaptureLimits, BoundedProcessLimits};
use omega_build::{
    BuildReport, BuildRequest, BuildSession, BuildTargets, NativeDestination, ProjectProduct,
    execute_package_command,
};
use package_manager::{
    PackageCommand, PackageCommandKind, PackageCommandOptions, PackageCommandStatus,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use terminal_runtime::{
    TerminalDomainExitDelegation, TerminalExecutionStatus, TerminalRuntimeRequest,
};

/// The resource ceilings one sample's validation already holds each native
/// child to, so moving that validation here cannot widen them.
///
/// A sample that reviewed bounded native execution declares the exact limits it
/// reviewed; a sample that reviewed a plain child declares none. Both the
/// process ceilings and the capture ceilings are carried, because a bound on
/// time or memory that the capture contract also enforces is not the bound the
/// sample reviewed unless both move together.
#[derive(Clone, Copy)]
pub struct Caps {
    /// The process ceilings that bound and close one native child.
    pub process: BoundedProcessLimits,
    /// The capture ceilings that bound one child's streams and deadline.
    pub capture: BoundedCaptureLimits,
    /// Bytes of captured output one child may produce in total.
    pub budget_bytes: u64,
    /// The sample's own name for the boundary, used in bounded-child
    /// diagnostics so a refusal names the sample that overran.
    pub boundary: &'static str,
}

/// The evidence layout a sample's `validate.py` reads back when it only
/// captures.
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
    /// rows alone or nothing is tabulated at all.
    const fn header(self) -> &'static str {
        match self {
            Self::Terminal { .. } => "case\texit\tinput_sha256\tstdout_sha256\tstderr_sha256\n",
            Self::Captured => "case\texit\n",
            Self::Adjacent | Self::Numbered | Self::Named => "",
        }
    }

    /// Every named input as one capture case in this layout.
    fn cases(self, inputs: &[PathBuf], directory: &Path) -> Vec<Case> {
        inputs
            .iter()
            .enumerate()
            .map(|(ordinal, input)| {
                let input_bytes = fs::read(input).unwrap();
                if !self.bundled() {
                    let case_directory = input.parent().unwrap();
                    return Case {
                        label: case_directory.display().to_string(),
                        input: input_bytes,
                        stdout: case_directory.join("terminal.stdout"),
                        stderr: case_directory.join("terminal.stderr"),
                        status: Some(Status {
                            path: case_directory.join("terminal.status"),
                            exit: false,
                        }),
                        digests: false,
                        expected: None,
                    };
                }
                let label = match self {
                    Self::Numbered => format!("{ordinal:02}"),
                    _ => case_name(input).to_owned(),
                };
                Case {
                    stdout: directory.join(format!("{label}.stdout")),
                    stderr: directory.join(format!("{label}.stderr")),
                    status: match self {
                        Self::Named => Some(Status {
                            path: directory.join(format!("{label}.exit")),
                            exit: true,
                        }),
                        _ => None,
                    },
                    digests: matches!(self, Self::Terminal { .. }),
                    label,
                    input: input_bytes,
                    expected: None,
                }
            })
            .collect()
    }
}

/// The frozen cases one sample still compares, and where their captures land.
///
/// A sample that had a comparison of its own before it moved here restates that
/// comparison instead of letting this harness record whatever the product now
/// prints.
#[derive(Clone, Copy)]
pub struct FrozenCases {
    /// Capture directory relative to the sample directory. The build that
    /// produced the artifact stays in `build/terminal-compilation` beside it.
    pub directory: &'static str,
    /// Sponsor fuel for one case. Each sample keeps the allowance it froze
    /// under, so a program that exhausted it before still fails rather than
    /// now fitting.
    pub fuel: u64,
    /// How this sample states its cases.
    pub cases: Cases,
}

/// The two shapes an independent sample has frozen its cases in.
#[derive(Clone, Copy)]
pub enum Cases {
    /// `data/*.stdin` in sorted order. A case whose name starts with `invalid_`
    /// exits 2 and every other case exits 0; `expected/<case>.stdout` is its
    /// exact stdout.
    ExpectedFiles,
    /// `data/oracle.json`: each case names its own input and frozen exit. Its
    /// numerical stdout belongs to the sample's own oracle script, which this
    /// harness does not duplicate.
    OracleRows,
}

/// What one case must produce. A case without an expectation is only captured.
struct Expected {
    exit: i32,
    /// Exact stdout when the sample froze it; `None` leaves numerical output to
    /// the sample's own script.
    stdout: Option<Vec<u8>>,
}

/// The file one case records its completion in.
struct Status {
    path: PathBuf,
    /// Whether the file holds the integer exit a validator reads rather than
    /// the observed status a plain adjacent capture keeps.
    exit: bool,
}

impl Status {
    /// Record this case's completion at its status path.
    fn write(&self, status: &TerminalExecutionStatus, label: &str) {
        let text = if self.exit {
            exit_code(status, label).to_string()
        } else {
            format!("{status:?}")
        };
        fs::write(&self.path, text).unwrap();
    }
}

/// One case this harness runs: the stdin it replays, where its capture lands
/// and what it must produce when the sample froze an expectation.
struct Case {
    /// The case's name, which names its captures inside a bundle and its
    /// diagnostics outside one.
    label: String,
    input: Vec<u8>,
    stdout: PathBuf,
    stderr: PathBuf,
    /// The completion file this case records, absent when the layout tabulates
    /// the case in `records.tsv` instead.
    status: Option<Status>,
    /// Whether the `records.tsv` row also carries the input and capture
    /// digests.
    digests: bool,
    expected: Option<Expected>,
}

/// One resolved producer invocation: the entry source to build, where the
/// bundle lands and which cases this mode runs.
struct Invocation {
    sample_directory: PathBuf,
    root: PathBuf,
    directory: PathBuf,
    product_directory: PathBuf,
    inputs: Vec<PathBuf>,
    cases: Vec<Case>,
    fuel: u64,
    /// Whether this invocation replays frozen cases instead of only capturing.
    comparing: bool,
}

/// One sample's roster entry for the shared capture harness.
#[derive(Clone, Copy)]
pub struct Sample {
    /// Cargo package name of the sample's validation crate. It names the
    /// producer in harness diagnostics; the published binary keeps that name.
    pub package: &'static str,
    /// Whether the sample has reviewed package decisions to refresh.
    pub review: bool,
    /// The native-child ceilings the sample's own validation reviewed, or
    /// `None` when it reviewed a plain unbounded child.
    pub caps: Option<Caps>,
    /// The evidence layout this sample's validation reads back when it only
    /// captures.
    pub evidence: Evidence,
    /// The frozen cases this sample still compares, or `None` when it only
    /// captures.
    pub frozen: Option<FrozenCases>,
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
/// `runner --evidence SAMPLE_DIRECTORY` builds the same product and compares
/// every frozen case against the expectation the sample restated.
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
        product_directory,
        inputs,
        cases,
        fuel,
        comparing,
    } = invocation(sample, &arguments);
    fs::create_dir_all(&directory).unwrap();
    let report = build(root, product_directory.clone()).report;
    let artifact = report
        .retained_native_program()
        .map(|program| program.psi_artifact())
        .unwrap();
    let mut records = sample.evidence.header().to_owned();
    for case in &cases {
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
            fuel_allowance: fuel,
            domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
            standard_input: Some(Box::new(Cursor::new(case.input.clone()))),
            clock: None,
        })
        .unwrap_or_else(|error| fail(format!("{}: {error}", case.label)));
        fs::write(&case.stdout, &observation.stdout).unwrap();
        fs::write(&case.stderr, &observation.stderr).unwrap();
        let Some(expected) = &case.expected else {
            if let Some(status) = &case.status {
                status.write(&observation.status, &case.label);
            }
            if sample.evidence.tabulated() {
                let exit = exit_code(&observation.status, &case.label);
                records.push_str(&format!("{}\t{exit}", case.label));
                if case.digests {
                    records.push_str(&format!(
                        "\t{}\t{}\t{}",
                        digest(&case.input),
                        digest(&observation.stdout),
                        digest(&observation.stderr),
                    ));
                }
                records.push('\n');
            }
            println!("{} Terminal={:?}", case.label, observation.status);
            continue;
        };
        let exit = exit_code(&observation.status, &case.label);
        if let Some(status) = &case.status {
            fs::write(&status.path, exit.to_string()).unwrap();
        }
        compare(
            case,
            expected,
            &observation.stdout,
            &observation.stderr,
            exit,
        )
        .unwrap_or_else(|error| fail(error));
        println!(
            "{}: PASS Terminal exit={exit}, {} and empty stderr",
            case.label,
            if expected.stdout.is_some() {
                "exact stdout"
            } else {
                "frozen exit"
            }
        );
    }
    if comparing {
        println!(
            "{} Terminal cases matched their frozen evidence in {}",
            cases.len(),
            directory.display()
        );
        return;
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
                let (stdout, stderr, code) = match sample.caps {
                    None => {
                        let native = Command::new(&executable)
                            .stdin(Stdio::from(fs::File::open(input_path).unwrap()))
                            .output()
                            .unwrap();
                        println!("{} native={}", case_directory.display(), native.status);
                        (native.stdout, native.stderr, native.status.code().unwrap())
                    }
                    // The ceilings this sample's validation reviewed still
                    // close the child here, so moving it onto this harness
                    // widened none of them.
                    Some(caps) => {
                        let prepared = BoundedProcessPrepared::new(
                            Command::new(&executable),
                            caps.process,
                            caps.boundary,
                        )
                        .unwrap();
                        let native = run_bounded_process(
                            prepared,
                            BoundedProcessInput::File(fs::File::open(input_path).unwrap()),
                            caps.capture,
                            BoundedCaptureBudget::new(caps.budget_bytes),
                        )
                        .unwrap();
                        println!("{} native={:?}", case_directory.display(), native.status);
                        (native.stdout, native.stderr, native.status.code().unwrap())
                    }
                };
                fs::write(case_directory.join("native.stdout"), stdout).unwrap();
                fs::write(case_directory.join("native.stderr"), stderr).unwrap();
                fs::write(case_directory.join("native.status"), code.to_string()).unwrap();
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

/// Resolve one sample's producer arguments into the cases this mode runs.
fn invocation(sample: Sample, arguments: &[String]) -> Invocation {
    if arguments
        .get(1)
        .is_some_and(|argument| argument == "--evidence")
    {
        assert_eq!(
            arguments.len(),
            3,
            "--evidence requires exactly a sample directory"
        );
        assert!(
            !sample.evidence.bundled(),
            "{}: frozen cases name their own captures",
            sample.package
        );
        let frozen = sample
            .frozen
            .unwrap_or_else(|| panic!("{} has no frozen cases", sample.package));
        let sample_directory = PathBuf::from(&arguments[2])
            .canonicalize()
            .expect("existing sample directory");
        let directory = sample_directory.join(frozen.directory);
        fs::create_dir_all(&directory).unwrap();
        let cases = frozen
            .cases
            .read(&sample_directory, &directory)
            .unwrap_or_else(|error| fail(error));
        return Invocation {
            root: sample_directory.join("main.omg"),
            product_directory: sample_directory.join("build/terminal-compilation"),
            cases,
            inputs: Vec::new(),
            fuel: frozen.fuel,
            comparing: true,
            sample_directory,
            directory,
        };
    }
    assert!(
        !sample.evidence.bundled() || sample.caps.is_none(),
        "{}: only the adjacent layout publishes a native child to bound",
        sample.package
    );
    let (root, sample_directory, directory, inputs) = match sample.evidence {
        Evidence::Adjacent => {
            assert!(
                arguments.len() >= 4,
                "usage: runner ROOT.omg OUTPUT_DIRECTORY INPUT_FILE..."
            );
            let root = PathBuf::from(&arguments[1]);
            let sample_directory = sample_directory(&root);
            (
                root,
                sample_directory,
                PathBuf::from(&arguments[2]),
                arguments.iter().skip(3).map(PathBuf::from).collect(),
            )
        }
        Evidence::Terminal { .. } | Evidence::Captured => {
            assert_eq!(
                arguments.len(),
                3,
                "usage: runner ROOT.omg|SAMPLE_DIRECTORY OUTPUT_DIRECTORY (a bundle names its cases by data/*.stdin)"
            );
            let (root, sample_directory) = source(&PathBuf::from(&arguments[1]));
            let inputs = matrix(&sample_directory);
            (root, sample_directory, PathBuf::from(&arguments[2]), inputs)
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
            (
                sample_directory.join("main.omg"),
                sample_directory,
                PathBuf::from(&arguments[3]),
                inputs,
            )
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
            (
                sample_directory.join("main.omg"),
                sample_directory,
                PathBuf::from(&arguments[3]),
                inputs,
            )
        }
    };
    let cases = sample.evidence.cases(&inputs, &directory);
    Invocation {
        product_directory: directory.join("product"),
        cases,
        inputs,
        fuel: 500_000_000,
        comparing: false,
        sample_directory,
        root,
        directory,
    }
}

/// Build one sample's product from `root` into `product_directory`.
fn build(root: PathBuf, product_directory: PathBuf) -> BuildReport {
    let profile = target::TargetProfile::from_omega_target_name(Some("linux_x86_64")).unwrap();
    let outcomes = omega_build::build(BuildRequest {
        root_path: root,
        build_directory: Some(product_directory),
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
    outcome.result.unwrap().into_report().unwrap()
}

/// Fail on the first field that differs from the sample's frozen expectation.
/// A wrong expectation must never be recorded as a capture.
fn compare(
    case: &Case,
    expected: &Expected,
    stdout: &[u8],
    stderr: &[u8],
    exit: i32,
) -> Result<(), String> {
    let stdout_match = expected
        .stdout
        .as_deref()
        .is_none_or(|frozen| frozen == stdout);
    if exit == expected.exit && stdout_match && stderr.is_empty() {
        return Ok(());
    }
    Err(format!(
        "{}: Terminal capture differs from frozen evidence; exit={exit}, expected={}, stdout_match={stdout_match}, stderr_empty={}",
        case.label,
        expected.exit,
        stderr.is_empty()
    ))
}

fn fail(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

impl Cases {
    /// Read every case this sample froze, in its declared order.
    fn read(self, root: &Path, directory: &Path) -> Result<Vec<Case>, String> {
        match self {
            Cases::ExpectedFiles => expected_files(root, directory),
            Cases::OracleRows => oracle_rows(root, directory),
        }
    }
}

/// `data/*.stdin` with `expected/<case>.stdout`; `invalid_` cases exit 2.
fn expected_files(root: &Path, directory: &Path) -> Result<Vec<Case>, String> {
    let mut fixtures: Vec<PathBuf> = fs::read_dir(root.join("data"))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "stdin")
        })
        .collect();
    fixtures.sort();
    fixtures
        .into_iter()
        .map(|fixture| {
            let name = fixture
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or("case without a UTF-8 name")?;
            checked_name(name)?;
            let stdout = fs::read(root.join("expected").join(format!("{name}.stdout")))
                .map_err(|error| format!("{name}: {error}"))?;
            Ok(Case {
                label: name.to_string(),
                input: fs::read(&fixture).map_err(|error| format!("{name}: {error}"))?,
                stdout: directory.join(format!("{name}.stdout")),
                stderr: directory.join(format!("{name}.stderr")),
                status: Some(Status {
                    path: directory.join(format!("{name}.exit")),
                    exit: true,
                }),
                digests: false,
                expected: Some(Expected {
                    exit: if name.starts_with("invalid_") { 2 } else { 0 },
                    stdout: Some(stdout),
                }),
            })
        })
        .collect()
}

/// `data/oracle.json` rows: each names its own input and frozen exit.
fn oracle_rows(root: &Path, directory: &Path) -> Result<Vec<Case>, String> {
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("data/oracle.json")).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    manifest["cases"]
        .as_array()
        .ok_or("missing frozen cases")?
        .iter()
        .map(|row| {
            let name = row["name"].as_str().ok_or("non-string case name")?;
            checked_name(name)?;
            let exit = i32::try_from(row["exit"].as_i64().ok_or("non-integer case exit")?)
                .map_err(|_| "case exit outside i32")?;
            Ok(Case {
                label: name.to_string(),
                input: row["input"]
                    .as_str()
                    .ok_or("non-string case input")?
                    .as_bytes()
                    .to_vec(),
                stdout: directory.join(format!("{name}.stdout")),
                stderr: directory.join(format!("{name}.stderr")),
                status: Some(Status {
                    path: directory.join(format!("{name}.exit")),
                    exit: true,
                }),
                digests: false,
                expected: Some(Expected { exit, stdout: None }),
            })
        })
        .collect()
}

/// Case names become capture file names, so they stay one safe path component.
fn checked_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(format!("invalid case name {name:?}"));
    }
    Ok(())
}

/// Every `data/*.stdin` case of one sample in name order, which is the matrix
/// a captured validation froze and reads back.
fn matrix(sample_directory: &Path) -> Vec<PathBuf> {
    stdin_files(&sample_directory.join("data"))
}

/// Every `*.stdin` case in one directory in name order.
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

/// The entry source to build and the sample directory holding it, from either
/// the entry `main.omg` or the sample directory an observer names.
fn source(declared: &Path) -> (PathBuf, PathBuf) {
    if declared.is_dir() {
        (declared.join("main.omg"), declared.to_path_buf())
    } else {
        (declared.to_path_buf(), sample_directory(declared))
    }
}

/// The sample directory a source root belongs to.
fn sample_directory(root: &Path) -> PathBuf {
    root.parent()
        .expect("the source root names a directory")
        .to_path_buf()
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

#[cfg(test)]
mod tests {
    use super::{Case, Cases, Evidence, Expected, compare, matrix};
    use std::path::{Path, PathBuf};

    fn capture(name: &str) -> Case {
        Case {
            label: name.to_string(),
            input: Vec::new(),
            stdout: PathBuf::new(),
            stderr: PathBuf::new(),
            status: None,
            digests: false,
            expected: None,
        }
    }

    fn frozen(exit: i32, stdout: Option<&[u8]>) -> Expected {
        Expected {
            exit,
            stdout: stdout.map(<[u8]>::to_vec),
        }
    }

    #[test]
    fn a_capture_that_matches_is_accepted() {
        assert_eq!(
            compare(&capture("coarse"), &frozen(0, None), b"GRID 4", b"", 0),
            Ok(())
        );
        assert_eq!(
            compare(
                &capture("baseline"),
                &frozen(0, Some(b"1.0\n")),
                b"1.0\n",
                b"",
                0
            ),
            Ok(())
        );
    }

    #[test]
    fn a_wrong_frozen_exit_fails() {
        let error = compare(&capture("coarse"), &frozen(0, None), b"GRID 4", b"", 2).unwrap_err();
        assert!(
            error.contains("exit=2") && error.contains("expected=0"),
            "{error}"
        );
    }

    #[test]
    fn a_wrong_frozen_stdout_fails() {
        let error = compare(
            &capture("baseline"),
            &frozen(0, Some(b"1.0\n")),
            b"1.1\n",
            b"",
            0,
        )
        .unwrap_err();
        assert!(error.contains("stdout_match=false"), "{error}");
    }

    #[test]
    fn runtime_stderr_fails() {
        let error =
            compare(&capture("baseline"), &frozen(0, None), b"", b"warning", 0).unwrap_err();
        assert!(error.contains("stderr_empty=false"), "{error}");
    }

    /// The numbered and named layouts still name their cases the way their own
    /// validators read them: position for `records.tsv`, file stem for
    /// `NAME.exit`.
    #[test]
    fn bundled_layouts_still_name_their_cases() {
        let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let inputs = matrix(&samples.join("pareto_experiment_selection"));
        let captures = Path::new("unused");
        let numbered = Evidence::Numbered.cases(&inputs[..2], captures);
        assert_eq!(
            numbered
                .iter()
                .map(|case| case.label.as_str())
                .collect::<Vec<_>>(),
            ["00", "01"]
        );
        assert_eq!(numbered[1].stdout, captures.join("01.stdout"));
        assert!(numbered.iter().all(|case| case.status.is_none()));
        assert!(Evidence::Numbered.tabulated() && !Evidence::Named.tabulated());
        let named = Evidence::Named.cases(&inputs[..2], captures);
        let names: Vec<&str> = inputs[..2]
            .iter()
            .map(|input| super::case_name(input))
            .collect();
        assert_eq!(
            named
                .iter()
                .map(|case| case.label.as_str())
                .collect::<Vec<_>>(),
            names
        );
        let status = named[0].status.as_ref().unwrap();
        assert!(status.exit);
        assert_eq!(status.path, captures.join(format!("{}.exit", names[0])));
    }

    /// The two samples that froze cases read exactly the evidence they ship.
    #[test]
    fn sample_rosters_read_their_frozen_cases() {
        let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let captures = Path::new("unused");
        let spatial = Cases::OracleRows
            .read(&samples.join("spatial_diffusion"), captures)
            .unwrap();
        assert_eq!(spatial.len(), 20);
        assert!(spatial.iter().all(|case| {
            let expected = case.expected.as_ref().unwrap();
            expected.stdout.is_none()
        }));
        assert!(spatial.iter().any(|case| {
            let expected = case.expected.as_ref().unwrap();
            case.label == "coarse" && expected.exit == 0
        }));
        assert!(spatial.iter().any(|case| {
            let expected = case.expected.as_ref().unwrap();
            case.label == "empty" && expected.exit == 64
        }));
        let pareto = Cases::ExpectedFiles
            .read(&samples.join("pareto_experiment_selection"), captures)
            .unwrap();
        assert_eq!(pareto.len(), 28);
        assert!(pareto.iter().any(|case| {
            let expected = case.expected.as_ref().unwrap();
            case.label == "invalid_negative" && expected.exit == 2
        }));
        assert!(pareto.iter().any(|case| {
            let expected = case.expected.as_ref().unwrap();
            case.label == "baseline"
                && expected
                    .stdout
                    .as_deref()
                    .is_some_and(|text| !text.is_empty())
        }));
    }

    /// Every shipped case passes when matched and fails when it is not, so a
    /// wrong expectation cannot be recorded as a capture.
    #[test]
    fn every_frozen_case_fails_when_its_expectation_is_wrong() {
        let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let captures = Path::new("unused");
        let mut checked = 0;
        for (root, cases) in [
            ("spatial_diffusion", Cases::OracleRows),
            ("pareto_experiment_selection", Cases::ExpectedFiles),
        ] {
            for case in cases.read(&samples.join(root), captures).unwrap() {
                let expected = case.expected.as_ref().unwrap();
                let stdout = expected.stdout.clone().unwrap_or_default();
                assert_eq!(
                    compare(&case, expected, &stdout, b"", expected.exit),
                    Ok(()),
                    "{}",
                    case.label
                );
                let wrong_exit =
                    compare(&case, expected, &stdout, b"", expected.exit.wrapping_add(1))
                        .unwrap_err();
                assert!(wrong_exit.contains("expected="), "{wrong_exit}");
                assert!(
                    compare(&case, expected, &stdout, b"note", expected.exit).is_err(),
                    "{}",
                    case.label
                );
                if expected.stdout.is_some() {
                    let mut wrong_stdout = stdout.clone();
                    wrong_stdout.push(b'!');
                    assert!(
                        compare(&case, expected, &wrong_stdout, b"", expected.exit).is_err(),
                        "{}",
                        case.label
                    );
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 48);
    }
}
