/*
Child-before-parent derivation of completed array, constructor, borrow, and
call result types (`retain_constructed_result_types`) and Prop identity peer
context.
*/

use crate::expression_roots::{append_statement_roots, visit_post_order};
use crate::tables::{expressions_of, expressions_of_mut};
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::type_identity::applied_type_identity;
use typed_trees::types::TypeReferenceHandle;
use typed_trees::types::{FixedArrayLength, TypeReferenceNode};
use validation::canonical_place_source_type_application;
use validation::native_locators::unused_coordinate_widths;

pub(crate) mod call_results {
    use crate::tables::expressions_of;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::name::Identifier;
    use typed_trees::types::TypeReferenceHandle;
    use typed_trees::types::TypeReferenceNode;

    pub(crate) mod region_application {
        use crate::tables::expressions_of;
        use diagnostics::Diagnostic;
        use typed_trees::TypedTrees;
        use typed_trees::expression::TableCallExpression;
        use typed_trees::machine::Machine;
        use typed_trees::name::Identifier;
        use typed_trees::state::State;

        pub(crate) fn bindings(
            program: &TypedTrees,
            caller: &Machine,
            state: &State,
            call: &TableCallExpression,
        ) -> Result<Vec<(Identifier, Option<Identifier>)>, Vec<Diagnostic>> {
            let (parameters, lifetimes, declaration) =
                if let Some(signature) = program.requirement_call_signature(caller.symbol, call) {
                    (
                        program.state_signature_parameters(signature),
                        &signature.lifetime_parameters,
                        signature.symbol,
                    )
                } else if let Some((machine, entry)) =
                    validation::find_state(program, call.target_symbol)
                {
                    (
                        program.state_parameters(entry),
                        &machine.lifetime_parameters,
                        machine.symbol,
                    )
                } else if let Some((_, signature)) =
                    program.machine_parameter_signature(call.target_symbol)
                {
                    (
                        program.state_signature_parameters(signature),
                        &signature.lifetime_parameters,
                        signature.symbol,
                    )
                } else {
                    return Ok(Vec::new());
                };
            let mut regions = program
                .static_application_lifetimes_for_declaration(
                    call.authored_machine_application,
                    declaration,
                )
                .map_err(|reason| vec![Diagnostic::error(reason)])?
                .iter()
                .map(|binding| (binding.parameter.clone(), Some(binding.argument.clone())))
                .collect::<Vec<_>>();
            let mut conflict = false;
            for (parameter, argument) in parameters
                .iter()
                .zip(expressions_of(program).expression_handles(call.arguments))
            {
                let Some(actual) = crate::source_type_queries::expression_result_type_reference(
                    program, caller, state, *argument,
                ) else {
                    continue;
                };
                crate::conformance::conformance_application_lifetimes::visit_lifetime_bindings(
                    program,
                    parameter.type_reference,
                    actual,
                    lifetimes,
                    &mut |parameter, actual| {
                        let actual = actual.cloned();
                        if let Some((_, previous)) =
                            regions.iter_mut().find(|(bound, _)| bound == parameter)
                        {
                            if previous.is_none() {
                                *previous = actual;
                            } else if actual.is_some() && previous.as_ref() != actual.as_ref() {
                                conflict = true;
                            }
                        } else {
                            regions.push((parameter.clone(), actual));
                        }
                    },
                );
            }
            if conflict {
                return Err(vec![Diagnostic::error(
                    "call result has conflicting lifetime operands",
                )]);
            }
            Ok(regions)
        }
    }

