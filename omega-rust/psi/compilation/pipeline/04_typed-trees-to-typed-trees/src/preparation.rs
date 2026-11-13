/*
Typed elaboration coordinator (`prepare_typed_trees`, `PreparationRequest`,
`PreparationAdmission`, and `TypedPreparation`), constant and runtime value
application preparation, array extents, opaque property receipts, and value
location alignment.
*/

pub(crate) mod array_extents {
    use language_semantics::const_value::DecodedCanonicalConstValue;
    use typed_trees::{
        TypedTrees,
        types::{FixedArrayLength, TypeReferenceNode},
    };

    pub(crate) fn normalize(program: &mut TypedTrees, start: usize) {
        let pending = program
            .type_reference_table
            .type_reference_rows_from(start)
            .filter_map(|(array, node)| {
                let TypeReferenceNode::FixedArray {
                    length: FixedArrayLength::Expression(argument),
                    ..
                } = node
                else {
                    return None;
                };
                let TypeReferenceNode::ConstExpression(expression) =
                    program.type_reference_table.type_reference(*argument)
                else {
                    return None;
                };
                let supplier = program
                    .type_reference_table
                    .operand_lifetime_owner(*argument);
                let owner = if supplier.is_valid() {
                    supplier
                } else {
                    program.type_reference_table.operand_lifetime_owner(array)
                };
                owner.is_valid().then_some((array, *expression, owner))
            })
            .collect::<Vec<_>>();
        for (array, expression, owner) in pending {
            let Some(carrier) = program
                .closed_integer_value_in(expression, owner)
                .and_then(|value| value.type_reference)
            else {
                continue;
            };
            let Ok(Some(value)) =
                validation::check_closed_const_expression(program, owner, expression, carrier)
            else {
                continue;
            };
            let Some(DecodedCanonicalConstValue::Integer { value, .. }) = value.decode_encoding()
            else {
                continue;
            };
            let Ok(length) = usize::try_from(value) else {
                continue;
            };
            program
                .type_reference_table
                .set_fixed_array_length(array, length);
        }
    }
}
pub(crate) mod constant_entries {
    use crate::tables::{expressions_of, expressions_of_mut};
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::constant_recipes::{RetainedConstantPosition, create_evaluation_entry};

    pub(crate) fn materialize(program: &mut TypedTrees) -> Result<usize, Vec<Diagnostic>> {
        let mut initializers = Vec::new();
        for declaration in program.const_declarations() {
            if declaration.evaluated_value.is_some()
                || !declaration.evaluation_entry.is_valid()
                || has_open_scope(program, declaration.symbol)
            {
                continue;
            }
            if !program
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
            {
                return Err(vec![Diagnostic::error(
                    "constant initializer lost its retained evaluation entry",
                )]);
            }
            let entry = program.tables.machines.get(declaration.evaluation_entry);
            if program.machine_by_symbol(entry.symbol).is_none() {
                initializers.push(entry.clone());
            }
        }
        let initializer_count = initializers.len();
        for entry in initializers {
            program.push_machine(entry);
        }
        let mut owners = typed_trees::references::program_scoped_proof_facts(program)
            .map(|(owner, _, _)| owner)
            .collect::<Vec<_>>();
        owners.sort_unstable_by_key(|owner| (owner.arena_index(), owner.generation()));
        owners.dedup();
        let operands = typed_trees::references::collect_program_operand_owners(program);
        let mut pending = Vec::new();
        for (_, recipe) in program.constant_recipes.iter() {
            let expression = recipe.position.expression();
            if !expression.is_valid() {
                continue;
            }
            if expressions_of(program)
                .expression_handles(recipe.invocations)
                .iter()
                .any(|invocation| !expressions_of(program).expression_is_valid(*invocation))
            {
                return Err(vec![Diagnostic::error(
                    "constant entry lost its invocation correspondence",
                )]);
            }
            for handle in recipe.contexts.handles() {
                if !program.constant_expression_contexts.is_valid(handle) {
                    return Err(vec![Diagnostic::error(
                        "constant entry lost its lexical context",
                    )]);
                }
                let context = *program.constant_expression_contexts.get(handle);
                let live = match recipe.position {
                    RetainedConstantPosition::ContractExpression(_) => {
                        owners.contains(&context.owner)
                    }
                    RetainedConstantPosition::ConstApplication { site, .. } => operands
                        .type_owners(site)
                        .any(|owner| owner == context.owner),

                    _ => false,
                };
                if context.evaluation_entry.is_valid()
                    || !live
                    || (has_open_scope(program, context.owner)
                        && !has_closed_inputs(program, expression, context.result_type))
                {
                    continue;
                }
                pending.push((handle, context, expression));
            }
        }
        let count = initializer_count + pending.len();
        for (handle, context, expression) in pending {
            if !expressions_of(program).expression_is_valid(expression) {
                return Err(vec![Diagnostic::error(
                    "constant entry lost its complete expression",
                )]);
            }
            let expression = expressions_of_mut(program).insert_copy(expression);
            let entry = create_evaluation_entry(
                program,
                context.owner,
                format!("@constant-expression-{}", handle.arena_index()),
                expression,
                context.result_type,
            );
            let context = program.constant_expression_contexts.get_mut(handle);
            context.evaluation_entry = entry.symbol;
            context.evaluation_expression = expression;
            program.push_machine(entry);
        }
        Ok(count)
    }

    fn has_closed_inputs(
        program: &TypedTrees,
        expression: typed_trees::expression::ExpressionHandle,
        result_type: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        let mut inputs = ClosedInputs {
            program,
            closed: true,
            pending_binders: Vec::new(),
            bound_scopes: Vec::new(),
        };
        typed_trees::references::visit_expression(program, &mut inputs, expression);
        typed_trees::references::visit_type_reference(program, &mut inputs, result_type);
        inputs.closed
    }

    struct ClosedInputs<'program> {
        program: &'program TypedTrees,
        closed: bool,
        pending_binders: Vec<SymbolHandle>,
        bound_scopes: Vec<Vec<SymbolHandle>>,
    }

    impl Extend<SymbolHandle> for ClosedInputs<'_> {
        fn extend<I: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: I) {
            use symbols::SymbolKind;
            for symbol in symbols {
                if !symbol.is_valid()
                    || self
                        .bound_scopes
                        .iter()
                        .any(|scope| scope.contains(&symbol))
                {
                    continue;
                }
                self.closed &= match self.program.symbols.get(symbol).kind {
                    SymbolKind::Unknown
                    | SymbolKind::Parameter
                    | SymbolKind::Local
                    | SymbolKind::TypeParameter
                    | SymbolKind::MachineParameter
                    | SymbolKind::ConformanceParameter
                    | SymbolKind::PropositionParameter
                    | SymbolKind::PropositionMachineParameter => false,
                    SymbolKind::Const => !has_open_scope(self.program, symbol),
                    _ => true,
                };
            }
        }
    }

    impl typed_trees::references::TypedReferences for ClosedInputs<'_> {
        fn callable_signature(&mut self, signature: &typed_trees::signature::StateSignature) {
            self.pending_binders.extend(
                self.program
                    .tables
                    .state_parameters
                    .span_or_empty(signature.parameters)
                    .iter()
                    .map(|parameter| parameter.symbol),
            );
            self.pending_binders.extend(
                self.program
                    .data_type_parameters
                    .span_or_empty(signature.type_parameters)
                    .iter()
                    .map(|parameter| parameter.symbol),
            );
        }

        fn enter_lexical_scope(&mut self, _: SymbolHandle) {
            self.bound_scopes
                .push(std::mem::take(&mut self.pending_binders));
        }

        fn leave_lexical_scope(&mut self) {
            self.bound_scopes.pop();
        }
    }

    fn has_open_scope(program: &TypedTrees, mut owner: SymbolHandle) -> bool {
        while owner.is_valid() {
            if !program.declaration_type_parameters(owner).is_empty()
                || program
                    .machine_by_own_or_state_symbol(owner)
                    .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
            {
                return true;
            }
            owner = program.declaration_scope_parent(owner);
        }
        false
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::expression::ExpressionNode;
        use typed_trees::references::TypedReferences;
        use typed_trees::types::TypeReferenceNode;

        #[test]
        fn constant_entry_inputs_include_the_result_carrier() {
            let mut symbols = SymbolTableBuilder::new();
            let parameter =
                symbols.insert_root(SymbolKind::TypeParameter, SymbolNameRef::Borrowed("T"));
            let mut program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let root = expressions_of_mut(&mut program).insert(ExpressionNode::Boolean(true));
            let closed = program.type_reference_table.insert(TypeReferenceNode::Unit);
            let open = program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: parameter,
                    name: typed_trees::name::Identifier::generated("T"),
                });
            assert!(has_closed_inputs(&program, root, closed));
            assert!(!has_closed_inputs(&program, root, open));
        }

        #[test]
        fn constant_entry_signature_binders_do_not_hide_free_operands() {
            let mut symbols = SymbolTableBuilder::new();
            let parameter =
                symbols.insert_root(SymbolKind::Parameter, SymbolNameRef::Borrowed("value"));
            let program = TypedTrees {
                symbols: symbols.finish(),
                ..Default::default()
            };
            let mut inputs = ClosedInputs {
                program: &program,
                closed: true,
                pending_binders: vec![parameter],
                bound_scopes: Vec::new(),
            };
            inputs.enter_lexical_scope(SymbolHandle::invalid());
            inputs.extend([SymbolHandle::invalid(), parameter]);
            assert!(
                inputs.closed,
                "a signature's own formal is bound; zero is absent"
            );
            inputs.leave_lexical_scope();
            inputs.extend([parameter]);
            assert!(
                !inputs.closed,
                "the same formal outside its scope is a free input"
            );
        }
    }
}
pub(crate) use crate::data_and_product_operands::data_applications;
pub(crate) use crate::data_and_product_operands::product_operands;
pub(crate) use data_applications::elaborate;
pub(crate) use diagnostics::Diagnostic;
pub use product_operands::{
    PreparedProductRequirement, prepare_product_operand_application,
    prepare_product_requirement_operand, prepare_retained_product_application,
    prepare_retained_product_requirement_operand,
};
pub(crate) mod default_domain_zero {
    use typed_trees::TypedTrees;

