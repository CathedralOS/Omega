//! Optimizer module role: executable entrance. Exact constant state-argument dispatch specialization boundary.
//!
//! One bounded specialization family: a dispatch state is a non-entry block
//! whose terminator is a `Conditional` reading one of that block's own scalar
//! parameters — the state argument. The condition reads the parameter either
//! directly (a Boolean argument) or through an in-block integer comparison
//! `parameter CMP literal` (an integer argument), where every other node in
//! the block is a pure scalar constant. When an incoming edge entering the
//! dispatch binds that parameter to an argument proven to be one exact
//! constant the condition resolves — a Boolean for the direct read, an
//! integer evaluated under the operand type's own ordering for the
//! comparison — the traversal "this edge, then the dispatch's resolved arm"
//! specializes into a single edge: the incoming edge is retargeted to the
//! resolved arm's block with its scalar bindings composed through the edge's
//! own bindings, and the fused edge carries both source edges' custody —
//! its own `PsiProvenance::Edge` first, then the resolved arm's — with one
//! paired fuel settlement each. The incoming edge is either an unconditional
//! `Jump` successor or one arm of a `Conditional` predecessor; a fused
//! conditional arm leaves its sibling arm byte-exact. The dispatch state and
//! both of its arm edges remain for every other incoming edge, so no source
//! work is removed and no block becomes unreachable.
//!
//! Two constant-proof sources admit the bound argument. First, the sparse
//! conditional constant analysis proves the argument's own value. Second —
//! the result specialization — the argument resolves to the scalar `result`
//! of an in-function direct `Call` whose callee's function carries exactly
//! one `Return`, and the callee's own lattice proves that returned value
//! constant: because the lowering delivers evaluated state-call arguments
//! through single-predecessor forwarding blocks, the bound argument is first
//! resolved through unique-incoming-edge parameter bindings to the delivered
//! value, then that value is matched to its producing `Call`. The call still
//! executes at the predecessor — only its proven result resolves the
//! dispatch — so no callee purity is required, a callee with several
//! `Return` nodes or a non-constant return admits nothing, and dynamic,
//! structural, and boundary calls produce their results through different
//! operations and never match.
//!
//! Only machines absent from the authenticated Terminal-cycle component roster
//! are eligible: a machine containing a verified cyclic component is frozen
//! byte-exact under `validate_frozen_component_blocks`, so edge surgery there
//! can never be admitted.
//!
//! This family owns admission and the plan only. `propose::plan` derives one
//! dispatch state's complete `StateArgumentSpecializationRewrite` — every
//! constant-supplied incoming edge with its supplying edge, resolved arm, and
//! proven constant — and `accounting::provenance_rows` derives the edge
//! custody that plan carries. `rules::state_specialization` publishes the
//! plan as a `PsiRewriteCandidate`, and
//! `optimization_unit_semantics::validate_state_argument_specialization_candidate`
//! re-admits every row, rebuilds the output, and reconstructs the custody
//! independently, so a forged or mismatched edge fails by recomputation
//! rather than trust. There is no second proposal or application route.

use optimization_unit::{
    NodeLocation, OptimizationBlock, OptimizationEdge, OptimizationNode, ProvenanceDisposition,
    ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit, PsiRealizationSite,
    SpecializedStateEdgeRow, StateArgumentSpecializationRewrite,
};
use semantic_vocabulary::BlockId;

use abstract_operations::AbstractOperation as O;

use crate::{ScalarConstant, ScalarConstantAnalysis};

pub(crate) mod accounting;
mod admission;
pub(crate) mod propose;

#[cfg(test)]
mod tests;