    pub(crate) fn retain(
        program: &mut TypedTrees,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        expression: ExpressionHandle,
    ) -> Result<Option<TypeReferenceHandle>, Vec<diagnostics::Diagnostic>> {
        let Some(call) = expressions_of(program).call_node(expression) else {
            return Ok(None);
        };
        if call.native_code_layout.is_some() || call.layout_query.is_some() {
            let retained = expressions_of(program).constructed_value_type_reference(expression);
            return Ok(retained.is_valid().then_some(retained));
        }
        let Some(caller) = program.machine_by_symbol(machine_symbol) else {
            return Ok(None);
        };
        let Some(caller_state) = program.state_by_symbol(state_symbol) else {
            return Ok(None);
        };
        let (returned, substitutions) = if let Some(signature) =
            program.requirement_call_signature(machine_symbol, call)
        {
            if !signature.type_parameters.is_empty() {
                return Ok(None);
            }
            (signature.return_type, Vec::new())
        } else if let Some((machine, state)) = validation::find_state(program, call.target_symbol) {
            let Some(substitutions) = validation::call_type_arguments(
                program,
                caller,
                Some(caller_state),
                machine,
                state,
                &call.machine_arguments,
                expressions_of(program).expression_handles(call.arguments),
            ) else {
                return Ok(None);
            };
            (state.return_type, substitutions)
        } else if let Some((_, signature)) = program.machine_parameter_signature(call.target_symbol)
        {
            (signature.return_type, Vec::new())
        } else {
            return Ok(None);
        };
        if !returned.is_valid() {
            return Ok(None);
        }
        let lifetimes = region_application::bindings(program, caller, caller_state, call)?;
        let target = call.target_symbol;
        let parameters = validation::CallSite::Expression { expression, call }
            .parameters(program, machine_symbol)
            .map(|parameters| parameters.to_vec());
        let arguments = expressions_of(program)
            .expression_handles(call.arguments)
            .to_vec();
        let returned = if substitutions.is_empty() {
            returned
        } else {
            program
                .apply_callable_type(returned, &substitutions, &[], target)
                .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
        };
        let returned = apply_result_type(
            program,
            target,
            parameters.as_deref(),
            &arguments,
            returned,
            &lifetimes,
        )?;
        Ok(Some(retain_access_result_type(program, returned)))
    }

    fn retain_access_result_type(
        program: &mut TypedTrees,
        returned: TypeReferenceHandle,
    ) -> TypeReferenceHandle {
        match program
            .type_reference_table
            .type_reference(returned)
            .clone()
        {
            TypeReferenceNode::Reference {
                referee,
                access,
                lifetime,
            } => {
                let applied = retain_access_result_type(program, referee);
                if applied == referee {
                    return returned;
                }
                let node = TypeReferenceNode::Reference {
                    referee: applied,
                    access,
                    lifetime,
                };
                let existing = program
                    .type_reference_table
                    .type_reference_rows()
                    .find_map(|(reference, candidate)| (candidate == &node).then_some(reference));
                existing.unwrap_or_else(|| program.type_reference_table.insert(node))
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                lifetime_arguments,
                ..
            } if lifetime_arguments.is_empty() => {
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(arguments);
                let [policy, schema] = arguments else {
                    return returned;
                };
                let (
                    TypeReferenceNode::Named { symbol: policy, .. },
                    TypeReferenceNode::Named { symbol: schema, .. },
                ) = (
                    program.type_reference_table.type_reference(*policy),
                    program.type_reference_table.type_reference(*schema),
                )
                else {
                    return returned;
                };
                let Some(view) = program.access_view_plans.iter().find(|view| {
                    view.template_symbol.is_valid()
                        && view.template_symbol == base_symbol
                        && view.policy_symbol == *policy
                        && view.schema_symbol == *schema
                }) else {
                    return returned;
                };
                let symbol = view.data_symbol;
                if let Some(reference) = program
                    .type_reference_table
                    .find_named_type_reference(symbol)
                {
                    return reference;
                }
                let name = view.data_name.clone();
                program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol,
                        name: Identifier::generated(name),
                    })
            }
            _ => returned,
        }
    }

    fn apply_result_type(
        program: &mut TypedTrees,
        target: SymbolHandle,
        parameters: Option<&[typed_trees::signature::StateParameter]>,
        arguments: &[ExpressionHandle],
        return_type: TypeReferenceHandle,
        lifetimes: &[(Identifier, Option<Identifier>)],
    ) -> Result<TypeReferenceHandle, Vec<diagnostics::Diagnostic>> {
        let return_type = if lifetimes.is_empty() {
            return_type
        } else {
            program
                .apply_callable_result_type(return_type, lifetimes, target)
                .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
        };
        apply_runtime_result_operands(program, target, parameters, arguments, return_type)
    }

    fn apply_runtime_result_operands(
        program: &mut TypedTrees,
        target: SymbolHandle,
        parameters: Option<&[typed_trees::signature::StateParameter]>,
        arguments: &[ExpressionHandle],
        return_type: TypeReferenceHandle,
    ) -> Result<TypeReferenceHandle, Vec<diagnostics::Diagnostic>> {
        let Some(parameters) = parameters else {
            return Ok(return_type);
        };
        if parameters.len() != arguments.len() {
            return Ok(return_type);
        }
        let symbols = parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .collect::<Vec<_>>();
        let names_parameter = |program: &dyn validation::CheckingInput, expression| {
            let mut tree = Vec::new();
            validation::expression_traversal::collect_expression_tree(
                program, expression, &mut tree,
            );
            tree.into_iter().any(|handle| matches!(expressions_of(program).expression(handle),
            ExpressionNode::Name(path) if symbols.contains(&path.symbol) || symbols.contains(&path.head_symbol)))
        };
        let uses_actual = crate::monomorphization::for_each_type_reference(
            program,
            &[return_type],
            |program, node| match node {
                TypeReferenceNode::Named { symbol, .. } => symbols.contains(symbol),
                TypeReferenceNode::ConstExpression(expression) => {
                    names_parameter(program, *expression)
                }
                TypeReferenceNode::FixedArray {
                    length: typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. },
                    ..
                } => symbols.contains(symbol),
                _ => false,
            },
        );
        if !uses_actual {
            return Ok(return_type);
        }
        let substitutions = symbols
            .into_iter()
            .zip(arguments)
            .map(|(symbol, argument)| {
                (
                    symbol,
                    program
                        .type_reference_table
                        .insert(TypeReferenceNode::ConstExpression(*argument)),
                )
            })
            .collect::<Vec<_>>();
        program
            .apply_callable_type(return_type, &substitutions, &[], target)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])
    }
}
pub(crate) mod identity_context {
    /*
    Source relations and mathematical inputs supply constructor type arguments.
    Retain the exact peer or declared receiving type under the same telescope.
    These are inference proposals, never domain, ownership or identity evidence.
    */

