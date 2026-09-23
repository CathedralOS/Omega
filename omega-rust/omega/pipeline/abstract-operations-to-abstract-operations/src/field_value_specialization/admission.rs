//! Optimizer module role: admission leaf. Proven field-value predicates behind the plan.
//!
//! The plan is built from two predicates only: the place's
//! declaration/type/proof evidence and the per-node admissibility that
//! resolves one field observation's proven stored value. The independent
//! validator in `optimization-unit-semantics` re-derives the same predicates
//! from the candidate's rows rather than trusting this enumeration — a
//! matcher cannot attest to itself.

use super::{
    FieldValueResolution, FieldValueRow, FoldedFieldValue, NodeLocation, O, OperationId, PlaceId,
    PsiOptimizationFunction, PsiOptimizationUnit, ScalarType, StructuralPlaceKind,
};
use crate::representation_specialization::admission::declared_structural_type;
use optimization_unit::{
    ForwardedFieldValue, OptimizationBlock, OptimizationNode, ValueDefinition, ValueDefinitionSite,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, StructuralCaseId, StructuralFieldId, StructuralTypeId,
    ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    RecordFieldValue, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPlaceDeclaration, StructuralTypeShape,
};

/// The establishing operation the place's operation-result producer resolves
/// to, when it is one this family's proofs can draw on: `Record` for an
/// `EstablishRecord`, `Variant` for an `EstablishScalarCase` carrying its
/// `result_case`, `None` for every other producer or place kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RootProducer {
    None,
    Record(OperationId),
    Variant(OperationId, StructuralCaseId),
}

impl RootProducer {
    /// The establishing operation identity, when the place's producer is an
    /// establishing op this family's proofs can draw on.
    pub(super) const fn operation(&self) -> Option<OperationId> {
        match self {
            Self::Record(operation) | Self::Variant(operation, _) => Some(*operation),
            Self::None => None,
        }
    }
}

/// The evidence every field observation of `place` resolves against: the
/// place's rostered declaration, its declared structural type (when the place
/// kind carries one), and the establishing producer the place's
/// operation-result kind names. `None` when `place` is not rostered in
/// `function`.
pub(super) struct FieldEvidence<'a> {
    pub(super) declaration: &'a StructuralPlaceDeclaration,
    pub(super) root_type: Option<StructuralTypeId>,
    pub(super) root_producer: RootProducer,
}

/// The field evidence for one rostered place, or `None` when the place does
/// not exist in `function`.
pub(super) fn field_evidence<'a>(
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
) -> Option<FieldEvidence<'a>> {
    let declaration = function
        .structural_places
        .iter()
        .find(|declaration| declaration.id == place)?;
    Some(FieldEvidence {
        declaration,
        root_type: declared_structural_type(function, declaration),
        root_producer: root_producer(function, declaration),
    })
}

/// The establishing producer `declaration`'s operation-result kind names, or
/// `RootProducer::None` when the place is not an operation result or its
/// producer is not an `EstablishRecord`/`EstablishScalarCase` in `function`.
fn root_producer(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> RootProducer {
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        return RootProducer::None;
    };
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        match &node.operation {
            O::EstablishRecord {
                psi_operation,
                result,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return RootProducer::Record(producer);
            }
            O::EstablishScalarCase {
                psi_operation,
                result,
                result_case,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return RootProducer::Variant(producer, *result_case);
            }
            _ => {}
        }
    }
    RootProducer::None
}

/// The per-function value graph a forward admission resolves against: every
/// value's defining site and scalar type, plus the block dominator tree the
/// initializer must dominate each rewritten use under.
pub(super) struct FunctionAnalysis {
    definitions: BTreeMap<ValueId, ValueDefinition>,
    dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
}

/// Independently derived value definitions and block dominators for
/// `function`, matching the shape unit validation itself reconstructs.
pub(super) fn function_analysis(function: &PsiOptimizationFunction) -> FunctionAnalysis {
    let mut predecessors: BTreeMap<BlockId, BTreeSet<BlockId>> = function
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect();
    let mut definitions: BTreeMap<ValueId, ValueDefinition> = BTreeMap::new();
    for parameter in &function.parameters {
        definitions.insert(parameter.value, *parameter);
    }
    for block in &function.blocks {
        for parameter in &block.parameters {
            definitions.insert(parameter.value, *parameter);
        }
        for node in &block.nodes {
            for definition in &node.definitions {
                definitions.insert(definition.value, *definition);
            }
            for edge in &node.successors {
                predecessors
                    .get_mut(&edge.target)
                    .expect("validated successor target")
                    .insert(block.id);
            }
        }
    }
    FunctionAnalysis {
        definitions,
        dominators: block_dominators(function, &predecessors),
    }
}

