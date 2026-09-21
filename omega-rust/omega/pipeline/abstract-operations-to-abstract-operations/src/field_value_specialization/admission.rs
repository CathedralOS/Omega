//! Optimizer module role: admission leaf. Proven field-value predicates the
//! producer enumeration and the independent validator share.
//!
//! Proposal decides *which* field observations enter a plan; validation must
//! never rerun that decision through the producer's own plan — a matcher
//! cannot attest to itself. Both sides share only these predicates: the
//! place's declaration/type/proof evidence and the per-node admissibility
//! that resolves one field observation's proven stored value.

use super::{
    FoldedFieldValue, NodeLocation, O, OperationId, PlaceId, PsiOptimizationFunction,
    PsiOptimizationUnit, ResolvedFieldValue, ScalarType, StructuralPlaceKind,
};
use crate::representation_specialization::admission::declared_structural_type;
use optimization_unit::OptimizationNode;
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, StructuralCaseId, StructuralFieldId, StructuralTypeId,
    ValueId,
};
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

/// The scalar kind a folded observation must carry: `Boolean` for a
/// `BooleanStructuralField` result, `Integer` for an `IntegerStructuralField`
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ObservedFieldKind {
    Boolean,
    Integer,
}

/// The proven stored value of one field observation plus the witness it was
/// proven under: `Some(producer)` for an establishment basis, `None` for a
/// declared singleton bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ProvenFieldValue {
    pub(super) producer: Option<OperationId>,
    pub(super) value: FoldedFieldValue,
}

/// The admissibility of one node under `evidence`: a `BooleanStructuralField`
/// or `IntegerStructuralField` observing the evidence's place whose stored
/// value the unit proves. `None` for every other node.
pub(super) fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    block: BlockId,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<ResolvedFieldValue> {
    let (psi_operation, result, source, path, field, kind) = match &node.operation {
        O::BooleanStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (
            psi_operation,
            *result,
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
    let proven = proven_field_value(unit, evidence, function, path, *field, kind)?;
    Some(ResolvedFieldValue {
        site: NodeLocation {
            machine: function.machine,
            block,
            node: u32::try_from(node_index).ok()?,
        },
        psi_operation: *psi_operation,
        result,
        source: evidence.declaration.id,
        path: path.clone(),
        field: *field,
        producer: proven.producer,
        value: proven.value,
    })
}

/// The stored value the unit proves for `field` at `path` under `place`'s
/// evidence, or `None` when no basis applies. The establishment basis is
/// tried first — an `EstablishRecord` producer at an empty path or an
/// `EstablishScalarCase` producer whose `result_case` matches a lone `Case`
/// path, each proving the value when the field's initializer scalar resolves
/// through block-parameter bindings to a same-function constant — then the
/// declared `BoundedInteger` singleton
/// bound, which proves the value at any resolvable path independently of
/// producer.
pub(super) fn proven_field_value(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
    kind: ObservedFieldKind,
) -> Option<ProvenFieldValue> {
    let declaration = observed_field_declaration(unit, evidence, path, field)?;
    if !matches_observed_kind(declaration, kind) {
        return None;
    }
    if let Some(proven) = establishment_value(evidence, function, path, field, kind) {
        return Some(proven);
    }
    bound_value(declaration, kind)
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
fn shape<'a>(
    unit: &'a PsiOptimizationUnit,
    type_id: StructuralTypeId,
) -> Option<&'a StructuralTypeShape> {
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
    match (kind, declaration.field_type.scalar_type()) {
        (ObservedFieldKind::Boolean, Some(ScalarType::Boolean)) => true,
        (ObservedFieldKind::Integer, Some(ScalarType::Integer(_))) => true,
        _ => false,
    }
}

/// The establishment basis: the place's producer proves the field's stored
/// scalar and that scalar is a same-function constant. `None` when the
/// producer does not establish the observed position or the initializer is
/// not a constant.
fn establishment_value(
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    path: &[CanonicalStructuralPathSegment],
    field: StructuralFieldId,
    kind: ObservedFieldKind,
) -> Option<ProvenFieldValue> {
    match (evidence.root_producer, path) {
        (RootProducer::Record(producer), []) => {
            // An `EstablishRecord` proves its declaration-ordered field
            // initializers: the observed field's scalar initializer fixes
            // the stored value permanently.
            let value = record_initializer(function, producer, evidence, field)
                .and_then(|value| constant_definition(function, value))?;
            matches_constant_kind(value, kind).then_some(ProvenFieldValue {
                producer: Some(producer),
                value,
            })
        }
        (
            RootProducer::Variant(producer, result_case),
            [CanonicalStructuralPathSegment::Case(observed_case)],
        ) if result_case == *observed_case => {
            // An `EstablishScalarCase` proves its scalar case-field
            // initializers for exactly the case it establishes.
            let value = case_field_initializer(function, producer, evidence, field)
                .and_then(|value| constant_definition(function, value))?;
            matches_constant_kind(value, kind).then_some(ProvenFieldValue {
                producer: Some(producer),
                value,
            })
        }
        _ => None,
    }
}

/// The `ValueId` the `EstablishRecord` producer stores into `field`, or
/// `None` when the producer node is absent, is not an `EstablishRecord` on
/// this place, or the initializer is not a scalar.
fn record_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    evidence: &FieldEvidence<'_>,
    field: StructuralFieldId,
) -> Option<ValueId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .find_map(|node| match &node.operation {
            O::EstablishRecord {
                psi_operation,
                result,
                fields,
            } if *psi_operation == producer && result.place == evidence.declaration.id => fields
                .iter()
                .find(|initializer| initializer.field == field)
                .and_then(|initializer| match &initializer.value {
                    RecordFieldValue::Scalar { value, .. } => Some(*value),
                    _ => None,
                }),
            _ => None,
        })
}

/// The `ValueId` the `EstablishScalarCase` producer stores into `field`, or
/// `None` when the producer node is absent, is not an `EstablishScalarCase`
/// on this place, or the case field is absent.
fn case_field_initializer(
    function: &PsiOptimizationFunction,
    producer: OperationId,
    evidence: &FieldEvidence<'_>,
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
            } if *psi_operation == producer && result.place == evidence.declaration.id => fields
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
        value: FoldedFieldValue::Integer(bound.minimum()),
    })
}
