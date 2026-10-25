#![forbid(unsafe_code)]

//! Attaches type and signature meaning to Psi symbol-resolved source trees.
//!
//! One entrance: [`lower_symbol_resolved_trees`], in `lowerer.rs`. Read that
//! function to read this stage — the module order below is the order it runs
//! them in. The `Lowerer` it builds there owns the growing typed trees and the
//! exposure scope every module below borrows.
//!
//! 1. `expressions` first rejects the resolved shapes typing is about to
//!    erase: `==` against bare payload-bearing case names, synthesized
//!    `Equatable` rows over malformed types, and non-exhaustive case dispatch. Those checks
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
//! Two operations in `lowerer.rs` carry the whole stage and every route
//! through it: `type_resolved_cohort` types the declaration suffix one
//! `TypingCohort` names, and `close_typed_cohort` settles that cohort and
//! publishes the identities later stages read. The fresh entrance and both
//! continuation routes run those two operations; they differ only in the
//! cohort and in the retained-base checks that admit it, so `lowerer.rs` is
//! where a step added to typing is added once rather than at three call sites.
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
mod placed_borrow_factory;
mod type_reference;
pub use placed_borrow_factory::placed_borrow_factory;

// Beneath `declarations`, not a step: the interface vocabulary every
// declaration form binds through.
mod signatures;

// The entrance.
pub use lowerer::lower_symbol_resolved_trees;

// The append-only continuation seam: orchestration that extends a retained
// typed base instead of retyping the whole package.
pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base,
};

// The typing tests' front-end pipelines, one file shared with the `suite`
// integration target; see its module documentation.

// Lets the shared front-end module above spell this crate's typing entry point
// the same way the integration target does.
