//! Independent field-value specialization replay mechanics.
//!
//! Validation never trusts the candidate's field rows: it re-derives the
//! claimed place's proof evidence — an `EstablishRecord` producer on an
//! operation-result place at an empty path, an `EstablishScalarCase`
//! producer whose `result_case` matches at a lone `Case` path, the same two
//! reached through `Field` descents across owned, complete structural
//! children, or a declared `BoundedInteger` singleton bound at the position
//! each observation resolves to — replays the admissible observation set
//! for that place,
//! requires the claimed rows to equal the replayed rows exactly, rebuilds
//! the output itself, and reconstructs the exact node custody the resolved
//! observations carry. A `Constant` row folds its read in place; a `Forward`
//! row rebinds every covered scalar-operand use of the read's result to the
//! proven nonconstant initializer and retires the observation node, so the
//! replay also recomputes the substitution set, the exact use-site roster,
//! initializer dominance, and the shifted-custody provenance ledger.

use crate::candidates::rewrite_accounting::preserve_edge_custody;
use crate::candidates::rewrite_accounting::substitutions::rewrite_scalar_value_uses;
use crate::candidates::state_specialization::cyclic_machines;
use crate::candidates::structural_bindings::bound_place;
use crate::unit_validation::derived_metadata::{
    dominators, expected_definitions, expected_ownership, expected_uses,
    reconstruct_declared_places,
};
use crate::unit_validation::function_structure::reconstruct_fact_index;
use crate::{OptimizationUnitValidationError, ValidatedPsiRewrite, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation as O;
use optimization_core::OptimizationValidatorIdentity;
use optimization_unit::{
    FieldValueResolution, FieldValueRow, FoldedFieldValue, ForwardedFieldValue, OptimizationNode,
    ProvenanceDisposition, ProvenanceRewrite, PsiOptimizationFunction, PsiOptimizationUnit,
    PsiRealizationSite, PsiRewriteCandidate, PsiRewritePatch, ScalarSubstitution, ValueDefinition,
    ValueDefinitionSite, recompute_psi_optimization_unit_identity,
};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, OperationId, PlaceId, ScalarType, StructuralCaseId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
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
enum RootProducer {
    None,
    Record(OperationId),
    Variant(OperationId, StructuralCaseId),
}

impl RootProducer {
    /// The establishing operation identity, when the place's producer is an
    /// establishing op this family's proofs can draw on.
    const fn operation(&self) -> Option<OperationId> {
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
struct FieldEvidence<'a> {
    declaration: &'a StructuralPlaceDeclaration,
    root_type: Option<StructuralTypeId>,
    root_producer: RootProducer,
}

/// The field evidence for one rostered place, or `None` when the place does
/// not exist in `function`.
fn field_evidence<'a>(
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

/// The declared structural type of one rostered place, or `None` when the
/// place kind carries no resolvable type in this function.
fn declared_structural_type(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralTypeId> {
    match declaration.kind {
        StructuralPlaceKind::OperationResult {
            structural_type, ..
        }
        | StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        }
        | StructuralPlaceKind::TrivialAffineLocal {
            structural_type, ..
        } => Some(structural_type),
        StructuralPlaceKind::ProviderAttachment { attachment, .. } => Some(attachment),
        StructuralPlaceKind::Parameter { .. } => function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::BlockParameter { .. } => function
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .find(|parameter| parameter.place == declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::Result => function
            .result
            .structural()
            .and_then(|result| (result.place == declaration.id).then_some(result.structural_type)),
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

/// The per-function value graph a forward admission resolves against: every
/// value's defining site and scalar type, plus the block dominator tree the
/// initializer must dominate each rewritten use under.
struct FunctionAnalysis {
    definitions: BTreeMap<ValueId, ValueDefinition>,
    dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
}

/// Independently derived value definitions and block dominators for
/// `function`, matching the shape unit validation itself reconstructs.
fn function_analysis(function: &PsiOptimizationFunction) -> FunctionAnalysis {
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
        dominators: dominators(
            function.entry,
            function.blocks.iter().map(|block| block.id),
            &predecessors,
        ),
    }
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
fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &optimization_unit::OptimizationBlock,
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
        site: optimization_unit::NodeLocation {
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
/// `EstablishRecord` producer at an empty path or an `EstablishScalarCase`
/// producer whose `result_case` matches a lone `Case` path, or the same two
/// reached through `Field` descents across owned, complete structural
/// children — proves the field's initializer scalar directly: a
/// same-function constant folds the read to a literal, while a nonconstant
/// initializer forwards to the read's uses when the substitution lane
/// covers every use site and the initializer dominates them all. A declared
/// `BoundedInteger` singleton bound proves a literal at any resolvable path
/// independently of producer and outranks a nonconstant initializer — the
/// literal is strictly more resolved.
fn proven_field_value(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &optimization_unit::OptimizationBlock,
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
/// the nested position unproven. A place with no producer of its own — a
/// block parameter — crosses only into the one place every incoming edge
/// binds it to whole when nothing rewrites or mutably re-lends it. The
/// caller decides what the initializer proves — a literal fold when it
/// resolves to a same-function constant, a use substitution when it does
/// not.
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
    let mut visiting = BTreeSet::from([place]);
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
                if !visiting.insert(child) {
                    return None;
                }
                place = child;
                producer = root_producer(function, declaration);
                declared = Some(next);
                segments = &segments[1..];
            }
            (RootProducer::None, _) => {
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
    block: &optimization_unit::OptimizationBlock,
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
                    | O::IndexedPrimitiveRead { .. }
                    | O::ByteSequenceWrite { .. }
                    | O::ByteSequenceSubslice { .. }
                    | O::StructuralByteSequenceFieldStore { .. }
                    | O::StructuralByteSequenceFieldByteStore { .. }
            ) {
                return None;
            }
            let site = optimization_unit::NodeLocation {
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

/// The independently derived resolution roster for `place`: every
/// `BooleanStructuralField`/`IntegerStructuralField` observing it whose
/// stored value the per-site basis proves, sorted by site.
fn plan_reads(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
) -> Vec<FieldValueRow> {
    let analysis = function_analysis(function);
    let mut reads = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) =
                admit_field_node(unit, evidence, function, &analysis, block, node_index, node)
            else {
                continue;
            };
            reads.push(row);
        }
    }
    reads.sort_by_key(|row| row.site);
    reads
}

/// The folded node a `Constant` row admits at its site: a
/// `BooleanConstant`/`IntegerConstant` with the read's own custody identity
/// and result value. The roster equality check has already proven the
/// claimed basis — this construction only re-verifies the site's shape.
/// `Forward` rows retire their node instead of folding it, so this
/// construction never applies to them.
fn folded_node(
    row: &FieldValueRow,
    function: &PsiOptimizationFunction,
) -> Result<OptimizationNode, OptimizationUnitValidationError> {
    let index = usize::try_from(row.site.node)
        .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
    let node = function
        .blocks
        .iter()
        .find(|block| block.id == row.site.block)
        .and_then(|block| block.nodes.get(index))
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let (psi_operation, result, source, path, field) = match &node.operation {
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (psi_operation, *result, source, path, field),
        O::IntegerStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (psi_operation, result.value, source, path, field),
        _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
    };
    if row.site.machine != function.machine
        || *psi_operation != row.psi_operation
        || result != row.result
        || *source != row.source
        || path.as_slice() != row.path.as_slice()
        || *field != row.field
    {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let operation = match &row.resolution {
        FieldValueResolution::Constant(FoldedFieldValue::Boolean(value)) => O::BooleanConstant {
            psi_operation: *psi_operation,
            result,
            value: *value,
        },
        FieldValueResolution::Constant(FoldedFieldValue::Integer(value)) => O::IntegerConstant {
            psi_operation: *psi_operation,
            result,
            scalar_type: match &node.operation {
                O::IntegerStructuralField { result, .. } => result.scalar_type,
                _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
            },
            value: *value,
        },
        FieldValueResolution::Forward(_) => {
            return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
        }
    };
    Ok(OptimizationNode {
        operation,
        provenance: node.provenance.clone(),
        fuel: node.fuel.clone(),
        effect: node.effect,
        definitions: node.definitions.clone(),
        uses: node.uses.clone(),
        successors: node.successors.clone(),
        ownership: node.ownership.clone(),
    })
}

/// The substitution set a roster of rows carries: exactly one
/// `result` → `initializer` pair per `Forward` row, in canonical order.
fn expected_substitutions(rows: &[FieldValueRow]) -> Vec<ScalarSubstitution> {
    let mut substitutions = rows
        .iter()
        .filter_map(|row| match &row.resolution {
            FieldValueResolution::Forward(forwarded) => Some(ScalarSubstitution {
                from: row.result,
                to: forwarded.initializer,
                scalar_type: forwarded.scalar_type,
            }),
            FieldValueResolution::Constant(_) => None,
        })
        .collect::<Vec<_>>();
    substitutions.sort();
    substitutions
}

/// The exact block region and node custody a row roster carries. A `Constant`
/// row folds its observation in place: the read's own provenance and fuel
/// stay realized at the same node. A `Forward` row retires its observation
/// node: the read's custody lands at the node that inherits the vacated
/// index — the next surviving input node — and every later node in the block
/// shifts down one coordinate per earlier retirement. Because effects are a
/// function-wide sequence, every block at or after the earliest retired-node
/// block is inside the region, and every provenance-bearing node in a
/// region block whose custody moved — or whose own operation changed — gets
/// one ledger row.
fn expected_accounting(
    function: &PsiOptimizationFunction,
    rows: &[FieldValueRow],
) -> Result<(Vec<BlockId>, Vec<ProvenanceRewrite>), OptimizationUnitValidationError> {
    let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
    let mut fold_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    let mut use_sites: BTreeSet<(BlockId, u32)> = BTreeSet::new();
    for row in rows {
        match &row.resolution {
            FieldValueResolution::Constant(_) => {
                fold_sites.insert((row.site.block, row.site.node));
            }
            FieldValueResolution::Forward(forwarded) => {
                removals
                    .entry(row.site.block)
                    .or_default()
                    .insert(row.site.node);
                for site in &forwarded.uses {
                    use_sites.insert((site.block, site.node));
                }
            }
        }
    }
    let earliest_removal = function
        .blocks
        .iter()
        .position(|block| removals.contains_key(&block.id));
    let mut affected = BTreeSet::new();
    let mut provenance = Vec::new();
    for (block_position, block) in function.blocks.iter().enumerate() {
        let removal = removals.get(&block.id);
        let first_removed = removal.and_then(|set| set.iter().next().copied());
        let in_suffix = earliest_removal.is_some_and(|position| block_position > position);
        let mut block_affected = removal.is_some() || in_suffix;
        for (index, node) in block.nodes.iter().enumerate() {
            let node_index = u32::try_from(index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            let coordinate = (block.id, node_index);
            if fold_sites.contains(&coordinate) || use_sites.contains(&coordinate) {
                block_affected = true;
            }
            let retired = removal.is_some_and(|set| set.contains(&node_index));
            let shifted = first_removed.is_some_and(|first| node_index > first);
            if !retired
                && !shifted
                && !in_suffix
                && !fold_sites.contains(&coordinate)
                && !use_sites.contains(&coordinate)
            {
                continue;
            }
            if !retired && node.provenance.is_empty() {
                continue;
            }
            let input_site = PsiRealizationSite::Node(optimization_unit::NodeLocation {
                machine: function.machine,
                block: block.id,
                node: node_index,
            });
            let output_index = node_index
                .checked_sub(
                    u32::try_from(
                        removal
                            .map(|set| set.iter().filter(|removed| **removed < node_index).count())
                            .unwrap_or(0),
                    )
                    .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
                )
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            let output_site = PsiRealizationSite::Node(optimization_unit::NodeLocation {
                machine: function.machine,
                block: block.id,
                node: output_index,
            });
            provenance.push(ProvenanceRewrite {
                input: input_site,
                disposition: ProvenanceDisposition::RealizedAt(output_site),
                sources: node.provenance.clone(),
                fuel: node.fuel.clone(),
            });
        }
        if block_affected {
            affected.insert(block.id);
        }
    }
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Ok((affected.into_iter().collect(), provenance))
}

pub(super) fn validate(
    input: &PsiOptimizationUnit,
    candidate: &PsiRewriteCandidate,
) -> Result<ValidatedPsiRewrite, OptimizationUnitValidationError> {
    let PsiRewritePatch::SpecializeFieldValue(patch) = candidate.patch() else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    if candidate.node_decision_point() != patch.reads.first().map(|row| row.site) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if patch.reads.is_empty()
        || candidate.predicted_cost_delta()
            != -i64::try_from(patch.reads.len())
                .map_err(|_| OptimizationUnitValidationError::CandidatePatchMismatch)?
    {
        return Err(OptimizationUnitValidationError::CandidateAnalysisContractMismatch);
    }
    // A machine holding a cyclic component is frozen byte-exact: none of its
    // nodes may fold, so any candidate naming it is rejected before replay.
    if cyclic_machines(input).contains(&patch.machine) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let function = input
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    let Some(evidence) = field_evidence(function, patch.place) else {
        return Err(OptimizationUnitValidationError::CandidateLocationMissing);
    };
    if patch.producer != evidence.root_producer.operation() {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let replayed = plan_reads(input, &evidence, function);
    if replayed.is_empty() || patch.reads != replayed {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    if candidate.substitutions() != expected_substitutions(&patch.reads) {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let (expected_blocks, expected_provenance) = expected_accounting(function, &patch.reads)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let input_function = function;
    let folded = patch
        .reads
        .iter()
        .filter(|row| matches!(row.resolution, FieldValueResolution::Constant(_)))
        .map(|row| folded_node(row, input_function).map(|node| (row.site, node)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = input.clone();
    let output_function = output
        .functions
        .iter_mut()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    for (location, node) in folded {
        let index = usize::try_from(location.node)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        let Some(slot) = output_function
            .blocks
            .iter_mut()
            .find(|block| block.id == location.block)
            .and_then(|block| block.nodes.get_mut(index))
        else {
            return Err(OptimizationUnitValidationError::CandidateLocationMissing);
        };
        *slot = node;
    }
    // Rebind every covered use of each forwarded result to its proven
    // initializer before retiring the observation nodes.
    for row in &patch.reads {
        let FieldValueResolution::Forward(forwarded) = &row.resolution else {
            continue;
        };
        for node in output_function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
        {
            rewrite_scalar_value_uses(&mut node.operation, row.result, forwarded.initializer);
        }
    }
    // Retire each forwarded observation in descending node order inside its
    // block so earlier removals never shift a pending coordinate, and fuse
    // the retired custody into the node inheriting the vacated index.
    let mut removals: BTreeMap<BlockId, BTreeSet<u32>> = BTreeMap::new();
    for row in &patch.reads {
        if matches!(row.resolution, FieldValueResolution::Forward(_)) {
            removals
                .entry(row.site.block)
                .or_default()
                .insert(row.site.node);
        }
    }
    for (block_id, sites) in removals {
        let block = output_function
            .blocks
            .iter_mut()
            .find(|block| block.id == block_id)
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        for site in sites.iter().rev() {
            let index = usize::try_from(*site)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            let removed = block.nodes.remove(index);
            let receiver = block
                .nodes
                .get_mut(index)
                .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
            receiver.provenance.extend_from_slice(&removed.provenance);
            receiver.fuel.extend_from_slice(&removed.fuel);
        }
    }
    let mut effect = 0u64;
    for block in &mut output_function.blocks {
        for (node_index, node) in block.nodes.iter_mut().enumerate() {
            let node_index = u32::try_from(node_index)
                .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
            node.definitions = expected_definitions(&node.operation, block.id, node_index);
            node.uses = expected_uses(&node.operation, block.id, node_index);
            node.successors = preserve_edge_custody(node);
            node.ownership = expected_ownership(&node.operation);
            node.effect = optimization_unit::EffectLink {
                input: effect,
                output: effect
                    .checked_add(1)
                    .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?,
            };
            effect = node.effect.output;
        }
    }
    output_function.facts = reconstruct_fact_index(output_function);
    output_function.declared_places = reconstruct_declared_places(output_function)?;
    output.identity = recompute_psi_optimization_unit_identity(&output);
    validate_psi_optimization_unit(&output)?;
    let output_function = output
        .functions
        .iter()
        .find(|function| function.machine == patch.machine)
        .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
    for input_block in &input_function.blocks {
        if !expected_blocks.contains(&input_block.id)
            && output_function
                .blocks
                .iter()
                .find(|block| block.id == input_block.id)
                != Some(input_block)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    for input_function_other in &input.functions {
        if input_function_other.machine != patch.machine
            && !output.functions.contains(input_function_other)
        {
            return Err(OptimizationUnitValidationError::CandidateOutsideRegionMismatch);
        }
    }
    Ok(ValidatedPsiRewrite {
        unit: output,
        candidate: candidate.identity(),
        validator: OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.validator.field-value-specialization.v1",
        ),
        provenance: expected_provenance,
    })
}
