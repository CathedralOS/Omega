//! The corpus runtime leg uses the shipped Terminal runtime owner. It supplies
//! the native report's exact retained provisions, never a second compilation,
//! dummy receiver, expected-output callback or Unit-returning exit mock.

use compiler::runtime::{
    TerminalDomainExitDelegation, TerminalRuntimeError, TerminalRuntimeRequest,
};
use terminal_interpreter::{TerminalExecutionResult, TerminalExecutionStatus};

pub(super) const TARGET_ORACLE: &str = "expected_terminal_targets.txt";

#[derive(Clone, Copy)]
enum ExpectedTargetOutcome {
    Exit(i32),
    Trap,
}

fn target_expectations(
    fixture_dir: &std::path::Path,
) -> Result<Vec<(String, ExpectedTargetOutcome)>, String> {
    let text = std::fs::read_to_string(fixture_dir.join(TARGET_ORACLE))
        .map_err(|error| format!("cannot read {TARGET_ORACLE}: {error}"))?;
    let mut expectations: Vec<(String, ExpectedTargetOutcome)> = Vec::new();
    for (line_index, line) in text.lines().enumerate() {
        let words = line.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            continue;
        }
        let [target, expected] = words.as_slice() else {
            return Err(format!(
                "{TARGET_ORACLE}: line {} needs target and decimal exit or trap",
                line_index + 1
            ));
        };
        if target.is_empty()
            || !target
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            || expectations
                .iter()
                .any(|(prior, _)| prior.as_str() == *target)
        {
            return Err(format!(
                "{TARGET_ORACLE}: line {} has an invalid or duplicate target",
                line_index + 1
            ));
        }
        let outcome = if *expected == "trap" {
            ExpectedTargetOutcome::Trap
        } else {
            let digits = expected.strip_prefix('-').unwrap_or(expected);
            if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(format!(
                    "{TARGET_ORACLE}: line {} has no decimal i32 exit or trap",
                    line_index + 1
                ));
            }
            ExpectedTargetOutcome::Exit(expected.parse::<i32>().map_err(|_| {
                format!(
                    "{TARGET_ORACLE}: line {} exit is outside i32",
                    line_index + 1
                )
            })?)
        };
        expectations.push(((*target).to_owned(), outcome));
    }
    if expectations.is_empty() {
        return Err(format!("{TARGET_ORACLE}: no target expectations"));
    }
    Ok(expectations)
}

/// Trusted fixture configuration requests product-root capture, not authority.
/// Every supplied root still comes from the exact target's normal admission.
/// Absence retains the independent artifact-only installation route.
pub(super) fn compiled_target_provisions(fixture_dir: &std::path::Path) -> Result<bool, String> {
    match std::fs::read_to_string(fixture_dir.join("terminal_target_provisions.txt")) {
        Ok(text) if text.trim() == "compiled-application" => Ok(true),
        Ok(_) => Err("terminal_target_provisions.txt: expected compiled-application".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "cannot read terminal_target_provisions.txt: {error}"
        )),
    }
}

pub(super) fn requested_target_names(fixture_dir: &std::path::Path) -> Result<Vec<String>, String> {
    target_expectations(fixture_dir)
        .map(|rows| rows.into_iter().map(|(target, _)| target).collect())
}

