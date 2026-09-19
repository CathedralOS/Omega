//! Optimizer module role: stage group. Abstract-optimization admission and publication replay.
//!
//! Independent unit and rewrite meaning lives in optimization-unit-semantics.
//! These checks additionally consume the preceding stage's sealed Terminal input.

use abstract_operations::AbstractOperation as O;
use optimization_unit::*;
use optimization_unit_semantics::*;
use semantic_vocabulary::*;
use std::collections::{BTreeMap, BTreeSet};

mod context;
mod prephysical_manifest;
mod projection;
mod scalar_case_frontiers;

pub(crate) use scalar_case_frontiers::{
    relocated_scalar_case_result_places, rewrite_relocated_case_result_frontiers,
};

pub use context::{
    ValidatedOptimizerCycleComponents, ValidatedOptimizerRankingCertificates,
    validate_psi_cycle_component_snapshot, validate_psi_ranking_certificate_snapshot,
    validate_transformed_psi_cycle_components, validate_transformed_psi_optimization_unit,
    validate_verified_psi_cycle_components, validate_verified_psi_optimization_unit,
};

/// Scalar-constant leaf nodes are one operation class admitted for
/// loop-invariant motion out of a cyclic component. They read no values,
/// carry no control flow or ownership events, and keep their own operation
/// identity as the first provenance row, so an independent validator can track
/// the exact source node across the relocation.
pub(crate) fn admissible_scalar_leaf_relocation(node: &OptimizationNode) -> bool {
    let psi_operation = match &node.operation {
        O::IntegerConstant { psi_operation, .. }
        | O::IeeeFloatConstant { psi_operation, .. }
        | O::BooleanConstant { psi_operation, .. } => *psi_operation,
        _ => return false,
    };
    node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
        && node.uses.is_empty()
        && node.successors.is_empty()
        && node.ownership.is_empty()
}

/// Side-effect-free total scalar computations are the second operation class
/// admitted for loop-invariant motion. Every admitted variant is pure scalar
/// (no place, claim, call, service, or control payload) and either carries no
/// verifier obligation or carries one the checker already discharged against
/// its operand values — a range, divisor, or shift-count totality proof. The
/// discharge stays valid after relocation because operand substitution only
/// rebinds a member parameter to the representative every reaching edge
/// proves equal, or keeps a run-internal operand bound to the result identity
/// the same relocation run preserves; the obligation itself moves byte-exact
/// inside the operation. The non-speculative gate already guarantees the
/// moved node ran on every traversal, so executing it once in the preheader
/// cannot introduce a crash or an observation the source node did not already
/// own. The node must still name its own operation as the first provenance
/// row, define exactly one result, and carry no successors or ownership
/// events; whether its operand uses are actually loop-invariant is decided
/// per use site by [`invariant_scalar_operand_substitution`].
pub(crate) fn admissible_invariant_scalar_computation(node: &OptimizationNode) -> bool {
    let psi_operation = match &node.operation {
        O::BooleanNot { psi_operation, .. }
        | O::BooleanEqual { psi_operation, .. }
        | O::IntegerEqual { psi_operation, .. }
        | O::IntegerLessThan { psi_operation, .. }
        | O::IntegerLessOrEqual { psi_operation, .. }
        | O::IntegerBitwiseNot { psi_operation, .. }
        | O::IntegerBitwiseAnd { psi_operation, .. }
        | O::IntegerBitwiseOr { psi_operation, .. }
        | O::IntegerBitwiseXor { psi_operation, .. }
        | O::IntegerWiden { psi_operation, .. }
        | O::IntegerExactCast { psi_operation, .. }
        | O::WrappingIntegerShiftLeft { psi_operation, .. }
        | O::WrappingIntegerShiftRight { psi_operation, .. }
        | O::ExactIntegerShiftLeft { psi_operation, .. }
        | O::ExactIntegerShiftRight { psi_operation, .. }
        | O::WrappingIntegerAdd { psi_operation, .. }
        | O::ExactIntegerAdd { psi_operation, .. }
        | O::SaturatingIntegerAdd { psi_operation, .. }
        | O::WrappingIntegerSubtract { psi_operation, .. }
        | O::ExactIntegerSubtract { psi_operation, .. }
        | O::SaturatingIntegerSubtract { psi_operation, .. }
        | O::WrappingIntegerMultiply { psi_operation, .. }
        | O::ExactIntegerMultiply { psi_operation, .. }
        | O::SaturatingIntegerMultiply { psi_operation, .. }
        | O::WrappingIntegerDivide { psi_operation, .. }
        | O::ExactIntegerDivide { psi_operation, .. }
        | O::SaturatingIntegerDivide { psi_operation, .. }
        | O::WrappingIntegerRemainder { psi_operation, .. }
        | O::ExactIntegerRemainder { psi_operation, .. }
        | O::SaturatingIntegerRemainder { psi_operation, .. }
        | O::IeeeFloatCompare { psi_operation, .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. } => *psi_operation,
        _ => return false,
    };
    node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
        && !node.uses.is_empty()
        && node.definitions.len() == 1
        && node.successors.is_empty()
        && node.ownership.is_empty()
}

/// Place-observation leaf nodes are the first non-scalar family admitted for
/// loop-invariant motion out of a cyclic component: a fresh observation of an
/// established storage root. Every admitted variant is a verifier-approved
/// read that defines exactly one result, carries no scalar uses, successors,
/// or ownership events, and keeps its own operation identity as the first
/// provenance row. `ByteSequenceRead` is not a leaf here — its scalar
/// index/length operands and its bounds obligation add a second evidence
/// dimension — so it admits through [`admissible_invariant_byte_read`] and
/// [`invariant_byte_read_admission`] instead. Invariance of the observed root
/// and the root's preheader visibility are decided separately by
/// [`invariant_place_observation_admission`].
pub(crate) fn admissible_invariant_place_read(node: &OptimizationNode) -> Option<PlaceId> {
    let (psi_operation, source) = match &node.operation {
        O::PrimitiveScalarRead {
            psi_operation,
            source,
            ..
        }
        | O::StructuralCaseMembership {
            psi_operation,
            source,
            ..
        }
        | O::ByteSequenceLength {
            psi_operation,
            source,
            ..
        }
        | O::StructuralByteSequenceFieldLength {
            psi_operation,
            source,
            ..
        }
        | O::BooleanStructuralField {
            psi_operation,
            source,
            ..
        }
        | O::IntegerStructuralField {
            psi_operation,
            source,
            ..
        } => (*psi_operation, *source),
        _ => return None,
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(psi_operation))
        && node.definitions.len() == 1
        && node.uses.is_empty()
        && node.successors.is_empty()
        && node.ownership.is_empty())
    .then_some(source)
}

/// A `ByteSequenceRead` is the non-scalar family admitted after place
/// observations: still one verifier-approved observation of an established
/// storage root, but it additionally reads two scalar operands — the dynamic
/// `index` and the `length` a `ByteSequenceLength` on the same source defined —
/// and carries the bounds obligation the index proof produced. The node must
/// keep its own operation identity as the first provenance row, define exactly
/// one result, use exactly its `index` and `length` operands in operand order,
/// and carry no successors or ownership events. Root invariance and preheader
/// visibility are decided by the shared [`invariant_observation_root`]
/// resolution, operand invariance by [`invariant_byte_read_admission`]'s
/// substitution half.
pub(crate) fn admissible_invariant_byte_read(
    node: &OptimizationNode,
) -> Option<(PlaceId, ValueId, ValueId)> {
    let O::ByteSequenceRead {
        psi_operation,
        source,
        index,
        length,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.len() == 1
        && node.uses.len() == 2
        && node.uses[0].value == *index
        && node.uses[1].value == *length
        && node.successors.is_empty()
        && node.ownership.is_empty())
    .then_some((*source, *index, *length))
}

/// A `ByteSequenceSubslice` is the structural-producing member of the byte
/// observation family: still a verifier-approved read of an established
/// storage root's extent, but it additionally evaluates two scalar endpoints,
/// pairs them with the `length` a `ByteSequenceLength` on the same source
/// defined, and establishes a fresh view root under its bounds obligation.
/// The node must keep its own operation identity as the first provenance row,
/// define no scalar value — its result is the structural view the operation
/// spells — use exactly its `start`, `end`, and `length` operands in operand
/// order, and carry no successors or ownership events. A result carrying
/// qualifications or claims keeps evidence rows this boundary does not yet
/// re-express, so only an unqualified view relocates. Root invariance and
/// preheader visibility are decided by the shared [`invariant_observation_root`]
/// resolution, operand invariance by [`invariant_subslice_admission`]'s
/// substitution half.
pub(crate) fn admissible_invariant_subslice(
    node: &OptimizationNode,
) -> Option<(PlaceId, ValueId, ValueId, ValueId)> {
    let O::ByteSequenceSubslice {
        psi_operation,
        result,
        source,
        start,
        end,
        length,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == 3
        && node.uses[0].value == *start
        && node.uses[1].value == *end
        && node.uses[2].value == *length
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then_some((*source, *start, *end, *length))
}

/// An `EstablishByteSequenceLiteral` is the first structural establishment
/// admitted for loop-invariant motion — the byte family's non-observation
/// member. The node declares a fresh immutable borrowed-view place over
/// constant bytes: it reads no scalar or structural operand, mutates no
/// existing place, and its declared root can anchor another parameter's
/// invariant representative only when the relocation run covers this node —
/// [`invariant_member_place_parameters`] treats an uncovered member-produced
/// root as loop-carried. The relocated operation therefore moves byte-exact — the place
/// declaration, structural type, and payload stay inside it — while every
/// consumer keeps spelling the same place identity. The node must still
/// name its own operation as the first provenance row, define no scalar
/// value (its result is the declared place), and carry no uses, successors,
/// or ownership events. Whether it may leave its member block at all is the
/// shared non-speculative gate applied by the proposal and replayed by the
/// freeze fence: establishing the view performs work a bypassed traversal
/// would not, so no leaf exemption applies.
pub(crate) fn admissible_invariant_byte_literal(node: &OptimizationNode) -> bool {
    let O::EstablishByteSequenceLiteral { psi_operation, .. } = &node.operation else {
        return false;
    };
    node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.is_empty()
        && node.successors.is_empty()
        && node.ownership.is_empty()
}

/// An `EstablishPrimitiveLocal` is the second structural establishment
/// admitted for loop-invariant motion — and the one a `CallStructuralScalar`
/// borrows: the cyclic eligibility fence only lets a shared-borrow
/// structural-scalar argument name a `let mut` primitive local's place, so
/// the call can leave its member block only when the establishment that
/// produced the borrowed root leaves in the same run. The node declares a
/// fresh claim-free mutable storage cell initialized to one scalar `value`:
/// it reads exactly that scalar operand, mutates no existing place, and its
/// declared root can anchor another parameter's invariant representative only
/// when the relocation run covers this node —
/// [`invariant_member_place_parameters`] treats an uncovered member-produced
/// root as loop-carried. The node must name its own operation as
/// the first provenance row, define no scalar value (its result is the
/// declared place), use exactly its `value` operand, carry no successors or
/// ownership events, and keep the result claim-free the way
/// `primitive_storage::local_result` requires — vacuous qualifications,
/// unrestricted multiplicity, and no claims — so the moved declaration still
/// validates as primitive-local storage. Whether `value` is actually
/// loop-invariant and whether the component preserves the local's stored
/// contents are decided separately by
/// [`invariant_primitive_local_admission`].
pub(crate) fn admissible_invariant_primitive_local(
    node: &OptimizationNode,
) -> Option<terminal_psi::StructuralOperationResult> {
    let O::EstablishPrimitiveLocal {
        psi_operation,
        result,
        value,
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == 1
        && node.uses[0].value == value.value
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then(|| result.clone())
}

/// The complete primitive-local-establishment admission shared by the
/// proposal and the relocation freeze replay: `node` must carry the
/// source-owned establishment shape
/// ([`admissible_invariant_primitive_local`]) — which yields the declared
/// place — the component must perform no place mutation or custody movement
/// ([`component_preserves_place_observations`]), and the initializing
/// `value` operand must obey the shared use-site invariance rule
/// ([`member_scalar_operand_substitution`]). The custody bound is what makes
/// hoisting a *re-established-every-iteration* local sound: the source
/// semantics hand each traversal a fresh cell initialized to `value`, so
/// moving the establishment into the preheader initializes that cell once —
/// only when no member stores to the declared place does the persistent
/// cell still read `value` on every traversal, which is exactly what a
/// mutation-free component guarantees. Returns the scalar substitution the
/// relocated establishment performs on `value`.
pub(crate) fn invariant_primitive_local_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    admissible_invariant_primitive_local(node)?;
    if !component_preserves_place_observations(function, component) {
        return None;
    }
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// An `EstablishRecord` is the third structural establishment admitted for
/// loop-invariant motion — the primitive local's multi-field sibling. The
/// node declares a fresh claim-free record place initialized
/// from declaration-ordered field initializers: it reads exactly the scalar
/// field values in field order, mutates no existing place, and carries no
/// successors or ownership events. A structural field initializer copies a
/// whole place root into the record — the cyclic-eligibility fence already
/// proved the copy is `Owned` over an unrestricted source with an empty
/// path, so it reads the root without moving custody — and its root's
/// landing is decided separately by [`invariant_record_admission`]. The
/// node must
/// name its own operation as the first provenance row, define no scalar
/// value (its result is the declared place), use exactly its scalar field
/// values in declaration order, and keep the result claim-free the way
/// `record::fields` requires for the cyclic-eligibility fence — vacuous
/// qualification, projection, and claim rosters, with either unrestricted
/// multiplicity or the affine multiplicity the fence admits only for an
/// empty declaration — so the moved declaration still validates as an owned
/// establishment. An affine result is admitted only when `fields` is empty:
/// the empty record is the composed-control spelling of a trivial affine
/// local, so its custody is the place itself with no initializer whose
/// contents could diverge between one establishment and per-traversal
/// re-establishment; an affine record carrying initializers has no admitted
/// custody model here and refuses. A bounded-integer field's
/// `range_obligation` is not an operand position: it was discharged against
/// the field value and operand substitution only rebinds a member parameter
/// to the representative every reaching edge proves equal, so the
/// obligation moves byte-exact inside
/// the operation. Whether the field values are actually loop-invariant and
/// whether the component preserves the record's fixed contents — or, for
/// an affine result, contains its disposal custody in a shape the
/// relocation can re-express — are decided separately by
/// [`invariant_record_admission`].
pub(crate) fn admissible_invariant_record(
    node: &OptimizationNode,
) -> Option<terminal_psi::StructuralOperationResult> {
    let O::EstablishRecord {
        psi_operation,
        result,
        fields,
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len()
            == fields
                .iter()
                .filter(|field| {
                    matches!(field.value, terminal_psi::RecordFieldValue::Scalar { .. })
                })
                .count()
        && node
            .uses
            .iter()
            .zip(fields.iter().filter_map(|field| match &field.value {
                terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(value),
                terminal_psi::RecordFieldValue::Structural(_) => None,
            }))
            .all(|(value_use, value)| value_use.value == *value)
        && fields.iter().all(|field| match &field.value {
            terminal_psi::RecordFieldValue::Scalar { .. } => true,
            terminal_psi::RecordFieldValue::Structural(argument) => {
                argument.access == terminal_psi::StructuralAccess::Owned && argument.path.is_empty()
            }
        })
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && (result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
            || (result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && fields.is_empty()))
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then(|| result.clone())
}

/// The complete record-establishment admission shared by the proposal and
/// the relocation freeze replay: `node` must carry the source-owned
/// establishment shape ([`admissible_invariant_record`]) — which yields the
/// declared place — and the custody bound matching its result multiplicity
/// must hold. An unrestricted result is copyable custody: the component
/// must perform no place mutation or custody movement
/// ([`component_preserves_place_observations`]), and each scalar
/// field value must obey the shared use-site invariance rule
/// ([`member_scalar_operand_substitution`]). The custody bound is what
/// makes hoisting a *re-established-every-iteration* record sound: the
/// source semantics hand each traversal a fresh record whose fields read
/// the initializer values, so moving the establishment into the preheader
/// binds those values once — only when no member stores to the declared
/// place does the persistent record still read those fields on every
/// traversal, which is exactly what a mutation-free component guarantees.
/// Each structural field initializer then copies a root the relocated run
/// must see: already visible at the unique preheader insertion point, the
/// representative an invariant member structural parameter resolves to
/// ([`invariant_member_place_parameters`]), or a root a node earlier in the
/// same run produced — `relocating_roots` — because the run preserves the
/// producer's declared place identity and orders it ahead of the record. A
/// member-produced root the run does not cover is re-established fresh
/// every traversal, so a record copying it cannot collapse into one
/// preheader copy and refuses. An affine result — admitted only with an
/// empty declaration — is the family's custody-rewriting shape the scalar
/// case introduced: the cyclic eligibility fence already confined the fresh
/// place to the member block that established it, discarding it on every
/// departing edge, so hoisting the establishment keeps the one persistent
/// result live through the whole component while member-internal edges stop
/// discarding it and every exit edge and member return disposes it instead.
/// [`scalar_case_result_contained`] is the bound proving the component only
/// ever spells the result through positions that rewrite covers; the empty
/// declaration reads no scalar or structural operand, so the substitution
/// and the structural-field rewrites below both come out empty. Returns the
/// scalar substitution plus the
/// `(member parameter or member-produced root, preheader-visible root)`
/// rewrites the relocated establishment performs on its structural field
/// arguments — empty when every copied root already names a visible or
/// run-produced place.
pub(crate) fn invariant_record_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    let result = admissible_invariant_record(node)?;
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine {
        if !scalar_case_result_contained(function, component, result.place) {
            return None;
        }
    } else if !component_preserves_place_observations(function, component) {
        return None;
    }
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let O::EstablishRecord { fields, .. } = &node.operation else {
        return None;
    };
    let preheader_source = shared_entry_source(component)?;
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)?;
    let representatives = invariant_member_place_parameters(function, component, relocating_roots);
    let mut rewrites = Vec::new();
    for field in fields {
        let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
            continue;
        };
        // The copied root must land somewhere the relocated run can see it
        // — the shared-borrow landing rule `borrow_call_admission` replays
        // for a call's structural arguments: already visible at the
        // preheader insertion point or uniquely produced by a node the same
        // run relocated (the argument keeps spelling the preserved place),
        // else the representative the member structural parameter resolves
        // to under the same two landings.
        let resolved = if place_observation_root_visible(function, preheader, argument.place)
            || (relocating_roots.contains(&argument.place)
                && member_root_producer_count(function, component, argument.place) == 1)
        {
            argument.place
        } else {
            let representative = *representatives.get(&argument.place)?;
            (place_observation_root_visible(function, preheader, representative)
                || (relocating_roots.contains(&representative)
                    && member_root_producer_count(function, component, representative) == 1))
                .then_some(representative)?
        };
        if resolved != argument.place {
            rewrites.push((argument.place, resolved));
        }
    }
    Some((substitution, rewrites))
}

