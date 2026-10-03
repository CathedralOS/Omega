//! Retain primitive storage identity separately from scalar observations.
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
    let declaration = machine
        .structural_places
        .iter()
        .find(|row| row.id == place)?;
    let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
        return None;
    };
    let parameter = machine
        .blocks
        .iter()
        .find(|row| row.id == block)?
        .structural_parameters
        .get(position as usize)?;
    (parameter.place == place && parameter.position == position)
    .then_some(parameter)
}

/// A state's owned structural parameter: the value arrives whole on its block
/// edge and lives in the block's own value storage, exactly as the root a
/// structural field read observes. Only a projected read below it is a
/// primitive leaf; the whole owned record never is. The owning block is the
/// home's establishment identity, which a field store retains.
pub(super) fn owned_state_parameter(
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
        .blocks
        .iter()
        .find(|row| row.id == block)?
        .structural_parameters
        .get(position as usize)?;
    (parameter.place == place
        && parameter.position == position
        && parameter.access == StructuralAccess::Owned
        && matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
        && parameter.qualifications.is_empty()
        && parameter.projected_qualifications.is_empty()
        && super::structural_scalar_fields::has_empty_structural_custody(machine, place))
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
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = declaration.kind
    else {
        return None;
    };
    let mut operations = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == producer);
    let operation = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    let result = operation.result.structural()?;
    (result.place == place
        && result.structural_type == structural_type
        && matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        )
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

/// The scalar type of one runtime selector a primitive read carries.
///
/// A selector declared in the read's own block (a block parameter or an
/// earlier result) or as a machine parameter keeps the in-block rule: it must
/// precede the read. A selector declared in another block is the common
/// shape of a state's scalar parameter read inside a conditional arm: the
/// arm's successor block reads `self.a[k]` while `k` is the state block's
/// parameter. Terminal verification already required that value to be
/// defined before this use (`require_defined` over the dominating
/// definitions), so its machine-wide SSA declaration is its type here; this
/// lowering checks only that the selector is an integer the address model
/// scales, never where it was defined.
fn read_selector_type(
    machine: &TerminalMachine,
    block: &terminal_psi::Block,
    operation: semantic_vocabulary::OperationId,
    index: ValueId,
    values: &BTreeMap<ValueId, ScalarType>,
) -> Option<ScalarType> {
    let declared_here = machine
        .parameters
        .iter()
        .chain(block.parameters.iter())
        .any(|declaration| declaration.id == index)
        || block
            .operations
            .iter()
            .filter_map(|candidate| candidate.result.scalar())
            .any(|result| result.id == index);
    if declared_here {
        return super::structural_scalar_fields::dominating_scalar_type(
            machine, block, operation, index,
        );
    }
    values.get(&index).copied()
}

