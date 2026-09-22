//! Independent field-value specialization replay mechanics.
//!
//! Validation never trusts the candidate's field rows: it re-derives the
//! claimed place's proof evidence — an `EstablishRecord` producer on an
//! operation-result place at an empty path, an `EstablishScalarCase`
//! producer whose `result_case` matches at a lone `Case` path, or a declared
//! `BoundedInteger` singleton bound at the position each observation
//! resolves to — replays the admissible observation set for that place,
//! requires the claimed rows to equal the replayed rows exactly, rebuilds
//! the output itself, and reconstructs the exact node custody the folded
//! observations carry.

use crate::BTreeMap;
use crate::FoldedFieldValue;
use crate::FoldedFieldValueRow;
use crate::O;
use crate::OperationId;
use crate::OptimizationFact;
use crate::OptimizationNode;
use crate::OptimizationUnitValidationError;
use crate::OptimizationValidatorIdentity;
use crate::PlaceId;
use crate::ProvenanceDisposition;
use crate::ProvenanceRewrite;
use crate::PsiOptimizationFunction;
use crate::PsiOptimizationUnit;
use crate::PsiProvenance;
use crate::PsiRealizationSite;
use crate::PsiRewriteCandidate;
use crate::PsiRewritePatch;
use crate::ScalarType;
use crate::StructuralPlaceKind;
use crate::StructuralTypeId;
use crate::ValidatedPsiRewrite;
use crate::candidates::state_specialization::cyclic_machines;
use crate::recompute_psi_optimization_unit_identity;
use crate::validate_psi_optimization_unit;
use semantic_vocabulary::{
    BlockId, CanonicalStructuralPathSegment, StructuralCaseId, StructuralFieldId, ValueId,
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

/// The scalar kind a folded observation must carry: `Boolean` for a
/// `BooleanStructuralField` result, `Integer` for an `IntegerStructuralField`
/// result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ObservedFieldKind {
    Boolean,
    Integer,
}

/// The proven stored value of one field observation plus the witness it was
/// proven under: `Some(producer)` for an establishment basis, `None` for a
/// declared singleton bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProvenFieldValue {
    producer: Option<OperationId>,
    value: FoldedFieldValue,
}

