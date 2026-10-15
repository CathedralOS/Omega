//! Typed invocation storage, using the same caller prefix as write frames.
use super::super::caller_aliases::{
    CallOriginContext, CallWriteOrigins, CallerWriteSite, LocalWriteSource, caller_statement_owner,
    with_caller_origin_context,
};
use super::{CallFrameResolver, ExpressionHandle, Machine, TableCall};
use crate::machine_calls::calls::write_frames as frames;
use symbols::JoinedPath;

impl<'program> CallFrameResolver<'program> {
    pub fn call_write_origins(
        &self,
        machine: &'program Machine,
        call: &'program TableCall,
    ) -> Option<CallWriteOrigins> {
        self.invocation_write_origins(machine, CallerWriteSite::Call(call))
    }

    pub fn expression_call_write_origins(
        &self,
        machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> Option<CallWriteOrigins> {
        self.invocation_write_origins(machine, CallerWriteSite::Expression(expression))
    }

    pub fn transition_write_origins(
        &self,
        machine: &'program Machine,
        target: typed_trees::statement::TransitionTargetHandle,
    ) -> Option<CallWriteOrigins> {
        self.invocation_write_origins(machine, CallerWriteSite::Transition(target))
    }

    /// Resolve the exact operand roster retained by operator selection through
    /// the same chronological origin capture as ordinary invocations. This
    /// query supplies storage, not permission or operator-selection authority.
    pub fn operator_write_origins(
        &self,
        machine: &'program Machine,
        operator_use: &checked_trees::CheckedOperatorUseFact,
        parameter_types: &[typed_trees::types::TypeReferenceHandle],
    ) -> Option<CallWriteOrigins> {
        let arguments = operator_use.operands(self.program)?;
        if arguments.len() != parameter_types.len()
            || operator_use
                .origin
                .machine_symbol()
                .is_some_and(|owner| owner != machine.symbol)
        {
            return None;
        }
        let site = CallerWriteSite::Expression(operator_use.expression);
        let (state, statement, ordinal) = caller_statement_owner(self.program, machine, site)?;
        if let checked_trees::CheckedValueOrigin::StateStatement {
            state_symbol,
            statement_index,
            ..
        } = operator_use.origin
            && (state_symbol != state.symbol || statement_index != ordinal)
        {
            return None;
        }
        let preceding_effect = !matches!(
            operator_use.occurrence,
            checked_trees::CheckedOperatorOccurrence::Expression
        ) || self.prefix_may_change_origins(machine, statement, site);
        with_caller_origin_context(
            self.program,
            machine,
            &self.tables.symbols,
            site,
            &self.caches.state_write_collections,
            |inference, prefix| {
                Some(CallWriteOrigins {
                    receiver: Some(Vec::new()),
                    arguments: self.capture_operand_origins(
                        machine,
                        inference,
                        prefix,
                        &arguments,
                        parameter_types,
                        preceding_effect,
                    )?,
                })
            },
        )
    }

    fn invocation_write_origins(
        &self,
        machine: &'program Machine,
        site: CallerWriteSite<'_>,
    ) -> Option<CallWriteOrigins> {
        let program = self.program;
        let machine_symbols = self.machine_symbols(machine)?;
        // Unlike a footprint's untracked shortcut, a public typed query must
        // establish that this is a real occurrence in the selected owner.
        let (state, statement, _) = caller_statement_owner(program, machine, site)?;
        let (target, name, receiver_expression, receiver_members, arguments) = match site {
            CallerWriteSite::Call(call) => (
                call.target_symbol,
                call.target.as_str(),
                ExpressionHandle::invalid(),
                crate::machine_calls::calls::signature_queries::call_receiver_member_names(
                    program, call,
                ),
                program.statement_table.expression_handles(call.arguments),
            ),
            CallerWriteSite::Expression(expression) => {
                let Some(call) = program.expression_table.call_node(expression) else {
                    return None;
                };
                (
                    call.target_symbol,
                    call.target.as_str(),
                    call.receiver,
                    frames::receiver_member_chain(program, call.receiver).unwrap_or_default(),
                    program.expression_table.expression_handles(call.arguments),
                )
            }
            CallerWriteSite::Transition(target) => {
                let typed_trees::statement::TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(target)
                else {
                    return None;
                };
                (
                    path.symbol,
                    "",
                    ExpressionHandle::invalid(),
                    Vec::new(),
                    program.statement_table.expression_handles(*arguments),
                )
            }
            _ => return None,
        };
        let argument_types = if matches!(site, CallerWriteSite::Transition(_)) {
            let entry = frames::machine_state_by_symbol(program, target)
                .map(|(_, state)| state)
                .or_else(|| {
                    program
                        .machine_by_symbol(target)
                        .and_then(|target_machine| program.machine_states(target_machine).first())
                })?;
            program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.type_reference)
                .collect()
        } else {
            frames::call_targets::call_argument_types(
                program,
                machine,
                target,
                name,
                &receiver_members,
                site,
                &self.tables.symbols,
            )
        };
        with_caller_origin_context(
            program,
            machine,
            &self.tables.symbols,
            site,
            &self.caches.state_write_collections,
            |inference, prefix| {
                // A missing result in one operand does not erase known siblings.
                let mut result = CallWriteOrigins {
                    receiver: Some(Vec::new()),
                    arguments: Vec::new(),
                };
                let callee_is_resolved = frames::machine_state_by_symbol(program, target).is_some();
                // A nested call can follow effects in an enclosing operand. The
                // prefix owner has not captured those substatement updates.
                // Reuse its expression inventory and frame query; do not
                // rediscover an evaluation order from rendered paths.
                let nested_effect = self.prefix_may_change_origins(machine, statement, site);
                let preceding_effect = nested_effect
                    || (receiver_expression.is_valid()
                        && self.operand_may_change_origins(machine, receiver_expression));
                if !callee_is_resolved
                    && (!receiver_members.is_empty() || receiver_expression.is_valid())
                {
                    result.receiver = if receiver_expression.is_valid() {
                        if preceding_effect {
                            None
                        } else {
                            prefix
                                .argument_origins(
                                    program,
                                    machine,
                                    &machine_symbols,
                                    &self.tables.symbols,
                                    inference,
                                    &[receiver_expression],
                                    &[],
                                )
                                .and_then(|mut origins| origins.pop().flatten())
                                .map(|origins| {
                                    origins.into_iter().map(LocalWriteSource::from).collect()
                                })
                        }
                    } else if let CallerWriteSite::Call(call) = site {
                        self.statement_receiver_origins(machine, state, statement, call)
                    } else {
                        None
                    };
                }
                result.arguments = self.capture_operand_origins(
                    machine,
                    inference,
                    prefix,
                    arguments,
                    &argument_types,
                    preceding_effect,
                )?;
                Some(result)
            },
        )
    }