    use super::ValueTypeOwner;
    use std::collections::HashMap;
    use typed_trees::{
        TypedTrees,
        domain::ProofFact,
        expression::{BinaryOperator, ExpressionHandle},
        types::{TypeReferenceHandle, TypeReferenceNode},
    };

    #[derive(Clone, Copy)]
    pub(crate) enum ConstructorContext {
        Peer(ExpressionHandle),
        Declared(TypeReferenceHandle),
    }

    pub(crate) type ConstructorPeers =
        HashMap<(ValueTypeOwner, ExpressionHandle), Vec<ConstructorContext>>;

    pub(crate) fn append(
        program: &dyn validation::CheckingInput,
        owner: ValueTypeOwner,
        fact: &ProofFact,
        peers: &mut ConstructorPeers,
    ) {
        let ProofFact::Expression(root) = fact else {
            return;
        };
        let mut pending = vec![*root];
        let mut visited = Vec::new();
        while let Some(expression) = pending.pop() {
            if visited.contains(&expression) {
                continue;
            }
            visited.push(expression);
            pending.extend(
                validation::logical_proposition_children(program, expression)
                    .into_iter()
                    .flatten(),
            );
            if let Some(call) = program.expression_table.call_node(expression)
                && let Some(types) =
                    validation::mathematical_applications::argument_type_references(program, call)
            {
                for (subject, reference) in program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .zip(types)
                {
                    if let Some(reference) = reference
                        && program
                            .expression_table
                            .struct_literal_node(*subject)
                            .is_some()
                    {
                        peers
                            .entry((owner, *subject))
                            .or_default()
                            .push(ConstructorContext::Declared(reference));
                    }
                }
            }
            let Some(binary) = program.expression_table.binary_node(expression) else {
                continue;
            };
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) {
                continue;
            }
            for (constructor, peer) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if program
                    .expression_table
                    .struct_literal_node(constructor)
                    .is_some()
                {
                    peers
                        .entry((owner, constructor))
                        .or_default()
                        .push(ConstructorContext::Peer(peer));
                }
            }
        }
    }

    pub(crate) fn argument_tuples(
        program: &TypedTrees,
        scope: ValueTypeOwner,
        constructor: ExpressionHandle,
        declaration: symbols::SymbolHandle,
        peers: &ConstructorPeers,
    ) -> Vec<Vec<TypeReferenceHandle>> {
        peers
            .get(&(scope, constructor))
            .into_iter()
            .flatten()
            .filter_map(|peer| {
                let reference = match peer {
                    ConstructorContext::Peer(peer) => scope.result_type(program, *peer)?,
                    ConstructorContext::Declared(reference) => *reference,
                };
                application_arguments(program, reference, declaration)
            })
            .collect()
    }

    pub(crate) fn application_arguments(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        declaration: symbols::SymbolHandle,
    ) -> Option<Vec<TypeReferenceHandle>> {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        let mut carrier = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        if let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(carrier)
        {
            carrier = program
                .unique_data_definition_by_symbol(*symbol)?
                .generic_instance?;
        }
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = program.type_reference_table.type_reference(carrier)
        else {
            return None;
        };
        (*base_symbol == declaration).then(|| {
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .to_vec()
        })
    }
}

