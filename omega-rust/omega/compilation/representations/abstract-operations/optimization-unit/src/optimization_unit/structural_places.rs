//! Structural-place evidence queries shared by specialization admission and
//! replay: a rostered place's declared structural type and establishing
//! producer.

use abstract_operations::{AbstractOperation as O, StructuralCopyPath};
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, EdgeId, IntegerType, IntegerValue, OperationId,
    PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId, ValueId,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    RecordFieldValue, StructuralAccess, StructuralArgument, StructuralCaseDeclaration,
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalIdentityIndex,
};

use super::{
    FieldValueResolution, FieldValueRow, FoldedFieldValue, ForwardedFieldValue, NodeLocation,
    OptimizationBlock, OptimizationNode, PsiOptimizationFunction, PsiOptimizationUnit,
    ScalarConstantValue, ValueDefinition,
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
    let declaration = function.structural_place(place)?;
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
    for node in function.block_nodes() {
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
            .structural_parameter(declaration.id)
            .map(|parameter| parameter.structural_type),
        StructuralPlaceKind::BlockParameter { .. } => function
            .block_structural_parameters()
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
pub(crate) fn establishment_fields(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
) -> Option<&[terminal_psi::RecordFieldInitializer]> {
    function
        .block_nodes()
        .find_map(|node| match &node.operation {
            abstract_operations::establish_record_or_case_pattern!(
                psi_operation,
                result,
                fields,
                ..
            ) if *psi_operation == producer && result.place == place => Some(fields.as_slice()),
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
pub(crate) fn descended_field(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    identity: TerminalIdentityIndex,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    descended_field_matching(unit, current, |field| field.identity == identity)
}

/// The same common-field descent by verifier field handle, for the
/// canonical steps a spliced copy projection carries.
pub(crate) fn descended_field_id(
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
pub(crate) fn descended_field_matching(
    unit: &PsiOptimizationUnit,
    current: StructuralTypeId,
    matching: impl Fn(&StructuralFieldDeclaration) -> bool,
) -> Option<(StructuralFieldId, StructuralTypeId)> {
    let fields = match shape(unit, current)? {
        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
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
pub(crate) fn canonicalize_ordinary_path(
    unit: &PsiOptimizationUnit,
    mut current: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    let mut canonical = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(identity) => {
                let fields = match shape(unit, current)? {
                    terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                    _ => return None,
                };
                let field = terminal_psi::live_field_by_identity(fields, *identity)?;
                let next = field.field_type.as_structural()?;
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
            | StructuralPathSegment::RepresentationView { .. }
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
                    terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
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
                terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
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
pub(crate) enum EstablishmentScalar {
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
pub(crate) fn record_initializer(
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
pub(crate) fn case_field_initializer(
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
pub(crate) fn matches_observed_kind(
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
pub(crate) fn zeroed_fold(
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
pub(crate) fn constant_definition(
    function: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<FoldedFieldValue> {
    let mut current = value;
    let mut visiting = std::collections::BTreeSet::new();
    loop {
        if !visiting.insert(current) {
            return None;
        }
        if let Some(node) = function.block_nodes().find(|node| {
            node.definitions
                .iter()
                .any(|definition| definition.value == current)
        }) {
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
            .block_edges()
            .filter(|edge| edge.target == owner.id)
        {
            let binding = edge.binding(current)?;
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
pub(crate) fn matches_constant_kind(value: FoldedFieldValue, kind: ObservedFieldKind) -> bool {
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
pub type Establishment = RootProducer;

/// The establishing producer `declaration`'s operation-result kind names,
/// or `Establishment::None` when the place is not an operation result or
/// its producer is not an `EstablishRecord`/`EstablishCase`/
/// `EstablishZeroedStructural` in `function`.
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
pub(crate) fn descended_type(
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
        StructuralPathSegment::RepresentationView {
            structural_type,
            stored_scalar,
        } => terminal_semantics::representation_view_declaration(
            unit.structural_types.iter(),
            *structural_type,
            *stored_scalar,
        )
        .map(|declaration| declaration.id),
        StructuralPathSegment::Field(identity) => {
            let fields = match shape {
                terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                _ => return None,
            };
            match terminal_psi::field_by_identity(fields, *identity)?.field_type {
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
pub(crate) fn comparison_operands(
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
        let integer_type = scalar_type.as_integer()?;
        Some((*value, integer_type))
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
        let block = function.block(block_id)?;
        for edge in block.successors() {
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
        .chain(function.block_parameters())
        .chain(function.node_definitions())
        .copied()
        .find(|definition| definition.value == value)
}

/// The case roster of `type_id`'s declared shape, or `None` when the type
/// is absent or its shape is not `Sum`/`Mixed`.
pub(crate) fn roster_cases(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<&[StructuralCaseDeclaration]> {
    match &unit
        .structural_types
        .as_slice()
        .iter()
        .find(|entry| entry.id == type_id)?
        .shape
    {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            Some(cases.as_slice())
        }
        _ => None,
    }
}

/// The first declared case of `type_id`'s roster — the case a zeroed tag's
/// ordinal-zero bits select — or `None` when the shape is not a
/// `Sum`/`Mixed` or declares no case at all.
pub(crate) fn roster_first_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<StructuralCaseId> {
    let cases = roster_cases(unit, type_id)?;
    cases.first().map(|case| case.id)
}

/// The one case of `type_id`'s closed roster, or `None` when its shape is
/// not a `Sum`/`Mixed` or names more than one case.
pub(crate) fn roster_sole_case(
    unit: &PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<StructuralCaseId> {
    let cases = roster_cases(unit, type_id)?;
    let [sole] = cases else {
        return None;
    };
    Some(sole.id)
}

/// The one case of the closed roster the `path` resolves to under the
/// place's declared structural type, or `None` when the path fails to
/// descend — a `Field` name absent from a `Record`/`Mixed` common-field
/// roster, a `FixedIndex` on a non-array, a `Referent` crossing on a
/// non-reference — or when the resolved end type is not a `Sum`/`Mixed`
/// roster of exactly one case.
pub fn sole_case_at_path(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    resolved_sole_case(unit, declared_structural_type(function, declaration)?, path)
}

/// The one case of a declared closed roster, or `None` when the place's
/// declared type is not a sum shape or names more than one case.
pub(crate) fn sole_case(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
) -> Option<StructuralCaseId> {
    sole_case_at_path(unit, function, declaration, &[])
}

/// The sole case of the closed roster at `path`'s end under `root`, or
/// `None` when the path fails to descend or the end type is not a sole-case
/// `Sum`/`Mixed` roster.
pub fn resolved_sole_case(
    unit: &PsiOptimizationUnit,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralCaseId> {
    let mut current = root;
    for segment in path {
        current = descended_type(unit, current, segment)?;
    }
    roster_sole_case(unit, current)
}

/// A field descent's shared child step: the producer's structural child,
/// its rostered declaration, the declared-type equality check, and the
/// cycle guard; returns the child place and its establishment for the next
/// step.
pub(crate) fn descended_field_child(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    place: PlaceId,
    field: StructuralFieldId,
    next: StructuralTypeId,
    visiting: &mut BTreeSet<PlaceId>,
) -> Option<(PlaceId, Establishment)> {
    let child = structural_child(function, producer, place, field)?;
    let declaration = function.structural_place(child)?;
    if declared_structural_type(function, declaration) != Some(next) || !visiting.insert(child) {
        return None;
    }
    Some((child, root_producer(function, declaration)))
}

/// The `StructuralLeafCopy` that produces `place` — its source place and
/// spelled projection — or `None` when `place` is not that copy's operation
/// result.
pub(crate) fn leaf_copy_source<'a>(
    function: &'a PsiOptimizationFunction,
    place: PlaceId,
) -> Option<(PlaceId, &'a StructuralCopyPath)> {
    let StructuralPlaceKind::OperationResult { producer, .. } =
        function.structural_place(place)?.kind
    else {
        return None;
    };
    function
        .block_nodes()
        .find_map(|node| match &node.operation {
            O::StructuralLeafCopy {
                psi_operation,
                result,
                source,
                path,
            } if *psi_operation == producer && result.place == place => {
                // A nominal whole-value adaptation is not an alias of its
                // source. Rebasing a later field observation would cross the
                // opaque quotient boundary and discard its call license.
                if matches!(path, StructuralCopyPath::Ordinary(path) if path.is_empty()) {
                    let source_type =
                        declared_structural_type(function, function.structural_place(*source)?);
                    if source_type != Some(result.structural_type) {
                        return None;
                    }
                }
                Some((*source, path))
            }
            _ => None,
        })
}

/// One unresolved path segment inside `established_case_at`: either a
/// name-spelled segment of the observation's own path or a verifier-handle
/// `CanonicalStructuralPathSegment` a `StructuralLeafCopy` spliced in from
/// its projection — a case-qualified path directly, an ordinary path after
/// its field identities resolve to roster handles. The observation's own
/// path never spells a `Case` step, so a case-qualified descent appears
/// only under a copy crossing.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PathStep<'a> {
    Ordinary(&'a StructuralPathSegment),
    Canonical(CanonicalStructuralPathSegment),
}
impl<'a> Position<'a> {
    pub(crate) fn descend_step(
        self,
        unit: &'a PsiOptimizationUnit,
        step: PathStep<'_>,
    ) -> Option<Position<'a>> {
        match (self, step) {
            (Self::Type(current), PathStep::Ordinary(segment)) => match segment {
                StructuralPathSegment::Field(identity) => {
                    descended_field(unit, current, *identity).map(|(_, next)| Self::Type(next))
                }
                StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. } => match shape(unit, current)? {
                    StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                    _ => None,
                },
                StructuralPathSegment::Referent
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::RepresentationView { .. }
                | StructuralPathSegment::FixedByteRange { .. } => None,
            },
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
            ) => descended_field_id(unit, current, identity).map(|(_, next)| Self::Type(next)),
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::FixedIndex(_)),
            ) => match shape(unit, current)? {
                StructuralTypeShape::FixedArray { element, .. } => Some(Self::Type(*element)),
                _ => None,
            },
            (
                Self::Type(current),
                PathStep::Canonical(CanonicalStructuralPathSegment::Case(identity)),
            ) => {
                let cases = match shape(unit, current)? {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return None,
                };
                cases
                    .iter()
                    .find(|case| case.id == identity)
                    .map(Self::CasePayload)
            }
            (
                Self::CasePayload(case),
                PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
            ) => match case
                .fields
                .iter()
                .find(|field| field.id == identity)?
                .field_type
            {
                StructuralFieldType::Structural(next) => Some(Self::Type(next)),
                _ => None,
            },
            (Self::CasePayload(_), _) => None,
        }
    }
}

/// The case `place`'s root is proven to hold, plus its establishment
/// producer when the proof is one `EstablishCase` operation result —
/// whether the place is itself that result or a block parameter every
/// incoming edge binds to it whole.
pub fn proof_basis(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    bound_place: &dyn Fn(&PsiOptimizationFunction, PlaceId) -> Option<PlaceId>,
) -> Option<(StructuralCaseId, Option<OperationId>)> {
    if let Some((producer, case)) =
        established_case_at(unit, function, declaration, &[], bound_place)
    {
        return Some((case, Some(producer)));
    }
    sole_case(unit, function, declaration).map(|case| (case, None))
}
/// The case an establishing producer proves at `path`'s resolved position
/// under `declaration`'s place, witnessed by the `EstablishCase` or
/// `EstablishZeroedStructural` operation that fixes it. `None` when any
/// step is not established: a `Field` segment crosses only into an
/// `EstablishRecord` field stored as an owned, complete structural child
/// whose declared type is exactly the field's declared carrier, a `Case`
/// then `Field` pair descends an `EstablishCase` into the payload child it
/// stores whole for exactly the case it establishes, and a block parameter
/// — which holds no producer of its own — crosses only into the one place
/// every incoming edge binds it to whole when nothing rewrites or mutably
/// re-lends it. A `StructuralLeafCopy` result crosses into its source with
/// the copy's projection — canonical steps directly, ordinary field
/// identities resolved to their roster handles — prefixed to the path
/// still to resolve. `FixedIndex`, `RuntimeIndex`, `FixedByteRange`, and
/// `Referent` segments never cross a record or variant establishment: an
/// `EstablishScalarArray` element is not a placed child. A zeroed
/// establishment crosses every `Field`, `Case`, and index segment inside
/// its own storage instead — the verifier admits
/// `EstablishZeroedStructural` only on zero-constructible shapes, so every
/// sum the declared path resolves holds its first declared case.
pub fn established_case_at(
    unit: &PsiOptimizationUnit,
    function: &PsiOptimizationFunction,
    declaration: &StructuralPlaceDeclaration,
    path: &[StructuralPathSegment],
    bound_place: &dyn Fn(&PsiOptimizationFunction, PlaceId) -> Option<PlaceId>,
) -> Option<(OperationId, StructuralCaseId)> {
    let mut place = declaration.id;
    let mut producer = root_producer(function, declaration);
    let mut declared = declared_structural_type(function, declaration);
    // Owned because a leaf copy prepends its own canonical projection —
    // case-qualified steps directly, ordinary field identities resolved to
    // roster handles — to the path still to resolve beneath its source.
    let mut segments: Vec<PathStep> = path.iter().map(PathStep::Ordinary).collect();
    let mut visiting = BTreeSet::from([place]);
    loop {
        match (producer, segments.as_slice()) {
            (Establishment::Variant(producer, case), []) => {
                // An `EstablishCase` proves the case of the position
                // its result occupies permanently.
                return Some((producer, case));
            }
            (Establishment::Zeroed(operation), _) => {
                // Zeroed storage selects each admissible sum's first
                // declared case — the tag's all-zero bits name case ordinal
                // zero — at every position inside the place's own storage.
                // The remaining path descends the declared type alone:
                // `Field`, `Case`, and both index segments stay inside the
                // zeroed storage — a `Case` step enters a payload namespace
                // whose fields read the same zeroed bytes — while
                // `Referent` and `FixedByteRange` leave it and never
                // resolve under a zero-admissible shape anyway.
                let mut position = Position::Type(declared?);
                for step in &segments {
                    position = position.descend_step(unit, *step)?;
                }
                let Position::Type(terminal) = position else {
                    return None;
                };
                return roster_first_case(unit, terminal).map(|case| (operation, case));
            }
            (
                Establishment::Variant(operation, result_case),
                [
                    PathStep::Canonical(CanonicalStructuralPathSegment::Case(observed_case)),
                    PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)),
                    ..,
                ],
            ) if result_case == *observed_case => {
                // A `Case` then `Field` pair descends into the payload
                // child an `EstablishCase` stores whole — the
                // structural-case establishment. The same custody as a
                // record child applies: the payload carrier must be
                // exactly the child place's declared type, reached as one
                // owned, complete structural argument. A different
                // observed case never crosses: its payload overlays
                // storage this establishment did not write.
                let identity = *identity;
                let Position::Type(next) = Position::Type(declared?)
                    .descend_step(unit, segments[0])?
                    .descend_step(unit, segments[1])?
                else {
                    return None;
                };
                let (child, child_producer) = descended_field_child(
                    function,
                    operation,
                    place,
                    identity,
                    next,
                    &mut visiting,
                )?;
                place = child;
                producer = child_producer;
                declared = Some(next);
                segments.drain(..2);
            }
            (Establishment::Record(operation), [step, ..]) => {
                // A `Field` segment descends into the nested carrier the
                // record stores whole, whether it spells a field identity —
                // the observation's own path — or a verifier field handle —
                // a spliced copy projection: the declared carrier must be
                // exactly the child place's declared type, and the child
                // must reach this producer's initializer as one owned,
                // complete structural argument. A pathed or borrowed
                // argument does not carry the child's own establishment.
                // Index, referent, and byte-range segments never cross —
                // an `EstablishScalarArray` element is not a placed child.
                let (field, next) = match step {
                    PathStep::Ordinary(StructuralPathSegment::Field(identity)) => {
                        descended_field(unit, declared?, *identity)?
                    }
                    PathStep::Canonical(CanonicalStructuralPathSegment::Field(identity)) => {
                        descended_field_id(unit, declared?, *identity)?
                    }
                    _ => return None,
                };
                let (child, child_producer) =
                    descended_field_child(function, operation, place, field, next, &mut visiting)?;
                place = child;
                producer = child_producer;
                declared = Some(next);
                segments.drain(..1);
            }
            (Establishment::None, _) => {
                // A leaf copy — the case-payload place a `Case { payload }`
                // arm binding mints, or an ordinary projection's fresh leaf
                // — holds exactly its source's contents at the copied
                // projection: the copy is fresh storage of an
                // `Unrestricted` or owned non-reference leaf, and the
                // source resolves only through establishments and uniform
                // bindings nothing rewrites. The copy's canonical
                // projection therefore prefixes the path still to resolve
                // beneath its source, and the source's own chain decides
                // the proof; a source that establishes a different case
                // declines at the `Case` step. Ordinary projections spell
                // field identities, so the crossing first resolves each
                // name to its roster's verifier handle; a name absent from
                // the reached roster, a non-structural field carrier, and
                // every segment with no canonical step leave the copy
                // uncrossed.
                if let Some((source, spelled)) = leaf_copy_source(function, place) {
                    let source_declaration = function.structural_place(source)?;
                    let source_type = declared_structural_type(function, source_declaration)?;
                    let copied = match spelled {
                        StructuralCopyPath::Case(path) => path.clone(),
                        StructuralCopyPath::Ordinary(path) => {
                            canonicalize_ordinary_path(unit, source_type, path)?
                        }
                    };
                    let mut position = Position::Type(source_type);
                    for segment in &copied {
                        position = position.descend_step(unit, PathStep::Canonical(*segment))?;
                    }
                    if !matches!(position, Position::Type(landed) if Some(landed) == declared) {
                        return None;
                    }
                    if !visiting.insert(source) {
                        return None;
                    }
                    place = source;
                    producer = root_producer(function, source_declaration);
                    declared = Some(source_type);
                    segments.splice(..0, copied.iter().copied().map(PathStep::Canonical));
                    continue;
                }
                // The position's place is not itself established, but a
                // block parameter arrives carrying exactly the place its
                // uniform binding names — the bound place's own
                // establishment is the parameter's.
                let bound = bound_place(function, place)?;
                let bound_declaration = function.structural_place(bound)?;
                let bound_type = declared_structural_type(function, bound_declaration);
                if declared.is_some() && bound_type != declared {
                    return None;
                }
                if !visiting.insert(bound) {
                    return None;
                }
                place = bound;
                producer = root_producer(function, bound_declaration);
                declared = bound_type;
            }
            _ => return None,
        }
    }
}
/// The one place `place` is bound to by every incoming edge when `place` is
/// a structural block parameter, or `None` when it is not a block
/// parameter, when any incoming edge fails to bind it or binds a different
/// place, when any bound argument is pathed or carries write authority, or
/// when the parameter's contents may still be rewritten or mutably re-lent
/// inside `function`. A uniform whole `Owned`/`SharedBorrow` argument means
/// the parameter's contents are exactly the bound place's for the block's
/// lifetime — an owned binding transfers the storage itself, and a shared
/// loan freezes the bound place's contents for the borrow — so the bound
/// place's establishment is the parameter's own.
pub fn bound_place(
    function: &PsiOptimizationFunction,
    place: PlaceId,
    rewrites: &dyn Fn(&O, PlaceId) -> bool,
) -> Option<PlaceId> {
    let StructuralPlaceKind::BlockParameter { block, .. } = function.structural_place(place)?.kind
    else {
        return None;
    };
    if place_is_rewritten(function, place, rewrites) {
        return None;
    }
    let mut bound = None;
    for edge in function.block_edges().filter(|edge| edge.target == block) {
        let binding = edge.structural_binding(place)?;
        let argument = &binding.argument;
        if !argument.path.is_empty()
            || !matches!(
                argument.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow
            )
        {
            return None;
        }
        match bound {
            None => bound = Some(argument.place),
            Some(seen) if seen != argument.place => return None,
            _ => {}
        }
    }
    bound
}
/// Whether `place`'s contents can change inside `function` after its
/// binding: a store or vacating move naming it, a mutable or pathed
/// structural argument re-lending it, or an atomic event joining its
/// residence each break the uniform-binding proof.
fn place_is_rewritten(
    function: &PsiOptimizationFunction,
    place: PlaceId,
    rewrites: &dyn Fn(&O, PlaceId) -> bool,
) -> bool {
    function.blocks.iter().any(|block| {
        block.nodes.iter().any(|node| {
            rewrites(&node.operation, place)
                || node.successors.iter().any(|edge| {
                    edge.structural_bindings
                        .iter()
                        .any(|binding| argument_rewrites_place(&binding.argument, place))
                })
        })
    })
}
/// Whether `argument` gives `place`'s contents away: a pathed argument
/// projects or vacates only a subtree, while a whole `MutableBorrow` or
/// `WriteOnlyBorrow` argument lends write authority over the whole place.
/// Whole `Owned`/`SharedBorrow` arguments preserve the observed contents —
/// a move transfers the storage itself and a shared loan freezes it.
pub fn argument_rewrites_place(argument: &StructuralArgument, place: PlaceId) -> bool {
    argument.place == place
        && (!argument.path.is_empty()
            || matches!(
                argument.access,
                terminal_psi::exclusive_borrow_pair_pattern!()
            ))
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
    bound_place: &dyn Fn(&PsiOptimizationFunction, PlaceId) -> Option<PlaceId>,
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
                let (child, child_producer) = descended_field_child(
                    function,
                    operation,
                    place,
                    identity,
                    next,
                    &mut visiting,
                )?;
                place = child;
                producer = child_producer;
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
                let (child, child_producer) = descended_field_child(
                    function,
                    operation,
                    place,
                    identity,
                    next,
                    &mut visiting,
                )?;
                place = child;
                producer = child_producer;
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
                if let Some((source, spelled)) = leaf_copy_source(function, place) {
                    let declaration = function.structural_place(source)?;
                    let source_type = declared_structural_type(function, declaration)?;
                    let copied = match spelled {
                        StructuralCopyPath::Case(path) => path.clone(),
                        StructuralCopyPath::Ordinary(path) => {
                            canonicalize_ordinary_path(unit, source_type, path)?
                        }
                    };
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
                let declaration = function.structural_place(bound)?;
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
#[allow(clippy::too_many_arguments)]
pub(crate) fn proven_field_value(
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
    bound_place: &dyn Fn(&PsiOptimizationFunction, PlaceId) -> Option<PlaceId>,
    forwarded_resolution: &dyn Fn(
        &PsiOptimizationFunction,
        &FunctionAnalysis,
        &OptimizationBlock,
        usize,
        &OptimizationNode,
        ValueId,
        ScalarType,
        ValueId,
    ) -> Option<ForwardedFieldValue>,
) -> Option<ProvenFieldValue> {
    let declaration = observed_field_declaration(unit, evidence, path, field)?;
    if !matches_observed_kind(declaration, kind) {
        return None;
    }
    let establishment = establishment_scalar(unit, evidence, function, path, field, bound_place);
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

/// The admissibility of one node under `evidence`: a Boolean or integer
/// `StructuralScalarFieldRead` observing the evidence's place whose stored
/// value the unit proves. `None` for every other node. The bound-place and
/// forward-resolution policies are the caller's own: the admission and
/// replay sides each supply theirs, and the node gate itself is the shared
/// derivation.
pub fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    analysis: &FunctionAnalysis,
    block: &OptimizationBlock,
    node_index: usize,
    node: &OptimizationNode,
    bound_place: &dyn Fn(&PsiOptimizationFunction, PlaceId) -> Option<PlaceId>,
    forwarded_resolution: &dyn Fn(
        &PsiOptimizationFunction,
        &FunctionAnalysis,
        &OptimizationBlock,
        usize,
        &OptimizationNode,
        ValueId,
        ScalarType,
        ValueId,
    ) -> Option<ForwardedFieldValue>,
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
        bound_place,
        forwarded_resolution,
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
