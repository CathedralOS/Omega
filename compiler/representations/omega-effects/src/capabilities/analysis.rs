//! Builds the boundary provider registry from a program and audits host calls.
//!
//! The registry whitelists every declared boundary trait that the package does
//! *not* implement itself (toolchain / host provider edges). A boundary trait
//! that the application implements in-package and that mints host authority is
//! treated as ordinary code minting its own authority, which the resolution gate
//! rejects as an unapproved host call (chapter 18, "Host Providers").

use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::signature::StateSignature;

use crate::EffectSet;
use crate::capabilities::host_authority::{
    HostAuthorityProvider, HostAuthorityRegistry, HostCallAuthorization, authority_effects,
};
use crate::{EffectPlan, requires_host_authority};

/// A boundary call that requests host authority but is not backed by an approved
/// provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnapprovedHostCall {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub boundary_trait_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub missing_authority: EffectSet,
}

/// Builds the boundary provider registry for `program`.
///
/// Every boundary trait becomes a provider carrying the union of authority
/// effects declared across its machine signatures. A boundary trait is
/// whitelisted unless the application provides an in-package implementation of
/// it, since ordinary code may not mint fresh host providers.
pub fn build_host_authority_registry(program: &TypedTrees) -> HostAuthorityRegistry {
    let mut registry = HostAuthorityRegistry::new();

    for trait_definition in program.traits() {
        if !trait_definition.is_boundary {
            continue;
        }

        let mut effects = EffectSet::empty();
        for signature in program.trait_machine_signatures(trait_definition) {
            effects.insert_all(signature_effects(program, signature));
        }

        let implemented_in_package =
            boundary_trait_is_implemented(program, trait_definition.symbol);
        registry.register(HostAuthorityProvider::new(
            trait_definition.symbol,
            authority_effects(effects),
            !implemented_in_package,
        ));
    }

    registry
}

/// Audits every boundary call in `effects` against the provider registry,
/// returning the host calls that no approved provider authorizes.
pub fn audit_host_calls(
    program: &TypedTrees,
    effects: &EffectPlan,
    registry: &HostAuthorityRegistry,
) -> Vec<UnapprovedHostCall> {
    let mut unapproved = Vec::new();

    for machine in effects.machines() {
        for state in effects.states.span_or_empty(machine.states) {
            for call in effects.calls.span_or_empty(state.calls) {
                let Some((boundary_trait_symbol, signature)) =
                    boundary_signature(program, call.target_state_symbol)
                else {
                    continue;
                };

                let mut call_effects = call.direct;
                call_effects.insert_all(signature_effects(program, signature));
                if !requires_host_authority(call_effects) {
                    continue;
                }

                match registry.authorize_host_call(boundary_trait_symbol, call_effects) {
                    HostCallAuthorization::Unapproved { missing } => {
                        unapproved.push(UnapprovedHostCall {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            boundary_trait_symbol,
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                            missing_authority: missing,
                        });
                    }
                    HostCallAuthorization::Authorized | HostCallAuthorization::NotAHostCall => {}
                }
            }
        }
    }

    unapproved
}

fn boundary_trait_is_implemented(program: &TypedTrees, trait_symbol: SymbolHandle) -> bool {
    program.machines().iter().any(|machine| {
        program
            .machine_trait_conformances(machine)
            .iter()
            .any(|conformance| conformance.symbol == trait_symbol)
    })
}

/// Resolves a call target to the boundary trait signature it reaches. The
/// target may be a boundary trait signature directly, or an in-package
/// implementation state whose machine conforms to a boundary trait.
fn boundary_signature<'program>(
    program: &'program TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<(SymbolHandle, &'program StateSignature)> {
    if !target_symbol.is_valid() {
        return None;
    }

    if let Some(found) = program.traits().iter().find_map(|trait_definition| {
        if !trait_definition.is_boundary {
            return None;
        }
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
            .map(|signature| (trait_definition.symbol, signature))
    }) {
        return Some(found);
    }

    // The target is an implementation state; map it back to the boundary trait
    // signature its machine conforms to, matched by state name.
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_symbol)
            .map(|state| (machine, state))
    })?;

    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|trait_definition| trait_definition.symbol == conformance.symbol)
        else {
            continue;
        };
        if !trait_definition.is_boundary {
            continue;
        }
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.name == state.name)
        {
            return Some((trait_definition.symbol, signature));
        }
    }

    None
}

fn signature_effects(program: &TypedTrees, signature: &StateSignature) -> EffectSet {
    let mut effects = EffectSet::empty();
    for effect in program.state_signature_effects(signature) {
        effects.insert_name(effect.as_str());
    }
    effects
}
