//! Structural-place evidence queries shared by specialization admission and
//! replay: a rostered place's declared structural type and establishing
//! producer.

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, IntegerType, IntegerValue, OperationId,
    PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    RecordFieldValue, StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeShape,
};

use super::{
    FieldValueResolution, FoldedFieldValue, OptimizationBlock, OptimizationNode,
    PsiOptimizationFunction, PsiOptimizationUnit, ScalarConstantValue, ValueDefinition,
};

/// The establishing operation the place's operation-result producer resolves
/// to, when it is one this family's proofs can draw on: `Record` for an
/// `EstablishRecord`, `Variant` for an `EstablishCase` carrying its
/// `result_case`, `Zeroed` for an `EstablishZeroedStructural` whose all-zero
/// storage fixes every scalar leaf at zero, `None` for every other producer
/// or place kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootProducer {
    None,
    Record(OperationId),
    Variant(OperationId, StructuralCaseId),
    Zeroed(OperationId),
}

impl RootProducer {
    /// The establishing operation identity, when the place's producer is an
    /// establishing op this family's proofs can draw on.
    pub const fn operation(&self) -> Option<OperationId> {
        match self {
            Self::Record(operation) | Self::Variant(operation, _) | Self::Zeroed(operation) => {
                Some(*operation)
            }
            Self::None => None,
        }
    }
}

/// The evidence every field observation of `place` resolves against: the
/// place's rostered declaration, its declared structural type (when the place
/// kind carries one), and the establishing producer the place's
/// operation-result kind names. `None` when `place` is not rostered in
/// `function`.
pub struct FieldEvidence<'a> {
    pub declaration: &'a StructuralPlaceDeclaration,
    pub root_type: Option<StructuralTypeId>,
    pub root_producer: RootProducer,
}

/// The field evidence for one rostered place, or `None` when the place does
/// not exist in `function`.
pub fn field_evidence<'a>(
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
/// producer is not an `EstablishRecord`/`EstablishCase` in `function`.
pub fn root_producer(
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
            O::EstablishCase {
                psi_operation,
                result,
                result_case,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return RootProducer::Variant(producer, *result_case);
            }
            O::EstablishZeroedStructural {
                psi_operation,
                result,
            } if *psi_operation == producer && result.place == declaration.id => {
                return RootProducer::Zeroed(producer);
            }
            _ => {}
        }
    }
    RootProducer::None
}

/// The declared structural type of one rostered place, or `None` when the
/// place kind carries no resolvable type in this function.
pub fn declared_structural_type(
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

/// The `EstablishRecord` or `EstablishCase` node's field initializers when
/// `producer` is that node on `place`, or `None` when no such node exists.
pub fn establishment_fields(
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
            }
            | O::EstablishCase {
                psi_operation,
                result,
                fields,
                ..
            } if *psi_operation == producer && result.place == place => Some(fields.as_slice()),
            _ => None,
        })
}

