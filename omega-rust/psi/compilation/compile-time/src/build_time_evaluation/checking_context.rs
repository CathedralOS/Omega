//! Private ordinary checking closes template descriptions independently of
//! execution permission. Specializations and conformance captures retain exact
//! source edges; their full contracts and transitive effects are checked in
//! this forest. The originating domain remains the immutable execution ceiling.

use super::EvaluationDomain;

impl EvaluationDomain {
    /// Ordinary checking needs the authored templates whose universal contracts
    /// justify retained instances. Close those exact declarations and their
    /// dependencies in the original forest; this does not change execution.
    pub(crate) fn checking_program(
        &self,
        original: &typed_trees::TypedTrees,
    ) -> Result<typed_trees::TypedTrees, Vec<diagnostics::Diagnostic>> {
        let declarations = typed_trees::references::collect_program_declarations(original);
        let mut roots = declarations
            .into_iter()
            .filter(|symbol| self.execution.contains(*symbol))
            .collect::<Vec<_>>();
        loop {
            let context = crate::syntax_probes::PreparedEvaluationInputs::domain_for_program(
                original,
                &roots,
                &[],
            );
            let mut added = false;
            for receipt in &original.machine_specializations {
                if !context.execution.contains(receipt.instance) {
                    continue;
                }
                let mut instances = original
                    .machine_specializations
                    .iter()
                    .filter(|candidate| candidate.instance == receipt.instance);
                if instances.next().is_none()
                    || instances.next().is_some()
                    || original
                        .machines()
                        .iter()
                        .filter(|machine| machine.symbol == receipt.template)
                        .count()
                        != 1
                    || !original
                        .expression_table
                        .static_machine_application_is_valid(receipt.binding_application)
                {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "build-time checking context lost its unique authored specialization template",
                    )]);
                }
                if !roots.contains(&receipt.template) {
                    roots.push(receipt.template);
                    added = true;
                }
                let provider = original.machine(receipt.instance).ok_or_else(|| {
                    vec![diagnostics::Diagnostic::error(
                        "build-time checking context lost its exact specialized instance",
                    )]
                })?;
                if provider.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                    || !original.machine_type_parameters(provider).is_empty()
                {
                    continue;
                }
                for realization in &receipt.operator_realizations {
                    let Some(requirement) = original.machine(realization.requirement_symbol) else {
                        if original
                            .operator_definition_by_symbol(realization.requirement_symbol)
                            .is_some()
                        {
                            continue;
                        }
                        return Err(vec![diagnostics::Diagnostic::error(
                            "build-time checking context lost its exact realized requirement",
                        )]);
                    };
                    if requirement.supply_mode
                        != language_semantics::MachineSupplyMode::TopLevelRequirement
                    {
                        continue;
                    }
                    let closed = validation::resolve_closed_requirement_application(
                        original,
                        provider,
                        requirement,
                    )
                    .map_err(|diagnostic| vec![diagnostic])?;
                    if closed.supply_mode
                        != language_semantics::MachineSupplyMode::TopLevelRequirement
                        || !original.machine_type_parameters(closed).is_empty()
                        || original.machine_states(closed).len() != 1
                    {
                        return Err(vec![diagnostics::Diagnostic::error(
                            "build-time checking context requirement did not retain a closed signature",
                        )]);
                    }
                    if !roots.contains(&closed.symbol) {
                        roots.push(closed.symbol);
                        added = true;
                    }
                }
            }
            for (_, capture) in original.captured_machine_applications.iter() {
                if !context.execution.contains(capture.instance) {
                    continue;
                }
                if original
                    .captured_machine_applications
                    .iter()
                    .filter(|(_, candidate)| candidate.instance == capture.instance)
                    .count()
                    != 1
                    || original
                        .machines()
                        .iter()
                        .filter(|machine| machine.symbol == capture.template)
                        .count()
                        != 1
                    || !original
                        .expression_table
                        .static_machine_application_is_valid(capture.binding_application)
                    || !original
                        .expression_table
                        .static_machine_application_is_valid(capture.supplying_application)
                {
                    return Err(vec![diagnostics::Diagnostic::error(
                        "build-time checking context lost its unique captured template application",
                    )]);
                }
                if !roots.contains(&capture.template) {
                    roots.push(capture.template);
                    added = true;
                }
            }
            if !added {
                let mut program = context.execution_program(original);
                if let Some(frontier) = &self.product_frontier {
                    program.product_declaration_frontier = Some(frontier.clone());
                }
                return Ok(program);
            }
        }
    }
}
