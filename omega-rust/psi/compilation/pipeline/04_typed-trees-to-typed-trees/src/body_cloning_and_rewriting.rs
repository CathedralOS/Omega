/*
Deep-copying of specialized machine bodies and constant roots (`body_cloning`)
and post-clone call, const, type-parameter, machine-argument, and evidence
rewriting (`body_rewriting`).
*/

pub(crate) mod body_cloning {
    pub(crate) mod constant_roots {
        use crate::tables::{expressions_of, expressions_of_mut};
        use diagnostics::Diagnostic;
        use typed_trees::TypedTrees;
        use typed_trees::constant_recipes::{
            ConstantExpressionContext, RetainedConstantPosition, RetainedConstantRecipe,
            retain_constant_recipes,
        };
        use typed_trees::expression::{ExpressionCopy, ExpressionHandle};

        pub(crate) fn compose_type_copies(
            staged: &[typed_trees::types::TypeExpressionCopy],
            copied: &mut [typed_trees::types::TypeExpressionCopy],
        ) -> Result<(), super::TypeCopyError> {
            for copied in copied {
                let staged = staged
                    .iter()
                    .find(|staged| staged.destination == copied.source)
                    .ok_or(super::TypeCopyError(
                        "type copy lost its staging occurrence",
                    ))?;
                copied.source = staged.source;
                for (original, _) in &mut copied.expression.correspondence {
                    *original = staged
                        .expression
                        .correspondence
                        .iter()
                        .find(|(_, intermediate)| intermediate == original)
                        .map(|(source, _)| *source)
                        .ok_or(super::TypeCopyError("type copy lost its staged expression"))?;
                }
            }
            Ok(())
        }

        pub(crate) fn copy_type_contexts<'scope>(
            source: Option<&dyn validation::CheckingInput>,
            program: &mut TypedTrees,
            expressions: &[typed_trees::types::TypeExpressionCopy],
            symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
        ) -> Result<(), super::TypeCopyError> {
            let symbols = symbols.into();
            let source_program = source.unwrap_or(program);
            let mut pending = Vec::new();
            let mut marked = Vec::new();
            for copied in expressions {
                marked.extend(
                    copied
                        .expression
                        .correspondence
                        .iter()
                        .filter(|(original, _)| {
                            source_program
                                .pending_const_range_endpoints
                                .contains(original)
                        })
                        .map(|(_, destination)| *destination),
                );
                for (_, recipe) in source_program.constant_recipes.iter() {
                    let RetainedConstantPosition::ConstApplication { site, .. } = recipe.position
                    else {
                        continue;
                    };
                    if site != copied.source {
                        continue;
                    }
                    let contexts = source_program
                        .constant_expression_contexts
                        .span_or_empty(recipe.contexts);
                    if contexts.len() != recipe.contexts.len() {
                        return Err(super::TypeCopyError(
                            "copied constant root lost its lexical context span",
                        ));
                    }
                    let contexts = contexts.to_vec();
                    let mapped = |original| {
                        copied
                            .expression
                            .correspondence
                            .iter()
                            .find(|(source, _)| *source == original)
                            .map(|(_, destination)| *destination)
                            .ok_or(super::TypeCopyError(
                                "copied constant root lost its invocation correspondence",
                            ))
                    };
                    let invocations =
                        expressions_of(source_program).expression_handles(recipe.invocations);
                    if invocations.len() != recipe.invocations.len() {
                        return Err(super::TypeCopyError(
                            "copied constant root lost its invocation span",
                        ));
                    }
                    let invocations = invocations
                        .iter()
                        .map(|invocation| mapped(*invocation))
                        .collect::<Result<Vec<_>, _>>()?;
                    pending.push((
                        recipe.clone(),
                        copied.destination,
                        copied.expression.root,
                        contexts,
                        invocations,
                    ));
                }
            }
            for (recipe, site, root, contexts, invocations) in pending {
                let contexts = contexts
                    .into_iter()
                    .map(|context| {
                        Ok(ConstantExpressionContext::new(
                            super::remapped_symbol(context.owner, &symbols),
                            super::copy_type_reference(
                                source,
                                program,
                                context.result_type,
                                symbols,
                            )?,
                        ))
                    })
                    .collect::<Result<Vec<_>, super::TypeCopyError>>()?;
                let recipe = RetainedConstantRecipe {
                    position: RetainedConstantPosition::ConstApplication { site, root },
                    contexts: program.constant_expression_contexts.insert_many(contexts),
                    invocations: expressions_of_mut(program).insert_expression_handles(invocations),
                    machine: super::remapped_symbol(recipe.machine, &symbols),
                    name: recipe.name,
                    source_span: recipe.source_span,
                };
                retain_constant_recipes(program, [recipe]);
            }
            program.pending_const_range_endpoints.extend(marked);
            Ok(())
        }

