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
use std::collections::{HashMap, HashSet};

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
            Some((trait_definition.name.as_str().to_string(), signatures))
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

    for (type_name, trait_name) in conformances {
        if !data_names.contains(&type_name) {
            continue;
        }
        let Some(signatures) = traits.get(&trait_name) else {
            continue;
        };
        for signature in signatures {
            if !signature.is_default
                || !attached_methods
                    .insert((type_name.clone(), signature.name.as_str().to_string()))
            {
                continue;
            }
            synthesize_default_machine(syntax, &type_name, signature);
        }
    }

    Ok(())
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
