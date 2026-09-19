#![forbid(unsafe_code)]

//! Psi-owned parsing of Omega tokens into unresolved source-shaped syntax.
//!
//! Start at `parser.rs`: `parse` owns source traversal, declaration dispatch,
//! and root publication into the caller's `SyntaxTrees`. Its sibling grammar
//! domains own declarations, expressions, type syntax, parameters, contracts,
//! and bodies. Diagnostics own the failure surface. Meaning belongs to
//! the resolution and typing stages that follow; this stage only recognizes shape.
//! Source registration lives here; the registered files own the implementation.

mod bodies {
    pub(crate) mod parse_body;
    pub(crate) mod sequence;
    pub(crate) mod states;
    pub(crate) mod tail_calls;
    pub(crate) mod trait_default;
    pub(crate) mod statements {
        pub(crate) mod atomic_lets;
        pub(crate) mod destructure_and_proof_output;
        pub(crate) mod discard_and_local_data;
        pub(crate) mod inline_assembly;
        pub(crate) mod parse_statement;
        pub(crate) mod statement_tables;
    }
    pub(crate) mod transitions {
        pub(crate) mod guards;
        pub(crate) mod parse_transition;
        pub(crate) mod targets {
            pub(crate) mod copy;
            pub(crate) mod parse_target;
        }
    }
}

mod contracts {
    pub(crate) mod conformance {
        mod external_binding;
        pub(crate) mod parse_conformance;
    }
    pub(crate) mod facts;
    pub(crate) mod parse_contract_clauses;
    pub(crate) mod signature;
    pub(crate) mod state_arrival;
}

mod declarations {
    mod capability;
    mod conformance;
    mod const_item;
    mod data;
    mod domain;
    mod let_definition;
    mod machines;
    mod measure;
    mod namespace;
    mod operator;
    pub(crate) mod parse_declaration;
    mod proposition;
    mod trait_definition;
    mod use_item;
}

mod diagnostics {
    pub mod parse_error;
    pub(crate) mod render_diagnostic;
}

mod expressions {
    pub(crate) mod context;
    mod membership;
    pub(crate) mod parse_expression;
    pub(crate) mod parse_postfix;
    mod primary;
    #[cfg(test)]
    mod static_targets_tests;
}

mod input {
    mod delimited;
    mod literals;
    pub(crate) mod paths;
    pub(crate) mod token_cursor;
}

mod parameters {
    pub(crate) mod binding_properties;
    pub(crate) mod contracts;
    #[cfg(test)]
    mod generic_parameters_tests;
    pub(crate) mod parse_generic_parameters;
    pub(crate) mod parse_parameters;
}

mod type_syntax {
    #[cfg(test)]
    mod nested_application_tests;
    pub(crate) mod parse_type;
    pub(crate) mod properties;
    #[cfg(test)]
    mod qualified_names;
    #[cfg(test)]
    mod remainder_tests;
    #[cfg(test)]
    mod value_dispatch_tests;
}

pub mod parser;

pub use diagnostics::parse_error::ParseError;
pub use parser::{
    parse, parse_syntax_trees, parse_syntax_trees_into_with_id, parse_syntax_trees_with_id,
};

#[cfg(test)]
mod tests {
    mod bindings_and_conformances;
    mod body_sequences;
    mod constructors;
    mod definitions_and_contracts;
    mod domains_and_transitions;
    mod expression_stack;
    mod inline_assembly;
    mod items_and_selections;
    mod mathematical_definitions;
    mod numbered_data;
    mod properties_and_requirements;
    mod source_parsing;
    mod type_constraints;
    mod value_dispatch;
}