pub(crate) struct UnavailableValueType {
    pub(crate) expression: ExpressionHandle,
    pub(crate) diagnostics: Vec<diagnostics::Diagnostic>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ValueTypeOwner {
    State {
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
    },
    Declaration {
        callable: symbols::SymbolHandle,
        parameters: arena::HandleSpan<typed_trees::signature::StateParameter>,
        trait_scope: Option<typed_trees::trait_definition::RetainedTraitScope>,
    },
}

impl ValueTypeOwner {
    fn symbol(self) -> symbols::SymbolHandle {
        match self {
            Self::State { machine, .. } => machine,
            Self::Declaration { callable, .. } => callable,
        }
    }

    fn source_applications(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<Vec<TypeReferenceHandle>> {
        match self {
            Self::State { state, .. } => {
                let Some(place) = validation::canonical_place_from_expression_in_state(
                    program,
                    state,
                    usize::MAX,
                    expression,
                ) else {
                    return self.result_type(program, expression).map(|_| Vec::new());
                };
                match canonical_place_source_type_application(program, state, usize::MAX, &place) {
                    Some((_, applications)) => Some(applications),
                    None if place.segments.is_empty() => Some(Vec::new()),
                    None => None,
                }
            }
            Self::Declaration {
                callable,
                parameters,
                ..
            } => validation::parameter_source_type_application(
                program,
                callable,
                program.state_parameters.span_or_empty(parameters),
                expression,
            )
            .map(|(_, applications)| applications)
            .or_else(|| {
                if validation::canonical_place_from_expression(program, expression).is_some() {
                    None
                } else {
                    self.result_type(program, expression).map(|_| Vec::new())
                }
            }),
        }
    }

    fn result_type(
        self,
        program: &TypedTrees,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        match self {
            Self::State { machine, state } => {
                crate::source_type_queries::expression_result_type_reference(
                    program,
                    program.machine_by_symbol(machine)?,
                    program.state_by_symbol(state)?,
                    expression,
                )
            }
            Self::Declaration {
                callable,
                parameters,
                trait_scope,
            } => crate::source_type_queries::parameter_expression_result_type_reference_in_scope(
                program,
                callable,
                program.state_parameters.span_or_empty(parameters),
                trait_scope,
                expression,
            ),
        }
    }
}

fn occurrences(
    program: &TypedTrees,
    selected: fn(&ExpressionNode) -> bool,
) -> (
    Vec<(ValueTypeOwner, ExpressionHandle)>,
    identity_context::ConstructorPeers,
) {
    let contract_input = validation::CheckingContext::new(program);
    let mut occurrences = Vec::new();
    let mut peers = identity_context::ConstructorPeers::new();
    for machine in program.machines() {
        for (state_ordinal, state) in program.machine_states(machine).iter().enumerate() {
            let mut expressions = Vec::new();
            let contracts = if state_ordinal == 0 {
                program.machine_contracts(machine)
            } else {
                &[]
            };
            for contract in contracts.iter().chain(program.state_contracts(state)) {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    identity_context::append(
                        &contract_input,
                        ValueTypeOwner::State {
                            machine: machine.symbol,
                            state: state.symbol,
                        },
                        fact,
                        &mut peers,
                    );
                    validation::append_fact_expression_roots(program, fact, &mut expressions);
                }
            }
            append_statement_roots(program, state, &mut expressions);
            visit_post_order(program, expressions, |expression, node| {
                if selected(node) {
                    occurrences.push((
                        ValueTypeOwner::State {
                            machine: machine.symbol,
                            state: state.symbol,
                        },
                        expression,
                    ));
                }
            });
        }
    }
    let mut declarations = Vec::<(ValueTypeOwner, Vec<ExpressionHandle>)>::new();
    let mut declaration_positions = symbols::SymbolKeyMap::<ValueTypeOwner, usize>::default();
    for (callable, _, parameters, trait_scope, contract) in
        validation::callable_contract_scopes(program)
    {
        // Entry contracts already use their actual executable state above.
        if program.machine_by_symbol(callable).is_some_and(|machine| {
            program
                .machine_states(machine)
                .first()
                .is_some_and(|state| state.parameters == parameters)
        }) {
            continue;
        }
        let owner = ValueTypeOwner::Declaration {
            callable,
            parameters,
            trait_scope,
        };
        let position = *declaration_positions.entry(owner).or_insert_with(|| {
            let position = declarations.len();
            declarations.push((owner, Vec::new()));
            position
        });
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            identity_context::append(&contract_input, owner, fact, &mut peers);
            validation::append_fact_expression_roots(program, fact, &mut declarations[position].1);
        }
    }
    for (owner, roots) in declarations {
        visit_post_order(program, roots, |expression, node| {
            if selected(node) {
                occurrences.push((owner, expression));
            }
        });
    }
    (occurrences, peers)
}

