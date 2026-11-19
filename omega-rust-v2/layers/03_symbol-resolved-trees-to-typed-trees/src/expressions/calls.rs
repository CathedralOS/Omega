use super::call_arguments::SelectedMachine;
use crate::expressions::body_typer::BodyTyper;
use crate::{TypeError, ValueType};
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
                ValueType::Callable(c) => self.indirect(node_index, callee, args, &c),
                _ => Err(TypeError::at(source, callee, "indirect call requires a callable value")),
            };
        }
        match self.unit.symbol(callee) {
            Some(Symbol::Binding {
                span,
                kind: NodeKind::ConstParam,
                ..
            }) => {
                let c = self.static_callable(*span, callee)?;
                return self.indirect(node_index, callee, args, &c);
            }
            Some(Symbol::Binding {
                kind: NodeKind::Hypothesis | NodeKind::MachineParam,
                ..
            }) => return Err(TypeError::at(source, callee, "logical evidence cannot be executed")),
            Some(Symbol::Binding { span, .. }) => {
                let Some(ValueType::Callable(c)) = self.locals.get(span).cloned() else {
                    return Err(TypeError::at(source, callee, "indirect call requires a callable value"));
                };
                return self.indirect(node_index, callee, args, &c);
            }
            _ => {}
        }
        let mut evidence = None;
        let mut evidence_lineage = None;
        let mut requirements = Vec::new();
        let declarations = match self.unit.symbol(callee) {
            Some(Symbol::StaticMember { binder, member }) => {
                evidence = self.unit.parameters.get(binder).cloned();
                let carrier = evidence
                    .as_ref()
                    .and_then(|parameter| self.catalog.types.parameter_carrier(parameter));
                let Some(ValueType::Evidence(slot)) = carrier else {
                    if member == "ensures" {
                        return self.project_guarantee(node_index, callee, args, *binder);
                    }
                    return Err(TypeError::at(source, callee, "member selection requires a trait evidence binder"));
                };
                let lineage = self.catalog.trait_lineage(slot);
                requirements.extend(
                    lineage
                        .iter()
                        .flat_map(|owner| &self.catalog.requirements[&owner.declaration])
                        .filter(|id| id.path.rsplit("::").next() == Some(member.as_str()))
                        .copied(),
                );
                if requirements.is_empty() {
                    return Err(TypeError::at(source, callee, format!("{} has no requirement {member}", slot.declaration.path)));
                }
                evidence_lineage = Some(lineage);
                &[][..]
            }
            Some(Symbol::Declaration(id)) if matches!(id.kind, NodeKind::Machine | NodeKind::AsmMachine) => std::slice::from_ref(id),
            Some(Symbol::Candidates(ids)) => ids.as_slice(),
            _ => return Err(TypeError::at(source, callee, "unsupported indirect call or non-machine callee")),
        };
        let mut explicit = &[][..];
        for &section in &source.nodes[callee].children {
            let arguments = source.nodes[section].children.as_slice();
            match self.unit.application_name(section).map(|name| &name.target) {
                Some(Symbol::Declaration(id)) if matches!(id.kind, NodeKind::Trait | NodeKind::Conformance) => {}
                Some(Symbol::Binding { .. }) if evidence.is_some() => {
                    return Err(TypeError::at(source, section, "type arguments cannot be applied to the evidence binder"));
                }
                Some(_) => explicit = arguments,
                None => return Err(TypeError::unsupported(source, section)),
            }
        }
        let enclosing_arguments = if let Some(lineage) = evidence_lineage {
            lineage
                .into_iter()
                .map(|slot| (slot.declaration, slot.trait_arguments))
                .collect()
        } else {
            self.unit.receiver_arguments(callee, self.catalog)?
        };
        let capture = |t: &ValueType| t.in_enclosing_scopes(&enclosing_arguments);
        let mut prepared = Vec::new();
        let mut argument_error = None;
        let candidate_count = declarations.len() + requirements.len();
        for id in declarations.iter().chain(requirements) {
            let Some(signature) = self.catalog.signatures.get(id) else { continue };
            if signature.parameters.len() != source.nodes[args].children.len() {
                continue;
            }
            let formals = match self
                .unit
                .applied_formals(callee, id, explicit, &self.catalog.types, capture)
            {
                Ok(formals) => formals,
                Err(e) => {
                    argument_error = Some(e);
                    continue;
                }
            };
            let result = capture(&signature.result).substitute(id, &formals);
            let wanted = expected.map_or(&[][..], ValueType::domains);
            // An operand can determine the entire return type or an index in a
            // declared result domain. Keep that candidate until its operands bind.
            let domain_destination = expected.filter(|ty| result.can_infer_result_domains(id, ty));
            if domain_destination.is_none() && result.domains() != wanted {
                continue;
            }
            prepared.push((id, signature, formals, domain_destination));
        }
        let mut operands = Vec::new();
        let mut subjects = Vec::new();
        for (position, &arg) in source.nodes[args].children.iter().enumerate() {
            let mut parameters = prepared.iter().map(|(id, signature, formals, _)| {
                let ty = capture(&signature.parameters[position].1)
                    .substitute(id, formals)
                    .instantiate_subjects(signature, &subjects);
                (explicit.len() == formals.len() || !ty.contains_parameter()).then_some(ty)
            });
            let common = parameters
                .next()
                .flatten()
                .filter(|ty| parameters.all(|other| other.as_ref() == Some(ty)));
            let operand = self.argument_operand(arg, common.as_ref())?;
            subjects.push(operand.subject(self.unit, common.as_ref().unwrap_or(&operand.value)));
            operands.push(operand);
        }
        let mut matches = Vec::new();
        for (id, signature, formals, domain_destination) in prepared {
            let arity = formals.len();
            let instantiate = |t: &ValueType| {
                capture(t)
                    .substitute(id, &formals)
                    .instantiate_subjects(signature, &subjects)
            };
            let mut bindings = vec![None; arity];
            for (i, ty) in formals[..explicit.len()].iter().enumerate() {
                bindings[i] = Some(ty.clone());
            }
            if signature
                .parameters
                .iter()
                .zip(&operands)
                .enumerate()
                .all(|(i, ((_, p), op))| {
                    let formal = instantiate(p);
                    self.infer_operand(NodeKind::Call, i, op, &formal, id, &mut bindings)
                        || (candidate_count == 1
                            && self
                                .catalog
                                .types
                                .indices
                                .defer_indexed_landing(&op.value, &formal))
                })
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
            } else if candidate_count == 1 {
                let mut inferred = vec![None; arity];
                if signature
                    .parameters
                    .iter()
                    .zip(&operands)
                    .enumerate()
                    .all(|(i, ((_, p), op))| self.infer_operand(NodeKind::Call, i, op, &capture(p), id, &mut inferred))
                {
                    for (i, ty) in formals[..explicit.len()].iter().enumerate() {
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
                .map(|op| op.value.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(TypeError::at(source, callee, format!("no matching overload for {name}({types})")));
        };
        if matches.next().is_some() {
            return Err(TypeError::at(source, callee, format!("ambiguous call to {name}")));
        }
        self.unit.check_signature_selection(callee, id)?;
        if let Some(expected) = expected {
            capture(&signature.result).infer(expected, id, &mut bindings);
        }
        let bindings = bindings
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| TypeError::at(source, callee, "cannot infer all generic arguments"))?;
        self.apply_machine(
            node_index,
            SelectedMachine {
                machine: id,
                explicit_arguments: explicit,
                arguments: bindings,
                enclosing: enclosing_arguments,
                evidence,
            },
            &operands,
        )
    }
}
