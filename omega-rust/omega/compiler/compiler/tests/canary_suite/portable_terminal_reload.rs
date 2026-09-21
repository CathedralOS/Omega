use super::{
    Command, CompileRequest, CompilerOptions, Path, PathBuf, RequestedCompileProduct, fs,
    pass_canary,
};
use terminal_interpreter::TerminalStructuralInputs;
#[path = "../fixture_rosters/portable_terminal_reload.rs"]
pub(super) mod fixture_roster;

const STAGE_ENV: &str = "OMEGA_PORTABLE_TERMINAL_RELOAD_STAGE";
const PATH_ENV: &str = "OMEGA_PORTABLE_TERMINAL_RELOAD_PATH";
const FIXTURE_ENV: &str = "OMEGA_PORTABLE_TERMINAL_RELOAD_FIXTURE";
const PRODUCE: &str = "produce";
const CONSUME: &str = "consume";
const TEST_NAME: &str =
    "portable_terminal_reload::portable_terminal_product_reloads_across_process_boundary";

#[test]
fn portable_terminal_product_reloads_across_process_boundary() {
    match std::env::var(STAGE_ENV).as_deref() {
        Ok(PRODUCE) => produce_portable_terminal_product(),
        Ok(CONSUME) => consume_portable_terminal_product(),
        Ok(stage) => panic!("unknown portable Terminal reload stage `{stage}`"),
        Err(_) => orchestrate_process_boundary(),
    }
}

fn orchestrate_process_boundary() {
    let directory = std::env::temp_dir().join(format!(
        "omega-portable-terminal-reload-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create portable Terminal reload directory");

    for fixture in fixture_roster::RELOAD_CANARIES {
        let artifact_path = directory.join(format!("{}.psi", fixture.replace('/', "_")));
        run_stage(PRODUCE, &artifact_path, fixture);
        assert!(
            artifact_path.is_file(),
            "producer must publish one Psi product for {fixture}"
        );
        run_stage(CONSUME, &artifact_path, fixture);

        // The portable contract's refused half: a consumer that silently
        // accepts a tampered product is not a boundary. Truncation and
        // trailing bytes fail the envelope cursor, and a mutated section
        // byte fails decode or the byte-exact canonicality replay.
        let exact_bytes = fs::read(&artifact_path).expect("read produced artifact");
        for (leg, tamper) in REFUSAL_LEGS {
            let tampered_path = directory.join(format!("{}-{leg}.psi", fixture.replace('/', "_")));
            let mut bytes = exact_bytes.clone();
            tamper(&mut bytes);
            fs::write(&tampered_path, &bytes).expect("write tampered artifact");
            assert_consume_refuses(&tampered_path, fixture, leg);
            fs::remove_file(&tampered_path).expect("remove tampered artifact");
        }
    }

    let _ = fs::remove_dir_all(directory);
}

const REFUSAL_LEGS: &[(&str, fn(&mut Vec<u8>))] = &[
    ("truncated", |bytes| {
        bytes.pop();
    }),
    ("mutated", |bytes| {
        let index = bytes.len() / 2;
        bytes[index] ^= 0xFF;
    }),
    ("trailing", |bytes| bytes.push(0)),
];

fn assert_consume_refuses(artifact_path: &Path, fixture: &str, leg: &str) {
    let output = Command::new(std::env::current_exe().expect("locate canary test executable"))
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(STAGE_ENV, CONSUME)
        .env(PATH_ENV, artifact_path)
        .env(FIXTURE_ENV, fixture)
        .output()
        .unwrap_or_else(|error| panic!("run portable Terminal {leg} refusal: {error}"));
    assert!(
        !output.status.success(),
        "portable Terminal consume accepted a {leg} artifact for {fixture}:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn run_stage(stage: &str, artifact_path: &Path, fixture: &str) {
    let output = Command::new(std::env::current_exe().expect("locate canary test executable"))
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(STAGE_ENV, stage)
        .env(PATH_ENV, artifact_path)
        .env(FIXTURE_ENV, fixture)
        .output()
        .unwrap_or_else(|error| panic!("run portable Terminal {stage} invocation: {error}"));
    assert!(
        output.status.success(),
        "portable Terminal {stage} invocation failed for {fixture}:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn produce_portable_terminal_product() {
    let artifact_path = artifact_path();
    let fixture =
        pass_canary(&std::env::var(FIXTURE_ENV).expect("portable Terminal fixture selection"));
    let report = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: fixture.join("main.omg"),
            build_dir: artifact_path.parent().map(Path::to_path_buf),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact),
    )
    .and_then(compiler::CompileOutcomes::into_single_report)
    .expect("first invocation must produce standalone Terminal Psi");
    let retained = report
        .into_retained_terminal_artifact()
        .expect("Terminal request retains exactly one product");
    let bytes = retained.artifact().to_bytes();
    drop(retained);
    fs::write(&artifact_path, bytes).expect("serialize standalone Terminal Psi");
}

fn consume_portable_terminal_product() {
    let bytes = fs::read(artifact_path()).expect("read standalone Terminal Psi");
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("consumer independently decodes the complete Psi product");
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("consumer independently decodes the semantic module");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("decoded module carries its entry machine");
    let bundle = terminal_codec::decode_proof_bundle(artifact.proof_bytes())
        .expect("consumer independently decodes the proof bundle");
    terminal_verifier::verify_module(
        &module,
        &bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("consumer's verifier discharges the reconstructed obligations");
    let validated = terminal_verifier::validate_module(&module).expect("decoded module validates");
    let reconstructed = terminal_verifier::reconstruct_execution_terminal_obligations(validated)
        .expect("consumer independently reconstructs the obligation ledger");
    let fixture = std::env::var(FIXTURE_ENV).expect("portable Terminal fixture selection");
    if fixture_roster::OBLIGATION_BEARING_RELOAD_CANARIES.contains(&fixture.as_str()) {
        assert!(
            !reconstructed.obligations().is_empty(),
            "reconstructed obligation ledger for {fixture} must not be empty"
        );
    }
    let structural_arguments = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(
            |(index, parameter)| terminal_interpreter::TerminalStructuralValue {
                opaque_identity: index as u64 + 1,
                structural_type: parameter.structural_type,
                qualifications: parameter.qualifications.clone(),
                path: Vec::new(),
            },
        )
        .collect::<Vec<_>>();
    drop(artifact);
    let mut authority = RejectUnexpectedEffects;
    let execution = terminal_interpreter::interpret_serialized_terminal_artifact_measured(
        &bytes,
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &structural_arguments,
            ..Default::default()
        },
        &mut authority,
    )
    .expect("second invocation must decode, verify, and interpret standalone Terminal Psi");
    assert_eq!(
        execution.value(),
        terminal_interpreter::TerminalExecutionResult::Unit
    );
    assert!(execution.effects().is_empty());
}

fn artifact_path() -> PathBuf {
    std::env::var_os(PATH_ENV)
        .map(PathBuf::from)
        .expect("portable Terminal stage requires its artifact path")
}

struct RejectUnexpectedEffects;

impl terminal_interpreter::TerminalEffectHandler for RejectUnexpectedEffects {
    fn handle_effect(
        &mut self,
        effect: &terminal_interpreter::TerminalEffect,
    ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
        Err(terminal_interpreter::TerminalEffectRejection::new(format!(
            "fresh reload authority rejects unexpected effect {effect:?}"
        )))
    }
}