fn retain_borrow_result_type(
    program: &mut TypedTrees,
    scope: ValueTypeOwner,
    expression: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let derived = (|| {
        let formation = validation::reference_formation_source(program, expression)?;
        let target = scope.result_type(program, formation.source)?;
        if let ValueTypeOwner::State { state, .. } = scope {
            let state = program.state_by_symbol(state)?;
            let mut requested = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .filter_map(|statement| match statement {
                    typed_trees::statement::StatementNode::LocalData(local)
                        if local.initial_value == expression && !local.type_is_inferred =>
                    {
                        match program
                            .type_reference_table
                            .type_reference(local.type_reference)
                        {
                            TypeReferenceNode::Reference {
                                referee,
                                access,
                                lifetime,
                            } if *access == formation.access
                                && program.normalized_type_identity(*referee)
                                    == program.normalized_type_identity(target) =>
                            {
                                Some(TypeReferenceNode::Reference {
                                    referee: target,
                                    access: *access,
                                    lifetime: lifetime.clone(),
                                })
                            }
                            _ => None,
                        }
                    }
                    _ => None,
                });
            if let Some(reference) = requested.next() {
                if requested.next().is_none() {
                    return Some(reference);
                }
                return None;
            }
        }
        let carrier = program
            .type_reference_table
            .unconstrained_type_reference(target);
        let (referee, lifetime) = match program.type_reference_table.type_reference(carrier) {
            TypeReferenceNode::Reference {
                referee, lifetime, ..
            } => (*referee, lifetime.clone()),
            _ => (target, None),
        };
        Some(TypeReferenceNode::Reference {
            referee: if formation.view_type.is_valid() {
                formation.view_type
            } else {
                referee
            },
            access: formation.access,
            lifetime,
        })
    })();
    derived.map(|node| {
        let retained = expressions_of(program).constructed_value_type_reference(expression);
        let existing = if retained.is_valid()
            && program.type_reference_table.type_reference(retained) == &node
        {
            Some(retained)
        } else {
            program
                .type_reference_table
                .type_reference_rows()
                .find_map(|(reference, candidate)| (candidate == &node).then_some(reference))
        };
        existing.unwrap_or_else(|| program.type_reference_table.insert(node))
    })
}

