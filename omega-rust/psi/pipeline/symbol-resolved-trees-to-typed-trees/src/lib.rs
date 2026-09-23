#![forbid(unsafe_code)]

//! Attaches type and signature meaning to Psi symbol-resolved source trees.
//!
//! Start at `lowerer.rs`: [`lower_symbol_resolved_trees`] owns complete typing.
//! It validates resolved meaning, lowers declarations in dependency order,
//! retains initializer custody, then settles and normalizes typed trees. Its
//! `seeded_continuation` child owns append-only admission and transactional
//! recovery; both routes use the same lowering operations. `declarations`
//! types each declaration form; `signatures` owns shared parameters and callable
//! interfaces; `contracts` owns facts, invocations and parameter obligations.
//! `expressions` types expressions and statements; `type_reference` owns type
//! references, domain aliases and constraint normalization.

// The typing tests' front-end pipelines, one file shared with the `suite`
// integration target; see its module documentation.
#[cfg(test)]
#[path = "../tests/support/front_end.rs"]
mod front_end;

// Lets the shared front-end module above spell this crate's typing entry point
// the same way the integration target does.
#[cfg(test)]
extern crate self as symbol_resolved_trees_to_typed_trees;

mod declarations {
    pub(crate) mod conformance;
    #[cfg(test)]
    mod conformance_tests;
    pub(crate) mod data;
    pub(crate) mod domain;
    pub(crate) mod machine;
    pub(crate) mod mathematical;
    pub(crate) mod measure;
    pub(crate) mod operator;
    pub(crate) mod proposition;
    pub(crate) mod state;
    pub(crate) mod trait_definition;
    pub(crate) mod wire;
}

mod signatures {
    pub(crate) mod callable_signature;
    pub(crate) mod parameters;
    #[cfg(test)]
    mod type_parameter_tests;
    pub(crate) mod type_parameters;
}

mod contracts {
    pub(crate) mod invocations;
    pub(crate) mod parameter_domains;
    pub(crate) mod proof_facts;
}
mod expressions;
mod lowerer;
mod type_reference;

pub use lowerer::lower_symbol_resolved_trees;
pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base, retained_typed_base_is_exact_prefix,
};
