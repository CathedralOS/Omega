//! Which member operations may leave a cyclic component: the loop-invariance
//! admission of scalar leaves and total scalar computations, place, byte and
//! subslice reads, byte literals, primitive locals, records, scalar arrays,
//! scalar cases and trivial affine locals. Each `admissible_*` gate decides
//! the operation shape alone; each `*_admission` decides it against the
//! component -- its member parameters, place observations and relocation run.
//! Calls have their own owner in `invariant_calls`.

use super::member_blocks::{member_scalar_operand_substitution, shared_entry_source};
use super::place_observations::{
    component_preserves_place_observations, invariant_member_place_parameters,
    member_root_producer_count, place_observation_root_visible,
};
use super::relocation_rewrites::scalar_case_result_contained;
use abstract_operations::AbstractOperation as O;
use optimization_unit::*;
use semantic_vocabulary::*;
use std::collections::{BTreeMap, BTreeSet};

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

/// An `EstablishTrivialAffineLocal` is the scalar-case family's operand-free
/// sibling — the direct spelling of the affine empty-record establishment.
/// The node declares one machine-owned, whole, claim-free affine place of
/// empty-record type: it defines no scalar value (its result is the declared
/// place), reads no operand, carries no successors or ownership events, and
/// keeps the declaration byte-exact inside the moved operation. The node must
/// name its own operation as the first provenance row. The only custody the
/// relocation re-expresses is the affine place's own lifecycle, which
/// [`invariant_trivial_affine_local_admission`] bounds separately.
pub(crate) fn admissible_invariant_trivial_affine_local(
    node: &OptimizationNode,
) -> Option<terminal_psi::StructuralPlaceDeclaration> {
    let O::EstablishTrivialAffineLocal {
        psi_operation,
        place,
        ..
    } = &node.operation
    else {
        return None;
    };
    (node.provenance.first() == Some(&PsiProvenance::Operation(*psi_operation))
        && node.definitions.is_empty()
        && node.uses.is_empty()
        && node.successors.is_empty()
        && node.ownership.is_empty())
    .then_some(*place)
}

/// The complete trivial-affine-local-establishment admission shared by the
/// proposal and the relocation freeze replay: `node` must carry the
/// source-owned establishment shape
/// ([`admissible_invariant_trivial_affine_local`]) — which yields the
/// declared place — and that place must stay inside `component` spelled
/// only through positions the relocation's custody rewrite covers
/// ([`scalar_case_result_contained`]): the cyclic eligibility fence already
/// confined the fresh affine place to the member block that establishes it,
/// with every departing edge disposing it, so hoisting the establishment
/// keeps the one persistent place live through the whole component —
/// stripping it from member-internal edges and disposing it on every exit
/// edge and member return that did not already carry it. The operation is
/// operand-free, so admission yields the place and no substitution.
pub(crate) fn invariant_trivial_affine_local_admission(
    function: &PsiOptimizationFunction,
    component: &OptimizerCycleComponent,
    node: &OptimizationNode,
) -> Option<PlaceId> {
    let place = admissible_invariant_trivial_affine_local(node)?;
    scalar_case_result_contained(function, component, place.id).then_some(place.id)
}
