use crate::contracts::effect_clauses;
use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, Site, TypeError, TypedCall, ValueType};
use omega_parser::NodeKind;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    pub(super) fn type_call(
        &mut self,
        node_index: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let [callee, args] = *source.nodes[node_index].children.as_slice() else {
            return Err(TypeError::unsupported(source, node_index));
        };
        if let Some(Symbol::Declaration(id)) = self.unit.symbol(callee)
            && id.kind == NodeKind::MathLet
        {
            return self.mathematical_application(
                node_index,
                callee,
                Some(id),
                self.catalog.types.mathematical[id].clone(),
                &source.nodes[args].children,
                expected,
            );
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(callee)
            && let Some(ty @ ValueType::Function(..)) = self
                .locals
                .get(span)
                .or_else(|| {
                    self.unit
                        .parameters
                        .get(span)
                        .and_then(|p| self.catalog.types.parameter_carrier(p))
                })
                .cloned()
        {
            return self.mathematical_application(node_index, callee, None, ty, &source.nodes[args].children, expected);
        }
        if source.nodes[callee].kind != NodeKind::PathExpression {
            return match self.type_expression(callee)? {
                ty @ ValueType::Function(..) => self.mathematical_application(node_index, callee, None, ty, &source.nodes[args].children, expected),
                ValueType::Callable(c) => self.indirect(node_index, callee, args, &c, false),
                _ => Err(TypeError::at(source, callee, "indirect call requires a callable value")),
            };
        }
        match self.unit.symbol(callee) {
            Some(Symbol::StaticGuarantee { binder }) => {
                if let Some(site) = self.unit.hypotheses.get(binder) {
                    if !source
                        .enclosing_contexts(node_index)
                        .any(|p| matches!(source.nodes[p].kind, NodeKind::Requires | NodeKind::Ensures | NodeKind::MathLet))
                    {
                        return Err(TypeError::at(source, callee, "logical evidence cannot be executed"));
                    }
                    let requirement = &self.catalog.hypotheses[site];
                    let mut parameters: Vec<_> = requirement
                        .parameters
                        .iter()
                        .map(|(_, ty)| ty.clone())
                        .collect();
                    if requirement.result != ValueType::NoResult {
                        parameters.push(requirement.result.clone());
                    }
                    if requirement.conclusions.is_empty() || parameters.len() != source.nodes[args].children.len() {
                        return Err(TypeError::at(
                            source,
                            node_index,
                            "logical guarantee needs its declared subjects and conclusion",
                        ));
                    }
                    for (&arg, ty) in source.nodes[args].children.iter().zip(&parameters) {
                        let actual = self.type_expression_with_expected_type(arg, Some(ty))?;
                        self.check_destination(arg, &actual, ty)?;
                    }
                    self.record_obligation(
                        node_index,
                        ObligationKind::ApplyHypothesis {
                            requirement: site.clone(),
                            operands: source.nodes[args]
                                .children
                                .iter()
                                .map(|&a| Site::at(source, a))
                                .collect(),
                        },
                    );
                    return Ok(ValueType::Bool);
                }
                let c = self.static_callable(*binder, callee)?;
                return self.indirect(node_index, callee, args, &c, true);
            }
            Some(Symbol::Binding {
                span,
                kind: NodeKind::ConstParam,
            }) => {
                let c = self.static_callable(*span, callee)?;
                return self.indirect(node_index, callee, args, &c, false);
            }
            Some(Symbol::Binding {
                kind: NodeKind::Hypothesis | NodeKind::MachineParam,
                ..
            }) => return Err(TypeError::at(source, callee, "logical evidence cannot be executed")),
            Some(Symbol::Binding { span, .. }) => {
                let Some(ValueType::Callable(c)) = self.locals.get(span).cloned() else {
                    return Err(TypeError::at(source, callee, "indirect call requires a callable value"));
                };
                return self.indirect(node_index, callee, args, &c, false);
            }
            _ => {}
        }
        let evidence = match self.unit.symbol(callee) {
            Some(Symbol::EvidenceMember { binder, .. }) => self.unit.parameters.get(binder).cloned(),
            _ => None,
        };
        let candidates = match self.unit.symbol(callee) {
            Some(Symbol::EvidenceMember { requirements, .. }) => requirements.as_slice(),
            Some(Symbol::Declaration(id)) if matches!(id.kind, NodeKind::Machine | NodeKind::AsmMachine) => std::slice::from_ref(id),
            Some(Symbol::Candidates(ids)) => ids.as_slice(),
            _ => return Err(TypeError::at(source, callee, "unsupported indirect call or non-machine callee")),
        };
        let mut explicit = &[][..];
        let mut enclosing_arguments = Vec::new();
        if let Some(value) = &evidence
            && let Some(ValueType::Evidence(slot)) = self.catalog.types.parameter_carrier(value)
        {
            enclosing_arguments.extend(
                self.catalog
                    .trait_lineage(slot)
                    .into_iter()
                    .map(|s| (s.declaration, s.trait_arguments)),
            );
        }
        for &section in &source.nodes[callee].children {
            let Some(symbol) = self.unit.application_segment(callee, section) else {
                return Err(TypeError::unsupported(source, section));
            };
            match symbol {
                Symbol::Declaration(owner) if owner.kind == NodeKind::Trait || owner.kind == NodeKind::Conformance => {
                    let arguments = source.nodes[section]
                        .children
                        .iter()
                        .enumerate()
                        .map(|(i, &arg)| self.unit.argument(arg, &self.catalog.types, owner, i))
                        .collect::<Result<Vec<_>, _>>()?;
                    if self.catalog.types.arities[owner] != arguments.len() {
                        return Err(TypeError::at(source, section, "incorrect enclosing application arity"));
                    }
                    enclosing_arguments.push((owner.clone(), arguments));
                }
                _ => explicit = source.nodes[section].children.as_slice(),
            }
        }
        if let Some(Symbol::Declaration(owner)) = self.unit.qualifier(callee)
            && owner.kind == NodeKind::Trait
            && self.catalog.trait_parents.contains_key(owner)
        {
            let supplied = enclosing_arguments
                .iter()
                .find(|(decl, _)| decl == owner)
                .map(|(_, args)| args.clone())
                .unwrap_or_default();
            if supplied.len() != self.catalog.types.arities[owner] {
                return Err(TypeError::at(source, callee, "incorrect enclosing application arity"));
            }
            let slot = crate::RequirementSlot {
                declaration: owner.clone(),
                trait_arguments: supplied,
            };
            enclosing_arguments.extend(
                self.catalog
                    .trait_lineage(&slot)
                    .into_iter()
                    .skip(1)
                    .map(|s| (s.declaration, s.trait_arguments)),
            );
        }
        let capture = |t: &ValueType| {
            enclosing_arguments
                .iter()
                .fold(t.clone(), |ty, (owner, args)| ty.substitute(owner, args))
        };
        let mut prepared = Vec::new();
        let mut argument_error = None;
        for id in candidates {
            let Some(signature) = self.catalog.signatures.get(id) else { continue };
            if signature.params.len() != source.nodes[args].children.len() {
                continue;
            }
            let arity = self.catalog.types.arities[id];
            if explicit.len() > arity {
                argument_error = Some(TypeError::at(source, callee, "excess generic arguments"));
                continue;
            }
            let mut formals = self.catalog.types.formals(id);
            let parsed = explicit
                .iter()
                .enumerate()
                .map(|(i, &arg)| self.unit.argument(arg, &self.catalog.types, id, i))
                .collect::<Result<Vec<_>, _>>();
            let explicit_values = match parsed {
                Ok(args) => args,
                Err(e) => {
                    argument_error = Some(e);
                    continue;
                }
            };
            formals[..explicit_values.len()].clone_from_slice(&explicit_values);
            let result = capture(&signature.result).substitute(id, &formals);
            let wanted = expected.map_or(&[][..], ValueType::domains);
            // An operand can determine the entire return type or an index in a
            // declared result domain. Keep that candidate until its operands bind.
            let domain_destination = expected.filter(|ty| result.can_infer_result_domains(id, ty));
            if domain_destination.is_none() && result.domains() != wanted {
                continue;
            }
            prepared.push((id, signature, formals, explicit_values, domain_destination));
        }
        let mut operands = Vec::new();
        let mut subjects = Vec::new();
        for (position, &arg) in source.nodes[args].children.iter().enumerate() {
            let mut parameters = prepared.iter().map(|(id, signature, formals, explicit, _)| {
                let ty = capture(&signature.params[position].1)
                    .substitute(id, formals)
                    .instantiate_subjects(signature, &subjects);
                (explicit.len() == formals.len() || !ty.contains_parameter()).then_some(ty)
            });
            let common = parameters
                .next()
                .flatten()
                .filter(|ty| parameters.all(|other| other.as_ref() == Some(ty)));
            let value = self.type_expression_with_expected_type(arg, common.as_ref())?;
            let version = match self.unit.symbol(arg) {
                Some(Symbol::Binding { span, .. }) if self.writable.contains(span) => self.version.as_ref(),
                _ => None,
            };
            subjects.push(
                self.unit
                    .term_subject(arg, common.as_ref().unwrap_or(&value), version),
            );
            operands.push((arg, value));
        }
        let mut matches = Vec::new();
        for (id, signature, formals, explicit_values, domain_destination) in prepared {
            let arity = formals.len();
            let instantiate = |t: &ValueType| {
                capture(t)
                    .substitute(id, &formals)
                    .instantiate_subjects(signature, &subjects)
            };
            let mut bindings = vec![None; arity];
            for (i, ty) in explicit_values.iter().enumerate() {
                bindings[i] = Some(ty.clone());
            }
            if signature
                .params
                .iter()
                .zip(&operands)
                .all(|((_, p), (_, v))| Self::infer_argument(&instantiate(p), v, id, &mut bindings))
            {
                if let Some(destination) = domain_destination {
                    let result = capture(&signature.result).instantiate_subjects(signature, &subjects);
                    result.infer(destination, id, &mut bindings);
                    let mut arguments = formals;
                    for (argument, bound) in arguments.iter_mut().zip(&bindings) {
                        if let Some(bound) = bound {
                            argument.clone_from(bound);
                        }
                    }
                    if result.substitute(id, &arguments).domains() != destination.domains() {
                        continue;
                    }
                }
                matches.push((id, signature, bindings));
            } else if candidates.len() == 1 {
                let mut inferred = vec![None; arity];
                if signature
                    .params
                    .iter()
                    .zip(&operands)
                    .all(|((_, p), (_, v))| Self::infer_argument(&capture(p), v, id, &mut inferred))
                {
                    for (i, ty) in explicit_values.iter().enumerate() {
                        if inferred[i].as_ref().is_some_and(|v| v != ty) {
                            argument_error = Some(TypeError::at(source, explicit[i], format!("conflicting type argument {ty}")));
                            break;
                        }
                    }
                }
            }
        }
        let mut matches = matches.into_iter();
        let name = source.nodes[callee].path();
        let Some((id, signature, mut bindings)) = matches.next() else {
            if let Some(e) = argument_error {
                return Err(e);
            }
            let types = operands
                .iter()
                .map(|(_, t)| t.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(TypeError::at(source, callee, format!("no matching overload for {name}({types})")));
        };
        if matches.next().is_some() {
            return Err(TypeError::at(source, callee, format!("ambiguous call to {name}")));
        }
        if let Some(expected) = expected {
            capture(&signature.result).infer(expected, id, &mut bindings);
        }
        let bindings = bindings
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| TypeError::at(source, callee, "cannot infer all generic arguments"))?;
        self.validate_call_bindings(node_index, callee, id, &bindings)?;
        for ((arg, value), (_, param)) in operands.iter().zip(&signature.params) {
            self.admit_argument(
                *arg,
                value,
                &capture(param)
                    .substitute(id, &bindings)
                    .instantiate_subjects(signature, &subjects),
            )?;
        }
        let result = capture(&signature.result)
            .substitute(id, &bindings)
            .instantiate_subjects(signature, &subjects);
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
            result: result.clone(),
            operands: operands
                .iter()
                .map(|(site, _)| Site::at(source, *site))
                .collect(),
            type_arguments: bindings,
            enclosing_arguments,
            caller: self.caller.clone(),
            evidence,
            acknowledgements: effect_clauses::markers(self.unit, node_index)?,
        });
        Ok(result)
    }
}