    fn capture_operand_origins(
        &self,
        machine: &'program Machine,
        inference: &mut frames::FrameInference,
        prefix: &CallOriginContext<'_>,
        arguments: &[ExpressionHandle],
        argument_types: &[typed_trees::types::TypeReferenceHandle],
        mut preceding_effect: bool,
    ) -> Option<Vec<Option<Vec<LocalWriteSource>>>> {
        let program = self.program;
        let machine_symbols = self.machine_symbols(machine)?;
        let mut result = Vec::with_capacity(arguments.len());
        for (index, argument) in arguments.iter().enumerate() {
            let changes_origins = self.operand_may_change_origins(machine, *argument);
            let reference = argument_types.get(index).copied();
            let detached = reference.is_some_and(|reference| {
                frames::type_is_caller_isolated_local(program, reference)
                    && !frames::type_reference_is_reference(program, reference)
            });
            let origins = if detached {
                Some(Vec::new())
            } else if preceding_effect || changes_origins {
                // The prefix predates operand execution. Never pretend
                // a later operand still sees that prefix after a write.
                // Earlier captured referents remain unchanged.
                None
            } else {
                prefix
                    .argument_origins(
                        program,
                        machine,
                        &machine_symbols,
                        &self.tables.symbols,
                        inference,
                        &[*argument],
                        &reference.into_iter().collect::<Vec<_>>(),
                    )
                    .and_then(|mut origins| origins.pop().flatten())
                    .map(|origins| origins.into_iter().map(LocalWriteSource::from).collect())
            };
            result.push(origins);
            preceding_effect |= changes_origins;
        }
        Some(result)
    }

