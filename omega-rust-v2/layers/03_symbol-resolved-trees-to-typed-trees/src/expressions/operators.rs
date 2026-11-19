use crate::evaluation::numeric_literals;
use crate::expressions::body_typer::BodyTyper;
use crate::{Access, ObligationKind, Site, TypeError, TypedCall, TypedExpression, TypedOperation, ValueType};
use omega_parser::NodeKind;

impl BodyTyper<'_, '_> {
    pub(super) fn declared_operator(
        &mut self,
        node_index: usize,
        token: &str,
        operands: &[(usize, &ValueType)],
    ) -> Result<Option<ValueType>, TypeError> {
        let source = self.unit.file;
        let Some(family) = self.catalog.token_bindings.get(token) else {
            return Ok(None);
        };
        let collection = matches!(token, "[]" | "[..]");
        let mut selected = None;
        for binding in family {
            let id = &binding.machine;
            let signature = &self.catalog.signatures[id];
            if signature.params.len() != operands.len() || id.package != source.package && !signature.public {
                continue;
            }
            let mut contexts = Vec::new();
            let participates = match &binding.home {
                Some(home) if home.kind == NodeKind::Domain => operands
                    .iter()
                    .any(|(_, v)| v.domains().iter().any(|d| &d.declaration == home)),
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
                Some(_) => false,
                None => operands.iter().enumerate().any(|(i, (_, v))| {
                    let carrier = if i == 0
                        && collection
                        && let ValueType::Reference(_, t) = v
                    {
                        t.as_ref()
                    } else {
                        *v
                    };
                    carrier
                        .nominal_parts()
                        .is_some_and(|(carrier, _)| carrier.package == id.package)
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
                let mut args = vec![None; self.catalog.types.arities[id]];
                if !signature
                    .params
                    .iter()
                    .zip(operands)
                    .enumerate()
                    .all(|(i, ((_, t), (site, v)))| {
                        let param = capture(t);
                        if i == 0
                            && collection
                            && let ValueType::Reference(access, inner) = &param
                            && !matches!(v, ValueType::Reference(..) | ValueType::AtLifetime(..))
                        {
                            return *access != Access::Write
                                && (*access != Access::Mut || self.require_writable(*site).is_ok())
                                && inner.infer(v, id, &mut args);
                        }
                        param.infer(v, id, &mut args)
                    })
                {
                    continue;
                }
                let Some(args) = args.into_iter().collect::<Option<Vec<_>>>() else {
                    continue;
                };
                if selected
                    .replace((id, signature, args, evidence, owner))
                    .is_some()
                {
                    return Err(TypeError::at(source, node_index, format!("ambiguous operator {token}")));
                }
            }
        }
        let Some((id, signature, args, evidence, owner)) = selected else {
            return Ok(None);
        };
        let instantiate = |t: &ValueType| {
            owner
                .as_ref()
                .map_or_else(|| t.clone(), |slot| t.substitute(&slot.declaration, &slot.trait_arguments))
                .substitute(id, &args)
        };
        for (i, ((site, value), (_, param))) in operands.iter().zip(&signature.params).enumerate() {
            let parameter = instantiate(param);
            if i == 0
                && collection
                && let ValueType::Reference(access, inner) = &parameter
                && !matches!(value, ValueType::Reference(..) | ValueType::AtLifetime(..))
            {
                self.check_destination(*site, value, inner)?;
                self.record_obligation(
                    node_index,
                    ObligationKind::Borrow {
                        access: access.clone(),
                        place: Site::at(source, *site),
                    },
                );
            } else {
                self.check_destination(*site, value, &parameter)?;
            }
        }
        let result = instantiate(&signature.result);
        if signature.requires {
            self.record_obligation(node_index, ObligationKind::CallPrecondition(id.clone()));
        }
        if self.call_use(node_index) == crate::CallUse::Execution {
            self.version = Some(Site::at(source, node_index));
            self.cases.clear();
        }
        self.output.calls.push(TypedCall {
            node: source.node_id(node_index),
            use_site: self.call_use(node_index),
            site: Site::at(source, node_index),
            machine: id.clone(),
            caller: self.caller.clone(),
            evidence,
            acknowledgements: Default::default(),
            enclosing_arguments: owner
                .into_iter()
                .map(|s| (s.declaration, s.trait_arguments))
                .collect(),
            type_arguments: args,
            operands: operands.iter().map(|(node, _)| source.node_id(*node)).collect(),
            result: result.clone(),
        });
        Ok(Some(result))
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
                .operator(node_index)
                .ok_or_else(|| TypeError::at(source, node_index, "missing resolved operator"))?;
            match operator.role {
                omega_resolver::OperatorRole::TypeEquality { left, right } => {
                    break self.type_equality(node_index, left, right)?;
                }
                omega_resolver::OperatorRole::Value => {
                    let [left, right] = *source.nodes[node_index].children.as_slice() else {
                        return Err(TypeError::unsupported(source, node_index));
                    };
                    pending.push((node_index, left, right, operator.token.as_str()));
                    node_index = left;
                }
            }
        };
        for (node_index, left, right, token) in pending.into_iter().rev() {
            let ty = if token == "in" {
                self.membership(node_index, &left_type, right)?
            } else {
                let fact = (token == "&&").then(|| self.case_guard(left)).flatten();
                let previous = fact
                    .as_ref()
                    .and_then(|(s, id)| self.cases.insert(*s, id.clone()));
                let right_type = self.type_expression(right)?;
                if let Some((s, _)) = fact
                    && self.cases.remove(&s).is_some()
                    && let Some(old) = previous
                {
                    self.cases.insert(s, old);
                }
                self.binary(node_index, token, (left, &left_type), (right, &right_type))?
            };
            self.output.expressions.push(TypedExpression {
                node: source.node_id(node_index),
                site: Site::at(source, node_index),
                ty: ty.clone(),
            });
            left_type = ty;
        }
        Ok(left_type)
    }
    fn binary(
        &mut self,
        node_index: usize,
        token: &str,
        (left, left_type): (usize, &ValueType),
        (right, right_type): (usize, &ValueType),
    ) -> Result<ValueType, TypeError> {
        use ValueType::*;
        let source = self.unit.file;
        if let Some(ty) = self.declared_operator(node_index, token, &[(left, left_type), (right, right_type)])? {
            return Ok(ty);
        }
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
