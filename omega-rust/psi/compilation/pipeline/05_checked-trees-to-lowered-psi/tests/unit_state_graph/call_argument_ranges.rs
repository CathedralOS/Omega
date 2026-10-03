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

/// Changing the surrounding control flow does not change collection-view
/// establishment. The unchanged field range also lowers in an ordinary body.
#[test]
fn ordinary_body_establishes_the_same_field_range() {
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
    let lowered = lower(&source).expect("the ordinary route shares range establishment");
    verify(&lowered);
}

/// The descriptor's backing is resolved storage, including a prior local in a
/// scalar-result body. Empty storage needs the same operation and no read.
#[test]
fn local_array_ranges_compose_with_scalar_results() {
    let source = r#"
        boundary trait Output { machine count(value: u64) reaches Output; }
        data Root {}
        machine Root::length(values: &[i32]) -> u64 { values.len }
        machine Root::compute() -> u64 {
            let values: [i32; 2] = [31, 39];
            let empty: [i32; 0] = [];
            match Root::length(&empty[..]) == 0 {
                true -> Root::length(&values[..]),
                false -> 99,
            }
        }
        machine Root::enter() reaches Output { Output::count(Root::compute()); }
    "#;
    let lowered = lower(source).expect("local backing composes with scalar calls");
    verify(&lowered);
    let mut changed = lowered.clone();
    let source = changed
        .semantic_module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::EstablishElementView { source, .. } => Some(source),
            _ => None,
        })
        .expect("array-backed view producer");
    source.access = terminal_psi::StructuralAccess::WriteOnlyBorrow;
    assert!(
        terminal_verifier::verify_module(
            &changed.semantic_module,
            &changed.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "a write-only backing cannot manufacture a readable view"
    );
    assert_eq!(counts(&lowered), [2]);
}

#[test]
fn nested_byte_array_ranges_use_the_same_primitive_storage() {
    let source = r#"
        boundary trait Output { machine count(value: u64) reaches Output; }
        data Root {}
        machine Root::length(values: &[u8]) -> u64 { values.len }
        machine Root::compute() -> u64 {
            let rows: [[u8; 2]; 2] = [[3, 5], [7, 9]];
            Root::length(&rows[1][..])
        }
        machine Root::enter() reaches Output { Output::count(Root::compute()); }
    "#;
    let lowered = lower(source).expect("nested backing uses the shared range producer");
    assert_eq!(counts(&lowered), [2]);
}

#[test]
fn projected_array_ranges_compose_with_ordinary_calls() {
    let source = r#"
        boundary trait Output { machine count(value: u64) reaches Output; }
        data Root {}
        machine Root::report(values: &[u8]) reaches Output {
            Output::count(values.len);
        }
        machine Root::enter() reaches Output {
            let rows: [[u8; 2]; 2] = [[3, 5], [7, 9]];
            Root::report(&rows[1][..]);
        }
    "#;
    let lowered = lower(source).expect("a collection projection does not transfer an owned claim");
    assert_eq!(counts(&lowered), [2]);
}

#[test]
fn mutable_field_range_state_transfer_replays_exact_backing_and_access() {
    let original = crate::front_end::checked_program(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/slices/mutable_field_range_call_state_exit/main.omg"
    )));
    let lower = |checked: &checked_trees::CheckedTrees| {
        checked_trees_to_lowered_psi::lower_machine(checked, TerminalMachineSelection::Name("main"))
    };
    verify(&lower(&original).expect("the unchanged mutable call/state range publishes"));
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "through_state")
        .expect("the authored range-forwarding helper")
        .symbol;
    for mutation in 0..3 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .find(|plan| plan.machine == machine)
            .expect("the helper state graph");
        let checked_trees::CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
            &mut plan.states[0].terminator
        else {
            panic!("the authored direct state edge");
        };
        let target = successor.target_state;
        if mutation < 2 {
            let checked_trees::CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
                root,
                path,
                ..
            } = &mut successor.transfers[0].source
            else {
                panic!("the retained field range");
            };
            if mutation == 0 {
                *root = checked_trees::CheckedStorageRoot::Parameter { index: 1 };
            } else {
                assert!(
                    !path.is_empty(),
                    "the original range selects the items field"
                );
                path.clear();
            }
        } else {
            let destination = plan
                .states
                .iter_mut()
                .find(|state| state.state == target)
                .expect("the authored mutable-view destination");
            assert_eq!(
                destination.structural_parameters[0].access,
                checked_trees::CheckedStructuralAccess::MutableBorrow
            );
            destination.structural_parameters[0].access =
                checked_trees::CheckedStructuralAccess::SharedBorrow;
        }
        assert!(
            lower(&changed).is_err(),
            "range custody mutation {mutation} must reject"
        );
    }
}

