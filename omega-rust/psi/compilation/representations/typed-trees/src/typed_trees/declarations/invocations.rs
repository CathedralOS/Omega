//! Authored `invokes` declaration queries over the typed program.
//!
//! These resolvers normalize the exact target set an `invokes` clause authors:
//! parameter targets rejoin the declaring signature's positional parameters and
//! service targets must name boundary traits. Checking's synchronous-invocation
//! inference consumes the same normalization, so a caller comparing inferred
//! and authored cardinality sees one resolver.

use crate::TypedTrees;
use crate::machine::Machine;
use crate::signature::{
    AuthoredInvocation, AuthoredInvocationTarget, StateParameter, StateSignature,
};
use crate::types::TypeReferenceHandle;
use flow_effects::InvocationTarget;
use symbols::SymbolHandle;

/// Exact normalized target set authored by one signature's `invokes` clauses.
pub fn declared_signature_invocations(
    program: &TypedTrees,
    signature: &StateSignature,
) -> Vec<InvocationTarget> {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .collect::<Vec<_>>();
    declared_targets(
        program,
        program.state_signature_invokes(signature),
        &parameters,
    )
}

/// Exact normalized target set authored by one machine's `invokes` clauses.
///
/// This is the same resolver used by synchronous-invocation inference. Callers
/// that retain per-occurrence provenance may compare its cardinality with the
/// authored declaration list to reject unresolved or duplicate targets.
pub fn declared_machine_invocations(
    program: &TypedTrees,
    machine: &Machine,
) -> Vec<InvocationTarget> {
    declared_targets(
        program,
        program.machine_invokes(machine),
        &machine_entry_parameters(program, machine),
    )
}

/// The entry-state positional parameters an `invokes` parameter ordinal binds.
pub fn machine_entry_parameters<'a>(
    program: &'a TypedTrees,
    machine: &Machine,
) -> Vec<&'a StateParameter> {
    program
        .machine_states(machine)
        .first()
        .map(|state| program.state_parameters(state).iter().collect())
        .unwrap_or_default()
}

/// The exact type head named by a call-target roster entry.
pub fn boundary_requirement_symbol(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> SymbolHandle {
    program
        .type_reference_table
        .type_reference(type_reference)
        .type_symbol(&program.type_reference_table)
}

/// The boundary service `type_reference` resolves to, when it names one.
pub fn boundary_service_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let symbol = boundary_requirement_symbol(program, type_reference);
    program
        .traits()
        .iter()
        .any(|definition| definition.is_boundary && definition.symbol == symbol)
        .then_some(symbol)
}

/// Insert one normalized target, keeping ordinal-then-symbol ordering.
pub fn insert_target(targets: &mut Vec<InvocationTarget>, target: InvocationTarget) {
    if !targets.contains(&target) {
        targets.push(target);
        targets.sort_by_key(|target| match target {
            InvocationTarget::Parameter(index) => (0u8, *index),
            InvocationTarget::Service(symbol) => (1u8, symbol.arena_index()),
        });
    }
}

fn declared_targets(
    program: &TypedTrees,
    declarations: &[AuthoredInvocation],
    parameters: &[&StateParameter],
) -> Vec<InvocationTarget> {
    let mut targets = Vec::new();
    for declaration in declarations {
        let target = match declaration.target {
            AuthoredInvocationTarget::Unresolved => None,
            AuthoredInvocationTarget::Parameter { ordinal, symbol } => parameters
                .get(ordinal as usize)
                .filter(|parameter| parameter.symbol == symbol)
                .and_then(|parameter| {
                    boundary_service_for_type(program, parameter.type_reference)
                        .map(|_| InvocationTarget::Parameter(ordinal))
                }),
            AuthoredInvocationTarget::Service(symbol) => program
                .traits()
                .iter()
                .any(|definition| definition.symbol == symbol && definition.is_boundary)
                .then_some(InvocationTarget::Service(symbol)),
        };
        if let Some(target) = target {
            insert_target(&mut targets, target);
        }
    }
    targets
}
