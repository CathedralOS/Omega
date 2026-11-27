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
                .map(|location| storage.get(&location))
                .unwrap_or_default(),
        };
        storage.set(&location, contents);
    }
    Some(root)
}

pub(super) enum Dispatch<'m> {
    Fork(Vec<usize>),
    Selected(&'m StructuralCaseSuccessorEdge, Vec<Option<ScalarFact>>),
}

pub(super) fn dispatch<'m>(
    module: &TerminalModule,
    frame: &Frame<'_>,
    storage: &mut Storage,
    source: PlaceId,
    path: &[Segment],
    cases: &'m [StructuralCaseSuccessorEdge],
) -> Option<Dispatch<'m>> {
    let location = storage.locate(&frame.bindings.places, source, path);
    let value = location
        .as_ref()
        .map_or(Stored::Unknown, |v| storage.get(v));
    let root = frame.machine.structural_place_type(source)?;
    let zero = if value.is_zero() {
        Some(
            crate::runtime_structural_path_tip(module.types.structural_types.iter(), root, path)?
                .shape
                .cases()
                .first()?
                .id,
        )
    } else {
        None
    };
    let edge = if let Some(arm) = frame.selected_arm {
        cases.get(arm)?
    } else if let Some(tag) = zero.or_else(|| value.tag()) {
        cases.iter().find(|edge| edge.case == tag)?
    } else {
        let tags = value.tags();
        let mut admitted = cases.iter().enumerate().filter_map(|(i, edge)| {
            (tags.is_empty() || tags.binary_search(&edge.case).is_ok()).then_some(i)
        });
        let first = admitted.next()?;
        if let Some(second) = admitted.next() {
            let mut arms = vec![first, second];
            arms.extend(admitted);
            return Some(Dispatch::Fork(arms));
        }
        &cases[first]
    };
    let destination = frame.machine.block(&edge.target)?;
    if edge.payload_fields.len() != destination.parameters.len() {
        return None;
    }
    let Some(location) = location else {
        // Borrowed external input or merged aliases can leave the home
        // unknown. Its checked tag still selects one of the declared edges.
        return Some(Dispatch::Selected(
            edge,
            vec![None; edge.payload_fields.len()],
        ));
    };
    if value.tag() != Some(edge.case) {
        let selected = value.select_case(edge.case);
        storage.set(&location, selected);
    }
    let mut payload_path = path.to_vec();
    payload_path.push(Segment::Case(edge.case));
    let payload = storage.project(location, &[Segment::Case(edge.case)])?;
    let mut fields = Vec::with_capacity(edge.payload_fields.len());
    for (&field, formal) in edge.payload_fields.iter().zip(&destination.parameters) {
        let declared = crate::projected_field_declaration(
            module.types.structural_types.iter(),
            root,
            &payload_path,
            field,
        )?;
        let field = storage.project(payload.clone(), &[Segment::Field(declared.identity)])?;
        fields.push(storage.get(&field).scalar(formal.scalar_type));
    }
    Some(Dispatch::Selected(edge, fields))
}