/// An `EstablishScalarArray` is the fourth structural establishment admitted
/// for loop-invariant motion — the record establishment's flat sibling. The
/// node declares a fresh claim-free unrestricted scalar-array place over
/// row-major scalar leaves: it reads exactly the `elements` values in leaf
/// order, mutates no existing place, and carries no successors or ownership
/// events. The node must name its own operation as the first provenance row,
/// define no scalar value (its result is the declared place), use exactly its
/// `elements` operands in order, and keep the result claim-free the way the
/// cyclic-eligibility fence's `scalar_array::shape` requires — unrestricted
/// multiplicity and vacuous qualification, projection, and claim rosters — so
/// the moved declaration still validates as an owned establishment. Whether
/// the elements are actually loop-invariant and whether the component
/// preserves the array's fixed contents are decided separately by
/// [`invariant_scalar_array_admission`].
pub(crate) fn admissible_invariant_scalar_array(
    node: &OptimizationNode,
) -> Option<terminal_psi::StructuralOperationResult> {
    let O::EstablishScalarArray {
        psi_operation,
        result,
        elements,
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == elements.len()
        && node
            .uses
            .iter()
            .zip(elements.iter())
            .all(|(value_use, element)| value_use.value == *element)
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then(|| result.clone())
}

/// The complete scalar-array-establishment admission shared by the proposal
/// and the relocation freeze replay: `node` must carry the source-owned
/// establishment shape ([`admissible_invariant_scalar_array`]) — which yields
/// the declared place — the component must perform no place mutation or
/// custody movement ([`component_preserves_place_observations`]), and each
/// element operand must obey the shared use-site invariance rule
/// ([`member_scalar_operand_substitution`]). The custody bound is what makes
/// hoisting a *re-established-every-iteration* array sound: the source
/// semantics hand each traversal a fresh payload read from the element
/// values, so moving the establishment into the preheader binds that payload
/// once — only when no member stores to the declared place does the
/// persistent array still read those elements on every traversal, which is
/// exactly what a mutation-free component guarantees. Returns the scalar
/// substitution the relocated establishment performs on `elements`.
pub(crate) fn invariant_scalar_array_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    admissible_invariant_scalar_array(node)?;
    if !component_preserves_place_observations(function, component) {
        return None;
    }
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// An `EstablishScalarCase` is the fifth structural establishment admitted
/// for loop-invariant motion — and the family's first member whose affine
/// result carries a per-iteration disposal obligation the relocation must
/// re-express rather than preserve. The node declares a fresh claim-free sum
/// place initialized from declaration-ordered scalar case fields: it reads
/// exactly the `fields` values in order, mutates no existing place, and
/// carries no successors or ownership events. The node must name its own
/// operation as the first provenance row, define no scalar value (its result
/// is the declared sum place), use exactly its `fields` values in
/// declaration order, and keep the result claim-free the way the
/// cyclic-eligibility fence's `scalar_case::fields` requires — unrestricted
/// or affine multiplicity and vacuous qualification, projection, and claim
/// rosters — so the moved declaration still validates as an owned
/// establishment. A bounded-integer field's `range_obligation` is not an
/// operand position: it was discharged against the field value and operand
/// substitution only rebinds a member parameter to the representative every
/// reaching edge proves equal, so the obligation moves byte-exact inside the
/// operation. Whether the field values are actually loop-invariant — and,
/// for an affine result, whether the component contains the result's
/// dispatch custody in a shape the relocation can re-express — is decided
/// separately by [`invariant_scalar_case_admission`].
pub(crate) fn admissible_invariant_scalar_case(
    node: &OptimizationNode,
) -> Option<terminal_psi::StructuralOperationResult> {
    let O::EstablishScalarCase {
        psi_operation,
        result,
        fields,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == fields.len()
        && node
            .uses
            .iter()
            .zip(fields.iter())
            .all(|(value_use, field)| value_use.value == field.value)
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && matches!(
            result.multiplicity,
            terminal_psi::StructuralMultiplicity::Unrestricted
                | terminal_psi::StructuralMultiplicity::Affine
        )
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty())
    .then(|| result.clone())
}

/// The complete scalar-case-establishment admission shared by the proposal
/// and the relocation freeze replay: `node` must carry the source-owned
/// establishment shape ([`admissible_invariant_scalar_case`]) — which yields
/// the declared sum place — and each scalar field value must obey the shared
/// use-site invariance rule ([`member_scalar_operand_substitution`]). An
/// unrestricted result is copyable custody like a record's or array's: the
/// component must perform no place mutation or custody movement
/// ([`component_preserves_place_observations`]) so a payload established
/// once still reads those fields on every traversal. An affine result is the
/// family's first relocation that rewrites retained custody: the cyclic
/// eligibility fence already confined the fresh sum to the member block that
/// dispatches or returns it, discarding the place on every dispatch edge, so
/// hoisting the establishment keeps the one persistent result live through
/// the whole component — stripping it from member-internal edges (each
/// traversal would otherwise discard the persistent place its next
/// traversal's dispatch inspects) and disposing it instead on every exit
/// edge and member return that did not already carry it. That re-expressed
/// frontier is exactly what the reconstructed ownership replay requires, and
/// [`scalar_case_result_contained`] is the bound proving the component only
/// ever spells the result through positions the rewrite covers. Returns the
/// scalar substitution the relocated establishment performs on `fields`.
pub(crate) fn invariant_scalar_case_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    let result = admissible_invariant_scalar_case(node)?;
    if result.multiplicity == terminal_psi::StructuralMultiplicity::Affine {
        if !scalar_case_result_contained(function, component, result.place) {
            return None;
        }
    } else if !component_preserves_place_observations(function, component) {
        return None;
    }
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// Whether the affine scalar-case, empty-record, or structural-call result
/// `picked` stays
/// inside `component`'s
/// member roster spelled only through positions the relocation's custody
/// rewrite covers: the producing establishment or call itself, a `StructuralCase`
/// dispatch or read-only inspection of the sum, a structural return's source
/// or exit-disposal roster, and the member-internal or exit edges whose
/// discard rosters the rewrite adjusts. The cyclic eligibility fence already
/// proved `picked` is produced and dispatched (or returned) in one member
/// block and discarded on every dispatch edge; once the establishment lands
/// in the preheader the result lives through every member, so any other
/// spelling — a store destination, a call's structural argument, a reference
/// carrier, an edge structural binding — would move or observe custody the
/// rewrite cannot re-express and refuses. An unmodeled member operation
/// fails closed for the same reason. `picked` on a residual discard, or on
/// an edge departing a non-member block, is impossible in a verified seed —
/// the result is dead outside the roster — and refuses anyway so the replay
/// never trusts it. A member `ReturnUnit` whose cleanup already invokes a
/// nominal receiver has no `DiscardRoot` slot the rewrite can spell, so that
/// one terminator shape refuses; every other member terminator is
/// expressible: member `Return`/`ReturnUnit` blocks insert
/// `DiscardRoot(picked)` in schedule order, a member `ReturnStructural`
/// either returns `picked` outright or lists it in `trivial_affine_discards`,
/// and a `Crash` carries only claims.
fn scalar_case_result_contained(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    picked: PlaceId,
) -> bool {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    for block in &function.blocks {
        for node in &block.nodes {
            let member = members.contains(&block.id);
            let confined = match &node.operation {
                // The producer spells its own result — a declaration, not a
                // use of an existing place — and only a member node can
                // produce it inside this component. A `CallStructural`
                // producer is the same declaration position: the admitted
                // relocation carries an affine claim-free result the cyclic
                // eligibility fence already confined to the producing
                // member block's dispatch or return. An `EstablishRecord`
                // producer is likewise the empty declaration's own place —
                // the admissibility gate confines an affine result to the
                // field-free shape, so no structural field argument can
                // spell `picked` here.
                O::EstablishScalarCase { result, .. }
                | O::EstablishRecord { result, .. }
                | O::CallStructural { result, .. }
                    if result.place == picked =>
                {
                    member
                }
                // Dispatch, inspection, and structural-return positions keep
                // the result inside re-expressible custody when they live
                // inside the roster.
                O::StructuralCase { source, .. }
                | O::StructuralCaseMembership { source, .. }
                | O::IntegerStructuralField { source, .. }
                | O::BooleanStructuralField { source, .. }
                | O::PrimitiveScalarRead { source, .. }
                | O::ByteSequenceRead { source, .. }
                | O::ByteSequenceSubslice { source, .. }
                | O::ByteSequenceLength { source, .. }
                | O::StructuralByteSequenceFieldLength { source, .. }
                    if *source == picked =>
                {
                    member
                }
                // A member structural return disposes `picked` through its
                // exit roster or returns it outright — either shape is
                // expressible — while a non-member return cannot name a
                // place that is dead outside the member roster.
                O::ReturnStructural { source, .. } => member || *source != picked,
                operation => {
                    // Every modeled operation spelling `picked` outside the
                    // whitelist — a store destination, a call argument, a
                    // produced root collision — refuses. An operation this
                    // scan cannot model could spell it anywhere, so a member
                    // occurrence fails closed; a non-member operation cannot
                    // spell `picked` in a verified seed at all.
                    let mut references = BTreeSet::new();
                    if member_place_references(operation, &mut references) {
                        !references.contains(&picked)
                    } else {
                        !member
                    }
                }
            };
            if !confined {
                return false;
            }
            if !member {
                continue;
            }
            // A member `Return`/`ReturnUnit` cleanup invoking a nominal
            // receiver has no `DiscardRoot` slot for `picked` — the cleanup
            // shape requires all-nominal actions once any appear — so the
            // custody rewrite could not express its disposal and the
            // relocation refuses.
            if let O::Return {
                cleanup_actions, ..
            }
            | O::ReturnUnit {
                cleanup_actions, ..
            } = &node.operation
                && cleanup_actions.iter().any(|action| {
                    matches!(
                        action,
                        terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
                    )
                })
            {
                return false;
            }
            for edge in &node.successors {
                if edge
                    .residual_affine_discards
                    .iter()
                    .any(|discard| discard.place == picked)
                    || edge.structural_bindings.iter().any(|binding| {
                        binding.parameter == picked || binding.argument.place == picked
                    })
                {
                    return false;
                }
            }
        }
    }
    // `picked` on an edge departing outside the member roster is dead-place
    // custody a verified seed cannot carry — refuse rather than trust it.
    function
        .blocks
        .iter()
        .filter(|block| !members.contains(&block.id))
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .all(|edge| {
            !edge.trivial_affine_discards.contains(&picked)
                && !edge
                    .residual_affine_discards
                    .iter()
                    .any(|discard| discard.place == picked)
        })
}

/// Rewrite one retained member node's edge and cleanup custody for the
/// relocated affine scalar-case, empty-record, and structural-call results
/// in
/// `case_results`: strip each such
/// place from every member-internal edge — the persistent preheader result
/// stays live across the traversal where the source's fresh place died at
/// dispatch — and insert it, in the Terminal cleanup schedule's order, on
/// every exit edge that did not already dispose it, in every member
/// `Return`/`ReturnUnit` cleanup roster, and in a member `ReturnStructural`'s
/// `trivial_affine_discards` when some other root is returned. The operation's
/// own edge lists, the `successors` mirror, and the `Cleanup` ownership
/// mirror are rewritten in lockstep so the transformed node still satisfies
/// `successors_match_operation` and `expected_ownership`. The same rewrite
/// normalizes the seed's retained nodes inside the relocation freeze replay,
/// so the fence compares the admitted transformed spelling against the same
/// re-derived custody rather than trusting it.
pub(crate) fn rewrite_scalar_case_custody(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    members: &BTreeSet<BlockId>,
    case_results: &BTreeSet<PlaceId>,
    node: &mut OptimizationNode,
) {
    if case_results.is_empty() {
        return;
    }
    match &mut node.operation {
        O::Jump {
            target,
            trivial_affine_discards,
            ..
        } => {
            rewrite_case_edge_discards(
                structural_places,
                members,
                *target,
                case_results,
                trivial_affine_discards,
            );
        }
        O::Conditional {
            when_true,
            when_false,
            ..
        } => {
            for successor in [when_true, when_false] {
                rewrite_case_edge_discards(
                    structural_places,
                    members,
                    successor.target,
                    case_results,
                    &mut successor.trivial_affine_discards,
                );
            }
        }
        O::StructuralCase { cases, .. } => {
            for case in cases {
                rewrite_case_edge_discards(
                    structural_places,
                    members,
                    case.target,
                    case_results,
                    &mut case.trivial_affine_discards,
                );
            }
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for place in case_results {
                insert_case_cleanup_discard(structural_places, cleanup_actions, *place);
            }
        }
        O::ReturnStructural {
            source,
            trivial_affine_discards,
            ..
        } => {
            for place in case_results {
                if *source != *place {
                    insert_case_discard_ordered(structural_places, trivial_affine_discards, *place);
                }
            }
        }
        _ => {}
    }
    for edge in &mut node.successors {
        rewrite_case_edge_discards(
            structural_places,
            members,
            edge.target,
            case_results,
            &mut edge.trivial_affine_discards,
        );
    }
    for event in &mut node.ownership {
        if let OwnershipEvent::Cleanup(actions) = event {
            for place in case_results {
                insert_case_cleanup_discard(structural_places, actions, *place);
            }
        }
    }
}

/// Adjust one edge's `trivial_affine_discards` for relocated scalar-case
/// results: a member-internal target keeps the persistent place live, so its
/// discard is stripped; an exit target disposes it at the position the
/// Terminal cleanup schedule assigns — operation-result places precede
/// locals and parameters, ordered by descending producer.
fn rewrite_case_edge_discards(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    members: &BTreeSet<BlockId>,
    target: BlockId,
    case_results: &BTreeSet<PlaceId>,
    discards: &mut Vec<PlaceId>,
) {
    if members.contains(&target) {
        discards.retain(|place| !case_results.contains(place));
    } else {
        for place in case_results {
            insert_case_discard_ordered(structural_places, discards, *place);
        }
    }
}

/// Insert `place` into an ordered `trivial_affine_discards` roster at the
/// slot the Terminal cleanup schedule assigns: every relocated case result
/// is an operation-result place, and the schedule sorts results by
/// descending producer with declaration order breaking ties, ahead of every
/// local and parameter. A roster already listing `place` — a dispatch edge
/// that exits the component — is left byte-exact.
fn insert_case_discard_ordered(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    discards: &mut Vec<PlaceId>,
    place: PlaceId,
) {
    if discards.contains(&place) {
        return;
    }
    let Some(key) = case_result_schedule_key(structural_places, place) else {
        return;
    };
    let position = discards
        .iter()
        .position(|existing| {
            case_result_schedule_key(structural_places, *existing)
                .is_none_or(|existing_key| existing_key >= key)
        })
        .unwrap_or(discards.len());
    discards.insert(position, place);
}

/// Insert `DiscardRoot(place)` into a `Return`/`ReturnUnit` cleanup roster at
/// the same schedule slot [`insert_case_discard_ordered`] computes: ahead of
/// the first action that is not an operation-result root with a strictly
/// earlier schedule key, which keeps it inside the leading root run ahead of
/// any residual or parameter actions.
fn insert_case_cleanup_discard(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    actions: &mut Vec<terminal_psi::TerminalAffineCleanupAction>,
    place: PlaceId,
) {
    if actions.iter().any(|action| {
        matches!(
            action,
            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(existing)
                if *existing == place
        )
    }) {
        return;
    }
    let Some(key) = case_result_schedule_key(structural_places, place) else {
        return;
    };
    let position = actions
        .iter()
        .position(|action| {
            !matches!(
                action,
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(existing)
                    if case_result_schedule_key(structural_places, *existing)
                        .is_some_and(|existing_key| existing_key < key)
            )
        })
        .unwrap_or(actions.len());
    actions.insert(
        position,
        terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place),
    );
}

