#![forbid(unsafe_code)]

//! Attaches type and signature meaning to Psi symbol-resolved source trees.
//!
//! One entrance: [`lower_symbol_resolved_trees`], in `lowerer.rs`. Read that
//! function to read this stage — the module order below is the order it runs
//! them in. The `Lowerer` it builds there owns the growing typed trees and the
//! exposure scope every module below borrows.
//!
//! 1. `expressions` first rejects the resolved shapes typing is about to
//!    erase: `==` against bare payload-bearing case names, malformed
//!    `Equatable` conformances, and non-exhaustive case dispatch. Those checks
//!    must run while membership is still a distinct node.
//! 2. `declarations` then types each declaration form, in the order the
//!    entrance walks the package: constants, data, domains, propositions,
//!    mathematical definitions, machines, measures, operators, traits,
//!    conformances, wire schemas. Each form re-enters `expressions` for its
//!    bodies and statements.
//! 3. `contracts` interns what those declarations promise — authored contract
//!    invocations, `where` facts, and parameter domain membership.
//! 4. `type_reference` owns every written type: type references, domain
//!    aliases, generic origins, and constraint normalization.
//!
//! `lowerer::finish` closes the stage. It settles satisfied declarations and
//! normalizes progress premises, rebuilds the typed trees, then runs the
//! normalization passes the last three modules own over that rebuild: domain
//! constraints, proof membership interning, qualification casts, fixed byte
//! array literals, and range argument validation.
//!
//! `signatures` is not a step of its own. The entrance never calls it; it is
//! the shared parameter, type-parameter and callable-interface vocabulary each
//! declaration form binds through.
//!
//! `lowerer::seeded_continuation` is an orchestration seam rather than a step:
//! it wraps this same entrance so a retained typed base can be extended
//! append-only, and fails the transaction rather than mutating a base that has
//! drifted.

// The entrance and the shared lowering state it owns.
mod lowerer;

// The route, in the order `lower_symbol_resolved_trees` runs it.
mod contracts;
mod declarations;
mod expressions;
mod type_reference;

// Beneath `declarations`, not a step: the interface vocabulary every
// declaration form binds through.
mod signatures;

// The entrance.
pub use lowerer::lower_symbol_resolved_trees;

// The append-only continuation seam: orchestration that extends a retained
// typed base instead of retyping the whole package.
pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base, retained_typed_base_is_exact_prefix,
};

// The typing tests' front-end pipelines, one file shared with the `suite`
// integration target; see its module documentation.
#[cfg(test)]
#[path = "../tests/support/front_end.rs"]
mod front_end;

// Lets the shared front-end module above spell this crate's typing entry point
// the same way the integration target does.
#[cfg(test)]
extern crate self as symbol_resolved_trees_to_typed_trees;
