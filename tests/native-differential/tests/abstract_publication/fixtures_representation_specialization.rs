//! Representation-specialization projection fixture: a source-produced machine
//! whose `EstablishScalarCase` producer proves two membership observations.

use super::{VerifiedPsiOptimizationUnit, verified};
use checked_trees_to_lowered_psi::lower_machine;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

/// Two memberships observe one established place: `c in Choice::Some` folds
/// to `true` and `c in Choice::Empty` folds to `false` inside a single
/// candidate covering the place, so the pass commits exactly once.
const ESTABLISHED_MEMBERSHIPS_SOURCE: &str = r#"
    data Choice { case Empty; case Some(value: u32); }
    machine probe() -> bool {
        let c: Choice = Choice::Some { value: 37 };
        (c in Choice::Some) == (c in Choice::Empty)
    }
"#;

/// Two scalar field reads observe one established place: `p.x` folds to `37`
/// and `p.flag` folds to `true` inside a single candidate covering the place
/// under the `EstablishRecord` producer basis, so the field-value rule
/// commits exactly once.
const ESTABLISHED_FIELDS_SOURCE: &str = r#"
    data Point { x: u32; flag: bool; }
    machine probe() -> bool {
        let p: Point = Point { x: 37, flag: true };
        (p.x == 37) == p.flag
    }
"#;

/// A real source-produced membership machine: the full Psi front half lowers
/// `probe` to Terminal Psi, then the ordinary artifact admission builds the
/// verified optimization unit — no hand-constructed structural places.
pub(super) fn representation_specialization_membership_verified() -> VerifiedPsiOptimizationUnit {
    let tokens = Lexer::new(ESTABLISHED_MEMBERSHIPS_SOURCE)
        .tokenize()
        .expect("tokenize established memberships");
    let syntax = parse_syntax_trees(&tokens).expect("parse established memberships");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve established memberships");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type established memberships");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("check established memberships");
    let lowered = lower_machine(&checked, "probe").expect("lower established memberships");
    verified(lowered.semantic_module, lowered.proof_bundle)
}

/// A real source-produced field-read machine: the full Psi front half lowers
/// `probe` to Terminal Psi, then the ordinary artifact admission builds the
/// verified optimization unit — no hand-constructed structural places.
pub(super) fn representation_specialization_field_value_verified() -> VerifiedPsiOptimizationUnit {
    let tokens = Lexer::new(ESTABLISHED_FIELDS_SOURCE)
        .tokenize()
        .expect("tokenize established field values");
    let syntax = parse_syntax_trees(&tokens).expect("parse established field values");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("resolve established field values");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type established field values");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("check established field values");
    let lowered = lower_machine(&checked, "probe").expect("lower established field values");
    verified(lowered.semantic_module, lowered.proof_bundle)
}