/// The schedule key `expected_trivial_affine_discards` assigns an
/// operation-result place: `Reverse(producer)` — later producers discard
/// first — with the `structural_places` declaration index preserving the
/// roster's stable order for equal producers. Locals and parameters have no
/// key and always sort after the operation-result run.
fn case_result_schedule_key(
    structural_places: &[terminal_psi::StructuralPlaceDeclaration],
    place: PlaceId,
) -> Option<(std::cmp::Reverse<OperationId>, usize)> {
    structural_places
        .iter()
        .enumerate()
        .find_map(|(index, declaration)| {
            (declaration.id == place).then_some(match declaration.kind {
                StructuralPlaceKind::OperationResult { producer, .. } => {
                    (std::cmp::Reverse(producer), index)
                }
                _ => return None,
            })
        })
}

/// The storage root an admitted place observation, byte read, or subslice
/// names — whichever observation gate the node's operation shape admits
/// through. `same_relocated_node` needs the expected root to replay the
/// member parameter's rebind without trusting the transformed unit's
/// spelling.
pub(crate) fn invariant_observation_source(node: &OptimizationNode) -> Option<PlaceId> {
    admissible_invariant_place_read(node)
        .or_else(|| admissible_invariant_byte_read(node).map(|(source, _, _)| source))
        .or_else(|| admissible_invariant_subslice(node).map(|(source, _, _, _)| source))
}

