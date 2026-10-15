//! Case layout correspondence after exact operation and abstract custody replay.
use super::super::super::scalar_shape;
use super::super::BlockId;
use super::{AbstractOperationPlan, PsiOptimizationFunction, TargetControlGraph};
use crate::legalization::scalar_graph_input::target::control_flow::sources;
use semantic_vocabulary::PlaceId;
use target_operations::{
    TargetControlCaseSuccessor, TargetStructuralCaseSource, TargetStructuralHomeRequirement,
    TargetStructuralParameter,
};

pub(super) fn home_available(
    graph: &TargetControlGraph,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    block: BlockId,
    source: &TargetStructuralHomeRequirement,
    expected_source: PlaceId,
) -> bool {
    if source.place() != expected_source {
        return false;
    }
    // All producer rows are independently replayed by the caller. Presence in
    // the graph alone is insufficient: the result must precede this dispatch.
    match &source.origin {
        target_operations::TargetStructuralHomeOrigin::OperationResult { .. } => {
            let mut producers = graph.blocks.iter().flat_map(|candidate| {
                candidate.operations.iter().filter_map(move |operation| {
                    let (producer, result) = operation.structural_result()?;
                    if result.place != expected_source {
                        return None;
                    }
                    let expected = super::super::super::aggregate_results::result_home(
                        optimized,
                        expected_source,
                        plan,
                    )
                    .ok()?;
                    if !matches!(&expected.origin,
                        target_operations::TargetStructuralHomeOrigin::OperationResult {
                            operation, result: declaration,
                        } if *operation == producer && declaration == result)
                    {
                        return None;
                    }
                    // An eagerly allocated home must agree too. Descriptor
                    // producers retain the same exact result without claiming
                    // that their backing is a fresh allocation.
                    if operation
                        .structural_result_home()
                        .is_some_and(|(identity, home)| identity != producer || home != &expected)
                    {
                        return None;
                    }
                    Some((candidate.block, expected))
                })
            });
            let Some((producer_block, home)) = producers.next() else {
                return false;
            };
            if producers.next().is_some()
                || source != &home
                || (producer_block != block
                    && !sources::dominates(optimized, producer_block, block))
            {
                return false;
            }
        }
        target_operations::TargetStructuralHomeOrigin::BlockParameter {
            block: owner,
            declaration,
        } => {
            let Some(original) = optimized
                .block(*owner)
                .and_then(|candidate| candidate.structural_parameter(expected_source))
            else {
                return false;
            };
            let Some(target_parameter) = graph
                .blocks
                .iter()
                .find(|candidate| candidate.block == *owner)
                .and_then(|candidate| candidate.structural_parameter(expected_source))
            else {
                return false;
            };
            // A linear arrival is held at its block home exactly as an
            // operation result's linear value is: its claims are the
            // ownership the arriving edge moved, not storage of their own.
            if declaration != original
                || target_parameter != original
                || declaration.access != terminal_psi::StructuralAccess::Owned
                || (*owner != block && !sources::dominates(optimized, *owner, block))
            {
                return false;
            }
            if super::super::super::aggregate_results::block_home_layout(declaration, plan)
                .ok()
                .as_ref()
                != Some(&source.layout)
            {
                return false;
            }
        }
    }
    true
}

/// A parameter root is the function's own owned arrival: the retained row must
/// be the graph's exact prepared parameter, the source declaration must meet
/// the same owned-arrival contract a block home meets, and the sum layout must
/// equal the one derived independently from that declaration. The parameter is
/// live on entry, so it dominates every dispatch.
fn parameter_available(
    graph: &TargetControlGraph,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    parameter: &TargetStructuralParameter,
    layout: &target_operations::TargetStructuralHomeLayout,
    expected_source: PlaceId,
) -> bool {
    let Some(declaration) = optimized.structural_parameter(expected_source) else {
        return false;
    };
    parameter_corresponds(graph, parameter, declaration, expected_source)
        && super::super::super::aggregate_results::block_home_layout(declaration, plan)
            .ok()
            .as_ref()
            == Some(layout)
        && layout.shape() == parameter.shape
}

