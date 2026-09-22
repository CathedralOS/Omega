use super::{
    EvalResult, EvaluatedArgument, Evaluator, ExpressionHandle, Frame, Halt, SymbolHandle,
    TableCall, Value, trap,
};
impl<'program> Evaluator<'program> {
    pub(super) fn selected_boundary_adapter(
        &self,
        receiver: SymbolHandle,
        requirement: SymbolHandle,
        machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    ) -> Option<checked_trees::CheckedBoundaryAdapterDispatch> {
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

    /// Receiver forwarding is an argument operation, not a synthetic source edit.
    pub(super) fn eval_boundary_receiver_path(
        &mut self,
        call: &TableCall,
        frame: &Frame,
    ) -> EvalResult<EvaluatedArgument> {
        let members = self
            .program
            .statement_table
            .name_path_members(call.receiver);
        let Some((head, rest)) = members.split_first() else {
            return trap("selected boundary adapter lost its receiver");
        };
        let mut cell = if head.is_self_receiver() {
            frame.self_cell.clone()
        } else if let Some(local) = frame.local_cell(call.receiver_root_symbol) {
            local
        } else {
            self.field_cell(&frame.self_cell, head.as_str())?
        };
        for member in rest {
            cell = self.field_cell(&self.deref_cell(cell), member.as_str())?;
        }
        let value = cell.borrow().clone();
        Ok(EvaluatedArgument::plain(self.allocate_cell(value)?))
    }

    pub(super) fn run_boundary_adapter(
        &mut self,
        dispatch: checked_trees::CheckedBoundaryAdapterDispatch,
        receiver: Option<EvaluatedArgument>,
        arguments: &[ExpressionHandle],
        frame: &mut Frame,
    ) -> EvalResult<Value> {
        let (machine, state, instance) = self
            .resolve_entry_state_symbol(dispatch.realization_state, frame)
            .ok_or_else(|| {
                Halt::Trap("selected boundary adapter lost its exact entry state".into())
            })?;
        let destinations = self
            .program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .map(|parameter| parameter.type_reference)
            .collect::<Vec<_>>();
        let offset = usize::from(dispatch.forward_receiver);
        if receiver.is_some() != dispatch.forward_receiver
            || destinations.len() != arguments.len() + offset
        {
            return trap(
                "selected boundary adapter argument count differs from its checked signature",
            );
        }
        let mut evaluated = Vec::with_capacity(destinations.len());
        evaluated.extend(receiver);
        for (argument, destination) in arguments.iter().zip(&destinations[offset..]) {
            evaluated.push(self.eval_state_argument(*argument, *destination, frame)?);
        }
        let guard_depth = self.guard_depth;
        self.guard_depth = 0;
        let result = self.run_state_collect(machine, state, instance, evaluated);
        self.guard_depth = guard_depth;
        result.map(|value| value.unwrap_or(Value::Unit))
    }
}
