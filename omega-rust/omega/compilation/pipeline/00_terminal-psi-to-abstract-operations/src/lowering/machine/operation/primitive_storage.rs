//! Retain primitive storage identity separately from scalar observations.
//! Reads reconstruct one declared primitive projection without transferring
//! its owner's claims, cleanup or descriptor loans. Whole-carrier copy rules
//! cannot classify that observation; Psi has already checked its availability.
use std::collections::BTreeMap;

use abstract_operations::{AbstractOperation, AbstractResult};
use semantic_vocabulary::{PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralMultiplicity, StructuralOperationResult,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
};

use crate::lowering::LoweringError;

/// Resolve the declared parameter independently of its physical carrier.
/// State parameters retain their own block/place correspondence, never an entry
/// parameter's position. The observing operation checks access, custody and its
/// projection; being a native address join is not a semantic identity condition.
pub(super) fn parameter<'a>(
    machine: &'a TerminalMachine,
    place: PlaceId,
) -> Option<&'a terminal_psi::StructuralParameterDeclaration> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|row| row.place == place)
    {
        return Some(parameter);
    }
    let declaration = machine.structural_place(place)?;
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    let parameter = machine
        .block(&block)?
        .structural_parameters
        .get(position as usize)?;
    (parameter.place == place && parameter.position == position).then_some(parameter)
}

/// Resolve an exact owned block home for scalar-field mutation. Live linear
/// claims are preserved by a field store; they do not prohibit exclusive
/// mutation. Membership and Content restoration remain separate.
pub(super) fn owned_state_storage_parameter(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<(
    semantic_vocabulary::BlockId,
    &terminal_psi::StructuralParameterDeclaration,
)> {
    let mut declarations = machine
        .structural_places
        .iter()
        .filter(|row| row.id == place);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    let parameter = machine
        .block(&block)?
        .structural_parameters
        .get(position as usize)?;
    (parameter.place == place
        && parameter.position == position
        && parameter.access == StructuralAccess::Owned)
        .then_some((block, parameter))
}

pub(super) fn local(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<&StructuralOperationResult> {
    let (operation, result) = owned_result(machine, place)?;
    (result.multiplicity == StructuralMultiplicity::Unrestricted
        && matches!(
            operation.kind,
            OperationKind::EstablishPrimitiveLocal { .. }
        ))
    .then_some(result)
}

/// A projected read observes the producer's existing owned storage. Rejoin
/// the declared operation result, not an ABI parameter or a fresh copy. The
/// Terminal verifier separately checks availability and outstanding loans.
fn owned_result(
    machine: &TerminalMachine,
    place: PlaceId,
) -> Option<(&Operation, &StructuralOperationResult)> {
    let mut declarations = machine
        .structural_places
        .iter()
        .filter(|entry| entry.id == place);
    let declaration = declarations.next()?;
    if declarations.next().is_some() {
        return None;
    }
    let (producer, structural_type) = declaration.kind.as_operation_result()?;
    let mut operations = machine
        .operations()
        .filter(|operation| operation.id == producer);
    let operation = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place && result.structural_type == structural_type)
        .then_some((operation, result))
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
        OperationKind::EstablishPrimitiveLocal {
            value,
            range_obligation,
        } => {
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
                range_obligation,
            })
        }
        OperationKind::PrimitiveScalarRead { source, ref path } => {
            let invalid = || LoweringError::InvalidPrimitiveScalarRead(operation.id);
            let result = operation.result.scalar().ok_or_else(invalid)?;
            let identity = local(machine, source)
                .map(|local| local.structural_type)
                .or_else(|| {
                    (!path.is_empty())
                        .then(|| owned_result(machine, source))
                        .flatten()
                        .map(|(_, result)| result.structural_type)
                })
                .or_else(|| {
                    parameter(machine, source)
                        .filter(|parameter| {
                            // Observing a primitive leaf keeps the owner's
                            // claims, cleanup and refinements at its home.
                            // Neither linear ownership nor stored descriptor
                            // loans turn a read into a whole-value copy. Psi
                            // owns availability; this stage retains readable
                            // access and reconstructs the exact leaf geometry.
                            matches!(
                                parameter.access,
                                terminal_psi::reading_borrow_pair_pattern!()
                            ) || parameter.access == StructuralAccess::Owned
                        })
                        .map(|parameter| parameter.structural_type)
                })
                .ok_or_else(invalid)?;
            if super::primitive_projection::read_leaf_type(types, identity, path, |index| {
                values.get(&index).copied()
            }) != Some(result.scalar_type)
            {
                return Err(invalid());
            }
            Ok(AbstractOperation::PrimitiveScalarRead {
                psi_operation: operation.id,
                source,
                path: path.clone(),
                result: result.into(),
            })
        }
        _ => Err(LoweringError::InvalidPrimitiveLocalEstablishment(
            operation.id,
        )),
    }
}
