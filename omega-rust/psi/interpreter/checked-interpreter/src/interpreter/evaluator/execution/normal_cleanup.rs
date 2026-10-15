//! Execute checked normal-edge actions before the current frame is retired.
//! Ordinary completion and attached Unit cleanup plans select nominal actions. A
//! selected dependency without an executable plan declines; typed-only evaluation has
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
            symbols::SymbolHandle,
        )>,
    > {
        let Some(checked) = self.checked else {
            return Ok(Vec::new());
        };
        // Action sources use dense structural positions. Parameter rows retain
        // authored positions for looking up the actual runtime binding.
        let flow = &checked.facts.flow;
        let mut actions = Vec::new();
        if let Some(plan) = flow.terminal_unit_effects.for_machine(machine.symbol) {
            if plan.state != state.symbol {
                return unsupported("normal cleanup plan names another return state");
            }
            let control = &flow.terminal_unit_effects.cleanup_control;
            if control
                .normal_exits(plan.normal_exit_cleanups)
                .iter()
                .any(|row| !row.selected_regions.is_empty())
            {
                return unsupported(
                    "unimplemented: checked-tree interpreter selected owner cleanup",
                );
            }
            for action in control.normal_actions(plan.normal_exit_cleanups) {
                let CheckedNormalCleanupAction::InvokeNominal(action) = action else {
                    continue;
                };
                let owner = self.normal_cleanup_owner(
                    state,
                    action,
                    &plan.structural_parameters,
                    &plan.operations,
                )?;
                actions.push((action, owner));
            }
        }
        Ok(actions)
    }

    /// Rejoin the same checked current-storage source for every body kind.
    fn normal_cleanup_owner(
        &self,
        state: &State,
        action: &CheckedUnitNominalAffineCleanupPlan,
        parameters: &[CheckedUnitStructuralParameterPlan],
        operations: &[checked_trees::CheckedUnitEffectOperationPlan],
    ) -> EvalResult<symbols::SymbolHandle> {
        Ok(match &action.source {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index,
            } => {
                let parameter = parameters
                    .get(*parameter_index as usize)
                    .filter(|parameter| {
                        !action.path.is_empty() || parameter.type_identity == action.type_identity
                    })
                    .ok_or_else(|| {
                        Halt::Unsupported("normal cleanup parameter custody drifted".into())
                    })?;
                self.program
                    .state_parameters(state)
                    .get(parameter.position as usize)
                    .ok_or_else(|| Halt::Unsupported("normal cleanup parameter is absent".into()))?
                    .symbol
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                *symbol
            }
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal,
            } => {
                let result = operations
                    .iter()
                    .filter_map(|operation| operation.structural_result_binding())
                    .find(|result| {
                        // A projected action names the child type;
                        // the result binding names its whole owner.
                        result.binding_ordinal == *binding_ordinal
                            && (!action.path.is_empty()
                                || result.type_identity == action.type_identity)
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
                    return unsupported("normal cleanup result has no retained local binding");
                };
                local.symbol
            }
            _ => return unsupported("normal cleanup source has no retained runtime owner"),
        })
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
            .and_then(|plan| plan.state(state.symbol));
        // This retiring comparison interpreter has no occurrence custody
        // ledger. Refuse candidate deaths before execution rather than run an
        // absent owner's hook; native and Terminal use the selected CFG.
        if composed.is_some_and(|state| checked.facts.flow.terminal_unit_effects.cleanup_control.normal_exits(state.normal_exit_cleanups).iter().any(|row| !row.selected_regions.is_empty()))
            || self.cleanup_operations(machine.symbol, state.symbol).iter().any(|operation| {
                matches!(operation, checked_trees::CheckedUnitEffectOperationPlan::ContinuationCleanup { selected_regions, .. } if !selected_regions.is_empty())
            })
        {
            return unsupported("unimplemented: checked-tree interpreter selected owner cleanup");
        }
        for (_, dependency) in checked.facts.operational.cleanup_dependencies.iter() {
            if dependency.caller_machine == machine.symbol
                && dependency.caller_state == state.symbol
                && !actions.iter().any(|(action, _)| {
                    action.cleanup_machine == dependency.target_machine
                        && action.cleanup_state == dependency.target_state
                })
                && !composed.is_some_and(|state| {
                    checked
                        .facts
                        .flow
                        .terminal_unit_effects
                        .cleanup_control
                        .normal_actions(state.normal_exit_cleanups)
                        .any(|action| {
                            matches!(action,
                        CheckedNormalCleanupAction::InvokeNominal(action)
                            if action.cleanup_machine == dependency.target_machine
                                && action.cleanup_state == dependency.target_state)
                        })
                })
                && !self
                    .cleanup_operations(machine.symbol, state.symbol)
                    .iter()
                    .any(|operation| {
                        matches!(operation,
                        checked_trees::CheckedUnitEffectOperationPlan::ContinuationCleanup {
                            boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement { .. },
                            cleanup_actions,
                            ..
                        } if cleanup_actions.iter().any(|action| matches!(action,
                            CheckedNormalCleanupAction::InvokeNominal(action)
                                if action.cleanup_machine == dependency.target_machine
                                    && action.cleanup_state == dependency.target_state)))
                    })
            {
                return unsupported(
                    "selected normal cleanup dependency has no executable checked boundary plan",
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
        if let Some((control, rows, parameters, operations)) = self.checked.and_then(|checked| {
            let plans = &checked.facts.flow.terminal_unit_effects;
            let body = plans
                .for_machine(machine.symbol)
                .filter(|plan| plan.state == state.symbol)
                .map(|plan| {
                    (
                        plan.normal_exit_cleanups,
                        plan.structural_parameters.as_slice(),
                        plan.operations.as_slice(),
                    )
                })
                .or_else(|| {
                    plans
                        .composed_for_machine(machine.symbol)
                        .and_then(|machine| machine.state(state.symbol))
                        .map(|state| {
                            (
                                state.normal_exit_cleanups,
                                state.structural_parameters.as_slice(),
                                state.operations.as_slice(),
                            )
                        })
                });
            body.map(|(rows, parameters, operations)| {
                (&plans.cleanup_control, rows, parameters, operations)
            })
        }) {
            let row = control
                .normal_exits(rows)
                .iter()
                .find(|row| row.exit == exit)
                .ok_or_else(|| {
                    Halt::Unsupported("normal cleanup has no exact outgoing edge plan".into())
                })?;
            if !row.selected_regions.is_empty() {
                return unsupported(
                    "unimplemented: checked-tree interpreter selected owner cleanup",
                );
            }
            for action in control.normal_exit_actions.span_or_empty(row.actions) {
                let CheckedNormalCleanupAction::InvokeNominal(action) = action else {
                    continue;
                };
                let symbol = self.normal_cleanup_owner(state, action, parameters, operations)?;
                let source = frame.local_cell(symbol).ok_or_else(|| {
                    Halt::Unsupported("normal cleanup source binding is absent".into())
                })?;
                self.execute_nominal_cleanup(action, source)?;
            }
            return Ok(());
        }
        for (action, owner) in self.normal_cleanup_actions(machine, state)? {
            let source = frame.local_cell(owner).ok_or_else(|| {
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
        let source = self.normal_cleanup_cell(source, &action.path)?;
        self.execute_nominal_cleanup_receiver(action, source)
    }

    pub(super) fn execute_nominal_cleanup_receiver(
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
                    self.allocate_cell(Value::reference(source.clone()))?,
                )]
            }
            _ => return unsupported("nominal cleanup target argument custody drifted"),
        };
        let _returned = self.run_state_collect(target, target_state, arguments)?;
        Ok(())
    }

    /// Resolve the selected structural path to the existing runtime cell.
    /// Schema ordinals select declaration identity, not a runtime field spelling;
    /// the declaration then supplies the interpreter's field-name payload.
    pub(super) fn normal_cleanup_cell(
        &self,
        mut source: Cell,
        path: &[checked_trees::CheckedUnitStructuralPathSegment],
    ) -> EvalResult<Cell> {
        use checked_trees::CheckedUnitStructuralPathSegment;
        use typed_trees::data::DataMember;
        for segment in path {
            source = self.deref_cell(source);
            match segment {
                CheckedUnitStructuralPathSegment::AllElements => {
                    return unsupported("universal qualification is not a cleanup address");
                }
                CheckedUnitStructuralPathSegment::RepresentationView { .. } => {
                    return unsupported(
                        "storage interpretation cannot provide an owned cleanup receiver",
                    );
                }
                CheckedUnitStructuralPathSegment::Referent => {}
                CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                    source = self.element_cell(
                        &source,
                        usize::try_from(*index).map_err(|_| {
                            Halt::Unsupported("cleanup index exceeds host storage".into())
                        })?,
                    )?;
                }
                CheckedUnitStructuralPathSegment::Field(identity)
                | CheckedUnitStructuralPathSegment::Case(identity) => {
                    let value = source.borrow();
                    let (type_symbol, case_name) = match &*value {
                        Value::Struct { type_symbol, .. } => (*type_symbol, None),
                        Value::Enum {
                            type_symbol,
                            variant_name,
                            ..
                        } => (*type_symbol, Some(variant_name.as_ref())),
                        _ => return trap("cleanup projection is not a record or case"),
                    };
                    let data = self
                        .program
                        .data_definition_by_symbol(type_symbol)
                        .ok_or_else(|| {
                            Halt::Unsupported("cleanup projection declaration is absent".into())
                        })?;
                    let variant = case_name.and_then(|name| {
                        self.program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Variant(variant) if variant.name.as_str() == name => {
                                    Some(variant)
                                }
                                _ => None,
                            })
                    });
                    if matches!(segment, CheckedUnitStructuralPathSegment::Case(_)) {
                        if !variant
                            .is_some_and(|variant| variant.path_identity() == identity.spelling())
                        {
                            return trap(
                                "cleanup case membership does not match its selected path",
                            );
                        }
                        continue;
                    }
                    let field = if let Some(variant) = variant {
                        self.program
                            .data_payload_fields(variant)
                            .iter()
                            .find(|field| field.path_identity() == identity.spelling())
                    } else {
                        self.program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                DataMember::Field(field)
                                    if field.path_identity() == identity.spelling() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })
                    }
                    .ok_or_else(|| Halt::Unsupported("cleanup field identity is absent".into()))?;
                    let selected = self.field_cell(&source, field.name.as_str())?;
                    drop(value);
                    source = selected;
                }
                CheckedUnitStructuralPathSegment::FixedByteRange { .. }
                | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                | CheckedUnitStructuralPathSegment::ComputedIndex(_) => {
                    return unsupported("normal cleanup requires a retained static receiver path");
                }
            }
        }
        Ok(source)
    }
}
