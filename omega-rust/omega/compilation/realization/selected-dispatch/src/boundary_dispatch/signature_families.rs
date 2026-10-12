//! Finite signature families, probes and const identities.
//!
//! The roster extraction is owned by the typed representation
//! (`typed_trees::finite_family`) so the local `dyn` surface and the
//! selected-dispatch boundary share one authority. This module only re-exports
//! that authority under the boundary's historical names.

use typed_trees::TypedTrees;

pub(crate) use typed_trees::finite_family::FamilyProbe;

/// Extract the explicit finite family declared by a generic requirement's
/// signature `where` clause. See [`TypedTrees::finite_signature_family`].
pub(crate) fn finite_signature_family(
    typed: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> FamilyProbe {
    typed.finite_signature_family(signature)
}
