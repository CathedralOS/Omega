use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, Site, TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::DeclarationId;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    pub(super) fn crash_route(
        &mut self,
        node_index: usize,
        contract: usize,
    ) -> Result<(), TypeError> {
        let source = &self.unit.file;
        let (&head, guards) = source.nodes[node_index]
            .children
            .split_first()
            .ok_or_else(|| TypeError::unsupported(source, node_index))?;
        let cause = match self.unit.symbol(head) {
            Some(Symbol::Builtin(s)) if s == "none" => return Ok(()),
            Some(Symbol::Builtin(s)) if s == "Trap" => crate::CrashCause::Trap,
            Some(Symbol::Builtin(s)) if s == "Abort" => crate::CrashCause::Abort,
            _ => return Err(TypeError::at(source, head, "invalid crash cause")),
        };
        let guard = if let [guard] = *guards {
            if let Some(op) = (guard..source.nodes[guard].end).find(|&i| {
                source.nodes[i].kind == NodeKind::Binary
                    && source.nodes[i]
                        .leaves
                        .last()
                        .is_some_and(|l| l.text.ends_with('!'))
            }) {
                return Err(TypeError::at(source, op, "a crash guard cannot execute trapping arithmetic"));
            }
            let ty = self.type_expression_with_expected_type(guard, Some(&ValueType::Bool))?;
            self.check_destination(guard, &ty, &ValueType::Bool)?;
            self.record_obligation(guard, ObligationKind::TotalCrashGuard);
            Some(Site::at(source, guard))
        } else {
            None
        };
        self.output.crash_routes.push(crate::CrashRoute {
            clause: Site::at(source, node_index),
            contract: Site::at(source, contract),
            cause,
            guard,
        });
        Ok(())
    }

    pub(super) fn ranking(
        &mut self,
        node_index: usize,
        machine: &DeclarationId,
    ) -> Result<(), TypeError> {
        let source = &self.unit.file;
        let node = &source.nodes[node_index];
        self.record_obligation(node_index, ObligationKind::Termination(machine.clone()));
        let Some((&subject, rest)) = node.children.split_first() else {
            return Ok(());
        };
        let actual = self.type_expression(subject)?;
        let explicit = node.leaves.iter().any(|l| l.text == "->");
        let (view, args, bounds) = if explicit {
            let (&term, bounds) = rest
                .split_first()
                .ok_or_else(|| TypeError::unsupported(source, node_index))?;
            let (head, args) = if source.nodes[term].kind == NodeKind::Call {
                let [head, args] = *source.nodes[term].children.as_slice() else {
                    return Err(TypeError::unsupported(source, term));
                };
                (head, source.nodes[args].children.as_slice())
            } else {
                (term, &[][..])
            };
            let Some(Symbol::Declaration(id)) = self.unit.symbol(head) else {
                return Err(TypeError::at(source, head, "ranking view requires one exact declaration"));
            };
            (
                self.catalog
                    .ranking_views
                    .iter()
                    .find(|v| &v.declaration == id)
                    .ok_or_else(|| TypeError::at(source, head, "ranking view is absent from the admitted theory"))?,
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
                .ok_or_else(|| TypeError::at(source, subject, "carrier has no admitted canonical ranking view"))?;
            if choices.next().is_some() {
                return Err(TypeError::at(source, subject, "ambiguous canonical ranking view"));
            }
            (view, &[][..], rest)
        };
        self.check_destination(subject, &actual, &view.subject)?;
        if view.parameters.len() != args.len() {
            return Err(TypeError::at(source, node_index, "incorrect ranking view arity"));
        }
        for (&arg, param) in args.iter().zip(&view.parameters) {
            let value = self.type_expression_with_expected_type(arg, Some(param))?;
            self.check_destination(arg, &value, param)?;
        }
        let mut bound_sites = Vec::new();
        let mut inclusive = false;
        for &bound in bounds {
            let endpoints = if source.nodes[bound].kind == NodeKind::Range {
                inclusive = source.nodes[bound].leaves.iter().any(|l| l.text == "..=");
                source.nodes[bound].children.as_slice()
            } else {
                std::slice::from_ref(&bound)
            };
            for &e in endpoints {
                let value = self.type_expression_with_expected_type(e, Some(&view.rank))?;
                self.check_destination(e, &value, &view.rank)?;
                bound_sites.push(Site::at(source, e));
            }
        }
        self.output.ranking_witnesses.push(crate::RankingWitness {
            site: Site::at(source, node_index),
            machine: machine.clone(),
            subject: Site::at(source, subject),
            view: view.declaration.clone(),
            arguments: args.iter().map(|&a| Site::at(source, a)).collect(),
            bounds: bound_sites,
            inclusive_upper: inclusive,
        });
        Ok(())
    }
    pub(super) fn invocation(
        &mut self,
        node_index: usize,
        machine: &DeclarationId,
    ) -> Result<(), TypeError> {
        let source = &self.unit.file;
        let head = self.unit.single_child(node_index)?;
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(head) {
            let actual = self.locals.get(span).or_else(|| {
                self.unit
                    .parameters
                    .get(span)
                    .and_then(|p| self.catalog.types.parameter_carrier(p))
            });
            if !matches!(actual, Some(ValueType::Callable(_))) {
                return Err(TypeError::at(source, head, "invokes parameter must be callable"));
            }
            let entry = self.invocation_target(head);
            self.output
                .invocation_ceilings
                .push((machine.clone(), Site::at(source, node_index), entry));
            return Ok(());
        }
        let Some(Symbol::Declaration(id)) = self.unit.symbol(head) else {
            return Err(TypeError::at(source, head, "invokes needs an exact entry or callback interface"));
        };
        if !matches!(id.kind, NodeKind::Trait | NodeKind::Machine | NodeKind::AsmMachine) {
            return Err(TypeError::at(source, head, "invokes needs an entry or callback interface"));
        }
        let args = source
            .child(head, NodeKind::TypeArgs)
            .into_iter()
            .flat_map(|section| source.nodes[section].children.iter().copied())
            .enumerate()
            .map(|(i, arg)| self.unit.argument(arg, &self.catalog.types, id, i))
            .collect::<Result<Vec<_>, _>>()?;
        if args.len() != self.catalog.types.arities[id] {
            return Err(TypeError::at(source, head, "incomplete invokes application"));
        }
        self.output.invocation_ceilings.push((
            machine.clone(),
            Site::at(source, node_index),
            crate::InvocationTarget::Declaration(crate::RequirementSlot {
                declaration: id.clone(),
                trait_arguments: args,
            }),
        ));
        Ok(())
    }
}
