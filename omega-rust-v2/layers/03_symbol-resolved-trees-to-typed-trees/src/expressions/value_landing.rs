use crate::expressions::body_typer::BodyTyper;
use crate::{Access, ObligationKind, TypeError, TypedExpression, ValueType};

impl BodyTyper<'_, '_> {
    pub(super) fn check_destination(
        &mut self,
        node_index: usize,
        value: &ValueType,
        expected: &ValueType,
    ) -> Result<(), TypeError> {
        self.land_value(node_index, value, expected, false)
    }
    /// Whether reading a value of this type through a reference may copy it.
    pub(super) fn copy_eligible(
        &self,
        ty: &ValueType,
    ) -> bool {
        self.catalog.types.multiplicity(ty) == crate::Multiplicity::Copy
    }
    /// Reading a value out of borrowed storage copies it, so the value needs
    /// ordinary copy eligibility (patterns.md: "A field-value binding from
    /// borrowed storage still needs ordinary copy eligibility"). A non-copy
    /// value needs an explicit borrow instead, except that a place reached
    /// through `@mut` may be moved out (ownership.md, borrowed-storage
    /// invariant windows).
    pub(super) fn read_borrowed(
        &mut self,
        node_index: usize,
        access: &Access,
        ty: &ValueType,
    ) -> Result<(), TypeError> {
        match access {
            Access::Write => Err(TypeError::at(self.unit.file, node_index, "cannot read through a write-only reference")),
            _ if self.copy_eligible(ty) => {
                self.record_obligation(node_index, ObligationKind::CopyRead(ty.clone()));
                Ok(())
            }
            Access::Mut => {
                let file = self.unit.file;
                self.record_obligation(
                    node_index,
                    ObligationKind::RestoreBorrowed {
                        place: file.node_id(file.ungroup(node_index)),
                        ty: ty.clone(),
                    },
                );
                Ok(())
            }
            Access::Shared => Err(TypeError::at(
                self.unit.file,
                node_index,
                format!("reading {ty} out of borrowed storage requires a copy type; borrow it with @ instead"),
            )),
        }
    }
    pub(super) fn land_value(
        &mut self,
        node_index: usize,
        value: &ValueType,
        expected: &ValueType,
        explicit: bool,
    ) -> Result<(), TypeError> {
        if let (ValueType::Callable(actual), ValueType::Callable(destination)) = (value, expected)
            && actual.contract != destination.contract
            && destination.admits_contract(actual)
        {
            self.record_obligation(
                node_index,
                ObligationKind::RefineCallableValue {
                    source: actual.contract.clone(),
                    destination: destination.contract.clone(),
                },
            );
        }
        if let ValueType::AtLifetime(inner, region) = expected
            && (value == inner.as_ref()
                || matches!((region.as_ref(),value),(ValueType::InferredLifetime {..},ValueType::AtLifetime(actual,_)) if actual==inner && value!=expected))
        {
            self.record_obligation(
                node_index,
                ObligationKind::BindLifetime {
                    reference: value.clone(),
                    destination: expected.clone(),
                },
            );
            return Ok(());
        }
        // The lifetime belongs to the loan. Reading its pointee preserves that
        // loan and uses the same access check as an unannotated reference.
        if let Some((Access::Shared | Access::Mut, inner)) = value.reference_parts()
            && inner == expected
            && self.copy_eligible(expected)
        {
            self.record_obligation(node_index, ObligationKind::CopyRead(expected.clone()));
            return Ok(());
        }
        if !value.fits(expected) {
            return Err(TypeError::at(self.unit.file, node_index, format!("expected {expected}, found {value}")));
        }
        if let ValueType::Exact {
            fractional_origin: Some(origin),
            ..
        } = value
            && matches!(expected, ValueType::Integer { .. } | ValueType::IndexedInteger { .. })
            && !explicit
        {
            return Err(TypeError::at(
                self.unit.file,
                node_index,
                format!(
                    "fractional intermediate at bytes {}..{} yields {value}; use an explicit cast to {expected}",
                    origin.get_start_offset(),
                    origin.get_end_offset()
                ),
            ));
        }
        if matches!(value, ValueType::Exact { .. }) && matches!(expected, ValueType::IndexedInteger { .. }) {
            self.record_obligation(
                node_index,
                ObligationKind::ExactCast {
                    source: value.clone(),
                    destination: expected.clone(),
                },
            );
        }
        if matches!(value, ValueType::Exact { .. }) {
            self.output
                .landings
                .push(TypedExpression::at(self.unit.file, node_index, expected.clone()));
        }
        Ok(())
    }
}