#[test]
fn mutable_field_range_calls_reject_forged_authority_and_missing_cleanup() {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, Terminator};
    let checked = crate::front_end::checked_program(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../tests/omega/pass/slices/mutable_field_range_call_state_exit/main.omg"
    )));
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("main"),
    )
    .expect("mutable range calls publish");
    let (semantic, _) = verify(&lowered);
    let original = terminal_codec::decode_module(&semantic).unwrap();
    for mutation in 0..4 {
        let mut module = original.clone();
        let mut altered = false;
        let mut missing_cleanup = None;
        for machine in &mut module.machines {
            if mutation == 3 {
                let transfer = machine.blocks.iter().find_map(|block| {
                    let Terminator::Jump {
                        target,
                        structural_arguments,
                        ..
                    } = &block.terminator
                    else {
                        return None;
                    };
                    structural_arguments
                        .iter()
                        .enumerate()
                        .find_map(|(ordinal, argument)| {
                            (argument.access == StructuralAccess::MutableBorrow
                                && block.operations.iter().any(|operation| {
                                    matches!(
                                        operation.kind,
                                        OperationKind::ElementViewSubslice { .. }
                                    ) && operation
                                        .result
                                        .structural()
                                        .is_some_and(|result| result.place == argument.place)
                                }))
                            .then_some((
                                *target,
                                ordinal,
                                argument.place,
                            ))
                        })
                });
                if let Some((target, ordinal, retired)) = transfer {
                    let block = machine
                        .blocks
                        .iter_mut()
                        .find(|block| block.id == target)
                        .unwrap();
                    let arrival = block.structural_parameters[ordinal].place;
                    for operation in &mut block.operations {
                        if let OperationKind::CallUnit {
                            structural_arguments,
                            ..
                        } = &mut operation.kind
                            && let Some(argument) = structural_arguments
                                .iter_mut()
                                .find(|argument| argument.place == arrival)
                        {
                            argument.place = retired;
                            altered = true;
                            break;
                        }
                    }
                }
            } else if mutation == 2 {
                let descriptors: Vec<_> = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter_map(|operation| match &operation.kind {
                        OperationKind::ElementViewSubslice { .. } => operation
                            .result
                            .structural()
                            .filter(|result| result.multiplicity == StructuralMultiplicity::Affine)
                            .map(|result| result.place),
                        _ => None,
                    })
                    .filter(|place| {
                        machine.blocks.iter().any(|block| {
                            matches!(&block.terminator,
                                Terminator::Jump { structural_arguments, .. }
                                    if structural_arguments.iter().any(|argument|
                                        argument.place == *place
                                            && argument.access == StructuralAccess::MutableBorrow))
                        })
                    })
                    .collect();
                for block in &mut machine.blocks {
                    if let Terminator::ReturnUnit {
                        trivial_affine_discards,
                        ..
                    } = &mut block.terminator
                        && let Some(position) = trivial_affine_discards
                            .iter()
                            .position(|place| descriptors.contains(place))
                    {
                        trivial_affine_discards.remove(position);
                        missing_cleanup = Some((machine.id, block.id));
                        altered = true;
                        break;
                    }
                }
            } else {
                for operation in machine
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                {
                    if mutation == 0
                        && matches!(operation.kind, OperationKind::ElementViewSubslice { .. })
                    {
                        let terminal_psi::OperationResult::Structural(result) =
                            &mut operation.result
                        else {
                            panic!("range result");
                        };
                        if result.multiplicity == StructuralMultiplicity::Affine {
                            result.multiplicity = StructuralMultiplicity::Unrestricted;
                            altered = true;
                            break;
                        }
                    } else if mutation == 1
                        && let OperationKind::EstablishMutableElementView {
                            source,
                            destination,
                            element,
                            region,
                        } = &operation.kind
                    {
                        let source = terminal_psi::StructuralArgument {
                            access: StructuralAccess::SharedBorrow,
                            ..source.clone()
                        };
                        operation.kind = OperationKind::EstablishElementView {
                            source,
                            destination: *destination,
                            element: *element,
                            region: *region,
                        };
                        let terminal_psi::OperationResult::Structural(result) =
                            &mut operation.result
                        else {
                            panic!("view result");
                        };
                        result.multiplicity = StructuralMultiplicity::Unrestricted;
                        altered = true;
                        break;
                    }
                }
            }
            if altered {
                break;
            }
        }
        assert!(
            altered,
            "mutation {mutation} changes an actual descriptor contract"
        );
        let error = terminal_verifier::validate_module(&module)
            .err()
            .expect("forged descriptor must fail source-free structural replay");
        if let Some((expected_machine, expected_block)) = missing_cleanup {
            assert!(
                matches!(error,
                terminal_verifier::ModuleError::UnitReturnAffineDiscardsMismatch { machine, block }
                    if machine == expected_machine && block == expected_block),
                "the carried state descriptor needs exact exit cleanup: {error:?}"
            );
        }
    }
}
