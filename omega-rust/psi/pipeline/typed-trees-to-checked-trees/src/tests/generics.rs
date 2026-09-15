//! Fixtures shared by the generics tests: specialized machine lookup.

mod conformance_binders;
mod const_arguments;
mod const_values;
mod empty_ranges;
mod named_conformance;
mod named_witnesses;
mod nested_calls;
mod nominal_machine_parameters;
mod result_local_providers;
mod specialization_identities;
mod specializations;
mod symbolic_ranges;

use crate::tests::{Lexer, lower_symbol_resolved_trees, parse_syntax_trees};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

fn specialized_machine<'program>(
    program: &'program checked_trees::CheckedTrees,
    name: &str,
) -> &'program typed_trees::machine::Machine {
    let template = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("authored generic template");
    assert!(!program.machine_type_parameters(template).is_empty());
    let instances = program
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == template.symbol)
        .collect::<Vec<_>>();
    let [specialization] = instances.as_slice() else {
        panic!("expected one selected instance of {name}")
    };
    assert_ne!(specialization.instance, template.symbol);
    let instance = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == specialization.instance)
        .expect("selected private instance");
    assert!(!instance.is_public);
    instance
}

fn typed_source(source: &str) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}
