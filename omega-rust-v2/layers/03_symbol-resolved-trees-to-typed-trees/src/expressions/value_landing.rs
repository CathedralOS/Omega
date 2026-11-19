use crate::expressions::body_typer::BodyTyper;
use crate::{Access, ObligationKind, Site, TypeError, TypedExpression, ValueType};

impl BodyTyper<'_, '_> {
    pub(super) fn check_destination(
        &mut self,
        node_index: usize,
        value: &ValueType,
        expected: &ValueType,
    ) -> Result<(), TypeError> {
        self.land_value(node_index, value, expected, false)
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
        if let ValueType::AtLifetime(inner, _) = expected
            && value == inner.as_ref()
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
        let reference = match value {
            ValueType::AtLifetime(reference, _) => reference.as_ref(),
            other => other,
        };
        if let ValueType::Reference(Access::Shared | Access::Mut, inner) = reference
            && inner.as_ref() == expected
            && (expected.numeric()
                || expected == &ValueType::Bool
                || expected.nominal_parts().is_some_and(|(id, _)| {
                    self.catalog
                        .data_properties
                        .get(id)
                        .is_some_and(|p| p.multiplicity == crate::Multiplicity::Copy)
                }))
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
            self.output.landings.push(TypedExpression {
                node: self.unit.file.node_id(node_index),
                site: Site::at(self.unit.file, node_index),
                ty: expected.clone(),
            });
        }
        Ok(())
    }
}
