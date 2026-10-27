//! One resolved callable destination shared by construction and correspondence.
//! Source calls may name an exact requirement, a dynamic slot, or a declared
//! machine/state. These are semantic identities, not interchangeable raw handles
//! or selected provider identities. Resolve once through the producer's order.

use super::trait_boundary_call::BoundaryRequirement;
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) enum ResolvedCallTarget<'program> {
    Requirement(BoundaryRequirement<'program>),
    DynamicSlot(&'program typed_trees::dynamic_traits::DynamicSlotSignature),
    Declared {
        machine: &'program typed_trees::machine::Machine,
        state: &'program typed_trees::state::State,
    },
}

impl ResolvedCallTarget<'_> {
    pub(in crate::execution::terminal_unit) fn identity(self) -> (SymbolHandle, SymbolHandle) {
        match self {
            Self::Requirement(requirement) => {
                let target = requirement.target_symbol();
                (target, target)
            }
            Self::DynamicSlot(slot) => (slot.symbol, slot.symbol),
            Self::Declared { machine, state } => (machine.symbol, state.symbol),
        }
    }
}

pub(in crate::execution::terminal_unit) fn resolve_call_target(
    program: &dyn validation::CheckingInput,
    target: SymbolHandle,
) -> Option<ResolvedCallTarget<'_>> {
    match boundary_requirement(program, target) {
        BoundaryTarget::Requirement(requirement) => {
            return Some(ResolvedCallTarget::Requirement(requirement));
        }
        BoundaryTarget::Ambiguous => return None,
        BoundaryTarget::Body => {}
    }
    if let Some(slot) = program.dynamic_slot_by_symbol(target) {
        return Some(ResolvedCallTarget::DynamicSlot(slot));
    }
    let (machine, state) = executable_target(program, target)?;
    Some(ResolvedCallTarget::Declared { machine, state })
}

/// What a call's authored target completes through.
enum BoundaryTarget<'program> {
    /// A boundary requirement's call route.
    Requirement(super::trait_boundary_call::BoundaryRequirement<'program>),
    /// More than one boundary requirement declares the target.
    Ambiguous,
    /// No boundary requirement: a machine body or a dynamic slot.
    Body,
}

/// Only an authored requirement or operator invokes a selected provider.
/// A concrete external leaf keeps its own declared callable coordinate even
/// when a target-scoped caller already resolves its exact family member.
/// Its satisfaction edge supplies a contract, not another call destination.
/// Nominal binders still require specialization before executable planning.
fn boundary_requirement(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> BoundaryTarget<'_> {
    let static_boundaries = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_instances(definition)
                .filter(move |signature| signature.symbol == target)
                .map(move |signature| (definition, signature))
        })
        .collect::<Vec<_>>();
    match static_boundaries.as_slice() {
        [(definition, signature)] => {
            BoundaryTarget::Requirement(super::trait_boundary_call::BoundaryRequirement::Trait {
                definition,
                signature,
            })
        }
        [] => program
            .operators()
            .iter()
            .find(|operator| operator.is_boundary && operator.symbol == target)
            .map_or(BoundaryTarget::Body, |operator| {
                BoundaryTarget::Requirement(
                    super::trait_boundary_call::BoundaryRequirement::Operator(operator),
                )
            }),
        _ => BoundaryTarget::Ambiguous,
    }
}

/// The machine and state a call to `target` executes. Source custody can name
/// a machine head or an exact stored state: a machine invokes its entry, and
/// an explicitly selected state keeps its own signature.
fn executable_target(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> Option<(&typed_trees::machine::Machine, &typed_trees::state::State)> {
    validation::find_machine_head(program, target)
        .or_else(|| validation::find_state(program, target))
}
