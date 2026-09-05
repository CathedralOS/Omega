use super::*;

const STAGE_ENV: &str = "OMEGA_PORTABLE_TERMINAL_RELOAD_STAGE";
const PATH_ENV: &str = "OMEGA_PORTABLE_TERMINAL_RELOAD_PATH";
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
    let artifact_path = directory.join("program.psi");
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create portable Terminal reload directory");

    run_stage(PRODUCE, &artifact_path);
    assert!(
        artifact_path.is_file(),
        "producer must publish one Psi product"
    );
    run_stage(CONSUME, &artifact_path);

    let _ = fs::remove_dir_all(directory);
}

fn run_stage(stage: &str, artifact_path: &Path) {
    let output = Command::new(std::env::current_exe().expect("locate canary test executable"))
        .args(["--exact", TEST_NAME, "--nocapture"])
        .env(STAGE_ENV, stage)
        .env(PATH_ENV, artifact_path)
        .output()
        .unwrap_or_else(|error| panic!("run portable Terminal {stage} invocation: {error}"));
    assert!(
        output.status.success(),
        "portable Terminal {stage} invocation failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn produce_portable_terminal_product() {
    let artifact_path = artifact_path();
    let fixture = pass_canary("terminal_psi/selected_empty_component");
    let report = compiler::compile(
        CompileRequest::new(CompilerOptions {
            root_path: fixture.join("main.omg"),
            build_dir: artifact_path.parent().map(Path::to_path_buf),
            target_name: Some("linux_x86_64".into()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact)
        .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
    )
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
    drop(artifact);
    let mut authority = RejectUnexpectedEffects;
    let execution =
        terminal_interpreter::interpret_serialized_terminal_artifact_with_effect_handler_measured(
            &bytes,
            &proof_admission::AdmissionProfile::default(),
            &[],
            &[],
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
