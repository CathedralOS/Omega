//! Concrete encoded stores performed through ordinary invocations. Each leaf
//! write is checked at the actual storage and operand snapshots of this call;
//! a returned value or an aggregate mutation footprint cannot replace it.
//! A cycle requires fitting evidence established inside its invocation, not
//! merely at its first arrival. Track that boundary per active invocation:
//! reaching a cycle does not invalidate caller-proven stores outside it, and
//! acyclic helper substitution can establish a store within the cyclic body.
use super::contracts::call_values;
use checked_trees::{
    CheckFacts, CheckedBooleanExpression as Boolean, CheckedStoredIntegerWrite, FlowCallFact,
    FlowStateFact, PerformedIntegerWrite, StoredIntegerWriteSite,
};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{machine::Machine, state::State};
use validation::{CallFrameResolver, CallWriteOrigins, LocalWriteSource};

struct Invocation<'a> {
    state: &'a FlowStateFact,
    call: &'a FlowCallFact,
    target: &'a Machine,
    origins: CallWriteOrigins,
    cyclic: bool,
    /// Every reached store fits before crossing this invocation's entry.
    interior_writes_fit: bool,
}

struct Admission<'a> {
    program: &'a dyn ::validation::CheckingInput,
    facts: &'a CheckFacts,
    frames: &'a CallFrameResolver<'a>,
    entry_origins: &'a [(SymbolHandle, crate::flow::MachineParameterOrigins)],
    stack: Vec<Invocation<'a>>,
    decisions: Vec<CheckedStoredIntegerWrite>,
}

