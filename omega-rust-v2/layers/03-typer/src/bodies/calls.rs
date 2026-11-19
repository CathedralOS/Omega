use super::*;

impl Body<'_, '_> {
    fn infer_argument(
        param: &ValueType,
        actual: &ValueType,
        id: &omega_resolver::DeclarationId,
        bindings: &mut [Option<ValueType>],
    ) -> bool {
        if let ValueType::Reference(Access::Shared, inner) = param
            && !matches!(actual, ValueType::Reference(..) | ValueType::AtLifetime(..))
        {
            return inner.infer(actual, id, bindings);
        }
        param.infer(actual, id, bindings)
    }
    pub(super) fn admit_argument(
        &mut self,
        n: usize,
        actual: &ValueType,
        param: &ValueType,
    ) -> Result<(), TypeError> {
        if let ValueType::Reference(Access::Shared, inner) = param
            && !matches!(actual, ValueType::Reference(..) | ValueType::AtLifetime(..))
        {
            self.destination(n, actual, inner)?;
            self.obligation(
                n,
                ObligationKind::Borrow {
                    access: Access::Shared,
                    place: Site::at(&self.unit.file, n),
                },
            );
            return Ok(());
        }
        self.destination(n, actual, param)
    }
    pub(super) fn validate_call_bindings(
        &mut self,
        n: usize,
        callee: usize,
        id: &omega_resolver::DeclarationId,
        bindings: &[ValueType],
    ) -> Result<(), TypeError> {
        let f = &self.unit.file;
        for (index, ty) in bindings.iter().enumerate() {
            if let Some(formal @ ValueType::Evidence(_)) = self.catalog.types.bound(id, index)
                && let ValueType::Evidence(slot) = formal.substitute(id, bindings)
            {
                let valid = match ty {
                    ValueType::Implementation(implementation) => self
                        .catalog
                        .implementations
                        .get(implementation.as_ref())
                        .is_some_and(|s| self.catalog.admits_conformance(s, &slot)),
                    ValueType::Parameter { .. } => {
                        matches!(self.catalog.types.parameter_carrier(ty),Some(ValueType::Evidence(other)) if self.catalog.trait_lineage(other).contains(&slot))
                    }
                    _ => false,
                };
                if valid
                    && let ValueType::Implementation(selected) = ty
                    && self
                        .catalog
                        .trait_lineage(&slot)
                        .iter()
                        .any(|s| self.catalog.refinements.contains_key(&s.declaration))
                {
                    self.obligation(
                        n,
                        ObligationKind::RefineEvidence {
                            binder: (*slot).clone(),
                            implementation: (**selected).clone(),
                        },
                    );
                }
                if !valid {
                    return Err(TypeError::at(f, callee, "selected evidence has the wrong trait application"));
                }
            }
            if let Some(formal @ ValueType::Callable(_)) = self.catalog.types.bound(id, index)
                && let ValueType::Callable(c) = formal.substitute(id, bindings)
            {
                match ty {
                    ValueType::MachineSelection(m) => self.refine_callable(n, &c, m)?,
                    ValueType::Parameter { .. } if self.catalog.types.parameter_carrier(ty) == Some(&ValueType::Callable(c)) => {}
                    _ => return Err(TypeError::at(f, callee, "forwarded callback has incompatible requirements")),
                }
            }
            self.catalog
                .types
                .validate_bound(id, index, ty)
                .map_err(|m| TypeError::at(f, callee, m))?;
        }
        Ok(())
    }
    pub(super) fn call(
        &mut self,
        n: usize,
        expected: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let [callee, args] = *f.nodes[n].children.as_slice() else {
            return Err(TypeError::unsupported(f, n));
        };
        if let Some(Symbol::Declaration(id)) = self.unit.symbol(callee)
            && id.kind == NodeKind::MathLet
        {
            return self.mathematical_application(n, callee, Some(id), self.catalog.mathematical[id].clone(), &f.nodes[args].children, expected);
        }
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(callee)
            && let Some(ty @ ValueType::Function(..)) = self.locals.get(span).cloned()
        {
            return self.mathematical_application(n, callee, None, ty, &f.nodes[args].children, expected);
        }
        if f.nodes[callee].kind != NodeKind::PathExpression {
            return match self.expression(callee)? {
                ty @ ValueType::Function(..) => self.mathematical_application(n, callee, None, ty, &f.nodes[args].children, expected),
                ValueType::Callable(c) => self.indirect(n, callee, args, &c, false),
                _ => Err(TypeError::at(f, callee, "indirect call requires a callable value")),
            };
        }
        match self.unit.symbol(callee) {
            Some(Symbol::StaticGuarantee { binder }) => {
                let c = self.static_callable(*binder, callee)?;
                return self.indirect(n, callee, args, &c, true);
            }
            Some(Symbol::Binding {
                span,
                kind: NodeKind::ConstParam,
            }) => {
                let c = self.static_callable(*span, callee)?;
                return self.indirect(n, callee, args, &c, false);
            }
            Some(Symbol::Binding { span, .. }) => {
                let Some(ValueType::Callable(c)) = self.locals.get(span).cloned() else {
                    return Err(TypeError::at(f, callee, "indirect call requires a callable value"));
                };
                return self.indirect(n, callee, args, &c, false);
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
            _ => return Err(TypeError::at(f, callee, "unsupported indirect call or non-machine callee")),
        };
        let mut operands = Vec::new();
        for (position, &arg) in f.nodes[args].children.iter().enumerate() {
            // A common concrete parameter is already a destination for a nested
            // call or aggregate, even while the outer overload is unresolved.
            let mut parameter_types = candidates
                .iter()
                .filter_map(|id| self.catalog.signatures.get(id))
                .filter(|sig| sig.params.len() == f.nodes[args].children.len() && sig.result.domains() == expected.map_or(&[][..], ValueType::domains))
                .map(|sig| &sig.params[position].1);
            let common = parameter_types
                .next()
                .filter(|ty| !ty.contains_parameter() && parameter_types.all(|other| other == *ty));
            operands.push((arg, self.expected(arg, common)?));
        }
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
        for &section in &f.nodes[callee].children {
            let Some(symbol) = self.unit.application_segment(callee, section) else {
                return Err(TypeError::unsupported(f, section));
            };
            match symbol {
                Symbol::Declaration(owner) if owner.kind == NodeKind::Trait || owner.kind == NodeKind::Conformance => {
                    let arguments = f.nodes[section]
                        .children
                        .iter()
                        .enumerate()
                        .map(|(i, &arg)| self.unit.argument(arg, &self.catalog.types, owner, i))
                        .collect::<Result<Vec<_>, _>>()?;
                    if self.catalog.types.arities[owner] != arguments.len() {
                        return Err(TypeError::at(f, section, "incorrect enclosing application arity"));
                    }
                    enclosing_arguments.push((owner.clone(), arguments));
                }
                _ => explicit = f.nodes[section].children.as_slice(),
            }
        }
        let capture = |t: &ValueType| {
            enclosing_arguments
                .iter()
                .fold(t.clone(), |ty, (owner, args)| ty.substitute(owner, args))
        };
        let mut matches = Vec::new();
        let mut argument_error = None;
        for id in candidates {
            let Some(signature) = self.catalog.signatures.get(id) else { continue };
            if signature.params.len() != operands.len() {
                continue;
            }
            let arity = self.catalog.types.arities[id];
            if explicit.len() > arity {
                argument_error = Some(TypeError::at(f, callee, "excess generic arguments"));
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
            let instantiate = |t: &ValueType| capture(t).substitute(id, &formals);
            if instantiate(&signature.result).domains() != expected.map_or(&[][..], ValueType::domains) {
                continue;
            }
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
                            argument_error = Some(TypeError::at(f, explicit[i], format!("conflicting type argument {ty}")));
                            break;
                        }
                    }
                }
            }
        }
        let mut matches = matches.into_iter();
        let name = f.nodes[callee].path();
        let Some((id, signature, mut bindings)) = matches.next() else {
            if let Some(e) = argument_error {
                return Err(e);
            }
            let types = operands
                .iter()
                .map(|(_, t)| t.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            return Err(TypeError::at(f, callee, format!("no matching overload for {name}({types})")));
        };
        if matches.next().is_some() {
            return Err(TypeError::at(f, callee, format!("ambiguous call to {name}")));
        }
        if let Some(expected) = expected {
            capture(&signature.result).infer(expected, id, &mut bindings);
        }
        let bindings = bindings
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| TypeError::at(f, callee, "cannot infer all generic arguments"))?;
        self.validate_call_bindings(n, callee, id, &bindings)?;
        for ((arg, value), (_, param)) in operands.iter().zip(&signature.params) {
            self.admit_argument(*arg, value, &capture(param).substitute(id, &bindings))?;
        }
        let result = capture(&signature.result).substitute(id, &bindings);
        if signature.requires {
            self.obligation(n, ObligationKind::CallPrecondition(id.clone()));
        }
        self.version = Some(Site::at(f, n));
        self.cases.clear();
        self.output.calls.push(TypedCall {
            site: Site::at(f, n),
            machine: id.clone(),
            result: result.clone(),
            operands: operands.iter().map(|(site, _)| Site::at(f, *site)).collect(),
            type_arguments: bindings,
            enclosing_arguments,
            caller: self.caller.clone(),
            evidence,
            acknowledgements: crate::effects::markers(self.unit, n)?,
        });
        Ok(result)
    }
}
