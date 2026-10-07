//! Optimizer module role: admission leaf. Proven field-value predicates behind the plan.
//!
//! The plan is built from two predicates only: the place's
//! declaration/type/proof evidence and the per-node admissibility that
//! resolves one field observation's proven stored value. The independent
//! validator in `optimization-unit-semantics` re-derives the same predicates
//! from the candidate's rows rather than trusting this enumeration — a
//! matcher cannot attest to itself.

use super::{
    FieldValueResolution, FieldValueRow, NodeLocation, O, OperationId, PlaceId,
    PsiOptimizationFunction, PsiOptimizationUnit, ScalarType,
};
use crate::representation_specialization::admission::{bound_place, leaf_copy_projection};
use optimization_unit::{
    EstablishmentScalar, FieldEvidence, ForwardedFieldValue, FunctionAnalysis, ObservedFieldKind,
    OptimizationBlock, OptimizationNode, Position, ProvenFieldValue, RootProducer, ValueDefinition,
    ValueDefinitionSite, bound_value, case_field_initializer, constant_definition,
    declared_structural_type, matches_constant_kind, matches_observed_kind,
    observed_field_declaration, record_initializer, root_producer, structural_child, zeroed_fold,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, StructuralFieldId, StructuralTypeId, ValueId,
};
use std::collections::BTreeSet;
use terminal_psi::StructuralPlaceDeclaration;

/// The admissibility of one node under `evidence`: a Boolean or integer
/// `StructuralScalarFieldRead` observing the evidence's place whose stored
/// value the unit proves. `None` for every other node.
pub(super) fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<FieldValueRow> {
    let O::StructuralScalarFieldRead {
        psi_operation,
        result,
        source,
        path,
        field,
    } = &node.operation
    else {
        return None;
    };
    let kind = ObservedFieldKind::of(result.scalar_type)?;
    let (result, scalar_type) = (result.value, result.scalar_type);
    if *source != evidence.declaration.id {
        return None;
    }
    let proven = proven_field_value(
        unit,
        evidence,
        function,
        analysis,
        block,
        node_index,
        node,
        result,
        scalar_type,
        path,
        *field,
        kind,
    )?;
    Some(FieldValueRow {
        site: NodeLocation {
            machine: function.machine,
            block: block.id,
            node: u32::try_from(node_index).ok()?,
        },
        psi_operation: *psi_operation,
        result,
        source: evidence.declaration.id,
        path: path.clone(),
        field: *field,
        producer: proven.producer,
        resolution: proven.resolution,
    })
}

/// The stored value the unit proves for `field` at `path` under `place`'s
/// evidence, or `None` when no basis applies. An establishment basis — an
/// `EstablishRecord` producer at an empty path, an `EstablishCase`
/// producer whose `result_case` matches a lone `Case` path, or the same two
/// reached through `Field` descents across owned, complete structural
/// children — proves the field's initializer scalar directly: a
/// same-function constant folds the read to a literal, while a nonconstant
/// initializer forwards to the read's uses when the substitution lane
/// covers every use site and the initializer dominates them all. A declared
/// `BoundedInteger` singleton bound proves a literal at any resolvable path
/// independently of producer and outranks a nonconstant initializer — the
/// literal is strictly more resolved.
#[allow(clippy::too_many_arguments)]
fn proven_field_value(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    result: ValueId,
    scalar_type: ScalarType,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
    kind: ObservedFieldKind,
) -> Option<ProvenFieldValue> {
    let declaration = observed_field_declaration(unit, evidence, path, field)?;
    if !matches_observed_kind(declaration, kind) {
        return None;
    }
    let establishment = establishment_scalar(unit, evidence, function, path, field);
    match establishment {
        // A zeroed establishment fixes the leaf at literal zero outright —
        // there is no initializer operand to fold or forward.
        Some(EstablishmentScalar::Zeroed(producer)) => {
            return Some(ProvenFieldValue {
                producer: Some(producer),
                resolution: FieldValueResolution::Constant(zeroed_fold(declaration, kind)?),
            });
        }
        Some(EstablishmentScalar::Initializer(producer, initializer)) => {
            if let Some(value) = constant_definition(function, initializer)
                && matches_constant_kind(value, kind)
            {
                return Some(ProvenFieldValue {
                    producer: Some(producer),
                    resolution: FieldValueResolution::Constant(value),
                });
            }
        }
        None => {}
    }
    if let Some(proven) = bound_value(declaration, kind) {
        return Some(proven);
    }
    let EstablishmentScalar::Initializer(producer, initializer) = establishment? else {
        return None;
    };
    forwarded_resolution(
        function,
        analysis,
        block,
        node_index,
        node,
        result,
        scalar_type,
        initializer,
    )
    .map(|forwarded| ProvenFieldValue {
        producer: Some(producer),
        resolution: FieldValueResolution::Forward(forwarded),
    })
}

