//! Element views: establishing one, measuring it, reading through it,
//! storing through it, and narrowing it to a subslice.

pub(crate) mod establishment;
pub(crate) mod length;
pub(crate) mod mutable_establishment;
pub(crate) mod read;
pub(crate) mod subslice;
pub(crate) mod write;

use crate::validation::{StructuralAccess, StructuralPlaceKind, TerminalMachine, TerminalModule};

/// The operation result a live view binding occupies, paired with the
/// access its establishment minted: a shared view lends only shared; a
/// mutable view supplies every borrow kind. Neither kind lets the place
/// masquerade as an owned local.
pub(crate) fn view_loan_result_in_module<'a>(
    module: &TerminalModule,
    caller: &'a TerminalMachine,
    place: crate::validation::PlaceId,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    StructuralAccess,
)> {
    let access = view_access(module, caller, place)?;
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = caller
        .structural_places
        .iter()
        .find(|row| row.id == place)?
        .kind
    else {
        return None;
    };
    let result = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == producer)?
        .result
        .structural()?;
    (result.place == place && result.structural_type == structural_type).then_some((result, access))
}

/// Reconstruct authority from the exact input or producer chain. Multiplicity
/// chooses how a range retains its loan, but cannot promote a shared source.
/// Every step consumes a distinct declaration in a valid chain, so the place
/// count bounds malformed cycles without using the host stack.
fn view_access(
    module: &TerminalModule,
    machine: &TerminalMachine,
    mut place: crate::validation::PlaceId,
) -> Option<StructuralAccess> {
    use crate::validation::{OperationKind, StructuralMultiplicity};
    let mut requested = None;
    let mut mutable_required = false;
    for _ in 0..machine.structural_places.len() {
        let declaration = machine
            .structural_places
            .iter()
            .find(|row| row.id == place)?;
        let established = match declaration.kind {
            StructuralPlaceKind::Parameter { position, is_self } => {
                machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| {
                        parameter.place == place
                            && parameter.position == position
                            && parameter.is_self == is_self
                    })?
                    .access
            }
            StructuralPlaceKind::BlockParameter { .. } => {
                crate::validation::block_views::parameter(machine, place)?.access
            }
            StructuralPlaceKind::ByteSequenceLiteral { .. } => StructuralAccess::SharedBorrow,
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                let operation = machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|operation| operation.id == producer)?;
                let result = operation.result.structural()?;
                if result.place != place
                    || result.structural_type != structural_type
                    || !result.qualifications.is_empty()
                    || !result.projected_qualifications.is_empty()
                    || !result.claims.is_empty()
                {
                    return None;
                }
                match &operation.kind {
                    OperationKind::EstablishElementView { source, .. }
                        if source.access == StructuralAccess::SharedBorrow
                            && result.multiplicity == StructuralMultiplicity::Unrestricted =>
                    {
                        StructuralAccess::SharedBorrow
                    }
                    OperationKind::EstablishMutableElementView { source, .. }
                        if source.access == StructuralAccess::MutableBorrow
                            && result.multiplicity == StructuralMultiplicity::Affine =>
                    {
                        StructuralAccess::MutableBorrow
                    }
                    OperationKind::ElementViewSubslice { source, .. }
                    | OperationKind::ByteSequenceSubslice { source, .. } => {
                        let access = match result.multiplicity {
                            StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
                            StructuralMultiplicity::Affine => StructuralAccess::MutableBorrow,
                            _ => return None,
                        };
                        if mutable_required && access != StructuralAccess::MutableBorrow {
                            return None;
                        }
                        requested.get_or_insert(access);
                        mutable_required = access == StructuralAccess::MutableBorrow;
                        place = *source;
                        continue;
                    }
                    OperationKind::StructuralLeafCopy { .. } => {
                        if result.multiplicity != StructuralMultiplicity::Unrestricted
                            || !terminal_semantics::descriptor_results::is_shared_descriptor(
                                module.structural_types.iter(),
                                structural_type,
                            )
                        {
                            return None;
                        }
                        // Leaf-copy verification checks the exact readable
                        // source and copyable selected type independently.
                        StructuralAccess::SharedBorrow
                    }
                    OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. } => {
                        // A signature alone cannot lend backing. Reconstruct
                        // every normal return and substitute this exact call.
                        terminal_semantics::descriptor_results::call_origins(
                            module, machine, operation,
                        )?;
                        StructuralAccess::SharedBorrow
                    }
                    _ => return None,
                }
            }
            _ => return None,
        };
        if !matches!(
            established,
            StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
        ) || mutable_required && established != StructuralAccess::MutableBorrow
        {
            return None;
        }
        return Some(requested.unwrap_or(established));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, MachineId, ObligationId, OperationId, PlaceId,
        StructuralTypeId, ValueId,
    };
    use terminal_psi::{
        Block, MachineContract, Operation, OperationKind, OperationResult, StructuralMultiplicity,
        StructuralOperationResult, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        TerminalMachineResult, Terminator,
    };

    fn fixture(access: StructuralAccess) -> TerminalMachine {
        let structural_type = StructuralTypeId::new(1).unwrap();
        let source = PlaceId::new(1).unwrap();
        let entry = BlockId::new(1).unwrap();
        let operations: Vec<_> = (2..=3)
            .map(|ordinal| Operation {
                id: OperationId::new(ordinal).unwrap(),
                static_reach_binding: None,
                suspension_crossing: None,
                result: OperationResult::Structural(StructuralOperationResult {
                    place: PlaceId::new(ordinal).unwrap(),
                    structural_type,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                }),
                kind: OperationKind::ElementViewSubslice {
                    source: PlaceId::new(ordinal - 1).unwrap(),
                    start: ValueId::new(1).unwrap(),
                    end: ValueId::new(2).unwrap(),
                    length: ValueId::new(2).unwrap(),
                    obligation: ObligationId::new(ordinal).unwrap(),
                },
            })
            .collect();
        let mut structural_places = vec![StructuralPlaceDeclaration {
            id: source,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        }];
        structural_places.extend(
            operations
                .iter()
                .map(|operation| StructuralPlaceDeclaration {
                    id: operation.result.structural().unwrap().place,
                    kind: StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type,
                    },
                }),
        );
        TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: source,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places,
            entry_claims: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block {
                id: entry,
                parameters: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                operations,
                terminator: Terminator::ReturnUnit {
                    edge: EdgeId::new(1).unwrap(),
                    trivial_affine_discards: Vec::new(),
                },
            }],
            contract: MachineContract {
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }
    }

    #[test]
    fn view_subrange_authority_reconstructs_each_parent() {
        let module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        let mut machine = fixture(StructuralAccess::MutableBorrow);
        let destination = PlaceId::new(3).unwrap();
        assert_eq!(
            view_loan_result_in_module(&module, &machine, destination)
                .unwrap()
                .1,
            StructuralAccess::MutableBorrow
        );
        machine.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        assert!(view_loan_result_in_module(&module, &machine, destination).is_none());
        for operation in &mut machine.blocks[0].operations {
            let OperationResult::Structural(result) = &mut operation.result else {
                unreachable!()
            };
            result.multiplicity = StructuralMultiplicity::Unrestricted;
        }
        assert_eq!(
            view_loan_result_in_module(&module, &machine, destination)
                .unwrap()
                .1,
            StructuralAccess::SharedBorrow
        );
        let OperationResult::Structural(result) = &mut machine.blocks[0].operations[1].result
        else {
            unreachable!()
        };
        result.multiplicity = StructuralMultiplicity::Affine;
        assert!(view_loan_result_in_module(&module, &machine, destination).is_none());
    }

    #[test]
    fn view_subrange_authority_rejects_replaced_results_and_cycles() {
        let module = TerminalModule::for_entry(MachineId::new(1).unwrap());
        let baseline = fixture(StructuralAccess::MutableBorrow);
        let destination = PlaceId::new(3).unwrap();
        for mutation in 0..3 {
            let mut machine = baseline.clone();
            match mutation {
                0 => {
                    let OperationResult::Structural(result) =
                        &mut machine.blocks[0].operations[0].result
                    else {
                        unreachable!()
                    };
                    result.structural_type = StructuralTypeId::new(2).unwrap();
                }
                1 => {
                    let OperationResult::Structural(result) =
                        &mut machine.blocks[0].operations[0].result
                    else {
                        unreachable!()
                    };
                    result.place = PlaceId::new(4).unwrap();
                }
                _ => {
                    let OperationKind::ElementViewSubslice { source, .. } =
                        &mut machine.blocks[0].operations[0].kind
                    else {
                        unreachable!()
                    };
                    *source = destination;
                }
            }
            assert!(
                view_loan_result_in_module(&module, &machine, destination).is_none(),
                "mutation {mutation}"
            );
        }
    }
}
