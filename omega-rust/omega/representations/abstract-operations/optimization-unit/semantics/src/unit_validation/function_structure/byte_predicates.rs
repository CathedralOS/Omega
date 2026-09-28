//! Replay declared byte invariants against the current optimized operands.
//! A retained capacity fact cannot authorize replacing a qualified destination
//! with a different source of equal length but incompatible byte content.
use abstract_operations::AbstractOperation as Operation;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{PlaceId, StructuralPlaceKind};
use terminal_psi::{
    ByteSequencePredicate, ByteSequencePredicateSet, StructuralAccess, StructuralTypeShape,
};

pub(crate) fn replacement_preserves_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    operation: &Operation,
) -> bool {
    let (source, required) = match operation {
        Operation::ByteSequenceStore {
            destination,
            source,
            ..
        } => {
            let Some(parameter) = function
                .structural_parameters
                .iter()
                .find(|parameter| parameter.place == *destination)
            else {
                return false;
            };
            (
                *source,
                terminal_semantics::byte_parameter_predicates_from_domains(
                    &unit.structural_domains,
                    parameter,
                ),
            )
        }
        Operation::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            ..
        } => (
            *source,
            field_predicates(unit, function, *destination, path, *field),
        ),
        _ => return false,
    };
    let Some(required) = required else {
        return false;
    };
    required.is_empty()
        || source_predicates(
            unit,
            function,
            source,
            function.structural_places.len().saturating_add(1),
        )
        .is_some_and(|available| available.implies(required))
}

fn field_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    root: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<ByteSequencePredicateSet> {
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == root)?;
    let declaration = terminal_semantics::runtime_structural_path_tip(
        unit.structural_types.iter(),
        parameter.structural_type,
        path,
    )?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let field = fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?;
    field.field_type.byte_sequence_carrier()?;
    Some(field.field_type.byte_predicates())
}

fn source_predicates(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    source: PlaceId,
    remaining: usize,
) -> Option<ByteSequencePredicateSet> {
    let remaining = remaining.checked_sub(1)?;
    let declaration = function
        .structural_places
        .iter()
        .find(|place| place.id == source)?;
    match declaration.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            let parameter = function.structural_parameters.iter().find(|parameter| {
                parameter.place == source
                    && parameter.position == position
                    && parameter.is_self == is_self
            })?;
            if !matches!(
                parameter.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) {
                return None;
            }
            terminal_semantics::byte_parameter_predicates_from_domains(
                &unit.structural_domains,
                parameter,
            )
        }
        StructuralPlaceKind::ByteSequenceLiteral { .. } => {
            let mut producers = function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter_map(|node| match &node.operation {
                    Operation::EstablishByteSequenceLiteral { place, bytes, .. }
                        if place.id == source =>
                    {
                        Some(bytes)
                    }
                    _ => None,
                });
            let bytes = producers.next()?;
            if producers.next().is_some() {
                return None;
            }
            let mut predicates = ByteSequencePredicateSet::EMPTY;
            for predicate in ByteSequencePredicate::ALL {
                if predicate.holds_for(bytes) {
                    predicates.insert(predicate);
                }
            }
            Some(predicates)
        }
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let node = function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .find(|node| node.operation.psi_operation() == Some(producer))?;
            let Operation::EstablishByteSequenceConcatenation {
                result,
                left,
                right,
                ..
            } = &node.operation
            else {
                return None;
            };
            if result.place != source || result.structural_type != structural_type {
                return None;
            }
            let operand = |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand
            {
                terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                    source_predicates(unit, function, *source, remaining)
                }
                terminal_psi::control_flow::ByteSequenceOperand::Field {
                    source, field, ..
                } => {
                    if source.access != StructuralAccess::SharedBorrow {
                        return None;
                    }
                    field_predicates(unit, function, source.place, &source.path, *field)
                }
            };
            Some(operand(left)?.concatenation(operand(right)?))
        }
        _ => None,
    }
}
