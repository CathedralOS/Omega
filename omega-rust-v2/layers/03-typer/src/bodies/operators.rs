use super::*;

impl Body<'_, '_> {
    pub(super) fn declared_operator(
        &mut self,
        n: usize,
        token: &str,
        operands: &[(usize, &ValueType)],
    ) -> Result<Option<ValueType>, TypeError> {
        let f = &self.unit.file;
        let Some(family) = self.catalog.token_bindings.get(token) else {
            return Ok(None);
        };
        let collection = matches!(token, "[]" | "[..]");
        let mut selected = None;
        for binding in family {
            let id = &binding.machine;
            let sig = &self.catalog.signatures[id];
            if sig.params.len() != operands.len() || id.package != f.package && !sig.public {
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
                if !sig
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
                if selected.replace((id, sig, args, evidence, owner)).is_some() {
                    return Err(TypeError::at(f, n, format!("ambiguous operator {token}")));
                }
            }
        }
        let Some((id, sig, args, evidence, owner)) = selected else {
            return Ok(None);
        };
        let instantiate = |t: &ValueType| {
            owner
                .as_ref()
                .map_or_else(|| t.clone(), |slot| t.substitute(&slot.declaration, &slot.trait_arguments))
                .substitute(id, &args)
        };
        for (i, ((site, value), (_, param))) in operands.iter().zip(&sig.params).enumerate() {
            let parameter = instantiate(param);
            if i == 0
                && collection
                && let ValueType::Reference(access, inner) = &parameter
                && !matches!(value, ValueType::Reference(..) | ValueType::AtLifetime(..))
            {
                self.destination(*site, value, inner)?;
                self.obligation(
                    n,
                    ObligationKind::Borrow {
                        access: access.clone(),
                        place: Site::at(f, *site),
                    },
                );
            } else {
                self.destination(*site, value, &parameter)?;
            }
        }
        let result = instantiate(&sig.result);
        if sig.requires {
            self.obligation(n, ObligationKind::CallPrecondition(id.clone()));
        }
        if self.call_use(n) == crate::CallUse::Execution {
            self.version = Some(Site::at(f, n));
            self.cases.clear();
        }
        self.output.calls.push(TypedCall {
            use_site: self.call_use(n),
            site: Site::at(f, n),
            machine: id.clone(),
            caller: self.caller.clone(),
            evidence,
            acknowledgements: Default::default(),
            enclosing_arguments: owner
                .into_iter()
                .map(|s| (s.declaration, s.trait_arguments))
                .collect(),
            type_arguments: args,
            operands: operands.iter().map(|(site, _)| Site::at(f, *site)).collect(),
            result: result.clone(),
        });
        Ok(Some(result))
    }
    pub(super) fn record_operator(
        &mut self,
        n: usize,
        token: &str,
        operands: Vec<ValueType>,
        result: &ValueType,
    ) {
        self.output.operations.push(TypedOperation {
            site: Site::at(&self.unit.file, n),
            token: token.into(),
            operands,
            result: result.clone(),
        });
    }
    pub(super) fn binary_chain(
        &mut self,
        mut n: usize,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let mut pending = Vec::new();
        while f.nodes[n].kind == NodeKind::Binary {
            let [left, right] = *f.nodes[n].children.as_slice() else {
                return Err(TypeError::unsupported(f, n));
            };
            if f.nodes[n].leaves.iter().any(|w| w.text == "==") && self.type_term(left) {
                break;
            }
            pending.push((n, left, right));
            n = left;
        }
        let mut lhs = if f.nodes[n].kind == NodeKind::Binary {
            self.type_equality(n)?
        } else {
            self.expression(n)?
        };
        for (n, left, right) in pending.into_iter().rev() {
            let token: String = f.nodes[n].leaves.iter().map(|t| t.text).collect();
            let ty = if token == "in" {
                self.membership(n, &lhs, right)?
            } else {
                let fact = (token == "&&").then(|| self.case_guard(left)).flatten();
                let previous = fact
                    .as_ref()
                    .and_then(|(s, id)| self.cases.insert(*s, id.clone()));
                let rhs = self.expression(right)?;
                if let Some((s, _)) = fact
                    && self.cases.remove(&s).is_some()
                    && let Some(old) = previous
                {
                    self.cases.insert(s, old);
                }
                self.binary(n, &token, (left, &lhs), (right, &rhs))?
            };
            self.output.expressions.push(TypedExpression {
                site: Site::at(f, n),
                ty: ty.clone(),
            });
            lhs = ty;
        }
        Ok(lhs)
    }
    fn binary(
        &mut self,
        n: usize,
        token: &str,
        (left, lhs): (usize, &ValueType),
        (right, rhs): (usize, &ValueType),
    ) -> Result<ValueType, TypeError> {
        use ValueType::*;
        let f = &self.unit.file;
        if let Some(ty) = self.declared_operator(n, token, &[(left, lhs), (right, rhs)])? {
            return Ok(ty);
        }
        let bad = || TypeError::at(f, n, format!("no matching operator {token} for {lhs} and {rhs}"));
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
        ) = (lhs, rhs)
        {
            (
                numeric::exact_binary(token, a, b, af.or(*bf), f.span(n)).map_err(|reason| TypeError::at(f, n, reason))?,
                vec![lhs.clone(), rhs.clone()],
            )
        } else if lhs == &Bool && rhs == &Bool && matches!(token, "&&" | "||" | "==" | "!=") || integer(lhs) && integer(rhs) && compare {
            (Bool, vec![lhs.clone(), rhs.clone()])
        } else {
            let carrier = if matches!(lhs, Exact { .. }) { rhs } else { lhs };
            let shift = token.starts_with("<<") || token.starts_with(">>");
            let accepted = match token {
                "+" | "-" | "*" | "/" | "==" | "!=" | "<" | "<=" | ">" | ">=" => carrier.numeric(),
                "%" => integer(carrier),
                "&" | "|" | "^" => word(carrier),
                "+%" | "-%" | "*%" | "/%" | "%%" | "+|" | "-|" | "*|" | "/|" | "%|" | "+!" | "-!" | "*!" | "/!" | "%!" => word(carrier),
                _ if shift => word(lhs),
                _ => false,
            };
            if !accepted || !carrier.admits(lhs) || !(carrier.admits(rhs) || shift && integer(rhs)) {
                return Err(bad());
            }
            self.destination(left, lhs, carrier)?;
            let count_ty = if shift && integer(rhs) { rhs } else { carrier };
            self.destination(right, rhs, count_ty)?;
            (if compare { Bool } else { carrier.clone() }, vec![carrier.clone(), count_ty.clone()])
        };
        if result.numeric() && operands.iter().any(ValueType::numeric) {
            self.obligation(
                n,
                ObligationKind::OperationContract {
                    token: token.to_owned(),
                    operands: operands.clone(),
                },
            );
            if token.ends_with('!')
                && let Some(machine) = &self.caller
            {
                self.obligation(
                    n,
                    ObligationKind::CrashCoverage {
                        machine: machine.clone(),
                        cause: crate::CrashCause::Trap,
                    },
                );
            }
        }
        self.record_operator(n, token, operands, &result);
        Ok(result)
    }
}