pub(super) fn check(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    frames: Option<&CallFrameResolver<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    if !program.plan_laid_layouts.iter().any(|layout| {
        layout.geometries(program.typed_program()).any(|geometry| {
            geometry
                .integer_fields
                .iter()
                .any(|field| !field.write_is_total)
        })
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
    let proof_only = validation::proof_only_classification(program);
    for machine in program.machines() {
        if proof_only.is_proof_machine(program, machine) {
            continue;
        }
        let mut admission = Admission {
            program,
            facts,
            frames,
            entry_origins: &entry_origins,
            stack: Vec::new(),
            decisions: Vec::new(),
        };
        let result = admission.visit_body(machine);
        match result {
            Ok(()) => decisions.extend(admission.decisions),
            Err(reason) => diagnostics.push(Diagnostic::error(format!(
                "IntegerAt call in {} requires every performed value to fit its physical storage: {reason}",
                program.symbols.name(machine.symbol),
            ))),
        }
    }
    if diagnostics.is_empty() {
        facts.stored_integer_writes.insert_many(decisions);
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn internal_state_call(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> bool {
    call_values::target_machine(program, call).is_none()
        && validation::find_state(program, call.target_symbol)
            .is_some_and(|(machine, _)| machine.symbol == state.machine_symbol)
}

impl<'a> Admission<'a> {
    fn owner(&self, state: &FlowStateFact) -> Result<(&'a Machine, &'a State), &'static str> {
        validation::machine_state_by_symbols(self.program, state.machine_symbol, state.state_symbol)
            .ok_or("call has no exact source owner")
    }

    fn call_origins(&self, state: &FlowStateFact, call: &FlowCallFact) -> Option<CallWriteOrigins> {
        let (machine, body) = self.owner(state).ok()?;
        match validation::find_call_site(
            self.program,
            machine.symbol,
            body.symbol,
            call.statement_index,
            call.call_ordinal,
        )? {
            validation::CallSite::Statement(call) => self.frames.call_write_origins(machine, call),
            validation::CallSite::Expression { expression, .. } => self
                .frames
                .expression_call_write_origins(machine, expression),
            validation::CallSite::TransitionNamed { .. } => {
                let target = validation::transition_call_target(
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
        match validation::find_call_site(
            self.program,
            machine.symbol,
            body.symbol,
            call.statement_index,
            call.call_ordinal,
        ) {
            Some(validation::CallSite::Statement(call)) => self
                .frames
                .may_write_frame(machine, call)
                .complete_paths()
                .is_some_and(|paths| paths.is_empty()),
            Some(validation::CallSite::Expression { expression, .. }) => self
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
            .any(|access| access.kind.is_exclusive() && access.end_before_access.is_none());
        let declared_writable =
            validation::call_target_parameters(self.program, call.target_symbol).is_some_and(
                |parameters| {
                    parameters
                        .iter()
                        .any(|parameter| self.runtime_write_parameter(parameter))
                },
            );
        if !writable && !declared_writable {
            return Ok(());
        }
        if call_values::target_machine(self.program, call)
            .and_then(|symbol| self.program.machine_by_symbol(symbol))
            .is_some_and(|machine| {
                validation::proof_only_classification(self.program)
                    .is_proof_machine(self.program, machine)
            })
        {
            return Ok(());
        }
        let origins = match self.call_origins(state, call) {
            Some(origins) => origins,
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
        if let Some(cycle_entry) = self
            .stack
            .iter()
            .position(|entry| entry.target.symbol == target.symbol)
        {
            // No recursive edge is accepted as an empty body. The completed
            // traversal must establish fit within each member of this cycle,
            // including writes visited before discovery of the backedge.
            for invocation in &mut self.stack[cycle_entry..] {
                invocation.cyclic = true;
            }
            return self.check_recursive_origins(state, call, &origins);
        }
        self.stack.push(Invocation {
            state,
            call,
            target,
            origins,
            cyclic: false,
            interior_writes_fit: true,
        });
        let result = self.visit_body(target);
        let invocation = self
            .stack
            .pop()
            .ok_or("invocation storage frame is absent")?;
        result?;
        if invocation.cyclic && !invocation.interior_writes_fit {
            Err("recursive write values have no occurrence-wide fitting bounds")
        } else {
            Ok(())
        }
    }

    fn check_operator_invocations(&self, machine: &'a Machine) -> Result<(), &'static str> {
        for (handle, operator_use) in self.facts.operators.uses.iter() {
            if operator_use.origin.machine_symbol() != Some(machine.symbol)
                || operator_use.status != checked_trees::CheckedOperatorResolutionStatus::Resolved
            {
                continue;
            }
            let operator = typed_trees::operator::declaration_by_symbol(
                self.program,
                operator_use.selected_operator_symbol,
            )
            .ok_or("selected operator has no exact declaration")?;
            let parameters = self.program.operator_parameters(operator);
            if !parameters
                .iter()
                .any(|parameter| self.runtime_write_parameter(parameter))
            {
                continue;
            }
            let checked_trees::CheckedValueOrigin::StateStatement { state_symbol, .. } =
                operator_use.origin
            else {
                return Err("write-capable operator has no runtime occurrence owner");
            };
            let body = self
                .program
                .state_in_machine(machine, state_symbol)
                .ok_or("selected operator state is absent")?;
            let operands = operator_use
                .operands(self.program)
                .ok_or("selected operator has no exact operand roster")?;
            let mut invocations = self
                .facts
                .flow
                .control
                .operator_invocations
                .iter()
                .filter(|(_, invocation)| invocation.operator_use == handle);
            let (_, invocation) = invocations
                .next()
                .ok_or("write-capable operator has no captured invocation")?;
            if invocations.next().is_some()
                || self
                    .facts
                    .flow
                    .control
                    .operands
                    .span_or_empty(invocation.operands)
                    .iter()
                    .map(|operand| operand.expression)
                    .ne(operands.iter().copied())
                || parameters.len() != operands.len()
            {
                return Err("selected operator operand custody disagrees with its invocation");
            }
            let types = parameters
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect::<Vec<_>>();
            let origins = self
                .frames
                .operator_write_origins(machine, operator_use, &types)
                .ok_or("write-capable operator has unknown operand storage")?;
            for (position, parameter) in parameters.iter().enumerate() {
                if !self.runtime_write_parameter(parameter) {
                    continue;
                }
                let sources = actual_origins(parameters, position, &origins)
                    .ok_or("write-capable operator has unknown actual storage")?;
                for source in sources {
                    for (_owner, state, destination) in
                        self.destinations(machine, body, source, self.stack.len())?
                    {
                        if validation::stored_integer_source_may_require_fit(
                            self.program,
                            state,
                            &destination,
                        )
                        .map_err(|()| "write-capable operator has unknown encoded reach")?
                        {
                            return Err(
                                "write-capable selected operator has no complete performed-write inventory",
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn visit_body(&mut self, machine: &'a Machine) -> Result<(), &'static str> {
        self.check_operator_invocations(machine)?;
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
                if let Some(assignment) = statement.as_assignment() {
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
                            .assignment_storage_origins(machine, statement)
                            .map(|storage| storage.destinations)
                            .or_else(|| {
                                self.owned_destination(machine, body, ordinal, assignment.target)
                            })
                            .ok_or("performed write has unknown destination origins")?;
                        for source in sources {
                            let destinations =
                                self.destinations(machine, body, &source, self.stack.len())?;
                            for (_owner, state, destination) in destinations {
                                for (field, stored) in
                                    validation::stored_integer_source_projections(
                                        self.program,
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
                let sites = validation::collect_call_sites_in_statement(
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

    /// Owned storage has its declaration's geometry even when the alias
    /// inventory has no entry for it. This fallback cannot turn a reference
    /// endpoint into ordinary storage: the whole owner must contain no loans.
    fn owned_destination(
        &self,
        _machine: &Machine,
        state: &State,
        ordinal: usize,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<Vec<LocalWriteSource>> {
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            state.symbol,
            ordinal,
            expression,
        )?;
        let root = place.root.as_symbol()?;
        let reference = self
            .program
            .state_parameter_by_symbol(state, root)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                self.program
                    .state_local_declaration(state, root)
                    .map(|(_, local)| local.type_reference)
            })?;
        if is_reference(self.program, reference)
            || !validation::has_contents(
                self.program,
                reference,
                &[],
                validation::ContentsRequirement::CleanupOwned,
            )
        {
            return None;
        }
        Some(vec![LocalWriteSource {
            root,
            segments: place.segments,
            representation: None,
            recasts: Vec::new(),
            collection_coarse: false,
            path: String::new(),
        }])
    }

    fn destinations(
        &self,
        machine: &'a Machine,
        state: &'a State,
        source: &LocalWriteSource,
        depth: usize,
    ) -> Result<Vec<(&'a Machine, &'a State, LocalWriteSource)>, &'static str> {
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
        if depth == 0 {
            return Ok(sources
                .into_iter()
                .map(|source| {
                    let owner_state = if self
                        .program
                        .state_parameter_by_symbol(entry, source.root)
                        .is_some()
                    {
                        entry
                    } else {
                        state
                    };
                    (machine, owner_state, source)
                })
                .collect());
        }
        let parameters = self.program.state_parameters(entry);
        let invocation = &self.stack[depth - 1];
        if invocation.target.symbol != machine.symbol {
            return Err("write origin crossed the wrong invocation");
        }
        let (caller, caller_state) = self.owner(invocation.state)?;
        let mut result = Vec::new();
        for source in sources {
            let Some(position) =
                typed_trees::signature::state_parameter_position(parameters, source.root)
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

    fn runtime_write_parameter(&self, parameter: &typed_trees::signature::StateParameter) -> bool {
        !parameter.relevance.is_erased()
            && !self
                .frames
                .proof_value_is_caller_isolated(parameter.type_reference)
            && self
                .frames
                .local_requires_write_origin(parameter.type_reference)
    }

    fn call_may_require_fit(
        &self,
        state: &FlowStateFact,
        call: &FlowCallFact,
        origins: &CallWriteOrigins,
    ) -> Result<bool, &'static str> {
        let parameters = validation::call_target_parameters(self.program, call.target_symbol)
            .ok_or("effectful invocation has no exact parameter roster")?;
        let (machine, body) = self.owner(state)?;
        let mut needs_fit = false;
        for (position, parameter) in parameters.iter().enumerate() {
            if !self.runtime_write_parameter(parameter) {
                continue;
            }
            let sources = actual_origins(parameters, position, origins)
                .ok_or("effectful invocation has unknown writable storage")?;
            for source in sources {
                for (_owner, state, destination) in
                    self.destinations(machine, body, source, self.stack.len())?
                {
                    needs_fit |= validation::stored_integer_source_may_require_fit(
                        self.program,
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
        let parameters = validation::call_target_parameters(self.program, call.target_symbol)
            .ok_or("recursive parameter roster is absent")?;
        let (caller, body) = self.owner(state)?;
        let (prior_caller, prior_body) = self.owner(prior.state)?;
        for (position, parameter) in parameters.iter().enumerate() {
            if !self.runtime_write_parameter(parameter) {
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
        // Ordinary assignment checking already owns occurrence arithmetic,
        // guards, and physical landing. Reuse that exact source judgment when
        // its destination, value, field and width all match this transported
        // write; a weaker entry-only value summary must not invalidate it.
        let established = self.facts.stored_integer_writes.iter().any(|(_, checked)| {
            checked.machine == write.machine
                && checked.state == write.state
                && checked.site
                    == StoredIntegerWriteSite::Assignment {
                        destination: write.destination,
                    }
                && checked.value == write.value
                && checked.field == field
                && checked.stored_type == stored
        });
        let mut predicate = if established {
            Boolean::Constant(true)
        } else {
            crate::facts::integer_write_obligation(write, stored)
                .ok_or("performed integer value has no fitting bounds or entry relation")?
        };
        for invocation in self.stack.iter_mut().rev() {
            // Proof established below this entry applies on every arrival.
            // Crossing the entry may instead use only the current caller's
            // actual value, which cannot justify later recursive arrivals.
            invocation.interior_writes_fit &= predicate == Boolean::Constant(true);
            if predicate == Boolean::Constant(true) {
                continue;
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
                Some(self.frames),
            ) {
                predicate = Boolean::Constant(true);
                continue;
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
        let (machine, state, site) = match self.stack.first() {
            Some(root) => (
                root.state.machine_symbol,
                root.state.state_symbol,
                StoredIntegerWriteSite::Call {
                    statement_index: root.call.statement_index,
                    call_ordinal: root.call.call_ordinal,
                    write: handle,
                },
            ),
            None => (
                write.machine,
                write.state,
                StoredIntegerWriteSite::Assignment {
                    destination: write.destination,
                },
            ),
        };
        let decision = CheckedStoredIntegerWrite {
            machine,
            state,
            site,
            value: write.value,
            field,
            stored_type: stored,
        };
        if !self
            .facts
            .stored_integer_writes
            .iter()
            .any(|(_, row)| row == &decision)
            && !self.decisions.contains(&decision)
        {
            self.decisions.push(decision);
        }
        Ok(())
    }
}

fn actual_origins<'a>(
    parameters: &[typed_trees::signature::StateParameter],
    position: usize,
    origins: &'a CallWriteOrigins,
) -> Option<&'a [LocalWriteSource]> {
    parameters.get(position)?;
    origins.arguments.get(position)?.as_deref()
}

fn is_reference(
    program: &dyn ::validation::CheckingInput,
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
        && left.recasts == right.recasts
        && left.collection_coarse == right.collection_coarse
}
