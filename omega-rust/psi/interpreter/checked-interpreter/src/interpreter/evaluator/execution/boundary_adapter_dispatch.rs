use crate::interpreter::evaluator::{
    EvalResult, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle, Value, trap,
};
impl<'program> Evaluator<'program> {
    /// Named invocations share provider selection regardless of whether their
    /// result is discarded by a statement or completes a selected transition arm.
    /// The binding a call names selects the provider row; it is not an argument.
    pub(in crate::interpreter::evaluator) fn try_selected_named_call(
        &mut self,
        receiver_symbol: SymbolHandle,
        target_symbol: SymbolHandle,
        machine_arguments: &[typed_trees::expression::StaticMachineArgument],
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Option<Value>> {
        if receiver_symbol.is_valid()
            && let Some(dispatch) =
                self.selected_boundary_adapter(receiver_symbol, target_symbol, machine_arguments)
        {
            return self
                .run_boundary_adapter(dispatch, arguments, frame)
                .map(Some);
        }
        self.try_operator_application(target_symbol, ExpressionHandle::invalid(), arguments, frame)
    }

    pub(in crate::interpreter::evaluator) fn selected_boundary_adapter(
        &self,
        receiver: SymbolHandle,
        requirement: SymbolHandle,
        machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    ) -> Option<checked_trees::CheckedBoundaryAdapterDispatch> {
        let requirement = self
            .program
            .closed_requirement_application(requirement)
            .map_or(requirement, |application| application.template);
        self.boundary_adapter_dispatch
            .iter()
            .filter(|dispatch| dispatch.receiver == receiver && dispatch.requirement == requirement)
            .find(|dispatch| {
                // An exact nongeneric row matches every call to its
                // requirement. A family row settles only for the call whose
                // static arguments canonically equal its declared tuple.
                dispatch.family_tuple.is_empty()
                    || machine_arguments
                        .iter()
                        .map(|argument| self.program.static_const_argument_identity(argument))
                        .collect::<Option<Vec<_>>>()
                        .is_some_and(|tuple| tuple.as_slice() == dispatch.family_tuple.as_ref())
            })
            .cloned()
    }

    pub(in crate::interpreter::evaluator) fn run_boundary_adapter(
        &mut self,
        dispatch: checked_trees::CheckedBoundaryAdapterDispatch,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        self.run_selected_realization(dispatch.realization_state, arguments, frame)
    }

    /// Run one selected realization entry state. Static conformance dispatch
    /// and receiver-carried adapter rows both settle to this same execution:
    /// the exact provider body the checker recorded on the call.
    pub(in crate::interpreter::evaluator) fn run_selected_realization(
        &mut self,
        realization_state: SymbolHandle,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let (machine, state) = self
            .resolve_entry_state_symbol(realization_state)
            .ok_or_else(|| {
                Halt::Trap("selected realization lost its exact entry state".into())
            })?;
        // Providers receive exactly the requirement's authored arguments.
        // Checking rejects any satisfier parameter its requirement lacks, so
        // no provider instance or receiver is allocated or inserted here.
        let destinations = self
            .program
            .state_parameters(state)
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect::<Vec<_>>();
        if destinations.len() != arguments.len() {
            return trap(
                "selected boundary adapter argument count differs from its checked signature",
            );
        }
        let mut evaluated = Vec::with_capacity(destinations.len());
        for (argument, destination) in arguments.iter().zip(&destinations) {
            evaluated.push(self.eval_state_argument(*argument, *destination, frame)?);
        }
        let guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let result = self.run_state_collect(machine, state, evaluated);
        self.guard_depth = guard_depth;
        result.map(|value| value.unwrap_or(Value::Unit))
    }

    /// Run one `dyn Trait` selector call. The checker recorded the slot on
    /// the call target; operand 0 is the selector value whose runtime type
    /// identity names the realizing machine. That machine's `satisfies`
    /// conformance names the exact impl state, which receives only the
    /// requirement's authored operands — the selector itself is dispatch
    /// evidence, never an argument.
    pub(in crate::interpreter::evaluator) fn run_dynamic_slot(
        &mut self,
        slot: &typed_trees::dynamic_traits::DynamicSlotSignature,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let Some((selector, arguments)) = arguments.split_first() else {
            return trap("dynamic slot call carries no selector operand");
        };
        let selector = self.eval_expression(*selector, frame)?;
        let Value::Struct { type_symbol, .. } = selector else {
            return trap("dynamic slot selector carries no implementation identity");
        };
        // A named `satisfies` conformance carries the exact checked
        // requirement→realization rows; its row's `realization_state` is the
        // same authority `static_requirement_dispatch` supplies.
        let selected_state = self
            .program
            .conformance_by_symbol(type_symbol)
            .and_then(|conformance| {
                conformance
                    .rows
                    .iter()
                    .find(|row| {
                        row.requirement == slot.requirement
                            || row.requirement == slot.call_requirement
                    })
                    .map(|row| row.realization_state)
            });
        if let Some(realization_state) = selected_state {
            return self.run_selected_realization(realization_state, arguments, frame);
        }
        let machine = self
            .find_machine_by_symbol(type_symbol)
            .ok_or_else(|| Halt::Trap("dynamic selector names no realized machine".into()))?;
        // `requirement_symbol` retains the declaring requirement the impl
        // entry satisfies; the impl state is the machine's state authored
        // under that binding's `as` alias or requirement name. A
        // trait-level `satisfies` with neither token still resolves by the
        // requirement's own declared name.
        let conformance = self
            .program
            .machine_trait_conformances(machine)
            .iter()
            .find(|conformance| {
                conformance.requirement_symbol == slot.requirement
                    || conformance.requirement_symbol == slot.call_requirement
            });
        let state_name = conformance
            .and_then(|conformance| {
                conformance
                    .alias
                    .as_ref()
                    .or(conformance.requirement.as_ref())
            })
            .or_else(|| {
                self.program
                    .dynamic_slot_requirement(slot)
                    .map(|signature| &signature.name)
            })
            .map(|name| name.as_str().to_owned())
            .ok_or_else(|| {
                Halt::Trap(
                    "dynamic selector's implementation names no realizing state".into(),
                )
            })?;
        let state = self.find_state(machine, &state_name).ok_or_else(|| {
            Halt::Trap(format!(
                "dynamic realization state `{state_name}` is absent from `{}`",
                machine.name.as_str()
            ))
        })?;
        let destinations = self
            .program
            .state_parameters(state)
            .iter()
            .map(|parameter| parameter.type_reference)
            .collect::<Vec<_>>();
        if destinations.len() != arguments.len() {
            return trap("dynamic slot argument count differs from its realized signature");
        }
        let mut evaluated = Vec::with_capacity(destinations.len());
        for (argument, destination) in arguments.iter().zip(&destinations) {
            evaluated.push(self.eval_state_argument(*argument, *destination, frame)?);
        }
        let guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let result = self.run_state_collect(machine, state, evaluated);
        self.guard_depth = guard_depth;
        result.map(|value| value.unwrap_or(Value::Unit))
    }
}