    pub(crate) fn refine_default_domain_zero(program: &mut TypedTrees) {
        let admitted: Vec<_> = program
            .data_definitions()
            .iter()
            .enumerate()
            .filter_map(|(definition_index, definition)| {
                (definition.zero_gated
                    && validation::data_predicates_admit_zero(program, definition))
                .then_some(definition_index)
            })
            .collect();
        let definitions = program.roots.data_definitions;
        for definition_index in admitted {
            program
                .tables
                .data_definitions
                .span_mut_or_empty(definitions)[definition_index]
                .zero_gated = false;
        }
    }
}
pub(crate) mod deferred_constants {
    use crate::tables::expressions_of;
    use diagnostics::Diagnostic;
    use typed_trees::types::{FixedArrayLength, TypeReferenceNode};

    pub(crate) fn require_evaluated_constants(
        program: &dyn ::validation::CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        let references = typed_trees::references::collect_program_references(program);
        let pending_lengths = references
            .type_references
            .iter()
            .filter_map(|reference| {
                let TypeReferenceNode::FixedArray { length, .. } =
                    program.type_reference_table.type_reference(*reference)
                else {
                    return None;
                };
                let (name, source_span) = match length {
                    FixedArrayLength::ConstCall {
                        name, source_span, ..
                    } if !program
                        .constant_recipe_for_array_length(*reference)
                        .is_valid() =>
                    {
                        (name.as_str(), *source_span)
                    }
                    FixedArrayLength::ConstParameter { symbol, name }
                        if symbol.is_valid()
                            && program.symbols.get(*symbol).kind == symbols::SymbolKind::Const =>
                    {
                        let origins =
                            match program.type_reference_table.constant_origins(*reference) {
                                Ok(origins) => origins,
                                Err(reason) => return Some(Diagnostic::error(reason)),
                            };
                        let source_span = origins
                            .iter()
                            .find(|origin| origin.declaration == *symbol)
                            .map(|origin| origin.reference)
                            .or_else(|| program.symbols.symbol_source_span(*symbol))
                            .unwrap_or_default();
                        (name.as_str(), source_span)
                    }
                    _ => return None,
                };
                Some(
                    Diagnostic::error(format!(
                        "array length `{name}` has an unresolved build-time evaluation dependency"
                    ))
                    .with_source_span(source_span),
                )
            })
            .collect::<Vec<_>>();
        if !pending_lengths.is_empty() {
            return Err(pending_lengths);
        }
        let pending_endpoints = program
            .pending_const_range_endpoints
            .iter()
            .filter(|expression| references.expressions.contains(expression))
            .filter(|expression| {
                !program
                    .constant_recipe_for_contract_expression(**expression)
                    .is_valid()
            })
            .filter(|expression| !program.is_deferred_contract_invocation(**expression))
            .filter(|expression| !program.constant_recipe_root_for_const_application(**expression))
            .map(|expression| {
                Diagnostic::error(
                    "range endpoint has an unresolved build-time evaluation dependency",
                )
                .with_source_span(expressions_of(program).source_span(*expression))
            })
            .collect::<Vec<_>>();
        if pending_endpoints.is_empty() {
            Ok(())
        } else {
            Err(pending_endpoints)
        }
    }
}
pub(crate) mod opaque_property_receipts {
    use validation::{OpaqueDataPropertyReceipt, OpaqueDataPropertyReceiptKind};

