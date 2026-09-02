//! Exact structural scalar-field preservation after independent Terminal verification.

use omega_abstract_operations::{AbstractOperation, AbstractResult};
use psi_core::{PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind, StructuralTypeId};
use psi_terminal::{
    BindingRelevance, Block, Operation, OperationKind, OperationResult, StructuralAccess,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

pub(super) fn lower(
    operation: &Operation,
    block: &Block,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<AbstractOperation, LoweringError> {
    match &operation.kind {
        OperationKind::StructuralScalarFieldStore {
            destination,
            path,
            field,
            value,
        } => lower_store(
            operation,
            block,
            machine,
            structural_types,
            *destination,
            path,
            *field,
            *value,
        ),
        OperationKind::IntegerStructuralField { source, field } => {
            lower_integer_read(operation, machine, structural_types, *source, *field)
        }
        _ => unreachable!("structural scalar-field router is exhaustive"),
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_store(
    operation: &Operation,
    block: &Block,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    destination: PlaceId,
    path: &[StructuralPathSegment],
    field: StructuralFieldId,
    value: psi_core::ValueId,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidStructuralScalarFieldStore(operation.id);
    let destination = exact_parameter(machine, destination).ok_or_else(invalid)?;
    let scalar_type =
        dominating_scalar_type(machine, block, operation.id, value).ok_or_else(invalid)?;
    if operation.result != OperationResult::Unit
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !has_empty_structural_custody(machine, destination.place)
        || !matches!(path, [StructuralPathSegment::Field(_)])
    {
        return Err(invalid());
    }
    let parent_type = resolve_structural_path(structural_types, destination.structural_type, path)
        .ok_or_else(invalid)?;
    if direct_relevant_scalar_field(structural_types, parent_type, field) != Some(scalar_type) {
        return Err(invalid());
    }
    Ok(AbstractOperation::StructuralScalarFieldStore {
        psi_operation: operation.id,
        destination,
        path: path.to_vec(),
        field,
        value: AbstractResult { value, scalar_type },
    })
}

fn lower_integer_read(
    operation: &Operation,
    machine: &TerminalMachine,
    structural_types: &[StructuralTypeDeclaration],
    source: PlaceId,
    field: StructuralFieldId,
) -> Result<AbstractOperation, LoweringError> {
    let invalid = || LoweringError::InvalidIntegerStructuralField(operation.id);
    let source = exact_parameter(machine, source).ok_or_else(invalid)?;
    let result = operation.result.scalar().ok_or_else(invalid)?;
    if !matches!(
        source.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !matches!(
        source.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || !has_empty_structural_custody(machine, source.place)
        || !matches!(result.scalar_type, ScalarType::Integer(_))
        || direct_relevant_scalar_field(structural_types, source.structural_type, field)
            != Some(result.scalar_type)
    {
        return Err(invalid());
    }
    Ok(AbstractOperation::IntegerStructuralField {
        psi_operation: operation.id,
        result: AbstractResult {
            value: result.id,
            scalar_type: result.scalar_type,
        },
        source,
        field,
    })
}

fn exact_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<StructuralParameterDeclaration> {
    let mut parameters = machine
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.place == place);
    let parameter = parameters.next()?;
    if parameters.next().is_some() {
        return None;
    }
    let mut places = machine
        .structural_places
        .iter()
        .filter(|declaration| declaration.id == place);
    let declaration = places.next()?;
    if places.next().is_some()
        || !matches!(
            declaration.kind,
            StructuralPlaceKind::Parameter { position, is_self }
                if position == parameter.position && is_self == parameter.is_self
        )
    {
        return None;
    }
    Some(parameter.clone())
}

fn dominating_scalar_type(
    machine: &TerminalMachine,
    block: &Block,
    operation: psi_core::OperationId,
    value: psi_core::ValueId,
) -> Option<ScalarType> {
    let entry_declarations = machine
        .parameters
        .iter()
        .chain(block.parameters.iter())
        .filter(|declaration| declaration.id == value)
        .map(|declaration| declaration.scalar_type)
        .collect::<Vec<_>>();
    if let [scalar_type] = entry_declarations.as_slice() {
        return Some(*scalar_type);
    }
    if !entry_declarations.is_empty() {
        return None;
    }

    let mut definition = None;
    let mut found_operation = false;
    for candidate in &block.operations {
        if candidate.id == operation {
            if found_operation {
                return None;
            }
            found_operation = true;
            continue;
        }
        if !found_operation
            && let Some(result) = candidate.result.scalar()
            && result.id == value
            && definition.replace(result.scalar_type).is_some()
        {
            return None;
        }
    }
    found_operation.then_some(definition).flatten()
}

fn has_empty_structural_custody(machine: &TerminalMachine, place: PlaceId) -> bool {
    machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
        .is_some_and(|parameter| {
            parameter.qualifications.is_empty() && parameter.projected_qualifications.is_empty()
        })
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place)
}

fn resolve_structural_path(
    structural_types: &[StructuralTypeDeclaration],
    mut structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    for segment in path {
        let declaration = exact_structural_type(structural_types, structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
                let mut matching = fields.iter().filter(|field| {
                    field.identity == *identity && field.relevance == BindingRelevance::Relevant
                });
                let field = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                StructuralPathSegment::FixedIndex(index),
                StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

fn direct_relevant_scalar_field(
    structural_types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
    field: StructuralFieldId,
) -> Option<ScalarType> {
    let declaration = exact_structural_type(structural_types, structural_type)?;
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        return None;
    };
    let mut matching = fields.iter().filter(|candidate| {
        candidate.id == field && candidate.relevance == BindingRelevance::Relevant
    });
    let field = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match &field.field_type {
        StructuralFieldType::Scalar(scalar_type) => Some(*scalar_type),
        StructuralFieldType::IeeeFloat(_)
        | StructuralFieldType::ByteSequence(_)
        | StructuralFieldType::Structural(_)
        | StructuralFieldType::Erased { .. } => None,
    }
}

fn exact_structural_type(
    structural_types: &[StructuralTypeDeclaration],
    structural_type: StructuralTypeId,
) -> Option<&StructuralTypeDeclaration> {
    let mut matching = structural_types
        .iter()
        .filter(|declaration| declaration.id == structural_type);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}