pub(crate) fn retain_constructed_result_types(
    program: &mut TypedTrees,
) -> Result<Vec<UnavailableValueType>, Vec<diagnostics::Diagnostic>> {
    let mut unavailable = Vec::new();
    let (occurrences, peers) = occurrences(program, |node| {
        matches!(
            node,
            ExpressionNode::ArrayLiteral(_)
                | ExpressionNode::StructLiteral(_)
                | ExpressionNode::Borrow(_)
                | ExpressionNode::Call(_)
        ) || matches!(node, ExpressionNode::Cast(cast) if cast.form.is_recast())
    });
    for (scope, expression) in occurrences {
        let type_start = program.type_reference_table.type_reference_count();
        let reference = if expressions_of(program).call_node(expression).is_some() {
            let result = match scope {
                ValueTypeOwner::State { machine, state } => {
                    call_results::retain(program, machine, state, expression)
                }
                ValueTypeOwner::Declaration { .. } => {
                    expressions_of_mut(program).set_constructed_value_type_reference(
                        expression,
                        TypeReferenceHandle::invalid(),
                    );
                    Ok(scope.result_type(program, expression))
                }
            };
            match result {
                Ok(reference) => reference,
                Err(diagnostics) => {
                    unavailable.push(UnavailableValueType {
                        expression,
                        diagnostics,
                    });
                    None
                }
            }
        } else if validation::reference_formation_source(program, expression).is_some() {
            retain_borrow_result_type(program, scope, expression)
        } else if expressions_of(program)
            .struct_literal_node(expression)
            .is_some()
        {
            retain_constructor_result_type(program, scope, expression, &peers)
        } else {
            let derived = (|| {
                let elements = expressions_of(program).array_literal_node(expression)?;
                let elements = expressions_of(program).expression_handles(*elements);
                let references = elements
                    .iter()
                    .map(|element| scope.result_type(program, *element))
                    .collect::<Vec<_>>();
                let element_type = validation::join_result_type_references(
                    program,
                    &references,
                    &vec![false; references.len()],
                )?;
                Some((element_type, elements.len()))
            })();
            derived.map(|(element_type, length)| {
                let node = TypeReferenceNode::FixedArray {
                    element_type,
                    length: FixedArrayLength::Literal(length),
                };
                let retained = expressions_of(program).constructed_value_type_reference(expression);
                if retained.is_valid()
                    && program.type_reference_table.type_reference(retained) == &node
                {
                    retained
                } else {
                    program.type_reference_table.insert(node)
                }
            })
        };
        let produced = program
            .type_reference_table
            .type_reference_rows_from(type_start)
            .map(|(reference, _)| reference)
            .collect::<Vec<_>>();
        crate::monomorphization::finalize_type_domain_instance_identities(program, produced)
            .map_err(|diagnostic| vec![diagnostic])?;
        let Some(reference) = reference else {
            expressions_of_mut(program)
                .set_constructed_value_type_reference(expression, TypeReferenceHandle::invalid());
            continue;
        };
        expressions_of_mut(program).set_constructed_value_type_reference(expression, reference);
    }
    Ok(unavailable)
}

fn selected_constructor_field<'program>(
    program: &'program TypedTrees,
    literal: &typed_trees::expression::TableStructLiteral,
    field: symbols::SymbolHandle,
) -> Option<&'program typed_trees::data::DataField> {
    let owner = program.unique_data_definition_by_symbol(literal.type_symbol)?;
    program
        .data_members(owner)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(declared) if declared.symbol == field => Some(declared),
            DataMember::Variant(case) if Some(case.symbol) == literal.case_symbol => program
                .data_payload_fields(case)
                .iter()
                .find(|declared| declared.symbol == field),
            _ => None,
        })
}