        pub(crate) fn copy_expression<'scope>(
            source: Option<&dyn validation::CheckingInput>,
            program: &mut TypedTrees,
            expression: ExpressionHandle,
            symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
        ) -> Result<ExpressionHandle, Vec<Diagnostic>> {
            let symbols = symbols.into();
            let copied = expressions_of_mut(program)
                .copy_graph(source.map(|source| &source.expression_table), expression);
            expressions_of_mut(program).remap_symbols_in(copied.root, symbols);
            copy_contexts(source, program, &copied, symbols)?;
            Ok(copied.root)
        }

        fn copy_contexts<'scope>(
            source: Option<&dyn validation::CheckingInput>,
            program: &mut TypedTrees,
            copied: &ExpressionCopy,
            symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
        ) -> Result<(), Vec<Diagnostic>> {
            let symbols = symbols.into();
            let source_program = source.unwrap_or(program);
            let mut pending = Vec::new();
            for (_, recipe) in source_program.constant_recipes.iter() {
                let RetainedConstantPosition::ContractExpression(root) = recipe.position else {
                    continue;
                };
                let Some((_, copied_root)) = copied
                    .correspondence
                    .iter()
                    .find(|(original, _)| *original == root)
                else {
                    continue;
                };
                let contexts = source_program
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts);
                if contexts.len() != recipe.contexts.len() {
                    return Err(vec![Diagnostic::error(
                        "copied constant root lost its lexical context span",
                    )]);
                }
                let contexts = contexts
                    .iter()
                    .filter_map(|context| {
                        let (_, owner) = symbols
                            .iter()
                            .find(|(original, _)| *original == context.owner)?;
                        Some(ConstantExpressionContext {
                            owner: *owner,
                            result_type: context.result_type,
                            ..Default::default()
                        })
                    })
                    .collect::<Vec<_>>();
                if contexts.is_empty() {
                    continue;
                }
                let invocations =
                    expressions_of(source_program).expression_handles(recipe.invocations);
                if invocations.len() != recipe.invocations.len() {
                    return Err(vec![Diagnostic::error(
                        "copied constant root lost its invocation span",
                    )]);
                }
                let mut copied_invocations = Vec::with_capacity(invocations.len());
                for invocation in invocations {
                    let Some((_, mapped)) = copied
                        .correspondence
                        .iter()
                        .find(|(original, _)| original == invocation)
                    else {
                        return Err(vec![
                    Diagnostic::error(
                        "copied constant root has an invocation outside its expression graph",
                    )
                    .with_source_span(recipe.source_span),
                ]);
                    };
                    copied_invocations.push(*mapped);
                }
                let marked = copied
                    .correspondence
                    .iter()
                    .filter(|(original, _)| {
                        source_program
                            .pending_const_range_endpoints
                            .contains(original)
                    })
                    .map(|(_, mapped)| *mapped)
                    .collect::<Vec<_>>();
                pending.push((
                    recipe.clone(),
                    *copied_root,
                    contexts,
                    copied_invocations,
                    marked,
                ));
            }
            for (recipe, root, contexts, invocations, marked) in pending {
                let contexts = contexts
                    .into_iter()
                    .map(|context| {
                        Ok(ConstantExpressionContext {
                            owner: context.owner,
                            result_type: super::copy_type_reference(
                                source,
                                program,
                                context.result_type,
                                symbols,
                            )?,
                            ..Default::default()
                        })
                    })
                    .collect::<Result<Vec<_>, super::TypeCopyError>>()?;
                let recipe = RetainedConstantRecipe {
                    position: RetainedConstantPosition::ContractExpression(root),
                    contexts: program.constant_expression_contexts.insert_many(contexts),
                    invocations: expressions_of_mut(program).insert_expression_handles(invocations),
                    machine: super::remapped_symbol(recipe.machine, &symbols),
                    name: recipe.name,
                    source_span: recipe.source_span,
                };
                retain_constant_recipes(program, [recipe]);
                program.pending_const_range_endpoints.extend(marked);
            }
            Ok(())
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use symbols::SymbolHandle;
            use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
            use typed_trees::types::TypeReferenceNode;

            #[test]
            fn constant_type_copy_retains_context_and_pending_work_without_closed_authority() {
                let mut program = TypedTrees::default();
                let owner = SymbolHandle::from_parts(17, 1);
                let instance = SymbolHandle::from_parts(23, 1);
                let root = expressions_of_mut(&mut program).insert(ExpressionNode::default());
                let site = program
                    .type_reference_table
                    .insert(TypeReferenceNode::ConstExpression(root));
                let carrier = program.type_reference_table.insert(TypeReferenceNode::Unit);
                let contexts =
                    program
                        .constant_expression_contexts
                        .insert_many([ConstantExpressionContext {
                            owner,
                            result_type: carrier,
                            evaluation_entry: SymbolHandle::from_parts(31, 1),
                            ..Default::default()
                        }]);
                let invocations =
                    expressions_of_mut(&mut program).insert_expression_handles([root]);
                retain_constant_recipes(
                    &mut program,
                    [RetainedConstantRecipe {
                        position: RetainedConstantPosition::ConstApplication { site, root },
                        contexts,
                        invocations,
                        ..Default::default()
                    }],
                );
                program.pending_const_range_endpoints.insert(root);
                let copied = super::super::copy_type_reference(
                    None,
                    &mut program,
                    site,
                    &[(owner, instance)],
                )
                .expect("copy the type and its complete constant application");
                assert_ne!(copied, site);
                let recipe = program.constant_recipe_for_const_application(copied);
                assert!(recipe.is_valid());
                assert!(program.closed_constant_recipe_value(recipe).is_none());
                let recipe = program.constant_recipes.get(recipe);
                let copied_root = recipe.position.expression();
                assert_ne!(copied_root, root);
                let [context] = program
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
                else {
                    panic!("one copied context");
                };
                assert_eq!(context.owner, instance);
                assert_ne!(context.result_type, carrier);
                assert!(!context.evaluation_entry.is_valid());
                assert_eq!(
                    expressions_of(&program).expression_handles(recipe.invocations),
                    &[copied_root]
                );
                assert!(program.pending_const_range_endpoints.contains(&copied_root));
            }

            #[test]
            fn constant_type_copy_keeps_preliminary_marks_before_recipes_exist() {
                let mut program = TypedTrees::default();
                let root = expressions_of_mut(&mut program).insert(ExpressionNode::default());
                let site = program
                    .type_reference_table
                    .insert(TypeReferenceNode::ConstExpression(root));
                program.pending_const_range_endpoints.insert(root);
                let copied = super::super::copy_type_reference(None, &mut program, site, &[])
                    .expect("copy the pending type expression");
                let TypeReferenceNode::ConstExpression(copied_root) =
                    program.type_reference_table.type_reference(copied)
                else {
                    panic!("copied const argument");
                };
                assert_ne!(*copied_root, root);
                assert!(program.pending_const_range_endpoints.contains(copied_root));
                assert!(
                    !program
                        .constant_recipe_for_const_application(copied)
                        .is_valid()
                );
            }

            #[test]
            fn constant_root_copy_retains_only_the_remapped_scope_and_fresh_type_payloads() {
                let mut program = TypedTrees::default();
                let owner = SymbolHandle::from_parts(17, 1);
                let foreign = SymbolHandle::from_parts(23, 1);
                let instance = SymbolHandle::from_parts(29, 1);
                let carrier = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Named {
                        symbol: SymbolHandle::invalid(),
                        name: typed_trees::name::Identifier::generated("u64"),
                    });
                let leaf = expressions_of_mut(&mut program).insert_tree(
                    &typed_trees::expression::Expression::Call(Box::new(
                        typed_trees::expression::CallExpression {
                            authored_call_selection: None,
                            authored_call_binding: None,
                            exact_coercion_target: Default::default(),
                            requirement_application: Default::default(),
                            receiver: None,
                            target_symbol: SymbolHandle::from_parts(37, 1),
                            target: typed_trees::name::Identifier::generated("extent"),
                            arguments: Default::default(),
                            evidence_arguments: Default::default(),
                            operational_acknowledgement: Default::default(),
                        },
                    )),
                );
                let root = expressions_of_mut(&mut program).insert(ExpressionNode::Binary(
                    TableBinaryExpression {
                        policy: numerics::arithmetic::ArithmeticDomain::Exact,
                        left: leaf,
                        operator: BinaryOperator::Add,
                        right: leaf,
                    },
                ));
                let contexts =
                    program
                        .constant_expression_contexts
                        .insert_many([owner, foreign].map(|owner| ConstantExpressionContext {
                            owner,
                            result_type: carrier,
                            ..Default::default()
                        }));
                let invocations =
                    expressions_of_mut(&mut program).insert_expression_handles([leaf]);
                retain_constant_recipes(
                    &mut program,
                    [RetainedConstantRecipe {
                        position: RetainedConstantPosition::ContractExpression(root),
                        contexts,
                        invocations,
                        ..Default::default()
                    }],
                );
                program.pending_const_range_endpoints.extend([root, leaf]);
                let contract = typed_trees::signature::SignatureContract {
                    facts: program
                        .proof_facts
                        .insert_many([typed_trees::domain::ProofFact::Expression(root)]),
                    ..Default::default()
                };
                let copied = super::super::copy_signature_contract(
                    None,
                    &mut program,
                    contract,
                    &[(owner, instance)],
                )
                .expect("copy contract and retained constant");
                let [typed_trees::domain::ProofFact::Expression(copied_root)] =
                    program.proof_facts.span_or_empty(copied.facts)
                else {
                    panic!("copied contract root");
                };
                let copied_root = *copied_root;
                assert_ne!(root, copied_root);
                let recipe = program.constant_recipe_for_contract_expression(copied_root);
                let recipe = program.constant_recipes.get(recipe);
                let [context] = program
                    .constant_expression_contexts
                    .span_or_empty(recipe.contexts)
                else {
                    panic!("only the copied owner belongs to this recipe");
                };
                assert_eq!(context.owner, instance);
                assert_ne!(context.result_type, carrier);
                assert_eq!(
                    program
                        .type_reference_table
                        .type_reference(context.result_type),
                    program.type_reference_table.type_reference(carrier)
                );
                let [invocation] = expressions_of(&program).expression_handles(recipe.invocations)
                else {
                    panic!("one copied invocation");
                };
                assert_ne!(*invocation, leaf);
                let ExpressionNode::Binary(binary) =
                    expressions_of(&program).expression(copied_root)
                else {
                    panic!("complete copied root");
                };
                assert_eq!(binary.left, *invocation);
                assert_eq!(binary.right, *invocation);
                assert!(program.pending_const_range_endpoints.contains(&copied_root));
                assert!(program.pending_const_range_endpoints.contains(invocation));
                let original = program.constant_recipe_for_contract_expression(root);
                assert_eq!(
                    program
                        .constant_expression_contexts
                        .span_or_empty(program.constant_recipes.get(original).contexts,)
                        .len(),
                    2
                );
            }

            #[test]
            fn constant_root_copy_refuses_a_dependency_outside_its_owned_graph() {
                let mut program = TypedTrees::default();
                let owner = SymbolHandle::from_parts(17, 1);
                let instance = SymbolHandle::from_parts(23, 1);
                let root = expressions_of_mut(&mut program).insert(ExpressionNode::default());
                let outside = expressions_of_mut(&mut program).insert(ExpressionNode::default());
                let contexts =
                    program
                        .constant_expression_contexts
                        .insert_many([ConstantExpressionContext {
                            owner,
                            ..Default::default()
                        }]);
                let invocations =
                    expressions_of_mut(&mut program).insert_expression_handles([outside]);
                retain_constant_recipes(
                    &mut program,
                    [RetainedConstantRecipe {
                        position: RetainedConstantPosition::ContractExpression(root),
                        contexts,
                        invocations,
                        ..Default::default()
                    }],
                );
                let before = program.constant_recipes.len();
                let errors = copy_expression(None, &mut program, root, &[(owner, instance)])
                    .expect_err("an unrelated invocation cannot become copied evidence");
                assert!(
                    errors
                        .iter()
                        .any(|error| error.to_string().contains("outside its expression graph"))
                );
                assert_eq!(program.constant_recipes.len(), before);
            }
        }
    }

    use crate::tables::{expressions_of, expressions_of_mut};
    use language_semantics::TerminationInterface;
    use symbols::JoinedPath;
    use typed_trees::data::DataMember;
    use typed_trees::domain::ProofFact;
    use typed_trees::name::Identifier;
    use typed_trees::proposition::PropositionBinderArgument;
    use typed_trees::ranking::resolve_machine_witness_subjects;
    use typed_trees::ranking::resolve_machine_witness_view_arguments;
    use typed_trees::references::StaticArgumentCall;
    use typed_trees::signature::AuthoredInvocationTarget;
    use typed_trees::signature::AuthoredServiceReachTarget;
    use typed_trees::typed_trees::MachineSpecialization;
    use validation::collect_statement_expression_trees;

    use crate::monomorphization::body_rewriting::{
        cloned_expression_roots, drop_refuted_closed_transition_arms,
        rebind_state_value_parameters, reject_runtime_bound_static_occurrences,
        rewrite_cloned_calls, statement_span_handles, substitute_cloned_type_parameters,
    };
    use crate::monomorphization::{
        Candidate, candidate_conformance_fingerprint_arguments,
        closed_operator_realizations_for_machine, const_values, remapped_symbol,
        specialized_attached_data,
    };
    use crate::monomorphization::{
        Diagnostic, ExpressionHandle, ExpressionNode, HandleSpan, StatementNode, SymbolHandle,
        SymbolKind, TypeReferenceHandle, TypeReferenceNode, TypedTrees, failed,
    };
    use validation::contract_identities::specialization_selection_report_fingerprint;

    #[derive(Debug)]
    pub(crate) struct TypeCopyError(&'static str);

    impl From<TypeCopyError> for Diagnostic {
        fn from(error: TypeCopyError) -> Self {
            Diagnostic::error(error.0)
        }
    }
    impl From<TypeCopyError> for Vec<Diagnostic> {
        fn from(error: TypeCopyError) -> Self {
            vec![Diagnostic::from(error)]
        }
    }
    impl From<TypeCopyError> for String {
        fn from(error: TypeCopyError) -> Self {
            error.0.to_owned()
        }
    }

    type SubstitutedDomain = (
        SymbolHandle,
        String,
        Vec<TypeReferenceHandle>,
        language_semantics::SemanticDomainId,
    );

    fn substituted_parameter_domains(
        source: &dyn ::validation::CheckingInput,
        authored: TypeReferenceHandle,
        program: &dyn ::validation::CheckingInput,
        applied: TypeReferenceHandle,
    ) -> Vec<SubstitutedDomain> {
        let mut authored = validation::parameter_domain_constraints(source, authored, &[]);
        validation::parameter_domain_constraints(program, applied, &[])
            .into_iter()
            .filter(|(domain, ..)| domain.is_valid())
            .filter(|(domain, ..)| {
                let Some(position) = authored
                    .iter()
                    .position(|(declared, ..)| declared == domain)
                else {
                    return true;
                };
                authored.swap_remove(position);
                false
            })
            .collect()
    }

    fn restate_substituted_parameter_domains(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        source_states: &[typed_trees::state::State],
        cloned: &typed_trees::machine::Machine,
    ) {
        let states = program.machine_states(cloned).to_vec();
        for (index, (source_state, mut state)) in source_states.iter().zip(states).enumerate() {
            let source_parameters = source
                .unwrap_or(program)
                .state_parameters(source_state)
                .to_vec();
            let parameters = program.state_parameters(&state).to_vec();
            let mut restated = false;
            for (source_parameter, parameter) in source_parameters.iter().zip(&parameters) {
                for (domain, domain_name, arguments, semantic_domain) in
                    substituted_parameter_domains(
                        source.unwrap_or(program),
                        source_parameter.type_reference,
                        program,
                        parameter.type_reference,
                    )
                {
                    let contract = program.parameter_domain_membership_contract(
                        parameter.symbol,
                        parameter.name.clone(),
                        domain,
                        &domain_name,
                        arguments,
                        semantic_domain,
                    );
                    program.push_state_contract(&mut state, contract);
                    restated = true;
                }
            }
            if restated {
                program.machine_states_mut(cloned)[index] = state;
            }
        }
    }

    fn bound_type_displays(
        bindings: &[Option<TypeReferenceHandle>],
        source: Option<&dyn ::validation::CheckingInput>,
        program: &dyn ::validation::CheckingInput,
    ) -> Vec<String> {
        bindings
            .iter()
            .map(|binding| {
                source
                    .unwrap_or(program)
                    .display_type_reference(binding.expect("complete specialization"))
            })
            .collect()
    }

    fn bound_type_identities(
        bindings: &[Option<TypeReferenceHandle>],
        source: Option<&dyn ::validation::CheckingInput>,
        program: &dyn ::validation::CheckingInput,
    ) -> Vec<String> {
        bindings
            .iter()
            .map(|binding| {
                source
                    .unwrap_or(program)
                    .normalized_type_identity_text(binding.expect("complete specialization"))
            })
            .collect()
    }

    pub(crate) fn clone_specialized_machine(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        candidate: &Candidate,
        ordinal: usize,
        normalized_template_identity: String,
        accepted_template_commitment: Option<String>,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Vec<Diagnostic>> {
        if let Some(states) =
            crate::monomorphization::accessor_primitives::select(source, program, candidate)?
        {
            return Ok(states);
        }
        crate::monomorphization::target_families::clone_specialization(
            source,
            program,
            candidate,
            ordinal,
            normalized_template_identity,
            accepted_template_commitment,
        )
    }

    pub(crate) fn clone_single_specialized_machine(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        candidate: &Candidate,
        ordinal: usize,
        normalized_template_identity: String,
        accepted_template_commitment: Option<String>,
        capture: Option<(
            &typed_trees::typed_trees::ClosedConformanceApplication,
            typed_trees::static_applications::StaticMachineApplicationHandle,
        )>,
    ) -> Result<Vec<(SymbolHandle, SymbolHandle)>, Vec<Diagnostic>> {
        let conformance_applications = candidate
            .evidence_bindings
            .iter()
            .map(|binding| {
                validation::conformance_applications::close_conformance_application(
                    source.unwrap_or(program),
                    binding.as_ref().expect("complete specialization"),
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|diagnostic| vec![diagnostic])?;
        let source_machine =
            source.unwrap_or(program).machines()[candidate.template.machine_index].clone();
        let predicates_closed = {
            let input = source.unwrap_or(program);
            let substitutions = candidate
                .template
                .type_parameters
                .iter()
                .zip(&candidate.type_bindings)
                .filter_map(|((formal, _), actual)| actual.map(|actual| (*formal, actual)))
                .chain(
                    candidate
                        .template
                        .const_parameters
                        .iter()
                        .zip(&candidate.const_bindings)
                        .filter_map(|((formal, _, _), actual)| {
                            actual.map(|actual| (*formal, actual))
                        }),
                )
                .collect::<Vec<_>>();
            let complete = true;
            for contract in input.machine_contracts(&source_machine) {
                if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
                    continue;
                }
                for fact in input.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Expression(predicate) = fact else {
                        continue;
                    };
                    if input.applied_static_type_predicate_value(*predicate, &substitutions)
                        == Some(false)
                    {
                        return Err(vec![Diagnostic::error(format!(
                        "machine `{}` requires type predicate does not hold for the supplied arguments",
                        source_machine.name
                    ))
                    .with_source_span(expressions_of(input).source_span(*predicate))]);
                    }
                }
            }
            complete
        };
        let source_states = source
            .unwrap_or(program)
            .machine_states(&source_machine)
            .to_vec();
        let source_owned = source
            .unwrap_or(program)
            .machine_owned_data(&source_machine)
            .to_vec();
        let specialized_attached_data =
            specialized_attached_data(source.unwrap_or(program), candidate, &source_machine);
        let type_start = program.type_reference_table.type_reference_count() + 1;
        let expression_start = expressions_of(program).expression_count() + 1;

        let type_arguments = bound_type_displays(&candidate.type_bindings, source, program);
        let type_identities = bound_type_identities(&candidate.type_bindings, source, program);
        let const_arguments = bound_type_displays(&candidate.const_bindings, source, program);
        let const_identities = bound_type_identities(&candidate.const_bindings, source, program);
        let machine_paths: Vec<String> = candidate
            .machine_bindings
            .iter()
            .map(|binding| {
                binding
                    .as_ref()
                    .expect("complete specialization")
                    .path
                    .joined_path()
            })
            .collect();
        let evidence_paths = candidate_conformance_fingerprint_arguments(candidate);
        let selection_report_fingerprint = specialization_selection_report_fingerprint(
            &candidate.template.template_name,
            &type_identities,
            &const_identities,
            &machine_paths,
            &evidence_paths,
        );
        let generated_name = format!(
            "{}$specialized${selection_report_fingerprint:016x}${ordinal}${}",
            candidate.template.template_name,
            program.symbols.symbols().len() + 1,
        );
        let machine_symbol = program.symbols.insert_generated_root_from(
            source_machine.symbol,
            SymbolKind::Machine,
            &generated_name,
        );

        let invocation_binders = if capture.is_some() {
            source
                .unwrap_or(program)
                .machine_type_parameters(&source_machine)
                .iter()
                .map(|parameter| {
                    (
                        parameter.symbol,
                        source.unwrap_or(program).symbols.get(parameter.symbol).kind,
                        parameter.name.as_str().to_owned(),
                    )
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let machine_children = program.symbols.insert_generated_children_from(
            machine_symbol,
            source_owned
                .iter()
                .map(|item| (item.symbol, SymbolKind::Field, item.name.as_str()))
                .chain(
                    source_states
                        .iter()
                        .map(|state| (state.symbol, SymbolKind::State, state.name.as_str())),
                )
                .chain(
                    invocation_binders
                        .iter()
                        .map(|(symbol, kind, name)| (*symbol, *kind, name.as_str())),
                )
                .chain(
                    source_machine
                        .conformance_bounds
                        .iter()
                        .filter(|_| capture.is_some())
                        .map(|bound| {
                            (
                                bound.binder,
                                SymbolKind::ConformanceParameter,
                                bound.binder_name.as_str(),
                            )
                        }),
                ),
        );
        let machine_children: Vec<SymbolHandle> =
            symbols::SymbolTableBuilder::child_handles(machine_children).collect();
        let mut next_child = machine_children.into_iter();
        let mut symbol_map = vec![(source_machine.symbol, machine_symbol)];
        for item in &source_owned {
            symbol_map.push((
                item.symbol,
                next_child.next().expect("owned-data clone symbol"),
            ));
        }
        let state_symbols: Vec<(SymbolHandle, SymbolHandle)> = source_states
            .iter()
            .map(|state| (state.symbol, next_child.next().expect("state clone symbol")))
            .collect();
        symbol_map.extend(state_symbols.iter().copied());

        if capture.is_some() {
            for parameter in source
                .unwrap_or(program)
                .machine_type_parameters(&source_machine)
            {
                symbol_map.push((
                    parameter.symbol,
                    next_child.next().expect("method binder clone symbol"),
                ));
            }
            for bound in &source_machine.conformance_bounds {
                let fresh = next_child.next().expect("method evidence clone symbol");
                symbol_map.push((bound.binder, fresh));
                let children = source
                    .unwrap_or(program)
                    .symbols
                    .child_handles(bound.binder)
                    .into_iter()
                    .flatten()
                    .map(|child| {
                        (
                            child,
                            source.unwrap_or(program).symbols.get(child).kind,
                            source.unwrap_or(program).symbols.name(child).to_owned(),
                        )
                    })
                    .collect::<Vec<_>>();
                let copied = program.symbols.insert_generated_children_from(
                    fresh,
                    children
                        .iter()
                        .map(|(child, kind, name)| (*child, *kind, name.as_str())),
                );
                symbol_map.extend(
                    children
                        .iter()
                        .map(|(child, _, _)| *child)
                        .zip(symbols::SymbolTableBuilder::child_handles(copied)),
                );
            }
        }

        let realized_parameters: Vec<(SymbolHandle, Identifier, TypeReferenceHandle)> = candidate
            .template
            .const_parameters
            .iter()
            .zip(&candidate.runtime_value_bindings)
            .filter_map(|((symbol, name, declared_type), runtime)| {
                runtime
                    .as_ref()
                    .map(|_| (*symbol, Identifier::generated(name.clone()), *declared_type))
            })
            .collect();
        let mut state_realized_parameters: Vec<Vec<(SymbolHandle, SymbolHandle)>> = Vec::new();

        for (source_state, (_, state_symbol)) in source_states.iter().zip(state_symbols.iter()) {
            let parameters = source
                .unwrap_or(program)
                .state_parameters(source_state)
                .to_vec();
            let locals: Vec<_> = source
                .unwrap_or(program)
                .statement_table
                .statements(source_state.statement_nodes)
                .iter()
                .filter_map(|statement| match statement {
                    StatementNode::LocalData(local) => Some(local.clone()),
                    _ => None,
                })
                .collect();
            let children =
                program.symbols.insert_generated_children_from(
                    *state_symbol,
                    parameters
                        .iter()
                        .map(|parameter| {
                            (
                                parameter.symbol,
                                SymbolKind::Parameter,
                                parameter.name.as_str(),
                            )
                        })
                        .chain(realized_parameters.iter().map(|(binder, name, _)| {
                            (*binder, SymbolKind::Parameter, name.as_str())
                        }))
                        .chain(
                            locals.iter().map(|local| {
                                (local.symbol, SymbolKind::Local, local.name.as_str())
                            }),
                        ),
                );
            let mut children = symbols::SymbolTableBuilder::child_handles(children);
            for parameter in parameters {
                symbol_map.push((
                    parameter.symbol,
                    children.next().expect("state-parameter clone symbol"),
                ));
            }
            state_realized_parameters.push(
                realized_parameters
                    .iter()
                    .map(|(binder, _, _)| {
                        (
                            *binder,
                            children.next().expect("realized-parameter clone symbol"),
                        )
                    })
                    .collect(),
            );
            for local in locals {
                symbol_map.push((local.symbol, children.next().expect("local clone symbol")));
            }
        }

        let authored_symbols = symbol_map
            .iter()
            .copied()
            .filter(|(symbol, _)| {
                !matches!(
                    source.unwrap_or(program).symbols.get(*symbol).kind,
                    SymbolKind::Machine | SymbolKind::State
                )
            })
            .collect::<Vec<_>>();
        let rebinding = typed_trees::SymbolRebinding::new(&symbol_map, &authored_symbols);

        let mut cloned = source_machine.clone();
        cloned.structural_type_equations_pending = !predicates_closed;
        let remap_progress = |guarantee: &mut language_semantics::TerminationGuarantee| {
            if let language_semantics::TerminationGuarantee::Terminates { premises } = guarantee {
                for premise in premises {
                    premise.subject.root = remapped_symbol(premise.subject.root, &symbol_map);
                    for projection in &mut premise.subject.projections {
                        *projection = remapped_symbol(*projection, &symbol_map);
                    }
                }
            }
        };
        if let TerminationInterface::Published(guarantee) = &mut cloned.termination_plan.interface {
            remap_progress(guarantee);
        }
        remap_progress(&mut cloned.termination_plan.checked_summary);
        cloned.is_public = false;
        cloned.symbol = machine_symbol;
        cloned.name = Identifier::generated(generated_name);
        if let Some((attached_data, attached_data_symbol)) = specialized_attached_data {
            cloned.attached_data = Some(attached_data);
            cloned.attached_data_symbol = attached_data_symbol;
        } else {
            cloned.attached_data = None;
            cloned.attached_data_symbol = SymbolHandle::invalid();
        }
        cloned.type_parameters = if capture.is_some() {
            let parameters = source
                .unwrap_or(program)
                .machine_type_parameters(&source_machine)
                .to_vec();
            let parameters = parameters
                .into_iter()
                .map(|mut parameter| {
                    parameter.symbol = remapped_symbol(parameter.symbol, &symbol_map);
                    parameter.kind = program
                        .copy_callable_parameter_kind(
                            parameter.kind,
                            &authored_symbols,
                            source_machine.symbol,
                        )
                        .map_err(|message| vec![Diagnostic::error(message)])?;
                    Ok(parameter)
                })
                .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            program.data_type_parameters.insert_many(parameters)
        } else {
            HandleSpan::empty()
        };
        cloned.attached_data_application =
            if cloned.attached_data_symbol == source_machine.attached_data_symbol {
                copy_type_reference(
                    source,
                    program,
                    source_machine.attached_data_application,
                    rebinding,
                )?
            } else {
                TypeReferenceHandle::invalid()
            };
        let equations = source
            .unwrap_or(program)
            .declared_type_equations
            .span_or_empty(source_machine.declared_type_equations)
            .to_vec();
        let equations = equations
            .into_iter()
            .map(|equation| {
                Ok(typed_trees::machine::DeclaredTypeEquation {
                    parameter: copy_type_reference(source, program, equation.parameter, rebinding)?,
                    definition: copy_type_reference(
                        source,
                        program,
                        equation.definition,
                        rebinding,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, TypeCopyError>>()?;
        cloned.declared_type_equations = program.declared_type_equations.insert_many(equations);

        if capture.is_none() {
            cloned.conformance_bounds.clear();
        } else {
            for bound in &mut cloned.conformance_bounds {
                bound.binder = remapped_symbol(bound.binder, &symbol_map);
                for argument in &mut bound.arguments {
                    *argument = copy_type_reference(source, program, *argument, rebinding)?;
                }
            }
        }
        cloned.owned_data = HandleSpan::empty();
        cloned.satisfies = HandleSpan::empty();
        cloned.invokes = HandleSpan::empty();
        for mut invocation in source
            .unwrap_or(program)
            .machine_invokes(&source_machine)
            .to_vec()
        {
            invocation.target = match invocation.target {
                AuthoredInvocationTarget::Unresolved => AuthoredInvocationTarget::Unresolved,
                AuthoredInvocationTarget::Parameter { ordinal, symbol } => {
                    AuthoredInvocationTarget::Parameter {
                        ordinal,
                        symbol: remapped_symbol(symbol, &symbol_map),
                    }
                }
                AuthoredInvocationTarget::Service(symbol) => {
                    AuthoredInvocationTarget::Service(remapped_symbol(symbol, &symbol_map))
                }
            };
            program.push_machine_invoke(&mut cloned, invocation);
        }
        let ranking_subjects =
            resolve_machine_witness_subjects(source.unwrap_or(program), &source_machine)
                .unwrap_or_default()
                .into_iter()
                .map(|expression| {
                    let copied = copy_expression(source, program, expression, rebinding);
                    if let Some(realized) = state_realized_parameters.first() {
                        expressions_of_mut(program).remap_symbols_in(copied, realized);
                    }
                    copied
                })
                .collect::<Vec<_>>();
        let ranking_view_arguments =
            resolve_machine_witness_view_arguments(source.unwrap_or(program), &source_machine)
                .unwrap_or_default()
                .into_iter()
                .map(|expression| {
                    let copied = copy_expression(source, program, expression, rebinding);
                    if let Some(realized) = state_realized_parameters.first() {
                        expressions_of_mut(program).remap_symbols_in(copied, realized);
                    }
                    copied
                })
                .collect::<Vec<_>>();
        let ranking_range = source
            .unwrap_or(program)
            .ranking_expression_custody_for(source_machine.symbol)
            .and_then(|custody| custody.rank_range)
            .map(|expression| {
                let copied = copy_expression(source, program, expression, rebinding);
                if let Some(realized) = state_realized_parameters.first() {
                    expressions_of_mut(program).remap_symbols_in(copied, realized);
                }
                copied
            });
        if !ranking_subjects.is_empty()
            || !ranking_view_arguments.is_empty()
            || ranking_range.is_some()
        {
            program.ranking_expression_custody.push(
                typed_trees::ranking::RankingExpressionCustody {
                    machine: machine_symbol,
                    subjects: ranking_subjects,
                    view_arguments: ranking_view_arguments,
                    rank_range: ranking_range,
                },
            );
        }
        cloned.contracts = HandleSpan::empty();
        cloned.states = HandleSpan::empty();

        let owned_symbol_offset = 1;
        for (index, source_item) in source_owned.iter().enumerate() {
            let mut item = source_item.clone();
            item.symbol = symbol_map[owned_symbol_offset + index].1;
            item.type_reference =
                copy_type_reference(source, program, source_item.type_reference, rebinding)?;
            item.initial_value =
                copy_expression(source, program, source_item.initial_value, rebinding);
            program.push_machine_owned_data(&mut cloned, item);
        }
        for conformance in source
            .unwrap_or(program)
            .machine_trait_conformances(&source_machine)
            .to_vec()
        {
            program.push_machine_trait_conformance(&mut cloned, conformance.clone());
        }
        for contract in source
            .unwrap_or(program)
            .machine_contracts(&source_machine)
            .to_vec()
        {
            let contract = copy_signature_contract(source, program, contract.clone(), rebinding)?;
            if let Some(realized) = state_realized_parameters.first() {
                remap_contract_value_subjects(program, &contract, realized);
            }
            program.push_machine_contract(&mut cloned, contract);
        }

        for (state_index, (source_state, (_, fresh_symbol))) in
            source_states.iter().zip(state_symbols.iter()).enumerate()
        {
            let mut state = source_state.clone();
            state.symbol = *fresh_symbol;
            state.parameters = HandleSpan::empty();
            state.contracts = HandleSpan::empty();
            state.return_type =
                copy_type_reference(source, program, source_state.return_type, rebinding)?;
            state.statement_nodes =
                copy_statements(source, program, source_state.statement_nodes, rebinding)?;
            program
                .tables
                .remap_statement_symbols(state.statement_nodes, rebinding);

            let realized = &state_realized_parameters[state_index];
            if !realized.is_empty() {
                for handle in statement_span_handles(state.statement_nodes) {
                    let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                    else {
                        continue;
                    };
                    let applications = StaticArgumentCall::Statement(call).applications();
                    let mut request = call.installed_interrupt_operation.take();
                    if let Some(request) = &mut request {
                        request.invalidate_preparation();
                        expressions_of_mut(program)
                            .remap_static_argument_symbols(&mut request.registrar, realized);
                        expressions_of_mut(program)
                            .remap_static_argument_symbols(&mut request.timer_slot, realized);
                    }
                    let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                    else {
                        unreachable!();
                    };
                    let mut arguments = std::mem::take(&mut call.machine_arguments);
                    for argument in &mut arguments {
                        expressions_of_mut(program)
                            .remap_static_argument_symbols(argument, realized);
                    }
                    let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                    else {
                        unreachable!();
                    };
                    call.machine_arguments = arguments;
                    call.installed_interrupt_operation = request;
                    for application in applications {
                        expressions_of_mut(program).remap_call_static_applications(
                            application,
                            None,
                            realized,
                        );
                    }
                }
                for statement in program
                    .statement_table
                    .statements(state.statement_nodes)
                    .to_vec()
                {
                    let mut roots = Vec::new();
                    collect_statement_expression_trees(program, &statement, &mut roots);
                    for root in roots {
                        expressions_of_mut(program).remap_symbols_in(root, realized);
                    }
                }
            }
            for source_parameter in source
                .unwrap_or(program)
                .state_parameters(source_state)
                .to_vec()
            {
                let mut parameter = source_parameter.clone();
                parameter.symbol = remapped_symbol(parameter.symbol, &symbol_map);
                parameter.type_reference = copy_type_reference(
                    source,
                    program,
                    source_parameter.type_reference,
                    rebinding,
                )?;
                if parameter.relevance.is_erased() && !realized.is_empty() {
                    parameter.type_reference =
                        copy_type_reference(None, program, parameter.type_reference, realized)?;
                }
                program.push_state_parameter(&mut state, parameter);
            }
            if !realized.is_empty() {
                for handle in statement_span_handles(state.statement_nodes) {
                    let Some(local) = program.statement_table.statement(handle).as_local_data()
                    else {
                        continue;
                    };
                    if !local.relevance.is_erased() {
                        continue;
                    }
                    let reference = local.type_reference;
                    let type_reference = copy_type_reference(None, program, reference, realized)?;
                    if let StatementNode::LocalData(local) =
                        program.statement_table.statement_mut(handle)
                    {
                        local.type_reference = type_reference;
                    }
                }
            }
            for ((_, name, declared_type), (_, realized_symbol)) in realized_parameters
                .iter()
                .zip(state_realized_parameters[state_index].iter())
            {
                let type_reference =
                    copy_type_reference(source, program, *declared_type, rebinding)?;
                program.push_state_parameter(
                    &mut state,
                    typed_trees::signature::StateParameter {
                        symbol: *realized_symbol,
                        name: name.clone(),
                        type_reference,
                        is_const: false,
                        is_mutable: false,
                        relevance: language_core::BindingRelevance::Relevant,
                    },
                );
            }
            rebind_realized_member_symbols(
                program,
                &state,
                &state_realized_parameters[state_index],
            );
            for contract in source
                .unwrap_or(program)
                .state_contracts(source_state)
                .to_vec()
            {
                let contract =
                    copy_signature_contract(source, program, contract.clone(), rebinding)?;
                remap_contract_value_subjects(
                    program,
                    &contract,
                    &state_realized_parameters[state_index],
                );
                program.push_state_contract(&mut state, contract);
            }
            program.push_machine_state(&mut cloned, state);
        }

        let proof_output_calls = source
            .unwrap_or(program)
            .proof_output_calls
            .iter()
            .filter(|call| call.machine_symbol == source_machine.symbol)
            .cloned()
            .collect::<Vec<_>>();
        for mut call in proof_output_calls {
            call.machine_symbol = cloned.symbol;
            call.state_symbol = remapped_symbol(call.state_symbol, &symbol_map);
            call.call = copy_expression(source, program, call.call, rebinding);
            program.proof_output_calls.push(call);
        }
        for state in program.machine_states(&cloned).to_vec() {
            for handle in statement_span_handles(state.statement_nodes) {
                let Some(call) = program.statement_table.statement(handle).as_call() else {
                    continue;
                };
                let mut call = call.clone();
                let applications = StaticArgumentCall::Statement(&call).applications();
                for application in applications {
                    let mut authored_arguments = expressions_of(program)
                        .static_machine_application_arguments(application)
                        .to_vec();
                    copy_static_argument_type_payloads(
                        source,
                        program,
                        &mut authored_arguments,
                        rebinding,
                    )?;
                    expressions_of_mut(program)
                        .static_machine_application_arguments_mut(application)
                        .clone_from_slice(&authored_arguments);
                    copy_application_binding_type_payloads(
                        source,
                        program,
                        application,
                        rebinding,
                    )?;
                }
                copy_static_argument_type_payloads(
                    source,
                    program,
                    &mut call.machine_arguments,
                    rebinding,
                )?;
                for argument in call.request_static_arguments_mut() {
                    copy_static_argument_type_payloads(
                        source,
                        program,
                        std::slice::from_mut(argument),
                        rebinding,
                    )?;
                }
                let StatementNode::Call(destination) =
                    program.statement_table.statement_mut(handle)
                else {
                    unreachable!();
                };
                *destination = call;
            }
        }
        copy_cloned_expression_type_payloads(source, program, expression_start, rebinding)?;
        if let Some((application, supplying_application)) = capture {
            let retained = expressions_of_mut(program)
                .static_machine_application_mut(candidate.binding_application);
            retained.registration_operation = application.declaration;
            retained.binding_declaration = application.declaration;
            let states = program
                .captured_machine_states
                .insert_many(state_symbols.iter().map(|(template, instance)| {
                    typed_trees::typed_trees::CapturedMachineState {
                        template: *template,
                        instance: *instance,
                    }
                }));
            program.captured_machine_applications.insert(
                typed_trees::typed_trees::CapturedMachineApplication {
                    template: source_machine.symbol,
                    instance: cloned.symbol,
                    binding_application: candidate.binding_application,
                    supplying_application,
                    conformance_application: application.clone(),
                    normalized_template_identity: normalized_template_identity.clone(),
                    accepted_template_commitment: accepted_template_commitment.clone(),
                    states,
                    parameters: source
                        .unwrap_or(program)
                        .machine_type_parameters(&source_machine)
                        .iter()
                        .map(|parameter| parameter.symbol)
                        .chain(
                            source_machine
                                .conformance_bounds
                                .iter()
                                .map(|bound| bound.binder),
                        )
                        .map(|parameter| (parameter, remapped_symbol(parameter, &symbol_map)))
                        .collect(),
                },
            );
        }
        if let Some((application, _)) =
            capture.filter(|(application, _)| !application.lifetime_arguments.is_empty())
        {
            let selecting = program
                .conformance_by_symbol(application.declaration)
                .ok_or_else(|| failed("capture application lost its selecting declaration"))?;
            // The call occurrence and its captured implementation can have
            // different region suppliers. Keep the checked capture's scope.
            let scope = application
                .trait_scope
                .scope(program)
                .ok_or_else(|| failed("capture application has no complete supplying scope"))?;
            let owner = scope.argument_owner;
            if !owner.is_valid()
                || selecting.trait_symbol != application.trait_definition
                || scope.declaration.symbol != application.trait_definition
                || program.conformance_type_parameters(selecting).len()
                    != application.arguments.len()
                || selecting.lifetime_parameters.len() != application.lifetime_arguments.len()
            {
                return Err(failed(
                    "capture application changed its complete supplying scope",
                ));
            }
            let environment = scope.environment();
            let mut bound_applications = Vec::new();
            for (index, bound) in cloned.conformance_bounds.iter().enumerate() {
                if bound.lifetime_arguments.is_empty() {
                    continue;
                }
                let arguments = program
                    .type_reference_table
                    .insert_type_reference_handles(bound.arguments.iter().copied());
                let reference = program
                    .type_reference_table
                    .insert(TypeReferenceNode::Generic {
                        base_symbol: bound.trait_symbol,
                        base_name: bound.trait_name.clone(),
                        lifetime_arguments: bound.lifetime_arguments.clone(),
                        arguments,
                    });
                let source_bound = &source_machine.conformance_bounds[index];
                let source_owner = source
                    .unwrap_or(program)
                    .symbols
                    .get(source_bound.binder)
                    .parent;
                program
                    .type_reference_table
                    .retain_operand_lifetime_owner(reference, source_owner)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
                bound_applications.push((index, reference));
            }
            let references = program
                .type_reference_table
                .references_from(type_start)
                .map(|(handle, _)| handle)
                .collect::<Vec<_>>();
            for reference in references {
                let applied = program
                .apply_captured_callable_type(
                    reference,
                    &environment,
                    &source_machine.lifetime_parameters,
                    source_machine.symbol,
                )
                .map_err(|message| {
                    vec![Diagnostic::error(format!(
                        "captured region application for `{}` type `{}` supplied by `{}` (copied region owner `{}`): {message}",
                        source_machine.name, program.display_type_reference(reference),
                        program.symbols.display_path(owner, "::"),
                        program.symbols.display_path(program.type_reference_table.operand_lifetime_owner(reference), "::")
                    ))]
                })?;
                program
                    .type_reference_table
                    .substitute_applied_from(reference, applied)
                    .map_err(|message| vec![Diagnostic::error(message)])?;
            }
            for (index, reference) in bound_applications {
                let TypeReferenceNode::Generic {
                    base_symbol,
                    lifetime_arguments,
                    arguments,
                    ..
                } = program.type_reference_table.type_reference(reference)
                else {
                    return Err(failed("captured evidence bound lost its trait application"));
                };
                let bound = &mut cloned.conformance_bounds[index];
                if *base_symbol != bound.trait_symbol {
                    return Err(failed(
                        "captured evidence bound changed its trait declaration",
                    ));
                }
                bound.lifetime_arguments = lifetime_arguments.clone();
                bound.arguments = program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .to_vec();
            }
        }
        let cloned_expression_roots = cloned_expression_roots(program, &cloned);
        const_values::substitute(
            program,
            candidate,
            Some(expression_start),
            &cloned_expression_roots,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        substitute_cloned_type_parameters(
            source,
            program,
            candidate,
            type_start,
            expression_start,
            std::slice::from_ref(&cloned.states),
            cloned.symbol,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        restate_substituted_parameter_domains(source, program, &source_states, &cloned);
        rebind_state_value_parameters(program, &cloned, &state_realized_parameters);
        reject_runtime_bound_static_occurrences(program, candidate, &cloned)
            .map_err(|diagnostic| vec![diagnostic])?;
        let state_transition_subjects: Vec<Vec<(Identifier, SymbolHandle)>> =
            state_realized_parameters
                .iter()
                .map(|realized| {
                    realized
                        .iter()
                        .zip(realized_parameters.iter())
                        .map(|((_, symbol), (_, name, _))| (name.clone(), *symbol))
                        .collect()
                })
                .collect();
        rewrite_cloned_calls(
            source,
            program,
            candidate,
            &state_symbols,
            &state_transition_subjects,
            expression_start,
            cloned.states,
        )?;
        drop_refuted_closed_transition_arms(program, candidate, &cloned);
        let instance_symbol = cloned.symbol;
        let authored_service_reach_rows = source
            .unwrap_or(program)
            .authored_service_reach_rows_for(source_machine.symbol)
            .map(|row| typed_trees::signature::AuthoredServiceReachRow {
                owner: instance_symbol,
                keyword_source_spans: row.keyword_source_spans.clone(),
                targets: row
                    .targets
                    .iter()
                    .map(|target| AuthoredServiceReachTarget {
                        service: remapped_symbol(target.service, &symbol_map),
                        source_span: target.source_span,
                    })
                    .collect(),
                installation_bound: row.installation_bound,
            })
            .collect::<Vec<_>>();
        program
            .authored_service_reach_rows
            .extend(authored_service_reach_rows);
        let evidence_forwardings = source
            .unwrap_or(program)
            .evidence_forwardings
            .iter()
            .filter(|forwarding| forwarding.machine_symbol == source_machine.symbol)
            .map(|forwarding| {
                let mut forwarding = forwarding.clone();
                forwarding.machine_symbol = instance_symbol;
                forwarding.state_symbol = remapped_symbol(forwarding.state_symbol, &symbol_map);
                forwarding.source_conformance = forwarding
                    .source_conformance
                    .map(|symbol| remapped_symbol(symbol, &symbol_map));
                forwarding
            })
            .collect::<Vec<_>>();
        program.evidence_forwardings.extend(evidence_forwardings);
        program.push_machine(cloned);
        let instance =
            typed_trees::references::DeclarationSelection::from_symbols([instance_symbol]);
        let operator_realizations =
            closed_operator_realizations_for_machine(program, instance_symbol)
                .map_err(|diagnostic| vec![diagnostic])?;
        let binding_application = if capture.is_some() {
            candidate.binding_application
        } else {
            crate::monomorphization::applications::retain_instance(
                source,
                program,
                candidate,
                instance_symbol,
            )?
        };
        if capture.is_none() {
            program.machine_specializations.push(MachineSpecialization {
                template: candidate.template.template_symbol,
                instance: instance_symbol,
                template_parameters: source_machine.type_parameters,
                binding_application,
                type_arguments,
                const_arguments,
                type_argument_identities: type_identities,
                const_argument_identities: const_identities,
                machine_arguments: candidate
                    .machine_bindings
                    .iter()
                    .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                    .collect(),
                conformance_arguments: candidate
                    .evidence_bindings
                    .iter()
                    .map(|binding| binding.as_ref().expect("complete specialization").symbol)
                    .collect(),
                conformance_applications,
                operator_realizations,
                normalized_template_identity,
                accepted_template_commitment,
            });
        }

        validation::resolve_selected_dynamic_call_targets(program, &instance)?;

        crate::monomorphization::finalize_domain_instance_identities(program, &instance)
            .map_err(|diagnostic| vec![diagnostic])?;
        Ok(state_symbols)
    }

    pub(crate) fn copy_statements<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        statements: HandleSpan<StatementNode>,
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<HandleSpan<StatementNode>, TypeCopyError> {
        let symbols = symbols.into();
        let mut expressions = Vec::new();
        let copied = if let Some(source) = source {
            program.tables.copy_statement_nodes_with_type_expressions(
                &source.typed_program().tables,
                statements,
                &mut expressions,
            )
        } else {
            let mut staging = typed_trees::typed_trees::TypedTreeTables::default();
            let mut staged_expressions = Vec::new();
            let staged = staging.copy_statement_nodes_with_type_expressions(
                &program.tables,
                statements,
                &mut staged_expressions,
            );
            let copied = program.tables.copy_statement_nodes_with_type_expressions(
                &staging,
                staged,
                &mut expressions,
            );
            constant_roots::compose_type_copies(&staged_expressions, &mut expressions)?;
            copied
        };
        constant_roots::copy_type_contexts(source, program, &expressions, symbols)?;
        Ok(copied)
    }

    pub(crate) fn copy_expression<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        expression: ExpressionHandle,
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> ExpressionHandle {
        if !expression.is_valid() {
            return ExpressionHandle::invalid();
        }
        let copied = match source {
            Some(source) => {
                expressions_of_mut(program).copy_from(expressions_of(source), expression)
            }
            None => expressions_of_mut(program).insert_copy(expression),
        };
        expressions_of_mut(program).remap_symbols_in(copied, symbols);
        copied
    }

    pub(crate) fn copy_cloned_expression_type_payloads<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        expression_start: usize,
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<(), TypeCopyError> {
        let symbols = symbols.into();
        let query_carriers = expressions_of(program)
            .iter_expressions_from(expression_start)
            .filter(|(_, expression)| {
                matches!(expression, ExpressionNode::Call(call)
            if call.native_code_layout.is_some() || call.layout_query.is_some())
            })
            .map(|(handle, _)| {
                (
                    handle,
                    expressions_of(program).constructed_value_type_reference(handle),
                )
            })
            .collect::<Vec<_>>();
        for (expression, carrier) in query_carriers {
            let carrier = copy_type_reference(source, program, carrier, symbols)?;
            expressions_of_mut(program).set_constructed_value_type_reference(expression, carrier);
        }
        let constructors = expressions_of(program)
            .iter_expressions_from(expression_start)
            .filter_map(|(handle, expression)| match expression {
                ExpressionNode::StructLiteral(literal) if literal.application_type.is_valid() => {
                    Some((handle, literal.application_type))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for (handle, application) in constructors {
            let application = copy_type_reference(source, program, application, symbols)?;
            if let ExpressionNode::StructLiteral(literal) =
                expressions_of_mut(program).expression_mut(handle)
            {
                literal.application_type = application;
            }
        }
        let index_payloads = expressions_of(program)
            .iter_expressions_from(expression_start)
            .filter_map(|(handle, _)| {
                expressions_of(program)
                    .index_expression_authority(handle)
                    .cloned()
                    .map(|authority| (handle, authority))
            })
            .collect::<Vec<_>>();
        for (handle, mut authority) in index_payloads {
            if let Some(normalization) = &mut authority.normalization {
                normalization.index_type =
                    copy_type_reference(source, program, normalization.index_type, symbols)?;
            }
            if let Some(operation) = &mut authority.operation {
                operation.operand_type =
                    copy_type_reference(source, program, operation.operand_type, symbols)?;
                if let Some(application) = &mut operation.application {
                    copy_static_argument_type_payloads(
                        source,
                        program,
                        std::slice::from_mut(application),
                        symbols,
                    )?;
                }
            }
            *expressions_of_mut(program).index_expression_authority_mut(handle) = authority;
        }
        let calls = expressions_of(program)
            .iter_expressions_from(expression_start)
            .filter_map(|(handle, expression)| match expression {
                ExpressionNode::Call(call) => Some((
                    handle,
                    call.clone(),
                    StaticArgumentCall::Expression(handle, call).applications(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        for (handle, mut call, applications) in calls {
            call.requirement_application =
                copy_type_reference(source, program, call.requirement_application, symbols)?;
            call.exact_coercion_target =
                copy_type_reference(source, program, call.exact_coercion_target, symbols)?;
            for application in applications {
                let mut authored_arguments = expressions_of(program)
                    .static_machine_application_arguments(application)
                    .to_vec();
                copy_static_argument_type_payloads(
                    source,
                    program,
                    &mut authored_arguments,
                    symbols,
                )?;
                expressions_of_mut(program)
                    .static_machine_application_arguments_mut(application)
                    .clone_from_slice(&authored_arguments);
                copy_application_binding_type_payloads(source, program, application, symbols)?;
            }
            copy_static_argument_type_payloads(
                source,
                program,
                &mut call.machine_arguments,
                symbols,
            )?;
            for argument in call.request_static_arguments_mut() {
                copy_static_argument_type_payloads(
                    source,
                    program,
                    std::slice::from_mut(argument),
                    symbols,
                )?;
            }
            let ExpressionNode::Call(destination) =
                expressions_of_mut(program).expression_mut(handle)
            else {
                unreachable!();
            };
            *destination = call;
        }
        let cast_payloads = expressions_of(program)
            .iter_expressions_from(expression_start)
            .filter_map(|(handle, expression)| {
                let ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                Some((
                    handle,
                    cast.target_type,
                    cast.result_type,
                    cast.semantic_domain_arguments,
                ))
            })
            .collect::<Vec<_>>();
        for (handle, target_type, result_type, arguments) in cast_payloads {
            let target_type = copy_type_reference(source, program, target_type, symbols)?;
            let result_type = copy_type_reference(source, program, result_type, symbols)?;
            let copied_arguments = source
                .unwrap_or(program)
                .type_reference_table
                .type_reference_handles(arguments)
                .to_vec()
                .into_iter()
                .map(|argument| copy_type_reference(source, program, argument, symbols))
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = program
                .type_reference_table
                .insert_type_reference_handles(copied_arguments);
            let ExpressionNode::Cast(cast) = expressions_of_mut(program).expression_mut(handle)
            else {
                unreachable!("collected cast changed kind")
            };
            cast.target_type = target_type;
            cast.result_type = result_type;
            cast.semantic_domain_arguments = arguments;
        }

        let zero_values = expressions_of(program)
            .iter_expressions_from(expression_start)
            .filter_map(|(handle, expression)| {
                let (ExpressionNode::ZeroValue(type_reference)
                | ExpressionNode::TypeExpression(type_reference)) = expression
                else {
                    return None;
                };
                Some((handle, *type_reference))
            })
            .collect::<Vec<_>>();
        for (handle, type_reference) in zero_values {
            let type_reference = copy_type_reference(source, program, type_reference, symbols)?;
            let (ExpressionNode::ZeroValue(current) | ExpressionNode::TypeExpression(current)) =
                expressions_of_mut(program).expression_mut(handle)
            else {
                unreachable!("collected zero-value expression changed kind")
            };
            *current = type_reference;
        }
        Ok(())
    }

    fn copy_application_binding_type_payloads<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<(), TypeCopyError> {
        let symbols = symbols.into();
        let mut selected = expressions_of(program)
            .static_machine_application_selected_machines(application)
            .to_vec();
        copy_static_argument_type_payloads(source, program, &mut selected, symbols)?;
        expressions_of_mut(program)
            .static_machine_application_selected_machines_mut(application)
            .clone_from_slice(&selected);
        let mut bindings = expressions_of(program)
            .static_machine_application_bindings(application)
            .to_vec();
        copy_binding_type_references(source, program, &mut bindings, symbols)?;
        expressions_of_mut(program)
            .static_machine_application_bindings_mut(application)
            .clone_from_slice(&bindings);
        Ok(())
    }

    pub(crate) fn copy_binding_type_payloads<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        bindings: &mut [typed_trees::static_applications::StaticMachineBinding],
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<(), TypeCopyError> {
        let symbols = symbols.into();
        copy_binding_type_references(source, program, bindings, symbols)?;
        for binding in bindings {
            if let typed_trees::static_applications::StaticMachineBindingValue::RuntimeValue {
                expression,
                ..
            } = &mut binding.value
            {
                *expression = copy_expression(source, program, *expression, symbols);
            }
        }
        Ok(())
    }

    fn copy_binding_type_references<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        bindings: &mut [typed_trees::static_applications::StaticMachineBinding],
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<(), TypeCopyError> {
        let symbols = symbols.into();
        use typed_trees::static_applications::StaticMachineBindingValue;
        for binding in bindings {
            match &mut binding.value {
                StaticMachineBindingValue::Type(reference)
                | StaticMachineBindingValue::Const(reference) => {
                    *reference = copy_type_reference(source, program, *reference, symbols)?;
                }
                StaticMachineBindingValue::RuntimeValue {
                    type_reference,
                    subject,
                    ..
                } => {
                    *type_reference =
                        copy_type_reference(source, program, *type_reference, symbols)?;
                    if let Some((_, replacement)) =
                        symbols.iter().find(|(original, _)| *original == *subject)
                    {
                        *subject = *replacement;
                    }
                }
                StaticMachineBindingValue::Machine(argument)
                | StaticMachineBindingValue::Conformance(argument) => {
                    copy_static_argument_type_payloads(
                        source,
                        program,
                        std::slice::from_mut(argument),
                        symbols,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn copy_static_argument_type_payloads<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        arguments: &mut [typed_trees::expression::StaticMachineArgument],
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<(), TypeCopyError> {
        let symbols = symbols.into();
        for argument in arguments {
            if argument.type_reference.is_valid() {
                argument.type_reference =
                    copy_type_reference(source, program, argument.type_reference, symbols)?;
            }
            if let Some(application) = &mut argument.application {
                copy_static_argument_type_payloads(
                    source,
                    program,
                    &mut application.arguments,
                    symbols,
                )?;
            }
        }
        Ok(())
    }

    pub(crate) fn copy_type_reference<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        type_reference: TypeReferenceHandle,
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<TypeReferenceHandle, TypeCopyError> {
        let symbols = symbols.into();
        if !type_reference.is_valid() {
            return Ok(TypeReferenceHandle::invalid());
        }
        let mut expressions = Vec::new();
        let copied = if let Some(source) = source {
            let tables = &mut program.tables;
            tables.type_reference_table.copy_with_expressions(
                &source.type_reference_table,
                expressions_of(source),
                &mut tables.expression_table,
                type_reference,
                &mut expressions,
            )
        } else {
            let mut staging = typed_trees::typed_trees::TypedTreeTables::default();
            let mut staged_expressions = Vec::new();
            let staged = staging.type_reference_table.copy_with_expressions(
                &program.type_reference_table,
                expressions_of(program),
                &mut staging.expression_table,
                type_reference,
                &mut staged_expressions,
            );
            let tables = &mut program.tables;
            let copied = tables.type_reference_table.copy_with_expressions(
                &staging.type_reference_table,
                &staging.expression_table,
                &mut tables.expression_table,
                staged,
                &mut expressions,
            );
            constant_roots::compose_type_copies(&staged_expressions, &mut expressions)?;
            copied
        };
        {
            let tables = &mut program.tables;
            tables.type_reference_table.remap_symbols_in(
                copied,
                &mut tables.expression_table,
                symbols,
            );
        }
        constant_roots::copy_type_contexts(source, program, &expressions, symbols)?;
        Ok(copied)
    }

    pub(crate) fn copy_signature_contract<'scope>(
        source: Option<&dyn ::validation::CheckingInput>,
        program: &mut TypedTrees,
        contract: typed_trees::signature::SignatureContract,
        symbols: impl Into<typed_trees::SymbolRebinding<'scope>>,
    ) -> Result<typed_trees::signature::SignatureContract, Vec<Diagnostic>> {
        let symbols = symbols.into();
        let original_facts = contract.facts;
        let mut copied = contract;
        copied.facts = HandleSpan::empty();
        for (offset, fact) in source
            .unwrap_or(program)
            .proof_facts
            .span_or_empty(original_facts)
            .to_vec()
            .iter()
            .enumerate()
        {
            let source_fact = arena::Handle::from_parts(
                original_facts
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(offset).expect("proof fact offset overflow"))
                    .expect("proof fact source handle overflow"),
                original_facts.start().generation(),
            );
            let source_span = source
                .unwrap_or(program)
                .proof_fact_source_span(source_fact);
            let fact = match fact {
                ProofFact::Expression(expression) => ProofFact::Expression(
                    constant_roots::copy_expression(source, program, *expression, symbols)?,
                ),
                ProofFact::Membership(membership) => {
                    let source_arguments = source
                        .unwrap_or(program)
                        .type_reference_table
                        .type_reference_handles(membership.domain_arguments)
                        .to_vec();
                    let domain_arguments =
                        if source_arguments.len() != membership.domain_arguments.len() {
                            arena::HandleSpan::from_parts(
                                arena::Handle::invalid(),
                                membership.domain_arguments.count(),
                            )
                        } else {
                            let arguments = source_arguments
                                .iter()
                                .map(|argument| {
                                    copy_type_reference(source, program, *argument, symbols)
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            program
                                .type_reference_table
                                .insert_type_reference_handles(arguments)
                        };
                    ProofFact::Membership(typed_trees::domain::ProofMembershipFact {
                        value: constant_roots::copy_expression(
                            source,
                            program,
                            membership.value,
                            symbols,
                        )?,
                        domain: {
                            let members = source
                                .unwrap_or(program)
                                .domain_path_members(membership.domain)
                                .to_vec();
                            program.domain_path_members.insert_many(members)
                        },
                        domain_symbol: remapped_symbol(membership.domain_symbol, &symbols),
                        domain_arguments,
                        semantic_domain: membership.semantic_domain,
                        authored_domain_selection: membership.authored_domain_selection,
                    })
                }
                ProofFact::Proposition(application) => {
                    let arguments = expressions_of(source.unwrap_or(program))
                        .expression_handles(application.arguments)
                        .to_vec()
                        .into_iter()
                        .map(|argument| {
                            constant_roots::copy_expression(source, program, argument, symbols)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let arguments =
                        expressions_of_mut(program).insert_expression_handles(arguments);
                    ProofFact::Proposition(typed_trees::proposition::PropositionApplication {
                        proposition: remapped_symbol(application.proposition, &symbols),
                        name: application.name.clone(),
                        binder_arguments: application
                            .binder_arguments
                            .iter()
                            .map(|argument| PropositionBinderArgument {
                                kind: argument.kind,
                                path: argument.path.clone(),
                                const_literal: argument.const_literal.clone(),
                                evidence_projection: argument.evidence_projection.clone(),
                                symbol: remapped_symbol(argument.symbol, symbols.authored()),
                            })
                            .collect::<Vec<_>>()
                            .into_boxed_slice(),
                        arguments,
                    })
                }
            };
            let copied_fact = program.proof_facts.append_to_span(&mut copied.facts, fact);
            if let Some(source_span) = source_span {
                program.set_proof_fact_source_span(copied_fact, source_span);
            }
        }
        Ok(copied)
    }

    pub(crate) fn remap_contract_value_subjects(
        program: &mut TypedTrees,
        contract: &typed_trees::signature::SignatureContract,
        realized: &[(SymbolHandle, SymbolHandle)],
    ) {
        if realized.is_empty() {
            return;
        }
        let mut roots = Vec::new();
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            validation::append_fact_expression_roots(program, fact, &mut roots);
        }
        for root in roots {
            expressions_of_mut(program).remap_symbols_in(root, realized);
        }
    }

    fn rebind_realized_member_symbols(
        program: &mut TypedTrees,
        state: &typed_trees::state::State,
        realized: &[(SymbolHandle, SymbolHandle)],
    ) {
        if realized.is_empty() {
            return;
        }
        let mut roots = Vec::new();
        for statement in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
        {
            collect_statement_expression_trees(program, statement, &mut roots);
        }
        let mut members = Vec::new();
        for root in roots {
            validation::expression_traversal::collect_expression_tree(program, root, &mut members);
        }
        for handle in members {
            let Some(member) = expressions_of(program).member_node(handle) else {
                continue;
            };
            if member.member_symbol.is_valid() || member.case_variant.is_some() {
                continue;
            }
            let receiver = member.receiver;
            let name = member.member.clone();
            let Some(field) =
                realized_receiver_field(program, state, realized, receiver, name.as_str())
            else {
                continue;
            };
            let ExpressionNode::Member(member) = expressions_of_mut(program).expression_mut(handle)
            else {
                unreachable!();
            };
            member.member_symbol = field.symbol;
        }
    }

    fn realized_receiver_field(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        realized: &[(SymbolHandle, SymbolHandle)],
        receiver: ExpressionHandle,
        name: &str,
    ) -> Option<typed_trees::data::DataField> {
        let reference = realized_receiver_type(program, state, realized, receiver, 0)?;
        carrier_field(program, reference, name)
    }

    fn realized_receiver_type(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        realized: &[(SymbolHandle, SymbolHandle)],
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<TypeReferenceHandle> {
        let node = expressions_of(program).expression_at_depth(expression, depth)?;
        match node {
            ExpressionNode::Borrow(inner) => {
                realized_receiver_type(program, state, realized, inner.target, depth + 1)
            }
            ExpressionNode::Name(path) => {
                let symbol = path.symbol;
                if !realized.iter().any(|(_, realized)| *realized == symbol) {
                    return None;
                }
                program
                    .state_parameter_by_symbol(state, symbol)
                    .map(|parameter| parameter.type_reference)
            }
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return None;
                }
                let receiver =
                    realized_receiver_type(program, state, realized, member.receiver, depth + 1)?;
                carrier_field(program, receiver, member.member.as_str())
                    .map(|field| field.type_reference)
            }
            _ => None,
        }
    }

    fn carrier_field(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
        name: &str,
    ) -> Option<typed_trees::data::DataField> {
        let reference = program
            .type_reference_table
            .peeled_type_reference(reference);
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                let mut definitions = program
                    .data_definitions()
                    .iter()
                    .filter(|data| data.symbol == *symbol);
                let definition = definitions.next()?;
                if definitions.next().is_some() {
                    return None;
                }
                let mut fields = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| match member {
                        DataMember::Field(field) if field.name.as_str() == name => Some(field),
                        _ => None,
                    });
                let field = fields.next()?;
                if fields.next().is_some() || !field.symbol.is_valid() {
                    return None;
                }
                Some(field.clone())
            }
            _ => None,
        }
    }
}

pub(crate) mod body_rewriting {

    use crate::tables::{expressions_of, expressions_of_mut};
    use typed_trees::static_applications::StaticMachineBindingValue;
    use validation::collect_statement_expression_trees;
    use validation::runtime_field_subject;

    pub(crate) mod cloned_calls {
        use crate::monomorphization::body_rewriting::evidence_rewrites::{
            evidence_argument_rewrites, evidence_requirement_rewrites, rewrite_index_algebra,
        };
        use crate::monomorphization::body_rewriting::machine_arguments::{
            forwarded_static_argument_rewrites, substitute_forwarded_machine_arguments,
        };
        use crate::monomorphization::body_rewriting::{
            cloned_runtime_call_subjects, insert_subject_name, insert_subject_names,
            insert_subject_operands, runtime_value_operands,
        };
        use crate::monomorphization::selection::state_by_symbol;
        use crate::monomorphization::{
            Candidate, ExpressionHandle, ExpressionNode, Handle, HandleSpan, StatementNode,
            SymbolHandle, TypedTrees,
        };
        use crate::tables::{expressions_of, expressions_of_mut};
        use typed_trees::expression::StaticMachineArgument;
        use typed_trees::references::StaticArgumentCall;

        pub(crate) mod source_calls {
            use crate::monomorphization::{
                CallSite, Candidate, ExpressionHandle, ExpressionNode, HandleSpan,
            };
            use crate::tables::{expressions_of, expressions_of_mut};
            use symbols::{SymbolHandle, SymbolKind};
            use typed_trees::TypedTrees;
            use typed_trees::expression::StaticMachineArgument;
            use typed_trees::references::StaticArgumentCall;
            use typed_trees::statement::StatementNode;
            use typed_trees::static_applications::StaticMachineBindingValue;
            use typed_trees::typed_trees::StaticRequirementDispatch;
            use typed_trees::types::TypeReferenceNode;

            struct CopiedCall {
                target: SymbolHandle,
                machine_parameter: SymbolHandle,
                application: typed_trees::static_applications::StaticMachineApplicationHandle,
                dispatch: Option<StaticRequirementDispatch>,
                arguments: Vec<ExpressionHandle>,
                static_arguments: Vec<StaticMachineArgument>,
            }

            pub(crate) fn restore(
                program: &mut TypedTrees,
                candidate: &Candidate<'_>,
                states: &[(SymbolHandle, SymbolHandle)],
                expression_start: usize,
                copied_states: HandleSpan<typed_trees::state::State>,
            ) -> Result<(), &'static str> {
                let statements = program
                    .machine_states
                    .span_or_empty(copied_states)
                    .iter()
                    .flat_map(|state| state.statement_nodes.handles())
                    .collect::<Vec<_>>();
                let expressions = expressions_of(program)
                    .iter_expressions_from(expression_start)
                    .map(|(handle, _)| handle)
                    .collect::<Vec<_>>();
                for site in statements
                    .into_iter()
                    .map(CallSite::Statement)
                    .chain(expressions.into_iter().map(CallSite::Expression))
                {
                    let Some(call) = snapshot(program, site) else {
                        continue;
                    };
                    if !call.application.is_valid() {
                        continue;
                    }
                    let source = expressions_of(program)
                        .static_machine_application(call.application)
                        .clone();
                    let operation = source.registration_operation;
                    let recursive_entry = states.first().is_some_and(|(entry, _)| {
                        operation == *entry || operation == candidate.template.template_symbol
                    });
                    if !recursive_entry && states.iter().any(|(_, copied)| *copied == call.target) {
                        continue;
                    }
                    let old_target = states
                        .iter()
                        .find(|(_, copied)| *copied == call.target)
                        .map_or(call.target, |(original, _)| *original);
                    let old_target = if recursive_entry
                        && states.first().is_some_and(|(_, entry)| {
                            program.symbols.get(*entry).parent == call.target
                        }) {
                        candidate.template.template_symbol
                    } else {
                        old_target
                    };
                    restore_call(program, site, call, source, old_target)?;
                }
                Ok(())
            }

            pub fn restore_expressions(
                program: &mut TypedTrees,
                expressions: &[ExpressionHandle],
            ) -> Result<(), &'static str> {
                for &expression in expressions {
                    let site = CallSite::Expression(expression);
                    let Some(call) = snapshot(program, site) else {
                        continue;
                    };
                    if !call.application.is_valid() {
                        continue;
                    }
                    let source = expressions_of(program)
                        .static_machine_application(call.application)
                        .clone();
                    if !source.registration_operation.is_valid() {
                        return Err("copied call has no exact authored operation");
                    }
                    let implementation = if let Some(dispatch) = &call.dispatch {
                        if !dispatch.selection_application.is_valid() {
                            return Err(
                                "copied selected call has no source implementation application",
                            );
                        }
                        let selected = expressions_of(program)
                            .static_machine_application(dispatch.selection_application);
                        if selected.registration_operation != dispatch.requirement {
                            return Err(
                                "copied implementation selection belongs to another requirement",
                            );
                        }
                        let selection = selected
                            .conformance_selection
                            .as_ref()
                            .ok_or("copied selected call has no source implementation operand")?;
                        let operand = expressions_of(program)
                            .static_machine_application_arguments(dispatch.selection_application)
                            .get(selection.operand_ordinal as usize)
                            .filter(|operand| operand.names_implementation(&program.symbols))
                            .cloned()
                            .ok_or("copied selected call has no complete implementation operand")?;
                        Some(operand)
                    } else {
                        None
                    };
                    let old_target = call.target;
                    let restored_roster =
                        restore_call(program, site, call, source.clone(), old_target)?;
                    let Some(restored) = snapshot(program, site) else {
                        return Err("copied source call disappeared during restoration");
                    };
                    let arguments = restored_roster
                        .map_or(restored.arguments.clone(), |(count, _)| {
                            restored.arguments[..count].to_vec()
                        });
                    let method_arguments = if implementation.is_some()
                        && restored_roster.is_some_and(|(_, has_parameters)| !has_parameters)
                    {
                        Vec::new()
                    } else {
                        restored.static_arguments
                    };
                    let application = expressions_of_mut(program)
                        .retain_static_machine_application(
                            source.registration_operation,
                            source.target,
                            &method_arguments,
                        );
                    let fresh =
                        expressions_of_mut(program).static_machine_application_mut(application);
                    fresh.input_selections = source.input_selections;
                    fresh.conformance_selection = None;
                    let lifetimes = expressions_of(program)
                        .static_machine_application_lifetimes(restored.application)
                        .to_vec();
                    expressions_of_mut(program).set_static_machine_application_lifetimes(
                        application,
                        source.lifetime_binding_declaration,
                        &lifetimes,
                    );
                    let qualifier = implementation
                        .as_ref()
                        .map(|operand| program.static_operand_type_reference(operand))
                        .transpose()?;
                    let receiver = implementation.map(|operand| {
                        let name = typed_trees::name::Identifier::generated(
                            program.symbols.name(operand.symbol),
                        );
                        crate::monomorphization::insert_subject_name(
                            expressions_of_mut(program),
                            name,
                            operand.symbol,
                        )
                    });
                    let arguments =
                        expressions_of_mut(program).insert_expression_handles(arguments);
                    let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(expression)
                    else {
                        return Err("copied source call lost its expression");
                    };
                    call.authored_machine_application = application;
                    call.target_symbol = source.registration_operation;
                    call.static_requirement_dispatch = None;
                    call.machine_arguments = method_arguments.into();
                    if let Some(qualifier) = qualifier {
                        call.requirement_application = qualifier;
                        call.receiver =
                            receiver.expect("a copied qualifier retains its selected operand");
                    }
                    call.arguments = arguments;
                }
                Ok(())
            }

            fn restore_call(
                program: &mut TypedTrees,
                site: CallSite,
                mut call: CopiedCall,
                source: typed_trees::static_applications::StaticMachineApplication,
                old_target: SymbolHandle,
            ) -> Result<Option<(usize, bool)>, &'static str> {
                let operation = source.registration_operation;
                let original = declaration_entry(program, authored_target(program, old_target)?);
                let placeholder = call.machine_parameter.is_valid()
                    || (operation.is_valid()
                        && matches!(
                            program.symbols.get(operation).kind,
                            SymbolKind::MachineParameter | SymbolKind::ConformanceParameter
                        ))
                    || (operation.is_valid()
                        && program.symbols.get(operation).parent.is_valid()
                        && program
                            .symbols
                            .get(program.symbols.get(operation).parent)
                            .kind
                            == SymbolKind::ConformanceParameter);
                let declaration_target = if let Some(dispatch) = &call.dispatch {
                    if original != dispatch.realization_state {
                        return Err("copied selected call lost its authored realization entry");
                    }
                    dispatch.realization_state
                } else if placeholder {
                    original
                } else {
                    let declared = declaration_entry(program, operation);
                    if original != declared {
                        if old_target == call.target && original == call.target {
                            return Ok(None);
                        }
                        return Err(
                            "copied call's executable belongs to another source declaration",
                        );
                    }
                    declared
                };
                if call.dispatch.is_none() && !placeholder && call.target == declaration_target {
                    return Ok(None);
                }
                let signature = call.dispatch.as_ref().and_then(|dispatch| {
                    program
                        .trait_machine_signature(dispatch.requirement)
                        .map(|(_, signature)| signature)
                });
                let parameters =
                    if let Some(machine) = program.machine_by_symbol(source.binding_declaration) {
                        let parameters = program.machine_callable_parameters(machine);
                        signature.map_or(parameters.clone(), |signature| {
                            program.requirement_callable_parameters(signature, &parameters)
                        })
                    } else if let Some((_, signature)) =
                        program.trait_machine_signature(source.binding_declaration)
                    {
                        program
                            .requirement_invocation_contract_parameters(signature)
                            .to_vec()
                    } else {
                        Vec::new()
                    };
                let bindings = expressions_of(program)
                    .static_machine_application_bindings(call.application)
                    .to_vec();
                let ordinary = if let Some(signature) = signature {
                    program.state_signature_parameters(signature).len()
                } else if let Some((_, signature)) =
                    program.trait_machine_signature(declaration_target)
                {
                    program.state_signature_parameters(signature).len()
                } else if let Some((_, signature)) = program.machine_parameter_signature(operation)
                {
                    program.state_signature_parameters(signature).len()
                } else if let Some(state) = program.state_by_symbol(declaration_target) {
                    program.state_parameters(state).len()
                } else {
                    return Ok(None);
                };
                let runtime = parameters
                    .iter()
                    .filter_map(|parameter| {
                        bindings
                            .iter()
                            .find(|binding| binding.parameter == parameter.symbol)
                            .filter(|binding| {
                                matches!(
                                    binding.value,
                                    StaticMachineBindingValue::RuntimeValue { .. }
                                )
                            })
                    })
                    .collect::<Vec<_>>();
                if call.arguments.len() != ordinary {
                    if call.arguments.len() != ordinary + runtime.len() {
                        return Err(
                            "copied call lost its ordinary and realized Value operand roster",
                        );
                    }
                    for (binding, actual) in runtime.iter().zip(&call.arguments[ordinary..]) {
                        let StaticMachineBindingValue::RuntimeValue {
                            subject,
                            expression,
                            ..
                        } = binding.value
                        else {
                            return Err("copied Value tail has another binding category");
                        };
                        if expression.is_valid() {
                            if expression != *actual {
                                return Err(
                                    "copied Value operand differs from its retained evaluation",
                                );
                            }
                        } else if !subject.is_valid()
                            || !validation::canonical_place_from_expression(program, *actual)
                                .is_some_and(|place| {
                                    place.root.as_symbol() == Some(subject)
                                        && place.segments.is_empty()
                                })
                        {
                            return Err("copied Value operand belongs to another runtime subject");
                        }
                    }
                }
                if (!placeholder || call.dispatch.is_some())
                    && !parameters.is_empty()
                    && !bindings.is_empty()
                {
                    let mut arguments = Vec::with_capacity(parameters.len());
                    for parameter in &parameters {
                        let mut matches = bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.symbol);
                        let binding = matches
                            .next()
                            .ok_or("copied call lost an inferred source operand")?;
                        if matches.next().is_some() {
                            return Err("copied call repeats an inferred source operand");
                        }
                        arguments.push(match &binding.value {
                            StaticMachineBindingValue::Type(reference)
                            | StaticMachineBindingValue::Const(reference) => {
                                program.static_operand_from_type_reference(*reference)?
                            }
                            StaticMachineBindingValue::Machine(argument)
                            | StaticMachineBindingValue::Conformance(argument) => argument.clone(),
                            StaticMachineBindingValue::RuntimeValue { expression, .. }
                                if expression.is_valid() =>
                            {
                                let reference = program
                                    .type_reference_table
                                    .insert(TypeReferenceNode::ConstExpression(*expression));
                                StaticMachineArgument {
                                    type_reference: reference,
                                    ..Default::default()
                                }
                            }
                            StaticMachineBindingValue::RuntimeValue { subject, .. }
                                if subject.is_valid() =>
                            {
                                StaticMachineArgument {
                                    symbol: *subject,
                                    path: Box::from([program.symbols.name(*subject).into()]),
                                    ..Default::default()
                                }
                            }
                            StaticMachineBindingValue::RuntimeValue { .. } => {
                                return Err("copied runtime Value has no retained subject");
                            }
                        });
                    }
                    call.static_arguments = arguments;
                } else {
                    call.static_arguments = expressions_of(program)
                        .static_machine_application_arguments(call.application)
                        .to_vec();
                }
                if let Some(dispatch) = &mut call.dispatch {
                    dispatch.contract_signature = SymbolHandle::invalid();
                    if dispatch.evidence_parameter.is_valid() {
                        call.target = operation;
                        call.dispatch = None;
                    } else {
                        call.target = dispatch.realization_state;
                    }
                } else {
                    call.target = if placeholder {
                        operation
                    } else {
                        declaration_target
                    };
                }
                publish(program, site, call);
                Ok(Some((ordinary, !parameters.is_empty())))
            }

            fn snapshot(program: &TypedTrees, site: CallSite) -> Option<CopiedCall> {
                let (call, arguments, target, machine_parameter, dispatch, static_arguments) =
                    match site {
                        CallSite::Expression(expression) => {
                            let call = expressions_of(program).call_node(expression)?;
                            (
                                StaticArgumentCall::Expression(expression, call),
                                expressions_of(program).expression_handles(call.arguments),
                                call.target_symbol,
                                call.static_machine_parameter,
                                call.static_requirement_dispatch.clone(),
                                call.machine_arguments.to_vec(),
                            )
                        }
                        CallSite::Statement(statement) => {
                            let call = program.statement_table.statement(statement).as_call()?;
                            (
                                StaticArgumentCall::Statement(call),
                                program.statement_table.expression_handles(call.arguments),
                                call.target_symbol,
                                call.static_machine_parameter,
                                call.static_requirement_dispatch.clone(),
                                call.machine_arguments.to_vec(),
                            )
                        }
                    };
                Some(CopiedCall {
                    target,
                    machine_parameter,
                    application: call.application(),
                    dispatch,
                    arguments: arguments.to_vec(),
                    static_arguments,
                })
            }

            fn publish(program: &mut TypedTrees, site: CallSite, restored: CopiedCall) {
                match site {
                    CallSite::Expression(expression) => {
                        let arguments = expressions_of_mut(program)
                            .insert_expression_handles(restored.arguments);
                        if let ExpressionNode::Call(call) =
                            expressions_of_mut(program).expression_mut(expression)
                        {
                            call.target_symbol = restored.target;
                            call.static_requirement_dispatch = restored.dispatch;
                            call.machine_arguments = restored.static_arguments.into();
                            call.arguments = arguments;
                        }
                    }
                    CallSite::Statement(statement) => {
                        let arguments = program
                            .statement_table
                            .insert_expression_handles(restored.arguments);
                        if let StatementNode::Call(call) =
                            program.statement_table.statement_mut(statement)
                        {
                            call.target_symbol = restored.target;
                            call.static_requirement_dispatch = restored.dispatch;
                            call.machine_arguments = restored.static_arguments.into();
                            call.arguments = arguments;
                        }
                    }
                }
            }

            fn declaration_entry(program: &TypedTrees, declaration: SymbolHandle) -> SymbolHandle {
                program
                    .machine_by_symbol(declaration)
                    .and_then(|machine| program.machine_states(machine).first())
                    .map_or(declaration, |entry| entry.symbol)
            }

            fn authored_target(
                program: &TypedTrees,
                mut target: SymbolHandle,
            ) -> Result<SymbolHandle, &'static str> {
                let mut visited = Vec::new();
                loop {
                    if visited.contains(&target) {
                        return Err("copied call has cyclic executable provenance");
                    }
                    visited.push(target);
                    if let Some(closed) = program.closed_requirement_application(target) {
                        target = closed.template;
                        continue;
                    }
                    let declaration = program.machine_by_symbol(target);
                    let Some(machine) =
                        declaration.or_else(|| program.machine_holding_state(target))
                    else {
                        return Ok(target);
                    };
                    let Some(template) = program.executable_application_template(machine.symbol)?
                    else {
                        return Ok(target);
                    };
                    if declaration.is_some() {
                        target = template;
                    } else if let Some((_, capture)) = program
                        .captured_machine_applications
                        .iter()
                        .find(|(_, capture)| capture.instance == machine.symbol)
                    {
                        let mut states = program
                            .captured_machine_states
                            .span_or_empty(capture.states)
                            .iter()
                            .filter(|state| state.instance == target);
                        let template_state = states
                            .next()
                            .ok_or("copied call lost its captured state correspondence")?
                            .template;
                        if states.next().is_some() {
                            return Err("copied call has ambiguous captured state correspondence");
                        }
                        target = template_state;
                    } else {
                        let ordinal = typed_trees::state::state_position(
                            program.machine_states(machine),
                            target,
                        )
                        .ok_or("copied call lost its specialized state position")?;
                        target = program
                            .machine_by_symbol(template)
                            .and_then(|template| program.machine_states(template).get(ordinal))
                            .ok_or("copied call lost its template state")?
                            .symbol;
                    }
                }
            }
        }
        pub use source_calls::restore_expressions as restore_copied_call_sources;

        pub(crate) fn statement_span_handles(
            span: HandleSpan<StatementNode>,
        ) -> Vec<Handle<StatementNode>> {
            span.handles().collect()
        }

        pub(crate) fn rewrite_cloned_calls(
            source: Option<&dyn ::validation::CheckingInput>,
            program: &mut TypedTrees,
            candidate: &Candidate,
            state_symbols: &[(SymbolHandle, SymbolHandle)],
            state_transition_subjects: &[Vec<(typed_trees::name::Identifier, SymbolHandle)>],
            expression_start: usize,
            states: HandleSpan<typed_trees::state::State>,
        ) -> Result<(), Vec<crate::monomorphization::Diagnostic>> {
            source_calls::restore(program, candidate, state_symbols, expression_start, states)
                .map_err(|reason| vec![crate::monomorphization::Diagnostic::error(reason)])?;
            let machine_rewrites: Vec<_> = candidate
                .template
                .machine_parameters
                .iter()
                .zip(candidate.machine_bindings.iter())
                .map(|((parameter, _, _), binding)| {
                    let binding = binding.as_ref().expect("complete specialization");
                    let name = binding
                        .path
                        .last()
                        .cloned()
                        .or_else(|| {
                            state_by_symbol(source.unwrap_or(program), binding.symbol)
                                .map(|state| state.name.clone())
                        })
                        .expect("static machine entry name");
                    (*parameter, binding.symbol, name)
                })
                .collect();
            let selected_carriers: Vec<_> = machine_rewrites
                .iter()
                .filter_map(|(parameter, target, _)| {
                    let declarations = source.unwrap_or(program);
                    let machine = declarations.machine_containing_state(*target)?;
                    if !machine.attached_data_symbol.is_valid() {
                        return None;
                    }
                    Some((
                        *parameter,
                        machine.attached_data_symbol,
                        machine.attached_data.as_ref()?.clone(),
                    ))
                })
                .collect();
            let evidence_target_rewrites =
                evidence_requirement_rewrites(source.unwrap_or(program), candidate);
            let mut target_rewrites = machine_rewrites.clone();
            target_rewrites.extend(
                evidence_target_rewrites
                    .iter()
                    .map(|rewrite| (rewrite.placeholder, rewrite.target, rewrite.name.clone())),
            );
            let mut argument_rewrites = machine_rewrites;
            let evidence_rewrites =
                evidence_argument_rewrites(source.unwrap_or(program), candidate);
            argument_rewrites.extend(evidence_rewrites.iter().map(|rewrite| {
                (
                    rewrite.binder,
                    rewrite.argument.symbol,
                    rewrite.alias.clone(),
                )
            }));
            let static_argument_rewrites =
                forwarded_static_argument_rewrites(source.unwrap_or(program), candidate);
            macro_rules! substitute_application {
                ($application:expr) => {{
                    let mut arguments = expressions_of(program)
                        .static_machine_application_arguments($application)
                        .to_vec();
                    substitute_forwarded_machine_arguments(
                        expressions_of_mut(program),
                        &mut arguments,
                        &static_argument_rewrites,
                        &argument_rewrites,
                    );
                    expressions_of_mut(program)
                        .static_machine_application_arguments_mut($application)
                        .clone_from_slice(&arguments);
                    let mut selected = expressions_of(program)
                        .static_machine_application_selected_machines($application)
                        .to_vec();
                    substitute_forwarded_machine_arguments(
                        expressions_of_mut(program),
                        &mut selected,
                        &static_argument_rewrites,
                        &argument_rewrites,
                    );
                    expressions_of_mut(program)
                        .static_machine_application_selected_machines_mut($application)
                        .clone_from_slice(&selected);
                }};
            }
            for (state_index, state) in program
                .machine_states
                .span_or_empty(states)
                .to_vec()
                .into_iter()
                .enumerate()
            {
                for statement_handle in statement_span_handles(state.statement_nodes) {
                    rewrite_static_machine_transition_targets(
                        program,
                        statement_handle,
                        candidate,
                        &target_rewrites,
                    );
                    rewrite_clone_transition_subjects(
                        program,
                        statement_handle,
                        state_transition_subjects
                            .get(state_index)
                            .map_or(&[], Vec::as_slice),
                        state_symbols,
                    );
                    let StatementNode::Call(snapshot) =
                        program.statement_table.statement(statement_handle).clone()
                    else {
                        continue;
                    };
                    let clone_state_subjects = if state_symbols
                        .iter()
                        .any(|(_, concrete)| *concrete == snapshot.target_symbol)
                    {
                        runtime_value_operands(
                            program,
                            state.symbol,
                            &snapshot
                                .machine_arguments
                                .iter()
                                .cloned()
                                .map(crate::monomorphization::RuntimeValueBinding::Authored)
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        Vec::new()
                    };
                    let clone_state_arguments = (!clone_state_subjects.is_empty()).then(|| {
                        super::insert_arguments_with_subjects(
                            program,
                            snapshot.arguments,
                            &clone_state_subjects,
                        )
                    });
                    let evidence_dispatch = evidence_target_rewrites
                        .iter()
                        .find(|rewrite| rewrite.placeholder == snapshot.target_symbol);
                    let selected_carrier = selected_carriers
                        .iter()
                        .find(|(parameter, _, _)| *parameter == snapshot.target_symbol)
                        .map(|(_, symbol, name)| {
                            let mut members = HandleSpan::empty();
                            program
                                .statement_table
                                .push_name_path_member(&mut members, name.clone());
                            (*symbol, members)
                        });
                    let StatementNode::Call(mut call) =
                        program.statement_table.statement(statement_handle).clone()
                    else {
                        unreachable!();
                    };
                    if candidate
                        .template
                        .machine_parameters
                        .iter()
                        .any(|(parameter, _, _)| *parameter == call.target_symbol)
                    {
                        call.static_machine_parameter = call.target_symbol;
                    }
                    if let Some((_, target, name)) = target_rewrites
                        .iter()
                        .find(|(parameter, _, _)| *parameter == call.target_symbol)
                    {
                        call.target_symbol = *target;
                        call.target = name.clone();
                    }
                    if let Some((symbol, members)) = selected_carrier {
                        call.receiver_root_symbol = symbol;
                        call.receiver_symbol = symbol;
                        call.receiver = members;
                    }
                    for argument in call.request_static_arguments_mut() {
                        substitute_forwarded_machine_arguments(
                            expressions_of_mut(program),
                            std::slice::from_mut(argument),
                            &static_argument_rewrites,
                            &argument_rewrites,
                        );
                    }
                    let erased_application = apply_evidence_dispatch_rewrite(
                        expressions_of_mut(program),
                        call.target_symbol,
                        &mut call.static_requirement_dispatch,
                        &mut call.machine_arguments,
                        &mut call.arguments,
                        evidence_dispatch,
                        &static_argument_rewrites,
                        &argument_rewrites,
                        state_symbols,
                        clone_state_arguments,
                        || {
                            call.receiver_root_symbol = SymbolHandle::invalid();
                            call.receiver_symbol = SymbolHandle::invalid();
                            call.receiver = HandleSpan::empty();
                        },
                    );
                    let mut application = call.authored_machine_application;
                    let target = call.target.clone();
                    let applications = StaticArgumentCall::Statement(&call).applications();
                    *program.statement_table.statement_mut(statement_handle) =
                        StatementNode::Call(call);
                    for application in applications {
                        substitute_application!(application);
                    }
                    if !application.is_valid()
                        && let Some((registration_operation, arguments)) = erased_application
                    {
                        application = expressions_of_mut(program)
                            .retain_static_machine_application(
                                registration_operation,
                                target,
                                &arguments,
                            );
                        let StatementNode::Call(call) =
                            program.statement_table.statement_mut(statement_handle)
                        else {
                            unreachable!();
                        };
                        call.authored_machine_application = application;
                    }
                    if !clone_state_subjects.is_empty() {
                        super::record_runtime_call_operands(
                            program,
                            application,
                            &clone_state_subjects,
                        );
                    }
                }
            }
            let runtime_calls =
                cloned_runtime_call_subjects(program, states, state_symbols, expression_start);
            let handles: Vec<_> = expressions_of(program)
                .iter_expressions_from(expression_start)
                .map(|(handle, _)| handle)
                .collect();
            for handle in handles {
                rewrite_index_algebra(
                    program,
                    handle,
                    &evidence_rewrites,
                    &static_argument_rewrites,
                );
                let (clone_state_arguments, clone_state_subjects): (_, &[ExpressionHandle]) =
                    match expressions_of(program).expression(handle).clone() {
                        ExpressionNode::Call(snapshot)
                            if state_symbols
                                .iter()
                                .any(|(_, concrete)| *concrete == snapshot.target_symbol) =>
                        {
                            let subjects = runtime_calls
                                .binary_search_by_key(
                                    &(handle.arena_index(), handle.generation()),
                                    |(expression, _)| {
                                        (expression.arena_index(), expression.generation())
                                    },
                                )
                                .ok()
                                .map(|ordinal| runtime_calls[ordinal].1.as_slice())
                                .unwrap_or(&[]);
                            let arguments = (!subjects.is_empty()).then(|| {
                                let mut arguments = expressions_of(program)
                                    .expression_handles(snapshot.arguments)
                                    .to_vec();
                                insert_subject_operands(&mut arguments, subjects);
                                expressions_of_mut(program).insert_expression_handles(arguments)
                            });
                            (arguments, subjects)
                        }
                        _ => (None, &[]),
                    };
                let evidence_dispatch = match expressions_of(program).expression(handle) {
                    ExpressionNode::Call(call) => evidence_target_rewrites
                        .iter()
                        .find(|rewrite| rewrite.placeholder == call.target_symbol),
                    _ => None,
                };
                let selected_carrier = match expressions_of(program).expression(handle) {
                    ExpressionNode::Call(call) => selected_carriers
                        .iter()
                        .find(|(parameter, _, _)| *parameter == call.target_symbol)
                        .map(|(_, symbol, name)| (*symbol, name.clone())),
                    _ => None,
                }
                .map(|(symbol, name)| {
                    insert_subject_name(expressions_of_mut(program), name, symbol)
                });
                let ExpressionNode::Call(mut call) =
                    expressions_of(program).expression(handle).clone()
                else {
                    continue;
                };
                for argument in call.request_static_arguments_mut() {
                    substitute_forwarded_machine_arguments(
                        expressions_of_mut(program),
                        std::slice::from_mut(argument),
                        &static_argument_rewrites,
                        &argument_rewrites,
                    );
                }
                if candidate
                    .template
                    .machine_parameters
                    .iter()
                    .any(|(parameter, _, _)| *parameter == call.target_symbol)
                {
                    call.static_machine_parameter = call.target_symbol;
                }
                if let Some((_, target, name)) = target_rewrites
                    .iter()
                    .find(|(parameter, _, _)| *parameter == call.target_symbol)
                {
                    call.target_symbol = *target;
                    call.target = name.clone();
                }
                if let Some(receiver) = selected_carrier {
                    call.receiver = receiver;
                }
                let erased_application = apply_evidence_dispatch_rewrite(
                    expressions_of_mut(program),
                    call.target_symbol,
                    &mut call.static_requirement_dispatch,
                    &mut call.machine_arguments,
                    &mut call.arguments,
                    evidence_dispatch,
                    &static_argument_rewrites,
                    &argument_rewrites,
                    state_symbols,
                    clone_state_arguments,
                    || call.receiver = ExpressionHandle::invalid(),
                );
                let mut application = call.authored_machine_application;
                let target = call.target.clone();
                let applications = StaticArgumentCall::Expression(handle, &call).applications();
                *expressions_of_mut(program).expression_mut(handle) = ExpressionNode::Call(call);
                for application in applications {
                    substitute_application!(application);
                }
                if !application.is_valid()
                    && let Some((registration_operation, arguments)) = erased_application
                {
                    application = expressions_of_mut(program).retain_static_machine_application(
                        registration_operation,
                        target,
                        &arguments,
                    );
                    let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(handle)
                    else {
                        unreachable!();
                    };
                    call.authored_machine_application = application;
                }
                if !clone_state_subjects.is_empty() {
                    super::record_runtime_call_operands(program, application, clone_state_subjects);
                }
            }
            Ok(())
        }

        pub(crate) fn rewrite_static_machine_transition_targets(
            program: &mut TypedTrees,
            statement: typed_trees::statement::StatementHandle,
            candidate: &Candidate,
            rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
        ) {
            let Some(transition) = program.statement_table.statement(statement).as_transition()
            else {
                return;
            };
            for target in [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
            {
                let typed_trees::statement::TransitionTargetNode::Named {
                    path,
                    static_machine_parameter,
                    ..
                } = program.statement_table.transition_target_mut(target)
                else {
                    continue;
                };
                if !candidate
                    .template
                    .machine_parameters
                    .iter()
                    .any(|(parameter, _, _)| *parameter == path.symbol)
                {
                    continue;
                }
                if let Some((parameter, selected, _)) = rewrites
                    .iter()
                    .find(|(parameter, _, _)| *parameter == path.symbol)
                {
                    *static_machine_parameter = *parameter;
                    path.symbol = *selected;
                    if path.head_symbol == *parameter {
                        path.head_symbol = *selected;
                    }
                }
            }
        }

        pub(crate) fn rewrite_clone_transition_subjects(
            program: &mut TypedTrees,
            statement: typed_trees::statement::StatementHandle,
            subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
            state_symbols: &[(SymbolHandle, SymbolHandle)],
        ) {
            if subjects.is_empty() {
                return;
            }
            let Some(transition) = program.statement_table.statement(statement).as_transition()
            else {
                return;
            };
            for target in [transition.target, transition.continuation]
                .into_iter()
                .filter(|handle| handle.is_valid())
            {
                let is_clone_state = matches!(
                    program.statement_table.transition_target(target),
                    typed_trees::statement::TransitionTargetNode::Named { path, .. }
                        if state_symbols
                            .iter()
                            .any(|(_, concrete)| *concrete == path.symbol)
                );
                if !is_clone_state {
                    continue;
                }
                let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target(target)
                else {
                    unreachable!("named transition target checked above");
                };
                let mut arguments = program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec();
                insert_subject_names(expressions_of_mut(program), &mut arguments, subjects);
                let new_arguments = program.statement_table.insert_expression_handles(arguments);
                let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target_mut(target)
                else {
                    unreachable!("named transition target checked above");
                };
                *arguments = new_arguments;
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn apply_evidence_dispatch_rewrite(
            expressions: &mut typed_trees::expression::ExpressionTable,
            target_symbol: SymbolHandle,
            static_requirement_dispatch: &mut Option<
                typed_trees::typed_trees::StaticRequirementDispatch,
            >,
            machine_arguments: &mut Box<[StaticMachineArgument]>,
            arguments: &mut HandleSpan<ExpressionHandle>,
            evidence_dispatch: Option<&super::evidence_rewrites::EvidenceRequirementRewrite>,
            static_argument_rewrites: &[(SymbolHandle, StaticMachineArgument)],
            argument_rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
            state_symbols: &[(SymbolHandle, SymbolHandle)],
            clone_state_arguments: Option<HandleSpan<ExpressionHandle>>,
            mut invalidate_receiver: impl FnMut(),
        ) -> Option<(SymbolHandle, Box<[StaticMachineArgument]>)> {
            if evidence_dispatch.is_some() {
                invalidate_receiver();
            }
            if let Some(rewrite) = evidence_dispatch {
                *static_requirement_dispatch = Some(rewrite.dispatch.clone());
            }
            substitute_forwarded_machine_arguments(
                expressions,
                machine_arguments,
                static_argument_rewrites,
                argument_rewrites,
            );
            if let Some((authored, _)) = state_symbols
                .iter()
                .find(|(_, concrete)| *concrete == target_symbol)
            {
                if let Some(cloned) = clone_state_arguments {
                    *arguments = cloned;
                }
                return Some((*authored, std::mem::take(machine_arguments)));
            }
            None
        }
    }
    pub use cloned_calls::restore_copied_call_sources;
    pub(crate) mod const_substitution {
        use crate::monomorphization::{
            Candidate, ExpressionNode, StatementNode, SymbolHandle, TypeReferenceNode, TypedTrees,
        };
        use crate::tables::{expressions_of, expressions_of_mut};
        use numerics::literals::{IntegerLiteral, IntegerRadix};
        use typed_trees::const_integer::{self, ConstIntegerExpression};
        use typed_trees::expression::{BinaryOperator, ExpressionHandle, StaticMachineArgument};
        use typed_trees::name::Identifier;
        use typed_trees::statement::TransitionTargetNode;
        use typed_trees::types::TypeReferenceHandle;
        use validation::parameter_expression_result_type_reference;

        pub(crate) fn fixed_array_const_replacements(
            program: &dyn ::validation::CheckingInput,
            candidate: &Candidate,
        ) -> Vec<(SymbolHandle, String, usize)> {
            candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .filter_map(|((symbol, name, _), binding)| {
                    let TypeReferenceNode::Named {
                        name: binding_name, ..
                    } = program
                        .type_reference_table
                        .type_reference(binding.expect("complete const specialization"))
                    else {
                        return None;
                    };
                    Some((*symbol, name.clone(), binding_name.as_str().parse().ok()?))
                })
                .collect()
        }

        pub(crate) fn substitute_fixed_array_const_parameters(
            program: &mut TypedTrees,
            replacements: &[(SymbolHandle, String, usize)],
            type_start: Option<usize>,
        ) {
            if replacements.is_empty() {
                return;
            }
            let updates = match type_start {
                Some(start) => {
                    program
                        .type_reference_table
                        .references_from(start)
                        .filter_map(|(handle, node)| {
                            let TypeReferenceNode::FixedArray {
                                length:
                                    typed_trees::types::FixedArrayLength::ConstParameter {
                                        symbol,
                                        name,
                                    },
                                ..
                            } = node
                            else {
                                return None;
                            };
                            replacements
                                .iter()
                                .find(|(parameter_symbol, parameter_name, _)| {
                                    parameter_symbol == symbol
                                        || (!parameter_symbol.is_valid()
                                            && !symbol.is_valid()
                                            && parameter_name == name.as_str())
                                })
                                .map(|(_, _, value)| (handle, *value))
                        })
                        .collect::<Vec<_>>()
                }
                None => program
                    .type_reference_table
                    .fixed_array_lengths()
                    .filter_map(|(handle, length)| {
                        let typed_trees::types::FixedArrayLength::ConstParameter { symbol, name } =
                            length
                        else {
                            return None;
                        };
                        replacements
                            .iter()
                            .find(|(parameter_symbol, parameter_name, _)| {
                                parameter_symbol == symbol
                                    || (!parameter_symbol.is_valid()
                                        && !symbol.is_valid()
                                        && parameter_name == name.as_str())
                            })
                            .map(|(_, _, value)| (handle, *value))
                    })
                    .collect::<Vec<_>>(),
            };
            for (handle, value) in updates {
                program
                    .type_reference_table
                    .set_fixed_array_length(handle, value);
            }
        }

        pub(crate) fn substitute_const_index_expression_parameters(
            source: Option<&dyn ::validation::CheckingInput>,
            program: &mut TypedTrees,
            candidate: &Candidate,
            type_start: usize,
        ) -> Result<(), String> {
            if candidate.template.const_parameters.is_empty() {
                return Ok(());
            }
            let mut replacements = Vec::new();
            for (index, ((parameter, _, carrier), binding)) in candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .enumerate()
            {
                if candidate.runtime_value_bindings[index].is_some() {
                    continue;
                }
                let binding = crate::monomorphization::copy_type_reference(
                    source,
                    program,
                    binding.expect("complete const specialization"),
                    &[],
                )?;
                let carrier = if let Some((index, _)) = candidate
            .template
            .type_parameters
            .iter()
            .enumerate()
            .find(|(_, (symbol, _))| {
                matches!(source.unwrap_or(program).type_reference_table.type_reference(*carrier),
                    TypeReferenceNode::Named { symbol: actual, .. } if actual == symbol)
            }) {
            crate::monomorphization::copy_type_reference(
                source,
                program,
                candidate.type_bindings[index].expect("complete type specialization"),
                &[],
            )?
        } else {
            crate::monomorphization::copy_type_reference(source, program, *carrier, &[])?
        };
                replacements.push((*parameter, binding, carrier));
            }
            let roots = program
                .type_reference_table
                .references_from(type_start)
                .filter_map(|(_, node)| match node {
                    TypeReferenceNode::ConstExpression(expression) => Some(*expression),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let mut expressions = Vec::new();
            for root in roots {
                validation::collect_expression_tree(program, root, &mut expressions);
            }
            for expression in expressions {
                let ExpressionNode::Name(path) = expressions_of(program).expression(expression)
                else {
                    continue;
                };
                let Some((_, binding, carrier)) = replacements
                    .iter()
                    .find(|(symbol, _, _)| path.symbol.is_valid() && path.symbol == *symbol)
                else {
                    continue;
                };
                let value = crate::monomorphization::materialize_static_value(
                    program, *binding, *carrier,
                )
                .ok_or_else(|| {
                    "const index binder has no exact canonical value under its declared carrier"
                        .to_owned()
                })?;
                *expressions_of_mut(program).expression_mut(expression) = value;
            }
            Ok(())
        }

        pub(crate) fn fold_closed_const_machine_arguments(
            program: &mut TypedTrees,
        ) -> Result<(), String> {
            let state_spans = program
                .machines()
                .iter()
                .map(|machine| machine.states)
                .collect::<Vec<_>>();
            fold_closed_const_machine_arguments_in(program, 0, &state_spans)
        }

        pub(crate) fn fold_closed_const_machine_arguments_in(
            program: &mut TypedTrees,
            expression_start: usize,
            state_spans: &[arena::HandleSpan<typed_trees::state::State>],
        ) -> Result<(), String> {
            let expression_handles = expressions_of(program)
                .iter_expressions_from(expression_start)
                .filter_map(|(handle, expression)| {
                    matches!(expression, ExpressionNode::Call(_)).then_some(handle)
                })
                .collect::<Vec<_>>();
            for handle in expression_handles {
                let application = expressions_of(program)
                    .call_node(handle)
                    .expect("call roster")
                    .authored_machine_application;
                let owner = if application.is_valid() {
                    expressions_of(program)
                        .static_machine_application(application)
                        .lexical_owner
                } else {
                    SymbolHandle::invalid()
                };
                let mut request_call = expressions_of(program)
                    .call_node(handle)
                    .expect("call roster")
                    .clone();
                let mut request_changed = false;
                for argument in request_call.request_static_arguments_mut() {
                    request_changed |=
                        normalize_const_arguments(program, owner, std::slice::from_mut(argument))?;
                }
                if request_changed
                    && let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(handle)
                {
                    *call = request_call;
                }
                let has_machine_args = expressions_of(program)
                    .call_node(handle)
                    .is_some_and(|call| !call.machine_arguments.is_empty());
                if !has_machine_args {
                    continue;
                }
                let call = expressions_of(program)
                    .call_node(handle)
                    .expect("call roster");
                let mut arguments = call.machine_arguments.clone();
                let carriers =
                    callee_value_carriers(program, call.target_symbol, &call.machine_arguments);
                if normalize_const_arguments_for(program, owner, &carriers, &mut arguments)? {
                    let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(handle)
                    else {
                        continue;
                    };
                    call.machine_arguments = arguments;
                }
            }
            let mut statement_handles = Vec::new();
            for states in state_spans {
                for state in program.machine_states.span_or_empty(*states) {
                    for (handle, statement) in program
                        .statement_table
                        .iter_statements(state.statement_nodes)
                    {
                        if statement.as_call().is_some_and(|call| {
                            !call.machine_arguments.is_empty()
                                || call.installed_interrupt_operation.is_some()
                        }) {
                            statement_handles.push(handle);
                        }
                    }
                }
            }
            for handle in statement_handles {
                let application = program
                    .statement_table
                    .statement(handle)
                    .as_call()
                    .expect("call roster")
                    .authored_machine_application;
                let owner = if application.is_valid() {
                    expressions_of(program)
                        .static_machine_application(application)
                        .lexical_owner
                } else {
                    SymbolHandle::invalid()
                };
                let call = program
                    .statement_table
                    .statement(handle)
                    .as_call()
                    .expect("call roster");
                let mut call = call.clone();
                let carriers =
                    callee_value_carriers(program, call.target_symbol, &call.machine_arguments);
                let mut changed = normalize_const_arguments_for(
                    program,
                    owner,
                    &carriers,
                    &mut call.machine_arguments,
                )?;
                for argument in call.request_static_arguments_mut() {
                    changed |=
                        normalize_const_arguments(program, owner, std::slice::from_mut(argument))?;
                }
                if changed {
                    let StatementNode::Call(destination) =
                        program.statement_table.statement_mut(handle)
                    else {
                        continue;
                    };
                    *destination = call;
                }
            }
            Ok(())
        }

        fn callee_value_carriers(
            program: &TypedTrees,
            target: SymbolHandle,
            arguments: &[StaticMachineArgument],
        ) -> Vec<Option<TypeReferenceHandle>> {
            if !target.is_valid() {
                return Vec::new();
            }
            let Some(machine) = program
                .machine_by_symbol(target)
                .or_else(|| program.machine_holding_state(target))
            else {
                return Vec::new();
            };
            let mut declared =
                declared_value_carriers(program.machine_type_parameters(machine)).into_iter();
            let carriers = arguments
                .iter()
                .map(|argument| {
                    let evidence = !machine.conformance_bounds.is_empty()
                        && program.conformance_by_symbol(argument.symbol).is_some();
                    if evidence {
                        Some(None)
                    } else {
                        declared.next()
                    }
                })
                .collect::<Option<Vec<_>>>();
            match carriers {
                Some(carriers) if declared.next().is_none() => carriers,
                _ => Vec::new(),
            }
        }

        pub(crate) fn normalize_const_arguments(
            program: &mut TypedTrees,
            owner: SymbolHandle,
            arguments: &mut [StaticMachineArgument],
        ) -> Result<bool, String> {
            normalize_const_arguments_for(program, owner, &[], arguments)
        }

        fn normalize_const_arguments_for(
            program: &mut TypedTrees,
            owner: SymbolHandle,
            carriers: &[Option<TypeReferenceHandle>],
            arguments: &mut [StaticMachineArgument],
        ) -> Result<bool, String> {
            let mut changed = false;
            for index in 0..arguments.len() {
                if arguments[index].application.is_some() {
                    let mut nested = arguments[index]
                        .application
                        .as_ref()
                        .expect("nested static application")
                        .arguments
                        .clone();
                    let nested_carriers = program
                        .conformance_by_symbol(arguments[index].symbol)
                        .map(|declaration| {
                            declared_value_carriers(
                                program.conformance_type_parameters(declaration),
                            )
                        })
                        .unwrap_or_default();
                    if normalize_const_arguments_for(program, owner, &nested_carriers, &mut nested)?
                    {
                        arguments[index]
                            .application
                            .as_mut()
                            .expect("nested static application")
                            .arguments = nested;
                        changed = true;
                    }
                }
                if let Some(application) = &arguments[index].application
                    && application.member.is_valid()
                {
                    let handle = application.member;
                    let mut member = expressions_of(program)
                        .static_member(handle)
                        .ok_or("qualified static member has no owning expression record")?
                        .clone();
                    if normalize_const_arguments_for(
                        program,
                        owner,
                        &[],
                        std::slice::from_mut(&mut member),
                    )? {
                        *expressions_of_mut(program)
                            .static_member_mut(handle)
                            .ok_or("qualified static member lost its owning expression record")? =
                            member;
                        changed = true;
                    }
                }
                let Some(value) = closed_const_argument(program, &arguments[index]) else {
                    let expected = carriers
                        .get(index)
                        .copied()
                        .flatten()
                        .filter(|_| carriers.len() == arguments.len());
                    if let Some(value) =
                        closed_structural_argument(program, owner, &arguments[index], expected)?
                    {
                        let atom = value.atom();
                        program
                            .type_reference_table
                            .insert(TypeReferenceNode::Named {
                                symbol: SymbolHandle::invalid(),
                                name: Identifier::generated(atom.clone()),
                            });
                        arguments[index] = StaticMachineArgument {
                            path: vec![Identifier::generated(atom)].into_boxed_slice(),
                            ..StaticMachineArgument::default()
                        };
                        changed = true;
                    }
                    continue;
                };
                remember_integer_type(program, value);
                arguments[index] = StaticMachineArgument {
                    const_literal: Some(integer_literal(value)),
                    ..StaticMachineArgument::default()
                };
                changed = true;
            }
            Ok(changed)
        }

        fn closed_const_argument(
            program: &TypedTrees,
            argument: &StaticMachineArgument,
        ) -> Option<i128> {
            let TypeReferenceNode::ConstExpression(expression) = program
                .type_reference_table
                .type_reference(argument.type_reference)
            else {
                return None;
            };
            match const_integer::classify(expressions_of(program), *expression, &|_, _| false)? {
                ConstIntegerExpression::Closed(value) => Some(value),
                ConstIntegerExpression::Open => None,
            }
        }

        fn remember_integer_type(program: &mut TypedTrees, value: i128) {
            let spelling = value.to_string();
            let exists = program
                .type_reference_table
                .named_references()
                .any(|(_, symbol, name)| !symbol.is_valid() && name == spelling);
            if exists {
                return;
            }
            program
                .type_reference_table
                .insert(TypeReferenceNode::Named {
                    symbol: SymbolHandle::invalid(),
                    name: Identifier::generated(spelling),
                });
        }

        fn integer_literal(value: i128) -> IntegerLiteral {
            if let Ok(narrow) = i64::try_from(value) {
                return IntegerLiteral::from_value(narrow);
            }
            let negative = value < 0;
            let digits = value.unsigned_abs().to_string();
            IntegerLiteral::from_parts(negative, IntegerRadix::Decimal, &digits)
                .expect("decimal digits of a closed const integer")
        }

        fn closed_structural_argument(
            program: &TypedTrees,
            owner: SymbolHandle,
            argument: &StaticMachineArgument,
            expected: Option<TypeReferenceHandle>,
        ) -> Result<Option<language_semantics::const_value::CanonicalConstValue>, String> {
            if !argument.type_reference.is_valid() {
                return Ok(None);
            }
            let TypeReferenceNode::ConstExpression(expression) = program
                .type_reference_table
                .type_reference(argument.type_reference)
            else {
                return Ok(None);
            };
            let carrier = match expressions_of(program).expression(*expression) {
                ExpressionNode::StructLiteral(literal) => {
                    let retained =
                        expressions_of(program).constructed_value_type_reference(*expression);
                    retained.is_valid().then_some(retained).or_else(|| {
                        program
                            .type_reference_table
                            .find_named_type_reference(literal.type_symbol)
                    })
                }
                ExpressionNode::ArrayLiteral(_) => expected,
                _ => parameter_expression_result_type_reference(program, owner, &[], *expression),
            };
            let Some(carrier) = carrier else {
                return Ok(None);
            };
            validation::normalize_closed_const_expression(program, owner, *expression, carrier)
        }

        pub(crate) fn drop_refuted_closed_transition_arms(
            program: &mut TypedTrees,
            candidate: &Candidate,
            machine: &typed_trees::machine::Machine,
        ) {
            let mut rewrites = Vec::new();
            for (index, state) in program.machine_states(machine).iter().enumerate() {
                let statements = program
                    .statement_table
                    .statements(state.statement_nodes)
                    .to_vec();
                if !statements.iter().any(|statement| {
                    transition_arm_requires_are_closed_false(program, candidate, machine, statement)
                }) {
                    continue;
                }
                let live = statements
                    .into_iter()
                    .filter(|statement| {
                        !transition_arm_requires_are_closed_false(
                            program, candidate, machine, statement,
                        )
                    })
                    .collect::<Vec<_>>();
                if live
                    .iter()
                    .any(|statement| matches!(statement, StatementNode::Transition(_)))
                {
                    rewrites.push((index, live));
                }
            }
            for (index, live) in rewrites {
                let body = program.statement_table.insert_statements(live);
                program.machine_states_mut(machine)[index].statement_nodes = body;
            }
        }

        fn transition_callee_symbol(
            program: &TypedTrees,
            target: typed_trees::statement::TransitionTargetHandle,
        ) -> Option<SymbolHandle> {
            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Named { path, .. } => {
                    path.symbol.is_valid().then_some(path.symbol)
                }
                TransitionTargetNode::Value(expression) => {
                    let expression = *expression;
                    let ExpressionNode::Call(call) = expressions_of(program).expression(expression)
                    else {
                        return None;
                    };
                    call.target_symbol.is_valid().then_some(call.target_symbol)
                }
                TransitionTargetNode::Terminal => None,
            }
        }

        fn transition_arm_requires_are_closed_false(
            program: &TypedTrees,
            candidate: &Candidate,
            machine: &typed_trees::machine::Machine,
            statement: &StatementNode,
        ) -> bool {
            let StatementNode::Transition(transition) = statement else {
                return false;
            };
            let Some(path_symbol) = transition_callee_symbol(program, transition.target) else {
                return false;
            };
            let owner = program.machine_holding_state(path_symbol).or(program
                .machine_states(machine)
                .iter()
                .any(|state| state.symbol == path_symbol)
                .then_some(machine));
            let Some(owner) = owner else {
                return false;
            };
            let Some(state) = program
                .machine_states(owner)
                .iter()
                .find(|state| state.symbol == path_symbol)
            else {
                return false;
            };
            let requirements = program
                .contract_expressions(
                    program
                        .machine_contracts(owner)
                        .iter()
                        .chain(program.state_contracts(state)),
                    typed_trees::signature::SignatureContractKind::Requires,
                )
                .collect::<Vec<_>>();
            requirements
                .into_iter()
                .any(|expression| closed_bool(program, candidate, expression) == Some(false))
        }

        fn closed_bool(
            program: &TypedTrees,
            candidate: &Candidate,
            expression: ExpressionHandle,
        ) -> Option<bool> {
            match expressions_of(program).expression(expression) {
                ExpressionNode::Boolean(value) => Some(*value),
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => Some(
                    closed_bool(program, candidate, binary.left)?
                        && closed_bool(program, candidate, binary.right)?,
                ),
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
                    let left = integer_spelling(program, candidate, binary.left)?;
                    let right = integer_spelling(program, candidate, binary.right)?;
                    Some(left == right)
                }
                _ => None,
            }
        }

        fn integer_spelling(
            program: &TypedTrees,
            candidate: &Candidate,
            expression: ExpressionHandle,
        ) -> Option<String> {
            match expressions_of(program).expression(expression) {
                ExpressionNode::Integer(literal) => {
                    Some(literal.canonical_spelling().as_str().to_owned())
                }
                ExpressionNode::Name(path) => {
                    let origin = program.symbols.get(path.symbol).generated_from;
                    let index = candidate.template.const_parameters.iter().position(
                        |(parameter, _, _)| *parameter == path.symbol || *parameter == origin,
                    )?;
                    let binding = candidate.const_bindings[index]?;
                    let literal = program.static_const_literal_from_type_reference(binding)?;
                    Some(literal.canonical_spelling().as_str().to_owned())
                }
                _ => None,
            }
        }

        fn declared_value_carriers(
            parameters: &[typed_trees::data::TypeParameter],
        ) -> Vec<Option<TypeReferenceHandle>> {
            parameters
                .iter()
                .map(|parameter| match parameter.kind {
                    typed_trees::data::TypeParameterKind::Const { type_reference }
                    | typed_trees::data::TypeParameterKind::Value { type_reference } => {
                        Some(type_reference)
                    }
                    _ => None,
                })
                .collect()
        }
    }
    pub(crate) mod evidence_rewrites {
        use crate::monomorphization::{
            Candidate, StateSignature, StaticMachineArgument, SymbolHandle,
        };
        use crate::tables::{expressions_of, expressions_of_mut};

        pub(crate) struct EvidenceArgumentRewrite {
            pub(crate) binder: SymbolHandle,
            pub(crate) argument: StaticMachineArgument,
            pub(crate) alias: typed_trees::name::Identifier,
        }

        pub(crate) fn evidence_argument_rewrites(
            program: &dyn ::validation::CheckingInput,
            candidate: &Candidate,
        ) -> Vec<EvidenceArgumentRewrite> {
            candidate
                .template
                .evidence_parameters
                .iter()
                .zip(candidate.evidence_bindings.iter())
                .filter_map(|(parameter, binding)| {
                    let binder = parameter.binder;
                    let binding = binding.as_ref()?;
                    let alias = if program.symbols.get(binding.symbol).kind
                        == symbols::SymbolKind::ConformanceParameter
                    {
                        typed_trees::name::Identifier::generated(
                            program.symbols.name(binding.symbol),
                        )
                    } else {
                        let selected = program.conformance_by_symbol(binding.symbol)?;
                        selected.alias.clone().unwrap_or_else(|| {
                            typed_trees::name::Identifier::generated("<unnamed-conformance>")
                        })
                    };
                    Some(EvidenceArgumentRewrite {
                        binder,
                        argument: binding.clone(),
                        alias,
                    })
                })
                .collect()
        }

        pub(crate) fn rewrite_index_algebra(
            program: &mut typed_trees::TypedTrees,
            expression: typed_trees::expression::ExpressionHandle,
            rewrites: &[EvidenceArgumentRewrite],
            static_rewrites: &[(SymbolHandle, StaticMachineArgument)],
        ) {
            if expressions_of(program)
                .index_expression_authority(expression)
                .is_none()
            {
                return;
            }
            let mut authority = expressions_of(program)
                .index_expression_authority(expression)
                .expect("index expression authority was checked")
                .clone();
            let Some(operation) = &mut authority.operation else {
                return;
            };
            if let Some(application) = &mut operation.application {
                super::machine_arguments::substitute_forwarded_machine_arguments(
                    expressions_of_mut(program),
                    std::slice::from_mut(application),
                    static_rewrites,
                    &[],
                );
            }
            if let Some(rewrite) = rewrites
                .iter()
                .find(|rewrite| rewrite.binder == operation.provider)
            {
                operation.apply_provider(&rewrite.argument, rewrite.alias.as_str());
            }
            *expressions_of_mut(program).index_expression_authority_mut(expression) = authority;
        }

        pub(crate) struct EvidenceRequirementRewrite {
            pub(crate) placeholder: SymbolHandle,
            pub(crate) target: SymbolHandle,
            pub(crate) name: typed_trees::name::Identifier,
            pub(crate) dispatch: typed_trees::typed_trees::StaticRequirementDispatch,
        }

        pub(crate) fn evidence_requirement_rewrites(
            program: &dyn ::validation::CheckingInput,
            candidate: &Candidate,
        ) -> Vec<EvidenceRequirementRewrite> {
            let mut rewrites = Vec::new();
            for (parameter, binding) in candidate
                .template
                .evidence_parameters
                .iter()
                .zip(candidate.evidence_bindings.iter())
            {
                let binder = parameter.binder;
                let Some(binding) = binding.as_ref() else {
                    continue;
                };
                let Some(trait_definition) =
                    program.trait_definition_by_symbol(parameter.trait_symbol)
                else {
                    continue;
                };
                let Some(selected) = program.conformance_by_symbol(binding.symbol) else {
                    continue;
                };
                let rows = selected.rows.as_slice();
                let application =
                    validation::conformance_applications::close_conformance_application(
                        program, binding,
                    )
                    .expect("validated selected conformance must close during specialization");
                let mut requirements = Vec::new();
                collect_evidence_requirement_closure(
                    program,
                    trait_definition,
                    &mut Vec::new(),
                    &mut requirements,
                );
                let placeholders = program.symbols.child_handles(binder).into_iter().flatten();
                for (placeholder, requirement) in placeholders.zip(requirements) {
                    let Some(row) = rows
                        .iter()
                        .find(|row| row.requirement == requirement.symbol)
                    else {
                        continue;
                    };
                    rewrites.push(EvidenceRequirementRewrite {
                        placeholder,
                        target: row.realization_state,
                        name: typed_trees::name::Identifier::generated(
                            row.realization_name
                                .as_str()
                                .rsplit("::")
                                .next()
                                .unwrap_or(row.realization_name.as_str()),
                        ),
                        dispatch: typed_trees::typed_trees::StaticRequirementDispatch {
                            selection_application: Default::default(),
                            evidence_parameter: binder,
                            application_report_fingerprint: application.report_fingerprint,
                            application_commitment: application.commitment,
                            declaring_trait: row.declaring_trait,
                            requirement: row.requirement,
                            contract_signature: SymbolHandle::invalid(),
                            realization_machine: row.realization_machine,
                            realization_state: row.realization_state,
                        },
                    });
                }
            }
            rewrites
        }

        pub(crate) fn collect_evidence_requirement_closure<'program>(
            program: &'program dyn ::validation::CheckingInput,
            trait_definition: &'program typed_trees::trait_definition::TraitDefinition,
            visited: &mut Vec<SymbolHandle>,
            output: &mut Vec<&'program StateSignature>,
        ) {
            if visited.contains(&trait_definition.symbol) {
                return;
            }
            visited.push(trait_definition.symbol);
            output.extend(program.trait_machine_signatures(trait_definition).iter());
            if let Some(base) = &trait_definition.refines
                && let Some(base_trait) = program.trait_definition_by_symbol(base.symbol)
            {
                collect_evidence_requirement_closure(program, base_trait, visited, output);
            }
            for parent in program.trait_requirements(trait_definition) {
                let Some(parent_trait) = program.trait_definition_by_symbol(parent.symbol) else {
                    continue;
                };
                collect_evidence_requirement_closure(program, parent_trait, visited, output);
            }
        }
    }
    pub(crate) mod machine_arguments {
        use crate::monomorphization::selection::state_by_symbol;
        use crate::monomorphization::{
            Candidate, StaticMachineArgument, SymbolHandle, TypeReferenceNode, TypedTrees,
        };

        pub(crate) fn substitute_machine_parameter_type_references(
            program: &mut TypedTrees,
            candidate: &Candidate,
            type_start: Option<usize>,
        ) {
            for ((parameter_symbol, _, _), binding) in candidate
                .template
                .machine_parameters
                .iter()
                .zip(candidate.machine_bindings.iter())
            {
                let binding = binding.as_ref().expect("complete specialization");
                let name = binding
                    .path
                    .last()
                    .cloned()
                    .or_else(|| {
                        state_by_symbol(program, binding.symbol).map(|state| state.name.clone())
                    })
                    .expect("admitted static machine argument has an entry name");
                let occurrences: Vec<_> = match type_start {
                    Some(start) => program
                        .type_reference_table
                        .named_references_from(start)
                        .filter(|(_, symbol, _)| symbol == parameter_symbol)
                        .map(|(handle, _, _)| handle)
                        .collect(),
                    None => program
                        .type_reference_table
                        .named_references()
                        .filter(|(_, symbol, _)| symbol == parameter_symbol)
                        .map(|(handle, _, _)| handle)
                        .collect(),
                };
                for occurrence in occurrences {
                    program.type_reference_table.substitute_node(
                        occurrence,
                        TypeReferenceNode::Named {
                            symbol: binding.symbol,
                            name: name.clone(),
                        },
                    );
                }
            }
        }

        pub(crate) fn substitute_forwarded_machine_arguments(
            expressions: &mut typed_trees::expression::ExpressionTable,
            arguments: &mut [StaticMachineArgument],
            static_rewrites: &[(SymbolHandle, StaticMachineArgument)],
            rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
        ) {
            for argument in arguments {
                if let Some((_, replacement)) = static_rewrites
                    .iter()
                    .find(|(parameter, _)| *parameter == argument.symbol)
                {
                    *argument = replacement.clone();
                    continue;
                } else if let Some((_, symbol, name)) = rewrites
                    .iter()
                    .find(|(parameter, _, _)| *parameter == argument.symbol)
                {
                    argument.symbol = *symbol;
                    argument.path = vec![name.clone()].into_boxed_slice();
                }
                if let Some(application) = &mut argument.application {
                    substitute_forwarded_machine_arguments(
                        expressions,
                        &mut application.arguments,
                        static_rewrites,
                        rewrites,
                    );
                    if application.member.is_valid() {
                        let mut member = expressions
                            .static_member(application.member)
                            .expect(
                                "qualified static member belongs to its receiving expression table",
                            )
                            .clone();
                        substitute_forwarded_machine_arguments(
                            expressions,
                            std::slice::from_mut(&mut member),
                            static_rewrites,
                            rewrites,
                        );
                        *expressions.static_member_mut(application.member).expect(
                            "qualified static member remains in its receiving expression table",
                        ) = member;
                    }
                }
            }
        }

        pub(crate) fn forwarded_static_argument_rewrites(
            program: &dyn ::validation::CheckingInput,
            candidate: &Candidate,
        ) -> Vec<(SymbolHandle, StaticMachineArgument)> {
            candidate
                .template
                .type_parameters
                .iter()
                .zip(candidate.type_bindings.iter())
                .filter_map(|((parameter, _), binding)| {
                    program
                        .static_operand_from_type_reference(binding.as_ref().copied()?)
                        .ok()
                        .map(|argument| (*parameter, argument))
                })
                .chain(
                    candidate
                        .template
                        .const_parameters
                        .iter()
                        .zip(candidate.const_bindings.iter())
                        .enumerate()
                        .filter_map(|(index, ((parameter, _, _), binding))| {
                            if candidate.runtime_value_bindings[index].is_some() {
                                return None;
                            }
                            let binding = binding.as_ref().copied()?;
                            let argument =
                                program.static_operand_from_type_reference(binding).ok()?;
                            Some((*parameter, argument))
                        }),
                )
                .chain(
                    candidate
                        .template
                        .machine_parameters
                        .iter()
                        .zip(candidate.machine_bindings.iter())
                        .filter_map(|((parameter, _, _), binding)| {
                            binding
                                .as_ref()
                                .cloned()
                                .map(|binding| (*parameter, binding))
                        }),
                )
                .chain(
                    candidate
                        .template
                        .evidence_parameters
                        .iter()
                        .zip(&candidate.evidence_bindings)
                        .filter_map(|(parameter, binding)| {
                            binding
                                .as_ref()
                                .cloned()
                                .map(|binding| (parameter.binder, binding))
                        }),
                )
                .collect()
        }
    }
    pub(crate) mod type_parameter_substitution {
        use validation::collect_expression_tree;
        use validation::collect_statement_expression_trees;

        use crate::monomorphization::body_cloning::copy_type_reference;
        use crate::monomorphization::body_rewriting::const_substitution::{
            fixed_array_const_replacements, fold_closed_const_machine_arguments_in,
            substitute_const_index_expression_parameters, substitute_fixed_array_const_parameters,
        };
        use crate::monomorphization::body_rewriting::machine_arguments::substitute_machine_parameter_type_references;
        use crate::monomorphization::{
            Candidate, Diagnostic, ExpressionHandle, ExpressionNode, ProofFact, StatementNode,
            SymbolHandle, TypeReferenceHandle, TypeReferenceNode, TypedTrees,
        };
        use crate::tables::expressions_of;

        pub(crate) fn substitute_cloned_type_parameters(
            source: Option<&dyn ::validation::CheckingInput>,
            program: &mut TypedTrees,
            candidate: &Candidate,
            type_start: usize,
            expression_start: usize,
            cloned_states: &[arena::HandleSpan<typed_trees::state::State>],
            instance: SymbolHandle,
        ) -> Result<(), Diagnostic> {
            let mut replacements = Vec::new();
            for ((parameter_symbol, _), binding) in candidate
                .template
                .type_parameters
                .iter()
                .zip(candidate.type_bindings.iter())
            {
                let input = source.unwrap_or(program);
                let definition = input
                    .machine_by_symbol(candidate.template.template_symbol)
                    .and_then(|machine| {
                        input
                    .declared_type_equations
                    .span_or_empty(machine.declared_type_equations)
                    .iter()
                    .find_map(|equation| {
                        matches!(input.type_reference_table.type_reference(equation.parameter),
                        TypeReferenceNode::Named { symbol, .. } if symbol == parameter_symbol)
                        .then_some(equation.definition)
                    })
                    });
                let replacement = definition.unwrap_or(binding.expect("complete specialization"));
                let replacement_handle = copy_type_reference(source, program, replacement, &[])?;
                let owner = if definition.is_some() {
                    candidate.template.template_symbol
                } else {
                    expressions_of(source.unwrap_or(program))
                        .static_machine_application(candidate.binding_application)
                        .lexical_owner
                };
                program
                    .type_reference_table
                    .retain_operand_lifetime_owner(replacement_handle, owner)
                    .map_err(Diagnostic::error)?;
                if instance.is_valid() && definition.is_none() {
                    program.type_reference_table.set_operand_origin(
                        replacement_handle,
                        typed_trees::types::TypeOperandOrigin::MachineParameter {
                            instance,
                            parameter: *parameter_symbol,
                        },
                    );
                }
                replacements.push((*parameter_symbol, replacement_handle));
            }
            for (parameter_symbol, replacement_handle) in replacements {
                let occurrences: Vec<_> = program
                    .type_reference_table
                    .named_references_from(type_start)
                    .filter(|(_, symbol, _)| *symbol == parameter_symbol)
                    .map(|(handle, _, _)| handle)
                    .collect();
                for occurrence in occurrences {
                    program
                        .type_reference_table
                        .substitute_from(occurrence, replacement_handle)
                        .map_err(Diagnostic::error)?;
                }
                program.type_reference_table.substitute_shared_children(
                    type_start,
                    parameter_symbol,
                    replacement_handle,
                );
            }
            for ((parameter_symbol, _, _), binding) in candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.const_bindings.iter())
                .enumerate()
                .filter_map(|(index, (parameter, binding))| {
                    (!candidate.runtime_value_bindings[index].is_some())
                        .then_some((parameter, binding))
                })
            {
                let occurrences: Vec<_> = program
                    .type_reference_table
                    .named_references_from(type_start)
                    .filter(|(_, symbol, _)| symbol == parameter_symbol)
                    .map(|(handle, _, _)| handle)
                    .collect();
                let replacement_handle = copy_type_reference(
                    source,
                    program,
                    binding.expect("complete const specialization"),
                    &[],
                )?;
                for occurrence in occurrences {
                    program
                        .type_reference_table
                        .substitute_from(occurrence, replacement_handle)
                        .map_err(Diagnostic::error)?;
                }
                program.type_reference_table.substitute_shared_children(
                    type_start,
                    *parameter_symbol,
                    replacement_handle,
                );
            }
            let fixed_array_replacements =
                fixed_array_const_replacements(source.unwrap_or(program), candidate);
            substitute_fixed_array_const_parameters(
                program,
                &fixed_array_replacements,
                Some(type_start),
            );
            substitute_const_index_expression_parameters(source, program, candidate, type_start)
                .map_err(Diagnostic::error)?;
            substitute_machine_parameter_type_references(program, candidate, Some(type_start));
            fold_closed_const_machine_arguments_in(program, expression_start, cloned_states)
                .map_err(Diagnostic::error)?;
            Ok(())
        }

        pub(crate) fn reject_runtime_bound_static_occurrences(
            program: &dyn ::validation::CheckingInput,
            candidate: &Candidate,
            cloned: &typed_trees::machine::Machine,
        ) -> Result<(), Diagnostic> {
            if candidate.runtime_value_bindings.iter().all(Option::is_none) {
                return Ok(());
            }
            let mut roots = Vec::new();
            for equation in program
                .declared_type_equations
                .span_or_empty(cloned.declared_type_equations)
            {
                roots.extend([equation.parameter, equation.definition]);
            }
            roots.extend(
                program
                    .machine_owned_data(cloned)
                    .iter()
                    .map(|item| item.type_reference),
            );
            for contract in program.machine_contracts(cloned) {
                collect_contract_type_roots(program, contract, &mut roots);
            }
            for state in program.machine_states(cloned) {
                roots.extend(
                    program
                        .state_parameters(state)
                        .iter()
                        .map(|parameter| parameter.type_reference),
                );
                roots.push(state.return_type);
                for contract in program.state_contracts(state) {
                    collect_contract_type_roots(program, contract, &mut roots);
                }
                for statement in program.statement_table.statements(state.statement_nodes) {
                    if let Some(local) = statement.as_local_data() {
                        roots.push(local.type_reference);
                    }
                    if let StatementNode::Call(call) = &statement {
                        collect_static_argument_type_roots(
                            program,
                            &call.machine_arguments,
                            &mut roots,
                        );
                        for argument in call.request_static_arguments() {
                            collect_static_argument_type_roots(
                                program,
                                std::slice::from_ref(argument),
                                &mut roots,
                            );
                        }
                    }
                    let mut expressions = Vec::new();
                    collect_statement_expression_trees(program, statement, &mut expressions);
                    for expression in expressions {
                        collect_expression_type_roots(program, expression, &mut roots);
                    }
                }
            }
            for ((parameter_symbol, parameter_name, _), runtime) in candidate
                .template
                .const_parameters
                .iter()
                .zip(candidate.runtime_value_bindings.iter())
            {
                if runtime.is_none() {
                    continue;
                }
                if runtime_bound_occurrence_in(program, &roots, *parameter_symbol) {
                    return Err(Diagnostic::error(format!(
                        "value parameter `{parameter_name}` of machine `{}` is bound to a runtime \
                 argument and cannot determine a static type or layout",
                        candidate.template.template_name,
                    )));
                }
            }
            Ok(())
        }

        pub(crate) fn rebind_state_value_parameters(
            program: &mut TypedTrees,
            cloned: &typed_trees::machine::Machine,
            state_realized_parameters: &[Vec<(SymbolHandle, SymbolHandle)>],
        ) {
            for (state, realized) in program
                .machine_states(cloned)
                .to_vec()
                .into_iter()
                .zip(state_realized_parameters.iter())
            {
                if realized.is_empty() {
                    continue;
                }
                let mut roots = Vec::new();
                roots.extend(
                    program
                        .state_parameters(&state)
                        .iter()
                        .map(|parameter| parameter.type_reference),
                );
                roots.push(state.return_type);
                for contract in program.state_contracts(&state) {
                    collect_contract_type_roots(program, contract, &mut roots);
                }
                for statement in program
                    .statement_table
                    .statements(state.statement_nodes)
                    .to_vec()
                {
                    if let StatementNode::LocalData(local) = &statement {
                        roots.push(local.type_reference);
                    }
                    if let StatementNode::Call(call) = &statement {
                        collect_static_argument_type_roots(
                            program,
                            &call.machine_arguments,
                            &mut roots,
                        );
                        for argument in call.request_static_arguments() {
                            collect_static_argument_type_roots(
                                program,
                                std::slice::from_ref(argument),
                                &mut roots,
                            );
                        }
                    }
                    let mut expressions = Vec::new();
                    collect_statement_expression_trees(program, &statement, &mut expressions);
                    for expression in expressions {
                        collect_expression_type_roots(program, expression, &mut roots);
                    }
                }
                for root in roots {
                    rebind_type_value_subjects(program, root, realized);
                }
            }
        }

        fn rebind_type_value_subjects(
            program: &mut TypedTrees,
            root: TypeReferenceHandle,
            realized: &[(SymbolHandle, SymbolHandle)],
        ) {
            let mut pending = vec![root];
            let mut visited = Vec::new();
            while let Some(reference) = pending.pop() {
                if !reference.is_valid() || visited.contains(&reference) {
                    continue;
                }
                visited.push(reference);
                match program
                    .type_reference_table
                    .type_reference(reference)
                    .clone()
                {
                    TypeReferenceNode::Reference { referee, .. } => pending.push(referee),
                    TypeReferenceNode::Slice { element_type }
                    | TypeReferenceNode::FixedArray { element_type, .. } => {
                        pending.push(element_type)
                    }
                    TypeReferenceNode::Constrained { base_type, .. } => pending.push(base_type),
                    TypeReferenceNode::Generic {
                        base_symbol,
                        arguments,
                        ..
                    } => {
                        let roles =
                            program
                                .unique_data_definition_by_symbol(base_symbol)
                                .map(|data| {
                                    program
                                        .data_type_parameters(data)
                                        .iter()
                                        .map(|parameter| parameter.kind.clone())
                                        .collect::<Vec<_>>()
                                });
                        let arguments = program
                            .type_reference_table
                            .type_reference_handles(arguments)
                            .to_vec();
                        if let Some(roles) = roles {
                            for (argument, role) in arguments.into_iter().zip(roles) {
                                match role {
                                    typed_trees::data::TypeParameterKind::Value { .. } => {
                                        let tables = &mut program.tables;
                                        tables.type_reference_table.remap_symbols_in(
                                            argument,
                                            &mut tables.expression_table,
                                            realized,
                                        );
                                    }
                                    typed_trees::data::TypeParameterKind::Type => {
                                        pending.push(argument)
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        pub(crate) fn cloned_expression_roots(
            program: &dyn ::validation::CheckingInput,
            cloned: &typed_trees::machine::Machine,
        ) -> Vec<ExpressionHandle> {
            let mut roots = Vec::new();
            for item in program.machine_owned_data(cloned) {
                roots.push(item.initial_value);
            }
            for state in program.machine_states(cloned) {
                for statement in program.statement_table.statements(state.statement_nodes) {
                    collect_statement_expression_trees(program, statement, &mut roots);
                }
            }
            for contract in program.machine_contracts(cloned).iter().chain(
                program
                    .machine_states(cloned)
                    .iter()
                    .flat_map(|state| program.state_contracts(state)),
            ) {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    validation::append_fact_expression_roots(program, fact, &mut roots);
                }
            }
            roots
        }

        pub(crate) fn collect_contract_type_roots(
            program: &dyn ::validation::CheckingInput,
            contract: &typed_trees::signature::SignatureContract,
            roots: &mut Vec<TypeReferenceHandle>,
        ) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => {
                        collect_expression_type_roots(program, *expression, roots)
                    }
                    ProofFact::Membership(membership) => {
                        roots.extend(
                            program
                                .type_reference_table
                                .type_reference_handles(membership.domain_arguments),
                        );
                        collect_expression_type_roots(program, membership.value, roots);
                    }
                    ProofFact::Proposition(application) => {
                        for argument in
                            expressions_of(program).expression_handles(application.arguments)
                        {
                            collect_expression_type_roots(program, *argument, roots);
                        }
                    }
                }
            }
        }

        pub(crate) fn collect_expression_type_roots(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            roots: &mut Vec<TypeReferenceHandle>,
        ) {
            let mut tree = Vec::new();
            collect_expression_tree(program, expression, &mut tree);
            for handle in tree {
                match expressions_of(program).expression(handle) {
                    ExpressionNode::Cast(cast) => {
                        roots.push(cast.target_type);
                        roots.push(cast.result_type);
                        roots.extend(
                            program
                                .type_reference_table
                                .type_reference_handles(cast.semantic_domain_arguments),
                        );
                    }
                    ExpressionNode::Call(call) => {
                        collect_static_argument_type_roots(program, &call.machine_arguments, roots);
                        for argument in call.request_static_arguments() {
                            collect_static_argument_type_roots(
                                program,
                                std::slice::from_ref(argument),
                                roots,
                            );
                        }
                    }
                    ExpressionNode::TypeExpression(type_reference)
                    | ExpressionNode::ZeroValue(type_reference) => roots.push(*type_reference),
                    _ => {}
                }
            }
        }

        pub(crate) fn name_mentions_symbol(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            symbol: SymbolHandle,
        ) -> bool {
            let mut tree = Vec::new();
            collect_expression_tree(program, expression, &mut tree);
            tree.into_iter().any(|handle| {
                matches!(
                    expressions_of(program).expression(handle),
                    ExpressionNode::Name(path) if path.symbol == symbol
                )
            })
        }

        pub(crate) fn runtime_bound_occurrence_in(
            program: &dyn ::validation::CheckingInput,
            roots: &[TypeReferenceHandle],
            parameter_symbol: SymbolHandle,
        ) -> bool {
            crate::monomorphization::for_each_type_reference(program, roots, |program, node| {
                match node {
                    TypeReferenceNode::Named { symbol, .. }
                    | TypeReferenceNode::DynamicTrait { symbol, .. } => *symbol == parameter_symbol,
                    TypeReferenceNode::FixedArray { length, .. } => matches!(
                        length,
                        typed_trees::types::FixedArrayLength::ConstParameter { symbol, .. }
                            if *symbol == parameter_symbol
                    ),
                    TypeReferenceNode::ConstExpression(expression) => {
                        name_mentions_symbol(program, *expression, parameter_symbol)
                    }
                    _ => false,
                }
            })
        }

        fn collect_static_argument_type_roots(
            program: &TypedTrees,
            arguments: &[typed_trees::expression::StaticMachineArgument],
            roots: &mut Vec<TypeReferenceHandle>,
        ) {
            for argument in arguments {
                if argument.type_reference.is_valid() {
                    roots.push(argument.type_reference);
                }
                if let Some(application) = &argument.application {
                    collect_static_argument_type_roots(program, &application.arguments, roots);
                    if application.member.is_valid() {
                        let member = expressions_of(program)
                            .static_member(application.member)
                            .expect("qualified static member belongs to its expression table");
                        collect_static_argument_type_roots(
                            program,
                            std::slice::from_ref(member),
                            roots,
                        );
                    }
                }
            }
        }
    }

    pub(crate) use const_substitution::drop_refuted_closed_transition_arms;
    pub(crate) use const_substitution::fold_closed_const_machine_arguments;

    pub(crate) use cloned_calls::{rewrite_cloned_calls, statement_span_handles};
    pub(crate) use const_substitution::normalize_const_arguments;

    pub(crate) use evidence_rewrites::collect_evidence_requirement_closure;
    pub(crate) use evidence_rewrites::{evidence_argument_rewrites, rewrite_index_algebra};
    pub(crate) use machine_arguments::forwarded_static_argument_rewrites;
    pub(crate) use type_parameter_substitution::{
        cloned_expression_roots, rebind_state_value_parameters,
        reject_runtime_bound_static_occurrences, substitute_cloned_type_parameters,
    };

    use crate::monomorphization::const_arguments;
    use crate::monomorphization::selection::state_by_symbol;
    use crate::monomorphization::{
        CallSite, ExpressionHandle, ExpressionNode, HandleSpan, StatementNode,
        StaticMachineArgument, SymbolHandle, TypedTrees,
    };
    use typed_trees::expression::TableMemberExpression;

    pub(crate) fn rewrite_selected_call(
        program: &mut TypedTrees,
        site: CallSite,
        target: SymbolHandle,
        subjects: &[ExpressionHandle],
    ) {
        let target_name = state_by_symbol(program, target)
            .map(|state| state.name.clone())
            .expect("cloned specialization state");
        rewrite_selected_call_with_name(program, site, target, target_name, subjects);
    }

    pub(crate) fn runtime_value_operands(
        program: &mut TypedTrees,
        state_symbol: SymbolHandle,
        machine_arguments: &[crate::monomorphization::RuntimeValueBinding],
    ) -> Vec<ExpressionHandle> {
        let plans = {
            let Some(state) = state_by_symbol(program, state_symbol) else {
                return Vec::new();
            };
            machine_arguments
                .iter()
                .filter_map(|argument| match argument {
                    crate::monomorphization::RuntimeValueBinding::Carrier => None,
                    crate::monomorphization::RuntimeValueBinding::Authored(argument) => {
                        plan_runtime_operand(program, state, argument)
                    }
                    crate::monomorphization::RuntimeValueBinding::CapturedArgument {
                        site,
                        position,
                        source,
                        descriptor,
                    } => {
                        if let Some(expression) = retained_capture_operand(
                            program,
                            *site,
                            *position,
                            *source,
                            *descriptor,
                        ) {
                            Some(OperandPlan::Existing(expression))
                        } else {
                            Some(OperandPlan::Captured {
                                site: *site,
                                position: *position,
                                source: *source,
                                descriptor: *descriptor,
                            })
                        }
                    }
                })
                .collect::<Vec<_>>()
        };
        plans
            .into_iter()
            .map(|plan| realize_operand(program, plan))
            .collect()
    }

    pub(crate) fn runtime_call_operands(
        program: &mut TypedTrees,
        site: CallSite,
        state_symbol: SymbolHandle,
        bindings: &[crate::monomorphization::RuntimeValueBinding],
    ) -> Vec<ExpressionHandle> {
        let count = bindings
            .iter()
            .filter(|binding| {
                !matches!(
                    binding,
                    crate::monomorphization::RuntimeValueBinding::Carrier
                )
            })
            .count();
        if let Some(operands) = retained_runtime_operands(program, site, count)
            && bindings
                .iter()
                .filter(|binding| {
                    !matches!(
                        binding,
                        crate::monomorphization::RuntimeValueBinding::Carrier
                    )
                })
                .zip(&operands)
                .all(|(binding, actual)| {
                    runtime_operand_matches(program, state_symbol, binding, *actual)
                })
        {
            return operands;
        }
        runtime_value_operands(program, state_symbol, bindings)
    }

    fn runtime_operand_matches(
        program: &TypedTrees,
        state: SymbolHandle,
        binding: &crate::monomorphization::RuntimeValueBinding,
        actual: ExpressionHandle,
    ) -> bool {
        let plan = match binding {
            crate::monomorphization::RuntimeValueBinding::Carrier => return false,
            crate::monomorphization::RuntimeValueBinding::Authored(argument) => {
                state_by_symbol(program, state)
                    .and_then(|state| plan_runtime_operand(program, state, argument))
            }
            crate::monomorphization::RuntimeValueBinding::CapturedArgument {
                site,
                position,
                source,
                descriptor,
            } => {
                return retained_capture_operand(program, *site, *position, *source, *descriptor)
                    == Some(actual);
            }
        };
        match plan {
            Some(OperandPlan::Existing(expression) | OperandPlan::Recipe(expression)) => {
                expression == actual
            }
            Some(OperandPlan::Name(_, subject)) => {
                validation::canonical_place_from_expression(program, actual).is_some_and(|place| {
                    place.root.as_symbol() == Some(subject) && place.segments.is_empty()
                })
            }
            Some(OperandPlan::Field {
                receiver, field, ..
            }) => {
                let ExpressionNode::Member(member) = expressions_of(program).expression(actual)
                else {
                    return false;
                };
                member.member_symbol == field
                    && validation::canonical_place_from_expression(program, member.receiver)
                        .is_some_and(|place| {
                            place.root.as_symbol() == Some(receiver) && place.segments.is_empty()
                        })
            }
            _ => false,
        }
    }

    fn retained_runtime_operands(
        program: &TypedTrees,
        site: CallSite,
        count: usize,
    ) -> Option<Vec<ExpressionHandle>> {
        use typed_trees::data::TypeParameterKind;
        let (application, arguments, dispatch) = match site {
            CallSite::Expression(handle) => {
                let call = expressions_of(program).call_node(handle)?;
                (
                    call.authored_machine_application,
                    expressions_of(program).expression_handles(call.arguments),
                    call.static_requirement_dispatch.as_ref(),
                )
            }
            CallSite::Statement(handle) => {
                let call = program.statement_table.statement(handle).as_call()?;
                (
                    call.authored_machine_application,
                    program.statement_table.expression_handles(call.arguments),
                    call.static_requirement_dispatch.as_ref(),
                )
            }
        };
        if !application.is_valid() || count == 0 {
            return None;
        }
        let source = expressions_of(program).static_machine_application(application);
        let parameters =
            if let Some(machine) = program.machine_by_symbol(source.binding_declaration) {
                program.machine_callable_parameters(machine)
            } else if let Some((_, signature)) =
                program.trait_machine_signature(source.binding_declaration)
            {
                program
                    .requirement_invocation_contract_parameters(signature)
                    .to_vec()
            } else {
                return None;
            };
        let bindings = expressions_of(program).static_machine_application_bindings(application);
        let runtime = parameters
            .iter()
            .filter(|parameter| {
                matches!(parameter.kind, TypeParameterKind::Value { .. })
                    && bindings.iter().any(|binding| {
                        binding.parameter == parameter.symbol
                            && matches!(
                                binding.value,
                                StaticMachineBindingValue::RuntimeValue { .. }
                            )
                    })
            })
            .collect::<Vec<_>>();
        if runtime.len() != count
            || bindings
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.value,
                        StaticMachineBindingValue::RuntimeValue { .. }
                    )
                })
                .count()
                != count
        {
            return None;
        }
        let operation = dispatch.map_or(source.registration_operation, |dispatch| {
            dispatch.requirement
        });
        let ordinary = if let Some((_, signature)) = program.trait_machine_signature(operation) {
            program.state_signature_parameters(signature).len()
        } else if let Some((_, signature)) = program.machine_parameter_signature(operation) {
            program.state_signature_parameters(signature).len()
        } else {
            let state = program.state_by_symbol(operation).or_else(|| {
                program
                    .machine_by_symbol(operation)
                    .and_then(|machine| program.machine_states(machine).first())
            })?;
            program.state_parameters(state).len()
        };
        if arguments.len() != ordinary + count {
            return None;
        }
        for (parameter, actual) in runtime.into_iter().zip(&arguments[ordinary..]) {
            let mut matches = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter.symbol);
            let binding = matches.next()?;
            if matches.next().is_some() {
                return None;
            }
            let StaticMachineBindingValue::RuntimeValue {
                subject,
                expression,
                ..
            } = binding.value
            else {
                return None;
            };
            if expression.is_valid() {
                if expression != *actual {
                    return None;
                }
            } else if !subject.is_valid()
                || !validation::canonical_place_from_expression(program, *actual).is_some_and(
                    |place| place.root.as_symbol() == Some(subject) && place.segments.is_empty(),
                )
            {
                return None;
            }
        }
        Some(arguments[ordinary..].to_vec())
    }

    fn retained_capture_operand(
        program: &TypedTrees,
        site: crate::monomorphization::CallSite,
        position: u32,
        source: ExpressionHandle,
        descriptor: SymbolHandle,
    ) -> Option<ExpressionHandle> {
        use typed_trees::expression::ValueCaptureOwner;
        let application = site.authored_machine_application(program)?;
        let owner = match site {
            crate::monomorphization::CallSite::Expression(call) => {
                ValueCaptureOwner::ExpressionArgument { call, position }
            }
            crate::monomorphization::CallSite::Statement(call) => {
                ValueCaptureOwner::StatementArgument { call, position }
            }
        };
        if !application.is_valid() {
            return None;
        }
        expressions_of(program)
            .static_machine_application_bindings(application)
            .iter()
            .find_map(|binding| {
                let StaticMachineBindingValue::RuntimeValue { expression, .. } = binding.value
                else {
                    return None;
                };
                if !expression.is_valid() {
                    return None;
                }
                let ExpressionNode::Member(member) = expressions_of(program).expression(expression)
                else {
                    return None;
                };
                let ExpressionNode::SavedSubject(saved) =
                    expressions_of(program).expression(member.receiver)
                else {
                    return None;
                };
                (member.member_symbol == descriptor
                    && program.saved_argument_source(member.receiver) == Some(source)
                    && expressions_of(program).value_capture(saved.capture).owner == owner)
                    .then_some(expression)
            })
    }

    enum OperandPlan {
        Captured {
            site: crate::monomorphization::CallSite,
            position: u32,
            source: ExpressionHandle,
            descriptor: SymbolHandle,
        },
        Existing(ExpressionHandle),
        Recipe(ExpressionHandle),
        Name(typed_trees::name::Identifier, SymbolHandle),
        Field {
            receiver_name: typed_trees::name::Identifier,
            receiver: SymbolHandle,
            field_name: typed_trees::name::Identifier,
            field: SymbolHandle,
        },
    }

    fn plan_runtime_operand(
        program: &TypedTrees,
        state: &typed_trees::state::State,
        argument: &StaticMachineArgument,
    ) -> Option<OperandPlan> {
        if let Some(expression) = const_arguments::runtime_subject_expression(program, argument) {
            return Some(OperandPlan::Recipe(expression));
        }
        if let Some(projection) = &argument.evidence_projection {
            let subject =
                runtime_field_subject(program, state, &projection.term, &projection.member)?;
            return Some(OperandPlan::Field {
                receiver_name: projection.term.clone(),
                receiver: subject.receiver,
                field_name: projection.member.clone(),
                field: subject.field,
            });
        }
        let symbol =
            const_arguments::resolve_runtime_subject(program, state, usize::MAX, argument)?;
        Some(OperandPlan::Name(argument.path.first()?.clone(), symbol))
    }

    fn realize_operand(program: &mut TypedTrees, plan: OperandPlan) -> ExpressionHandle {
        match plan {
            OperandPlan::Captured {
                site,
                position,
                source,
                descriptor,
            } => {
                use typed_trees::expression::{TableSavedSubject, ValueCaptureOwner};
                let owner = match site {
                    crate::monomorphization::CallSite::Expression(call) => {
                        ValueCaptureOwner::ExpressionArgument { call, position }
                    }
                    crate::monomorphization::CallSite::Statement(call) => {
                        ValueCaptureOwner::StatementArgument { call, position }
                    }
                };
                let capture =
                    expressions_of_mut(program).insert_call_operand_capture(source, owner);
                let receiver = expressions_of_mut(program).insert(ExpressionNode::SavedSubject(
                    TableSavedSubject {
                        component: Default::default(),
                        capture,
                        pattern_head: ExpressionHandle::invalid(),
                    },
                ));
                let member = program.symbols.name(descriptor).into();
                expressions_of_mut(program).insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member,
                    member_symbol: descriptor,
                    case_variant: None,
                }))
            }
            OperandPlan::Existing(expression) => expression,
            OperandPlan::Recipe(expression) => expressions_of_mut(program).insert_copy(expression),
            OperandPlan::Name(name, symbol) => {
                insert_subject_name(expressions_of_mut(program), name, symbol)
            }
            OperandPlan::Field {
                receiver_name,
                receiver,
                field_name,
                field,
            } => {
                let receiver =
                    insert_subject_name(expressions_of_mut(program), receiver_name, receiver);
                expressions_of_mut(program).insert(ExpressionNode::Member(TableMemberExpression {
                    receiver,
                    member_symbol: field,
                    member: field_name,
                    case_variant: None,
                }))
            }
        }
    }

    pub(crate) fn cloned_runtime_call_subjects(
        program: &mut TypedTrees,
        states: HandleSpan<typed_trees::state::State>,
        state_symbols: &[(SymbolHandle, SymbolHandle)],
        expression_start: usize,
    ) -> Vec<(ExpressionHandle, Vec<ExpressionHandle>)> {
        let mut pending = Vec::new();
        let mut expressions = Vec::new();
        for state in program.machine_states.span_or_empty(states) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                expressions.clear();
                collect_statement_expression_trees(program, statement, &mut expressions);
                for expression in expressions.iter().copied() {
                    if (expression.arena_index() as usize) < expression_start {
                        continue;
                    }
                    let Some(call) = expressions_of(program).call_node(expression) else {
                        continue;
                    };
                    if state_symbols
                        .iter()
                        .any(|(_, target)| *target == call.target_symbol)
                    {
                        let plans = call
                            .machine_arguments
                            .iter()
                            .filter_map(|argument| plan_runtime_operand(program, state, argument))
                            .collect::<Vec<_>>();
                        pending.push((expression, plans));
                    }
                }
            }
        }
        let mut calls = Vec::new();
        for (expression, plans) in pending {
            let operands = plans
                .into_iter()
                .map(|plan| realize_operand(program, plan))
                .collect();
            calls.push((expression, operands));
        }
        calls.sort_by_key(|(expression, _)| (expression.arena_index(), expression.generation()));
        calls.dedup_by_key(|(expression, _)| *expression);
        calls
    }

    pub(crate) fn insert_subject_name(
        table: &mut typed_trees::expression::ExpressionTable,
        member: typed_trees::name::Identifier,
        symbol: SymbolHandle,
    ) -> ExpressionHandle {
        let mut members = HandleSpan::empty();
        table.push_name_path_member(&mut members, member);
        let mut member_symbols = HandleSpan::empty();
        table.push_name_path_member_symbol(&mut member_symbols, symbol);
        table.insert(ExpressionNode::Name(
            typed_trees::expression::TableNamePath::new(members, member_symbols, symbol, symbol),
        ))
    }

    pub(crate) fn insert_subject_names(
        table: &mut typed_trees::expression::ExpressionTable,
        arguments: &mut Vec<ExpressionHandle>,
        subjects: &[(typed_trees::name::Identifier, SymbolHandle)],
    ) {
        for (member, symbol) in subjects.iter().cloned() {
            arguments.push(insert_subject_name(table, member, symbol));
        }
    }

    fn align_ordinary_arguments(
        arguments: &mut Vec<ExpressionHandle>,
        parameter_count: usize,
        subjects: &[ExpressionHandle],
    ) {
        let ordinary = parameter_count.saturating_sub(subjects.len());
        if arguments.len() > ordinary {
            arguments.truncate(ordinary);
        }
        insert_subject_operands(arguments, subjects);
    }

    pub(crate) fn insert_subject_operands(
        arguments: &mut Vec<ExpressionHandle>,
        operands: &[ExpressionHandle],
    ) {
        arguments.extend(operands.iter().copied());
    }

    pub(crate) fn insert_arguments_with_subjects(
        program: &mut TypedTrees,
        arguments_span: HandleSpan<ExpressionHandle>,
        subjects: &[ExpressionHandle],
    ) -> HandleSpan<ExpressionHandle> {
        let mut arguments = program
            .statement_table
            .expression_handles(arguments_span)
            .to_vec();
        insert_subject_operands(&mut arguments, subjects);
        program.statement_table.insert_expression_handles(arguments)
    }

    pub(crate) fn rewrite_selected_call_with_name(
        program: &mut TypedTrees,
        site: CallSite,
        target: SymbolHandle,
        target_name: typed_trees::name::Identifier,
        subjects: &[ExpressionHandle],
    ) {
        let retained = retained_runtime_operands(program, site, subjects.len());
        let reuse = retained.as_deref() == Some(subjects)
            && program.state_by_symbol(target).is_some_and(|state| {
                program.state_parameters(state).len()
                    == match site {
                        CallSite::Expression(handle) => expressions_of(program)
                            .call_node(handle)
                            .map_or(0, |call| call.arguments.len()),
                        CallSite::Statement(handle) => program
                            .statement_table
                            .statement(handle)
                            .as_call()
                            .map_or(0, |call| call.arguments.len()),
                    }
            });
        match site {
            CallSite::Statement(handle) => {
                let StatementNode::Call(snapshot) =
                    program.statement_table.statement(handle).clone()
                else {
                    return;
                };
                let authored_machine_application =
                    if snapshot.authored_machine_application.is_valid() {
                        snapshot.authored_machine_application
                    } else {
                        expressions_of_mut(program).retain_static_machine_application(
                            snapshot.target_symbol,
                            snapshot.target.clone(),
                            &snapshot.machine_arguments,
                        )
                    };
                let new_arguments = if reuse {
                    snapshot.arguments
                } else {
                    let parameter_count = program
                        .state_by_symbol(target)
                        .map(|state| program.state_parameters(state).len())
                        .unwrap_or(subjects.len());
                    let mut arguments = program
                        .statement_table
                        .expression_handles(snapshot.arguments)
                        .to_vec();
                    align_ordinary_arguments(&mut arguments, parameter_count, subjects);
                    program.statement_table.insert_expression_handles(arguments)
                };
                let StatementNode::Call(call) = program.statement_table.statement_mut(handle)
                else {
                    unreachable!();
                };
                call.target_symbol = target;
                call.target = target_name;
                call.arguments = new_arguments;
                call.authored_machine_application = authored_machine_application;
                call.machine_arguments = Box::default();
                record_runtime_call_operands(program, authored_machine_application, subjects);
            }
            CallSite::Expression(handle) => {
                let ExpressionNode::Call(snapshot) =
                    expressions_of(program).expression(handle).clone()
                else {
                    return;
                };
                if snapshot.native_code_layout.is_some() {
                    let ExpressionNode::Call(call) =
                        expressions_of_mut(program).expression_mut(handle)
                    else {
                        unreachable!()
                    };
                    call.native_code_layout.as_mut().unwrap().operand.symbol = target;
                    return;
                }
                let authored_machine_application =
                    if snapshot.authored_machine_application.is_valid() {
                        snapshot.authored_machine_application
                    } else {
                        expressions_of_mut(program).retain_static_machine_application(
                            snapshot.target_symbol,
                            snapshot.target.clone(),
                            &snapshot.machine_arguments,
                        )
                    };
                let mut arguments = expressions_of(program)
                    .expression_handles(snapshot.arguments)
                    .to_vec();
                if !reuse {
                    let parameter_count = program
                        .state_by_symbol(target)
                        .map(|state| program.state_parameters(state).len())
                        .unwrap_or(subjects.len());
                    align_ordinary_arguments(&mut arguments, parameter_count, subjects);
                }
                let new_arguments =
                    expressions_of_mut(program).insert_expression_handles(arguments);
                let ExpressionNode::Call(call) = expressions_of_mut(program).expression_mut(handle)
                else {
                    unreachable!();
                };
                call.target_symbol = target;
                call.target = target_name;
                call.arguments = new_arguments;
                call.authored_machine_application = authored_machine_application;
                call.machine_arguments = Box::default();
                record_runtime_call_operands(program, authored_machine_application, subjects);
            }
        }
    }

    pub(crate) fn record_runtime_call_operands(
        program: &mut TypedTrees,
        application: typed_trees::static_applications::StaticMachineApplicationHandle,
        operands: &[ExpressionHandle],
    ) {
        if !application.is_valid() {
            return;
        }
        let mut bindings = expressions_of(program)
            .static_machine_application_bindings(application)
            .to_vec();
        let mut operands = operands.iter().copied();
        for binding in &mut bindings {
            let StaticMachineBindingValue::RuntimeValue {
                subject,
                expression,
                ..
            } = &mut binding.value
            else {
                continue;
            };
            let Some(operand) = operands.next() else {
                break;
            };
            if !subject.is_valid() || expression.is_valid() {
                *expression = operand;
            }
        }
        expressions_of_mut(program).set_static_machine_application_bindings(application, &bindings);
    }
}
