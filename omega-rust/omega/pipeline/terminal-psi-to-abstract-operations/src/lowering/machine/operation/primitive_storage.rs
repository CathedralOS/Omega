//! Retain primitive storage identity separately from scalar observations.
use std::collections::BTreeMap;

use abstract_operations::{AbstractOperation, AbstractResult};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

pub(super) fn local(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralOperationResult> {
    let declaration = machine
        .structural_places
        .iter()
        .find(|entry| entry.id == place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let operation = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?;
    let result = operation.result.structural()?;
    (matches!(
        operation.kind,
        OperationKind::EstablishPrimitiveLocal { .. }
    ) && result.place == place
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && machine
            .entry_claims
            .iter()
            .all(|claim| claim.input != place)
        && machine
            .content_entry_claims
            .iter()
            .all(|claim| claim.input.root != place))
    .then_some(result)
}

pub(super) fn scalar_type(
    types: &[StructuralTypeDeclaration],
    identity: StructuralTypeId,
) -> Option<ScalarType> {
    types
        .iter()
        .find_map(|declaration| match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar_type) if declaration.id == identity => {
                Some(scalar_type)
            }
            _ => None,
        })
}

pub(super) fn lower(
    operation: &Operation,
    machine: &TerminalMachine,
    types: &[StructuralTypeDeclaration],
    values: &BTreeMap<ValueId, ScalarType>,
) -> Result<AbstractOperation, LoweringError> {
    match operation.kind {
        OperationKind::EstablishPrimitiveLocal { value } => {
            let invalid = || LoweringError::InvalidPrimitiveLocalEstablishment(operation.id);
            let result = operation.result.structural().ok_or_else(invalid)?;
            if local(machine, result.place) != Some(result) {
                return Err(invalid());
            }
            let scalar_type = scalar_type(types, result.structural_type).ok_or_else(invalid)?;
            if values.get(&value) != Some(&scalar_type) {
                return Err(invalid());
            }
            Ok(AbstractOperation::EstablishPrimitiveLocal {
                psi_operation: operation.id,
                result: result.clone(),
                value: AbstractResult { value, scalar_type },
            })
        }
        OperationKind::PrimitiveScalarRead { source } => {
            let invalid = || LoweringError::InvalidPrimitiveScalarRead(operation.id);
            let result = operation.result.scalar().ok_or_else(invalid)?;
            let identity = local(machine, source)
                .map(|local| local.structural_type)
                .or_else(|| {
                    machine
                        .structural_parameters
                        .iter()
                        .find(|parameter| {
                            parameter.place == source
                                && matches!(
                                    parameter.access,
                                    StructuralAccess::SharedBorrow
                                        | StructuralAccess::MutableBorrow
                                )
                                && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                && parameter.qualifications.is_empty()
                                && parameter.projected_qualifications.is_empty()
                        })
                        .map(|parameter| parameter.structural_type)
                })
                .ok_or_else(invalid)?;
            if scalar_type(types, identity) != Some(result.scalar_type) {
                return Err(invalid());
            }
            Ok(AbstractOperation::PrimitiveScalarRead {
                psi_operation: operation.id,
                source,
                result: AbstractResult {
                    value: result.id,
                    scalar_type: result.scalar_type,
                },
            })
        }
        _ => Err(LoweringError::InvalidPrimitiveLocalEstablishment(
            operation.id,
        )),
    }
}