/// The child place the `EstablishRecord` producer stores whole into `field`,
/// or `None` when the producer node is absent, is not an `EstablishRecord`
/// on this place, or `field`'s initializer is not an owned, complete
/// structural argument — a pathed argument or a borrow does not carry the
/// child's own establishment.
pub fn structural_child(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<PlaceId> {
    establishment_fields(function, producer, place)?
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

/// The declared structural shape of `type_id` in `unit`.
pub fn shape(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<&StructuralTypeShape> {
    unit.structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)
        .map(|entry| &entry.shape)
}

/// The field `identity` names inside `current`'s `Record`/`Mixed`
/// common-field roster, with the structural type the field declares.
/// `None` when the shape has no common-field roster, the name is absent, or
/// the field's declared type is not structural.
pub fn descended_field(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    identity: &str,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    descended_field_matching(unit, current, |field| field.identity == identity)
}

/// The same common-field descent by verifier field handle, for the
/// canonical steps a spliced copy projection carries.
pub fn descended_field_id(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    identity: StructuralFieldId,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    descended_field_matching(unit, current, |field| field.id == identity)
}

/// The `Record`/`Mixed` common-field entry `matching` selects inside
/// `current`, with the structural type the field declares. `None` when the
/// shape has no common-field roster, no field matches, or the field's
/// declared type is not structural.
pub fn descended_field_matching(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    matching: impl Fn(&StructuralFieldDeclaration) -> bool,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    let fields = match shape(unit, current)? {
        StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
            fields
        }
        _ => return None,
    };
    let field = fields.iter().find(|field| matching(field))?;
    match field.field_type {
        StructuralFieldType::Structural(next) => Some((field.id, next)),
        _ => None,
    }
}

/// The verifier-handle spelling of one ordinary leaf-copy projection
/// beneath `root`: a `Field` step resolves its source-level identity to the
/// reached `Record`/`Mixed` common-field roster's `StructuralFieldId`,
/// keeping only a `Structural` carrier — a scalar, byte-sequence, or erased
/// leaf mints a carrier this walk cannot follow into stored contents — and
/// a `FixedIndex` step keeps its element descent inside an in-bounds fixed
/// array. `None` when the roster lacks the name, the index leaves a fixed
/// array, or the segment has no canonical step.
pub fn canonicalize_ordinary_path(
    unit: &PsiOptimizationUnit,
    mut current: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut canonical = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                let fields = match shape(unit, current)? {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => return None,
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                canonical.push(CanonicalStructuralPathSegment::Field(field.id));
                current = next;
            }
            StructuralPathSegment::FixedIndex(index) => match shape(unit, current)? {
                StructuralTypeShape::FixedArray { element, length } if index < length => {
                    canonical.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                    current = *element;
                }
                _ => return None,
            },
            StructuralPathSegment::Case(_)
            | StructuralPathSegment::RuntimeIndex { .. }
            | StructuralPathSegment::Referent
            | StructuralPathSegment::FixedByteRange { .. } => return None,
        }
    }
    Some(canonical)
}

/// The scalar kind a resolved observation must carry: the Boolean or integer
/// type of its `StructuralScalarFieldRead` result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedFieldKind {
    Boolean,
    Integer,
}

impl ObservedFieldKind {
    /// `None` for a result type no field read observes.
    pub fn of(scalar_type: ScalarType) -> Option<Self> {
        match scalar_type {
            ScalarType::Boolean => Some(Self::Boolean),
            ScalarType::Integer(_) => Some(Self::Integer),
            ScalarType::IeeeFloat(_) => None,
        }
    }
}

pub struct FunctionAnalysis {
    pub definitions: BTreeMap<ValueId, ValueDefinition>,
    pub dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
}

pub fn function_analysis(function: &PsiOptimizationFunction) -> FunctionAnalysis {
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

/// One structural position a canonical path resolves to: either a declared
/// structural type or one case's payload-field roster.
#[derive(Debug, Clone, Copy)]
pub enum Position<'a> {
    Type(StructuralTypeId),
    CasePayload(&'a StructuralCaseDeclaration),
}

impl<'a> Position<'a> {
    /// The position one canonical segment descends to, or `None` when the
    /// segment does not apply to this position's shape.
    pub fn descend(
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
    pub fn field_declaration(
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

/// What an establishment chain proves about the observed field: either the
/// scalar initializer operand the terminal producer stores — which the
/// caller folds or forwards — or the literal zero an
/// `EstablishZeroedStructural` fixes without naming any operand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstablishmentScalar {
    Initializer(OperationId, ValueId),
    Zeroed(OperationId),
}

/// The proven stored value of one field observation plus the witness it was
/// proven under: `Some(producer)` for an establishment basis — whether the
/// initializer folds to a literal or forwards as a substitution — `None` for
/// a declared singleton bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvenFieldValue {
    pub producer: Option<OperationId>,
    pub resolution: FieldValueResolution,
}

/// The `ValueId` the `EstablishRecord` producer stores into `field`, or
/// `None` when the producer node is absent, is not an `EstablishRecord` on
/// this place, or the initializer is not a scalar.
pub fn record_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<ValueId> {
    establishment_fields(function, producer, place)?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match &initializer.value {
            RecordFieldValue::Scalar { value, .. } => Some(*value),
            _ => None,
        })
}

/// The `ValueId` the `EstablishCase` producer stores into `field`, or
/// `None` when the producer node is absent, is not an `EstablishCase`
/// on this place, or the case field is absent.
pub fn case_field_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
) -> Option<ValueId> {
    establishment_fields(function, producer, place)?
        .iter()
        .find(|initializer| initializer.field == field)
        .and_then(|initializer| match initializer.value {
            terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(value),
            terminal_psi::RecordFieldValue::Structural(_) => None,
        })
}