/// Install every authored target against one exact retained artifact. The
/// receiving runtime admits the retained whole-provider proposal for that
/// exact target. An explicitly requested compiled application cohort supplies
/// only that target's actual admitted root; artifact-only observations supply
/// none. Clock, input and domain authority remain absent and cannot be inferred.
/// Each install observes the artifact's unchanged semantic bytes,
/// identity, and canonical module storage, not a newly compiled target cohort.
pub(super) fn target_installations(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    proposals: &[backend_plan::TerminalTargetProviderProposal],
    root_provisions: &[&compilation_products::TerminalRuntimeProvisions],
    fixture_dir: &std::path::Path,
    fuel_allowance: u64,
) -> Observation {
    let expectations = match target_expectations(fixture_dir) {
        Ok(expectations) => expectations,
        Err(error) => {
            return Observation {
                facts: vec!["terminal-targets:invalid".to_owned()],
                diagnostics: vec![error],
            };
        }
    };
    let semantic_bytes = artifact.semantic_bytes().to_vec();
    let before = artifact.view();
    let mut observation = Observation::default();
    let mut matched = true;
    let mut unchanged = true;
    for (target, expected) in expectations {
        let mut matching = proposals
            .iter()
            .filter(|proposal| proposal.target == target);
        let provider_proposal = matching.next();
        if matching.next().is_some() {
            observation.diagnostics.push(format!(
                "Terminal target {target}: duplicate provider proposal"
            ));
            matched = false;
            continue;
        }
        let mut supplied = root_provisions
            .iter()
            .copied()
            .filter(|provisions| provisions.target_slot().owner.target_name() == target);
        let provisions = supplied.next();
        if supplied.next().is_some() {
            observation.diagnostics.push(format!(
                "Terminal target {target}: duplicate admitted root provisions"
            ));
            matched = false;
            continue;
        }
        let result = compiler::runtime::execute(TerminalRuntimeRequest {
            artifact,
            target: Some(&target),
            provisions,
            provider_proposal,
            fuel_allowance,
            domain_exit: TerminalDomainExitDelegation::Unprovided,
            standard_input: None,
            clock: None,
        });
        let (actual, matches) = match result {
            Ok(result) => match result.status {
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => (
                    "0".to_owned(),
                    matches!(expected, ExpectedTargetOutcome::Exit(0)),
                ),
                TerminalExecutionStatus::ExternalCompletion(completion) => (
                    completion.return_code.to_string(),
                    matches!(expected, ExpectedTargetOutcome::Exit(code) if code == completion.return_code),
                ),
                TerminalExecutionStatus::Crashed(crash)
                    if crash.cause == terminal_psi::CrashCause::Trap =>
                {
                    (
                        "trap".to_owned(),
                        matches!(expected, ExpectedTargetOutcome::Trap),
                    )
                }
                status => {
                    observation.diagnostics.push(format!(
                        "Terminal target {target}: unsupported entry outcome {status:?}"
                    ));
                    ("unsupported".to_owned(), false)
                }
            },
            Err(error) => {
                observation
                    .diagnostics
                    .push(format!("Terminal target {target}: {error}"));
                ("declined".to_owned(), false)
            }
        };
        observation
            .facts
            .push(format!("terminal-target.{target}:{actual}"));
        if !matches {
            let expected = match expected {
                ExpectedTargetOutcome::Exit(code) => code.to_string(),
                ExpectedTargetOutcome::Trap => "trap".to_owned(),
            };
            observation.diagnostics.push(format!(
                "Terminal target {target}: expected {expected}, observed {actual}"
            ));
            matched = false;
        }
        let after = artifact.view();
        if semantic_bytes != artifact.semantic_bytes()
            || before.identity() != after.identity()
            || !std::ptr::eq(before.module(), after.module())
        {
            unchanged = false;
            observation.diagnostics.push(format!("Terminal target {target}: installation changed canonical artifact storage or identity"));
        }
    }
    observation.facts.push(format!(
        "terminal-target-artifact:{}",
        if unchanged { "unchanged" } else { "changed" }
    ));
    observation.facts.push(format!(
        "terminal-targets:{}",
        if matched && unchanged {
            "match"
        } else {
            "differs"
        }
    ));
    observation
}

#[derive(Default)]
pub(super) struct Observation {
    pub facts: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl Observation {
    pub fn failure(error: impl std::fmt::Display) -> Self {
        Self {
            facts: vec!["interpreter:declined".to_owned()],
            diagnostics: vec![format!("Terminal interpreter: {error}")],
        }
    }

    fn missing(capability: &str, task: &str, reason: impl std::fmt::Display) -> Self {
        Self {
            facts: Vec::new(),
            diagnostics: vec![format!(
                "unimplemented: capability={capability} task={task} route=interpreter: {reason}"
            )],
        }
    }
}

pub(super) fn interpret(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    provisions: Option<&compilation_products::TerminalRuntimeProvisions>,
    target: Option<&str>,
    fixture_dir: &std::path::Path,
    fuel_allowance: u64,
    standard_input: Box<dyn std::io::Read + Send>,
) -> Observation {
    let clock = match compiler::runtime::TerminalClockSnapshot::capture_host_epoch() {
        Ok(clock) => clock,
        Err(error) => return Observation::failure(error),
    };
    let result = match compiler::runtime::execute(TerminalRuntimeRequest {
        provider_proposal: None,
        artifact,
        target,
        provisions,
        fuel_allowance,
        domain_exit: TerminalDomainExitDelegation::IsolatedContainingDomain,
        standard_input: Some(standard_input),
        clock: Some(clock),
    }) {
        Ok(result) => result,
        Err(TerminalRuntimeError::MissingProvision { capability, reason }) => {
            return Observation::missing(capability, "TERMINAL-RUNTIME-DRIVERS", reason);
        }
        Err(error) => return Observation::failure(error),
    };
    let mut observation = match result.status {
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit) => Observation {
            facts: vec!["interpreter-exit:0".to_owned()],
            diagnostics: Vec::new(),
        },
        TerminalExecutionStatus::ExternalCompletion(completion) => Observation {
            facts: vec![format!("interpreter-exit:{}", completion.return_code)],
            diagnostics: Vec::new(),
        },
        TerminalExecutionStatus::Crashed(crash)
            if crash.cause == terminal_psi::CrashCause::Trap =>
        {
            Observation {
                facts: vec!["interpreter:trapped".to_owned()],
                diagnostics: Vec::new(),
            }
        }
        TerminalExecutionStatus::SponsorExhausted(_) => {
            return Observation {
                facts: vec!["interpreter:exhausted".to_owned()],
                diagnostics: Vec::new(),
            };
        }
        other => {
            return Observation::failure(format!(
                "entry outcome is not process completion: {other:?}"
            ));
        }
    };
    observation.facts.extend(super::output_oracles::observe(
        fixture_dir,
        "interpreter-",
        Some(&result.stdout),
        Some(&result.stderr),
    ));
    observation
}
