//! Catalog fence effects survive source transport, canonical wire admission,
//! target selection and fuel pauses. Scalar output alone cannot detect a lost
//! fence, so compare the exact completed event stream for every fact place.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use compiler::{CompileOptions, CompileRequest, RequestedCompileProduct};
use fixture_package_inputs::{repo_root, reviewed_repository_fixture_package_inputs};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInstantiation,
    TerminalMemoryOrderingEnvironment, TerminalProgram, TerminalRuntimeSupplyRequest,
};
use terminal_psi::MemoryFenceKind;

#[test]
fn checked_fence_rows_preserve_exact_events_without_pause_replay() {
    for place in [
        "root",
        "local",
        "field",
        "parameter",
        "call_result",
        "match_join",
        "const_name",
        "other_state",
        "other_machine",
        "target_body",
    ] {
        let source = repo_root().join(format!(
            "tests/omega/pass/facts/checked_asm_fence_contracts_through_{place}_exit/main.omg"
        ));
        let prepared = reviewed_repository_fixture_package_inputs(&source, Some("linux_x86_64"))
            .unwrap_or_else(|errors| panic!("{place} package inputs: {errors:#?}"));
        let mut request = CompileRequest::new(CompileOptions {
            root_path: prepared.root_path,
            build_dir: Some(repo_root().join(format!("build/checked-fence-events/{place}"))),
            target_name: Some("linux_x86_64".to_owned()),
        })
        .with_requested_product(RequestedCompileProduct::TerminalArtifact);
        if let Some(inputs) = prepared.package_inputs {
            request = request.with_package_inputs(inputs);
        }
        let retained = compiler::compile(request)
            .and_then(compiler::CompileOutcomes::into_single_report)
            .unwrap_or_else(|errors| panic!("{place} checked production: {errors:#?}"))
            .into_retained_terminal_artifact()
            .expect("requested Terminal artifact");
        // External ingress must preserve the event kinds and target demands too.
        let decoded =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
                .unwrap();
        let program = TerminalProgram::borrow(decoded.view());
        for target in [
            "linux_x86_64",
            "windows_x86_64",
            "linux_arm64",
            "macos_arm64",
        ] {
            let selected =
                terminal_interpreter::admit_target_machine_selections(decoded.view(), target);
            let layouts =
                terminal_interpreter::admit_target_layouts(decoded.view(), &selected).unwrap();
            let supplies = |model| {
                program.admit_runtime_supplies(TerminalRuntimeSupplyRequest {
                    x86_flags: None,
                    memory_ordering: model,
                    entry: None,
                    installation: None,
                    host_imports: &[],
                    target_machines: Some(&selected),
                    target_layouts: Some(&layouts),
                })
            };
            let absent = supplies(None).unwrap();
            assert!(
                matches!(
                    program.execution_requirements(Some(&absent)),
                    Err(
                        terminal_interpreter::TerminalInterpretError::UnsupportedCapability(
                            terminal_interpreter::TerminalUnsupportedCapability::RegisterAccess {
                                operation: terminal_psi::RegisterOperation::MemoryFence(_)
                            }
                        )
                    )
                ),
                "{place}/{target}: missing model must refuse before execution"
            );
            let other = if target.ends_with("arm64") {
                "linux_x86_64"
            } else {
                "linux_arm64"
            };
            assert!(
                supplies(TerminalMemoryOrderingEnvironment::private_sequential(other)).is_err(),
                "wrong architecture supplies no ordering capability"
            );
            let installed = supplies(TerminalMemoryOrderingEnvironment::private_sequential(
                target,
            ))
            .unwrap();
            let requirements = program.execution_requirements(Some(&installed)).unwrap();
            assert!(
                requirements.imports.is_empty()
                    && requirements.missing_providers.is_empty()
                    && requirements.external_completions.is_empty()
            );
            let instantiate = || {
                program
                    .instantiate(TerminalInstantiation {
                        installation: Some(&installed),
                        ..Default::default()
                    })
                    .unwrap()
            };
            let mut reference = instantiate();
            let mut unbounded = TerminalFuelMeter::unbounded();
            let complete = TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit);
            assert_eq!(
                reference.invoke(&mut unbounded).unwrap(),
                complete,
                "{place}/{target}"
            );
            let sequence = if target.ends_with("arm64") {
                [
                    MemoryFenceKind::Aarch64Load,
                    MemoryFenceKind::Aarch64Store,
                    MemoryFenceKind::Aarch64Full,
                ]
            } else {
                [
                    MemoryFenceKind::Load,
                    MemoryFenceKind::Store,
                    MemoryFenceKind::Full,
                ]
            };
            let expected = sequence.repeat(if place == "match_join" { 2 } else { 1 });
            let observed = reference
                .effects()
                .iter()
                .map(|effect| match effect {
                    terminal_interpreter::TerminalEffect::MemoryFence { kind, .. } => *kind,
                    other => panic!("unexpected effect: {other:?}"),
                })
                .collect::<Vec<_>>();
            assert_eq!(
                observed, expected,
                "{place}/{target}: no skipped or substituted ordering event"
            );
            let mut paused = instantiate();
            let mut fuel = TerminalFuelMeter::with_allowance(0);
            let mut finished = false;
            let mut paused_inside_sequence = false;
            for _ in 0..=unbounded.usage().total_units() {
                match paused.invoke(&mut fuel).unwrap() {
                    TerminalExecutionStatus::SponsorExhausted(exhaustion) => {
                        assert!(
                            reference.effects().starts_with(paused.effects()),
                            "effects remain the completed prefix"
                        );
                        paused_inside_sequence |=
                            !paused.effects().is_empty() && paused.effects().len() < expected.len();
                        let before = paused.effects().to_vec();
                        assert_eq!(
                            paused.invoke(&mut fuel).unwrap(),
                            TerminalExecutionStatus::SponsorExhausted(exhaustion)
                        );
                        assert_eq!(
                            paused.effects(),
                            before,
                            "unfunded retry repeats no completed fence"
                        );
                        fuel.replenish(
                            exhaustion
                                .required_units
                                .checked_sub(exhaustion.remaining_units)
                                .filter(|units| *units != 0)
                                .unwrap(),
                        )
                        .unwrap();
                    }
                    actual => {
                        assert_eq!(actual, complete);
                        finished = true;
                        break;
                    }
                }
            }
            assert!(
                finished && paused_inside_sequence,
                "{place}/{target}: exercise an actual pause inside the sequence"
            );
            assert_eq!(
                paused.effects(),
                reference.effects(),
                "completed operations are not replayed across calls or fuel pauses"
            );
        }
    }
}
