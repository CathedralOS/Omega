//! Optimizer module role: executable entrance. Proven-case membership specialization boundary.
//!
//! One bounded specialization family: a `StructuralCaseMembership`
//! observation whose `source` place carries a case the unit itself proves
//! folds to a `BooleanConstant` holding that verdict. Two proofs qualify.
//! Establishment: the position the observation names resolves to an
//! `EstablishScalarCase` — the observed place's own producer when it is an
//! operation result, the establishment a uniform whole
//! `Owned`/`SharedBorrow` block-parameter binding forwards to when every
//! incoming edge names the same place and nothing rewrites or mutably
//! re-lends it, or a stored-whole child's producer when a `Field` path
//! descends an `EstablishRecord` field carried by one owned, complete
//! structural argument of exactly the declared carrier type. Sole-case
//! roster: the place's declared structural type is a closed `Sum` or
//! `Mixed` roster of exactly one case, so every inhabitant of the place
//! holds that case independently of how it arrived — parameters, results,
//! and non-`EstablishScalarCase` producers all qualify. In either proof the
//! membership's Boolean answer is `proven_case == case` at every
//! observation site. The traversal "observe
//! the proven case" specializes into `BooleanConstant` at the same node: the
//! result keeps its value identity, the node keeps its
//! `PsiProvenance::Operation` custody and fuel settlement, and successors,
//! definitions, uses, and ownership events are unchanged.
//!
//! Memberships on places whose position resolves to no establishment and
//! whose declared type holds more than one case are not covered — a block
//! parameter whose incoming bindings diverge, bind a pathed place, or lend
//! write authority proves nothing either. A membership's
//! non-empty path names a nested position — a `Record`/`Mixed` common field,
//! a `FixedArray` element, or a `Reference` referent — which folds when the
//! establishment the path resolves to proves a case or the roster at the
//! resolved position closes over exactly one case; positions that resolve
//! to neither decline. Only machines absent from
//! the authenticated Terminal-cycle component roster are eligible: a machine
//! containing a verified cyclic component is frozen byte-exact under
//! `validate_frozen_component_blocks`.
//!
//! This family owns admission and the plan only. `propose::plan` derives one
//! place's complete `CaseMembershipSpecializationRewrite` — every proven
//! observation with its site, custody identity, proof basis, and verdict —
//! and `accounting::provenance_rows` derives the node custody that plan
//! carries. `rules::case_membership_specialization` publishes the plan as a
//! `PsiRewriteCandidate`, and
//! `optimization_unit_semantics::validate_case_membership_specialization_candidate`
//! re-admits every row, rebuilds the output, and reconstructs the custody
//! independently, so a forged or stale row fails by recomputation rather
//! than trust. There is no second proposal or application route.

use optimization_unit::{
    CaseMembershipSpecializationRewrite, FoldedCaseMembershipRow, NodeLocation,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRealizationSite,
};
use semantic_vocabulary::{OperationId, PlaceId, ScalarType, StructuralPlaceKind};

use abstract_operations::AbstractOperation as O;

pub(crate) mod accounting;
pub(crate) mod admission;
pub(crate) mod propose;

#[cfg(test)]
mod tests;
