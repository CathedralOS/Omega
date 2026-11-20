//! A field or case number belongs to its publishing module. A typed projection
//! selects a member through its checked parent. An untyped cell requires the
//! same answer from every declaration that could have supplied the number.

use super::CheckedPrefix;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    Operation, OperationKind, OperationResult, RecordFieldValue, StructuralArgument,
    StructuralPathSegment, TerminalMachine,
};

fn shared_member<T: Copy + Eq>(mut candidates: impl Iterator<Item = Option<T>>) -> Option<T> {
    let first = candidates.next()??;
    candidates
        .all(|found| found == Some(first))
        .then_some(first)
}

impl CheckedPrefix<'_> {
    pub fn field_id(
        &self,
        parent: StructuralTypeId,
        field: StructuralFieldId,
    ) -> Option<StructuralFieldId> {
        self.structural_type(parent)?;
        self.nominal_field_id(parent, field)
    }

    pub fn case_id(
        &self,
        parent: StructuralTypeId,
        case: StructuralCaseId,
    ) -> Option<StructuralCaseId> {
        self.structural_type(parent)?;
        self.nominal_case_id(parent, case)
    }

    // Type requirements can themselves name a field of the type being checked.
    // Select its declaration here while the enclosing type walk proves its layout.
    pub(super) fn nominal_field_id(
        &self,
        parent: StructuralTypeId,
        field: StructuralFieldId,
    ) -> Option<StructuralFieldId> {
        let (target, rejoined) = self.nominal_type(parent)?;
        let old = &self.old.structural_type(parent)?.shape;
        let new = &self.new.structural_type(target)?.shape;
        let mut selected = old
            .fields_with_case()
            .enumerate()
            .filter(|(_, (_, f))| f.id == field);
        let (ordinal, (a_case, a)) = selected.next()?;
        let (b_case, b) = new.fields_with_case().nth(ordinal)?;
        let same_case = match (a_case, b_case) {
            (None, None) => true,
            (Some(x), Some(y)) => self.nominal_case_id(parent, x.id) == Some(y.id),
            _ => false,
        };
        (same_case
            && selected.next().is_none()
            && new.fields().filter(|f| f.id == b.id).count() == 1
            && (a.id == b.id || !a.identity.is_absent() || rejoined)
            && self.identity(a.identity, b.identity)
            && (rejoined || self.identity(a.declaration_identity, b.declaration_identity)))
        .then_some(b.id)
    }

    pub(super) fn nominal_case_id(
        &self,
        parent: StructuralTypeId,
        case: StructuralCaseId,
    ) -> Option<StructuralCaseId> {
        let (target, rejoined) = self.nominal_type(parent)?;
        let old = self.old.structural_type(parent)?.shape.cases();
        let new = self.new.structural_type(target)?.shape.cases();
        let mut selected = old.iter().enumerate().filter(|(_, c)| c.id == case);
        let (ordinal, a) = selected.next()?;
        let b = new.get(ordinal)?;
        (selected.next().is_none()
            && new.iter().filter(|c| c.id == b.id).count() == 1
            && (a.id == b.id || !a.identity.is_absent() || rejoined)
            && self.identity(a.identity, b.identity)
            && (rejoined || self.identity(a.declaration_identity, b.declaration_identity)))
        .then_some(b.id)
    }

    pub fn unambiguous_field_id(&self, field: StructuralFieldId) -> Option<StructuralFieldId> {
        shared_member(
            self.old
                .types
                .structural_types
                .iter()
                .filter(|t| t.shape.fields().any(|f| f.id == field))
                .map(|t| self.field_id(t.id, field)),
        )
    }

    pub fn unambiguous_case_id(&self, case: StructuralCaseId) -> Option<StructuralCaseId> {
        shared_member(
            self.old
                .types
                .structural_types
                .iter()
                .filter(|t| t.shape.cases().iter().any(|c| c.id == case))
                .map(|t| self.case_id(t.id, case)),
        )
    }

    pub(super) fn projected_type(
        &self,
        machine: &TerminalMachine,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Option<StructuralTypeId> {
        crate::runtime_structural_path_tip(
            self.old.types.structural_types.iter(),
            machine.structural_place_type(root)?,
            path,
        )
        .map(|t| t.id)
    }

    fn check_argument(
        &self,
        old: &StructuralArgument,
        new: &mut StructuralArgument,
    ) -> Result<(), &'static str> {
        if !self.same_argument(old, new) {
            return Err("successor changed a projected argument");
        }
        new.path.clone_from_slice(&old.path);
        Ok(())
    }

    fn check_field(
        &self,
        parent: Option<StructuralTypeId>,
        old: StructuralFieldId,
        new: &mut StructuralFieldId,
    ) -> Result<(), &'static str> {
        let mapped = match parent {
            Some(ty) => self.field_id(ty, old),
            None => self.unambiguous_field_id(old),
        };
        if mapped != Some(*new) {
            return Err("successor changed a selected field declaration");
        }
        *new = old;
        Ok(())
    }

    pub(super) fn check_case(
        &self,
        parent: Option<StructuralTypeId>,
        old: StructuralCaseId,
        new: &mut StructuralCaseId,
    ) -> Result<(), &'static str> {
        let mapped = match parent {
            Some(ty) => self.case_id(ty, old),
            None => self.unambiguous_case_id(old),
        };
        if mapped != Some(*new) {
            return Err("successor changed a selected case declaration");
        }
        *new = old;
        Ok(())
    }

    pub(super) fn payload_fields(
        &self,
        parent: Option<StructuralTypeId>,
        old: &[StructuralFieldId],
        new: &mut [StructuralFieldId],
    ) -> Result<(), &'static str> {
        if old.len() != new.len() {
            return Err("successor changed the number of payload fields");
        }
        for (&a, b) in old.iter().zip(new) {
            self.check_field(parent, a, b)?;
        }
        Ok(())
    }

    pub(super) fn operation_coordinates(
        &self,
        machine: &TerminalMachine,
        op: &Operation,
        other: &mut OperationKind,
    ) -> Result<(), &'static str> {
        use OperationKind::*;
        let result_type = match &op.result {
            OperationResult::Structural(r) => Some(r.structural_type),
            _ => None,
        };
        match (&op.kind, &mut *other) {
            (StoreStructuralField { value: a, .. }, StoreStructuralField { value: b, .. }) => {
                self.check_argument(a, b)?
            }
            (
                EstablishStructuralArray { elements: a },
                EstablishStructuralArray { elements: b },
            ) => {
                for (x, y) in a.iter().zip(b) {
                    self.check_argument(x, y)?;
                }
            }
            (
                EstablishStructuralCase { result_case: a, .. },
                EstablishStructuralCase { result_case: b, .. },
            )
            | (
                EstablishScalarCase { result_case: a, .. },
                EstablishScalarCase { result_case: b, .. },
            ) => self.check_case(result_type, *a, b)?,
            _ => {}
        }
        match (&op.kind, other) {
            (EstablishRecord { fields: a }, EstablishRecord { fields: b })
            | (
                EstablishStructuralCase { fields: a, .. },
                EstablishStructuralCase { fields: b, .. },
            ) => {
                for (x, y) in a.iter().zip(b) {
                    self.check_field(result_type, x.field, &mut y.field)?;
                    if let (RecordFieldValue::Structural(old), RecordFieldValue::Structural(new)) =
                        (&x.value, &mut y.value)
                    {
                        self.check_argument(old, new)?;
                    }
                }
            }
            (EstablishScalarCase { fields: a, .. }, EstablishScalarCase { fields: b, .. }) => {
                for (x, y) in a.iter().zip(b) {
                    self.check_field(result_type, x.field, &mut y.field)?;
                }
            }
            (
                StoreStructuralField {
                    destination: root,
                    path,
                    field: a,
                    ..
                },
                StoreStructuralField { field: b, .. },
            )
            | (
                MoveStructuralField {
                    source: root,
                    path,
                    field: a,
                    ..
                },
                MoveStructuralField { field: b, .. },
            )
            | (
                StructuralByteViewFieldStore {
                    destination: root,
                    path,
                    field: a,
                    ..
                },
                StructuralByteViewFieldStore { field: b, .. },
            ) => {
                if let (Some(a), Some(b)) = (a, b) {
                    self.check_field(self.projected_type(machine, *root, path), *a, b)?;
                }
            }
            (
                StructuralScalarFieldStore {
                    destination,
                    path,
                    field: a,
                    ..
                },
                StructuralScalarFieldStore { field: b, .. },
            ) => {
                self.check_field(self.projected_type(machine, *destination, path), *a, b)?;
            }
            (
                ScalarStructuralField {
                    source,
                    path: a,
                    field,
                    ..
                },
                ScalarStructuralField {
                    path: b, field: y, ..
                },
            )
            | (
                BooleanStructuralField {
                    source,
                    path: a,
                    field,
                    ..
                },
                BooleanStructuralField {
                    path: b, field: y, ..
                },
            ) => {
                let root = machine
                    .structural_place_type(*source)
                    .ok_or("field read has no source declaration")?;
                if a.len() != b.len() {
                    return Err("successor changed a field projection");
                }
                for (index, (x, y)) in a.iter().zip(b.iter_mut()).enumerate() {
                    let parent = crate::canonical_structural_path_tip(
                        self.old.types.structural_types.iter(),
                        root,
                        &a[..index],
                    )
                    .map(|t| t.id);
                    match (x, y) {
                        (
                            CanonicalStructuralPathSegment::Field(x),
                            CanonicalStructuralPathSegment::Field(y),
                        ) => self.check_field(parent, *x, y)?,
                        (
                            CanonicalStructuralPathSegment::Case(x),
                            CanonicalStructuralPathSegment::Case(y),
                        ) => self.check_case(parent, *x, y)?,
                        _ => {}
                    }
                }
                let parent = crate::canonical_structural_path_tip(
                    self.old.types.structural_types.iter(),
                    root,
                    a,
                )
                .map(|t| t.id);
                self.check_field(parent, *field, y)?;
            }
            (
                EstablishElementView {
                    source: a,
                    element: x,
                    ..
                },
                EstablishElementView {
                    source: b,
                    element: y,
                    ..
                },
            )
            | (
                EstablishMutableElementView {
                    source: a,
                    element: x,
                    ..
                },
                EstablishMutableElementView {
                    source: b,
                    element: y,
                    ..
                },
            ) => {
                if self.structural_type(*x) != Some(*y) {
                    return Err("successor changed the element type of a view");
                }
                *y = *x;
                self.check_argument(a, b)?;
            }
            (ByteSequenceLength { source: a }, ByteSequenceLength { source: b })
            | (ElementViewLength { source: a }, ElementViewLength { source: b })
            | (ElementViewSubslice { source: a, .. }, ElementViewSubslice { source: b, .. })
            | (ElementViewRead { source: a, .. }, ElementViewRead { source: b, .. })
            | (ElementViewWrite { destination: a, .. }, ElementViewWrite { destination: b, .. }) => {
                self.check_argument(a, b)?
            }
            _ => {}
        }
        Ok(())
    }
}
