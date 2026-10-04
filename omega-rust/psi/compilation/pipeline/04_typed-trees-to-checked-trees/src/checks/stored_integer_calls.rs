//! Concrete encoded stores performed through ordinary invocations. Each leaf
//! write is checked at the actual storage and operand snapshots of this call;
//! a returned value or an aggregate mutation footprint cannot replace it.
use super::contracts::call_values;
use checked_trees::{
    CheckFacts, CheckedBooleanExpression as Boolean, CheckedStoredIntegerWrite, FlowCallFact,
    FlowStateFact, PerformedIntegerWrite, StoredIntegerWriteSite,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, machine::Machine, state::State, statement::StatementNode};
use validation::{CallFrameResolver, CallWriteOrigins, LocalWriteSource};

#[cfg(test)]
mod tests;

struct Invocation<'a> {
    state: &'a FlowStateFact,
    call: &'a FlowCallFact,
    target: &'a Machine,
    origins: CallWriteOrigins,
}

struct Admission<'a> {
    program: &'a TypedTrees,
    facts: &'a CheckFacts,
    frames: &'a CallFrameResolver<'a>,
    entry_origins: &'a [(SymbolHandle, crate::flow::MachineParameterOrigins)],
    stack: Vec<Invocation<'a>>,
    decisions: Vec<CheckedStoredIntegerWrite>,
    /// Cyclic invocation cannot inherit a proof about only its first arrival.
    /// All reached stores must then have occurrence-wide fitting bounds.
    recursive: bool,
    all_ranges_fit: bool,
}