/// The block dominator tree for `function`: the iterative meet-over-all-
/// predecessors fixpoint with the entry block dominating only itself.
fn block_dominators(
    function: &PsiOptimizationFunction,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let all: BTreeSet<BlockId> = function.blocks.iter().map(|block| block.id).collect();
    let mut result: BTreeMap<BlockId, BTreeSet<BlockId>> = all
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == function.entry {
                [function.entry].into_iter().collect()
            } else {
                all.clone()
            };
            (block, initial)
        })
        .collect();
    loop {
        let mut changed = false;
        for block in all.iter().copied().filter(|block| *block != function.entry) {
            let incoming = &predecessors[&block];
            let mut next = if let Some(first) = incoming.first() {
                result[first].clone()
            } else {
                BTreeSet::new()
            };
            for predecessor in incoming.iter().skip(1) {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if result[&block] != next {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

/// The scalar kind a resolved observation must carry: `Boolean` for a
/// `BooleanStructuralField` result, `Integer` for an `IntegerStructuralField`
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservedFieldKind {
    Boolean,
    Integer,
}

/// The proven stored value of one field observation plus the witness it was
/// proven under: `Some(producer)` for an establishment basis — whether the
/// initializer folds to a literal or forwards as a substitution — `None` for
/// a declared singleton bound.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProvenFieldValue {
    producer: Option<OperationId>,
    resolution: FieldValueResolution,
}

/// The admissibility of one node under `evidence`: a `BooleanStructuralField`
/// or `IntegerStructuralField` observing the evidence's place whose stored
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
    let (psi_operation, result, scalar_type, source, path, field, kind) = match &node.operation {
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (
            psi_operation,
            *result,
            ScalarType::Boolean,
            source,
            path,
            field,
            ObservedFieldKind::Boolean,
        ),
        O::IntegerStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => {
            if !matches!(result.scalar_type, ScalarType::Integer(_)) {
                return None;
            }
            (
                psi_operation,
                result.value,
                result.scalar_type,
                source,
                path,
                field,
                ObservedFieldKind::Integer,
            )
        }
        _ => return None,
    };
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
/// `EstablishRecord` producer at an empty path, an `EstablishScalarCase`
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
    if let Some((producer, initializer)) = establishment
        && let Some(value) = constant_definition(function, initializer)
        && matches_constant_kind(value, kind)
    {
        return Some(ProvenFieldValue {
            producer: Some(producer),
            resolution: FieldValueResolution::Constant(value),
        });
    }
    if let Some(proven) = bound_value(declaration, kind) {
        return Some(proven);
    }
    let (producer, initializer) = establishment?;
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

/// The declared field the observation resolves to: the `path` descends from
/// the place's declared root type to a structural position, and `field`
/// selects one scalar declaration inside it. `None` when the root carries no
/// type, a segment does not apply — a `Field` name absent from a
/// `Record`/`Mixed` common-field or case-payload roster, a `FixedIndex` on a
/// non-array, a `Case` on a non-sum — or the path ends on a position with no
/// field roster.
fn observed_field_declaration<'a>(
    unit: &'a PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<&'a StructuralFieldDeclaration> {
    let mut position = Position::Type(evidence.root_type?);
    for segment in path {
        position = position.descend(unit, segment)?;
    }
    position.field_declaration(unit, field)
}

/// One structural position a canonical path resolves to: either a declared
/// structural type or one case's payload-field roster.
#[derive(Debug, Clone, Copy)]
enum Position<'a> {
    Type(StructuralTypeId),
    CasePayload(&'a StructuralCaseDeclaration),
}

impl<'a> Position<'a> {
    /// The position one canonical segment descends to, or `None` when the
    /// segment does not apply to this position's shape.
    fn descend(
        self,
        unit: &'a PsiOptimizationUnit,
        segment: &CanonicalStructuralPathSegment,
    ) -> Option<Position<'a>> {
        match (self, segment) {
            (Self::Type(current), CanonicalStructuralPathSegment::Field(identity)) => {
                let fields = match shape(unit, current)? {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => return None,
                };
                match fields
                    .iter()
                    .find(|field| field.id == *identity)?
                    .field_type
                {
                    StructuralFieldType::Structural(next) => Some(Self::Type(next)),
                    _ => None,
                }
            }
            (Self::Type(current), CanonicalStructuralPathSegment::FixedIndex(_)) => {
                match shape(unit, current)? {
                    StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                    _ => None,
                }
            }
            (Self::Type(current), CanonicalStructuralPathSegment::Case(identity)) => {
                let cases = match shape(unit, current)? {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return None,
                };
                cases
                    .iter()
                    .find(|case| case.id == *identity)
                    .map(Self::CasePayload)
            }
            (Self::CasePayload(case), CanonicalStructuralPathSegment::Field(identity)) => {
                match case
                    .fields
                    .iter()
                    .find(|field| field.id == *identity)?
                    .field_type
                {
                    StructuralFieldType::Structural(next) => Some(Self::Type(next)),
                    _ => None,
                }
            }
            (Self::CasePayload(_), _) => None,
        }
    }

    /// The declaration `field` names inside this position's field roster, or
    /// `None` when the position carries no roster or no such field.
    fn field_declaration(
        self,
        unit: &'a PsiOptimizationUnit,
        field: StructuralFieldId,
    ) -> Option<&'a StructuralFieldDeclaration> {
        let fields = match self {
            Self::Type(current) => match shape(unit, current)? {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return None,
            },
            Self::CasePayload(case) => &case.fields,
        };
        fields.iter().find(|declaration| declaration.id == field)
    }
}

/// The declared structural shape of `type_id` in `unit`.
fn shape(unit: &PsiOptimizationUnit, type_id: StructuralTypeId) -> Option<&StructuralTypeShape> {
    unit.structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)
        .map(|entry| &entry.shape)
}

