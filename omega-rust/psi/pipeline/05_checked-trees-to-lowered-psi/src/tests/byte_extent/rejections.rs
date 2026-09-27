//! Rejections for the derived equation at the added Terminal length read.

use super::super::LoweringError;
use super::{
    IntegerValue, LoweredPsi, OperationKind, OperationResult, PrimitiveType, fixture,
    terminal_scalar_type,
};
use semantic_vocabulary::{Proposition, ScalarTerm, StructuralPlaceKind};
use terminal_psi::TerminalMachine;
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
            matches!(crate::proofs::operation_proofs::finalize_operation_proofs(&mut lowered),
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