pub(super) fn lower(
    operation: &Operation,
    block: &terminal_psi::Block,
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
                    (!path.is_empty())
                        .then(|| owned_state_parameter(machine, source))
                        .flatten()
                        .map(|(_, parameter)| parameter.structural_type)
                })
                .or_else(|| {
                    parameter(machine, source)
                        .filter(|parameter| {
                            // A projected read observes the owned aggregate's
                            // existing storage. An empty path still requires
                            // a primitive referent or established local.
                            (matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                            ) || (parameter.access == StructuralAccess::Owned && !path.is_empty()))
                                && (parameter.multiplicity == StructuralMultiplicity::Unrestricted
                                    || (!path.is_empty()
                                        && parameter.multiplicity
                                            == StructuralMultiplicity::Affine))
                                && parameter.qualifications.is_empty()
                                && parameter.projected_qualifications.is_empty()
                                && super::structural_scalar_fields::has_empty_structural_custody(
                                    machine, source,
                                )
                        })
                        .map(|parameter| parameter.structural_type)
                })
                .ok_or_else(invalid)?;
            if super::primitive_projection::read_leaf_type(types, identity, path, |index| {
                read_selector_type(machine, block, operation.id, index, values)
            }) != Some(result.scalar_type)
            {
                return Err(invalid());
            }
            Ok(AbstractOperation::PrimitiveScalarRead {
                psi_operation: operation.id,
                source,
                path: path.clone(),
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

#[cfg(test)]
mod tests {
    use super::read_selector_type;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, IntegerSign, IntegerType, MachineId, OperationId, ScalarType,
        ValueId,
    };
    use std::collections::BTreeMap;
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
        TerminalMachineResult, Terminator, ValueDeclaration,
    };

    fn u64_type() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
    }

    fn declaration(id: u64) -> ValueDeclaration {
        ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(id).unwrap(),
            scalar_type: u64_type(),
        }
    }

    fn operation(id: u64, result: Option<u64>) -> Operation {
        Operation {
            static_reach_binding: None,
            suspension_crossing: None,
            id: OperationId::new(id).unwrap(),
            result: result.map_or(OperationResult::Unit, |value| {
                OperationResult::Scalar(declaration(value))
            }),
            kind: OperationKind::BooleanConstant { value: true },
        }
    }

    fn block(id: u64, parameters: Vec<ValueDeclaration>, operations: Vec<Operation>) -> Block {
        Block {
            erased_proof_formals: Vec::new(),
            erased_scalar_formals: Vec::new(),
            id: BlockId::new(id).unwrap(),
            parameters,
            structural_parameters: Vec::new(),
            operations,
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(id).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }
    }

    /// The state block declares the selector `k` (value 2) as a block
    /// parameter; the conditional arm's block reads with it at operation 7.
    fn machine(arm: Block) -> TerminalMachine {
        TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(2).unwrap(),
            blocks: vec![block(2, vec![declaration(2)], Vec::new()), arm],
            contract: MachineContract {
                erased_proof_formals: Vec::new(),
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(9).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    #[test]
    fn projected_owned_storage_retains_multiplicity_and_producer_identity() {
        use semantic_vocabulary::{PlaceId, StructuralPlaceKind, StructuralTypeId};
        use terminal_psi::{
            StructuralMultiplicity, StructuralOperationResult, StructuralPlaceDeclaration,
        };
        let place = PlaceId::new(3).unwrap();
        let structural_type = StructuralTypeId::new(4).unwrap();
        for multiplicity in [
            StructuralMultiplicity::Unrestricted,
            StructuralMultiplicity::Affine,
            StructuralMultiplicity::Linear,
        ] {
            let mut producer = operation(6, None);
            producer.kind = OperationKind::EstablishZeroedStructural {};
            producer.result = OperationResult::Structural(StructuralOperationResult {
                place,
                structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                qualification_establishments: Vec::new(),
                claims: Vec::new(),
            });
            let mut source = machine(block(3, Vec::new(), vec![producer]));
            source.structural_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(6).unwrap(),
                    structural_type,
                },
            });
            assert_eq!(
                super::owned_result(&source, place).is_some(),
                multiplicity != StructuralMultiplicity::Linear
            );
            assert!(
                super::local(&source, place).is_none(),
                "record storage is not a primitive local"
            );
            source.structural_places[0].kind = StructuralPlaceKind::OperationResult {
                producer: OperationId::new(7).unwrap(),
                structural_type,
            };
            assert!(
                super::owned_result(&source, place).is_none(),
                "a mismatched producer grants no storage"
            );
        }
    }

    fn machine_values(machine: &TerminalMachine) -> BTreeMap<ValueId, ScalarType> {
        machine
            .blocks
            .iter()
            .flat_map(|block| {
                block.parameters.iter().cloned().chain(
                    block
                        .operations
                        .iter()
                        .filter_map(|operation| operation.result.scalar()),
                )
            })
            .map(|value| (value.id, value.scalar_type))
            .collect()
    }

    #[test]
    fn selector_declared_in_a_dominating_block_takes_its_machine_declaration() {
        let machine = machine(block(5, Vec::new(), vec![operation(7, None)]));
        let values = machine_values(&machine);
        let arm = &machine.blocks[1];
        assert_eq!(
            read_selector_type(
                &machine,
                arm,
                OperationId::new(7).unwrap(),
                ValueId::new(2).unwrap(),
                &values,
            ),
            Some(u64_type())
        );
    }

    #[test]
    fn selector_declared_in_the_read_block_must_precede_the_read() {
        // Value 3 is produced in the arm's own block after the read: the
        // in-block rule still refuses it rather than trusting the global map.
        let machine = machine(block(
            5,
            Vec::new(),
            vec![operation(7, None), operation(8, Some(3))],
        ));
        let values = machine_values(&machine);
        let arm = &machine.blocks[1];
        assert_eq!(
            read_selector_type(
                &machine,
                arm,
                OperationId::new(7).unwrap(),
                ValueId::new(3).unwrap(),
                &values,
            ),
            None
        );
        // A selector no block declares has no type to scale.
        assert_eq!(
            read_selector_type(
                &machine,
                arm,
                OperationId::new(7).unwrap(),
                ValueId::new(11).unwrap(),
                &values,
            ),
            None
        );
    }

    #[test]
    fn a_state_owned_arrival_is_a_projected_read_root() {
        use semantic_vocabulary::{BlockId, PlaceId, StructuralPlaceKind, StructuralTypeId};
        use terminal_psi::{
            StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
            StructuralPlaceDeclaration,
        };
        let place = PlaceId::new(4).unwrap();
        let arrival = |access, multiplicity| StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type: StructuralTypeId::new(2).unwrap(),
            multiplicity,
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let with_arrival = |parameter: StructuralParameterDeclaration| {
            let mut arm = block(5, Vec::new(), Vec::new());
            arm.structural_parameters.push(parameter);
            let mut source = machine(arm);
            source.structural_places.push(StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::BlockParameter {
                    block: BlockId::new(5).unwrap(),
                    position: 0,
                },
            });
            source
        };
        for multiplicity in [
            StructuralMultiplicity::Unrestricted,
            StructuralMultiplicity::Affine,
        ] {
            let source = with_arrival(arrival(StructuralAccess::Owned, multiplicity));
            assert!(super::owned_state_parameter(&source, place).is_some());
        }
        // Linear custody, a borrowed join and a misdeclared position are not
        // owned arrival storage.
        let linear = with_arrival(arrival(
            StructuralAccess::Owned,
            StructuralMultiplicity::Linear,
        ));
        assert!(super::owned_state_parameter(&linear, place).is_none());
        let borrowed = with_arrival(arrival(
            StructuralAccess::MutableBorrow,
            StructuralMultiplicity::Unrestricted,
        ));
        assert!(super::owned_state_parameter(&borrowed, place).is_none());
        let mut misplaced = with_arrival(arrival(
            StructuralAccess::Owned,
            StructuralMultiplicity::Unrestricted,
        ));
        misplaced.structural_places[0].kind = StructuralPlaceKind::BlockParameter {
            block: BlockId::new(2).unwrap(),
            position: 0,
        };
        assert!(super::owned_state_parameter(&misplaced, place).is_none());
    }
}
