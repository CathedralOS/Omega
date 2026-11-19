use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, Site, TypeError, TypedExpression, ValueType};
use omega_parser::NodeKind;

impl BodyTyper<'_, '_> {
    pub(super) fn check_block(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = &self.unit.file;
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
                        .raw_type(self.unit.single_child(statement)?, None, &self.catalog.types)?;
                    self.output.expressions.push(TypedExpression {
                        site: Site::at(source, statement),
                        ty,
                    });
                }
                Use | Data | Const | Trait | Domain | Conformance | MathLet if source.nodes[node_index].kind == Fragment => {}
                Transition => result = Some(self.dispatch(statement, expected)?),
                Tail | ExpressionStatement | Discard => {
                    let expr = self.unit.single_child(statement)?;
                    let tail = source.nodes[statement].kind == Tail;
                    let value = self.type_expression_with_expected_type(expr, if tail { expected } else { None })?;
                    if tail {
                        if let Some(ty) = expected {
                            self.check_destination(expr, &value, ty)?;
                        }
                        if source.nodes[node_index]
                            .parent
                            .is_some_and(|p| source.nodes[p].kind == Machine)
                            && matches!(value, ValueType::Reference(..) | ValueType::AtLifetime(..))
                        {
                            if self.local_view(expr) {
                                return Err(TypeError::at(source, expr, "returned reference borrows invocation-local storage"));
                            }
                            self.record_obligation(
                                expr,
                                ObligationKind::ReturnBorrow {
                                    machine: self
                                        .unit
                                        .declaration(source.nodes[node_index].parent.unwrap())?
                                        .clone(),
                                },
                            );
                        }
                        result = Some(expected.cloned().unwrap_or(value));
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
    pub(super) fn check_binding(
        &mut self,
        node_index: usize,
    ) -> Result<(), TypeError> {
        let source = &self.unit.file;
        if !source.nodes[node_index].leaves.iter().any(|l| l.text == "=") {
            let [pattern, annotation] = *source.nodes[node_index].children.as_slice() else {
                return Err(TypeError::unsupported(source, node_index));
            };
            let ty = self.unit.annotation(annotation, &self.catalog.types)?;
            self.record_obligation(node_index, ObligationKind::ZeroValue(ty.clone()));
            if source.nodes[node_index].words().any(|w| w.text == "mut") {
                self.writable.insert(source.span(pattern));
            }
            if !self.bind_pattern(pattern, &ty)?.is_total() {
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
            .map(|t| self.unit.annotation(t, &self.catalog.types))
            .transpose()?;
        let value = self.type_expression_with_expected_type(expr, declared.as_ref())?;
        let local_type = if let Some(expected) = declared {
            self.check_destination(expr, &value, &expected)?;
            expected
        } else {
            if matches!(value, ValueType::Exact { .. }) {
                return Err(TypeError::at(
                    source,
                    expr,
                    "an unsuffixed numeric binding must declare its type, as in `let count: u8 = 5`",
                ));
            }
            value
        };
        if local_type == ValueType::NoResult {
            return Err(TypeError::at(source, expr, "binding requires a value"));
        }
        if source.nodes[node_index].words().any(|w| w.text == "mut") {
            self.writable.insert(source.span(pattern));
        }
        if self.local_view(expr) {
            self.local_views.insert(source.span(pattern));
        }
        if !self.bind_pattern(pattern, &local_type)?.is_total() {
            return Err(TypeError::at(source, pattern, "let requires an irrefutable pattern"));
        }
        Ok(())
    }
}