    fn prefix_may_change_origins(
        &self,
        machine: &'program Machine,
        statement: &typed_trees::statement::StatementNode,
        site: CallerWriteSite<'_>,
    ) -> bool {
        match site {
            CallerWriteSite::Expression(expression) => {
                let roots = frames::statement_value_expression_roots(self.program, statement);
                !roots.contains(&expression)
                    && !self
                        .statement_value_may_write_paths(machine, statement)
                        .is_some_and(|paths| paths.is_empty())
            }
            CallerWriteSite::Transition(_) => {
                let Some(transition) = statement.as_transition() else {
                    return true;
                };
                match transition.guard {
                    typed_trees::statement::TransitionGuardNode::When(condition) => {
                        self.operand_may_change_origins(machine, condition)
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn operand_may_change_origins(
        &self,
        machine: &'program Machine,
        expression: ExpressionHandle,
    ) -> bool {
        !self
            .expression_write_frame(machine, expression)
            .into_complete_paths()
            .is_some_and(|paths| paths.is_empty())
    }

    fn statement_receiver_origins(
        &self,
        machine: &Machine,
        state: &typed_trees::state::State,
        statement: &typed_trees::statement::StatementNode,
        call: &TableCall,
    ) -> Option<Vec<LocalWriteSource>> {
        let program = self.program;
        let members = program.statement_table.name_path_members(call.receiver);
        let root = call.receiver_root_symbol;
        let root_reference = program
            .state_parameter_by_symbol(state, root)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                program
                    .state_local_declaration(state, root)
                    .map(|(_, local)| local.type_reference)
            })?;
        let (segments, _) = frames::reference_origins::retained_receiver_projection(
            program,
            root_reference,
            root,
            call.receiver_symbol,
            members.get(1..)?.iter().map(|member| member.as_str()),
        )?;
        let inventory = self.local_write_origins_before_statement(machine, statement)?;
        let longest = inventory
            .iter()
            .filter(|origin| {
                origin.local_symbol == root && segments.starts_with(&origin.local_segments)
            })
            .map(|origin| origin.local_segments.len())
            .max();
        if let Some(longest) = longest {
            let mut sources = Vec::new();
            for origin in inventory.iter().filter(|origin| {
                origin.local_symbol == root
                    && origin.local_segments.len() == longest
                    && segments.starts_with(&origin.local_segments)
            }) {
                let mut source = origin.source.clone()?;
                let suffix = &segments[longest..];
                let carrier = crate::value_custody::places::unwrapped_type_reference(
                    program,
                    root_reference,
                )?;
                let slot = frames::stored_origins::projected_storage_type(
                    program,
                    carrier,
                    &segments[..longest],
                )?;
                frames::reference_subjects::validate_owned_projection(program, slot, suffix)?;
                if source.collection_coarse && !suffix.is_empty() {
                    return None;
                }
                source.segments.extend_from_slice(suffix);
                let suffix_names = members.get(longest + 1..)?.joined_path_with(".");
                if !suffix_names.is_empty() {
                    source.path =
                        frames::append_place_suffix(&source.path, &format!(".{suffix_names}"));
                }
                if let Some((_, path)) = &mut source.representation {
                    path.extend_from_slice(suffix);
                }
                sources.push(source);
            }
            return Some(sources);
        }
        if program.state_local_declaration(state, root).is_some()
            && frames::type_reference_is_reference(program, root_reference)
        {
            return None;
        }
        // This validates every selected declaration and refuses crossing a
        // stored reference without its retained origin.
        frames::reference_subjects::validate_owned_projection(program, root_reference, &segments)?;
        Some(vec![LocalWriteSource {
            is_image_storage: false,
            recasts: Vec::new(),
            root,
            segments,
            representation: None,
            collection_coarse: false,
            path: members.joined_path_with("."),
        }])
    }
}
