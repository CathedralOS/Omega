//! One resolved callable destination shared by construction and correspondence.
//! Source calls may name an exact requirement, a dynamic slot, or a declared
//! machine/state. These are semantic identities, not interchangeable raw handles
//! or selected provider identities. Resolve once through the producer's order.

use super::trait_boundary_call::BoundaryRequirement;
use symbols::SymbolHandle;
use validation::exact_external_leaf_boundary_requirement;

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

/// The boundary requirement a call to `target` invokes. A direct call to a
/// bodyless external leaf is a boundary call on the leaf's exact satisfied
/// requirement (see `validation`'s `intrinsic_boundaries` module); the leaf
/// owns no body to call. A nominal binder describes an obligation, not a
/// selected boundary body: specialization must replace it with the exact
/// executable target before a call is planned, and projecting its
/// requirement here would make an open generic entry executable and bypass
/// its selected implementation. Both declaration spellings retain a
/// requirement edge; the planner sees the authored signature, never its
/// selected provider.
fn boundary_requirement(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> BoundaryTarget<'_> {
    let direct = exact_external_leaf_boundary_requirement(program, target)
        .map(|(requirement, _)| requirement)
        .unwrap_or(target);
    let static_boundaries = program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .flat_map(|definition| {
            program
                .trait_machine_instances(definition)
                .filter(move |signature| signature.symbol == direct)
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
            .find(|operator| operator.is_boundary && operator.symbol == direct)
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