/// Whether `component`'s member blocks perform no place mutation or custody
/// movement visible to a member observation: no store, record establishment,
/// atomic event, or custody-moving ownership event inside a member, no call
/// that could reach a caller place through a mutating structural argument or
/// a transferred claim unless every such argument's root is exclusively the
/// call's own, and no
/// affine discard on any component-adjacent edge. A `ByteSequenceSubslice`,
/// an `EstablishByteSequenceLiteral`, an `EstablishPrimitiveLocal`, an
/// `EstablishRecord`, and an
/// `EstablishScalarArray` are
/// the only establishments this bound tolerates: the subslice reads its
/// source root's extent without mutating the root, the literal reads
/// nothing at all, the primitive local declares a fresh claim-free
/// storage cell without mutating an existing place, a record
/// establishment binds its field initializers into a fresh root — its
/// structural field copies read an unrestricted source without moving it —
/// and a scalar-array establishment binds its scalar leaves into a fresh
/// root — without mutating an existing place either. Each establishes only a
/// fresh root, and a member-produced root can anchor a rebind only when the
/// relocation run covers its producer
/// ([`invariant_member_place_parameters`]), so
/// no member observation of an existing root changes across traversals. A
/// member call node always carries one `ClaimTransfer` ownership row — the
/// custody mirror of its `claim_transfers` roster — so the bound reads the
/// row rather than requiring an empty list: only a row actually moving a
/// claim refuses. A member call carrying mutable or write-only borrows is
/// tolerated only when every root those borrows name is exclusive to the
/// call ([`exclusive_borrow_call_preserves_place_observations`]): the callee
/// can write a caller place only through those borrows — verified call
/// bindings hand a mutating parameter exactly the argument root — so its
/// writes land in a place no other member reads, borrows, or moves, which
/// leaves every member observation of every other root loop-invariant. An
/// `Owned` argument is tolerated on the same terms when its root declares a
/// copyable shape — an unrestricted owned parameter or an unrestricted
/// claim-free scalar-array result: the argument copies the payload into the
/// callee, so nothing the caller still holds changes hands.
/// When
/// this holds, every member place observation is loop-invariant — no
/// traversal can change what it observes — so an admitted read relocates
/// without a per-root write analysis. That whole-component bound is
/// deliberately conservative: it refuses every place read in a component
/// containing any other place-writing node rather than resolving member
/// structural parameters to decide which roots a store could reach.
pub(crate) fn component_preserves_place_observations(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> bool {
    component_preserves_place_observations_tolerating(function, component, &BTreeSet::new())
}

/// [`component_preserves_place_observations`] plus `tolerated`: a
/// member-departing edge — internal or exit — may discard a place in
/// `tolerated`, the run's already-relocated confined affine results (and the
/// admitting call's own), whose per-traversal disposal custody the
/// relocation re-expresses through [`rewrite_scalar_case_custody`] rather
/// than preserves. Every tolerated place's producer relocated under
/// `scalar_case_result_contained`, so the fresh member place's discards are
/// exactly the custody the persistent preheader result's frontier
/// reconstructs — internal edges keep it live, exits and member returns
/// dispose it. A tolerated place that is not an affine confined result can
/// never appear in `trivial_affine_discards`, so the set is inert for the
/// unrestricted roots `relocating_roots` also carries. Entry edges still
/// refuse every discard — a member-produced place cannot appear there in a
/// verified seed — and residual discards refuse absolutely: a relocated
/// confined result is trivial-affine custody, never nominal cleanup.
fn component_preserves_place_observations_tolerating(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    tolerated: &BTreeSet<PlaceId>,
) -> bool {
    for member in &component.members {
        let Some(block) = function.blocks.iter().find(|block| block.id == *member) else {
            return false;
        };
        for node in &block.nodes {
            // A member call's ownership roster is one claim-transfer row
            // spelling the same custody its `claim_transfers` carry: an empty
            // row moves nothing. Every other ownership event — completion,
            // cleanup, structural return, crash frontier — is custody
            // movement the bound refuses.
            let custody_quiet = node.ownership.iter().all(
                |event| matches!(event, OwnershipEvent::ClaimTransfer(claims) if claims.is_empty()),
            );
            if !custody_quiet
                || !(node_preserves_place_observations(&node.operation)
                    || exclusive_borrow_call_preserves_place_observations(
                        function, component, node,
                    ))
            {
                return false;
            }
        }
    }
    // Any discard adjacent to the component — on an internal edge, an entry
    // edge, or an exit — refuses the family, except a member-departing edge
    // may discard a tolerated place: the per-traversal disposal of a
    // confined result whose producer the run relocates. An entry-edge
    // discard runs once before the first iteration, but a place it ends
    // could not be read inside the loop at all, so the refusal is only
    // conservative.
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let adjacent: BTreeSet<EdgeId> = component
        .id
        .internal_edges
        .iter()
        .chain(component.entries.iter())
        .chain(component.exits.iter())
        .map(|edge| edge.edge)
        .collect();
    function
        .blocks
        .iter()
        .flat_map(|block| {
            let member_departing = members.contains(&block.id);
            block.nodes.iter().flat_map(move |node| {
                node.successors
                    .iter()
                    .map(move |edge| (member_departing, edge))
            })
        })
        .filter(|(_, edge)| adjacent.contains(&edge.psi_edge))
        .all(|(member_departing, edge)| {
            edge.residual_affine_discards.is_empty()
                && edge
                    .trivial_affine_discards
                    .iter()
                    .all(|place| member_departing && tolerated.contains(place))
        })
}

/// The operation whitelist [`component_preserves_place_observations`] applies
/// to every member node. Pure scalar work and scalar constants name no place;
/// read-only place observations cannot change what they observe; a byte
/// subslice reads its source root's extent and establishes only a fresh view
/// root, a byte-sequence literal establishes only a fresh immutable view
/// root over constant bytes, a primitive-local establishment declares
/// only a fresh claim-free storage cell, a record establishment binds
/// its initializers into a fresh root — a structural field copies an
/// unrestricted source without moving it — a scalar-array establishment
/// binds its scalar leaves into a fresh root, and a scalar-case
/// establishment binds scalar fields into a fresh sum root, so no existing
/// place mutates,
/// and a
/// fresh member-produced root anchors another parameter's invariant
/// representative only when its producer relocates in the same run; control
/// nodes carry their custody on their successor edges, which the edge scan
/// checks; a port write touches a service port rather than a place; a plain
/// scalar `Call` has no place or claim surface at all; and a unit, scalar,
/// or structural call that moves no claims and passes only shared-borrow
/// structural arguments cannot mutate any place it could observe. Every
/// other variant —
/// stores, affine-local establishments, dynamic-dispatch
/// calls, boundary calls, atomic events, descriptor stores — fails closed.
fn node_preserves_place_observations(operation: &O) -> bool {
    match operation {
        O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::ExactIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::ExactIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::ExactIntegerMultiply { .. }
        | O::SaturatingIntegerMultiply { .. }
        | O::WrappingIntegerDivide { .. }
        | O::ExactIntegerDivide { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::ExactIntegerRemainder { .. }
        | O::SaturatingIntegerRemainder { .. }
        | O::IeeeFloatCompare { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::PrimitiveScalarRead { .. }
        | O::StructuralCaseMembership { .. }
        | O::ByteSequenceRead { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceLength { .. }
        | O::StructuralByteSequenceFieldLength { .. }
        | O::BooleanStructuralField { .. }
        | O::IntegerStructuralField { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::PortWrite { .. }
        | O::DynamicDescriptorParameter { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishScalarCase { .. }
        | O::EstablishRecord { .. }
        | O::Call { .. } => true,
        O::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            claim_transfers,
            ..
        } => {
            claim_transfers.is_empty()
                && structural_arguments
                    .iter()
                    .all(|argument| argument.access == terminal_psi::StructuralAccess::SharedBorrow)
        }
        // A structural-result call carries `returned_claim_transfers` beside
        // the outgoing roster: a pure callee returning claims would spell a
        // second ownership row, which `custody_quiet` already refuses — the
        // field check keeps the same refusal local to the operation.
        O::CallStructural {
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } => {
            claim_transfers.is_empty()
                && returned_claim_transfers.is_empty()
                && structural_arguments
                    .iter()
                    .all(|argument| argument.access == terminal_psi::StructuralAccess::SharedBorrow)
        }
        _ => false,
    }
}

/// The second-tier member tolerance [`component_preserves_place_observations`]
/// applies to a call node the strict whitelist refuses: a `CallUnit`,
/// `CallStructuralScalar`, or `CallStructural` that moves no claims, whose
/// borrow arguments —
/// `SharedBorrow`, `MutableBorrow`, or `WriteOnlyBorrow` — are confined as
/// below, and whose `Owned` arguments each name a whole root whose declared
/// custody is copyable ([`copyable_owned_argument_root`]): an `Unrestricted`
/// owned parameter or an unrestricted claim-free scalar-array result. An
/// owned whole-root argument over an unrestricted payload copies the
/// elements into the callee — the caller's place keeps its contents, so the
/// argument is an observation, not custody movement — while an `Owned`
/// argument over an affine or linear root moves the caller's place into the
/// callee outright and refuses, as does any produced kind the cyclic
/// owned-argument fence does not recognize. Mutating borrows each name a
/// root no other member can observe. The callee's caller-visible write
/// authority is exactly its
/// mutable and write-only parameter roots — verified call bindings cannot
/// hand it another place — so confining those roots to the one call leaves
/// the bound's guarantee intact: no member observation of any place another
/// member wrote changes across traversals, and no custody moves.
fn exclusive_borrow_call_preserves_place_observations(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
) -> bool {
    let (structural_arguments, claim_transfers) = match &node.operation {
        O::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            claim_transfers,
            ..
        } => (structural_arguments, claim_transfers),
        // A structural-result call must also move no claims back: a
        // non-empty `returned_claim_transfers` would spell a second
        // ownership row the caller-side `custody_quiet` check refuses
        // anyway, so refusing here keeps the roster reasoning local.
        O::CallStructural {
            structural_arguments,
            claim_transfers,
            returned_claim_transfers,
            ..
        } if returned_claim_transfers.is_empty() => (structural_arguments, claim_transfers),
        _ => return false,
    };
    if !claim_transfers.is_empty() {
        return false;
    }
    let mut borrowed = BTreeSet::new();
    for argument in structural_arguments {
        match argument.access {
            terminal_psi::StructuralAccess::SharedBorrow => {}
            terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow => {
                borrowed.insert(argument.place);
            }
            // An owned whole-root argument over a copyable unrestricted
            // root copies the payload into the callee — a read of the
            // root's contents, not custody movement. An owned argument
            // over an affine or linear root, a projected owned argument,
            // or a produced kind outside the cyclic owned-argument fence's
            // admitted shapes moves the caller's place into the callee
            // outright.
            terminal_psi::StructuralAccess::Owned => {
                if !argument.path.is_empty()
                    || !copyable_owned_argument_root(function, argument.place)
                {
                    return false;
                }
            }
        }
    }
    // A mutating root must appear on exactly one argument in the borrower's
    // own roster: a second spelling — at any access — would alias the cell
    // the callee writes through, and this bound does not lean on the source
    // verifier's anti-aliasing rules to rule that roster out.
    if structural_arguments
        .iter()
        .filter(|argument| borrowed.contains(&argument.place))
        .count()
        != borrowed.len()
    {
        return false;
    }
    borrowed_roots_exclusive(function, component, node, &borrowed)
}

/// Whether every place in `roots` is referenced inside `component`'s member
/// roster only by `borrower` itself and, for a member-produced root, by the
/// one node that declares it. "Referenced" covers every place field a member
/// node can spell — an observed `source`, a store `destination`, a call's
/// structural-argument root at any access, a case-inspection root — and every
/// member structural parameter an edge landing inside the roster binds to a
/// borrowed root (or to another tainted parameter): the binding carries the
/// root's contents into the member, so a member spelling that parameter
/// observes the borrowed root even though it never names it directly. A
/// member node's own produced root is not a reference — declaring a fresh
/// place observes nothing. Member-adjacent edge discards spelling a tainted
/// place count as references too, even though the edge-discard half of
/// [`component_preserves_place_observations`] already refuses them.
///
/// Exclusivity is what makes a member's mutating borrow custody-preserving:
/// the callee's writes through the borrow land in a place no member read,
/// second borrow, store, case inspection, or moved node could observe, so
/// the only member that can ever see the root's contents is the borrowing
/// call itself — which relocation then reasons about directly.
fn borrowed_roots_exclusive(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    borrower: &OptimizationNode,
    roots: &BTreeSet<PlaceId>,
) -> bool {
    if roots.is_empty() {
        return true;
    }
    let Some(PsiProvenance::Operation(borrower_operation)) = borrower.provenance.first().copied()
    else {
        return false;
    };
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    // Member structural parameters that can carry a borrowed root: a fixed
    // point over the bindings of every edge landing inside the roster — a
    // binding whose argument spells a tainted place (at any projection
    // depth) makes its parameter an alias member nodes can observe through.
    let mut tainted: BTreeSet<PlaceId> = roots.clone();
    loop {
        let mut progressed = false;
        for edge in function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
            .filter(|edge| members.contains(&edge.target))
        {
            for binding in &edge.structural_bindings {
                if tainted.contains(&binding.argument.place) && tainted.insert(binding.parameter) {
                    progressed = true;
                }
            }
        }
        if !progressed {
            break;
        }
    }
    for member in &component.members {
        let Some(block) = function.blocks.iter().find(|block| block.id == *member) else {
            return false;
        };
        for node in &block.nodes {
            if node.provenance.first() == Some(&PsiProvenance::Operation(borrower_operation)) {
                continue;
            }
            let mut references = BTreeSet::new();
            if !member_place_references(&node.operation, &mut references) {
                return false;
            }
            if let Some(produced) = produced_place_root(&node.operation) {
                references.remove(&produced);
            }
            if references.iter().any(|place| tainted.contains(place)) {
                return false;
            }
            // A member terminator's own successor edges: a discard spelling a
            // tainted place is custody movement the exclusive-borrower
            // reading cannot allow, and a binding carrying one into a member
            // parameter is already accounted by the taint fixpoint.
            for edge in &node.successors {
                if edge
                    .trivial_affine_discards
                    .iter()
                    .any(|place| tainted.contains(place))
                    || edge
                        .residual_affine_discards
                        .iter()
                        .any(|discard| tainted.contains(&discard.place))
                {
                    return false;
                }
            }
        }
    }
    true
}

/// Collect every place `operation` can read, write, borrow, or move into
/// `references`, excluding the place it produces — the caller removes that
/// separately. Returns `false` for a variant whose place surface this scan
/// does not model, so the exclusivity check fails closed rather than
/// undercounting a member's references. Only place-free scalar work,
/// observations, stores, calls, and control variants are modeled; every
/// establishment's produced root is collected as well so the caller can
/// distinguish declaration from observation.
fn member_place_references(operation: &O, references: &mut BTreeSet<PlaceId>) -> bool {
    match operation {
        O::PrimitiveScalarRead { source, .. }
        | O::StructuralCaseMembership { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceSubslice { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::StructuralByteSequenceFieldLength { source, .. }
        | O::BooleanStructuralField { source, .. }
        | O::IntegerStructuralField { source, .. }
        | O::StructuralCase { source, .. } => {
            references.insert(*source);
        }
        O::PrimitiveLocalStore { destination, .. }
        | O::ByteSequenceWrite { destination, .. }
        | O::StructuralByteSequenceFieldByteStore { destination, .. } => {
            references.insert(*destination);
        }
        O::StructuralByteSequenceFieldStore {
            destination,
            source,
            ..
        } => {
            references.insert(*destination);
            references.insert(*source);
        }
        O::WriteOnlyPrimitiveStore { destination, .. }
        | O::StructuralScalarFieldStore { destination, .. } => {
            references.insert(destination.place);
        }
        // The three structural-signature call variants whose complete place
        // surface is `structural_arguments`: a dynamic-argument sibling's
        // descriptor arguments and a boundary call's completion claim
        // sources carry place surfaces this scan does not model, and a
        // `ReturnStructural` spells its returned place plus owned
        // declaration and discard rosters — all of those fail closed below.
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                references.insert(argument.place);
            }
        }
        // Establishments and scalar work: the establishment's declared place
        // is a production, not a reference; the caller removes it. A record
        // establishment's field initializers can embed a whole structural
        // argument — a borrowed or owned place moved into the record — so
        // each structural field value's root counts as a reference.
        // Constants,
        // scalar computations, plain scalar calls, port writes, descriptor
        // parameters, and non-structural terminators name no place at all.
        O::EstablishScalarArray { result, .. }
        | O::EstablishPrimitiveLocal { result, .. }
        | O::EstablishScalarCase { result, .. } => {
            references.insert(result.place);
        }
        O::EstablishRecord { result, fields, .. } => {
            references.insert(result.place);
            for field in fields {
                if let terminal_psi::RecordFieldValue::Structural(argument) = &field.value {
                    references.insert(argument.place);
                }
            }
        }
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => {
            references.insert(place.id);
        }
        O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::BooleanConstant { .. }
        | O::BooleanNot { .. }
        | O::BooleanEqual { .. }
        | O::IntegerEqual { .. }
        | O::IntegerLessThan { .. }
        | O::IntegerLessOrEqual { .. }
        | O::IntegerBitwiseNot { .. }
        | O::IntegerBitwiseAnd { .. }
        | O::IntegerBitwiseOr { .. }
        | O::IntegerBitwiseXor { .. }
        | O::IntegerWiden { .. }
        | O::IntegerExactCast { .. }
        | O::WrappingIntegerShiftLeft { .. }
        | O::WrappingIntegerShiftRight { .. }
        | O::ExactIntegerShiftLeft { .. }
        | O::ExactIntegerShiftRight { .. }
        | O::WrappingIntegerAdd { .. }
        | O::ExactIntegerAdd { .. }
        | O::SaturatingIntegerAdd { .. }
        | O::WrappingIntegerSubtract { .. }
        | O::ExactIntegerSubtract { .. }
        | O::SaturatingIntegerSubtract { .. }
        | O::WrappingIntegerMultiply { .. }
        | O::ExactIntegerMultiply { .. }
        | O::SaturatingIntegerMultiply { .. }
        | O::WrappingIntegerDivide { .. }
        | O::ExactIntegerDivide { .. }
        | O::SaturatingIntegerDivide { .. }
        | O::WrappingIntegerRemainder { .. }
        | O::ExactIntegerRemainder { .. }
        | O::SaturatingIntegerRemainder { .. }
        | O::IeeeFloatCompare { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::Call { .. }
        | O::PortWrite { .. }
        | O::DynamicDescriptorParameter { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::Crash { .. } => {}
        // Atomic events, dynamic calls, descriptor stores, and any variant
        // not modeled above fail closed: the exclusivity scan refuses rather
        // than guesses which fields carry places.
        _ => return false,
    }
    true
}

/// Whether `root`, the storage root an admitted place observation names, is
/// visible at the component's unique preheader insertion point: a
/// function structural parameter or result root, a provider-attachment root,
/// a structural parameter of the preheader block itself, or a place
/// established by a preheader node ahead of the terminator. The relocated run
/// inserts ahead of the countdown-certificate tail; those tail nodes are
/// scalar constants and never produce places, so scanning every
/// non-terminator preheader node here is exactly the proposal's
/// ahead-of-insertion computation. A root produced inside the component or a
/// place only another non-preheader block establishes stays invisible —
/// though [`invariant_observation_root`] still admits a member-produced root
/// the relocation run already covers, since its producer lands in the
/// preheader ahead of the run's consumers; a
/// member structural parameter is never itself visible but may rebind to a
/// visible or run-covered representative through
/// [`invariant_member_place_parameters`].
pub(crate) fn place_observation_root_visible(
    function: &PsiOptimizationFunction,
    preheader: &OptimizationBlock,
    root: PlaceId,
) -> bool {
    function
        .structural_parameters
        .iter()
        .any(|parameter| parameter.place == root)
        || function
            .result
            .structural()
            .is_some_and(|result| result.place == root)
        || function.structural_places.iter().any(|declaration| {
            declaration.id == root
                && matches!(
                    declaration.kind,
                    StructuralPlaceKind::ProviderAttachment { .. }
                )
        })
        || preheader
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == root)
        || preheader
            .nodes
            .iter()
            .take(preheader.nodes.len().saturating_sub(1))
            .any(|node| produces_place_root(&node.operation, root))
}

/// Whether `operation` establishes `root`: the structural-result places, the
/// declaration-carried literal and affine-local places, and a single-attempt
/// compare-exchange outcome. This is the producer half of
/// [`place_observation_root_visible`].
fn produces_place_root(operation: &O, root: PlaceId) -> bool {
    produced_place_root(operation) == Some(root)
}

/// The place root `operation` establishes, when it establishes one: the
/// structural-result places, the declaration-carried literal and
/// affine-local places, and a single-attempt compare-exchange outcome. A
/// relocated producer keeps its root byte-exact inside the moved operation,
/// so the relocation run's produced-root roster is exactly this relation
/// read off the moved nodes — a member node whose argument or observed root
/// a run-covered producer declared can treat the root as landing in the
/// preheader with its producer.
pub(crate) fn produced_place_root(operation: &O) -> Option<PlaceId> {
    match operation {
        O::EstablishPrimitiveLocal { result, .. }
        | O::ByteSequenceSubslice { result, .. }
        | O::EstablishScalarArray { result, .. }
        | O::EstablishScalarCase { result, .. }
        | O::EstablishRecord { result, .. }
        | O::CallStructural { result, .. } => Some(result.place),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Structural(result),
            ..
        } => Some(result.place),
        O::EstablishByteSequenceLiteral { place, .. }
        | O::EstablishTrivialAffineLocal { place, .. } => Some(place.id),
        O::AtomicEvent {
            event: abstract_operations::AbstractAtomicEvent::CompareExchangeOnce { outcome, .. },
            ..
        } => Some(outcome.place),
        _ => None,
    }
}

/// The place roots `operation` mutably borrows through its structural
/// arguments — the set the producer's move-together coupling and the
/// freeze's independent coverage replay both read. A mutable borrower can
/// read the cell it writes, so a relocated establishment would leave a
/// staying mutable borrower reading accumulated post-write contents where
/// the source traversal re-initialized the cell. Write-only borrows are not
/// collected: their callee cannot read the cell, so a staying write-only
/// borrower keeps writing deterministic contents no member observes.
pub(crate) fn mutable_borrow_roots(operation: &O) -> BTreeSet<PlaceId> {
    let structural_arguments = match operation {
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallUnitWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalarWithDynamicArguments {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments,
        _ => return BTreeSet::new(),
    };
    structural_arguments
        .iter()
        .filter(|argument| argument.access == terminal_psi::StructuralAccess::MutableBorrow)
        .map(|argument| argument.place)
        .collect()
}

/// The number of member nodes that produce `root` inside `component`'s
/// roster. A verified unit declares a fresh place per establishment, so an
/// admitted relocation expects exactly one; a roster producing a mut-borrowed
/// or run-consumed root twice would let a staying producer re-initialize the
/// cell a relocated consumer reads once, so the call admission and the
/// exclusivity scan both refuse every multi-producer spelling.
fn member_root_producer_count(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    root: PlaceId,
) -> usize {
    component
        .members
        .iter()
        .filter_map(|member| function.blocks.iter().find(|block| block.id == *member))
        .flat_map(|block| block.nodes.iter())
        .filter(|node| produced_place_root(&node.operation) == Some(root))
        .count()
}

/// Whether `root`'s declared custody makes an `Owned` whole-root argument a
/// copy rather than a custody move. The verifier binds an argument's declared
/// multiplicity equal to its callee parameter's, and the interpreter copies
/// an `Unrestricted` argument's payload into the callee's activation while
/// the caller keeps its place — where an affine or linear root spelled
/// `Owned` genuinely transfers custody. The admitted shapes are exactly the
/// ones the cyclic owned-argument fence recognizes, narrowed to the
/// copy-only multiplicities: a machine or block structural parameter
/// declared `!is_self`, `Owned`, `Unrestricted`, and qualification-free —
/// the fence's `plain_owned` shape without its `Affine` arm — and a
/// whole-root `EstablishScalarArray` result that is unrestricted,
/// claim-free, and qualification-free, produced by any block in the
/// function. Every other produced kind — and any root with restricted
/// custody — keeps the refusal: their `Owned` spellings genuinely transfer
/// custody or carry alias structure this boundary has not spelled out. Of
/// the three call families sharing this rule, only `CallStructuralScalar`
/// spells `Owned` arguments in a verified source unit today — attached
/// units confine call arguments to borrows, scalar-graph `CallUnit`
/// callees carry plain primitive parameters only, and a `CallStructural`
/// affine result confined to its own case terminator can never sit behind
/// an owned root — but the fence still answers for the other families:
/// the freeze replay independently re-derives admission on hand-moved
/// units, where no source grammar prunes the spellings first.
fn copyable_owned_argument_root(function: &PsiOptimizationFunction, root: PlaceId) -> bool {
    function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| block.structural_parameters.iter()),
        )
        .any(|parameter| {
            parameter.place == root
                && !parameter.is_self
                && parameter.access == terminal_psi::StructuralAccess::Owned
                && parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
        })
        || function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter())
            .any(|node| {
                matches!(
                    &node.operation,
                    O::EstablishScalarArray { result, .. }
                        if result.place == root
                            && result.multiplicity
                                == terminal_psi::StructuralMultiplicity::Unrestricted
                            && result.qualifications.is_empty()
                            && result.projected_qualifications.is_empty()
                            && result.claims.is_empty()
                )
            })
}

/// The complete invariant place-read admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned observation
/// shape ([`admissible_invariant_place_read`]), the component must perform no
/// place mutation or custody movement
/// ([`component_preserves_place_observations`]), and the root it observes must
/// land somewhere the relocated run can see it — visible at the unique
/// preheader insertion point ([`place_observation_root_visible`]), produced
/// by a node earlier in the same relocation run (`relocating_roots`), or
/// transitively when the observed root is a member structural parameter every
/// reaching edge binds to one such representative
/// ([`invariant_member_place_parameters`]). Returns the root the relocated
/// observation rebinds to: the node's own root when it is already
/// preheader-visible or run-covered, so a byte-exact move, a member-parameter
/// rebind, and a run-covered member-produced root share one admission.
pub(crate) fn invariant_place_observation_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<PlaceId> {
    let source = admissible_invariant_place_read(node)?;
    invariant_observation_root(function, component, source, relocating_roots)
}

/// The root an admitted observation rebinds to when it relocates: `source`
/// itself when it is already visible at the unique preheader insertion
/// point or produced by a node the same relocation run covers, so a
/// byte-exact move and a member-parameter rebind share one admission. The
/// whole-component place-custody gate
/// ([`component_preserves_place_observations`]) and the root's landing —
/// preheader visibility ([`place_observation_root_visible`]) or a uniquely
/// member-produced root the run already relocates — are enforced here so both
/// the proposal and the relocation freeze replay derive the same root from
/// the seed rather than trusting a plan. A run-covered root qualifies only
/// when its member producer is unique: a second producer would re-establish
/// the cell each traversal behind the relocated observation's single read.
pub(crate) fn invariant_observation_root(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    source: PlaceId,
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<PlaceId> {
    let preheader_source = shared_entry_source(component)?;
    if !component_preserves_place_observations(function, component) {
        return None;
    }
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)?;
    if place_observation_root_visible(function, preheader, source)
        || (relocating_roots.contains(&source)
            && member_root_producer_count(function, component, source) == 1)
    {
        return Some(source);
    }
    let representatives = invariant_member_place_parameters(function, component, relocating_roots);
    let representative = representatives.get(&source)?;
    (place_observation_root_visible(function, preheader, *representative)
        || (relocating_roots.contains(representative)
            && member_root_producer_count(function, component, *representative) == 1))
        .then_some(*representative)
}

/// The complete `ByteSequenceRead` admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned byte-read
/// shape ([`admissible_invariant_byte_read`]), its storage root must resolve
/// to a root the relocated run can see through the shared observation-root
/// admission ([`invariant_observation_root`]) — preheader-visible or produced
/// by a node the same run covers — and each scalar operand must satisfy the
/// same use-site invariance rule an admitted scalar computation obeys —
/// defined outside the component, an invariant member parameter rebound to
/// its agreed representative, or the preserved result of a node earlier in
/// the same relocation run.
///
/// The `length` operand carries one additional coupling the generic rule
/// cannot express: byte-view validation requires it to be defined by a
/// `ByteSequenceLength` measuring the very root the read observes. A
/// member-internal producer qualifies only when it relocates in the same run
/// and its own root resolves to the read's rebound root; a producer outside
/// the component qualifies only when it already measures that root. Anything
/// else — a member parameter, a function parameter, or a length observation
/// of a different root — would re-express the read against a length the
/// transformed unit could not validate, so the admission refuses.
///
/// Returns the root the relocated read rebinds to plus the operand
/// substitution its member-parameter uses need.
pub(crate) fn invariant_byte_read_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
    let (source, _, length) = admissible_invariant_byte_read(node)?;
    let root = invariant_observation_root(function, component, source, relocating_roots)?;
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let rebound_length = substitution.get(&length).copied().unwrap_or(length);
    byte_length_operand_measures_root(function, component, root, rebound_length, relocating_roots)
        .then_some((root, substitution))
}

/// The complete `ByteSequenceSubslice` admission shared by the proposal and
/// the relocation freeze replay: `node` must carry the source-owned subslice
/// shape ([`admissible_invariant_subslice`]), its storage root must resolve
/// to a root the relocated run can see through the shared observation-root
/// admission ([`invariant_observation_root`]) — preheader-visible or produced
/// by a node the same run covers — and each scalar operand — `start`,
/// `end`, and `length` — must satisfy the same use-site invariance rule an
/// admitted scalar computation obeys: defined outside the component, an
/// invariant member parameter rebound to its agreed representative, or the
/// preserved result of a node earlier in the same relocation run.
///
/// The `length` operand carries the same producer coupling a byte read obeys:
/// byte-view validation requires it to be defined by a `ByteSequenceLength`
/// measuring the very root the subslice observes, so the moved operation
/// still validates against its bounds obligation. The structural result —
/// the fresh view place with its type, multiplicity, and qualifications —
/// and the bounds obligation are not substitutable positions: the relocation
/// preserves them byte-exact rather than re-spelling them.
///
/// Returns the root the relocated subslice rebinds to plus the operand
/// substitution its member-parameter uses need.
pub(crate) fn invariant_subslice_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
) -> Option<(PlaceId, BTreeMap<ValueId, ValueId>)> {
    let (source, _, _, length) = admissible_invariant_subslice(node)?;
    let root = invariant_observation_root(function, component, source, relocating_roots)?;
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let rebound_length = substitution.get(&length).copied().unwrap_or(length);
    byte_length_operand_measures_root(function, component, root, rebound_length, relocating_roots)
        .then_some((root, substitution))
}