/// Whether the resolved field declaration's scalar carrier matches the
/// observation kind — a Boolean read needs a `Boolean` field carrier, an
/// integer read needs an `Integer` carrier (plain or bounded). `Erased`,
/// structural, byte-sequence, and float fields never match.
fn matches_observed_kind(
    declaration: &StructuralFieldDeclaration,
    kind: ObservedFieldKind,
) -> bool {
    matches!(
        (kind, declaration.field_type.scalar_type()),
        (ObservedFieldKind::Boolean, Some(ScalarType::Boolean))
            | (ObservedFieldKind::Integer, Some(ScalarType::Integer(_)))
    )
}

/// The establishment basis: the chain of establishing producers along `path`
/// proves the field's stored scalar by name. `Some((producer, initializer))`
/// when the producer at the path's terminal position establishes the
/// observed field and its initializer is a scalar; `None` when any step is
/// not established. A `Field` descent crosses only into an `EstablishRecord`
/// field stored as an owned, complete structural child place whose declared
/// type is exactly the field's declared carrier — a pathed or borrowed
/// argument, a non-operation-result place, and every other producer leave
/// the nested position unproven. The caller decides what the initializer
/// proves — a literal fold when it resolves to a same-function constant, a
/// use substitution when it does not.
fn establishment_scalar(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
) -> Option<(OperationId, ValueId)> {
    let mut place = evidence.declaration.id;
    let mut producer = evidence.root_producer;
    let mut declared = evidence.root_type;
    let mut segments = path;
    loop {
        match (producer, segments) {
            (RootProducer::Record(producer), []) => {
                // An `EstablishRecord` proves its declaration-ordered field
                // initializers: the observed field's scalar initializer fixes
                // the stored value permanently.
                return record_initializer(function, producer, place, field)
                    .map(|initializer| (producer, initializer));
            }
            (
                RootProducer::Variant(producer, result_case),
                [CanonicalStructuralPathSegment::Case(observed_case)],
            ) if result_case == *observed_case => {
                // An `EstablishScalarCase` proves its scalar case-field
                // initializers for exactly the case it establishes.
                return case_field_initializer(function, producer, place, field)
                    .map(|initializer| (producer, initializer));
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
                let Position::Type(next) = Position::Type(declared?).descend(unit, &segments[0])?
                else {
                    return None;
                };
                let child = structural_child(function, operation, place, *identity)?;
                let declaration = function
                    .structural_places
                    .iter()
                    .find(|declaration| declaration.id == child)?;
                if declared_structural_type(function, declaration) != Some(next) {
                    return None;
                }
                place = child;
                producer = root_producer(function, declaration);
                declared = Some(next);
                segments = &segments[1..];
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
    // uses plus `WriteOnlyIndexedPrimitiveStore` operands — the substitution
    // lane rewrites those positions even though the use index does not track
    // them. A use inside a byte-sequence operation is outside the lane, so
    // the row is inadmissible rather than partially substituted.
    let mut uses = BTreeSet::new();
    for use_block in &function.blocks {
        for (use_index, use_node) in use_block.nodes.iter().enumerate() {
            let referenced = use_node
                .uses
                .iter()
                .any(|use_site| use_site.value == result)
                || matches!(
                    &use_node.operation,
                    O::WriteOnlyIndexedPrimitiveStore { index, value, .. }
                        if index.value == result || value.value == result
                );
            if !referenced {
                continue;
            }
            if matches!(
                &use_node.operation,
                O::ByteSequenceRead { .. }
                    | O::ByteSequenceWrite { .. }
                    | O::ByteSequenceSubslice { .. }
                    | O::StructuralByteSequenceFieldStore { .. }
                    | O::StructuralByteSequenceFieldByteStore { .. }
            ) {
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

/// The `EstablishRecord` node's field initializers when `producer` is that
/// node on `place`, or `None` when no such node exists.
fn establish_record_fields(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
) -> Option<&[terminal_psi::RecordFieldInitializer]> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishRecord {
                psi_operation,
                result,
                fields,
            } if *psi_operation == producer && result.place == place => Some(fields.as_slice()),
            _ => None,
        })
}

/// The `ValueId` the `EstablishRecord` producer stores into `field`, or
/// `None` when the producer node is absent, is not an `EstablishRecord` on
/// this place, or the initializer is not a scalar.
fn record_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<ValueId> {
    establish_record_fields(function, producer, place)?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match &initializer.value {
            RecordFieldValue::Scalar { value, .. } => Some(*value),
            _ => None,
        })
}

/// The child place the `EstablishRecord` producer stores whole into `field`,
/// or `None` when the producer node is absent, is not an `EstablishRecord`
/// on this place, or `field`'s initializer is not an owned, complete
/// structural argument — a pathed argument or a borrow does not carry the
/// child's own establishment.
fn structural_child(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<PlaceId> {
    establish_record_fields(function, producer, place)?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match &initializer.value {
            RecordFieldValue::Structural(argument)
                if argument.path.is_empty()
                    && argument.access == terminal_psi::StructuralAccess::Owned =>
            {
                Some(argument.place)
            }
            _ => None,
        })
}

/// The `ValueId` the `EstablishScalarCase` producer stores into `field`, or
/// `None` when the producer node is absent, is not an `EstablishScalarCase`
/// on this place, or the case field is absent.
fn case_field_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<ValueId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishScalarCase {
                psi_operation,
                result,
                fields,
                ..
            } if *psi_operation == producer && result.place == place => fields
                .iter()
                .find(|initializer| initializer.field == field)
                .map(|initializer| initializer.value),
            _ => None,
        })
}

