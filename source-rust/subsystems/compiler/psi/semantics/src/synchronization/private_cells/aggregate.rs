//! Construct local records and cases, then pass the selected payload on its
//! control-flow edge. All field projections refer to the checked data type.

use super::activation::{Bindings, Frame};
use super::storage::{Location, Storage, Stored};
use crate::synchronization::progress::ScalarFact;
use semantic_vocabulary::PlaceId;
use std::borrow::Cow;
use terminal_psi::{
    OperationKind, RecordFieldInitializer, RecordFieldValue, StructuralCaseSuccessorEdge,
    StructuralPathSegment as Segment, StructuralTypeShape, TerminalModule,
};

pub(super) fn establish(
    shape: &StructuralTypeShape,
    operation: &OperationKind,
    bindings: &Bindings,
    storage: &mut Storage,
) -> Option<Location> {
    let (tag, fields): (_, Cow<'_, [RecordFieldInitializer]>) = match operation {
        OperationKind::EstablishRecord { fields } => (None, Cow::Borrowed(fields)),
        OperationKind::EstablishStructuralCase {
            result_case,
            fields,
        } => (Some(*result_case), Cow::Borrowed(fields)),
        OperationKind::EstablishScalarCase {
            result_case,
            fields,
        } => (
            Some(*result_case),
            Cow::Owned(
                fields
                    .iter()
                    .map(|field| RecordFieldInitializer {
                        field: field.field,
                        value: RecordFieldValue::Scalar {
                            value: field.value,
                            range_obligation: field.range_obligation,
                        },
                    })
                    .collect(),
            ),
        ),
        _ => return None,
    };
    let root = storage.allocate(tag.map_or(Stored::Unknown, Stored::variant));
    for (ordinal, field) in fields.iter().enumerate() {
        let path = match tag {
            Some(tag) => {
                terminal_psi::structural_case_initializer_path(shape, tag, ordinal, field.field)?
            }
            None => vec![Segment::Field(
                terminal_psi::live_field_by_id(shape.direct_fields()?, field.field)?.identity,
            )],
        };
        let location = storage.project(root.clone(), &path)?;
        let contents = match &field.value {
            RecordFieldValue::Scalar { value, .. } => bindings.values.get(value).copied().into(),
            RecordFieldValue::Structural(arg) => storage
                .locate(&bindings.places, arg.place, &arg.path)
                .map(|location| storage.get(&location).clone())
                .unwrap_or_default(),
        };
        storage.set(&location, contents);
    }
    Some(root)
}

pub(super) fn dispatch<'m>(
    module: &TerminalModule,
    frame: &Frame<'_>,
    storage: &Storage,
    source: PlaceId,
    path: &[Segment],
    cases: &'m [StructuralCaseSuccessorEdge],
) -> Option<(&'m StructuralCaseSuccessorEdge, Vec<Option<ScalarFact>>)> {
    let value = storage.locate(&frame.bindings.places, source, path)?;
    let tag = storage.get(&value).tag()?;
    let edge = cases.iter().find(|edge| edge.case == tag)?;
    let destination = frame.machine.block(&edge.target)?;
    if edge.payload_fields.len() != destination.parameters.len() {
        return None;
    }
    let mut payload_path = path.to_vec();
    payload_path.push(Segment::Case(tag));
    let payload = storage.project(value, &[Segment::Case(tag)])?;
    let mut fields = Vec::with_capacity(edge.payload_fields.len());
    for (&field, formal) in edge.payload_fields.iter().zip(&destination.parameters) {
        let declared = crate::projected_field_declaration(
            module.types.structural_types.iter(),
            frame.machine.structural_place_type(source)?,
            &payload_path,
            field,
        )?;
        let field = storage.project(payload.clone(), &[Segment::Field(declared.identity)])?;
        fields.push(storage.get(&field).scalar(formal.scalar_type));
    }
    Some((edge, fields))
}
