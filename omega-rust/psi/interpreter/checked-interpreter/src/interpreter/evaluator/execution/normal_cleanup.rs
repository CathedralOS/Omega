//! Execute checked normal-edge actions before the current frame is retired.
//! Unit and scalar returns select the same nominal action executor. A selected
//! dependency without an executable plan declines; typed-only evaluation has
//! no authority to invent a cleanup call from a type or machine name.

use crate::interpreter::evaluator::{
    Cell, DataMember, EvalResult, EvaluatedArgument, Evaluator, Frame, Halt, Machine, State, Value,
    trap, unsupported,
};
use checked_trees::{
    CheckedStructuralScalarReturnCleanupAction, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitStructuralParameterPlan,
};

impl<'program> Evaluator<'program> {
    fn normal_cleanup_actions(
        &self,
        machine: &Machine,
        state: &State,
    ) -> EvalResult<
        Vec<(
            &'program CheckedUnitNominalAffineCleanupPlan,
            &'program CheckedUnitStructuralParameterPlan,
        )>,
    > {
        let Some(checked) = self.checked else {
            return Ok(Vec::new());
        };
        // Checked cleanup actions retain authored parameter positions, including
        // gaps occupied by scalar or erased inputs; structural rows are compact.
        let flow = &checked.facts.flow;
        let mut actions = Vec::new();
        if let Some(plan) = flow
            .terminal_nominal_affine_unit_cleanups
            .for_machine(machine.symbol)
        {
            if plan.machine.state != state.symbol {
                return unsupported("nominal cleanup plan names another return state");
            }
            for cleanup in &plan.cleanups {
                let parameter = plan
                    .machine
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.position == cleanup.source_parameter_index)
                    .ok_or_else(|| {
                        Halt::Unsupported("cleanup source parameter is absent".into())
                    })?;
                actions.push((cleanup, parameter));
            }
        } else if let Some(plan) = flow
            .terminal_structural_scalar_returns
            .for_machine(machine.symbol)
        {
            if plan.state != state.symbol {
                return unsupported("scalar cleanup plan names another return state");
            }
            for action in &plan.cleanup_actions {
                if let CheckedStructuralScalarReturnCleanupAction::InvokeNominal(cleanup) = action {
                    let parameter = plan
                        .structural_parameters
                        .iter()
                        .find(|parameter| parameter.position == cleanup.source_parameter_index)
                        .ok_or_else(|| {
                            Halt::Unsupported("cleanup source parameter is absent".into())
                        })?;
                    actions.push((cleanup, parameter));
                }
            }
        }
        Ok(actions)
    }

    pub(super) fn validate_normal_cleanup_selection(
        &self,
        machine: &Machine,
        state: &State,
    ) -> EvalResult<()> {
        let Some(checked) = self.checked else {
            return Ok(());
        };
        let actions = self.normal_cleanup_actions(machine, state)?;
        for (_, dependency) in checked.facts.operational.cleanup_dependencies.iter() {
            if dependency.caller_machine == machine.symbol
                && dependency.caller_state == state.symbol
                && !actions.iter().any(|(action, _)| {
                    action.cleanup_machine == dependency.target_machine
                        && action.cleanup_state == dependency.target_state
                })
            {
                return unsupported(
                    "selected normal cleanup dependency has no executable checked return plan",
                );
            }
        }
        Ok(())
    }

    pub(super) fn execute_normal_cleanup(
        &mut self,
        machine: &Machine,
        state: &State,
        frame: &Frame,
    ) -> EvalResult<()> {
        for (action, parameter) in self.normal_cleanup_actions(machine, state)? {
            if parameter.type_identity != action.type_identity {
                return unsupported("normal cleanup source type custody drifted");
            }
            let source = self
                .program
                .state_parameters(state)
                .get(parameter.position as usize)
                .and_then(|parameter| frame.local_cell(parameter.symbol))
                .ok_or_else(|| {
                    Halt::Unsupported("normal cleanup source binding is absent".into())
                })?;
            self.execute_nominal_cleanup(action, source)?;
        }
        Ok(())
    }

    fn execute_nominal_cleanup(
        &mut self,
        action: &CheckedUnitNominalAffineCleanupPlan,
        source: Cell,
    ) -> EvalResult<()> {
        let checked = self
            .checked
            .expect("cleanup actions require checked custody");
        let target_plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .for_machine(action.cleanup_machine)
            .ok_or_else(|| {
                Halt::Unsupported("nominal cleanup target has no checked body".into())
            })?;
        if target_plan.state != action.cleanup_state
            || target_plan.contract_report_fingerprint != action.cleanup_contract_report_fingerprint
        {
            return unsupported("nominal cleanup target selection drifted");
        }
        let target = self
            .find_machine_by_symbol(action.cleanup_machine)
            .ok_or_else(|| Halt::Unsupported("nominal cleanup target is absent".into()))?;
        let target_state = self
            .find_state_by_symbol(target, action.cleanup_state)
            .ok_or_else(|| Halt::Unsupported("nominal cleanup target state is absent".into()))?;
        // Requirements name the exact source root and normalized declaration
        // fields. Observe each immediately before its own action, not once for
        // the entire cleanup sequence.
        {
            let value = source.borrow();
            let Value::Struct {
                type_symbol,
                fields,
                ..
            } = &*value
            else {
                return trap("nominal cleanup source is not its record value");
            };
            let definition = self
                .program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *type_symbol)
                .ok_or_else(|| Halt::Trap("nominal cleanup record declaration is absent".into()))?;
            for requirement in &action.requirements {
                let field = self
                    .program
                    .data_members(definition)
                    .iter()
                    .find_map(|member| match member {
                        DataMember::Field(field)
                            if field.path_identity() == requirement.field_identity =>
                        {
                            Some(field)
                        }
                        _ => None,
                    })
                    .ok_or_else(|| {
                        Halt::Trap("nominal cleanup prerequisite field is absent".into())
                    })?;
                if !fields.get(field.name.as_str()).is_some_and(|cell| {
                    matches!(*cell.borrow(), Value::Bool(value) if value == requirement.expected)
                }) {
                    return trap("nominal cleanup prerequisite is false at its owned place");
                }
            }
        }
        let parameters = self.program.state_parameters(target_state);
        let arguments = match parameters {
            [] => Vec::new(),
            [parameter]
                if validation::reference_shell(self.program, parameter.type_reference)
                    .is_some_and(|shell| shell.access.is_exclusive()) =>
            {
                vec![EvaluatedArgument::plain(
                    self.allocate_cell(Value::Ref(source.clone()))?,
                )]
            }
            _ => return unsupported("nominal cleanup target argument custody drifted"),
        };
        let _returned = self.run_state_collect(target, target_state, source, arguments)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
