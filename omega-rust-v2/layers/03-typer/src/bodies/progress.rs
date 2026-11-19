use super::*;
use omega_resolver::DeclarationId;

impl Body<'_, '_> {
    pub(super) fn crash_route(
        &mut self,
        n: usize,
        contract: usize,
    ) -> Result<(), TypeError> {
        let f = &self.unit.file;
        let (&head, guards) = f.nodes[n]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(f, n))?;
        let cause = match self.unit.symbol(head) {
            Some(Symbol::Builtin(s)) if s == "none" => return Ok(()),
            Some(Symbol::Builtin(s)) if s == "Trap" => crate::CrashCause::Trap,
            Some(Symbol::Builtin(s)) if s == "Abort" => crate::CrashCause::Abort,
            _ => return Err(TypeError::at(f, head, "invalid crash cause")),
        };
        let guard = if let [guard] = *guards {
            if let Some(op) =
                (guard..f.nodes[guard].end).find(|&i| f.nodes[i].kind == NodeKind::Binary && f.nodes[i].leaves.last().is_some_and(|l| l.text.ends_with('!')))
            {
                return Err(TypeError::at(f, op, "a crash guard cannot execute trapping arithmetic"));
            }
            let ty = self.expected(guard, Some(&ValueType::Bool))?;
            self.destination(guard, &ty, &ValueType::Bool)?;
            self.obligation(guard, ObligationKind::TotalCrashGuard);
            Some(Site::at(f, guard))
        } else {
            None
        };
        self.output.crash_routes.push(crate::CrashRoute {
            clause: Site::at(f, n),
            contract: Site::at(f, contract),
            cause,
            guard,
        });
        Ok(())
    }

    pub(super) fn ranking(
        &mut self,
        n: usize,
        machine: &DeclarationId,
    ) -> Result<(), TypeError> {
        let f = &self.unit.file;
        let node = &f.nodes[n];
        self.obligation(n, ObligationKind::Termination(machine.clone()));
        let Some((&subject, rest)) = node.children.split_first() else {
            return Ok(());
        };
        let actual = self.expression(subject)?;
        let explicit = node.leaves.iter().any(|l| l.text == "->");
        let (view, args, bounds) = if explicit {
            let (&term, bounds) = rest.split_first().ok_or_else(|| TypeError::unsupported(f, n))?;
            let (head, args) = if f.nodes[term].kind == NodeKind::Call {
                let [head, args] = *f.nodes[term].children.as_slice() else {
                    return Err(TypeError::unsupported(f, term));
                };
                (head, f.nodes[args].children.as_slice())
            } else {
                (term, &[][..])
            };
            let Some(Symbol::Declaration(id)) = self.unit.symbol(head) else {
                return Err(TypeError::at(f, head, "ranking view requires one exact declaration"));
            };
            (
                self.catalog
                    .ranking_views
                    .iter()
                    .find(|v| &v.declaration == id)
                    .ok_or_else(|| TypeError::at(f, head, "ranking view is absent from the admitted theory"))?,
                args,
                bounds,
            )
        } else {
            let mut choices = self
                .catalog
                .ranking_views
                .iter()
                .filter(|v| v.canonical_default && v.subject == actual);
            let view = choices
                .next()
                .ok_or_else(|| TypeError::at(f, subject, "carrier has no admitted canonical ranking view"))?;
            if choices.next().is_some() {
                return Err(TypeError::at(f, subject, "ambiguous canonical ranking view"));
            }
            (view, &[][..], rest)
        };
        self.destination(subject, &actual, &view.subject)?;
        if view.parameters.len() != args.len() {
            return Err(TypeError::at(f, n, "incorrect ranking view arity"));
        }
        for (&arg, param) in args.iter().zip(&view.parameters) {
            let value = self.expected(arg, Some(param))?;
            self.destination(arg, &value, param)?;
        }
        let mut bound_sites = Vec::new();
        let mut inclusive = false;
        for &bound in bounds {
            let endpoints = if f.nodes[bound].kind == NodeKind::Range {
                inclusive = f.nodes[bound].leaves.iter().any(|l| l.text == "..=");
                f.nodes[bound].children.as_slice()
            } else {
                std::slice::from_ref(&bound)
            };
            for &e in endpoints {
                let value = self.expected(e, Some(&view.rank))?;
                self.destination(e, &value, &view.rank)?;
                bound_sites.push(Site::at(f, e));
            }
        }
        self.output.ranking_witnesses.push(crate::RankingWitness {
            site: Site::at(f, n),
            machine: machine.clone(),
            subject: Site::at(f, subject),
            view: view.declaration.clone(),
            arguments: args.iter().map(|&a| Site::at(f, a)).collect(),
            bounds: bound_sites,
            inclusive_upper: inclusive,
        });
        Ok(())
    }
    pub(super) fn invocation(
        &mut self,
        n: usize,
        machine: &DeclarationId,
    ) -> Result<(), TypeError> {
        let f = &self.unit.file;
        let head = self.unit.single_child(n)?;
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(head) {
            let actual = self.locals.get(span).or_else(|| {
                self.unit
                    .parameters
                    .get(span)
                    .and_then(|p| self.catalog.types.parameter_carrier(p))
            });
            if !matches!(actual, Some(ValueType::Callable(_))) {
                return Err(TypeError::at(f, head, "invokes parameter must be callable"));
            }
            let entry = self.invocation_target(head);
            self.output
                .invocation_ceilings
                .push((machine.clone(), Site::at(f, n), entry));
            return Ok(());
        }
        let Some(Symbol::Declaration(id)) = self.unit.symbol(head) else {
            return Err(TypeError::at(f, head, "invokes needs an exact entry or callback interface"));
        };
        if !matches!(id.kind, NodeKind::Trait | NodeKind::Machine | NodeKind::AsmMachine) {
            return Err(TypeError::at(f, head, "invokes needs an entry or callback interface"));
        }
        let args = f
            .child(head, NodeKind::TypeArgs)
            .into_iter()
            .flat_map(|section| f.nodes[section].children.iter().copied())
            .enumerate()
            .map(|(i, arg)| self.unit.argument(arg, &self.catalog.types, id, i))
            .collect::<Result<Vec<_>, _>>()?;
        if args.len() != self.catalog.types.arities[id] {
            return Err(TypeError::at(f, head, "incomplete invokes application"));
        }
        self.output.invocation_ceilings.push((
            machine.clone(),
            Site::at(f, n),
            crate::InvocationTarget::Declaration(crate::RequirementSlot {
                declaration: id.clone(),
                trait_arguments: args,
            }),
        ));
        Ok(())
    }
}
