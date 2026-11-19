use super::call_arguments::{CallOperand, PendingComparisons, SelectedMachine};
use crate::declarations::operator_declarations::nominal_operator_operand;
use crate::evaluation::numeric_literals;
use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, Site, TypeError, TypedOperation, ValueType};
use omega_parser::NodeKind;

impl BodyTyper<'_, '_> {
    pub(super) fn declared_operator(
        &mut self,
        node_index: usize,
        token: &str,
        operands: &[CallOperand],
    ) -> Result<Option<ValueType>, TypeError> {
        let source = self.unit.file;
        let Some(family) = self.catalog.token_bindings.get(token) else {
            return Ok(None);
        };
        let mut selected = Vec::new();
        let indices = &self.catalog.types.indices;
        let mut waiting = false;
        for binding in family {
            let id = &binding.declaration.machine;
            let signature = &self.catalog.signatures[id];
            if signature.parameters.len() != operands.len()
                || !self.unit.can_select_from(&id.package)
                || id.package != source.package && !binding.declaration.public
            {
                continue;
            }
            let mut contexts = Vec::new();
            let participates = match &binding.home {
                Some(home) if home.kind == NodeKind::Domain => operands
                    .iter()
                    .any(|op| op.value.domains().iter().any(|d| &d.declaration == home)),
                Some(home) if home.kind == NodeKind::Trait => {
                    if let Some(owner) = &self.caller {
                        for formal in self.catalog.types.formals(owner) {
                            if let Some(ValueType::Evidence(slot)) = self.catalog.types.parameter_carrier(&formal)
                                && let Some(parent) = self
                                    .catalog
                                    .trait_lineage(slot)
                                    .into_iter()
                                    .find(|s| &s.declaration == home)
                            {
                                contexts.push((Some(formal), Some(parent)));
                            }
                        }
                    }
                    !contexts.is_empty()
                }
                nominal_home => operands.iter().enumerate().any(|(i, op)| {
                    nominal_operator_operand(token, i, &op.value).is_some_and(|carrier| {
                        nominal_home
                            .as_ref()
                            .map_or_else(|| carrier.package == id.package, |home| carrier == home)
                    })
                }),
            };
            if !participates {
                continue;
            }
            if contexts.is_empty() {
                contexts.push((None, None));
            }
            for (evidence, owner) in contexts {
                let capture = |t: &ValueType| {
                    owner
                        .as_ref()
                        .map_or_else(|| t.clone(), |slot| t.substitute(&slot.declaration, &slot.trait_arguments))
                };
                let mut args = vec![None; self.catalog.types.arity(id)];
                let mut subjects = Vec::with_capacity(operands.len());
                let mut pending = PendingComparisons::default();
                for (position, ((_, param), operand)) in signature.parameters.iter().zip(operands).enumerate() {
                    let formal = capture(param).instantiate_subjects(signature, &subjects);
                    if !self.infer_operand_with(source.nodes[node_index].kind, position, operand, &formal, id, &mut args, &mut |a, b| {
                        pending.compare(indices, a, b)
                    }) {
                        break;
                    }
                    subjects.push(operand.subject(self.unit, &formal));
                }
                if subjects.len() != operands.len() {
                    continue;
                }
                if pending.retain_demands(indices) {
                    waiting = true;
                    continue;
                }
                let Some(args) = args.into_iter().collect::<Option<Vec<_>>>() else {
                    continue;
                };
                selected.push(SelectedMachine {
                    machine: id,
                    explicit_arguments: &[],
                    arguments: args,
                    enclosing: owner
                        .into_iter()
                        .map(|s| (s.declaration, s.trait_arguments))
                        .collect(),
                    evidence,
                });
            }
        }
        if waiting {
            return Err(indices.wait(source, node_index));
        }
        if selected.len() > 1 {
            return Err(TypeError::at(source, node_index, format!("ambiguous operator {token}")));
        }
        selected
            .pop()
            .map(|target| self.apply_machine(node_index, target, operands))
            .transpose()
    }
    pub(super) fn record_operator(
        &mut self,
        node_index: usize,
        token: &str,
        operands: Vec<ValueType>,
        result: &ValueType,
    ) {
        self.output.operations.push(TypedOperation {
            node: self.unit.file.node_id(node_index),
            site: Site::at(self.unit.file, node_index),
            token: token.into(),
            operands,
            result: result.clone(),
        });
    }
    pub(super) fn binary_chain(
        &mut self,
        mut node_index: usize,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let mut pending = Vec::new();
        let mut left_type = loop {
            if source.nodes[node_index].kind != NodeKind::Binary {
                break self.type_expression(node_index)?;
            }
            let operator = self
                .unit
                .file
                .operator(node_index)
                .ok_or_else(|| TypeError::at(source, node_index, "missing resolved operator"))?;
            if let Some((left, right)) = self.unit.equality_operands(node_index)? {
                break self.type_equality(node_index, left, right)?;
            }

            let [left, right] = *source.nodes[node_index].children.as_slice() else {
                return Err(TypeError::unsupported(source, node_index));
            };
            pending.push((node_index, left, right, operator.token));
            node_index = left;
        };
        for (node_index, left, right, token) in pending.into_iter().rev() {
            let ty = if token == "in" {
                self.membership(node_index, &left_type, right)?
            } else {
                let fact = (token == "&&").then(|| self.case_guard(left)).flatten();
                let previous = fact
                    .as_ref()
                    .and_then(|(s, id)| self.cases.insert(*s, id.clone()));
                let left_operand = self.call_operand(left, left_type);
                let right_type = self.type_expression(right)?;
                if let Some((s, _)) = fact
                    && self.cases.remove(&s).is_some()
                    && let Some(old) = previous
                {
                    self.cases.insert(s, old);
                }
                self.binary(node_index, token, [left_operand, self.call_operand(right, right_type)])?
            };
            self.record_expression(node_index, ty.clone());
            left_type = ty;
        }
        Ok(left_type)
    }
    fn binary(
        &mut self,
        node_index: usize,
        token: &str,
        operands: [CallOperand; 2],
    ) -> Result<ValueType, TypeError> {
        use ValueType::*;
        let source = self.unit.file;
        if let Some(ty) = self.declared_operator(node_index, token, &operands)? {
            return Ok(ty);
        }
        let [left, right] = &operands;
        let (left, left_type) = (left.node, &left.value);
        let (right, right_type) = (right.node, &right.value);
        let bad = || TypeError::at(source, node_index, format!("no matching operator {token} for {left_type} and {right_type}"));
        let compare = matches!(token, "==" | "!=" | "<" | "<=" | ">" | ">=");
        let word = |t: &ValueType| matches!(t, Integer { width: Some(_), .. } | IndexedInteger { .. });
        let integer = |t: &ValueType| matches!(t, Integer { .. } | IndexedInteger { .. });
        let (result, operands) = if let (
            Exact {
                value: a,
                fractional_origin: af,
            },
            Exact {
                value: b,
                fractional_origin: bf,
            },
        ) = (left_type, right_type)
        {
            (
                numeric_literals::exact_binary(token, a, b, af.or(*bf), source.span(node_index)).map_err(|reason| TypeError::at(source, node_index, reason))?,
                vec![left_type.clone(), right_type.clone()],
            )
        } else if left_type == &Bool && right_type == &Bool && matches!(token, "&&" | "||" | "==" | "!=")
            || integer(left_type) && integer(right_type) && compare
        {
            (Bool, vec![left_type.clone(), right_type.clone()])
        } else {
            let carrier = if matches!(left_type, Exact { .. }) { right_type } else { left_type };
            let shift = token.starts_with("<<") || token.starts_with(">>");
            let accepted = match token {
                "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" => carrier.numeric(),
                "%" => integer(carrier),
                "&" | "|" | "^" => word(carrier),
                "+%" | "-%" | "*%" | "/%" | "%%" | "+|" | "-|" | "*|" | "/|" | "%|" | "+!" | "-!" | "*!" | "/!" | "%!" => word(carrier),
                _ if shift => word(left_type),
                _ => false,
            };
            if !accepted || !carrier.admits(left_type) || !(carrier.admits(right_type) || shift && integer(right_type)) {
                return Err(bad());
            }
            self.check_destination(left, left_type, carrier)?;
            let count_ty = if shift && integer(right_type) { right_type } else { carrier };
            self.check_destination(right, right_type, count_ty)?;
            (if compare { Bool } else { carrier.clone() }, vec![carrier.clone(), count_ty.clone()])
        };
        if result.numeric() && operands.iter().any(ValueType::numeric) {
            self.record_obligation(
                node_index,
                ObligationKind::OperationContract {
                    token: token.to_owned(),
                    operands: operands.clone(),
                },
            );
            if token.ends_with('!')
                && let Some(machine) = &self.caller
            {
                self.record_obligation(
                    node_index,
                    ObligationKind::CrashCoverage {
                        machine: machine.clone(),
                        cause: crate::CrashCause::Trap,
                    },
                );
            }
        }
        self.record_operator(node_index, token, operands, &result);
        Ok(result)
    }
}