/// The declared field the observation resolves to: the `path` descends from
/// the place's declared root type to a structural position, and `field`
/// selects one scalar declaration inside it. `None` when the root carries no
/// type, a segment does not apply — a `Field` name absent from a
/// `Record`/`Mixed` common-field or case-payload roster, a `FixedIndex` on a
/// non-array, a `Case` on a non-sum — or the path ends on a position with no
/// field roster.
pub fn observed_field_declaration<'a>(
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

/// Whether the resolved field declaration's scalar carrier matches the
/// observation kind — a Boolean read needs a `Boolean` field carrier, an
/// integer read needs an `Integer` carrier (plain or bounded). `Erased`,
/// structural, byte-sequence, and float fields never match.
pub fn matches_observed_kind(
    declaration: &StructuralFieldDeclaration,
    kind: ObservedFieldKind,
) -> bool {
    matches!(
        (kind, declaration.field_type.scalar_type()),
        (ObservedFieldKind::Boolean, Some(ScalarType::Boolean))
            | (ObservedFieldKind::Integer, Some(ScalarType::Integer(_)))
    )
}

/// The declared-bound basis: the field's declared `BoundedInteger` bound
/// closes over exactly one value, so every inhabitant holds it. `None` for a
/// non-singleton bound, a non-bounded carrier, or a Boolean observation.
pub fn bound_value(
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

/// The literal zero a zeroed establishment stores in `declaration`: `false`
/// for a Boolean carrier, all-zero bits for an integer carrier of either
/// sign. `None` when the observed kind does not match the declaration's
/// scalar — the caller already checked this, so the `None` is defensive.
pub fn zeroed_fold(
    declaration: &StructuralFieldDeclaration,
    kind: ObservedFieldKind,
) -> Option<FoldedFieldValue> {
    match (kind, declaration.field_type.scalar_type()) {
        (ObservedFieldKind::Boolean, Some(ScalarType::Boolean)) => {
            Some(FoldedFieldValue::Boolean(false))
        }
        (ObservedFieldKind::Integer, Some(ScalarType::Integer(integer_type))) => {
            Some(FoldedFieldValue::Integer(
                if integer_type.sign() == semantic_vocabulary::IntegerSign::Signed {
                    semantic_vocabulary::IntegerValue::Signed(0)
                } else {
                    semantic_vocabulary::IntegerValue::Unsigned(0)
                },
            ))
        }
        _ => None,
    }
}

/// The `FoldedFieldValue` `value` resolves to inside `function`: a
/// `BooleanConstant`/`IntegerConstant` defines it directly, or a block
/// parameter every incoming edge binds to the same argument resolves
/// transitively — the linear chains the front end emits route each
/// initializer through block parameters rather than spelling the constant at
/// the use site. A non-constant definition, a parameter bound to divergent
/// arguments on different edges, an edge that fails to bind it, or a cycle
/// in the chain yields `None`.
pub fn constant_definition(
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
pub fn matches_constant_kind(value: FoldedFieldValue, kind: ObservedFieldKind) -> bool {
    matches!(
        (value, kind),
        (FoldedFieldValue::Boolean(_), ObservedFieldKind::Boolean)
            | (FoldedFieldValue::Integer(_), ObservedFieldKind::Integer)
    )
}

/// The establishing operation a place's producer or uniform binding
/// resolves to, when it is one this family's proofs can draw on: `Record`
/// for an `EstablishRecord`, `Variant` for an `EstablishCase`
/// carrying its `result_case`, `Zeroed` for an `EstablishZeroedStructural`
/// whose all-zero storage selects each admissible sum's first declared
/// case, `None` for every other producer or place kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Establishment {
    None,
    Record(OperationId),
    Variant(OperationId, StructuralCaseId),
    Zeroed(OperationId),
}

/// The establishing producer `declaration`'s operation-result kind names,
/// or `Establishment::None` when the place is not an operation result or
/// its producer is not an `EstablishRecord`/`EstablishCase`/
/// `EstablishZeroedStructural` in `function`.
pub fn establishment_producer(
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Establishment {
    let StructuralPlaceKind::OperationResult { producer, .. } = declaration.kind else {
        return Establishment::None;
    };
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        match &node.operation {
            O::EstablishRecord {
                psi_operation,
                result,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return Establishment::Record(producer);
            }
            O::EstablishCase {
                psi_operation,
                result,
                result_case,
                ..
            } if *psi_operation == producer && result.place == declaration.id => {
                return Establishment::Variant(producer, *result_case);
            }
            O::EstablishZeroedStructural {
                psi_operation,
                result,
            } if *psi_operation == producer && result.place == declaration.id => {
                return Establishment::Zeroed(producer);
            }
            _ => {}
        }
    }
    Establishment::None
}

pub fn dominators(
    entry: BlockId,
    block_ids: impl Iterator<Item = BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let all = block_ids.collect::<BTreeSet<_>>();
    let mut result = all
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                [entry].into_iter().collect()
            } else {
                all.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();
    loop {
        let mut changed = false;
        for block in all.iter().copied().filter(|block| *block != entry) {
            let incoming = predecessors.get(&block).expect("all blocks indexed");
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

/// The structural type one path segment descends to under `current`, or
/// `None` when the segment does not apply to `current`'s shape or names a
/// field whose declared type is not structural.
pub fn descended_type(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    segment: &StructuralPathSegment,
) -> Option<StructuralTypeId> {
    let shape = &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == current)?
        .shape;
    match segment {
        StructuralPathSegment::Case(_) | StructuralPathSegment::FixedByteRange { .. } => None,
        StructuralPathSegment::Field(identity) => {
            let fields = match shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => return None,
            };
            match fields
                .iter()
                .find(|field| field.identity == *identity)?
                .field_type
            {
                StructuralFieldType::Structural(next) => Some(next),
                _ => None,
            }
        }
        StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
            match shape {
                StructuralTypeShape::FixedArray { element, .. } => Some(*element),
                _ => None,
            }
        }
        StructuralPathSegment::Referent => match shape {
            StructuralTypeShape::Reference { referent, .. } => Some(*referent),
            _ => None,
        },
    }
}

/// Split one comparison's operands into the dispatch parameter (one of the
/// block's own scalar parameters, on exactly one side) and the bound literal
/// (an in-block `IntegerConstant`, on the other). The operand's declared
/// integer type — shared by both sides under checked semantics — is the type
/// the comparison evaluates under.
pub fn comparison_operands(
    block: &OptimizationBlock,
    prefix: &[OptimizationNode],
    ordering: IntegerComparisonOrdering,
    left: ValueId,
    right: ValueId,
) -> Option<(ValueId, DispatchCondition)> {
    let left_is_parameter = block.parameters.iter().any(|p| p.value == left);
    let right_is_parameter = block.parameters.iter().any(|p| p.value == right);
    let (parameter, bound, parameter_on_left) = match (left_is_parameter, right_is_parameter) {
        (true, false) => (left, right, true),
        (false, true) => (right, left, false),
        _ => return None,
    };
    let (bound_value, integer_type) = prefix.iter().find_map(|node| {
        let O::IntegerConstant {
            result,
            scalar_type,
            value,
            ..
        } = &node.operation
        else {
            return None;
        };
        if *result != bound {
            return None;
        }
        let ScalarType::Integer(integer_type) = scalar_type else {
            return None;
        };
        Some((*value, *integer_type))
    })?;
    // This optional specialization records one homogeneous bound carrier.
    // Retain mixed comparisons for the general exact-value execution path.
    if !block.parameters.iter().any(|declaration| {
        declaration.value == parameter
            && declaration.scalar_type == ScalarType::Integer(integer_type)
    }) {
        return None;
    }
    Some((
        parameter,
        DispatchCondition::IntegerComparison {
            ordering,
            bound: bound_value,
            parameter_on_left,
            integer_type,
        },
    ))
}

/// The integer-comparison dispatch shape, or `None` when `prefix` is not a
/// pure scalar computation of `condition` as `parameter CMP literal`: every
/// prefix node must be a scalar constant, except exactly one integer
/// comparison producing `condition` whose operands pair one of the block's
/// own scalar parameters with an in-block `IntegerConstant`. Returns the
/// parameter and the condition descriptor the bound argument resolves
/// against.
pub fn integer_comparison_condition(
    block: &OptimizationBlock,
    prefix: &[OptimizationNode],
    condition: ValueId,
) -> Option<(ValueId, DispatchCondition)> {
    let mut resolved = None;
    for node in prefix {
        let (ordering, result, left, right) = match &node.operation {
            O::IntegerConstant { .. } | O::BooleanConstant { .. } => continue,
            O::IntegerEqual {
                result,
                left,
                right,
                ..
            } => (IntegerComparisonOrdering::Equal, *result, *left, *right),
            O::IntegerLessThan {
                result,
                left,
                right,
                ..
            } => (IntegerComparisonOrdering::LessThan, *result, *left, *right),
            O::IntegerLessOrEqual {
                result,
                left,
                right,
                ..
            } => (
                IntegerComparisonOrdering::LessOrEqual,
                *result,
                *left,
                *right,
            ),
            _ => return None,
        };
        if result != condition || resolved.is_some() {
            return None;
        }
        resolved = Some(comparison_operands(block, prefix, ordering, left, right)?);
    }
    resolved
}

/// The comparison an in-block dispatch condition computes against its bound
/// operand. Mirrors the three integer comparison operations lowered literal
/// guards and ordering guards produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerComparisonOrdering {
    Equal,
    LessThan,
    LessOrEqual,
}

