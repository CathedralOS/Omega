use crate::expressions::body_typer::BodyTyper;
use crate::expressions::storage_origin::PatternSource;
use crate::{ObligationKind, TypeError, ValueType};
use omega_parser::NodeKind;

impl BodyTyper<'_, '_> {
    pub(super) fn check_block(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let mut result = None;
        for &statement in &source.nodes[node_index].children {
            use NodeKind::*;
            match source.nodes[statement].kind {
                Let => self.check_binding(statement)?,
                AsmBlock => self.assembly(statement)?,
                Assign => self.assign(statement)?,
                Machine => {}
                ConstParam if source.nodes[node_index].kind == Fragment => {
                    let ty = self
                        .unit
                        .raw_type(self.unit.single_child(statement)?, &self.catalog.types)?;
                    self.record_expression(statement, ty);
                }
                Use | Data | Const | Trait | Domain | Conformance | MathLet if source.nodes[node_index].kind == Fragment => {}
                Transition => {
                    result = Some(self.dispatch(statement, expected)?);
                    self.set_origin(node_index, self.origin(statement));
                }
                Tail | ExpressionStatement | Discard => {
                    let expr = self.unit.single_child(statement)?;
                    let tail = source.nodes[statement].kind == Tail;
                    let value = self.type_expression_with_expected_type(expr, if tail { expected } else { None })?;
                    if tail {
                        self.set_origin(node_index, self.origin(expr));
                        if let Some(ty) = expected {
                            self.check_destination(expr, &value, ty)?;
                        }
                        let result_type = expected.cloned().unwrap_or(value);
                        self.check_return_borrow(node_index, expr, &result_type)?;
                        result = Some(result_type);
                    } else if source.nodes[statement].kind == ExpressionStatement && value != ValueType::NoResult {
                        return Err(TypeError::at(source, expr, "value-producing statement requires an explicit discard"));
                    }
                }
                _ => return Err(TypeError::unsupported(source, statement)),
            }
        }
        if result.is_none()
            && let Some(ty) = expected
            && *ty != ValueType::NoResult
        {
            return Err(TypeError::at(source, node_index, format!("missing result of type {ty}")));
        }
        Ok(result.unwrap_or(ValueType::NoResult))
    }
    /// A copy read yields an independent value. Only a machine result that
    /// keeps the reference carries its origin across the return.
    pub(super) fn check_return_borrow(
        &mut self,
        body: usize,
        expr: usize,
        result: &ValueType,
    ) -> Result<(), TypeError> {
        let source = self.unit.file;
        if let Some(machine) = source.nodes[body]
            .parent
            .filter(|&p| source.nodes[p].kind == NodeKind::Machine)
            && let Some(origin) = self.carried_loans(result, self.origin(expr))
        {
            if origin.is_local() {
                return Err(TypeError::at(source, expr, "returned reference borrows invocation-local storage"));
            }
            self.record_obligation(
                expr,
                ObligationKind::ReturnBorrow {
                    machine: self.unit.declaration(machine)?.clone(),
                },
            );
        }
        Ok(())
    }
    /// numeric_values.md refuses an unannotated binding of an unsuffixed number
    /// at its initializer, because later uses never choose a binding's type.
    pub(super) fn require_declared_number(
        &self,
        initializer: usize,
        ty: &ValueType,
    ) -> Result<(), TypeError> {
        if ty.holds_anonymous_number() {
            return Err(TypeError::at(
                self.unit.file,
                initializer,
                "an unsuffixed numeric binding must declare its type, as in `let count: u8 = 5` or `5u8`",
            ));
        }
        Ok(())
    }
    pub(super) fn check_binding(
        &mut self,
        node_index: usize,
    ) -> Result<(), TypeError> {
        let source = self.unit.file;
        if !source.nodes[node_index].leaves.iter().any(|l| l.text == "=") {
            let [pattern, annotation] = *source.nodes[node_index].children.as_slice() else {
                return Err(TypeError::unsupported(source, node_index));
            };
            let ty = self.unit.annotation(annotation, &self.catalog.types)?;
            self.record_obligation(node_index, ObligationKind::ZeroValue(ty.clone()));
            if source.nodes[node_index].words().any(|w| w.text == "mut") {
                self.writable.insert(source.span(pattern));
            }
            if !self
                .bind_pattern(pattern, &ty, annotation, &PatternSource::default())?
                .is_total()
            {
                return Err(TypeError::at(source, pattern, "let requires an irrefutable pattern"));
            }
            return Ok(());
        }
        let (pattern, annotation, expr) = match *source.nodes[node_index].children.as_slice() {
            [p, e] => (p, None, e),
            [p, t, e] => (p, Some(t), e),
            _ => return Err(TypeError::unsupported(source, node_index)),
        };
        let declared = annotation
            .map(|t| self.unit.initialized_annotation(t, &self.catalog.types, None))
            .transpose()?;
        // A destructuring `let` decomposes its initializer in place, as a
        // dispatch does; a plain binding reads its value.
        let decomposes = declared.is_none() && matches!(source.nodes[pattern].kind, NodeKind::RecordPattern | NodeKind::TuplePattern);
        let (value, from) = if decomposes {
            self.pattern_subject(expr)?
        } else {
            let value = self.type_expression_with_expected_type(expr, declared.as_ref())?;
            (value, PatternSource::of(self.origin(expr)))
        };
        let local_type = if let Some(expected) = declared {
            let expected = crate::type_system::local_lifetimes::FieldLifetimes::from_value(expected, &value);
            self.check_destination(expr, &value, &expected)?;
            expected
        } else {
            // The whole initializer is checked, not only the parts a binder
            // keeps: `let _ = 5;` and `let (_, b) = (5, 6u8);` also declare
            // a binding without saying what its anonymous part holds.
            self.require_declared_number(expr, &value)?;
            value
        };
        if local_type == ValueType::NoResult {
            return Err(TypeError::at(source, expr, "binding requires a value"));
        }
        if source.nodes[node_index].words().any(|w| w.text == "mut") {
            self.writable.insert(source.span(pattern));
        }
        if !self.bind_pattern(pattern, &local_type, expr, &from)?.is_total() {
            return Err(TypeError::at(source, pattern, "let requires an irrefutable pattern"));
        }
        Ok(())
    }
}