/// Whether `rebound_length` — the substituted `length` operand an admitted
/// byte read or subslice relocates with — is defined by a `ByteSequenceLength`
/// measuring `root`, the rebound storage root the operation observes. A
/// member-internal producer qualifies only when its own observation root
/// resolves to that same root under the same run-covered member roots — the
/// operand substitution keeps it bound only when it relocates in the same
/// run; a producer outside the component must already measure the root
/// directly.
fn byte_length_operand_measures_root(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    root: PlaceId,
    rebound_length: ValueId,
    relocating_roots: &BTreeSet<PlaceId>,
) -> bool {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let Some(ValueDefinitionSite::Node { block, node }) = sites.get(&rebound_length) else {
        return false;
    };
    let Some(producing) = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == *block)
        .and_then(|block| {
            usize::try_from(*node)
                .ok()
                .and_then(|node| block.nodes.get(node))
        })
    else {
        return false;
    };
    match &producing.operation {
        O::ByteSequenceLength {
            source: measured, ..
        } if members.contains(block) => {
            invariant_observation_root(function, component, *measured, relocating_roots)
                == Some(root)
        }
        O::ByteSequenceLength {
            source: measured, ..
        } => *measured == root,
        _ => false,
    }
}

/// Structural parameters of `component`'s member blocks whose root is
/// provably the same on every iteration — the place analog of
/// [`invariant_member_parameters`]. A member view parameter qualifies when
/// every edge reaching its block binds it to itself, to a member structural
/// parameter that resolves to the same representative, or to that
/// representative — a root no member block establishes, or a member-produced
/// root the relocation run already covers. An observation reading through
/// such a parameter can be re-expressed on the representative root when that
/// root is visible at the preheader insertion point or lands there with its
/// run-covered producer.
///
/// A projected binding (`argument.path` nonempty) cannot anchor a root
/// rebind: the observation grammar names one root place, so a parameter
/// bound to `self.field` or to a subview stays loop-carried. A binding to a
/// root a member block establishes — an operation result, a byte-sequence
/// literal, or an affine local — is likewise loop-carried when the run does
/// not cover its producer: the traversal re-establishes that root every
/// iteration. A run-covered member-produced root instead anchors like any
/// outside root — the relocated producer lands in the preheader ahead of the
/// run's consumers, so the root persists across traversals and the parameter
/// spells it on every one. Verified edge bindings already
/// guarantee the argument's access is compatible with the parameter's, so
/// the resolved representative carries at least the access the member
/// observation used.
///
/// The map is a fixed point over the structural bindings on edges reaching
/// member blocks, so a parameter carried across a member-to-member edge
/// resolves to the root its chain anchors on. Parameters whose bindings
/// never anchor outside the roster — pure self-carried cycles — stay
/// unresolved and are absent from the result.
pub(crate) fn invariant_member_place_parameters(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    relocating_roots: &BTreeSet<PlaceId>,
) -> BTreeMap<PlaceId, PlaceId> {
    /// Resolution states during the fixed point. `Unresolved` may promote once
    /// its deferred dependencies resolve; `Representative` can still degrade
    /// to `LoopCarried` when a deferred dependency resolves to a conflicting
    /// or carried root, and `LoopCarried` is final.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Resolution {
        Unresolved,
        Representative(PlaceId),
        LoopCarried,
    }

    let member_blocks: Vec<&OptimizationBlock> = component
        .members
        .iter()
        .filter_map(|member| function.blocks.iter().find(|block| block.id == *member))
        .collect();
    let member_parameters: BTreeSet<PlaceId> = member_blocks
        .iter()
        .flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
        })
        .collect();
    // Whether `place` is a root a member block establishes: every iteration
    // produces a fresh root, so a binding to one can anchor an invariant
    // representative only when the relocation run covers its producer — the
    // moved establishment lands in the preheader ahead of the run's
    // consumers, and the root then persists across traversals.
    let member_produced = |place: PlaceId| {
        member_blocks.iter().any(|block| {
            block
                .nodes
                .iter()
                .any(|node| produces_place_root(&node.operation, place))
        })
    };
    let parameters: Vec<(PlaceId, BlockId)> = member_blocks
        .iter()
        .flat_map(|block| {
            block
                .structural_parameters
                .iter()
                .map(|parameter| (parameter.place, block.id))
        })
        .collect();
    let mut resolutions: BTreeMap<PlaceId, Resolution> = parameters
        .iter()
        .map(|(parameter, _)| (*parameter, Resolution::Unresolved))
        .collect();
    loop {
        let mut progressed = false;
        for (parameter, block) in &parameters {
            if resolutions[parameter] == Resolution::LoopCarried {
                continue;
            }
            let mut anchor = None;
            let mut loop_carried = false;
            for edge in function
                .blocks
                .iter()
                .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
                .filter(|edge| edge.target == *block)
            {
                let Some(binding) = edge
                    .structural_bindings
                    .iter()
                    .find(|binding| binding.parameter == *parameter)
                else {
                    loop_carried = true;
                    break;
                };
                let argument = &binding.argument;
                if argument.place == *parameter && argument.path.is_empty() {
                    // A root binding that re-spells the parameter preserves
                    // whatever the other edges establish.
                    continue;
                }
                let contribution = if !argument.path.is_empty() {
                    // A projected argument names a sub-place, not a root the
                    // relocated observation could spell.
                    Resolution::LoopCarried
                } else if member_parameters.contains(&argument.place) {
                    // A member parameter contributes its own resolution; an
                    // unresolved dependency defers to a later pass.
                    resolutions
                        .get(&argument.place)
                        .copied()
                        .unwrap_or(Resolution::LoopCarried)
                } else if member_produced(argument.place)
                    && !relocating_roots.contains(&argument.place)
                {
                    // A member-produced root the run does not cover is
                    // re-established fresh every traversal — loop-carried.
                    // A covered root's producer lands in the preheader ahead
                    // of the run's consumers, so the root persists like any
                    // outside-established place and can anchor the
                    // representative.
                    Resolution::LoopCarried
                } else {
                    Resolution::Representative(argument.place)
                };
                match contribution {
                    Resolution::Unresolved => {}
                    Resolution::Representative(place) => match anchor {
                        None => anchor = Some(place),
                        Some(anchor) if anchor == place => {}
                        Some(_) => {
                            loop_carried = true;
                            break;
                        }
                    },
                    Resolution::LoopCarried => {
                        loop_carried = true;
                        break;
                    }
                }
            }
            let next = if loop_carried {
                Resolution::LoopCarried
            } else {
                match anchor {
                    Some(place) => Resolution::Representative(place),
                    None => Resolution::Unresolved,
                }
            };
            if resolutions[parameter] != next {
                resolutions.insert(*parameter, next);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    resolutions
        .into_iter()
        .filter_map(|(parameter, resolution)| match resolution {
            Resolution::Representative(representative) => Some((parameter, representative)),
            Resolution::Unresolved | Resolution::LoopCarried => None,
        })
        .collect()
}

/// Every scalar value definition site in `function`: function parameters,
/// block parameters, and node results. Sites are the only authority needed to
/// decide whether a use is loop-carried — a definition inside a component's
/// member blocks can change each iteration, and nothing else can.
pub(crate) fn value_definition_sites(
    function: &PsiOptimizationFunction,
) -> BTreeMap<ValueId, ValueDefinitionSite> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| node.definitions.iter()))
        }))
        .map(|definition| (definition.value, definition.site))
        .collect()
}

/// The block every one of `component`'s entry edges departs — the unique
/// preheader a relocation can target. Entries may arrive on several edges of
/// that one block's terminator (a multi-arm dispatch where every arm enters
/// the cycle); entries departing different blocks leave the component with no
/// shared preheader, and an entry source inside the roster is not a preheader
/// at all, so both decline.
pub(crate) fn shared_entry_source(component: &OptimizerCycleComponent) -> Option<BlockId> {
    let [first, rest @ ..] = component.entries.as_slice() else {
        return None;
    };
    (rest.iter().all(|entry| entry.source == first.source)
        && !component.members.contains(&first.source))
    .then_some(first.source)
}

