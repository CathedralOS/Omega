//! Representation-specialization projection fixture: a source-produced machine
//! whose `EstablishScalarCase` producer proves two membership observations.

use super::{VerifiedPsiOptimizationUnit, verified};
use checked_trees_to_lowered_psi::lower_machine;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;
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
    let checked = lower_typed_trees(typed).expect("check established memberships");
    let lowered = lower_machine(&checked, "probe").expect("lower established memberships");
    verified(lowered.semantic_module, lowered.proof_bundle)
}
