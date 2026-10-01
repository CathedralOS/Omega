//! Ordinary byte completions retain their owned carrier and declared predicates
//! when the optimizer independently replays the current graph.
use crate::tests::indexed_byte_fields::indexed_field_source_unit;
use crate::tests::support::refresh_node_derivatives;
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use optimization_unit::PsiOptimizationUnit;
use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};

fn owned_byte_call_unit() -> PsiOptimizationUnit {
    indexed_field_source_unit(
        r#"
        domain [u8;16]::Utf8 requires valid_utf8(self);
        data Record { text: [u8;16] in Utf8; }
        machine label(short: bool) -> [u8;16] in Utf8 {
            transition short { true -> "Gate" false -> "Branch Room" }
        }
        machine Record::replace(&mut self) {
            self.text = label(false);
            self.text = label(true);
            transition { _ -> finish() }
            state finish() {}
        }
        "#,
    )
}

#[test]
fn owned_byte_call_results_replay_predicates_and_return_contracts() {
    let unit = owned_byte_call_unit();
    validate_psi_optimization_unit(&unit)
        .expect("owned completion, measured length, and field replacement replay");
    assert!(
        unit.functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.nodes)
            .any(|node| matches!(
                node.operation,
                AbstractOperation::EstablishByteSequenceConcatenation { .. }
            ))
    );
}

#[test]
fn owned_byte_call_results_reject_changed_qualification_and_carrier() {
    let baseline = owned_byte_call_unit();
    for corruption in [
        "callee qualification",
        "caller qualification",
        "borrowed carrier",
    ] {
        let mut unit = baseline.clone();
        if corruption == "borrowed carrier" {
            let mut declarations = unit.structural_types.to_vec();
            let declaration = declarations
                .iter_mut()
                .find(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                            capacity: 16
                        })
                    )
                })
                .expect("owned return carrier");
            declaration.shape =
                StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access: None,
                });
            unit.structural_types = declarations.into();
        } else {
            let result =
                unit.functions
                    .iter_mut()
                    .flat_map(|function| &mut function.blocks)
                    .flat_map(|block| &mut block.nodes)
                    .find_map(|node| match &mut node.operation {
                        AbstractOperation::EstablishByteSequenceConcatenation {
                            result, ..
                        } if corruption == "callee qualification" => Some(result),
                        AbstractOperation::CallStructural {
                            result: AbstractStructuralCallResult::Structural { result, .. },
                            ..
                        } if corruption == "caller qualification" => Some(result),
                        _ => None,
                    })
                    .expect("owned producer result");
            assert!(!result.qualifications.is_empty(), "{corruption}");
            result.qualifications.clear();
        }
        // Reject semantic corruption, not just stale cached node metadata or
        // the unit identity left over from the admitted graph.
        for function_index in 0..unit.functions.len() {
            for block_index in 0..unit.functions[function_index].blocks.len() {
                for node_index in 0..unit.functions[function_index].blocks[block_index]
                    .nodes
                    .len()
                {
                    refresh_node_derivatives(&mut unit, function_index, block_index, node_index);
                }
            }
        }
        assert!(
            validate_psi_optimization_unit(&unit).is_err(),
            "{corruption}"
        );
    }
}