/// How a dispatch `Conditional` resolves one bound argument's proven constant
/// into the arm that edge takes. Two admitted shapes share the same fused
/// traversal — "this edge, then the resolved arm" — differing only in how the
/// condition reads the block's own scalar parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchCondition {
    /// The condition is the block's own scalar parameter directly — a
    /// Boolean state argument resolves on a proven `ScalarConstantValue::Boolean`.
    DirectParameter,
    /// The condition is produced in-block by `parameter CMP bound`, where
    /// `bound` is the literal an in-block `IntegerConstant` carries — an
    /// integer state argument resolves on a proven `ScalarConstantValue::Integer`
    /// evaluated under the operand type's own ordering via
    /// `IntegerType::compare`.
    IntegerComparison {
        ordering: IntegerComparisonOrdering,
        bound: IntegerValue,
        parameter_on_left: bool,
        integer_type: IntegerType,
    },
}

impl DispatchCondition {
    /// The Boolean the dispatch condition takes when the bound argument is
    /// `constant`, or `None` when the constant is not the kind this condition
    /// resolves — a Boolean argument can never answer an integer comparison
    /// and vice versa.
    pub fn resolve(&self, constant: &ScalarConstantValue) -> Option<bool> {
        match (self, constant) {
            (Self::DirectParameter, ScalarConstantValue::Boolean(value)) => Some(*value),
            (
                Self::IntegerComparison {
                    ordering,
                    bound,
                    parameter_on_left,
                    integer_type,
                },
                ScalarConstantValue::Integer(argument),
            ) => {
                let (left, right) = if *parameter_on_left {
                    (*argument, *bound)
                } else {
                    (*bound, *argument)
                };
                let order = integer_type.compare(left, right)?;
                Some(match ordering {
                    IntegerComparisonOrdering::Equal => order == Ordering::Equal,
                    IntegerComparisonOrdering::LessThan => order == Ordering::Less,
                    IntegerComparisonOrdering::LessOrEqual => order != Ordering::Greater,
                })
            }
            _ => None,
        }
    }
}

/// Blocks still reachable after a conditional fold keeps only the
/// selected edge of `source`.
pub fn reachable_blocks_after_conditional_fold(
    function: &PsiOptimizationFunction,
    source: BlockId,
    selected_edge: EdgeId,
) -> Option<BTreeSet<BlockId>> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![function.entry];
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        let block = function.blocks.iter().find(|block| block.id == block_id)?;
        for edge in block.nodes.iter().flat_map(|node| &node.successors) {
            if block_id != source || edge.psi_edge == selected_edge {
                pending.push(edge.target);
            }
        }
    }
    Some(reachable)
}

/// The definition site of `value` across the function's parameters,
/// block parameters and node definitions.
pub fn scalar_value_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ValueDefinition> {
    function
        .parameters
        .iter()
        .chain(function.blocks.iter().flat_map(|block| &block.parameters))
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .flat_map(|node| &node.definitions),
        )
        .copied()
        .find(|definition| definition.value == value)
}
