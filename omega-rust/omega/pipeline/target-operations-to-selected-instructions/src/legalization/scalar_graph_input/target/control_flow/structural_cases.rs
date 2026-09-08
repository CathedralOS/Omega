//! Case layout correspondence after exact operation and abstract custody replay.

use super::*;
use semantic_vocabulary::PlaceId;
use target_operations::{
    TargetBoundaryResult, TargetControlCaseSuccessor, TargetStructuralHomeRequirement,
};

pub(super) fn matches(
    graph: &TargetControlGraph,
    optimized: &PsiOptimizationFunction,
    block: BlockId,
    source: &TargetStructuralHomeRequirement,
    cases: &[TargetControlCaseSuccessor],
    expected_source: PlaceId,
    expected_cases: &[abstract_operations::AbstractStructuralCaseSuccessor],
) -> bool {
    if source.result.place != expected_source {
        return false;
    }
    // All producer rows are independently replayed by the caller. Presence in
    // the graph alone is insufficient: the result must precede this dispatch.
    let mut producers = graph.blocks.iter().flat_map(|candidate| {
        candidate
            .operations
            .iter()
            .filter_map(move |operation| match operation {
                TargetUnitOperation::BoundarySettlement {
                    result: TargetBoundaryResult::Structural(home),
                    ..
                } if home.result.place == expected_source => Some((candidate.block, home)),
                _ => None,
            })
    });
    let Some((producer_block, home)) = producers.next() else {
        return false;
    };
    if producers.next().is_some()
        || source != home
        || (producer_block != block && !sources::dominates(optimized, producer_block, block))
    {
        return false;
    }
    let Some(declaration) = graph
        .structural_types
        .iter()
        .find(|declaration| declaration.id == source.result.structural_type)
    else {
        return false;
    };
    let terminal_psi::StructuralTypeShape::Sum {
        cases: declared_cases,
    } = &declaration.shape
    else {
        return false;
    };
    let Some(layout) = source.layout.sum() else {
        return false;
    };
    if cases.len() != expected_cases.len()
        || cases.len() != declared_cases.len()
        || cases.len() != layout.cases.len()
    {
        return false;
    }
    cases
        .iter()
        .zip(expected_cases)
        .zip(declared_cases)
        .enumerate()
        .all(|(case_position, ((actual, expected), declared))| {
            let Some(target) = graph
                .blocks
                .iter()
                .find(|block| block.block == expected.target)
            else {
                return false;
            };
            if actual.psi_edge != expected.psi_edge
                || actual.target != expected.target
                || actual.case != expected.case
                || actual.case != declared.id
                || i32::try_from(case_position).ok() != Some(actual.case_tag)
                || actual.trivial_affine_discards != expected.trivial_affine_discards
                || actual.payloads.len() != expected.payloads.len()
                || actual.payloads.len() != target.parameters.len()
                || !target.structural_parameters.is_empty()
            {
                return false;
            }
            let fields = declared
                .fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect::<Vec<_>>();
            if fields.len() != layout.cases[case_position].fields.len() {
                return false;
            }
            actual
                .payloads
                .iter()
                .zip(&expected.payloads)
                .zip(&target.parameters)
                .all(|((actual, expected), parameter)| {
                    let Some(field_position) =
                        fields.iter().position(|field| field.id == expected.field)
                    else {
                        return false;
                    };
                    let field_layout = layout.cases[case_position].fields[field_position];
                    actual.field == expected.field
                        && actual.parameter.block == target.block
                        && actual.parameter.value == expected.parameter
                        && actual.parameter.value == parameter.value
                        && actual.parameter.scalar_type == expected.scalar_type
                        && actual.parameter.scalar_type == parameter.scalar_type
                        && fields[field_position].field_type
                            == terminal_psi::StructuralFieldType::Scalar(expected.scalar_type)
                        && scalar_shape(expected.scalar_type) == Some(field_layout.shape)
                        && actual.field_byte_offset == u32::from(field_layout.byte_offset)
                })
        })
}