    pub(crate) fn for_source_instances(
        program: &dyn ::validation::CheckingInput,
        receipts: &[OpaqueDataPropertyReceipt],
    ) -> Vec<OpaqueDataPropertyReceipt> {
        let mut instances = receipts.to_vec();
        for receipt in receipts {
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == receipt.declaration());
            let Some(definition) = definitions.next() else {
                continue;
            };
            if definitions.next().is_some() {
                continue;
            }
            let Some(declaration) = program
                .symbols
                .symbol_provenance_source_span(definition.symbol)
            else {
                continue;
            };
            let Some(file) = program.symbols.source_file(declaration) else {
                continue;
            };
            for candidate in program.data_definitions() {
                let Some(span) = program
                    .symbols
                    .symbol_provenance_source_span(candidate.symbol)
                else {
                    continue;
                };
                let Some(candidate_file) = program.symbols.source_file(span) else {
                    continue;
                };
                if span.span != declaration.span
                    || candidate_file.path != file.path
                    || candidate_file.package_identity != file.package_identity
                    || (file.package_identity.is_none()
                        && candidate_file.package_root != file.package_root)
                    || candidate_file.origin != file.origin
                    || candidate_file.resolution_stratum != file.resolution_stratum
                    || candidate_file.source != file.source
                {
                    continue;
                }
                let instance = match receipt.kind() {
                    OpaqueDataPropertyReceiptKind::Copy => {
                        OpaqueDataPropertyReceipt::copy(candidate.symbol)
                    }
                };
                if !instances.contains(&instance) {
                    instances.push(instance);
                }
            }
        }
        instances
    }
}
pub(crate) mod runtime_index_fields {
    use crate::tables::{expressions_of, expressions_of_mut};
    use diagnostics::Diagnostic;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteralField};
    use typed_trees::statement::{StatementNode, TransitionTargetNode};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use validation::expression_traversal::inert_value_tree_with_operations;
    use validation::expression_traversal::{
        collect_expression_tree, collect_statement_expression_trees,
    };
    use validation::has_builtin_binary_expression_meaning;
    use validation::land_owned_fixed_byte_literal;

    type Substitutions = Vec<(SymbolHandle, ExpressionHandle)>;

    struct CallerEffects {
        machine: SymbolHandle,
        track_effects: bool,
        no_write_calls: Vec<ExpressionHandle>,
    }

    impl CallerEffects {
        fn inert(&self, program: &TypedTrees, expression: ExpressionHandle) -> bool {
            if !self.track_effects {
                return true;
            }
            inert_value_tree_with_operations(program, expression, |operation| match expressions_of(
                program,
            )
            .expression(operation)
            {
                ExpressionNode::Call(_) => self
                    .no_write_calls
                    .binary_search_by_key(
                        &(operation.arena_index(), operation.generation()),
                        |candidate| (candidate.arena_index(), candidate.generation()),
                    )
                    .is_ok(),
                ExpressionNode::Binary(_) => {
                    program
                        .machine_by_symbol(self.machine)
                        .is_some_and(|machine| {
                            has_builtin_binary_expression_meaning(program, machine, None, operation)
                        })
                }
                _ => false,
            })
        }
    }

    fn contains_reachable_construction(program: &TypedTrees, expression: ExpressionHandle) -> bool {
        if !expression.is_valid() {
            return false;
        }
        match expressions_of(program).expression(expression) {
            ExpressionNode::StructLiteral(_) | ExpressionNode::String(_) => true,
            ExpressionNode::Borrow(borrow) => {
                contains_reachable_construction(program, borrow.target)
            }
            ExpressionNode::Atomic(atomic) => {
                contains_reachable_construction(program, atomic.value)
            }
            ExpressionNode::Cast(cast) => contains_reachable_construction(program, cast.value),
            ExpressionNode::Match(dispatch) => expressions_of(program)
                .match_arms(dispatch.arms)
                .iter()
                .any(|arm| contains_reachable_construction(program, arm.value)),
            ExpressionNode::ArrayLiteral(elements) => expressions_of(program)
                .expression_handles(*elements)
                .iter()
                .any(|element| contains_reachable_construction(program, *element)),
            _ => false,
        }
    }

    pub(crate) fn materialize(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let has_value_parameter_data = program.data_definitions().iter().any(|definition| {
            program
                .data_value_parameter_fields(definition)
                .next()
                .is_some()
        });
        let mut destinations = Vec::new();
        let mut calls = Vec::new();
        let mut caller_effects = Vec::new();
        let mut caller_effects_by_machine: symbols::SymbolKeyMap<SymbolHandle, usize> =
            symbols::SymbolKeyMap::default();
        let checking_context =
            has_value_parameter_data.then(|| validation::CheckingContext::new(program));
        let frames = checking_context
            .as_ref()
            .and_then(|context| validation::CallFrameResolver::new(context));
        let sidecars = has_value_parameter_data
            .then(|| typed_trees::references::MachineSidecarIndex::build(program));
        for machine in program.machines() {
            let mut no_write_calls = Vec::new();
            let mut statement_calls = Vec::new();
            let mut machine_has_struct_destination_or_call = false;
            for state in program.machine_states(machine) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    match statement {
                        StatementNode::LocalData(local) => {
                            if contains_reachable_construction(program, local.initial_value) {
                                machine_has_struct_destination_or_call = true;
                                destinations.push((
                                    machine.symbol,
                                    local.initial_value,
                                    local.type_reference,
                                    Vec::new(),
                                ));
                            }
                        }
                        StatementNode::Expression(value) if state.return_type.is_valid() => {
                            if contains_reachable_construction(program, *value) {
                                machine_has_struct_destination_or_call = true;
                                destinations.push((
                                    machine.symbol,
                                    *value,
                                    state.return_type,
                                    Vec::new(),
                                ));
                            }
                        }
                        StatementNode::Assignment(assignment) => {
                            if contains_reachable_construction(program, assignment.value)
                                && let Some(reference) =
                                    crate::source_type_queries::expression_result_type_reference(
                                        program,
                                        machine,
                                        state,
                                        assignment.target,
                                    )
                            {
                                machine_has_struct_destination_or_call = true;
                                destinations.push((
                                    machine.symbol,
                                    assignment.value,
                                    reference,
                                    Vec::new(),
                                ));
                            }
                        }
                        StatementNode::Call(call) => {
                            let arguments =
                                program.statement_table.expression_handles(call.arguments);
                            if arguments
                                .iter()
                                .any(|arg| contains_reachable_construction(program, *arg))
                            {
                                machine_has_struct_destination_or_call = true;
                                calls.push((
                                    machine.symbol,
                                    call.target_symbol,
                                    call.machine_arguments.to_vec(),
                                    arguments.to_vec(),
                                ));
                            }
                        }
                        StatementNode::Transition(transition) => {
                            for target in transition.targets() {
                                match program.statement_table.transition_target(target) {
                                    TransitionTargetNode::Value(value) => {
                                        if contains_reachable_construction(program, *value) {
                                            machine_has_struct_destination_or_call = true;
                                            destinations.push((
                                                machine.symbol,
                                                *value,
                                                state.return_type,
                                                Vec::new(),
                                            ));
                                        }
                                    }
                                    TransitionTargetNode::Named {
                                        path, arguments, ..
                                    } => {
                                        let arguments =
                                            program.statement_table.expression_handles(*arguments);
                                        if arguments.iter().any(|arg| {
                                            contains_reachable_construction(program, *arg)
                                        }) {
                                            machine_has_struct_destination_or_call = true;
                                            calls.push((
                                                machine.symbol,
                                                path.symbol,
                                                Vec::new(),
                                                arguments.to_vec(),
                                            ));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                    let mut expressions = Vec::new();
                    collect_statement_expression_trees(program, statement, &mut expressions);
                    for expression in expressions {
                        if let Some(call) = expressions_of(program).call_node(expression) {
                            if has_value_parameter_data {
                                statement_calls.push(expression);
                            }
                            let arguments =
                                expressions_of(program).expression_handles(call.arguments);
                            if arguments
                                .iter()
                                .any(|arg| contains_reachable_construction(program, *arg))
                            {
                                machine_has_struct_destination_or_call = true;
                                calls.push((
                                    machine.symbol,
                                    call.target_symbol,
                                    call.machine_arguments.to_vec(),
                                    arguments.to_vec(),
                                ));
                            }
                        }
                    }
                }
            }
            if has_value_parameter_data && machine_has_struct_destination_or_call {
                for expression in statement_calls {
                    if frames.as_ref().is_some_and(|frames| {
                        frames
                            .expression_write_frame(machine, expression)
                            .complete_paths()
                            .is_some_and(|paths| paths.is_empty())
                    }) {
                        no_write_calls.push(expression);
                    }
                }
                let references = typed_trees::references::collect_machine_references(
                    program,
                    machine,
                    sidecars.as_ref().expect("built sidecars"),
                );
                for expression in references.expressions {
                    if expressions_of(program).call_node(expression).is_some()
                        && frames.as_ref().is_some_and(|frames| {
                            frames.declaration_expression_has_empty_write_frame(
                                expression,
                                &|operation| {
                                    has_builtin_binary_expression_meaning(
                                        program, machine, None, operation,
                                    )
                                },
                            )
                        })
                    {
                        no_write_calls.push(expression);
                    }
                }
                no_write_calls.sort_by_key(|call| (call.arena_index(), call.generation()));
                no_write_calls.dedup();
            } else {
                no_write_calls.clear();
            }
            let effect_index = caller_effects.len();
            caller_effects.push(CallerEffects {
                machine: machine.symbol,
                track_effects: has_value_parameter_data,
                no_write_calls,
            });
            caller_effects_by_machine.insert(machine.symbol, effect_index);
        }
        drop(frames);
        drop(checking_context);
        for (caller, target, static_arguments, arguments) in calls {
            let effects = &caller_effects[*caller_effects_by_machine
                .get(&caller)
                .expect("collected caller effects")];
            let binders = program.declaration_type_parameters(target).to_vec();
            if binders.len() != static_arguments.len() {
                continue;
            }
            let mut type_arguments = Vec::with_capacity(binders.len());
            for (binder, argument) in binders.iter().zip(&static_arguments) {
                let actual = program
                    .static_operand_type_reference(argument)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                type_arguments.push((binder.symbol, actual));
            }
            let Some(parameters) = validation::call_target_parameters(program, target) else {
                continue;
            };
            let parameters = parameters.to_vec();
            let substitutions: Substitutions = if has_value_parameter_data {
                let can_reobserve = arguments
                    .iter()
                    .all(|argument| effects.inert(program, *argument));
                parameters
                    .iter()
                    .zip(&arguments)
                    .map(|(parameter, argument)| {
                        let stable_literal = matches!(
                            expressions_of(program).expression(*argument),
                            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                        );
                        let retained = if can_reobserve || stable_literal {
                            *argument
                        } else {
                            ExpressionHandle::invalid()
                        };
                        (parameter.symbol, retained)
                    })
                    .collect()
            } else {
                Vec::new()
            };
            for (parameter, argument) in parameters.iter().zip(arguments) {
                if !contains_reachable_construction(program, argument) {
                    continue;
                }
                let reference = if type_arguments.is_empty() {
                    parameter.type_reference
                } else {
                    program
                        .apply_callable_type(parameter.type_reference, &type_arguments, &[], target)
                        .map_err(|message| vec![Diagnostic::error(message)])?
                };
                destinations.push((caller, argument, reference, substitutions.clone()));
            }
        }
        let mut diagnostics = Vec::new();
        for (caller, expression, reference, substitutions) in destinations {
            let effects = &caller_effects[*caller_effects_by_machine
                .get(&caller)
                .expect("collected caller effects")];
            fill(
                program,
                expression,
                reference,
                &substitutions,
                effects,
                false,
                &mut diagnostics,
            );
        }
        let const_declarations: Vec<_> = program
            .const_declarations()
            .iter()
            .flat_map(|declaration| {
                [
                    (
                        declaration.materialized_initializer,
                        declaration.declared_type,
                    ),
                    (declaration.authored_initializer, declaration.declared_type),
                ]
            })
            .filter(|(value, _)| contains_reachable_construction(program, *value))
            .collect();
        let const_effects = CallerEffects {
            machine: SymbolHandle::invalid(),
            track_effects: has_value_parameter_data,
            no_write_calls: Vec::new(),
        };
        for (value, declared_type) in const_declarations {
            if value.is_valid() {
                fill(
                    program,
                    value,
                    declared_type,
                    &Vec::new(),
                    &const_effects,
                    false,
                    &mut diagnostics,
                );
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn fill(
        program: &mut TypedTrees,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        substitutions: &Substitutions,
        effects: &CallerEffects,
        earlier_effects: bool,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !expression.is_valid() || !reference.is_valid() {
            return;
        }
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        macro_rules! fill_child {
            ($expression:expr, $reference:expr, $earlier_effects:expr $(,)?) => {
                fill(
                    program,
                    $expression,
                    $reference,
                    substitutions,
                    effects,
                    $earlier_effects,
                    diagnostics,
                )
            };
        }
        match expressions_of(program).expression(expression).clone() {
            ExpressionNode::String(_) => {
                if let Err(error) = land_owned_fixed_byte_literal(program, expression, reference) {
                    diagnostics.push(error);
                }
            }
            ExpressionNode::Borrow(borrow) => {
                if let TypeReferenceNode::Reference { referee, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    let referee = *referee;
                    fill_child!(borrow.target, referee, earlier_effects);
                }
            }
            ExpressionNode::Atomic(atomic) => fill_child!(atomic.value, reference, true),
            ExpressionNode::Match(dispatch) => {
                let arms = expressions_of(program).match_arms(dispatch.arms).to_vec();
                let mut prior = earlier_effects || !effects.inert(program, dispatch.subject);
                for arm in arms {
                    for pattern in expressions_of(program).match_pattern_operands(arm.pattern) {
                        prior |= !effects.inert(program, pattern);
                    }
                    fill_child!(arm.value, reference, prior);
                }
            }
            ExpressionNode::Cast(cast) => {
                fill_child!(cast.value, cast.target_type, earlier_effects)
            }
            ExpressionNode::ArrayLiteral(elements) => {
                if let TypeReferenceNode::FixedArray { element_type, .. } =
                    program.type_reference_table.type_reference(reference)
                {
                    let element_type = *element_type;
                    let elements = expressions_of(program)
                        .expression_handles(elements)
                        .to_vec();
                    let mut prior = earlier_effects;
                    for element in elements {
                        fill_child!(element, element_type, prior);
                        prior |= !effects.inert(program, element);
                    }
                }
            }
            ExpressionNode::StructLiteral(mut literal) => {
                if program
                    .data_constructor_definition(reference, literal.type_symbol)
                    .is_some()
                {
                    let prior =
                        expressions_of(program).construction_required_type_reference(expression);
                    if prior.is_valid() {
                        if program.normalized_type_identity(
                            program
                                .type_reference_table
                                .unconstrained_type_reference(prior),
                        ) != program.normalized_type_identity(
                            program
                                .type_reference_table
                                .unconstrained_type_reference(reference),
                        ) {
                            diagnostics.push(Diagnostic::error(format!("constructor occurrence has conflicting receiving applications; preserve separate evaluation contexts: {} vs {}", program.normalized_type_identity_text(prior), program.normalized_type_identity_text(reference))));
                            return;
                        }
                    } else {
                        expressions_of_mut(program)
                            .set_construction_required_type_reference(expression, reference);
                    }
                }
                let mut fields = expressions_of(program)
                    .struct_fields(literal.fields)
                    .to_vec();
                let descriptors: Vec<_> = program
                    .data_application_value_fields(reference)
                    .into_iter()
                    .map(|(_, field, argument)| (field.clone(), argument))
                    .collect();
                let mut implicit_fields = Vec::new();
                for (field, argument) in descriptors {
                    if let Some(ordinal) = fields
                        .iter()
                        .position(|authored| authored.field_symbol == field.symbol)
                    {
                        let unavailable_argument = match program
                            .type_reference_table
                            .type_reference(argument)
                        {
                            TypeReferenceNode::Named { symbol, .. } => substitutions
                                .iter()
                                .any(|(parameter, value)| parameter == symbol && !value.is_valid()),
                            _ => false,
                        };
                        let value = fields[ordinal].value;
                        let stable_literal = matches!(
                            expressions_of(program).expression(value),
                            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
                        );
                        let stable_position = stable_literal
                            || !earlier_effects
                                && fields[..=ordinal]
                                    .iter()
                                    .all(|field| effects.inert(program, field.value));
                        if unavailable_argument || !stable_position {
                            diagnostics.push(Diagnostic::error(format!("cannot validate runtime index descriptor `{}` across argument or field effects without an evaluated scalar snapshot", field.name.as_str())));
                        }
                        continue;
                    }
                    match index_expression(program, argument, field.type_reference, substitutions, effects, earlier_effects) {
                            Some(value) => implicit_fields.push(TableStructLiteralField { name: field.name, field_symbol: field.symbol, value }),
                            None => diagnostics.push(Diagnostic::error(format!("cannot retain runtime index descriptor `{}` from its application argument without an evaluated scalar subject", field.name.as_str()))),
                        }
                }
                implicit_fields.extend(fields);
                fields = implicit_fields;
                let mut prior = earlier_effects;
                let children: Vec<_> = fields
                    .iter()
                    .filter_map(|field| {
                        let field_prior = prior;
                        prior |= !effects.inert(program, field.value);
                        if !contains_reachable_construction(program, field.value) {
                            return None;
                        }
                        let selected = program.data_constructor_field(
                            reference,
                            literal.type_symbol,
                            field.field_symbol,
                        )?;
                        Some((field.value, selected.type_reference, field_prior))
                    })
                    .collect();
                literal.fields = expressions_of_mut(program).insert_struct_fields(fields);
                *expressions_of_mut(program).expression_mut(expression) =
                    ExpressionNode::StructLiteral(literal);
                for (value, reference, prior) in children {
                    fill_child!(value, reference, prior);
                }
            }
            _ => {}
        }
    }

    fn index_expression(
        program: &mut TypedTrees,
        argument: TypeReferenceHandle,
        carrier: TypeReferenceHandle,
        substitutions: &Substitutions,
        effects: &CallerEffects,
        earlier_effects: bool,
    ) -> Option<ExpressionHandle> {
        if let TypeReferenceNode::ConstExpression(expression) =
            *program.type_reference_table.type_reference(argument)
        {
            return capture_recipe(program, expression, substitutions, effects, earlier_effects);
        }
        let TypeReferenceNode::Named { symbol, name } = program
            .type_reference_table
            .type_reference(argument)
            .clone()
        else {
            return None;
        };
        if program.const_declaration_by_symbol(symbol).is_some() {
            let value =
                crate::monomorphization::materialize_static_value(program, argument, carrier)?;
            return Some(expressions_of_mut(program).insert(value));
        }
        if symbol.is_valid() {
            if let Some((_, value)) = substitutions
                .iter()
                .find(|(parameter, _)| *parameter == symbol)
            {
                if !value.is_valid() {
                    return None;
                }
                return capture_recipe(program, *value, &Vec::new(), effects, earlier_effects);
            }
            if earlier_effects {
                return None;
            }
            return Some(crate::monomorphization::insert_subject_name(
                expressions_of_mut(program),
                name,
                symbol,
            ));
        }
        let value = crate::monomorphization::materialize_static_value(program, argument, carrier)?;
        Some(expressions_of_mut(program).insert(value))
    }

    fn capture_recipe(
        program: &mut TypedTrees,
        expression: ExpressionHandle,
        substitutions: &Substitutions,
        effects: &CallerEffects,
        earlier_effects: bool,
    ) -> Option<ExpressionHandle> {
        let stable_literal = matches!(
            expressions_of(program).expression(expression),
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
        );
        if !stable_literal && (earlier_effects || !effects.inert(program, expression)) {
            return None;
        }
        let mut tree = Vec::new();
        collect_expression_tree(program, expression, &mut tree);
        for member in &tree {
            if let ExpressionNode::Name(path) = expressions_of(program).expression(*member)
                && let Some((_, actual)) = substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
                && (!actual.is_valid() || !effects.inert(program, *actual))
            {
                return None;
            }
        }
        let copied = expressions_of_mut(program).insert_copy(expression);
        tree.clear();
        collect_expression_tree(program, copied, &mut tree);
        for member in tree {
            let replacement = match expressions_of(program).expression(member) {
                ExpressionNode::Name(path) => substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
                    .map(|(_, actual)| *actual),
                _ => None,
            };
            if let Some(actual) = replacement {
                let actual = expressions_of_mut(program).insert_copy(actual);
                let value = expressions_of(program).expression(actual).clone();
                *expressions_of_mut(program).expression_mut(member) = value;
            }
        }
        Some(copied)
    }
}
pub(crate) mod runtime_value_applications {
    use crate::tables::{expressions_of, expressions_of_mut};
    use arena::HandleSpan;
    use symbols::{SymbolHandle, SymbolKind};
    use typed_trees::TypedTrees;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::statement::{StatementNode, TableLocalData};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    type Operand = (
        HandleSpan<TypeReferenceHandle>,
        u32,
        ExpressionHandle,
        TypeReferenceHandle,
        SymbolHandle,
    );

    fn operands(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        seen: &mut Vec<TypeReferenceHandle>,
        result: &mut Vec<Operand>,
    ) {
        if !reference.is_valid() || seen.contains(&reference) {
            return;
        }
        seen.push(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } => {
                let Some(data) = program.unique_data_definition_by_symbol(*base_symbol) else {
                    return;
                };
                let actuals = program
                    .type_reference_table
                    .type_reference_handles(*arguments);
                let parameters = program.data_type_parameters(data);
                if actuals.len() != parameters.len() {
                    return;
                }
                for (position, (parameter, actual)) in parameters.iter().zip(actuals).enumerate() {
                    match parameter.kind {
                        TypeParameterKind::Value { type_reference } => {
                            if program.primitive_type_reference(type_reference).is_none() {
                                continue;
                            }
                            if let TypeReferenceNode::ConstExpression(expression) =
                                program.type_reference_table.type_reference(*actual)
                                && !matches!(
                                    expressions_of(program).expression(*expression),
                                    ExpressionNode::Integer(_)
                                        | ExpressionNode::Boolean(_)
                                        | ExpressionNode::Float(_)
                                )
                            {
                                result.push((
                                    *arguments,
                                    position as u32,
                                    *expression,
                                    type_reference,
                                    *base_symbol,
                                ));
                            }
                        }
                        TypeParameterKind::Type => operands(program, *actual, seen, result),
                        _ => {}
                    }
                }
            }
            TypeReferenceNode::Reference { referee, .. } => {
                operands(program, *referee, seen, result)
            }
            TypeReferenceNode::Constrained { base_type, .. } => {
                operands(program, *base_type, seen, result)
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => {
                operands(program, *element_type, seen, result)
            }
            _ => {}
        }
    }

    pub(crate) fn materialize(
        program: &mut TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let mut pending = Vec::new();
        for machine in program.machines() {
            for (state_index, state) in program.machine_states(machine).iter().enumerate() {
                let mut positions = Vec::new();
                for (position, statement) in program
                    .statement_table
                    .indexed_statements(state.statement_nodes)
                {
                    let Some(local) = statement.as_local_data() else {
                        continue;
                    };
                    if local.type_is_inferred
                        || local.is_operand_capture
                        || local.relevance.is_erased()
                        || local.role != language_core::LocalBindingRole::Value
                        || !local.initial_value.is_valid()
                    {
                        continue;
                    }
                    let mut found = Vec::new();
                    operands(program, local.type_reference, &mut Vec::new(), &mut found);
                    if !found.is_empty() {
                        positions.push(position);
                    }
                }
                if !positions.is_empty() {
                    pending.push((machine.clone(), state_index, state.clone(), positions));
                }
            }
        }
        let mut changed_owners = Vec::new();
        for (machine, state_index, state, positions) in pending {
            let statements = program
                .statement_table
                .statements(state.statement_nodes)
                .to_vec();
            let mut body = HandleSpan::empty();
            for (position, mut statement) in statements.into_iter().enumerate() {
                if positions.contains(&position) {
                    let StatementNode::LocalData(local) = &mut statement else {
                        unreachable!();
                    };
                    let applied = program
                        .apply_callable_type(local.type_reference, &[], &[], machine.symbol)
                        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
                    let mut found = Vec::new();
                    operands(program, applied, &mut Vec::new(), &mut found);
                    for (ordinal, (arguments, offset, expression, carrier, declaration)) in
                        found.into_iter().enumerate()
                    {
                        let parameters = program
                            .machine_by_symbol(declaration)
                            .map(|machine| program.machine_type_parameters(machine).to_vec())
                            .or_else(|| {
                                program
                                    .unique_data_definition_by_symbol(declaration)
                                    .map(|data| program.data_type_parameters(data).to_vec())
                            })
                            .expect("captured data application");
                        let substitutions = parameters
                            .iter()
                            .zip(
                                program
                                    .type_reference_table
                                    .type_reference_handles(arguments),
                            )
                            .map(|(parameter, actual)| (parameter.symbol, *actual))
                            .collect::<Vec<_>>();
                        let carrier = program
                            .apply_callable_type(carrier, &substitutions, &[], declaration)
                            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
                        let name =
                            format!("__value_index#{}#{ordinal}", local.symbol.arena_index());
                        let children = program.symbols.append_generated_children_from(
                            state.symbol,
                            [(local.symbol, SymbolKind::Local, name.as_str())],
                        );
                        let symbol: SymbolHandle =
                            symbols::SymbolTableBuilder::child_handles(children)
                                .next()
                                .expect("one Value index capture");
                        let name = typed_trees::name::Identifier::generated(name);
                        program.statement_table.push_statement(
                            &mut body,
                            StatementNode::LocalData(TableLocalData {
                                symbol,
                                name: name.clone(),
                                type_reference: carrier,
                                initial_value: expression,
                                ..Default::default()
                            }),
                        );
                        let reference = program
                            .type_reference_table
                            .insert(TypeReferenceNode::Named { symbol, name });
                        program
                            .type_reference_table
                            .set_type_reference_handle_at_offset(arguments, offset, reference);
                    }
                    local.type_reference = applied;
                }
                program.statement_table.push_statement(&mut body, statement);
                let source = state.statement_nodes.handle_at(position as u32);
                let target = body.handle_at(body.count() - 1);
                expressions_of_mut(program).relocate_statement_operand_captures(source, target);
            }
            program.machine_states_mut(&machine)[state_index].statement_nodes = body;
            changed_owners.push(machine.symbol);
        }
        if !changed_owners.is_empty() {
            let roots = typed_trees::references::DeclarationSelection::from_symbols(changed_owners);
            crate::monomorphization::finalize_domain_instance_identities(program, &roots)
                .map_err(|diagnostic| vec![diagnostic])?;
        }
        Ok(())
    }
}
pub(crate) mod selections {
    use crate::tables::expressions_of;
    use diagnostics::Diagnostic;
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionLateBinding, AuthoredDeclarationSelectionTarget,
    };
    use typed_trees::TypedTrees;
    use typed_trees::expression::ExpressionNode;
    use validation::selection_resolution::{
        AuthoredSelectionResolution, AuthoredSelectionResolutionTarget, authored_data_member,
        collect_checked_transition_target_selections, declaration_target,
        exact_named_operator_call, finalization_diagnostic, push_consistent_resolution,
    };

    pub fn bind_pre_specialization_authored_selections(
        program: &mut TypedTrees,
    ) -> Result<(), Diagnostic> {
        let contract_facts = arena::Arena::default();
        let mut resolutions = Vec::new();
        for (expression, target) in
            validation::collect_pre_specialization_operator_selections(program)?
        {
            for occurrence in expressions_of(program).authored_selection_occurrences(expression) {
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    continue;
                };
                let binding = AuthoredDeclarationSelectionLateBinding::CheckedOperator;
                if selection.target() == AuthoredDeclarationSelectionTarget::LateBound(binding) {
                    push_consistent_resolution(
                        program,
                        &mut resolutions,
                        AuthoredSelectionResolution {
                            occurrence,
                            binding,
                            target,
                        },
                    )?;
                }
            }
        }
        let owner_index =
            validation::expression_owners::OwnerEnvironmentIndex::for_preparation(program);
        validation::validate_domain_membership_carriers(program, &contract_facts, &owner_index)?;
        for (expression, node) in expressions_of(program).iter_expressions() {
            for occurrence in expressions_of(program).authored_selection_occurrences(expression) {
                let Some(selection) = program.authored_declaration_selections().get(occurrence)
                else {
                    return Err(Diagnostic::error(format!(
                        "expression retains unknown authored declaration selection occurrence {}",
                        occurrence.ordinal(),
                    )));
                };
                let AuthoredDeclarationSelectionTarget::LateBound(binding) = selection.target()
                else {
                    continue;
                };
                let target = match (binding, node) {
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedMember,
                    ExpressionNode::Member(member),
                ) => validation::expression_owners::checked_member_target_from_exact_owner(
                    program,
                    &contract_facts,
                    expression,
                    member,
                    Some(&owner_index),
                )
                .map(|target| match target {
                    validation::expression_owners::OwnerMemberTarget::Declaration(symbol) => {
                        AuthoredSelectionResolutionTarget::Declaration(authored_data_member(
                            program, symbol,
                        ))
                    }
                    validation::expression_owners::OwnerMemberTarget::CollectionMeasure(measure) => {
                        AuthoredSelectionResolutionTarget::Intrinsic(measure.intrinsic())
                    }
                }),
                (
                    AuthoredDeclarationSelectionLateBinding::CheckedCall,
                    ExpressionNode::Call(call),
                ) => validation::compiler_operations::classify_expression_call_intrinsic(
                    program, call,
                )
                .map(AuthoredSelectionResolutionTarget::Intrinsic)
                .or_else(|| {
                    exact_named_operator_call(program, call)
                        .and_then(|operator| declaration_target(operator.symbol))
                        .or_else(|| {
                            validation::expression_owners::checked_machine_call_target_from_exact_owner(
                                program,
                                &contract_facts,
                                expression,
                                call,
                                Some(&owner_index),
                            )
                            .and_then(declaration_target)
                        })
                }),
                _ => None,
            };
                let Some(target) = target else {
                    continue;
                };
                push_consistent_resolution(
                    program,
                    &mut resolutions,
                    AuthoredSelectionResolution {
                        occurrence,
                        binding,
                        target,
                    },
                )?;
            }
        }
        collect_checked_transition_target_selections(program, &mut resolutions)?;

        let mut selections = program.authored_declaration_selections().clone();
        for resolution in resolutions {
            match resolution.target {
                AuthoredSelectionResolutionTarget::Declaration(symbol) => selections
                    .finalize_late_bound(resolution.occurrence, resolution.binding, symbol),
                AuthoredSelectionResolutionTarget::Intrinsic(intrinsic) => selections
                    .finalize_intrinsic(resolution.occurrence, resolution.binding, intrinsic),
            }
            .map_err(|error| finalization_diagnostic(resolution, error))?;
        }
        program.retain_authored_declaration_selections(selections);
        Ok(())
    }
}
pub(crate) mod value_location_alignment {
    use crate::tables::{expressions_of, expressions_of_mut};
    use arena::HandleSpan;
    use diagnostics::Diagnostic;
    use language_core::data_layout_queries::DataLayoutQueryKind;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
    use validation::expression_traversal::collect_statement_expression_trees;

    pub(crate) fn settle(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let mut settled = Vec::new();
        let mut diagnostics = Vec::new();
        let mut rejected = Vec::new();
        for machine in program.machines() {
            for state in program.machine_states(machine) {
                for (statement_index, statement) in program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .enumerate()
                {
                    let mut handles = Vec::new();
                    collect_statement_expression_trees(program, statement, &mut handles);
                    for query in handles {
                        let Some(place) = value_query_operand(program, query) else {
                            continue;
                        };
                        if settled.iter().any(|(handle, _, _)| *handle == query) {
                            continue;
                        }
                        match observed_storage_type(program, state.symbol, statement_index, place) {
                            Ok(subject) => settled.push((query, place, subject)),
                            Err(message) => {
                                rejected.push(query);
                                diagnostics.push(
                                    Diagnostic::error(message).with_source_span(
                                        expressions_of(program).source_span(query),
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
        for (query, place, subject) in settled {
            let nonempty_view = match subject {
                ObservedStorage::Place(_) => ExpressionHandle::invalid(),
                ObservedStorage::FirstElement(_) => {
                    let span = expressions_of(program).source_span(query);
                    let table = expressions_of_mut(program);
                    let zero = table.insert(ExpressionNode::Integer(
                        numerics::literals::IntegerLiteral::from_value(0),
                    ));
                    table.set_source_span(zero, span);
                    let first = table.insert(ExpressionNode::Indexed(
                        typed_trees::expression::TableIndexedExpression {
                            collection: place,
                            index: zero,
                        },
                    ));
                    table.set_source_span(first, span);
                    first
                }
            };
            if let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(query)
                && let Some(request) = call.layout_query.as_mut()
            {
                request.operand.type_reference = subject.storage_type();
                request.nonempty_view = nonempty_view;
                call.arguments = HandleSpan::empty();
            }
        }
        let roots: Vec<_> = expressions_of(program)
            .iter_expressions()
            .map(|(root, _)| root)
            .collect();
        // One walk with one seen set: every expression is itself a root, so
        // walking each root's tree separately revisited each subtree once per
        // enclosing expression.
        let mut handles = Vec::new();
        validation::expression_traversal::collect_expression_trees(program, roots, &mut handles);
        let mut unsettled: Vec<_> = handles
            .into_iter()
            .filter(|query| {
                !rejected.contains(query) && value_query_operand(program, *query).is_some()
            })
            .collect();
        unsettled.sort_by_key(|query| (query.arena_index(), query.generation()));
        unsettled.dedup();
        let mut ensures_handles = Vec::new();
        let mut collect_ensures = |contract: &typed_trees::signature::SignatureContract| {
            if contract.kind == typed_trees::signature::SignatureContractKind::Ensures {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    if let Some(expression) = fact.as_expression() {
                        validation::expression_traversal::collect_expression_tree(
                            program,
                            *expression,
                            &mut ensures_handles,
                        );
                    }
                }
            }
        };
        for machine in program.machines() {
            program
                .signature_contracts
                .span_or_empty(machine.contracts)
                .iter()
                .for_each(&mut collect_ensures);
            for state in program.machine_states(machine) {
                program
                    .state_contracts(state)
                    .iter()
                    .for_each(&mut collect_ensures);
            }
        }
        for signature in program.prepared_callable_signatures() {
            program
                .state_signature_contracts(signature)
                .iter()
                .for_each(&mut collect_ensures);
        }
        let mut contract_settled = Vec::new();
        for query in unsettled {
            if ensures_handles.contains(&query) {
                diagnostics.push(
                Diagnostic::error(
                    "`align_of(value)` in an `ensures` contract is not yet supported; state the alignment in `requires` or check it in the body",
                )
                .with_source_span(expressions_of(program).source_span(query)),
            );
                continue;
            }
            let Some(place) = value_query_operand(program, query) else {
                continue;
            };
            match contract_storage_type(program, place) {
                Ok(subject) => contract_settled.push((query, subject)),
                Err(message) => diagnostics.push(
                    Diagnostic::error(message)
                        .with_source_span(expressions_of(program).source_span(query)),
                ),
            }
        }
        for (query, subject) in contract_settled {
            if let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(query)
                && let Some(request) = call.layout_query.as_mut()
            {
                request.operand.type_reference = subject;
                call.arguments = HandleSpan::empty();
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn value_query_operand(
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let ExpressionNode::Call(call) = expressions_of(program).expression(expression) else {
            return None;
        };
        let request = call.layout_query.as_ref()?;
        if request.kind != DataLayoutQueryKind::Alignment
            || request.operand.type_reference.is_valid()
        {
            return None;
        }
        match expressions_of(program).expression_handles(call.arguments) {
            [operand] => Some(*operand),
            _ => None,
        }
    }

    fn contract_storage_type(
        program: &TypedTrees,
        place: ExpressionHandle,
    ) -> Result<TypeReferenceHandle, &'static str> {
        const NO_PLACE: &str = "`align_of(value)` observes a storage place; outside a machine body only a parameter of the contracted machine names one";
        let canonical =
            validation::canonical_place_from_expression(program, place).ok_or(NO_PLACE)?;
        let root = canonical.root.as_symbol().ok_or(NO_PLACE)?;
        let declared = program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine).iter())
            .flat_map(|state| program.state_parameters(state).iter())
            .find(|parameter| parameter.symbol == root)
            .map(|parameter| parameter.type_reference);
        let Some(declared) = declared else {
            let trait_parameter = program
                .traits()
                .iter()
                .flat_map(|definition| program.trait_machine_signatures(definition).iter())
                .flat_map(|signature| program.state_signature_parameters(signature).iter())
                .any(|parameter| parameter.symbol == root);
            return Err(if trait_parameter {
                "`align_of(value)` in a trait requirement's contract is not yet supported; its implementations cannot carry the clause"
            } else {
                NO_PLACE
            });
        };
        let mut subject = referent(program, declared);
        for segment in &canonical.segments {
            if !matches!(segment, facts::PlaceSegment::Field { .. }) {
                return Err(
                    "`align_of(value)` in a contract observes a parameter, its referent or a field path; an element or other projection is not yet supported there",
                );
            }
            if program.layout_storage_arguments(subject).is_some() {
                return Err(
                    "`align_of(value)` on a field of an explicit layout representation needs that policy's placement, which is not yet observed",
                );
            }
            subject = validation::project_type_reference_from_segments(
                program,
                subject,
                std::slice::from_ref(segment),
            )
            .map(|field| referent(program, field))
            .ok_or("`align_of(value)` requires an operand whose storage type is known")?;
        }
        if matches!(
            program.type_reference_table.type_reference(subject),
            TypeReferenceNode::Slice { .. }
        ) {
            return Err(
                "`align_of(view)` on a slice in a contract needs a nonempty-view proof the contract cannot carry; query the slice in the body after its guard",
            );
        }
        Ok(subject)
    }

    fn observed_storage_type(
        program: &TypedTrees,
        state: symbols::SymbolHandle,
        statement_index: usize,
        place: ExpressionHandle,
    ) -> Result<ObservedStorage, &'static str> {
        let canonical =
        validation::canonical_place_from_expression_in_state(program, state, statement_index, place)
            .filter(|canonical| {
                canonical.root.as_symbol().is_some_and(|root| {
                    matches!(
                        program.symbols.get(root).kind,
                        symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
                    )
                })
            })
            .ok_or(
                "`align_of(value)` observes an addressable storage place; this operand is a value with no storage location",
            )?;
        if canonical
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
        {
            return Err(
                "`align_of(value)` on a case payload field needs a proof that the case is active, which the query does not yet carry; query a field outside the case",
            );
        }
        for prefix in 0..canonical.segments.len() {
            let enclosing = validation::CanonicalPlace {
                root: canonical.root,
                segments: canonical.segments[..prefix].to_vec(),
            };
            let through_representation = validation::canonical_place_type_reference(
                program,
                state,
                statement_index,
                &enclosing,
            )
            .map(|enclosing| referent(program, enclosing))
            .is_some_and(|enclosing| program.layout_storage_arguments(enclosing).is_some());
            if through_representation {
                return Err(
                    "`align_of(value)` on a field of an explicit layout representation needs that policy's placement, which is not yet observed",
                );
            }
        }
        let declared =
            validation::expression_type_reference_in_state(program, state, statement_index, place)
                .ok_or("`align_of(value)` requires an operand whose storage type is known")?;
        let subject = referent(program, declared);
        if let TypeReferenceNode::Slice { element_type } =
            program.type_reference_table.type_reference(subject)
        {
            return Ok(ObservedStorage::FirstElement(unconstrained(
                program,
                *element_type,
            )));
        }
        Ok(ObservedStorage::Place(subject))
    }

    #[derive(Clone, Copy)]
    enum ObservedStorage {
        Place(TypeReferenceHandle),
        FirstElement(TypeReferenceHandle),
    }

    impl ObservedStorage {
        fn storage_type(self) -> TypeReferenceHandle {
            match self {
                Self::Place(storage) | Self::FirstElement(storage) => storage,
            }
        }
    }

    fn referent(program: &TypedTrees, reference: TypeReferenceHandle) -> TypeReferenceHandle {
        let reference = unconstrained(program, reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => unconstrained(program, *referee),
            _ => reference,
        }
    }

    fn unconstrained(
        program: &TypedTrees,
        mut reference: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        reference
    }
}

pub use selections::bind_pre_specialization_authored_selections;

pub(crate) use data_applications::elaborate_projected_receivers;

pub(crate) use representation_selections::OpaqueRepresentationSelection;
pub(crate) use typed_trees::TypedTrees;
pub(crate) use typed_trees::references::DeclarationSelection;
pub(crate) use validation::AsmAuthorityAdmission;

#[derive(Clone, Copy)]
pub struct PreparationRequest<'request> {
    pub(crate) admits_pending_inputs: bool,
    pub(crate) deferred_layout_applications: &'request [typed_trees::types::TypeReferenceHandle],
    pub(crate) asm_authority: Option<AsmAuthorityAdmission>,
    pub(crate) opaque_property_receipts: &'request [validation::OpaqueDataPropertyReceipt],
    pub(crate) opaque_representation_selections: &'request [OpaqueRepresentationSelection],
    pub(crate) wire_compatibility_demands:
        &'request [language_semantics::wire::WireCompatibilityDemand],
}

impl<'request> PreparationRequest<'request> {
    pub const fn settled() -> Self {
        Self {
            admits_pending_inputs: false,
            deferred_layout_applications: &[],
            asm_authority: None,
            opaque_property_receipts: &[],
            opaque_representation_selections: &[],
            wire_compatibility_demands: &[],
        }
    }

    pub const fn preliminary() -> Self {
        Self {
            admits_pending_inputs: true,
            ..Self::settled()
        }
    }

    pub const fn with_asm_authority(mut self, authority: AsmAuthorityAdmission) -> Self {
        self.asm_authority = Some(authority);
        self
    }

    pub const fn with_opaque_property_receipts(
        mut self,
        receipts: &'request [validation::OpaqueDataPropertyReceipt],
    ) -> Self {
        self.opaque_property_receipts = receipts;
        self
    }

    pub const fn with_opaque_representation_selections(
        mut self,
        selections: &'request [OpaqueRepresentationSelection],
    ) -> Self {
        self.opaque_representation_selections = selections;
        self
    }

    pub const fn with_wire_compatibility_demands(
        mut self,
        demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    ) -> Self {
        self.wire_compatibility_demands = demands;
        self
    }
}

#[derive(Debug, Clone, PartialEq, compiler_storage::Record)]
pub struct PreparationAdmission {
    pub(crate) admits_pending_inputs: bool,
    pub(crate) deferred_layout_applications: Vec<typed_trees::types::TypeReferenceHandle>,
    pub(crate) asm_authority: Option<AsmAuthorityAdmission>,
    pub(crate) opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    pub(crate) opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    pub(crate) wire_compatibility_demands: Vec<language_semantics::wire::WireCompatibilityDemand>,
}

impl PreparationRequest<'_> {
    pub fn admission(&self) -> PreparationAdmission {
        PreparationAdmission {
            admits_pending_inputs: self.admits_pending_inputs,
            deferred_layout_applications: self.deferred_layout_applications.to_vec(),
            asm_authority: self.asm_authority,
            opaque_property_receipts: self.opaque_property_receipts.to_vec(),
            opaque_representation_selections: self.opaque_representation_selections.to_vec(),
            wire_compatibility_demands: self.wire_compatibility_demands.to_vec(),
        }
    }
}

impl PreparationAdmission {
    pub fn deferred_layout_applications(&self) -> &[typed_trees::types::TypeReferenceHandle] {
        &self.deferred_layout_applications
    }
}

pub struct TypedPreparation {
    pub(crate) admits_pending_inputs: bool,
    pub(crate) deferred_layout_applications: Vec<typed_trees::types::TypeReferenceHandle>,
    pub(crate) mathematics: proof::mathematical_declarations::CheckedMathematics,
    pub(crate) opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    pub(crate) opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
}

impl TypedPreparation {
    pub fn into_checking_judgments(
        self,
    ) -> (
        bool,
        Vec<typed_trees::types::TypeReferenceHandle>,
        proof::mathematical_declarations::CheckedMathematics,
        Vec<validation::OpaqueDataPropertyReceipt>,
        Vec<OpaqueRepresentationSelection>,
    ) {
        (
            self.admits_pending_inputs,
            self.deferred_layout_applications,
            self.mathematics,
            self.opaque_property_receipts,
            self.opaque_representation_selections,
        )
    }
}

pub fn prepare_typed_trees(
    mut program: TypedTrees,
    request: &PreparationRequest<'_>,
) -> Result<(typed_trees::FrozenTypedTrees, TypedPreparation), Vec<Diagnostic>> {
    let type_frontier = program.type_reference_table.type_reference_count();
    crate::policy_operators::elaborate(&mut program)?;
    crate::assembly_control::validate_primitive_meanings(&program)?;
    for machine in program.machines() {
        if machine.body_status == typed_trees::machine::MachineBodyStatus::Deferred {
            return Err(vec![Diagnostic::error(format!(
                "machine `{}` requires implementation completion before checking",
                machine.name.as_str(),
            ))]);
        }
    }
    validation::validate_wire_protocol(&program, request.wire_compatibility_demands)?;
    value_location_alignment::settle(&mut program)?;
    if !request.admits_pending_inputs {
        deferred_constants::require_evaluated_constants(&program)?;
    }
    if let Some(authority) = request.asm_authority {
        validation::validate_asm_discharge(&program, authority)?;
    }
    let opaque_property_receipts =
        opaque_property_receipts::for_source_instances(&program, request.opaque_property_receipts);
    let opaque_representation_selections = request.opaque_representation_selections.to_vec();
    crate::rebound_parameter_homes::materialize(&mut program);
    validation::resolve_dynamic_call_targets(&mut program)?;
    program
        .retain_static_argument_contexts()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    normalize_open_index_identities(&mut program)?;
    selections::bind_pre_specialization_authored_selections(&mut program)
        .map_err(|diagnostic| vec![diagnostic])?;
    crate::monomorphization::generate_dynamic_family_specializations(&mut program)?;
    runtime_value_applications::materialize(&mut program)?;
    crate::operators::bind_selected_operator_calls(&mut program, &DeclarationSelection::all())?;
    let provider_candidates =
        crate::monomorphization::OperatorCandidateTemplates::candidate_requests(&program);
    let provider_templates = crate::monomorphization::OperatorCandidateTemplates::prepare(
        &program,
        &provider_candidates,
    );
    data_applications::retain_declared_field_scopes(&mut program)?;
    let _ = crate::constructed_results::retain_constructed_result_types(&mut program)?;
    loop {
        let first_new_machine = program.machines().len();
        let mut changed_roots = specialize_machine_calls(&mut program, true)?;
        let mut materialized = match &provider_templates {
            Some(templates) => crate::monomorphization::specialize_operator_candidates(
                templates,
                &mut program,
                &provider_candidates,
            )?,
            None => 0,
        };
        materialized +=
            crate::monomorphization::generate_dynamic_family_specializations(&mut program)?;
        instantiate_requirement_applications(&mut program)?;
        materialized +=
            crate::monomorphization::generate_requirement_provider_specializations(&mut program)?;
        let unavailable =
            crate::constructed_results::retain_constructed_result_types(&mut program)?;
        let result_owners =
            crate::monomorphization::refresh_inferred_result_locals(&mut program, &unavailable)?;
        materialized += result_owners.len();
        for owner in result_owners {
            changed_roots.include(owner);
        }
        materialized += constant_entries::materialize(&mut program)?;
        for machine in &program.machines()[first_new_machine..] {
            changed_roots.include(machine.symbol);
        }
        materialized +=
            crate::operators::bind_selected_operator_calls(&mut program, &changed_roots)?;
        if materialized == 0 {
            break;
        }
    }
    crate::assembly_control::normalize(&mut program)?;
    settle_value_destinations(&mut program)?;
    default_domain_zero::refine_default_domain_zero(&mut program);
    crate::constructed_results::retain_constructed_result_types(&mut program)?;
    elaborate(&mut program)?;
    runtime_index_fields::materialize(&mut program)?;
    validation::resolve_dynamic_call_targets(&mut program)?;
    let signature_type_start = program.type_reference_table.type_reference_count();
    crate::monomorphization::instantiate_dynamic_slots(&mut program)?;
    instantiate_requirement_applications(&mut program)?;
    while crate::monomorphization::prepare_installed_interrupts(&mut program)? {
        specialize_machine_calls(&mut program, !request.admits_pending_inputs)?;
        instantiate_requirement_applications(&mut program)?;
    }
    array_extents::normalize(&mut program, type_frontier);
    data_applications::elaborate_new_types(&mut program, signature_type_start)?;
    crate::assignment_destinations::prepare(&mut program);
    crate::record_patterns::prepare(&mut program);
    {
        let contracts = arena::Arena::default();
        let owners =
            validation::expression_owners::OwnerEnvironmentIndex::for_preparation(&program);
        validation::validate_domain_membership_carriers(&program, &contracts, &owners)
            .map_err(|diagnostic| vec![diagnostic])
    }?;
    crate::assembly_control::admit(&program)?;
    crate::monomorphization::require_complete_dispatch_subjects(&program)?;
    let mathematics = proof::mathematical_declarations::check_mathematical_declarations(&program)?;
    Ok((
        program.into(),
        TypedPreparation {
            admits_pending_inputs: request.admits_pending_inputs,
            deferred_layout_applications: request.deferred_layout_applications.to_vec(),
            mathematics,
            opaque_property_receipts,
            opaque_representation_selections,
        },
    ))
}

pub(crate) fn settle_value_destinations(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    validation::resolve_named_result_overloads(program)?;
    validation::bind_float_conversion_casts(program)?;
    validation::land_float_literal_destinations(program);
    Ok(())
}

pub(crate) fn normalize_open_index_identities(
    program: &mut TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    ::validation::normalize_open_index_expressions(program)?;
    crate::monomorphization::finalize_domain_instance_identities(
        program,
        &DeclarationSelection::all(),
    )
    .map_err(|diagnostic| vec![diagnostic])
}

pub fn prepare_execution_applications(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    let type_frontier = program.type_reference_table.type_reference_count();
    crate::policy_operators::elaborate(program)?;
    program
        .retain_static_argument_contexts()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    data_applications::retain_declared_field_scopes(program)?;
    let _ = crate::constructed_results::retain_constructed_result_types(program)?;
    loop {
        let signatures = program.closed_requirement_applications().count();
        specialize_machine_calls(program, false)?;
        let mut materialized =
            crate::monomorphization::generate_dynamic_family_specializations(program)?;
        instantiate_requirement_applications(program)?;
        materialized +=
            crate::monomorphization::generate_requirement_provider_specializations(program)?;
        if materialized == 0 && program.closed_requirement_applications().count() == signatures {
            break;
        }
    }
    array_extents::normalize(program, type_frontier);
    let checking = validation::CheckingContext::new(program);
    validation::static_arguments::validate_authored_static_arguments(&checking, &mut Vec::new())?;
    validation::static_bindings::validate_static_machine_bindings(&checking, &mut Vec::new())?;
    validation::conformance_applications::validate_conformance_applications(&checking)?;
    validation::validate_static_machine_selections(&checking)?;
    // Generic proofs must read the same checked mathematical facts as ordinary
    // source checking before a constant or layout computation can execute.
    let mathematics = proof::mathematical_declarations::check_mathematical_declarations(&checking)?;
    let checking = checking.with_prepared_mathematics(mathematics.prepared_view());
    validation::validate_generic_machine_contract_entailment(&checking)
}

pub fn prepare_constant_execution(mut program: TypedTrees) -> Result<TypedTrees, Vec<Diagnostic>> {
    let type_frontier = program.type_reference_table.type_reference_count();
    crate::policy_operators::elaborate(&mut program)?;
    value_location_alignment::settle(&mut program)?;
    elaborate(&mut program)?;
    crate::rebound_parameter_homes::materialize(&mut program);
    validation::resolve_dynamic_call_targets(&mut program)?;
    program
        .retain_static_argument_contexts()
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    selections::bind_pre_specialization_authored_selections(&mut program)
        .map_err(|diagnostic| vec![diagnostic])?;
    prepare_execution_applications(&mut program)?;
    settle_value_destinations(&mut program)?;
    crate::constructed_results::retain_constructed_result_types(&mut program)?;
    crate::assignment_destinations::prepare(&mut program);
    crate::record_patterns::prepare(&mut program);
    array_extents::normalize(&mut program, type_frontier);
    crate::monomorphization::require_complete_dispatch_subjects(&program)?;
    Ok(program)
}

pub(crate) fn specialize_machine_calls(
    program: &mut TypedTrees,
    enforce_complete_concrete_selections: bool,
) -> Result<DeclarationSelection, Vec<Diagnostic>> {
    specialize_machine_calls_unchecked(program, enforce_complete_concrete_selections)
        .map_err(|refusal| invalid_binding_behind(program, refusal))
}

fn specialize_machine_calls_unchecked(
    program: &mut TypedTrees,
    enforce_complete_concrete_selections: bool,
) -> Result<DeclarationSelection, Vec<Diagnostic>> {
    data_applications::retain_declared_field_scopes(program)?;
    crate::conformance::conformance_application_lifetimes::resolve_elided_conformance_lifetimes(
        program,
    )?;
    crate::conformance::explicit_requirement_calls::route_explicit_requirement_calls(program)?;
    crate::monomorphization::monomorphize_generic_machine_value_calls(
        program,
        enforce_complete_concrete_selections,
    )
}

fn instantiate_requirement_applications(program: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    crate::monomorphization::instantiate_requirement_applications(program)
        .map_err(|refusal| invalid_binding_behind(program, refusal))
}

/*
    Specialization dispatches through each closed application's bindings. A
    binding that names an implementation its bound does not admit therefore
    makes specialization refuse with whichever internal agreement it tripped
    first, such as "selected map disagrees with its supplying evidence bound".
    The binding check owns that mistake and names it for the author. When it
    finds one in the program as far as it was specialized, that is the cause
    and it is reported instead. Any other refusal stands as it is, and no
    refusal becomes an acceptance.
*/
fn invalid_binding_behind(program: &TypedTrees, refusal: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let checking = validation::CheckingContext::new(program);
    match validation::static_bindings::validate_static_machine_bindings(&checking, &mut Vec::new())
    {
        Err(bindings) if !bindings.is_empty() => bindings,
        _ => refusal,
    }
}
