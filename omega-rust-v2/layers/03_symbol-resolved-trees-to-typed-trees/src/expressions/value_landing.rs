use crate::expressions::body_typer::BodyTyper;
use crate::{Access, ObligationKind, Site, TypeError, TypedExpression, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    pub(super) fn check_destination(
        &mut self,
        node_index: usize,
        value: &ValueType,
        expected: &ValueType,
    ) -> Result<(), TypeError> {
        self.land_value(node_index, value, expected, false)
    }
    pub(super) fn local_view(
        &self,
        node_index: usize,
    ) -> bool {
        let source = self.unit.file;
        match source.nodes[node_index].kind {
            NodeKind::Paren => self
                .unit
                .single_child(node_index)
                .is_ok_and(|i| self.local_view(i)),
            NodeKind::PathExpression => matches!(self.unit.symbol(node_index),Some(Symbol::Binding{span,..}) if self.local_views.contains(span)),
            NodeKind::Match | NodeKind::Transition => source.nodes[node_index]
                .children
                .iter()
                .filter(|&&i| source.nodes[i].kind == NodeKind::Arm)
                .any(|&arm| {
                    source.nodes[arm]
                        .children
                        .last()
                        .is_some_and(|&result| self.local_view(result))
                }),
            NodeKind::Body | NodeKind::Tail => source.nodes[node_index]
                .children
                .last()
                .is_some_and(|&last| self.local_view(last)),
            NodeKind::Borrow => {
                let Some(&place) = source.nodes[node_index].children.first() else {
                    return false;
                };
                // Borrowing the carrier itself borrows invocation-local storage,
                // even when a by-value parameter carries a reference.
                let mut root = place;
                while matches!(source.nodes[root].kind, NodeKind::FieldAccess | NodeKind::Index | NodeKind::Paren) {
                    let Some(&inner) = source.nodes[root].children.first() else {
                        return true;
                    };
                    root = inner;
                    if matches!(source.nodes[root].kind, NodeKind::PathExpression)
                        && let Some(Symbol::Binding { span, .. }) = self.unit.symbol(root)
                        && matches!(self.locals.get(span), Some(ValueType::Reference(..) | ValueType::AtLifetime(..)))
                    {
                        return self.local_views.contains(span);
                    }
                }
                matches!(self.unit.symbol(root), Some(Symbol::Binding { .. })) || source.nodes[root].kind == NodeKind::RecordLiteral
            }
            _ => false,
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