/// The `FoldedFieldValue` `value` resolves to inside `function`: a
/// `BooleanConstant`/`IntegerConstant` defines it directly, or a block
/// parameter every incoming edge binds to the same argument resolves
/// transitively — the linear chains the front end emits route each
/// initializer through block parameters rather than spelling the constant at
/// the use site. A non-constant definition, a parameter bound to divergent
/// arguments on different edges, an edge that fails to bind it, or a cycle
/// in the chain yields `None`.
fn constant_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<FoldedFieldValue> {
    let mut current = value;
    let mut visiting = std::collections::BTreeSet::new();
    loop {
        if !visiting.insert(current) {
            return None;
        }
        if let Some(node) = function
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .find(|node| {
                node.definitions
                    .iter()
                    .any(|definition| definition.value == current)
            })
        {
            return match &node.operation {
                O::BooleanConstant {
                    value: constant, ..
                } => Some(FoldedFieldValue::Boolean(*constant)),
                O::IntegerConstant {
                    value: constant, ..
                } => Some(FoldedFieldValue::Integer(*constant)),
                _ => None,
            };
        }
        let owner = function.blocks.iter().find(|block| {
            block
                .parameters
                .iter()
                .any(|parameter| parameter.value == current)
        })?;
        let mut argument = None;
        for edge in function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().flat_map(|node| node.successors.iter()))
            .filter(|edge| edge.target == owner.id)
        {
            let binding = edge
                .bindings
                .iter()
                .find(|binding| binding.parameter == current)?;
            match argument {
                None => argument = Some(binding.argument),
                Some(seen) if seen != binding.argument => return None,
                _ => {}
            }
        }
        current = argument?;
    }
}

/// Whether a proven constant matches the observation kind — a Boolean read
/// folds only to `Boolean`, an integer read only to `Integer`.
fn matches_constant_kind(value: FoldedFieldValue, kind: ObservedFieldKind) -> bool {
    matches!(
        (value, kind),
        (FoldedFieldValue::Boolean(_), ObservedFieldKind::Boolean)
            | (FoldedFieldValue::Integer(_), ObservedFieldKind::Integer)
    )
}

/// The declared-bound basis: the field's declared `BoundedInteger` bound
/// closes over exactly one value, so every inhabitant holds it. `None` for a
/// non-singleton bound, a non-bounded carrier, or a Boolean observation.
fn bound_value(
    declaration: &StructuralFieldDeclaration,
    kind: ObservedFieldKind,
) -> Option<ProvenFieldValue> {
    if kind != ObservedFieldKind::Integer {
        return None;
    }
    let StructuralFieldType::BoundedInteger(bound) = declaration.field_type else {
        return None;
    };
    (bound.minimum() == bound.maximum()).then_some(ProvenFieldValue {
        producer: None,
        resolution: FieldValueResolution::Constant(FoldedFieldValue::Integer(bound.minimum())),
    })
}
