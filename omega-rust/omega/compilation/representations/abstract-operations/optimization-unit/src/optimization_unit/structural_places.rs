//! Structural-place evidence queries shared by specialization admission and
//! replay: a rostered place's declared structural type and establishing
//! producer.

use abstract_operations::AbstractOperation as O;
use semantic_vocabulary::{
    OperationId, PlaceId, StructuralCaseId, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId,
};
use terminal_psi::{RecordFieldValue, StructuralPlaceDeclaration};

use super::PsiOptimizationFunction;

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
