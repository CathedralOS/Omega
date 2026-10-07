//! Structural-place evidence queries shared by specialization admission and
//! replay: a rostered place's declared structural type and establishing
//! producer.

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, OperationId, PlaceId, StructuralCaseId, StructuralFieldId,
    StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    RecordFieldValue, StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeShape,
};

use super::{PsiOptimizationFunction, PsiOptimizationUnit};

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