fn constructor_byte_literal_observations(
    program: &mut TypedTrees,
    expression: ExpressionHandle,
) -> Vec<(symbols::SymbolHandle, ExpressionHandle, TypeReferenceHandle)> {
    let observations = (|| {
        let literal = expressions_of(program).struct_literal_node(expression)?;
        Some(
            expressions_of(program)
                .struct_fields(literal.fields)
                .iter()
                .filter_map(|field| {
                    let ExpressionNode::String(bytes) =
                        expressions_of(program).expression(field.value)
                    else {
                        return None;
                    };
                    let declared =
                        selected_constructor_field(program, literal, field.field_symbol)?;
                    let required = program
                        .type_reference_table
                        .unconstrained_type_reference(declared.type_reference);
                    let TypeReferenceNode::FixedArray { element_type, .. } =
                        program.type_reference_table.type_reference(required)
                    else {
                        return None;
                    };
                    let element = program
                        .type_reference_table
                        .unconstrained_type_reference(*element_type);
                    let TypeReferenceNode::Named { symbol, .. } =
                        program.type_reference_table.type_reference(element)
                    else {
                        return None;
                    };
                    (program.symbols.builtin_type_atom(*symbol)
                        == Some(symbols::BuiltinTypeAtom::U8))
                    .then_some((field.field_symbol, field.value, element, bytes.len()))
                })
                .collect::<Vec<_>>(),
        )
    })()
    .unwrap_or_default();
    observations
        .into_iter()
        .map(|(field, value, element_type, length)| {
            let node = TypeReferenceNode::FixedArray {
                element_type,
                length: FixedArrayLength::Literal(length),
            };
            let existing = program
                .type_reference_table
                .type_reference_rows()
                .find_map(|(reference, candidate)| (candidate == &node).then_some(reference));
            (
                field,
                value,
                existing.unwrap_or_else(|| program.type_reference_table.insert(node)),
            )
        })
        .collect()
}

