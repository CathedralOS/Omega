//! Ranges passed to a terminal machine call in a guarded transition arm are
//! staged inside the arm, at their authored argument position. A range over
//! a receiver's array first establishes the whole field view it narrows; a
//! range over a view parameter narrows it against the length the arm's
//! guard observed, so the guard discharges the range's bound.
use super::{
    AdmissionProfile, TerminalEffect, TerminalExecutionResult, encode_module, encode_proof_section,
    interpret_terminal_artifact_measured,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_psi::OperationKind;

const FIELD_RANGE: &str = r#"
    boundary trait Output { machine count(value: u64) reaches Output; }
    data Root { items: [u64; 5]; }
    machine Root::consume(&mut self, sub: &[u64]) reaches Output {
        transition sub.len >= 1 {
            true -> report(sub)
            false -> done()
        }
        state report(&mut self, sub: &[u64]) {
            Output::count(sub.len);
            Output::count(sub[0]);
        }
        state done(&mut self) {}
    }
    machine Root::view(&mut self, lo: u64, hi: u64) reaches Output {
        transition lo <= hi && hi <= self.items.len {
            true -> self.consume(self.items[lo..hi])
            false -> rejected()
        }
        state rejected(&mut self) { Output::count(0); }
    }
    machine Root::enter(&mut self) reaches Output {
        self.items[1] = 42;
        self.view(1, 4);
        self.view(3, 2);
    }
"#;

const VIEW_RANGE: &str = r#"
    boundary trait Output { machine count(value: u64) reaches Output; }
    data Root {}
    machine Root::verify(prefix: &[u8]) reaches Output {
        Output::count(prefix.len);
    }
    machine Root::check(path: &[u8], end: u64) reaches Output {
        transition end < path.len {
            true -> Root::verify(path[0..end])
            false -> rejected()
        }
        state rejected() { Output::count(0); }
    }
    machine Root::enter() reaches Output {
        Root::check("/adir/x", 5);
        Root::check("/adir/x", 9);
    }
"#;

fn lower(source: &str) -> Result<lowered_psi::LoweredPsi, String> {
    checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(source),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .map_err(|error| format!("{error:?}"))
}

/// Verify the reloaded artifact independently, without producer state.
fn verify(lowered: &lowered_psi::LoweredPsi) -> (Vec<u8>, Vec<u8>) {
    let semantic = encode_module(&lowered.semantic_module).unwrap();
    let evidence = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(&semantic).unwrap(),
        &terminal_codec::decode_proof_bundle(&evidence).unwrap(),
        &AdmissionProfile::default(),
    )
    .expect("the staged range verifies independently");
    (semantic, evidence)
}

/// Verify the artifact, run it, and collect every counted value.
fn counts(lowered: &lowered_psi::LoweredPsi) -> Vec<u128> {
    let (semantic, evidence) = verify(lowered);
    let executed = interpret_terminal_artifact_measured(
        &semantic,
        &evidence,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .expect("the staged range executes");
    assert_eq!(executed.value(), TerminalExecutionResult::Unit);
    executed
        .effects()
        .iter()
        .map(|effect| {
            let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("a counted output");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one unsigned count");
            };
            *value
        })
        .collect()
}

/// The machine with an operation `subslice` selects.
fn ranging_machine(
    lowered: &lowered_psi::LoweredPsi,
    subslice: fn(&OperationKind) -> bool,
) -> &terminal_psi::TerminalMachine {
    lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| subslice(&operation.kind))
        })
        .expect("a machine stages the range")
}