/// The establishment basis: the chain of establishing producers along `path`
/// proves the field's stored scalar by name. `Some(Initializer(..))` when
/// the producer at the path's terminal position establishes the observed
/// field and its initializer is a scalar; `Some(Zeroed(..))` when that
/// producer is an `EstablishZeroedStructural`, which the verifier admits
/// only on zero-constructible shapes — every scalar leaf the remaining path
/// resolves is stored at zero; `None` when any step is not established. A
/// `Field` descent crosses only into an `EstablishRecord` field stored as
/// an owned, complete structural child place whose declared type is exactly
/// the field's declared carrier — a pathed or borrowed argument, a
/// non-operation-result place, and every other producer leave the nested
/// position unproven. A place with no producer of its own — a block
/// parameter — crosses only into the one place every incoming edge binds
/// it to whole when nothing rewrites or mutably re-lends it; a
/// `StructuralLeafCopy` result crosses into its source with the copy's
/// projection — canonical steps directly, ordinary field identities
/// resolved to their roster handles — prefixed to the path still to
/// resolve. The caller
/// decides what an initializer proves — a literal fold when it resolves to
/// a same-function constant, a use substitution when it does not.
fn establishment_scalar(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<EstablishmentScalar> {
    let mut place = evidence.declaration.id;
    let mut producer = evidence.root_producer;
    let mut declared = evidence.root_type;
    // Owned because a leaf copy prepends its own projection — canonical
    // steps directly, ordinary field identities resolved to roster handles
    // — to the path still to resolve beneath its source.
    let mut segments = path.to_vec();
    let mut visiting = BTreeSet::from([place]);
    loop {
        match (producer, segments.as_slice()) {
            (RootProducer::Record(producer), []) => {
                // An `EstablishRecord` proves its declaration-ordered field
                // initializers: the observed field's scalar initializer fixes
                // the stored value permanently.
                return record_initializer(function, producer, place, field)
                    .map(|initializer| EstablishmentScalar::Initializer(producer, initializer));
            }
            (RootProducer::Zeroed(operation), _) => {
                // A zeroed establishment writes every scalar leaf of the
                // resolved position to zero — the verifier admits it only
                // on zero-constructible shapes, so the remaining canonical
                // path needs no child place to cross: it descends the
                // declared type alone, and any field it reaches is stored
                // at zero. `Case` enters a case-payload namespace whose
                // scalar fields read the same zeroed bytes.
                let mut position = Position::Type(declared?);
                for segment in &segments {
                    position = position.descend(unit, segment)?;
                }
                position.field_declaration(unit, field)?;
                return Some(EstablishmentScalar::Zeroed(operation));
            }
            (
                RootProducer::Variant(producer, result_case),
                [CanonicalStructuralPathSegment::Case(observed_case)],
            ) if result_case == *observed_case => {
                // An `EstablishCase` proves its scalar case-field
                // initializers for exactly the case it establishes.
                return case_field_initializer(function, producer, place, field)
                    .map(|initializer| EstablishmentScalar::Initializer(producer, initializer));
            }
            (
                RootProducer::Variant(operation, result_case),
                [
                    CanonicalStructuralPathSegment::Case(observed_case),
                    CanonicalStructuralPathSegment::Field(identity),
                    ..,
                ],
            ) if result_case == *observed_case => {
                // A `Case` then `Field` pair descends into the payload child
                // an `EstablishCase` stores whole — the structural-case
                // establishment. The same custody as a record child applies:
                // the payload carrier must be exactly the child place's
                // declared type, reached as one owned, complete structural
                // argument. A different observed case never crosses: its
                // payload overlays storage this establishment did not write.
                let identity = *identity;
                let Position::Type(next) = Position::Type(declared?)
                    .descend(unit, &segments[0])?
                    .descend(unit, &segments[1])?
                else {
                    return None;
                };
                let (child, declaration) = descended_field_child(
                    function,
                    operation,
                    place,
                    identity,
                    next,
                    &mut visiting,
                )?;
                place = child;
                producer = root_producer(function, declaration);
                declared = Some(next);
                segments.drain(..2);
            }
            (
                RootProducer::Record(operation),
                [CanonicalStructuralPathSegment::Field(identity), ..],
            ) => {
                // A `Field` segment descends into the nested carrier the
                // record stores whole: the declared carrier must be exactly
                // the child place's declared type, and the child must reach
                // this producer's initializer as one owned, complete
                // structural argument. A `FixedIndex` segment never crosses —
                // an `EstablishScalarArray` element is not a placed child.
                let identity = *identity;
                let Position::Type(next) = Position::Type(declared?).descend(unit, &segments[0])?
                else {
                    return None;
                };
                let (child, declaration) = descended_field_child(
                    function,
                    operation,
                    place,
                    identity,
                    next,
                    &mut visiting,
                )?;
                place = child;
                producer = root_producer(function, declaration);
                declared = Some(next);
                segments.drain(..1);
            }
            (RootProducer::None, _) => {
                // A leaf copy — the case-payload place a `Case { payload }`
                // arm binding mints, or an ordinary projection's fresh leaf
                // — holds exactly its source's contents at the copied
                // projection: the copy is fresh storage of an
                // `Unrestricted` or owned non-reference leaf, and the source
                // resolves only through establishments and uniform bindings
                // nothing rewrites. The copy's projection therefore prefixes
                // the path still to resolve beneath its source — canonical
                // steps directly, ordinary field identities after resolving
                // each to its roster's verifier handle — and the source's
                // own chain decides the proof; a source that establishes a
                // different case declines at the `Case` step.
                if let Some((source, copied)) = leaf_copy_projection(unit, function, place) {
                    let declaration = function
                        .structural_places
                        .iter()
                        .find(|declaration| declaration.id == source)?;
                    let source_type = declared_structural_type(function, declaration)?;
                    let mut position = Position::Type(source_type);
                    for segment in &copied {
                        position = position.descend(unit, segment)?;
                    }
                    if !matches!(position, Position::Type(landed) if Some(landed) == declared) {
                        return None;
                    }
                    if !visiting.insert(source) {
                        return None;
                    }
                    place = source;
                    producer = root_producer(function, declaration);
                    declared = Some(source_type);
                    segments.splice(..0, copied.iter().copied());
                    continue;
                }
                // The position's place holds no producer of its own, but a
                // block parameter arrives carrying exactly the place its
                // uniform binding names — the bound place's own
                // establishment is the parameter's, provided the binding
                // preserves the declared structural type exactly.
                let bound = bound_place(function, place)?;
                let declaration = function
                    .structural_places
                    .iter()
                    .find(|declaration| declaration.id == bound)?;
                let bound_type = declared_structural_type(function, declaration);
                if declared.is_some() && bound_type != declared {
                    return None;
                }
                if !visiting.insert(bound) {
                    return None;
                }
                place = bound;
                producer = root_producer(function, declaration);
                declared = bound_type;
            }
            _ => return None,
        }
    }
}

/// The `Forward` resolution for one nonconstant-initialized read, or `None`
/// when the substitution cannot retire the observation exactly: the
/// initializer must resolve to a same-function definition of the read's own
/// scalar type and dominate every use site, every scalar-operand use of the
/// read's result must sit in an operation the substitution lane rewrites,
/// and the observation node itself must be cleanly removable — exactly its
/// result definition, no successors, no ownership events, a following node
/// to absorb its custody, and no shared provenance with that receiver. Field
/// observations emit no optimization facts, so no fact custody is owed.
fn forwarded_resolution(
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    result: ValueId,
    scalar_type: ScalarType,
    initializer: ValueId,
) -> Option<ForwardedFieldValue> {
    if initializer == result {
        return None;
    }
    let definition = analysis.definitions.get(&initializer)?;
    if definition.scalar_type != scalar_type {
        return None;
    }
    let node_index_u32 = u32::try_from(node_index).ok()?;
    if node.definitions
        != [ValueDefinition {
            value: result,
            scalar_type,
            site: ValueDefinitionSite::Node {
                block: block.id,
                node: node_index_u32,
            },
        }]
        || !node.successors.is_empty()
        || !node.ownership.is_empty()
    {
        return None;
    }
    let receiver = block.nodes.get(node_index.checked_add(1)?)?;
    if receiver
        .provenance
        .iter()
        .any(|source| node.provenance.contains(source))
    {
        return None;
    }
    // Every site that references the read's result: the tracked scalar-operand
    // uses, which include each runtime-selected path element's selector. A
    // use inside a byte-sequence operation, or as a runtime selector whose
    // accepted certificate names that exact value, is outside the lane, so
    // the row is inadmissible rather than partially substituted.
    let mut uses = BTreeSet::new();
    for use_block in &function.blocks {
        for (use_index, use_node) in use_block.nodes.iter().enumerate() {
            let referenced = use_node
                .uses
                .iter()
                .any(|use_site| use_site.value == result);
            if !referenced {
                continue;
            }
            if matches!(
                &use_node.operation,
                O::ByteSequenceRead { .. }
                    | O::ByteSequencePredicate { .. }
                    | O::ByteSequenceEqual { .. }
                    | O::StructuralByteSequenceFieldRead { .. }
                    | O::ByteSequenceWrite { .. }
                    | O::ByteSequenceSubslice { .. }
                    | O::ByteSequenceStore { .. }
                    | O::StructuralByteSequenceFieldStore { .. }
                    | O::StructuralByteSequenceFieldByteStore { .. }
            ) || use_node
                .operation
                .runtime_indices()
                .iter()
                .any(|(index, _)| *index == result)
            {
                return None;
            }
            let site = NodeLocation {
                machine: function.machine,
                block: use_block.id,
                node: u32::try_from(use_index).ok()?,
            };
            let dominates = match definition.site {
                ValueDefinitionSite::FunctionParameter(_) => true,
                ValueDefinitionSite::BlockParameter {
                    block: defining, ..
                } => analysis
                    .dominators
                    .get(&use_block.id)
                    .is_some_and(|set| set.contains(&defining)),
                ValueDefinitionSite::Node {
                    block: defining,
                    node: defined_at,
                } => {
                    if defining == use_block.id {
                        defined_at < site.node
                    } else {
                        analysis
                            .dominators
                            .get(&use_block.id)
                            .is_some_and(|set| set.contains(&defining))
                    }
                }
            };
            if !dominates {
                return None;
            }
            uses.insert(site);
        }
    }
    Some(ForwardedFieldValue {
        initializer,
        scalar_type,
        uses: uses.into_iter().collect(),
    })
}

/// The placed child `identity` establishes for `place`, after confirming its
/// declared type is `next` and admitting it to `visiting`.
fn descended_field_child<'function>(
    function: &'function PsiOptimizationFunction,
    operation: OperationId,
    place: PlaceId,
    identity: StructuralFieldId,
    next: StructuralTypeId,
    visiting: &mut BTreeSet<PlaceId>,
) -> Option<(PlaceId, &'function StructuralPlaceDeclaration)> {
    let child = structural_child(function, operation, place, identity)?;
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == child)?;
    if declared_structural_type(function, declaration) != Some(next) {
        return None;
    }
    if !visiting.insert(child) {
        return None;
    }
    Some((child, declaration))
}
