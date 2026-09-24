//! `self.field = <construction>` over a structural field, through a
//! source-produced Terminal artifact: the construction is established, the
//! displaced value moves out of the borrowed receiver, the new value closes
//! the hole, and an affine displaced value dies on the statement's
//! continuation. Independent verification and replay observe each replacement
//! through the field's active case.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralCaseValue, TerminalStructuralInputs, TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

#[derive(Default)]
struct Observations(Vec<bool>);

impl TerminalEffectHandler for Observations {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("only authored case observations are expected")
        };
        let [TerminalScalarValue::Boolean(value)] = arguments.as_slice() else {
            panic!("one Boolean observation per call: {arguments:?}")
        };
        self.0.push(*value);
        Ok(())
    }
}

fn case_id(
    module: &terminal_psi::TerminalModule,
    type_name: &str,
    case_name: &str,
) -> semantic_vocabulary::StructuralCaseId {
    module
        .structural_types
        .iter()
        .find_map(|declaration| {
            let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return None;
            };
            cases
                .iter()
                .find(|case| {
                    declaration.identity.contains(&format!("({type_name})"))
                        && case.identity == case_name
                })
                .map(|case| case.id)
        })
        .unwrap_or_else(|| panic!("{type_name}::{case_name} is declared"))
}

/// Produce, verify and replay `Main::main` with its receiver's sum field
/// `field` initially holding `initial`, returning the Boolean observations.
fn replay(source: &str, field: &str, sum: &str, initial: &str) -> Vec<bool> {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("constructed field replacements publish")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile)
        .expect("independent verification accepts each replacement");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let receiver = &entry.structural_parameters[0];
    let cases = [TerminalStructuralCaseValue {
        argument_index: 0,
        path: vec![terminal_psi::StructuralPathSegment::Field(field.into())],
        case: case_id(&module, sum, initial),
    }];
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: receiver.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            cases: &cases,
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(10_000);
    let mut observations = Observations::default();
    assert_eq!(
        execution.resume(&mut meter, &mut observations).unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    observations.0
}

#[test]
fn affine_case_literals_replace_the_receiver_field_in_order() {
    let observations = replay(
        r#"
        boundary trait Trace { machine observe(value: bool) reaches Trace; }
        data Mode { case Stand; case Walk(pace: i32); case Run(speed: i32); }
        data Main { mode: Mode; }
        machine Main::main(&mut self) reaches Trace {
            self.mode = Mode::Walk { pace: 9 };
            Trace::observe(self.mode in Mode::Walk);
            self.mode = Mode::Run { speed: 70 };
            Trace::observe(self.mode in Mode::Walk);
            Trace::observe(self.mode in Mode::Run);
        }
        "#,
        "mode",
        "Mode",
        "Stand",
    );
    assert_eq!(observations, [true, false, true]);
}

#[test]
fn copy_case_literal_replaces_a_field_without_disposal() {
    let observations = replay(
        r#"
        boundary trait Trace { machine observe(value: bool) reaches Trace; }
        data Signal [copy] { case Off; case Level(value: u8); }
        data Main { signal: Signal; }
        machine Main::main(&mut self) reaches Trace {
            self.signal = Signal::Level { value: 3 };
            Trace::observe(self.signal in Signal::Level);
        }
        "#,
        "signal",
        "Signal",
        "Off",
    );
    assert_eq!(observations, [true]);
}

/// The same replacement inside a multi-state body: the state graph rejoins
/// each establishment, window pair and continuation cleanup, and the selected
/// successor observes the last replacement's case.
#[test]
fn replacements_before_a_transition_reach_the_selected_state() {
    let observations = replay(
        r#"
        boundary trait Trace { machine observe(value: bool) reaches Trace; }
        data Mode { case Stand; case Walk(pace: i32); case Run(speed: i32); }
        data Main { mode: Mode; }
        machine Main::main(&mut self) reaches Trace {
            self.mode = Mode::Walk { pace: 9 };
            self.mode = Mode::Run { speed: 70 };
            let ran: bool = self.mode in Mode::Run;
            transition ran {
                true -> finished()
                _ -> stalled()
            }
            state finished(&mut self) { Trace::observe(true); }
            state stalled(&mut self) { Trace::observe(false); }
        }
        "#,
        "mode",
        "Mode",
        "Stand",
    );
    assert_eq!(observations, [true]);
}

/// A structural call result replaces its field through the very same roster:
/// only the producer differs.
#[test]
fn call_result_replaces_the_field_through_the_same_roster() {
    let observations = replay(
        r#"
        boundary trait Trace { machine observe(value: bool) reaches Trace; }
        data Mode { case Stand; case Walk(pace: i32); case Run(speed: i32); }
        machine Mode::run(speed: i32) -> Mode { Mode::Run { speed: speed } }
        data Main { mode: Mode; }
        machine Main::main(&mut self) reaches Trace {
            self.mode = Mode::run(70);
            Trace::observe(self.mode in Mode::Run);
        }
        "#,
        "mode",
        "Mode",
        "Stand",
    );
    assert_eq!(observations, [true]);
}

/// `self.copy = self.items[0]` over a `[copy]` record: the source element is
/// copied out of the borrowed receiver (`StructuralLeafCopy`, which leaves it
/// intact) and replaces `copy` through the same window pair a construction
/// uses, with no cleanup for the displaced copyable value.
#[test]
fn a_copied_place_replaces_a_structural_field() {
    let kinds = copy_replacement_kinds(
        "data Item [copy] { value: i32; }
        data Main { items: [Item; 2]; copy: Item; }
        machine Main::main(&mut self) {
            self.items[0].value = 17;
            self.copy = self.items[0];
        }",
    );
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, terminal_psi::OperationKind::StructuralLeafCopy { .. }))
    );
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        terminal_psi::OperationKind::StoreStructuralField { .. }
    )));
}

/// A whole owned `[copy]` record parameter is the degenerate copied place: it
/// is copied rather than moved, and replaces the field through the same
/// window pair.
#[test]
fn a_whole_copy_parameter_replaces_a_structural_field() {
    let kinds = copy_replacement_kinds(
        "data Item [copy] { value: i32; }
        data Main { copy: Item; }
        machine Main::main(&mut self, item: Item) {
            self.copy = item;
        }",
    );
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        terminal_psi::OperationKind::StructuralLeafCopy { path, .. } if path.is_empty()
    )));
    assert!(kinds.iter().any(|kind| matches!(
        kind,
        terminal_psi::OperationKind::StoreStructuralField { .. }
    )));
}

/// Produce and independently verify `Main::main`, returning its operations.
fn copy_replacement_kinds(source: &str) -> Vec<terminal_psi::OperationKind> {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("a copied place replaces its field")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent verification accepts the copy replacement");
    module
        .machines
        .into_iter()
        .flat_map(|machine| machine.blocks)
        .flat_map(|block| block.operations)
        .map(|operation| operation.kind)
        .collect()
}
