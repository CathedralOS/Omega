//! Rejections for the derived equation at the added Terminal length read.

use super::*;

fn slice_machine(lowered: &mut LoweredPsi) -> &mut TerminalMachine {
    lowered
        .semantic_module
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                })
        })
        .expect("fixture subslice machine")
}

#[test]
fn unchanged_view_has_no_strict_descent_and_positive_prefix_still_needs_bounds() {
    for prefix in [0, 2] {
        let mut lowered = fixture();
        let machine = slice_machine(&mut lowered);
        let (start, slice_obligation) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ByteSequenceSubslice {
                    start, obligation, ..
                } => Some((start, obligation)),
                _ => None,
            })
            .unwrap();
        let (read_obligation, extent) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ByteSequenceRead {
                    obligation, index, ..
                } => Some((obligation, index)),
                _ => None,
            })
            .unwrap();
        let definition = machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| {
                operation
                    .result
                    .scalar()
                    .is_some_and(|value| value.id == start)
            })
            .unwrap();
        assert!(matches!(
            definition.kind,
            OperationKind::IntegerConstant {
                value: IntegerValue::Unsigned(1)
            }
        ));
        definition.kind = OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(prefix),
        };

        // The equation exists at the read, but cannot discharge earlier slice bounds.
        let sites =
            terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).unwrap();
        let measured = ScalarTerm::value(extent, terminal_scalar_type(PrimitiveType::U64).unwrap());
        for (obligation, has_equation) in [(slice_obligation, false), (read_obligation, true)] {
            let site = sites
                .iter()
                .find(|site| site.obligation.id == obligation)
                .unwrap();
            assert_eq!(site.semantic_axioms.iter().any(|fact| matches!(fact,
                Proposition::Equal(result, ScalarTerm::ExactIntegerSubtract { .. }) if *result == measured
            )), has_equation);
        }
        let expected = if prefix == 0 {
            read_obligation
        } else {
            slice_obligation
        };
        assert!(
            matches!(crate::operation_emission::finalize_operation_proofs(&mut lowered),
                Err(LoweringError::OperationProofUnavailable(obligation)) if obligation == expected
            ),
            "prefix {prefix} must fail its own obligation"
        );
    }
}

#[test]
fn derived_length_requires_the_exact_defining_result() {
    for redirect_producer in [false, true] {
        let mut lowered = fixture();
        terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
        let machine = slice_machine(&mut lowered);
        let slice = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
            .unwrap();
        let tail = slice.result.structural().unwrap().place;
        let length_read = machine.blocks.iter().flat_map(|block| &block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceLength { source } if source == tail))
            .unwrap().id;
        if redirect_producer {
            let declaration = machine
                .structural_places
                .iter_mut()
                .find(|place| place.id == tail)
                .unwrap();
            let StructuralPlaceKind::OperationResult { producer, .. } = &mut declaration.kind
            else {
                panic!("subslice result declaration");
            };
            *producer = length_read;
        } else {
            let operation = machine
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| operation.id == length_read)
                .unwrap();
            let OperationResult::Scalar(result) = &mut operation.result else {
                panic!("length result");
            };
            result.scalar_type = terminal_scalar_type(PrimitiveType::U8).unwrap();
        }
        assert!(
            terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).is_err()
        );
    }
}

#[test]
fn new_length_read_cannot_precede_its_producer_or_escape_to_a_sibling() {
    for sibling in [false, true] {
        let mut lowered = fixture();
        terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
        let operations = || {
            lowered
                .semantic_module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
        };
        let fresh_operation = operation_id(
            operations()
                .map(|operation| operation.id.get())
                .max()
                .unwrap()
                + 1,
        );
        let fresh_value = value_id(
            operations()
                .filter_map(|operation| operation.result.scalar().map(|value| value.id.get()))
                .max()
                .unwrap()
                + 1,
        );
        let machine = slice_machine(&mut lowered);
        let block_position = machine
            .blocks
            .iter()
            .position(|block| {
                block.operations.iter().any(|operation| {
                    matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
                })
            })
            .unwrap();
        let block = &mut machine.blocks[block_position];
        let slice_position = block
            .operations
            .iter()
            .position(|operation| {
                matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. })
            })
            .unwrap();
        let tail = block.operations[slice_position]
            .result
            .structural()
            .unwrap()
            .place;
        let length_position = block.operations.iter().position(|operation| {
            matches!(operation.kind, OperationKind::ByteSequenceLength { source } if source == tail)
        }).unwrap();
        let mut read = block.operations[length_position].clone();
        if sibling {
            // Keep the original definition so its consumers remain valid.
            read.id = fresh_operation;
            let OperationResult::Scalar(result) = &mut read.result else {
                panic!("length result")
            };
            result.id = fresh_value;
            let sibling_id = machine
                .blocks
                .iter()
                .find_map(|block| match &block.terminator {
                    Terminator::Conditional {
                        when_true,
                        when_false,
                        ..
                    } if when_true.target == machine.blocks[block_position].id => {
                        Some(when_false.target)
                    }
                    _ => None,
                })
                .expect("selected tail has an empty sibling");
            machine
                .blocks
                .iter_mut()
                .find(|block| block.id == sibling_id)
                .unwrap()
                .operations
                .insert(0, read.clone());
        } else {
            block.operations.remove(length_position);
            block.operations.insert(slice_position, read.clone());
        }
        assert!(
            matches!(terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module),
                Err(terminal_verifier::ModuleError::ByteSequenceViewNotEstablished { operation, place })
                    if operation == read.id && place == tail
            )
        );
    }
}

#[test]
fn frozen_extent_proof_rejects_endpoint_and_measurement_source_drift() {
    let mut lowered = fixture();
    crate::operation_emission::finalize_operation_proofs(&mut lowered)
        .expect("green strict descent fixture");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile)
        .unwrap();
    let original = lowered.semantic_module.clone();
    let frozen = lowered.proof_bundle.clone();
    for redirect_measurement in [false, true] {
        lowered.semantic_module = original.clone();
        lowered.proof_bundle = frozen.clone();
        let machine = slice_machine(&mut lowered);
        let read_obligation = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation.kind {
                OperationKind::ByteSequenceRead { obligation, .. } => Some(obligation),
                _ => None,
            })
            .unwrap();
        let slice = machine
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceSubslice { .. }))
            .unwrap();
        let tail = slice.result.structural().unwrap().place;
        let OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            obligation,
            ..
        } = &mut slice.kind
        else {
            panic!("subslice");
        };
        let slice_obligation = *obligation;
        let original_source = *source;
        if redirect_measurement {
            let read = machine.blocks.iter_mut().flat_map(|block| &mut block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::ByteSequenceLength { source } if source == tail))
                .unwrap();
            read.kind = OperationKind::ByteSequenceLength {
                source: original_source,
            };
        } else {
            *end = *start;
        }
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("mutation remains structurally valid");
        assert!(
            terminal_verifier::verify_module(
                &lowered.semantic_module,
                &lowered.proof_bundle,
                &profile
            )
            .is_err()
        );
        // Repair only valid slice bounds: the frozen read must still reject its changed equation.
        lowered
            .proof_bundle
            .evidence
            .retain(|evidence| evidence.obligation != slice_obligation);
        crate::operation_emission::finalize_operation_proofs(&mut lowered)
            .expect("changed slice bounds remain provable");
        assert!(
            matches!(terminal_verifier::verify_module(&lowered.semantic_module, &lowered.proof_bundle, &profile),
                Err(terminal_verifier::VerificationError::RejectedEvidence { obligation, .. }) if obligation == read_obligation
            )
        );
    }
}