pub(super) fn check(
    program: &TypedTrees,
    facts: &mut CheckFacts,
    frames: Option<&CallFrameResolver<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    if !program.plan_laid_layouts.iter().any(|layout| {
        layout
            .integer_fields
            .iter()
            .any(|field| !field.write_is_total)
    }) {
        return Ok(());
    }
    let Some(frames) = frames else {
        return Err(vec![Diagnostic::error(
            "IntegerAt call stores have no exact mutation inventory",
        )]);
    };
    let entry_origins = program
        .machines()
        .iter()
        .map(|machine| {
            (
                machine.symbol,
                crate::flow::machine_storage_origins(program, frames, machine),
            )
        })
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut decisions = Vec::new();
    for (_, state) in facts.flow.control.states.iter() {
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            // Named states are covered by their owning machine's complete
            // state inventory and shared entry-storage fixed point.
            if internal_state_call(program, state, call) {
                continue;
            }
            let mut admission = Admission {
                program,
                facts,
                frames,
                entry_origins: &entry_origins,
                stack: Vec::new(),
                decisions: Vec::new(),
                recursive: false,
                all_ranges_fit: true,
            };
            let result = admission.visit(state, call).and_then(|()| {
                if admission.recursive && !admission.all_ranges_fit {
                    Err("recursive write values have no occurrence-wide fitting bounds")
                } else {
                    Ok(())
                }
            });
            match result {
                Ok(()) => decisions.extend(admission.decisions),
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "IntegerAt call in {} at statement {} requires every performed value to fit its physical storage: {reason}",
                    program.symbols.name(state.machine_symbol), call.statement_index,
                ))),
            }
        }
    }
    if diagnostics.is_empty() {
        facts.stored_integer_writes.insert_many(decisions);
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn internal_state_call(program: &TypedTrees, state: &FlowStateFact, call: &FlowCallFact) -> bool {
    call_values::target_machine(program, call).is_none()
        && crate::semantic::calls::find_state_with_machine(program, call.target_symbol)
            .is_some_and(|(machine, _)| machine.symbol == state.machine_symbol)
}

impl<'a> Admission<'a> {
    fn owner(&self, state: &FlowStateFact) -> Result<(&'a Machine, &'a State), &'static str> {
        crate::lookup::symbols::machine_state_by_symbol(
            self.program,
            state.machine_symbol,
            state.state_symbol,
        )
        .ok_or("call has no exact source owner")
    }

    fn call_origins(&self, state: &FlowStateFact, call: &FlowCallFact) -> Option<CallWriteOrigins> {
        let (machine, body) = self.owner(state).ok()?;
        match crate::semantic::calls::find_call_site(
            self.program,
            machine.symbol,
            body.symbol,
            call.statement_index,
            call.call_ordinal,
        )? {
            crate::semantic::calls::CallSite::Statement(call) => {
                self.frames.call_write_origins(machine, call)
            }
            crate::semantic::calls::CallSite::Expression { expression, .. } => self
                .frames
                .expression_call_write_origins(machine, expression),
            crate::semantic::calls::CallSite::TransitionNamed { .. } => {
                let target = crate::semantic::calls::transition_call_target(
                    self.program,
                    machine,
                    body,
                    call.statement_index,
                    call.call_ordinal,
                )?;
                self.frames.transition_write_origins(machine, target)
            }
        }
    }

    fn call_has_no_writes(&self, state: &FlowStateFact, call: &FlowCallFact) -> bool {
        let Ok((machine, body)) = self.owner(state) else {
            return false;
        };
        match crate::semantic::calls::find_call_site(
            self.program,
            machine.symbol,
            body.symbol,
            call.statement_index,
            call.call_ordinal,
        ) {
            Some(crate::semantic::calls::CallSite::Statement(call)) => self
                .frames
                .may_write_frame(machine, call)
                .complete_paths()
                .is_some_and(|paths| paths.is_empty()),
            Some(crate::semantic::calls::CallSite::Expression { expression, .. }) => self
                .frames
                .expression_write_frame(machine, expression)
                .complete_paths()
                .is_some_and(|paths| paths.is_empty()),
            _ => false,
        }
    }

    fn visit(
        &mut self,
        state: &'a FlowStateFact,
        call: &'a FlowCallFact,
    ) -> Result<(), &'static str> {
        if self.call_has_no_writes(state, call) {
            return Ok(());
        }
        // Exact checked access, not the mere presence of a layout in the
        // compilation, determines whether an opaque call can affect storage.
        let writable = self
            .facts
            .borrow
            .argument_accesses
            .span_or_empty(call.accesses)
            .iter()
            .any(|access| access.kind.is_exclusive());
        let origins = match self.call_origins(state, call) {
            Some(origins) => origins,
            None if !writable => return Ok(()),
            None => return Err("invocation storage origins are unknown"),
        };
        if !self.call_may_require_fit(state, call, &origins)? {
            return Ok(());
        }
        let Some(symbol) = call_values::target_machine(self.program, call) else {
            return self.check_unknown_call(state, call, &origins);
        };
        let target = self
            .program
            .machine_by_symbol(symbol)
            .ok_or("callee body is missing")?;
        if self.program.machine_states(target).is_empty() {
            return self.check_unknown_call(state, call, &origins);
        }
        if self
            .stack
            .iter()
            .any(|entry| entry.target.symbol == target.symbol)
        {
            // No recursive edge is accepted as an empty body. The completed
            // traversal must establish unconditional range fit for its entire
            // reached write inventory before a cycle can be closed.
            self.recursive = true;
            return self.check_recursive_origins(state, call, &origins);
        }
        self.stack.push(Invocation {
            state,
            call,
            target,
            origins,
        });
        let result = self.visit_body(target);
        self.stack.pop();
        result
    }

    fn visit_body(&mut self, machine: &'a Machine) -> Result<(), &'static str> {
        // Selected operators with write-capable operands have no ordinary
        // machine-call occurrence to traverse. Until their body is represented
        // in the same performed-write inventory, retain that coverage gap.
        for (_, operator_use) in self.facts.operators.uses.iter() {
            if operator_use.origin.machine_symbol() != Some(machine.symbol)
                || operator_use.status != checked_trees::CheckedOperatorResolutionStatus::Resolved
            {
                continue;
            }
            let operator = self
                .program
                .operators()
                .iter()
                .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
                .ok_or("selected operator has no exact declaration")?;
            if self
                .program
                .operator_parameters(operator)
                .iter()
                .any(|parameter| {
                    self.frames
                        .local_requires_write_origin(parameter.type_reference)
                })
            {
                return Err(
                    "write-capable selected operator has no complete performed-write inventory",
                );
            }
        }
        for body in self.program.machine_states(machine) {
            let flow = self
                .facts
                .flow
                .control
                .states
                .iter()
                .find_map(|(_, state)| {
                    (state.machine_symbol == machine.symbol && state.state_symbol == body.symbol)
                        .then_some(state)
                })
                .ok_or("callee state has no checked flow inventory")?;
            for (ordinal, statement) in self
                .program
                .statement_table
                .statements(body.statement_nodes)
                .iter()
                .enumerate()
            {
                if let StatementNode::Assignment(assignment) = statement {
                    let reference = validation::declared_place_type_raw(
                        self.program,
                        machine,
                        Some(body),
                        assignment.target,
                    )
                    .ok_or("assignment destination type is unresolved")?;
                    let value_type = validation::assignment_value_type(self.program, reference);
                    if self
                        .program
                        .primitive_type_reference(value_type)
                        .is_some_and(|primitive| primitive.accepts_integer_literal())
                    {
                        let mut rows =
                            self.facts
                                .mutation
                                .integer_writes
                                .iter()
                                .filter(|(_, row)| {
                                    row.machine == machine.symbol
                                        && row.state == body.symbol
                                        && row.statement_index == ordinal
                                        && row.destination == assignment.target
                                        && row.value == assignment.value
                                });
                        let (handle, row) = rows
                            .next()
                            .ok_or("integer assignment has no performed-write row")?;
                        if rows.next().is_some() {
                            return Err("integer assignment has duplicate performed-write rows");
                        }
                        let sources = self
                            .frames
                            .assignment_destination_origins(machine, statement)
                            .ok_or("performed write has unknown destination origins")?;
                        for source in sources {
                            let destinations =
                                self.destinations(machine, body, &source, self.stack.len())?;
                            for (owner, state, destination) in destinations {
                                if let Some((field, stored)) =
                                    validation::stored_integer_source_projection(
                                        self.program,
                                        owner,
                                        state,
                                        &destination,
                                        &[],
                                    )
                                    .map_err(
                                        |()| "performed write has no exact encoded projection",
                                    )?
                                {
                                    self.admit_write(handle, row, field, stored)?;
                                }
                            }
                        }
                    }
                }
                // Source occurrence coverage is separate from a may-write
                // footprint: a nested invocation cannot disappear from it.
                let sites = crate::semantic::calls::collect_call_sites_in_statement(
                    self.program,
                    machine,
                    body,
                    ordinal,
                );
                let calls = self.facts.flow.control.calls.span_or_empty(flow.calls);
                for call_ordinal in 0..sites.len() {
                    let mut matches = calls.iter().filter(|call| {
                        call.statement_index == ordinal && call.call_ordinal == call_ordinal
                    });
                    let call = matches
                        .next()
                        .ok_or("source invocation has no checked call occurrence")?;
                    if matches.next().is_some() {
                        return Err("source invocation has duplicate checked occurrences");
                    }
                    if !internal_state_call(self.program, flow, call) {
                        self.visit(flow, call)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn destinations(
        &self,
        machine: &'a Machine,
        state: &'a State,
        source: &LocalWriteSource,
        depth: usize,
    ) -> Result<Vec<(&'a Machine, &'a State, LocalWriteSource)>, &'static str> {
        if depth == 0 {
            return Ok(vec![(machine, state, source.clone())]);
        }
        let solved = self
            .entry_origins
            .iter()
            .find(|(symbol, _)| *symbol == machine.symbol)
            .map(|(_, solved)| solved)
            .ok_or("machine has no storage-origin solution")?;
        let sources =
            crate::flow::entry_storage_origins(self.program, machine, state, solved, source)
                .ok_or("write address has no finite invocation-entry origin")?;
        let entry = self
            .program
            .machine_states(machine)
            .first()
            .ok_or("callee entry is absent")?;
        let parameters = self.program.state_parameters(entry);
        let invocation = &self.stack[depth - 1];
        if invocation.target.symbol != machine.symbol {
            return Err("write origin crossed the wrong invocation");
        }
        let (caller, caller_state) = self.owner(invocation.state)?;
        let mut result = Vec::new();
        for source in sources {
            let Some(position) = parameters
                .iter()
                .position(|parameter| parameter.symbol == source.root)
            else {
                // The shared address solver retains proven private owned roots;
                // it refuses unknown reference-local roots rather than calling
                // their storage private.
                result.push((machine, state, source));
                continue;
            };
            let parameter = &parameters[position];
            let actuals = actual_origins(parameters, position, &invocation.origins)
                .ok_or("callee destination actual has unknown storage origins")?;
            if !is_reference(self.program, parameter.type_reference) {
                if actuals.is_empty() {
                    continue;
                }
                // A bare candidate list does not associate an aggregate's
                // individual reference slots with their captured referents.
                return Err("aggregate reference destination lacks an exact captured slot origin");
            }
            for actual in actuals {
                let substituted = actual
                    .substitute_storage(&source)
                    .ok_or("write projection cannot cross its captured source geometry")?;
                result.extend(self.destinations(caller, caller_state, &substituted, depth - 1)?);
            }
        }
        Ok(result)
    }

    fn call_may_require_fit(
        &self,
        state: &FlowStateFact,
        call: &FlowCallFact,
        origins: &CallWriteOrigins,
    ) -> Result<bool, &'static str> {
        let parameters =
            crate::semantic::calls::call_target_parameters(self.program, call.target_symbol)
                .ok_or("effectful invocation has no exact parameter roster")?;
        let (machine, body) = self.owner(state)?;
        let mut needs_fit = false;
        for (position, parameter) in parameters.iter().enumerate() {
            if !self
                .frames
                .local_requires_write_origin(parameter.type_reference)
            {
                continue;
            }
            let sources = actual_origins(parameters, position, origins)
                .ok_or("effectful invocation has unknown writable storage")?;
            for source in sources {
                for (owner, state, destination) in
                    self.destinations(machine, body, source, self.stack.len())?
                {
                    needs_fit |= validation::stored_integer_source_may_require_fit(
                        self.program,
                        owner,
                        state,
                        &destination,
                    )
                    .map_err(|()| "effectful invocation has unknown encoded reach")?;
                }
            }
        }
        Ok(needs_fit)
    }

    fn check_unknown_call(
        &self,
        state: &FlowStateFact,
        call: &FlowCallFact,
        origins: &CallWriteOrigins,
    ) -> Result<(), &'static str> {
        if self.call_has_no_writes(state, call)
            || !self.call_may_require_fit(state, call, origins)?
        {
            Ok(())
        } else {
            Err("effectful invocation has no complete performed-write body")
        }
    }

    fn check_recursive_origins(
        &self,
        state: &FlowStateFact,
        call: &FlowCallFact,
        origins: &CallWriteOrigins,
    ) -> Result<(), &'static str> {
        let symbol = call_values::target_machine(self.program, call)
            .ok_or("recursive callee identity is absent")?;
        let (prior_index, prior) = self
            .stack
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.target.symbol == symbol)
            .ok_or("recursive source invocation is absent")?;
        let parameters =
            crate::semantic::calls::call_target_parameters(self.program, call.target_symbol)
                .ok_or("recursive parameter roster is absent")?;
        let (caller, body) = self.owner(state)?;
        let (prior_caller, prior_body) = self.owner(prior.state)?;
        for (position, parameter) in parameters.iter().enumerate() {
            if !self
                .frames
                .local_requires_write_origin(parameter.type_reference)
            {
                continue;
            }
            let current = actual_origins(parameters, position, origins)
                .ok_or("recursive writable actual has unknown origins")?;
            let previous = actual_origins(parameters, position, &prior.origins)
                .ok_or("recursive entry actual has unknown origins")?;
            let mut current_places = Vec::new();
            let mut previous_places = Vec::new();
            for source in current {
                current_places.extend(self.destinations(caller, body, source, self.stack.len())?);
            }
            for source in previous {
                previous_places.extend(self.destinations(
                    prior_caller,
                    prior_body,
                    source,
                    prior_index,
                )?);
            }
            if current_places.len() != previous_places.len()
                || current_places.iter().any(|(machine, state, source)| {
                    !previous_places
                        .iter()
                        .any(|(other_machine, other_state, other)| {
                            machine.symbol == other_machine.symbol
                                && state.symbol == other_state.symbol
                                && same_source(source, other)
                        })
                })
            {
                return Err(
                    "recursive write address substitution does not preserve the finite checked footprint",
                );
            }
        }
        Ok(())
    }

    fn admit_write(
        &mut self,
        handle: arena::Handle<PerformedIntegerWrite>,
        write: &PerformedIntegerWrite,
        field: SymbolHandle,
        stored: semantic_vocabulary::IntegerType,
    ) -> Result<(), &'static str> {
        let mut predicate = crate::facts::integer_write_obligation(write, stored)
            .ok_or("performed integer value has no fitting bounds or entry relation")?;
        self.all_ranges_fit &= predicate == Boolean::Constant(true);
        for invocation in self.stack.iter().rev() {
            if predicate == Boolean::Constant(true) {
                break;
            }
            let contexts = self
                .facts
                .flow
                .semantic_constraint_contexts(invocation.call.entry_constraints)
                .collect::<Vec<_>>();
            if call_values::proves_entry_predicate(
                self.program,
                self.facts,
                invocation.state,
                invocation.call,
                &contexts,
                invocation.target.symbol,
                predicate.clone(),
            ) {
                predicate = Boolean::Constant(true);
                break;
            }
            predicate = call_values::entry_predicate(
                self.program,
                self.facts,
                invocation.state,
                invocation.call,
                invocation.target.symbol,
                predicate,
            )
            .ok_or("performed write cannot transport its actual value relation")?;
            if let Some(condition) = crate::facts::boolean_predicates::entry_path_condition(
                &self.facts.semantic,
                &contexts,
                invocation.state.machine_symbol,
            ) && condition != Boolean::Constant(true)
            {
                predicate = Boolean::Or {
                    left: Box::new(Boolean::Not(Box::new(condition))),
                    right: Box::new(predicate),
                };
            }
        }
        if predicate != Boolean::Constant(true) {
            return Err("performed integer value is not proven to fit");
        }
        let root = self
            .stack
            .first()
            .ok_or("write has no consuming invocation")?;
        self.decisions.push(CheckedStoredIntegerWrite {
            machine: root.state.machine_symbol,
            state: root.state.state_symbol,
            site: StoredIntegerWriteSite::Call {
                statement_index: root.call.statement_index,
                call_ordinal: root.call.call_ordinal,
                write: handle,
            },
            value: write.value,
            field,
            stored_type: stored,
        });
        Ok(())
    }
}

fn actual_origins<'a>(
    parameters: &[typed_trees::signature::StateParameter],
    position: usize,
    origins: &'a CallWriteOrigins,
) -> Option<&'a [LocalWriteSource]> {
    if parameters.get(position)?.is_self {
        origins.receiver.as_deref()
    } else {
        let ordinal = parameters[..position]
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count();
        origins.arguments.get(ordinal)?.as_deref()
    }
}

fn is_reference(
    program: &TypedTrees,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            typed_trees::types::TypeReferenceNode::Reference { .. } => return true,
            _ => return false,
        }
    }
}

fn same_source(left: &LocalWriteSource, right: &LocalWriteSource) -> bool {
    left.root == right.root
        && left.segments == right.segments
        && left.representation == right.representation
        && left.collection_coarse == right.collection_coarse
}