/// A readable borrow's referent stays caller-owned, so there is no block home
/// to reproduce: the retained parameter row, the clean declaration, and the
/// type-derived sum layout are the whole correspondence.
fn borrowed_parameter_available(
    graph: &TargetControlGraph,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    parameter: &TargetStructuralParameter,
    layout: &target_operations::TargetStructuralHomeLayout,
    expected_source: PlaceId,
) -> bool {
    let Some(declaration) = optimized.structural_parameter(expected_source) else {
        return false;
    };
    matches!(
        declaration.access,
        terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
    ) && declaration.multiplicity != terminal_psi::StructuralMultiplicity::Linear
        && parameter_corresponds(graph, parameter, declaration, expected_source)
        // The carrier is the pointer; the layout describes the referent.
        && parameter.shape.class == calling_conventions::ValueClass::BorrowedReference
        && super::super::super::aggregate_results::type_home_layout(
            parameter.structural_type,
            plan,
        )
        .ok()
        .as_ref()
            == Some(layout)
}

pub(super) fn matches(
    graph: &TargetControlGraph,
    optimized: &PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    block: BlockId,
    source: &TargetStructuralCaseSource,
    path: &[terminal_psi::StructuralPathSegment],
    byte_offset: u32,
    layout: &calling_conventions::ConventionalSumLayout,
    cases: &[TargetControlCaseSuccessor],
    expected_source: PlaceId,
    expected_cases: &[abstract_operations::AbstractStructuralCaseSuccessor],
) -> bool {
    let available = match source {
        TargetStructuralCaseSource::Home(home) => {
            home_available(graph, optimized, plan, block, home, expected_source)
        }
        TargetStructuralCaseSource::Parameter { parameter, layout } => {
            parameter_available(graph, optimized, plan, parameter, layout, expected_source)
        }
        TargetStructuralCaseSource::BorrowedParameter { parameter, layout } => {
            borrowed_parameter_available(graph, optimized, plan, parameter, layout, expected_source)
        }
        TargetStructuralCaseSource::BorrowedBlockParameter { parameter, layout } => {
            // The block's own declared parameter row must survive optimization
            // untouched: the dispatch replays it against the exact join
            // declaration, with the referent's sum layout derived by type.
            let Some(declaration) = optimized
                .block_structural_parameters()
                .find(|declaration| declaration.place == expected_source)
            else {
                return false;
            };
            matches!(
                declaration.access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            ) && declaration.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                && parameter.place == expected_source
                && parameter.structural_type == declaration.structural_type
                && parameter.access == declaration.access
                && parameter.multiplicity == declaration.multiplicity
                && parameter.projected_qualifications == declaration.projected_qualifications
                && super::super::super::aggregate_results::type_home_layout(
                    parameter.structural_type,
                    plan,
                )
                .ok()
                .as_ref()
                    == Some(layout)
        }
    };
    if !available {
        return false;
    }
    let Ok((selected, expected_offset, _, expected_layout)) =
        super::super::super::structural_case::dispatch_projection(
            optimized,
            expected_source,
            path,
            plan,
        )
    else {
        return false;
    };
    if expected_offset != byte_offset || expected_layout != *layout {
        return false;
    }
    let Some(declaration) = terminal_psi::find_structural_type(&graph.structural_types, selected)
    else {
        return false;
    };
    let declared_cases: &[terminal_psi::StructuralCaseDeclaration] = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases }
        | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return false,
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
                || actual.cleanup_actions != expected.cleanup_actions
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
                        && fields[field_position].field_type.scalar_type()
                            == Some(expected.scalar_type)
                        && scalar_shape(expected.scalar_type) == Some(field_layout.shape)
                        && byte_offset.checked_add(field_layout.byte_offset)
                            == Some(actual.field_byte_offset)
                })
        })
}

/// The retained `parameter` is the unique graph row for `expected_source`
/// and spells `declaration`'s type, access, multiplicity, and projections.
fn parameter_corresponds(
    graph: &TargetControlGraph,
    parameter: &TargetStructuralParameter,
    declaration: &terminal_psi::StructuralParameterDeclaration,
    expected_source: PlaceId,
) -> bool {
    parameter.place == expected_source
        && graph
            .parameters
            .iter()
            .filter(|candidate| candidate.place == expected_source)
            .eq(std::iter::once(parameter))
        && parameter.structural_type == declaration.structural_type
        && parameter.access == declaration.access
        && parameter.multiplicity == declaration.multiplicity
        // Preparation preserves the source roster verbatim. Compare it rather
        // than refusing qualified children; dispatch does not grant new facts.
        && parameter.projected_qualifications == declaration.projected_qualifications
}
