//! Pre-resolution synthesis for trait machines with authored bodies.
//!
//! A standalone `Data satisfies Trait;` conformance may omit a machine when
//! the trait supplies its body. Materialize that body as an ordinary attached
//! machine before symbol resolution, so typing, effects, proofs, dispatch, both
//! execution engines, and override precedence all reuse the established paths.

use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::SyntaxTrees;
use omega_syntax_trees::expression::ExpressionHandle;
use omega_syntax_trees::identifier::Identifier;
use omega_syntax_trees::item::{Item, Machine, State, StateSignatureNode};
use omega_syntax_trees::types::TypeReferenceNode;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone)]
struct TraitDefaultsInput {
    signatures: Vec<StateSignatureNode>,
    requirements: Vec<String>,
}

#[derive(Clone)]
struct DefaultCandidate {
    origin: String,
    signature: StateSignatureNode,
}

type EffectiveDefaults = BTreeMap<String, Vec<DefaultCandidate>>;

pub(super) fn synthesize_trait_defaults(syntax: &mut SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    let data_names = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Data(data) => Some(data.name.as_str().to_string()),
            _ => None,
        })
        .collect::<HashSet<_>>();

    let traits = syntax
        .root_items()
        .filter_map(|item| {
            let Item::Trait(trait_definition) = item else {
                return None;
            };
            if !trait_definition.type_parameters.is_empty() {
                return None;
            }
            let signatures = syntax
                .items
                .state_signatures(trait_definition.machines)
                .iter()
                .map(|handle| syntax.items.state_signature(*handle).clone())
                .collect::<Vec<_>>();
            let requirements = syntax
                .type_references
                .type_reference_handles(trait_definition.parents)
                .iter()
                .filter_map(
                    |handle| match syntax.type_references.type_reference(*handle) {
                        TypeReferenceNode::Named(name) => Some(name.as_str().to_string()),
                        TypeReferenceNode::Generic { base_name, .. } => {
                            Some(base_name.as_str().to_string())
                        }
                        _ => None,
                    },
                )
                .chain(
                    syntax
                        .items
                        .identifier_path_members(trait_definition.requires)
                        .iter()
                        .map(|name| name.as_str().to_string()),
                )
                .collect();
            Some((
                trait_definition.name.as_str().to_string(),
                TraitDefaultsInput {
                    signatures,
                    requirements,
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    let conformances = syntax
        .root_items()
        .filter_map(|item| match item {
            Item::Conformance(conformance) => Some((
                conformance.type_name.as_str().to_string(),
                conformance.trait_name.as_str().to_string(),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();

    let mut attached_methods = HashSet::new();
    for item in syntax.root_items() {
        let Item::Machine(machine) = item else {
            continue;
        };
        let Some(attached) = machine.attached_data.as_ref() else {
            continue;
        };
        for state in syntax.items.state_handles(machine.states) {
            attached_methods.insert((
                attached.as_str().to_string(),
                syntax.items.state(*state).name.as_str().to_string(),
            ));
        }
    }

    let mut effective_defaults = HashMap::new();
    let mut diagnostics = Vec::new();
    let mut reported_conflicts = HashSet::new();
    for (type_name, trait_name) in conformances {
        if !data_names.contains(&type_name) {
            continue;
        }
        if !traits.contains_key(&trait_name) {
            continue;
        }
        let defaults = collect_effective_defaults(
            &trait_name,
            &traits,
            &mut effective_defaults,
            &mut HashSet::new(),
        );
        for (method_name, candidates) in defaults {
            let attached_method = (type_name.clone(), method_name.clone());
            if attached_methods.contains(&attached_method) {
                continue;
            }
            if candidates.len() > 1 {
                if reported_conflicts.insert(attached_method) {
                    let origins = candidates
                        .iter()
                        .map(|candidate| format!("`{}`", candidate.origin))
                        .collect::<Vec<_>>()
                        .join(", ");
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{type_name}` inherits conflicting default machine \
                         `{type_name}::{method_name}` from traits {origins}; write an override"
                    )));
                }
                continue;
            }
            let Some(candidate) = candidates.first() else {
                continue;
            };
            attached_methods.insert(attached_method);
            synthesize_default_machine(syntax, &type_name, &candidate.signature);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Compute the defaults visible from one trait. A declaration on the child,
/// including a bodyless requirement, shadows all inherited declarations of
/// the same name. Distinct inherited bodies remain separate candidates so the
/// conformance site can require a written override; repeated paths to the same
/// originating trait (a diamond) are deduplicated.
fn collect_effective_defaults(
    trait_name: &str,
    traits: &HashMap<String, TraitDefaultsInput>,
    memo: &mut HashMap<String, EffectiveDefaults>,
    visiting: &mut HashSet<String>,
) -> EffectiveDefaults {
    if let Some(defaults) = memo.get(trait_name) {
        return defaults.clone();
    }
    if !visiting.insert(trait_name.to_string()) {
        // Requirement-cycle validation owns the diagnostic. Avoid recursing
        // forever in this earlier desugaring pass.
        return EffectiveDefaults::new();
    }
    let Some(trait_definition) = traits.get(trait_name) else {
        visiting.remove(trait_name);
        return EffectiveDefaults::new();
    };

    let mut defaults = EffectiveDefaults::new();
    for requirement in &trait_definition.requirements {
        for (method_name, candidates) in
            collect_effective_defaults(requirement, traits, memo, visiting)
        {
            let inherited = defaults.entry(method_name).or_default();
            for candidate in candidates {
                if !inherited
                    .iter()
                    .any(|existing: &DefaultCandidate| existing.origin == candidate.origin)
                {
                    inherited.push(candidate);
                }
            }
        }
    }

    for signature in &trait_definition.signatures {
        let method_name = signature.name.as_str().to_string();
        defaults.remove(&method_name);
        if signature.is_default {
            defaults.insert(
                method_name,
                vec![DefaultCandidate {
                    origin: trait_name.to_string(),
                    signature: signature.clone(),
                }],
            );
        }
    }

    visiting.remove(trait_name);
    memo.insert(trait_name.to_string(), defaults.clone());
    defaults
}

fn synthesize_default_machine(
    syntax: &mut SyntaxTrees,
    type_name: &str,
    signature: &StateSignatureNode,
) {
    let state = State {
        name: signature.name.clone(),
        parameters: signature.parameters,
        return_type: signature.return_type,
        statements: signature.default_body,
    };
    let state = syntax.items.insert_state(&state);
    let state = syntax.items.append_state_handle(state);
    syntax.push_root_item(Item::Machine(Machine {
        name: Identifier::generated(format!("{type_name}::{}", signature.name.as_str())),
        attached_data: Some(Identifier::generated(type_name)),
        bodyless: false,
        target: None,
        boundary: false,
        type_parameters: HandleSpan::empty(),
        satisfies: HandleSpan::empty(),
        terminates: signature.terminates_guarantee,
        terminates_guarantee: signature.terminates_guarantee,
        decreases: HandleSpan::<ExpressionHandle>::empty(),
        decrease_order: HandleSpan::empty(),
        decrease_view_arguments: HandleSpan::empty(),
        decrease_range: ExpressionHandle::invalid(),
        effects: signature.effects,
        contracts: signature.contracts,
        states: HandleSpan::from_parts(state, 1),
    }));
}