/// The arm establishes the receiver array's whole view, narrows it, and
/// passes the narrowed view, which the verifier bounds by the array's
/// declared extent. (The corpus fixture
/// `slices/runtime_bounded_fixed_array_subslice_arg_exit` executes this
/// shape; an opaque test receiver supplies no array storage to run it here.)
#[test]
fn guarded_tail_call_narrows_a_receiver_field_view_inside_its_arm() {
    let lowered = lower(FIELD_RANGE).expect("the field range lowers");
    let machine = ranging_machine(&lowered, |kind| {
        matches!(kind, OperationKind::ElementViewSubslice { .. })
    });
    let block = machine
        .blocks
        .iter()
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(operation.kind, OperationKind::ElementViewSubslice { .. })
            })
        })
        .unwrap();
    let whole = block
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::EstablishElementView { source, .. } if !source.path.is_empty() => {
                Some(operation.result.structural()?.place)
            }
            _ => None,
        })
        .expect("the arm establishes the field's whole view");
    let narrowed = block
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::ElementViewSubslice { source, .. } if source == whole => {
                Some(operation.result.structural()?.place)
            }
            _ => None,
        })
        .expect("the range narrows the field's view");
    assert!(
        block.operations.iter().any(|operation| matches!(
            &operation.kind,
            OperationKind::CallUnit { structural_arguments, .. }
                if structural_arguments.iter().any(|argument| argument.place == narrowed)
        )),
        "the call receives the narrowed view: {:#?}",
        block.operations
    );
    verify(&lowered);
}

/// A range over a view parameter is bounded by the length its arm's guard
/// already observed: the arm reuses that observation rather than measuring
/// the view again, so `end < path.len` discharges `path[0..end]`.
#[test]
fn guarded_tail_call_bounds_a_view_range_by_its_guard() {
    let lowered = lower(VIEW_RANGE).expect("the view range lowers");
    let machine = ranging_machine(&lowered, |kind| {
        matches!(kind, OperationKind::ByteSequenceSubslice { .. })
    });
    let (range_block, length) = machine
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .find_map(|operation| match operation.kind {
                    OperationKind::ByteSequenceSubslice { length, .. } => Some((block.id, length)),
                    _ => None,
                })
        })
        .unwrap();
    let measured = machine
        .blocks
        .iter()
        .find_map(|block| {
            block.operations.iter().find_map(|operation| {
                (matches!(operation.kind, OperationKind::ByteSequenceLength { .. })
                    && operation
                        .result
                        .scalar()
                        .is_some_and(|value| value.id == length))
                .then_some(block.id)
            })
        })
        .expect("the range's length is an observation");
    assert_ne!(
        measured, range_block,
        "the range reuses the guard's dominating observation"
    );
    assert_eq!(counts(&lowered), [5, 0]);
}

/// The range replays its endpoint facts at its own call-argument site: a
/// retained end rebound to another expression rejects.
#[test]
fn a_rebound_call_argument_endpoint_rejects() {
    let checked = crate::front_end::checked_program(VIEW_RANGE);
    let site = checked_trees::CheckedSubsliceSite::CallArgument {
        call_ordinal: 0,
        argument_ordinal: 0,
    };
    let end = checked_trees::CheckedScalarExpressionRole::SubsliceEnd { site };
    let handle = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find_map(|(handle, binding)| (binding.role == end).then_some(handle))
        .expect("the range end is source-bound at its call-argument site");
    let start = checked
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .iter()
        .find_map(|(_, binding)| {
            (binding.role == checked_trees::CheckedScalarExpressionRole::SubsliceStart { site })
                .then_some(binding.expression)
        })
        .expect("the range start is source-bound too");
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_expressions
        .source_bindings
        .get_mut(handle)
        .expression = start;
    let error = checked_trees_to_lowered_psi::lower_machine(
        &changed,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect_err("an endpoint bound to another expression is not the authored range");
    assert!(
        format!("{error:?}").contains("view subslice endpoint differs from its source-bound plan"),
        "{error:?}"
    );
}

/// An ordinary single-state body has no roster for the whole-field view a
/// field range establishes, so it refuses the range by name.
#[test]
fn ordinary_body_refuses_a_field_range_by_name() {
    let source = FIELD_RANGE.replace(
        "machine Root::enter(&mut self) reaches Output {",
        "machine Root::measure(&mut self, lo: u64, hi: u64) reaches Output
        requires lo <= hi && hi <= 5;
        {
            self.consume(self.items[lo..hi]);
        }
        machine Root::enter(&mut self) reaches Output {
            self.measure(1, 4);",
    );
    let error = lower(&source).expect_err("the ordinary route declines the field range");
    assert!(
        error.contains("an ordinary body declares no whole-field view for a field range"),
        "{error}"
    );
}
