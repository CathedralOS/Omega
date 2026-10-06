//! Execute checked normal-edge actions before the current frame is retired.
//! Unit, scalar and structural returns select the same nominal action executor. A selected
//! dependency without an executable plan declines; typed-only evaluation has
//! no authority to invent a cleanup call from a type or machine name.

use crate::interpreter::evaluator::{
    Cell, EvalResult, EvaluatedArgument, Evaluator, Frame, Halt, Machine, State, Value, trap,
    unsupported,
};
use checked_trees::{
    CheckedNormalCleanupAction, CheckedUnitNominalAffineCleanupPlan,
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
        // Action sources use dense structural positions. Parameter rows retain
        // authored positions for looking up the actual runtime binding.
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
                    .get(cleanup.source_parameter_index().ok_or_else(|| {
                        Halt::Unsupported("cleanup requires a parameter source".into())
                    })? as usize)
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
                if let CheckedNormalCleanupAction::InvokeNominal(cleanup) = action {
                    let parameter = plan
                        .structural_parameters
                        .get(cleanup.source_parameter_index().ok_or_else(|| {
                            Halt::Unsupported("cleanup requires a parameter source".into())
                        })? as usize)
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
        let composed = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine.symbol)
            .and_then(|plan| plan.states.iter().find(|row| row.state == state.symbol));
        for (_, dependency) in checked.facts.operational.cleanup_dependencies.iter() {
            if dependency.caller_machine == machine.symbol
                && dependency.caller_state == state.symbol
                && !actions.iter().any(|(action, _)| {
                    action.cleanup_machine == dependency.target_machine
                        && action.cleanup_state == dependency.target_state
                })
                && !composed.is_some_and(|state| {
                    state
                        .normal_exit_cleanups
                        .iter()
                        .flat_map(|row| &row.actions)
                        .any(|action| {
                            matches!(action,
                        CheckedNormalCleanupAction::InvokeNominal(action)
                            if action.cleanup_machine == dependency.target_machine
                                && action.cleanup_state == dependency.target_state)
                        })
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
        exit: checked_trees::FlowClaimJoinExit,
    ) -> EvalResult<()> {
        if let Some(plan) = self
            .checked
            .and_then(|checked| {
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .composed_for_machine(machine.symbol)
            })
            .and_then(|plan| plan.states.iter().find(|row| row.state == state.symbol))
        {
            let row = plan
                .normal_exit_cleanups
                .iter()
                .find(|row| row.exit == exit)
                .ok_or_else(|| {
                    Halt::Unsupported("normal cleanup has no exact outgoing edge plan".into())
                })?;
            for action in &row.actions {
                let CheckedNormalCleanupAction::InvokeNominal(action) = action else {
                    continue;
                };
                let symbol = match &action.source {
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    } => {
                        let parameter = plan
                            .structural_parameters
                            .get(*parameter_index as usize)
                            .filter(|parameter| parameter.type_identity == action.type_identity)
                            .ok_or_else(|| {
                                Halt::Unsupported("normal cleanup parameter custody drifted".into())
                            })?;
                        self.program
                            .state_parameters(state)
                            .get(parameter.position as usize)
                            .ok_or_else(|| {
                                Halt::Unsupported("normal cleanup parameter is absent".into())
                            })?
                            .symbol
                    }
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol,
                    } => *symbol,
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal,
                    } => {
                        let result = plan
                            .operations
                            .iter()
                            .filter_map(|operation| operation.structural_result_binding())
                            .find(|result| {
                                result.binding_ordinal == *binding_ordinal
                                    && result.type_identity == action.type_identity
                            })
                            .ok_or_else(|| {
                                Halt::Unsupported("normal cleanup result binding is absent".into())
                            })?;
                        let Some(typed_trees::statement::StatementNode::LocalData(local)) = self
                            .program
                            .statement_table
                            .statements(state.statement_nodes)
                            .get(result.statement_index as usize)
                        else {
                            return unsupported(
                                "normal cleanup result has no retained local binding",
                            );
                        };
                        local.symbol
                    }
                    _ => return unsupported("normal cleanup source has no retained runtime owner"),
                };
                let source = frame.local_cell(symbol).ok_or_else(|| {
                    Halt::Unsupported("normal cleanup source binding is absent".into())
                })?;
                self.execute_nominal_cleanup(action, source)?;
            }
            return Ok(());
        }
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
            .callable_entry_for_machine(action.cleanup_machine)
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
        // Source checking discharged this exact edge's callable prerequisites,
        // as for an authored call. Execution keeps the selected storage/body
        // join; it must not run a second, Boolean-only contract interpreter.
        if !matches!(*source.borrow(), Value::Struct { .. }) {
            return trap("nominal cleanup source is not its record value");
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
        let _returned = self.run_state_collect(target, target_state, arguments)?;
        Ok(())
    }
}
