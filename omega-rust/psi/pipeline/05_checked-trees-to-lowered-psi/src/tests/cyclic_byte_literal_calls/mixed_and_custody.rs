use super::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalStructuralValue,
};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::TerminalScalarValue;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[test]
fn composed_attached_literal_calls_preserve_positions_after_unused_self_erasure() {
    let checked = crate::front_end::checked_program(
        r#"
boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
data Main { counter: u64 in Wrapping; }
machine Main::forward(&mut self, first: &[u8], second: &[u8]) reaches Output {
    Output::write(first);
    Output::write(second);
}
machine Main::main(&mut self, bytes: &[u8]) reaches Output {
    transition { _ -> first(bytes) }
    state first(&mut self, bytes: &[u8]) {
        self.forward("left", bytes);
        transition { _ -> second(bytes) }
    }
    state second(&mut self, bytes: &[u8]) {
        self.forward("right", bytes);
    }
}
"#,
    );
    super::super::super::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("composed calls preserve literal and view positions when helper self is erased");
    let helper = checked
        .facts
        .flow
        .terminal_machines
        .machines
        .iter()
        .find(|selection| selection.name == "Main::forward")
        .unwrap()
        .machine;
    let mut changed = checked.clone();
    let target = changed
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|target| target.machine == helper)
        .unwrap();
    assert_eq!(target.structural_parameters[0].position, 1);
    target.structural_parameters[0].position = 0;
    let result =
        super::super::super::lower_machine(&changed, TerminalMachineSelection::Name("Main::main"))
            .map(|_| ());
    assert!(
        result.is_err(),
        "retained literal position drift must reject: {result:?}"
    );
}

#[test]
fn attached_literal_calls_bind_full_formal_positions_to_authored_sources() {
    let checked = crate::front_end::checked_program(
        r#"
boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
data Main { counter: u64 in Wrapping; }
machine Main::forward(&mut self, first: &[u8], second: &[u8]) reaches Output {
    Output::write(first);
    Output::write(second);
}
machine Main::main(&mut self, bytes: &[u8]) reaches Output {
    self.forward("left", bytes);
}
"#,
    );
    super::super::super::lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("ordinary attached literal and shared-view call is valid");
    for replace_with_parameter in [false, true] {
        let mut changed = checked.clone();
        let arguments = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .flat_map(|machine| &mut machine.operations)
            .find_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } if structural_arguments.iter().any(|argument| {
                    argument.byte_sequence_literal() == Some(b"left".as_slice())
                }) =>
                {
                    Some(structural_arguments)
                }
                _ => None,
            })
            .expect("ordinary call retains its sole authored literal");
        let shared_view_source = arguments
            .last()
            .expect("final formal is the shared byte-view parameter")
            .source
            .clone();
        assert!(matches!(
            shared_view_source,
            CheckedUnitStructuralArgumentSourcePlan::Parameter { .. }
        ));
        let literal = arguments
            .iter_mut()
            .find(|argument| argument.byte_sequence_literal() == Some(b"left".as_slice()))
            .unwrap();
        literal.source = if replace_with_parameter {
            shared_view_source
        } else {
            CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
                bytes: b"LEFT".to_vec(),
            }
        };
        let result = super::super::super::lower_machine(
            &changed,
            TerminalMachineSelection::Name("Main::main"),
        )
        .map(|_| ());
        assert!(
            matches!(
                &result,
                Err(super::super::super::LoweringError::Unsupported(_))
            ),
            "authored literal substitution (parameter={replace_with_parameter}) must reject: {result:?}"
        );
    }
}

const SOURCE: &str = r#"
boundary trait Trace {
    machine record(first: u16, left: &[u8], selected: bool, right: &[u8], last: u16) reaches Trace;
}
machine identity16(input: u16) -> u16
requires 0u16 == 0u16
ensures 0u16 == 0u16
{ input }
machine abort() -> bool crashes Abort { crash Abort; }
machine consume(first: u16, left: &[u8], selected: bool, right: &[u8], last: u16) reaches Trace {
    Trace::record(first, left, selected, right, last);
}
domain [u8;3]::Utf8 requires valid_utf8(self);
data Main { out: [u8;3] in Utf8; }
machine Main::main(&mut self, selected: bool, fail: bool) reaches Trace crashes Abort {
    self.out = "old";
    transition selected { true -> yes(fail) _ -> no() }
    state yes(&mut self, fail: bool) {
        consume(identity16(17u16), "left", fail && abort(), "", identity16(23u16));
    }
    state no(&mut self) {
        consume(identity16(23u16), "right", false, "λ", identity16(17u16));
    }
}
"#;

#[test]
fn mixed_literal_positions_keep_scalars_across_selective_operand_control() {
    let checked = crate::front_end::checked_program(SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let receiver = &entry.structural_parameters[0];
    let unsigned = |value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
        value: IntegerValue::Unsigned(value),
    };
    for (selected, fail) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut execution = TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[
                TerminalScalarValue::Boolean(selected),
                TerminalScalarValue::Boolean(fail),
            ],
            TerminalStructuralInputs {
                arguments: &[TerminalStructuralValue {
                    opaque_identity: 1,
                    structural_type: receiver.structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let mut trace = MixedTrace::default();
        let mut meter = TerminalFuelMeter::with_allowance(0);
        let status = loop {
            let status = execution.resume(&mut meter, &mut trace).unwrap();
            if !matches!(status, TerminalExecutionStatus::SponsorExhausted(_)) {
                break status;
            }
            let prefix = trace.0.clone();
            let usage = meter.usage().clone();
            assert!(matches!(
                execution.resume(&mut meter, &mut trace).unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert_eq!(trace.0, prefix);
            assert_eq!(meter.usage(), &usage);
            assert!(
                meter.usage().total_units() < 1000,
                "bounded fixture must complete"
            );
            meter.replenish(1).unwrap();
        };
        if selected && fail {
            assert!(
                matches!(status, TerminalExecutionStatus::Crashed(crash) if crash.cause == terminal_psi::CrashCause::Abort)
            );
            assert!(
                trace.0.is_empty(),
                "operand crash prevents entry into consume"
            );
        } else {
            assert_eq!(
                status,
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
            );
            let (first, left, right, last) = if selected {
                (17, "left", "", 23)
            } else {
                (23, "right", "λ", 17)
            };
            assert_eq!(
                trace.0,
                vec![(
                    vec![
                        unsigned(first),
                        TerminalScalarValue::Boolean(false),
                        unsigned(last)
                    ],
                    vec![
                        Some(left.as_bytes().to_vec()),
                        Some(right.as_bytes().to_vec())
                    ],
                )]
            );
        }
    }
}

#[derive(Default)]
struct MixedTrace(Vec<(Vec<TerminalScalarValue>, Vec<Option<Vec<u8>>>)>);

impl TerminalEffectHandler for MixedTrace {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            byte_sequence_arguments,
            ..
        } = effect
        else {
            panic!("only authored trace boundary is observable")
        };
        self.0
            .push((arguments.clone(), byte_sequence_arguments.clone()));
        Ok(())
    }
}
