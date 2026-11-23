use super::call_arguments::{CallOperand, PendingComparisons, SelectedMachine};
use crate::declarations::operator_declarations::nominal_operator_operand;
use crate::evaluation::numeric_literals;
use crate::expressions::body_typer::{BodyTyper, ExpressionContext};
use crate::type_error::SourceTypeError;
use crate::type_relations::TypeRelations;
use crate::type_system::dependent_subjects::DependentSubjects;
use crate::{ObligationKind, TypeError, TypedOperation, ValueType};
use omega_syntax::NodeKind;
use omega_typed::site_at;

impl BodyTyper<'_, '_> {
    pub(super) fn declared_operator(
        &mut self,
        node_index: usize,
        token: &str,
        operands: &[CallOperand],
    ) -> Result<Option<ValueType>, TypeError> {
        let source = self.unit.file;
        enum Candidate<'a> {
            Ready(&'a crate::TypedSignature),
            WaitingForHome(&'a crate::TypedSignature),
            Pending(Option<&'a crate::declarations::declaration_catalog::PendingSignature>, usize),
        }
        let ready = self.catalog.token_bindings.get(token).into_iter().flatten().map(|&id| {
            let signature = &self.catalog.signatures[id];
            let binding = signature
                .operator
                .as_ref()
                .expect("operator family indexes checked declarations");
            (id, binding.public, binding.home.as_ref(), Candidate::Ready(signature))
        });
        let pending = self.catalog.pending_operators.get(token).into_iter().flatten().map(|binding| {
            let candidate = match self.catalog.signatures.get(binding.machine) {
                Some(signature) => Candidate::WaitingForHome(signature),
                None => Candidate::Pending(self.catalog.pending_signatures.get(binding.machine), binding.arity),
            };
            (binding.machine, binding.public, binding.declared_home, candidate)
        });
        let mut selected = Vec::new();
        let indices = &self.catalog.types.indices;
        let mut waiting = false;
        for (id, public, home, candidate) in ready.chain(pending) {
            let visible = if let Some(owner) = &id.enclosing_machine {
                std::iter::successors(self.caller.as_ref(), |caller| caller.enclosing_machine.as_ref()).any(|scope| scope == owner)
            } else {
                id.package == source.package || public
            };
            let arity = match candidate {
                Candidate::Ready(s) | Candidate::WaitingForHome(s) => s.parameters.len(),
                Candidate::Pending(_, arity) => arity,
            };
            if arity != operands.len() || !self.unit.can_select_from(&id.package) || !visible {
                continue;
            }
            let mut contexts = Vec::new();
            let participates = match home {
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
                // The completed parameters may infer a domain home instead of a nominal one.
                None if matches!(candidate, Candidate::Pending(..)) && operands.iter().any(|op| !op.value.domains().is_empty()) => true,
                nominal_home => operands.iter().enumerate().any(|(i, op)| {
                    nominal_operator_operand(token, i, &op.value).is_some_and(|carrier| {
                        nominal_home.map_or_else(|| carrier.package == id.package, |home| carrier == home)
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
                let signature = match candidate {
                    Candidate::Ready(signature) | Candidate::WaitingForHome(signature) => signature,
                    Candidate::Pending(partial, _) => {
                        waiting |= partial.is_none_or(|partial| self.unfinished_call_may_match(source.nodes[node_index].kind, id, partial, operands, capture));
                        continue;
                    }
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
                if matches!(candidate, Candidate::WaitingForHome(_)) {
                    waiting = true;
                    continue;
                }
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
            site: site_at(self.unit.file, node_index),
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
        // The left spine can also pass through groups, as in `((a + b) + c)`.
        // A group is finished at its own node exactly as `type_expression`
        // would finish it; see :ExpressionLayers.
        let mut pending = Vec::new();
        let mut left_type = loop {
            match source.nodes[node_index].kind {
                NodeKind::Binary => {}
                NodeKind::Paren => {
                    pending.push((node_index, None));
                    node_index = self.unit.single_child(node_index)?;
                    continue;
                }
                _ => break self.type_expression(node_index)?,
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
            pending.push((node_index, Some((left, right, operator.token))));
            node_index = left;
        };
        for (node_index, operator) in pending.into_iter().rev() {
            let Some((left, right, token)) = operator else {
                left_type = self.finish_expression(node_index, left_type, None)?;
                continue;
            };
            let ty = if token == "in" {
                self.membership(node_index, &left_type, right)?
            } else {
                let fact = (token == "&&").then(|| self.case_guard(left)).flatten();
                let previous = fact
                    .as_ref()
                    .and_then(|(s, id)| self.cases.insert(s.clone(), id.clone()));
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
            if token.ends_with('!') && matches!(self.context, ExpressionContext::Proof | ExpressionContext::ContractValue) {
                return Err(TypeError::at(source, node_index, "a proof term cannot execute trapping arithmetic"));
            }
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