fn retain_constructor_result_type(
    program: &mut TypedTrees,
    scope: ValueTypeOwner,
    expression: ExpressionHandle,
    peers: &identity_context::ConstructorPeers,
) -> Option<TypeReferenceHandle> {
    let byte_copies = constructor_byte_literal_observations(program, expression);
    let defaults = expressions_of(program)
        .struct_literal_node(expression)
        .and_then(|literal| {
            unused_coordinate_widths(program, literal.type_symbol, literal.case_symbol?)
        });
    let zero = if defaults.is_some_and(|defaults| defaults.into_iter().any(|unused| unused)) {
        let retained = program.type_reference_table.named_references().find_map(
            |(reference, symbol, name)| (!symbol.is_valid() && name == "0").then_some(reference),
        );
        retained.unwrap_or_else(|| {
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: symbols::SymbolHandle::invalid(),
                    name: typed_trees::name::Identifier::generated("0"),
                })
        })
    } else {
        TypeReferenceHandle::invalid()
    };
    let literal = expressions_of(program).struct_literal_node(expression)?;
    let owner = program.unique_data_definition_by_symbol(literal.type_symbol)?;
    let parameters = program.data_type_parameters(owner);
    if parameters.is_empty() && owner.lifetime_parameters.is_empty() {
        return program
            .type_reference_table
            .find_named_type_reference(owner.symbol);
    }
    let type_parameters = parameters
        .iter()
        .filter(|parameter| parameter.kind == typed_trees::data::TypeParameterKind::Type)
        .map(|parameter| (parameter.symbol, parameter.name.to_string()))
        .collect::<Vec<_>>();
    let const_parameters = parameters
        .iter()
        .filter_map(|parameter| match parameter.kind {
            typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                Some((parameter.symbol, parameter.name.to_string(), type_reference))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let context = crate::monomorphization::BindingContext::new(
        program,
        &type_parameters,
        &const_parameters,
        0,
    );
    let mut inferred = crate::monomorphization::StaticProposals::default();
    let source_arguments =
        identity_context::application_arguments(program, literal.application_type, owner.symbol);
    for arguments in source_arguments
        .into_iter()
        .chain(identity_context::argument_tuples(
            program,
            scope,
            expression,
            owner.symbol,
            peers,
        ))
    {
        if arguments.len() != parameters.len() {
            continue;
        }
        for (parameter, argument) in parameters.iter().zip(arguments) {
            if let Some(ordinal) = type_parameters
                .iter()
                .position(|(symbol, _)| *symbol == parameter.symbol)
            {
                inferred.types.push((0, ordinal, argument));
            } else if let Some(ordinal) = const_parameters
                .iter()
                .position(|(symbol, _, _)| *symbol == parameter.symbol)
            {
                inferred.constants.push((0, ordinal, argument));
            }
        }
    }
    let mut lifetime_bindings = Vec::new();
    let mut lifetime_conflict = false;
    for field in expressions_of(program).struct_fields(literal.fields) {
        let required = selected_constructor_field(program, literal, field.field_symbol)?;
        let Some(actual) = byte_copies
            .iter()
            .find_map(|(selected_field, value, reference)| {
                (*selected_field == field.field_symbol && *value == field.value)
                    .then_some(*reference)
            })
            .or_else(|| scope.result_type(program, field.value))
        else {
            continue;
        };
        crate::monomorphization::infer_static_bindings(
            context,
            required.type_reference,
            actual,
            &mut inferred,
        );
        if owner.lifetime_parameters.is_empty()
            || program
                .primitive_type_reference(required.type_reference)
                .is_some()
        {
            continue;
        }
        let applications = scope.source_applications(program, field.value)?;
        crate::conformance::conformance_application_lifetimes::collect_declared_field_lifetime_bindings(
            program,
            owner,
            required,
            scope.symbol(),
            &applications,
            actual,
            &mut lifetime_bindings,
            &mut lifetime_conflict,
        )?;
    }
    if lifetime_conflict {
        return None;
    }
    let lifetime_arguments = owner
        .lifetime_parameters
        .iter()
        .map(|parameter| {
            lifetime_bindings
                .iter()
                .find(|(name, _)| name == parameter.as_str())
                .map(|(_, argument)| argument.as_str().into())
        })
        .collect::<Option<Vec<_>>>()?;
    let mut arguments = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let proposals = match parameter.kind {
            typed_trees::data::TypeParameterKind::Type => {
                let ordinal = type_parameters
                    .iter()
                    .position(|(symbol, _)| *symbol == parameter.symbol)?;
                inferred
                    .types
                    .iter()
                    .filter(|(_, candidate, _)| *candidate == ordinal)
                    .map(|(_, _, reference)| *reference)
                    .collect::<Vec<_>>()
            }
            typed_trees::data::TypeParameterKind::Const { .. }
            | typed_trees::data::TypeParameterKind::Value { .. } => {
                let ordinal = const_parameters
                    .iter()
                    .position(|(symbol, _, _)| *symbol == parameter.symbol)?;
                inferred
                    .constants
                    .iter()
                    .filter(|(_, candidate, _)| *candidate == ordinal)
                    .map(|(_, _, reference)| *reference)
                    .collect::<Vec<_>>()
            }
            _ => return None,
        };
        if proposals.is_empty()
            && let Some(defaults) = defaults
            && const_parameters
                .iter()
                .position(|(symbol, _, _)| *symbol == parameter.symbol)
                .is_some_and(|ordinal| defaults[ordinal])
        {
            arguments.push(zero);
            continue;
        }
        let first = *proposals.first()?;
        let identity = applied_type_identity(program, first, scope.symbol(), &[])?;
        if proposals.iter().any(|reference| {
            applied_type_identity(program, *reference, scope.symbol(), &[])
                .is_none_or(|candidate| candidate != identity)
        }) {
            return None;
        }
        arguments.push(first);
    }
    let base_symbol = owner.symbol;
    let base_name = owner.name.clone();
    for retained in [
        literal.application_type,
        expressions_of(program).constructed_value_type_reference(expression),
    ] {
        if !retained.is_valid() {
            continue;
        }
        let retained = program
            .type_reference_table
            .unconstrained_type_reference(retained);
        if let TypeReferenceNode::Generic {
            base_symbol: previous_base,
            lifetime_arguments: previous_lifetimes,
            arguments: previous_arguments,
            ..
        } = program.type_reference_table.type_reference(retained)
            && *previous_base == base_symbol
            && *previous_lifetimes == lifetime_arguments
            && program
                .type_reference_table
                .type_reference_handles(*previous_arguments)
                == arguments.as_slice()
        {
            return Some(retained);
        }
    }
    let arguments = program
        .type_reference_table
        .insert_type_reference_handles(arguments);
    Some(
        program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol,
                base_name,
                lifetime_arguments,
                arguments,
            }),
    )
}