/// Member blocks of `component` guaranteed to execute on every traversal that
/// leaves the component: they dominate every exit-edge source inside the
/// subgraph the component's internal edges induce over its member roster,
/// rooted at the entry targets — a traversal enters through any one of them,
/// so a member qualifies only when every path from every entry target to
/// every exit source passes through it. Relocating a node out of any other
/// member block would speculate executions the source traversal may never
/// perform — a bypassing exit can leave the component before the block runs —
/// so the scalar-motion boundary admits only these members even though every
/// admitted operation is total. A component with no exits admits every
/// member: no traversal leaves it. The bound is the standard
/// guaranteed-to-execute criterion; a traversal that enters and neither exits
/// nor completes an iteration may still bypass the block, which is the
/// residual speculation the boundary accepts.
pub(crate) fn guaranteed_executed_member_blocks(
    component: &OptimizerCycleComponent,
) -> BTreeSet<BlockId> {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let entry_targets: BTreeSet<BlockId> = component
        .entries
        .iter()
        .map(|entry| entry.target)
        .filter(|target| members.contains(target))
        .collect();
    if entry_targets.is_empty() {
        return BTreeSet::new();
    }
    let boundary: BTreeSet<BlockId> = component
        .exits
        .iter()
        .map(|exit| exit.source)
        .filter(|source| members.contains(source))
        .collect();
    let mut predecessors: BTreeMap<BlockId, BTreeSet<BlockId>> = members
        .iter()
        .map(|member| (*member, BTreeSet::new()))
        .collect();
    for edge in &component.id.internal_edges {
        if members.contains(&edge.source) && members.contains(&edge.target) {
            predecessors
                .get_mut(&edge.target)
                .expect("member target has a predecessor row")
                .insert(edge.source);
        }
    }
    // Every entry target is a dominator root: a traversal can reach it on an
    // entry edge without executing any other member. This is the usual
    // multi-root dominance — a virtual super-root over the entry targets —
    // with the super-root implicit since it is never itself a member.
    let mut dominators: BTreeMap<BlockId, BTreeSet<BlockId>> = members
        .iter()
        .map(|member| {
            (
                *member,
                if entry_targets.contains(member) {
                    BTreeSet::from([*member])
                } else {
                    members.clone()
                },
            )
        })
        .collect();
    loop {
        let mut changed = false;
        for member in members
            .iter()
            .copied()
            .filter(|member| !entry_targets.contains(member))
        {
            let mut incoming = predecessors[&member]
                .iter()
                .filter_map(|predecessor| dominators.get(predecessor));
            let mut next = incoming.next().cloned().unwrap_or_default();
            for set in incoming {
                next = next.intersection(set).copied().collect();
            }
            next.insert(member);
            if dominators[&member] != next {
                dominators.insert(member, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    members
        .iter()
        .copied()
        .filter(|member| {
            boundary
                .iter()
                .all(|source| dominators[source].contains(member))
        })
        .collect()
}

/// Parameters of `component`'s member blocks whose value is provably the same
/// on every iteration. A member parameter qualifies when every edge reaching
/// its block binds it to itself, to a member parameter that resolves to the
/// same representative, or to that representative — a value defined outside
/// the member roster. An operand use of such a parameter can be satisfied in
/// the preheader by substituting the representative.
///
/// The map is a fixed point over the edges reaching member blocks, so a
/// parameter carried across a member-to-member edge resolves to the
/// preheader-visible value its chain anchors on. A representative defined
/// inside the roster never qualifies: an edge that spells a member-internal
/// node result marks the parameter loop-carried. Parameters whose bindings
/// never anchor outside the roster — pure self-carried cycles — stay
/// unresolved and are absent from the result.
pub(crate) fn invariant_member_parameters(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
) -> BTreeMap<ValueId, ValueId> {
    /// Resolution states during the fixed point. `Unresolved` may promote once
    /// its deferred dependencies resolve; `Representative` can still degrade
    /// to `LoopCarried` when a deferred dependency resolves to a conflicting
    /// or carried value, and `LoopCarried` is final.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Resolution {
        Unresolved,
        Representative(ValueId),
        LoopCarried,
    }

    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let mut parameters = Vec::new();
    for member in &component.members {
        if let Some(block) = function.blocks.iter().find(|block| block.id == *member) {
            parameters.extend(
                block
                    .parameters
                    .iter()
                    .map(|parameter| (parameter.value, block.id)),
            );
        }
    }
    let mut resolutions: BTreeMap<ValueId, Resolution> = parameters
        .iter()
        .map(|(parameter, _)| (*parameter, Resolution::Unresolved))
        .collect();
    loop {
        let mut progressed = false;
        for (parameter, block) in &parameters {
            if resolutions[parameter] == Resolution::LoopCarried {
                continue;
            }
            let mut anchor = None;
            let mut loop_carried = false;
            for edge in function
                .blocks
                .iter()
                .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
                .filter(|edge| edge.target == *block)
            {
                let Some(binding) = edge
                    .bindings
                    .iter()
                    .find(|binding| binding.parameter == *parameter)
                else {
                    loop_carried = true;
                    break;
                };
                if binding.argument == *parameter {
                    // A binding that re-spells the parameter preserves whatever
                    // the other edges establish.
                    continue;
                }
                let contribution = match sites.get(&binding.argument) {
                    Some(ValueDefinitionSite::FunctionParameter(_)) => {
                        Resolution::Representative(binding.argument)
                    }
                    Some(ValueDefinitionSite::BlockParameter { block: site, .. })
                        if members.contains(site) =>
                    {
                        // A member parameter contributes its own resolution;
                        // an unresolved dependency defers to a later pass.
                        resolutions
                            .get(&binding.argument)
                            .copied()
                            .unwrap_or(Resolution::LoopCarried)
                    }
                    Some(ValueDefinitionSite::BlockParameter { block: site, .. })
                    | Some(ValueDefinitionSite::Node { block: site, .. })
                        if !members.contains(site) =>
                    {
                        Resolution::Representative(binding.argument)
                    }
                    _ => Resolution::LoopCarried,
                };
                match contribution {
                    Resolution::Unresolved => {}
                    Resolution::Representative(value) => match anchor {
                        None => anchor = Some(value),
                        Some(anchor) if anchor == value => {}
                        Some(_) => {
                            loop_carried = true;
                            break;
                        }
                    },
                    Resolution::LoopCarried => {
                        loop_carried = true;
                        break;
                    }
                }
            }
            let next = if loop_carried {
                Resolution::LoopCarried
            } else {
                match anchor {
                    Some(value) => Resolution::Representative(value),
                    None => Resolution::Unresolved,
                }
            };
            if resolutions[parameter] != next {
                resolutions.insert(*parameter, next);
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    resolutions
        .into_iter()
        .filter_map(|(parameter, resolution)| match resolution {
            Resolution::Representative(representative) => Some((parameter, representative)),
            Resolution::Unresolved | Resolution::LoopCarried => None,
        })
        .collect()
}

/// The operand substitution a relocated invariant scalar computation needs, or
/// `None` when the node is not an admitted computation or one of its uses is
/// genuinely loop-carried. A use whose definition already sits outside the
/// member roster needs no rewrite; a use of an invariant member parameter is
/// rebound to the representative every reaching edge agrees on; a use whose
/// member-internal definition is the result of another node in the same
/// relocation run — `relocating` — stays bound to that value, since the run
/// preserves the producer's result identity and places it earlier in the
/// preheader; and a member parameter every reaching edge binds to such a
/// run-covered result substitutes to that result directly. Any other
/// member-internal definition rejects the relocation.
pub(crate) fn invariant_scalar_operand_substitution(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    if !admissible_invariant_scalar_computation(node) {
        return None;
    }
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// The use-site invariance rule every scalar-operand relocation shares: a use
/// defined outside the member roster needs no rewrite; a use of an invariant
/// member parameter is rebound to the representative every reaching edge
/// agrees on; a use of a member-internal node result stays bound only when
/// that producer relocates in the same run (`relocating`); and a use of a
/// member parameter every reaching edge binds to one such run-covered result
/// is rebound to that result directly — the parameter spells the preserved
/// producer result on every traversal, so the moved node may name it.
/// Every other member-internal definition refuses. The operation-shape gate
/// stays with
/// the callers — [`invariant_scalar_operand_substitution`] admits the pure
/// computation whitelist and [`invariant_byte_read_admission`] admits the
/// byte-read shape — while this walk is deliberately operation-agnostic.
fn member_scalar_operand_substitution(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
) -> Option<BTreeMap<ValueId, ValueId>> {
    let members: BTreeSet<BlockId> = component.members.iter().copied().collect();
    let sites = value_definition_sites(function);
    let representatives = invariant_member_parameters(function, component);
    let mut substitution = BTreeMap::new();
    for value_use in &node.uses {
        let site = sites.get(&value_use.value)?;
        let inside = match site {
            ValueDefinitionSite::FunctionParameter(_) => false,
            ValueDefinitionSite::BlockParameter { block, .. }
            | ValueDefinitionSite::Node { block, .. } => members.contains(block),
        };
        if !inside {
            continue;
        }
        match site {
            ValueDefinitionSite::BlockParameter { block, .. } => {
                if let Some(representative) = representatives.get(&value_use.value) {
                    substitution.insert(value_use.value, *representative);
                } else {
                    let result = member_parameter_run_result(
                        function,
                        &members,
                        &sites,
                        value_use.value,
                        *block,
                        relocating,
                    )?;
                    substitution.insert(value_use.value, result);
                }
            }
            ValueDefinitionSite::Node { .. } if relocating.contains(&value_use.value) => {}
            _ => return None,
        }
    }
    Some(substitution)
}

/// The run-covered result a member block parameter may substitute to, or
/// `None` when the parameter does not spell one. [`invariant_member_parameters`]
/// deliberately marks a parameter bound to a member-internal node result
/// loop-carried — its static map cannot know which results the relocation run
/// preserves — but at the use site the run is known: when every edge reaching
/// the parameter's block binds it to the same member-internal result whose
/// producer relocates in the same run, the parameter is that result on every
/// traversal and the moved node may name it directly. A self-respelling
/// binding contributes nothing; any other binding — a member parameter, an
/// outside value, or a member result the run does not cover — refuses.
fn member_parameter_run_result(
    function: &PsiOptimizationFunction,
    members: &BTreeSet<BlockId>,
    sites: &BTreeMap<ValueId, ValueDefinitionSite>,
    parameter: ValueId,
    parameter_block: BlockId,
    relocating: &BTreeSet<ValueId>,
) -> Option<ValueId> {
    let mut result = None;
    let mut bound = false;
    for edge in function
        .blocks
        .iter()
        .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
        .filter(|edge| edge.target == parameter_block)
    {
        let binding = edge
            .bindings
            .iter()
            .find(|binding| binding.parameter == parameter)?;
        if binding.argument == parameter {
            continue;
        }
        let Some(ValueDefinitionSite::Node { block, .. }) = sites.get(&binding.argument) else {
            return None;
        };
        if !members.contains(block) || !relocating.contains(&binding.argument) {
            return None;
        }
        bound = true;
        match result {
            None => result = Some(binding.argument),
            Some(result) if result == binding.argument => {}
            Some(_) => return None,
        }
    }
    if bound { result } else { None }
}

/// Scalar-signature machine calls — `Call` — are the call family admitted for
/// loop-invariant motion: an exact internal callee invocation whose runtime
/// arguments are all scalars and whose single result is a scalar. The node
/// must keep its own operation identity as the first provenance row, define
/// exactly its spelled `result`/`scalar_type`, use exactly its `arguments` in
/// operand order, and carry no successors or ownership events. A call
/// carrying `crash_continuations` retains crash-route custody this family
/// does not yet re-express, so it stays inside; discharged
/// `requirement_obligations` instead move byte-exact inside the operation,
/// exactly like an obligated scalar computation's — they were proven against
/// the argument values, and operand substitution only rebinds a member
/// parameter to the representative every reaching edge proves equal. Callee
/// purity and member observability are decided separately by
/// [`invariant_scalar_call_admission`].
pub(crate) fn admissible_invariant_scalar_call(node: &OptimizationNode) -> Option<MachineId> {
    let O::Call {
        psi_operation,
        result,
        scalar_type,
        callee,
        arguments,
        crash_continuations,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.len() == 1
        && node.definitions[0].value == *result
        && node.definitions[0].scalar_type == *scalar_type
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && node.ownership.is_empty()
        && crash_continuations.is_empty())
    .then_some(*callee)
}

/// Unit-result machine calls — `CallUnit` — are the structural-signature
/// member of the call family admitted for loop-invariant motion: an exact
/// internal callee invocation whose scalar arguments ride the shared
/// substitution and whose structural arguments are borrows. The
/// node must keep its own operation identity as the first provenance row,
/// define no scalar (`CallUnit` produces no result — the relocated node
/// preserves the invocation itself), use exactly its scalar `arguments` in
/// operand order, carry no successors, and keep no crash-route custody. A
/// structural argument with `MutableBorrow` or `WriteOnlyBorrow` access hands
/// the callee write authority over a caller place — admitted only through
/// [`invariant_unit_call_admission`]'s exclusive member-produced-root
/// evidence — while `Owned` access admits only when the argument names a
/// whole root whose declared multiplicity is `Unrestricted`: the verifier
/// binds argument and callee parameter multiplicities equal, so the copy
/// shapes the cyclic owned-argument fence recognizes — an unrestricted
/// owned parameter or an unrestricted claim-free scalar-array result —
/// copy the payload into the callee's activation rather than moving
/// custody. An `Owned` argument over an affine or linear root genuinely
/// transfers the caller's place and stays refused.
/// `claim_transfers` must be empty: the node then carries exactly one
/// vacuous `ClaimTransfer` ownership row — the custody mirror of the empty
/// roster — which relocates byte-exact inside the moved operation.
/// `requirement_obligations` move byte-exact exactly like a scalar call's:
/// they were discharged against the argument values and operand substitution
/// only rebinds a member parameter to the representative every reaching edge
/// proves equal. Argument-root invariance and the whole-component
/// place-custody bound are decided separately by
/// [`invariant_unit_call_admission`].
pub(crate) fn admissible_invariant_unit_call(node: &OptimizationNode) -> Option<MachineId> {
    let O::CallUnit {
        psi_operation,
        callee,
        arguments,
        claim_transfers,
        crash_continuations,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && claim_transfers.is_empty()
        && node.ownership.as_slice() == [OwnershipEvent::ClaimTransfer(Vec::new())]
        && crash_continuations.is_empty())
    .then_some(*callee)
}

/// Scalar-result structural-signature machine calls — `CallStructuralScalar`
/// — are the call family's third admitted member: an exact internal callee
/// invocation carrying both the scalar `arguments` a `Call` spells and the
/// shared-borrow `structural_arguments` a `CallUnit` spells, and defining
/// exactly one scalar result. The node must keep its own operation identity
/// as the first provenance row, define exactly its spelled `result`, use
/// exactly its scalar `arguments` in operand order, carry no successors, and
/// keep no crash-route custody. The structural side obeys the unit call's
/// whitelist verbatim: borrow arguments admit — a `MutableBorrow` or
/// `WriteOnlyBorrow` argument still needs
/// [`invariant_structural_scalar_call_admission`]'s exclusive
/// member-produced-root evidence, and an `Owned` argument admits only over
/// a whole root whose declared multiplicity is `Unrestricted`, the copy
/// shapes the cyclic owned-argument fence recognizes — while `claim_transfers` must
/// be empty, so the node
/// carries exactly one vacuous `ClaimTransfer` ownership row that relocates
/// byte-exact inside the moved operation. `requirement_obligations` move
/// byte-exact exactly like the other call variants': they were discharged
/// against the argument values and operand substitution only rebinds a
/// member parameter to the representative every reaching edge proves equal.
/// Callee purity, member observability, argument-root invariance, and the
/// whole-component place-custody bound are decided separately by
/// [`invariant_structural_scalar_call_admission`].
pub(crate) fn admissible_invariant_structural_scalar_call(
    node: &OptimizationNode,
) -> Option<MachineId> {
    let O::CallStructuralScalar {
        psi_operation,
        result,
        callee,
        arguments,
        claim_transfers,
        crash_continuations,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.len() == 1
        && node.definitions[0].value == result.value
        && node.definitions[0].scalar_type == result.scalar_type
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && claim_transfers.is_empty()
        && node.ownership.as_slice() == [OwnershipEvent::ClaimTransfer(Vec::new())]
        && crash_continuations.is_empty())
    .then_some(*callee)
}

/// Structural-result machine calls — `CallStructural` — are the call family's
/// fourth admitted member: an exact internal callee invocation returning a
/// fresh structural place. The admitted shape is the one the cyclic
/// eligibility fence already confines: an affine, claim-free result the
/// producing member block dispatches through a `StructuralCase` or returns
/// outright, so the verifier's per-traversal custody — produce inside the
/// member, discard on every dispatch edge — is exactly what the relocation
/// re-expresses. The node must keep its own operation identity as the first
/// provenance row, define no scalar, use exactly its scalar `arguments` in
/// operand order, carry no successors, and keep no crash-route custody.
/// `claim_transfers` and `returned_claim_transfers` must both be empty — the
/// node then carries exactly one vacuous `ClaimTransfer` ownership row, which
/// relocates byte-exact inside the moved operation. `structural_arguments`
/// obeys the argument whitelist the unit and scalar-result calls share: a
/// borrow argument lets the callee observe — and for a mutating borrow,
/// write — a caller place, so
/// [`invariant_structural_call_admission`] replays the whole-component
/// place-custody bound and each argument root's landing rule, while an
/// `Owned` argument admits only over a whole root whose declared
/// multiplicity is `Unrestricted` — an unrestricted owned parameter or an
/// unrestricted claim-free scalar-array result copies its payload into the
/// callee — and an `Owned` argument over an affine or linear root moves the
/// caller's place outright, custody movement this boundary cannot
/// re-express, so it stays refused.
/// `requirement_obligations`, `crash_continuations`, and
/// `selected_evidence` must be empty: the admitted contract carries
/// none of them, and a call that does stays inside rather than re-expressing
/// evidence this family has not reconstructed. Callee purity, member
/// observability, the place-custody bound, argument-root invariance, and the
/// result's member-roster containment are decided
/// separately by [`invariant_structural_call_admission`].
pub(crate) fn admissible_invariant_structural_call(
    node: &OptimizationNode,
) -> Option<(MachineId, terminal_psi::StructuralOperationResult)> {
    let O::CallStructural {
        psi_operation,
        result,
        callee,
        arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
        selected_evidence,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.len() == arguments.len()
        && node
            .uses
            .iter()
            .zip(arguments.iter())
            .all(|(value_use, argument)| value_use.value == *argument)
        && node.successors.is_empty()
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && claim_transfers.is_empty()
        && returned_claim_transfers.is_empty()
        && requirement_obligations.is_empty()
        && crash_continuations.is_empty()
        && selected_evidence.is_empty()
        && node.ownership.as_slice() == [OwnershipEvent::ClaimTransfer(Vec::new())])
    .then(|| (*callee, result.clone()))
}

/// The complete unit-call admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned call shape
/// ([`admissible_invariant_unit_call`]) — which yields the exact internal
/// callee — the callee's transitive effect summary must prove no observable
/// effect, no crash, and no suspension, every node inside the component's
/// member roster must be unobservable under the same summaries, and the
/// component must preserve member-visible place contents and custody
/// ([`component_preserves_place_observations`]) — the whole-component bound
/// the byte family already replays, required here because the callee can
/// observe caller places through its borrows: only when no member mutates a
/// place any member could observe does the relocated call observe, on every
/// traversal, exactly what the in-loop invocation observed.
///
/// The callee's `structural_state` axis stays exempt under the same argument
/// the scalar call uses, tightened by the argument whitelist: every place the
/// callee could mutate is either activation-internal or reached through a
/// borrow the admission already gated — a shared borrow hands it no write
/// authority, and a mutable or write-only borrow may only name a
/// member-produced root the same run relocates and no other member observes —
/// so no member-visible place moves, and a `May` there reflects
/// callee-internal structural work or control edges carrying structural
/// bindings, not caller custody.
///
/// Each scalar `arguments` operand then obeys the shared use-site invariance
/// rule ([`member_scalar_operand_substitution`]). Each structural
/// argument's root must land somewhere the relocated run can see
/// it: already visible at the unique preheader insertion point, the
/// representative an invariant member structural parameter resolves to
/// ([`invariant_member_place_parameters`]), or a root a node earlier in the
/// same run produced — `relocating_roots` — because the run preserves the
/// producer's declared place identity and orders it ahead of the call. A
/// member parameter the fixed point resolves rebinds on the moved node; a
/// member-produced root the run does not cover — or any other invisible root
/// — refuses the relocation. A mutable or write-only borrow's root instead
/// must be member-produced and already relocated by the same run: the callee
/// may write — and for a mutable borrow read — the cell, so only a fresh
/// per-traversal initializer keeps the invocation's view identical. An
/// `Owned` whole-root argument takes the same landing and additionally
/// requires the landed root to be copyable
/// ([`copyable_owned_argument_root`]) — an unrestricted owned parameter or
/// an unrestricted claim-free scalar-array result — so the relocated
/// invocation copies exactly the payload every in-loop invocation copied;
/// an `Owned` argument over affine or linear custody would move the
/// caller's place outright and stays refused.
///
/// Returns the scalar substitution plus the `(member parameter or
/// member-produced root, preheader-visible root)` rewrites the relocated
/// call performs on its structural arguments — empty when every argument
/// already names a visible or run-produced root.
pub(crate) fn invariant_unit_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    let callee = admissible_invariant_unit_call(node)?;
    let O::CallUnit {
        structural_arguments,
        ..
    } = &node.operation
    else {
        return None;
    };
    borrow_call_admission(
        function,
        component,
        node,
        callee,
        structural_arguments,
        relocating,
        relocating_roots,
        &BTreeSet::new(),
        effects,
    )
}

/// The complete scalar-result structural-call admission shared by the
/// proposal and the relocation freeze replay: `node` must carry the
/// source-owned call shape
/// ([`admissible_invariant_structural_scalar_call`]) — which yields the
/// exact internal callee — and then passes the unit call's whole evidence
/// surface unchanged: the pure transitive callee summary, the unobservable
/// member roster, the whole-component place-custody bound, the shared
/// scalar-operand substitution, and each structural argument's root —
/// borrow or copyable owned — landing somewhere the relocated run can see
/// it. The
/// place-custody bound carries one additional weight here: the relocated
/// call's scalar result is whatever the callee computed from those
/// arguments and borrows, so only when no member mutates or moves a place
/// does the preheader invocation return what every in-loop traversal's
/// invocation returned.
pub(crate) fn invariant_structural_scalar_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    let callee = admissible_invariant_structural_scalar_call(node)?;
    let O::CallStructuralScalar {
        structural_arguments,
        ..
    } = &node.operation
    else {
        return None;
    };
    borrow_call_admission(
        function,
        component,
        node,
        callee,
        structural_arguments,
        relocating,
        relocating_roots,
        &BTreeSet::new(),
        effects,
    )
}

/// The complete structural-result call admission shared by the proposal and
/// the relocation freeze replay: `node` must carry the source-owned call
/// shape ([`admissible_invariant_structural_call`]) — which yields the exact
/// internal callee and its affine claim-free result — then the affine result
/// place must stay inside the member roster spelled only through positions
/// the relocation's custody rewrite re-expresses
/// ([`scalar_case_result_contained`]): the producing call itself, the
/// member-block dispatch or structural return consuming it, and the edges
/// whose discard rosters the rewrite adjusts. Every remaining evidence half
/// is the borrow calls' shared surface
/// ([`borrow_call_admission`]): the pure transitive callee, the unobservable
/// member roster, the shared scalar-operand substitution, the borrow and
/// copyable-owned argument whitelist, and each argument root's landing —
/// already preheader-visible, resolved through an invariant member
/// structural parameter, or produced by a node this component's run already
/// relocated — with a mutable or write-only borrow additionally requiring
/// that root's unique member producer among the relocated set and an
/// `Owned` argument requiring the landed root to declare an unrestricted
/// copyable shape.
///
/// The place-custody bound runs with the run's relocating roots plus this
/// call's own result tolerated: a borrow argument lets the callee observe a
/// caller place the containment bound alone does not freeze, so the bound
/// must prove no member-visible place mutates or moves across traversals —
/// while the tolerated discards are exactly the confined results'
/// per-traversal disposal the relocation re-expresses rather than preserves.
/// Returns the scalar substitution plus the structural-argument root
/// rewrites the relocated call performs.
pub(crate) fn invariant_structural_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    let (callee, result) = admissible_invariant_structural_call(node)?;
    if !scalar_case_result_contained(function, component, result.place) {
        return None;
    }
    let O::CallStructural {
        structural_arguments,
        ..
    } = &node.operation
    else {
        return None;
    };
    let mut tolerated = relocating_roots.clone();
    tolerated.insert(result.place);
    borrow_call_admission(
        function,
        component,
        node,
        callee,
        structural_arguments,
        relocating,
        relocating_roots,
        &tolerated,
        effects,
    )
}

/// The shared evidence every admitted structural-signature call replays once
/// its shape gate has yielded the exact internal callee and its
/// `structural_arguments` roster: the callee's transitive effect
/// summary must prove no observable effect, no crash, and no suspension
/// (the `structural_state` axis stays exempt — every caller place the callee
/// can write arrives through a borrow the place-custody bound already
/// accounted), every node inside the component's member roster
/// must be unobservable under the same summaries, and the component must
/// preserve member-visible place contents and custody
/// ([`component_preserves_place_observations`], run with `tolerated` member
/// discards — empty for the unit and scalar-result callers, the run's
/// relocating roots plus the call's own confined result for a
/// structural-result call, whose dispatch edges discard the fresh place the
/// relocation makes persistent) — only then does the
/// relocated call observe and return on every traversal exactly what the
/// in-loop invocation did. That bound is also what confines a mutating
/// borrow's authority: the call is tolerated inside the roster only when
/// every root its `MutableBorrow` or `WriteOnlyBorrow` arguments name is
/// exclusive to it, so no member read, second borrow, or store could observe
/// the callee's writes — and a `MutableBorrow` argument's root must be a
/// member-produced place a node earlier in the same run already relocated,
/// which is the only way the callee still reads the fresh initializer every
/// source traversal established. Each scalar `arguments` operand obeys the
/// shared use-site invariance rule
/// ([`member_scalar_operand_substitution`]), and
/// each structural argument's root must land where the relocated run can
/// see it — already visible at the unique preheader insertion point, the
/// representative an invariant member structural parameter resolves to
/// ([`invariant_member_place_parameters`]), or a root a node earlier in the
/// same run produced — `relocating_roots`. Access refines the landing: a
/// shared borrow names any landed root, a mutable or write-only borrow only
/// a uniquely member-produced one the run already relocated, and an `Owned`
/// whole-root argument additionally requires the landed root to declare one
/// of the copyable shapes [`copyable_owned_argument_root`] recognizes — an
/// unrestricted owned parameter or an unrestricted claim-free scalar-array
/// result — because `Owned` over an unrestricted payload copies it into the
/// callee while `Owned` over affine or linear custody would move the
/// caller's place outright.
fn borrow_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    callee: MachineId,
    structural_arguments: &[terminal_psi::StructuralArgument],
    relocating: &BTreeSet<ValueId>,
    relocating_roots: &BTreeSet<PlaceId>,
    tolerated: &BTreeSet<PlaceId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<(BTreeMap<ValueId, ValueId>, Vec<(PlaceId, PlaceId)>)> {
    if !scalar_call_callee_pure(effects, callee) {
        return None;
    }
    if !component_members_unobservable(function, component, effects) {
        return None;
    }
    if !component_preserves_place_observations_tolerating(function, component, tolerated) {
        return None;
    }
    let substitution = member_scalar_operand_substitution(function, component, node, relocating)?;
    let preheader_source = shared_entry_source(component)?;
    let preheader = function
        .blocks
        .iter()
        .find(|block| block.id == preheader_source)?;
    let representatives = invariant_member_place_parameters(function, component, relocating_roots);
    let mut rewrites = Vec::new();
    for argument in structural_arguments {
        match argument.access {
            // A mutating borrow hands the callee write — and for a mutable
            // borrow, read — authority over the argument's root. The only
            // admitted root is a member-produced place whose unique member
            // producer relocates in the same run ahead of the call: the
            // producer's declared place identity moves byte-exact, so the
            // argument keeps spelling it and no rewrite is needed. A second
            // member producer would let a staying establishment
            // re-initialize the cell behind the relocated call's single
            // invocation, so the producer count must be exactly one. A
            // caller root under
            // `MutableBorrow` is genuinely inadmissible — the callee may
            // read it, and its contents evolve across traversals as the
            // callee's own writes accumulate, so the relocated invocation
            // could not reproduce the per-traversal reads; under
            // `WriteOnlyBorrow` a caller root would be consistent in
            // principle — the callee cannot read it — but no admitted cyclic
            // source shape produces one today, so both take the single
            // member-produced rule. The spelled place must be the covered
            // root itself: a member structural parameter that resolves to
            // the root is not rebound here, because the callee writes
            // through the argument place it names and this admission never
            // substitutes a mutating argument's spelling.
            terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow => {
                if !(relocating_roots.contains(&argument.place)
                    && member_root_producer_count(function, component, argument.place) == 1)
                {
                    return None;
                }
            }
            terminal_psi::StructuralAccess::SharedBorrow => {
                let resolved = call_argument_root_landing(
                    function,
                    component,
                    preheader,
                    &representatives,
                    relocating_roots,
                    argument.place,
                )?;
                if resolved != argument.place {
                    rewrites.push((argument.place, resolved));
                }
            }
            // An `Owned` whole-root argument over a copyable root is the
            // observation a shared borrow is, spelled with value semantics:
            // the callee receives a copy of the payload, so the relocated
            // invocation copies — on the one traversal that runs — exactly
            // what every in-loop invocation copied. The root obeys the same
            // landing rule a borrow's does, then must declare one of the
            // copy shapes the cyclic owned-argument fence recognizes
            // ([`copyable_owned_argument_root`]): an unrestricted owned
            // parameter, or an unrestricted claim-free scalar-array result
            // — for a member-produced root, the landing rule already
            // required its unique producer to relocate in the same run
            // ahead of the call. An `Owned` argument over an affine or
            // linear root moves the caller's place into the callee — the
            // per-traversal transfer this boundary cannot re-express — and
            // a projected `Owned` argument or any other produced kind keeps
            // the refusal.
            terminal_psi::StructuralAccess::Owned => {
                if !argument.path.is_empty() {
                    return None;
                }
                let resolved = call_argument_root_landing(
                    function,
                    component,
                    preheader,
                    &representatives,
                    relocating_roots,
                    argument.place,
                )?;
                if !copyable_owned_argument_root(function, resolved) {
                    return None;
                }
                if resolved != argument.place {
                    rewrites.push((argument.place, resolved));
                }
            }
        }
    }
    Some((substitution, rewrites))
}

/// The root an admitted call argument may name once the call relocates:
/// `place` itself when it is already visible at the unique preheader
/// insertion point ([`place_observation_root_visible`]) or produced by a
/// node the same relocation run covers, else the representative an
/// invariant member structural parameter resolves to
/// ([`invariant_member_place_parameters`]) when that lands the same way —
/// the place-level analog of [`member_scalar_operand_substitution`]. A
/// member-produced root qualifies only when its member producer is unique:
/// a second producer would re-establish the cell each traversal behind the
/// relocated call's single invocation.
fn call_argument_root_landing(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    preheader: &OptimizationBlock,
    representatives: &BTreeMap<PlaceId, PlaceId>,
    relocating_roots: &BTreeSet<PlaceId>,
    place: PlaceId,
) -> Option<PlaceId> {
    if place_observation_root_visible(function, preheader, place)
        || (relocating_roots.contains(&place)
            && member_root_producer_count(function, component, place) == 1)
    {
        return Some(place);
    }
    let representative = *representatives.get(&place)?;
    (place_observation_root_visible(function, preheader, representative)
        || (relocating_roots.contains(&representative)
            && member_root_producer_count(function, component, representative) == 1))
        .then_some(representative)
}

/// The complete scalar-call admission shared by the proposal and the
/// relocation freeze replay: `node` must carry the source-owned call shape
/// ([`admissible_invariant_scalar_call`]) — which yields the exact internal
/// callee — the callee's transitive effect summary must prove no observable
/// effect, no crash, and no suspension, and every node inside the
/// component's member roster must be unobservable under the same summaries.
/// The member scan is the divergence half of call custody: a pure callee can
/// still fail to return, and relocating the call moves its possible
/// non-return ahead of every member node — member work the source traversal
/// performed before the call is skipped when the moved call never comes
/// back. That reorder is invisible only when no member performs observable
/// work, so the gate refuses the call when any member is observable; a
/// member call qualifies only under the same pure-callee rule the relocated
/// call obeys. Each scalar argument then obeys the shared use-site
/// invariance rule ([`member_scalar_operand_substitution`]): defined outside
/// the component, an invariant member parameter rebound to its agreed
/// representative, or the preserved result of a node earlier in the same
/// relocation run.
///
/// The callee's `structural_state` axis is deliberately exempt: a `Call`
/// passes only scalar arguments and returns only a scalar, so every place
/// the callee could touch is callee-internal — no caller-visible place
/// moves. The summary's `May` there only reflects control edges carrying
/// structural bindings, not reachable caller custody.
pub(crate) fn invariant_scalar_call_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
    relocating: &BTreeSet<ValueId>,
    effects: &crate::EffectSummaryAnalysis,
) -> Option<BTreeMap<ValueId, ValueId>> {
    let callee = admissible_invariant_scalar_call(node)?;
    if !scalar_call_callee_pure(effects, callee) {
        return None;
    }
    if !component_members_unobservable(function, component, effects) {
        return None;
    }
    member_scalar_operand_substitution(function, component, node, relocating)
}

/// The effect-summary product computed over `unit`: the whole-unit
/// transitive per-function effect table call admission consults. The product
/// is keyed to `unit.identity` at computation, so both the proposal and the
/// freeze replay derive callee purity and member observability from the seed
/// rather than trusting any plan. An absent product yields an empty table —
/// every callee lookup then fails closed.
pub(crate) fn unit_effect_summaries(unit: &PsiOptimizationUnit) -> crate::EffectSummaryAnalysis {
    match crate::compute_analysis(unit, optimization_core::AnalysisKind::EffectSummaries) {
        Some(crate::AnalysisProduct::EffectSummaries(analysis)) => analysis,
        _ => crate::EffectSummaryAnalysis {
            nodes: Vec::new(),
            functions: Vec::new(),
        },
    }
}

/// Whether the transitive effect summary proves `callee` performs no
/// observable effect, no crash, and no suspension — the call-family purity a
/// relocated scalar call and a member call partner both need.
/// `structural_state` is deliberately not consulted: a scalar `Call` passes
/// no places, so any structural work its callee performs is callee-internal.
fn scalar_call_callee_pure(effects: &crate::EffectSummaryAnalysis, callee: MachineId) -> bool {
    effects
        .functions
        .iter()
        .find(|summary| summary.machine == callee)
        .is_some_and(|summary| {
            summary.observable == crate::EffectKnowledge::No
                && summary.crash == crate::EffectKnowledge::No
                && summary.suspension == crate::EffectKnowledge::No
        })
}

/// The statically realized callee a member call node invokes, when its call
/// family carries one: the direct `callee` of the internal-call variants, or
/// the sole permitted realization row of a rebound or stored dynamic
/// dispatch. Descriptor-parameter and dynamic-argument calls have no single
/// static callee here and stay unresolvable.
fn member_call_target(operation: &O) -> Option<MachineId> {
    match operation {
        O::Call { callee, .. }
        | O::CallUnit { callee, .. }
        | O::CallStructuralScalar { callee, .. }
        | O::CallStructural { callee, .. } => Some(*callee),
        O::CallDynamicScalar {
            dynamic_dispatch, ..
        }
        | O::CallDynamicUnit {
            dynamic_dispatch, ..
        } => Some(dynamic_dispatch.dispatch.realization),
        O::CallStoredDynamicScalar {
            dynamic_dispatch, ..
        } => Some(dynamic_dispatch.dispatch.realization),
        _ => None,
    }
}

/// Whether every node inside `component`'s member roster is unobservable —
/// the member half of call custody. Non-call members consult their own node
/// summary's observable axis; member calls must invoke a statically resolved
/// callee whose transitive summary is pure under the same rule the relocated
/// call obeys, so a descriptor-parameter or dynamic-argument member call —
/// no single static callee — refuses. A member node absent from the summary
/// refuses too: the gate fails closed rather than trusting a drifted table.
fn component_members_unobservable(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    effects: &crate::EffectSummaryAnalysis,
) -> bool {
    for member in &component.members {
        let Some(block) = function.blocks.iter().find(|block| block.id == *member) else {
            return false;
        };
        for (index, node) in block.nodes.iter().enumerate() {
            let index = match u32::try_from(index) {
                Ok(index) => index,
                Err(_) => return false,
            };
            match member_call_target(&node.operation) {
                Some(callee) => {
                    if !scalar_call_callee_pure(effects, callee) {
                        return false;
                    }
                }
                None => {
                    if matches!(
                        node.operation,
                        O::CallUnitWithDynamicArguments { .. }
                            | O::CallStructuralScalarWithDynamicArguments { .. }
                            | O::CallDynamicParameterScalar { .. }
                            | O::CallDynamicParameterUnit { .. }
                    ) {
                        return false;
                    }
                    let observable = effects
                        .nodes
                        .iter()
                        .find(|summary| {
                            summary.machine == function.machine
                                && summary.block == *member
                                && summary.node == index
                        })
                        .map(|summary| summary.observable);
                    if observable != Some(crate::EffectKnowledge::No) {
                        return false;
                    }
                }
            }
        }
    }
    true
}

/// Rewrite the scalar operand fields of an admitted invariant computation.
/// Only the variants [`admissible_invariant_scalar_computation`] admits carry
/// plain `ValueId` operand positions; any other operation is left untouched.
/// An obligated variant's `obligation` is not an operand position — it stays
/// byte-exact inside the moved operation.
pub(crate) fn substitute_invariant_scalar_operands(
    operation: &mut O,
    substitution: &BTreeMap<ValueId, ValueId>,
) {
    fn substitute(field: &mut ValueId, substitution: &BTreeMap<ValueId, ValueId>) {
        if let Some(representative) = substitution.get(field) {
            *field = *representative;
        }
    }
    match operation {
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | O::IntegerWiden { operand, .. }
        | O::IntegerExactCast { operand, .. } => substitute(operand, substitution),
        O::BooleanEqual { left, right, .. }
        | O::IntegerEqual { left, right, .. }
        | O::IntegerLessThan { left, right, .. }
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | O::IntegerBitwiseOr { left, right, .. }
        | O::IntegerBitwiseXor { left, right, .. }
        | O::WrappingIntegerAdd { left, right, .. }
        | O::ExactIntegerAdd { left, right, .. }
        | O::SaturatingIntegerAdd { left, right, .. }
        | O::WrappingIntegerSubtract { left, right, .. }
        | O::ExactIntegerSubtract { left, right, .. }
        | O::SaturatingIntegerSubtract { left, right, .. }
        | O::WrappingIntegerMultiply { left, right, .. }
        | O::ExactIntegerMultiply { left, right, .. }
        | O::SaturatingIntegerMultiply { left, right, .. }
        | O::WrappingIntegerDivide { left, right, .. }
        | O::ExactIntegerDivide { left, right, .. }
        | O::SaturatingIntegerDivide { left, right, .. }
        | O::WrappingIntegerRemainder { left, right, .. }
        | O::ExactIntegerRemainder { left, right, .. }
        | O::SaturatingIntegerRemainder { left, right, .. }
        | O::IeeeFloatCompare { left, right, .. } => {
            substitute(left, substitution);
            substitute(right, substitution);
        }
        O::WrappingIntegerShiftLeft { value, count, .. }
        | O::WrappingIntegerShiftRight { value, count, .. }
        | O::ExactIntegerShiftLeft { value, count, .. }
        | O::ExactIntegerShiftRight { value, count, .. } => {
            substitute(value, substitution);
            substitute(count, substitution);
        }
        // A relocated byte read rebinds its `index` and `length` through the
        // same invariant-parameter substitution a pure computation uses. Its
        // `source` root and `obligation` are not scalar operand positions —
        // the root moves through `substitute_invariant_place_root` and the
        // obligation stays byte-exact.
        O::ByteSequenceRead { index, length, .. } => {
            substitute(index, substitution);
            substitute(length, substitution);
        }
        // A relocated subslice rebinds `start`, `end`, and `length` the same
        // way; its `source` root moves through `substitute_invariant_place_root`
        // while its structural result place and obligation stay byte-exact.
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => {
            substitute(start, substitution);
            substitute(end, substitution);
            substitute(length, substitution);
        }
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => {
            substitute(left, substitution);
            substitute(right, substitution);
            substitute(addend, substitution);
        }
        // A relocated call rebinds each scalar `arguments` operand through
        // the same invariant-parameter substitution a pure computation uses
        // — every admitted call variant spells its scalar operands in that
        // one field. `callee`, results, `structural_arguments`,
        // `claim_transfers`, `requirement_obligations`, and
        // `crash_continuations` are not scalar operand positions — they stay
        // byte-exact inside the moved operation, while the structural
        // argument roots rebind through `substitute_invariant_call_roots`.
        O::Call { arguments, .. }
        | O::CallUnit { arguments, .. }
        | O::CallStructuralScalar { arguments, .. }
        | O::CallStructural { arguments, .. } => {
            for argument in arguments {
                substitute(argument, substitution);
            }
        }
        // A relocated primitive-local establishment rebinds its initializing
        // `value` through the same invariant-parameter substitution a pure
        // computation uses — the declared place, structural type, and result
        // custody stay byte-exact inside the moved operation.
        O::EstablishPrimitiveLocal { value, .. } => {
            substitute(&mut value.value, substitution);
        }
        // A relocated scalar-array establishment rebinds each element operand
        // through the same invariant-parameter substitution — the declared
        // place, structural type, and result custody stay byte-exact inside
        // the moved operation.
        O::EstablishScalarArray { elements, .. } => {
            for element in elements {
                substitute(element, substitution);
            }
        }
        // A relocated scalar-case establishment rebinds each case-field value
        // through the same invariant-parameter substitution — the declared
        // place, structural type, result case, result custody, field
        // identities, and bounded-integer range obligations stay byte-exact
        // inside the moved operation.
        O::EstablishScalarCase { fields, .. } => {
            for field in fields {
                substitute(&mut field.value, substitution);
            }
        }
        // A relocated record establishment rebinds each scalar field
        // initializer through the same invariant-parameter substitution —
        // the declared place, structural type, result custody, declaration
        // order, structural field arguments, and bounded-integer range
        // obligations stay byte-exact inside the moved operation.
        O::EstablishRecord { fields, .. } => {
            for field in fields {
                if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                    substitute(value, substitution);
                }
            }
        }
        _ => {}
    }
}

/// Rewrite the observed storage root of an admitted place observation, byte
/// read, or subslice from an invariant member parameter to its agreed
/// representative. Only the variants [`admissible_invariant_place_read`],
/// [`admissible_invariant_byte_read`], and [`admissible_invariant_subslice`]
/// admit carry a `source` root position; the rewrite fires only when the
/// operation's current root is `parameter`, so a drifted plan cannot rebind a
/// different place. Returns whether the root was rebound.
pub(crate) fn substitute_invariant_place_root(
    operation: &mut O,
    parameter: PlaceId,
    representative: PlaceId,
) -> bool {
    let source = match operation {
        O::PrimitiveScalarRead { source, .. }
        | O::StructuralCaseMembership { source, .. }
        | O::ByteSequenceRead { source, .. }
        | O::ByteSequenceSubslice { source, .. }
        | O::ByteSequenceLength { source, .. }
        | O::StructuralByteSequenceFieldLength { source, .. }
        | O::BooleanStructuralField { source, .. }
        | O::IntegerStructuralField { source, .. } => source,
        _ => return false,
    };
    if *source != parameter {
        return false;
    }
    *source = representative;
    true
}

/// Rebind the structural-argument roots of an admitted call operation or the
/// structural-field argument roots of an admitted record establishment:
/// `rewrites` maps each member-parameter root the
/// admission resolved to the preheader-visible or run-covered root the
/// relocated call now
/// names. Every static-call variant carries the same `structural_arguments`
/// field shape, so the substitution walks whichever one the operation is —
/// `CallUnit` and `CallStructuralScalar` are the admitted
/// structural-signature families today (`CallStructural` stays matched for
/// shape completeness), and a relocation never reaches here for an
/// operation the admission refused. A shared-borrow argument whose root a
/// node earlier in the same run produced — an `EstablishPrimitiveLocal` or a
/// byte-literal declaration — needs no rewrite at all: the run keeps the
/// producer's declared place identity byte-exact. A relocated
/// `EstablishRecord` instead rebinds the `place` of each structural field
/// initializer whose copied root is an invariant member structural
/// parameter — a field naming a run-produced root keeps spelling it
/// byte-exact.
/// Returns `true` only when every requested rewrite found at least one
/// argument to rebind: a planned rewrite that fires on no argument means the
/// plan drifted from the operation, which the relocation callers treat as a
/// candidate mismatch. Multiple arguments may share a root — one rewrite
/// rebinds them all.
pub(crate) fn substitute_invariant_call_roots(
    operation: &mut O,
    rewrites: &BTreeMap<PlaceId, PlaceId>,
) -> bool {
    let mut applied = BTreeSet::new();
    match operation {
        O::CallUnit {
            structural_arguments,
            ..
        }
        | O::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | O::CallStructural {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments.iter_mut() {
                if let Some(root) = rewrites.get(&argument.place) {
                    applied.insert(argument.place);
                    argument.place = *root;
                }
            }
        }
        O::EstablishRecord { fields, .. } => {
            for field in fields.iter_mut() {
                if let terminal_psi::RecordFieldValue::Structural(argument) = &mut field.value
                    && let Some(root) = rewrites.get(&argument.place)
                {
                    applied.insert(argument.place);
                    argument.place = *root;
                }
            }
        }
        _ => {}
    }
    rewrites.keys().all(|from| applied.contains(from))
}
pub use prephysical_manifest::{
    PrePhysicalOptimizationManifestError, ValidatedPrePhysicalOptimizationManifest,
    project_pre_physical_optimization_manifest, validate_pre_physical_optimization_manifest,
};
pub use projection::{
    OptimizedAbstractPlanProjectionError, ValidatedOptimizedAbstractPlanProjection,
    validate_optimized_abstract_plan_projection,
};
