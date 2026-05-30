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
use crate::capabilities::providers::{
    BoundaryProvider, BoundaryProviderRegistry, HostCallAuthorization, authority_effects,
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
pub fn build_provider_registry(program: &TypedTrees) -> BoundaryProviderRegistry {
    let mut registry = BoundaryProviderRegistry::new();

    for trait_definition in program.traits() {
        if !trait_definition.is_boundary {
            continue;
        }

        let mut effects = EffectSet::empty();
        for signature in program.trait_machine_signatures(trait_definition) {
            effects.insert_all(signature_effects(program, signature));
        }

        let implemented_in_package = boundary_trait_is_implemented(program, trait_definition.symbol);
        registry.register(BoundaryProvider::new(
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
    registry: &BoundaryProviderRegistry,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infer_effects;
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;

    fn lower(source: &str) -> TypedTrees {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax_trees = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax_trees).expect("resolve");
        lower_symbol_resolved_trees(&resolved).expect("type")
    }

    #[test]
    fn abstract_boundary_provider_is_authorized() {
        let program = lower(
            r#"
            boundary trait Console {
                machine write_line(text: String)
                effects
                    stdout_io;
            }

            data Main {
                console: Console;
            }

            machine Main::main(&mut self) {
                self.console.write_line("hello");
            }
            "#,
        );
        let effects = infer_effects(&program);
        let registry = build_provider_registry(&program);
        let unapproved = audit_host_calls(&program, &effects, &registry);
        assert!(unapproved.is_empty(), "abstract provider should be approved");
    }

    #[test]
    fn in_package_host_provider_is_unapproved() {
        let program = lower(
            r#"
            boundary trait LocalFiles {
                machine write_bytes(path: String)
                effects
                    filesystem_io;
            }

            data Disk {
            }

            machine Disk::write_bytes(path: String) satisfies LocalFiles
            effects
                filesystem_io
            {
            }

            data Main {
                disk: Disk;
            }

            machine Main::main(&mut self) {
                self.disk.write_bytes("/etc/passwd");
            }
            "#,
        );
        let effects = infer_effects(&program);
        let registry = build_provider_registry(&program);
        let unapproved = audit_host_calls(&program, &effects, &registry);
        assert_eq!(
            unapproved.len(),
            1,
            "in-package host provider should be flagged as unapproved"
        );
        assert_eq!(
            unapproved[0].missing_authority.names().collect::<Vec<_>>(),
            ["filesystem_io"]
        );
    }
}
