//! Scalar merge and induction claims name exact blocks and every actual arrival.
//!
//! Predicates are scoped to the destination telescope, immutable invocation
//! formals, and storage observations rooted at places alive for the whole
//! invocation — not every value known to the machine. In particular a
//! branch-local definition cannot become meaningful on another branch. Entry
//! assertions are forbidden because invocation is an implicit arrival without
//! an edge proof.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, Proposition, PropositionContext, StructuralPlaceKind};
use terminal_psi::{Block, TerminalMachine, TerminalModule, Terminator};

use super::{IdRegistry, ModuleError, insert_unique};

/// The invariant telescope: machine and header scalar parameters plus the
/// structural places alive for the whole invocation. Only machine-owned
/// parameters and this header's own structural parameters qualify; an
/// operation result or another block's local cannot name storage that exists
/// at every arrival. Field terms rooted at these places denote current
/// storage, so their observations remain subject to exact write-path
/// invalidation inside each arrival proof.
pub fn scalar_block_invariant_scope(
    machine: &TerminalMachine,
    header: &Block,
) -> Result<PropositionContext, ModuleError> {
    PropositionContext::from_value_types_and_places(
        machine
            .parameters
            .iter()
            .chain(&machine.contract.erased_scalar_formals)
            .chain(&header.parameters)
            .chain(&header.erased_scalar_formals)
            .map(|parameter| (parameter.id, parameter.scalar_type)),
        machine
            .structural_places
            .iter()
            .filter(|place| {
                matches!(place.kind, StructuralPlaceKind::Parameter { .. })
                    || matches!(
                        place.kind,
                        StructuralPlaceKind::BlockParameter { block, .. } if block == header.id
                    )
            })
            .map(|place| (place.id, place.kind)),
    )
    .map_err(ModuleError::MalformedProposition)
}

pub(super) fn validate(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    let mut previous = None;
    for invariant in &module.scalar_block_invariants {
        let identity = (invariant.machine, invariant.header);
        if previous.is_some_and(|previous| previous >= identity) {
            return Err(ModuleError::NonCanonicalScalarBlockInvariants);
        }
        previous = Some(identity);
        let invalid = || ModuleError::InvalidScalarBlockInvariant {
            machine: invariant.machine,
            header: invariant.header,
        };
        let invalid_arrivals = || ModuleError::InvalidScalarBlockInvariantArrivals {
            machine: invariant.machine,
            header: invariant.header,
        };
        let machine = machines.get(&invariant.machine).ok_or_else(invalid)?;
        let header = machine
            .blocks
            .iter()
            .find(|block| block.id == invariant.header)
            .ok_or_else(invalid)?;
        if header.id == machine.entry || !scalar_predicate(&invariant.predicate) {
            return Err(invalid());
        }
        let context = scalar_block_invariant_scope(machine, header).map_err(|_| invalid())?;
        context
            .validate(&invariant.predicate)
            .map_err(|_| invalid())?;
        let mut arrivals = BTreeSet::new();
        for block in &machine.blocks {
            match &block.terminator {
                Terminator::Jump { edge, target, .. } if *target == invariant.header => {
                    arrivals.insert(*edge);
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for successor in [when_true, when_false] {
                        if successor.target == invariant.header {
                            arrivals.insert(successor.edge);
                        }
                    }
                }
                Terminator::StructuralCase { cases, .. }
                    if cases
                        .iter()
                        .any(|successor| successor.target == invariant.header) =>
                {
                    return Err(invalid_arrivals());
                }
                _ => {}
            }
        }
        if !invariant
            .arrivals
            .iter()
            .map(|arrival| arrival.edge)
            .eq(arrivals)
        {
            return Err(invalid_arrivals());
        }
        for arrival in &invariant.arrivals {
            insert_unique(
                &mut registry.obligations,
                arrival.obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
    }
    Ok(())
}

// The context admits only invocation-lived places: an observation rooted at
// an operation result or another block's local cannot be invariant scope.
// Explicitly reject non-scalar atoms which do not necessarily contain values.
fn scalar_predicate(predicate: &Proposition) -> bool {
    match predicate {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Equal(..)
        | Proposition::LessThan(..)
        | Proposition::LessOrEqual(..) => true,
        Proposition::Conjunction(members) | Proposition::Disjunction(members) => {
            members.iter().all(scalar_predicate)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => scalar_predicate(premise) && scalar_predicate(conclusion),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Block, MachineId, Proposition, StructuralPlaceKind, TerminalMachine, Terminator,
        scalar_block_invariant_scope,
    };
    use semantic_vocabulary::{
        BlockId, ContractId, EdgeId, OperationId, PlaceId, ScalarTerm, StructuralFieldId,
        StructuralTypeId,
    };
    use terminal_psi::{MachineContract, StructuralPlaceDeclaration, TerminalMachineResult};

    fn place(raw: u64) -> PlaceId {
        PlaceId::new(raw).unwrap()
    }

    fn fixture() -> (TerminalMachine, Block) {
        let header = BlockId::new(3).unwrap();
        let header_block = Block {
            erased_scalar_formals: Vec::new(),
            id: header,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        };
        let machine = TerminalMachine {
            id: MachineId::new(1).unwrap(),
            attachment: None,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Unit,
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: place(1),
                    kind: StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: true,
                    },
                },
                StructuralPlaceDeclaration {
                    id: place(2),
                    kind: StructuralPlaceKind::BlockParameter {
                        block: header,
                        position: 0,
                    },
                },
                StructuralPlaceDeclaration {
                    id: place(3),
                    kind: StructuralPlaceKind::BlockParameter {
                        block: BlockId::new(9).unwrap(),
                        position: 0,
                    },
                },
                StructuralPlaceDeclaration {
                    id: place(4),
                    kind: StructuralPlaceKind::OperationResult {
                        producer: OperationId::new(1).unwrap(),
                        structural_type: StructuralTypeId::new(1).unwrap(),
                    },
                },
            ],
            entry_claims: Vec::new(),
            declared_service_reach: Vec::new(),
            closed_reach_application: None,
            published_service_ceiling: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: vec![header_block.clone()],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                id: ContractId::new(1).unwrap(),
                crash_routes: Vec::new(),
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        };
        (machine, header_block)
    }

    #[test]
    fn scope_admits_invocation_lived_places_only() {
        let (machine, header) = fixture();
        let context = scalar_block_invariant_scope(&machine, &header).unwrap();
        let field = StructuralFieldId::new(1).unwrap();
        let observation = |root| {
            Proposition::Equal(
                ScalarTerm::boolean_field(root, field),
                ScalarTerm::boolean(false),
            )
        };
        // A machine parameter and the header's own block parameter name
        // storage alive at every arrival.
        for admitted in [place(1), place(2)] {
            context.validate(&observation(admitted)).unwrap();
        }
        // Another block's local, an operation result, and an undeclared place
        // cannot name storage that exists at every header arrival.
        for rejected in [place(3), place(4), place(5)] {
            assert!(context.validate(&observation(rejected)).is_err());
        }
    }
}
