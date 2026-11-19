use crate::expressions::body_typer::BodyTyper;
use crate::{ObligationKind, Site, TypeError, ValueType};
use omega_parser::{NodeKind, TerminationClause};
use omega_resolver::DeclarationId;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    pub(super) fn crash_route(
        &mut self,
        node_index: usize,
        contract: usize,
    ) -> Result<(), TypeError> {
        let source = self.unit.file;
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
        let source = self.unit.file;
        let clause = source.nodes[node_index]
            .termination_clause()
            .ok_or_else(|| TypeError::unsupported(source, node_index))?;
        self.record_obligation(node_index, ObligationKind::Termination(machine.clone()));
        let TerminationClause::By {
            measure: subject,
            order,
            bound,
        } = clause
        else {
            return Ok(());
        };
        let actual = self.type_expression(subject)?;
        let (view, args) = if let Some(term) = order {
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
            (view, &[][..])
        };
        self.check_destination(subject, &actual, &view.subject)?;
        if view.parameters.len() != args.len() {
            return Err(TypeError::at(source, node_index, "incorrect ranking view arity"));
        }
        for (&arg, param) in args.iter().zip(&view.parameters) {
            let value = self.type_expression_with_expected_type(arg, Some(param))?;
            self.check_destination(arg, &value, param)?;
        }
        if let Some(bound) = bound {
            let endpoints = if let Some(range) = source.range_bounds(bound) {
                [range.start, range.end]
            } else {
                [Some(bound), None]
            };
            for e in endpoints.into_iter().flatten() {
                let value = self.type_expression_with_expected_type(e, Some(&view.rank))?;
                self.check_destination(e, &value, &view.rank)?;
            }
        }
        self.output.ranking_witnesses.push(crate::RankingWitness {
            node: source.node_id(node_index),
            machine: machine.clone(),
            view: view.declaration.clone(),
            rank: view.rank.clone(),
        });
        Ok(())
    }
    pub(super) fn invocation(
        &mut self,
        node_index: usize,
        machine: &DeclarationId,
    ) -> Result<(), TypeError> {
        let source = self.unit.file;
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
        let selection = crate::RequirementSlot {
            declaration: id.clone(),
            trait_arguments: self.unit.application_arguments(head, id, &self.catalog.types)?,
        };
        self.output
            .invocation_ceilings
            .push((machine.clone(), Site::at(source, node_index), crate::InvocationTarget::Declaration(selection)));
        Ok(())
    }
}