/// The admissibility of one node under `evidence`: a `BooleanStructuralField`
/// or `IntegerStructuralField` observing the evidence's place whose stored
/// value the unit proves. `None` for every other node.
fn admit_field_node(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
    block: BlockId,
    node_index: usize,
    node: &OptimizationNode,
) -> Option<FoldedFieldValueRow> {
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
    Some(FoldedFieldValueRow {
        site: crate::NodeLocation {
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
/// path, each proving the value when the field's initializer scalar is a
/// same-function constant — then the declared `BoundedInteger` singleton
/// bound, which proves the value at any resolvable path independently of
/// producer.
fn proven_field_value(
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

/// The independently derived fold roster for `place`: every
/// `BooleanStructuralField`/`IntegerStructuralField` observing it whose
/// stored value the per-site basis proves, sorted by site.
fn plan_reads(
    unit: &PsiOptimizationUnit,
    evidence: &FieldEvidence<'_>,
    function: &PsiOptimizationFunction,
) -> Vec<FoldedFieldValueRow> {
    let mut reads = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let Some(row) = admit_field_node(unit, evidence, function, block.id, node_index, node)
            else {
                continue;
            };
            reads.push(row);
        }
    }
    reads.sort_by_key(|row| row.site);
    reads
}

/// The folded node a row admits at its site: a
/// `BooleanConstant`/`IntegerConstant` with the read's own custody identity
/// and result value, checked against the claimed basis — the claimed
/// producer and value must equal the basis the row's own site re-derives.
fn folded_node(
    row: &FoldedFieldValueRow,
    unit: &PsiOptimizationUnit,
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
        } => (
            psi_operation,
            result.value,
            source,
            path,
            field,
            ObservedFieldKind::Integer,
        ),
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
    // The claimed basis must re-derive under the place's own evidence: the
    // establishment witness and proven value must equal the row's.
    let Some(evidence) = field_evidence(function, row.source) else {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    };
    let proven = proven_field_value(unit, &evidence, function, path, *field, kind)
        .ok_or(OptimizationUnitValidationError::CandidatePatchMismatch)?;
    if proven.producer != row.producer || proven.value != row.value {
        return Err(OptimizationUnitValidationError::CandidatePatchMismatch);
    }
    let operation = match row.value {
        FoldedFieldValue::Boolean(value) => O::BooleanConstant {
            psi_operation: *psi_operation,
            result,
            value,
        },
        FoldedFieldValue::Integer(value) => O::IntegerConstant {
            psi_operation: *psi_operation,
            result,
            scalar_type: match &node.operation {
                O::IntegerStructuralField { result, .. } => result.scalar_type,
                _ => return Err(OptimizationUnitValidationError::CandidatePatchMismatch),
            },
            value,
        },
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

/// Rebuilds a function's retained optimization-fact index after folding.
/// Every fact row carries its emitting node's `support` custody, so the walk
/// consumes each untouched node's retained rows in order and pushes one
/// constant fact at each folded site.
fn refresh_facts(
    function: &mut PsiOptimizationFunction,
    rows: &[FoldedFieldValueRow],
) -> Result<(), OptimizationUnitValidationError> {
    let folded = rows
        .iter()
        .map(|row| ((row.site.block, row.site.node), row))
        .collect::<BTreeMap<_, _>>();
    let mut retained = std::mem::take(&mut function.facts).into_iter().peekable();
    let mut next_facts = Vec::new();
    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            if let Some(PsiProvenance::Operation(operation)) = node.provenance.first() {
                while let Some(fact) = retained.peek() {
                    let support = match fact {
                        OptimizationFact::OperationObligationReference { support, .. }
                        | OptimizationFact::BooleanConstant { support, .. }
                        | OptimizationFact::IntegerConstant { support, .. } => *support,
                    };
                    if support != *operation {
                        break;
                    }
                    next_facts.push(retained.next().expect("peeked fact exists"));
                }
            }
            if let Some(row) = folded.get(&(
                block.id,
                u32::try_from(node_index)
                    .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?,
            )) {
                next_facts.push(match row.value {
                    FoldedFieldValue::Boolean(constant) => OptimizationFact::BooleanConstant {
                        value: row.result,
                        constant,
                        support: row.psi_operation,
                    },
                    FoldedFieldValue::Integer(constant) => OptimizationFact::IntegerConstant {
                        value: row.result,
                        constant,
                        support: row.psi_operation,
                    },
                });
            }
        }
    }
    next_facts.extend(retained);
    function.facts = next_facts;
    Ok(())
}

/// The exact node custody the folded observations carry: each folded site
/// retains the read's own provenance and fuel settlement, realized at the
/// same node.
fn accepted_provenance(
    function: &PsiOptimizationFunction,
    rows: &[FoldedFieldValueRow],
) -> Result<Vec<ProvenanceRewrite>, OptimizationUnitValidationError> {
    let mut provenance = Vec::new();
    for row in rows {
        let index = usize::try_from(row.site.node)
            .map_err(|_| OptimizationUnitValidationError::CandidateLocationMissing)?;
        let node = function
            .blocks
            .iter()
            .find(|block| block.id == row.site.block)
            .and_then(|block| block.nodes.get(index))
            .ok_or(OptimizationUnitValidationError::CandidateLocationMissing)?;
        let site = PsiRealizationSite::Node(row.site);
        provenance.push(ProvenanceRewrite {
            input: site,
            disposition: ProvenanceDisposition::RealizedAt(site),
            sources: node.provenance.clone(),
            fuel: node.fuel.clone(),
        });
    }
    provenance.sort_by_key(|row| {
        (
            row.input,
            row.disposition.canonical_tag(),
            row.disposition.site(),
        )
    });
    Ok(provenance)
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
    let mut expected_blocks = patch
        .reads
        .iter()
        .map(|row| row.site.block)
        .collect::<Vec<_>>();
    expected_blocks.sort_unstable();
    expected_blocks.dedup();
    let expected_provenance = accepted_provenance(function, &patch.reads)?;
    if candidate.affected_blocks() != expected_blocks
        || candidate.provenance() != expected_provenance
    {
        return Err(OptimizationUnitValidationError::CandidateProvenanceMismatch);
    }

    let input_function = function;
    let folded = patch
        .reads
        .iter()
        .map(|row| folded_node(row, input, input_function).map(|node| (row.site, node)))
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
    refresh_facts(output_function, &patch.reads)?;
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
