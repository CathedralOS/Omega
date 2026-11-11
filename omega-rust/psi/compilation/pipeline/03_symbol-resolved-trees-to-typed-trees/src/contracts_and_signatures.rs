/*
Authored contract invocations, `requires` proof facts, and parameter domain
membership (`contracts`) alongside shared parameter, type-parameter, evidence,
and callable-signature lowering (`signatures`).
*/

pub(crate) mod contracts {
    pub(crate) mod invocations {
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use symbols::SymbolKind;
        use typed_trees as typed;

        pub(crate) fn lower_authored_invocations(
            program: &resolved::SymbolResolvedTrees,
            declarations: &[resolved::name::DiagnosticName],
            parameters: &[resolved::signature::StateParameter],
            owner: &str,
        ) -> Result<Vec<typed::signature::AuthoredInvocation>, Diagnostic> {
            let mut lowered = Vec::with_capacity(declarations.len());
            for declaration in declarations {
                let target = if let Some((ordinal, parameter)) = parameters
                    .iter()
                    .enumerate()
                    .find(|(_, parameter)| parameter.name.as_str() == declaration.as_str())
                {
                    typed::signature::AuthoredInvocationTarget::Parameter {
                ordinal: u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(format!(
                        "callable `{owner}` has too many invocation parameters for portable identity"
                    ))
                })?,
                symbol: parameter.symbol,
            }
                } else {
                    let service_symbol = program
                        .symbols
                        .find_top_level_by_name_and_kinds_from_source(
                            declaration.as_str(),
                            &[SymbolKind::Trait],
                            declaration.source_span(),
                        );
                    service_symbol.map_or(
                        typed::signature::AuthoredInvocationTarget::Unresolved,
                        typed::signature::AuthoredInvocationTarget::Service,
                    )
                };
                lowered.push(typed::signature::AuthoredInvocation {
                    name: crate::lowerer::name::lower_name(declaration),
                    source_span: declaration.source_span(),
                    target,
                });
            }
            Ok(lowered)
        }
    }
    pub(crate) mod parameter_domains {
        use crate::lowerer::Lowerer;
        use crate::signatures::parameters::lower_state_parameter;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) use validation::parameter_domain_constraints as domain_constraints;

        pub(crate) type DomainConstrainedParameter = (
            symbols::SymbolHandle,
            typed::name::Identifier,
            symbols::SymbolHandle,
            String,
            Vec<typed::types::TypeReferenceHandle>,
            language_semantics::SemanticDomainId,
        );

        pub(crate) fn lower_state_parameters_collecting_domain_constraints(
            lowerer: &mut Lowerer,
            parameters: arena::HandleSpan<resolved::signature::StateParameter>,
            mut push_parameter: impl FnMut(&mut typed::TypedTrees, typed::signature::StateParameter),
        ) -> Result<Vec<DomainConstrainedParameter>, diagnostics::Diagnostic> {
            let mut domain_constrained_parameters: Vec<DomainConstrainedParameter> = Vec::new();
            for parameter in lowerer.source_trees.state_parameters(parameters) {
                let parameter = lower_state_parameter(lowerer, parameter)?;
                for (domain_symbol, domain_full_name, domain_arguments, semantic_domain) in
                    domain_constraints(&lowerer.typed_trees, parameter.type_reference, &[])
                {
                    domain_constrained_parameters.push((
                        parameter.symbol,
                        parameter.name.clone(),
                        domain_symbol,
                        domain_full_name,
                        domain_arguments,
                        semantic_domain,
                    ));
                }
                push_parameter(&mut lowerer.typed_trees, parameter);
            }
            Ok(domain_constrained_parameters)
        }

        pub(crate) fn push_domain_membership_contracts(
            lowerer: &mut Lowerer,
            domain_constrained_parameters: Vec<DomainConstrainedParameter>,
            mut push_contract: impl FnMut(&mut typed::TypedTrees, typed::signature::SignatureContract),
        ) {
            for (
                param_symbol,
                param_name,
                domain_symbol,
                domain_full_name,
                domain_arguments,
                semantic_domain,
            ) in domain_constrained_parameters
            {
                let contract = lowerer.typed_trees.parameter_domain_membership_contract(
                    param_symbol,
                    param_name,
                    domain_symbol,
                    &domain_full_name,
                    domain_arguments,
                    semantic_domain,
                );
                push_contract(&mut lowerer.typed_trees, contract);
            }
        }
    }
    pub(crate) mod proof_facts {
        pub(crate) mod domain_selections {
            use diagnostics::Diagnostic;
            use language_semantics::declaration_selection::AuthoredDeclarationSelectionLateBinding;
            use symbols::{JoinedPath, SymbolHandle};
            use typed_trees::domain::ProofFact;
            use typed_trees::{
                TypedTrees,
                signature::{SignatureContract, StateParameter},
            };

            pub(crate) fn normalize(
                source: &symbol_resolved_trees::SymbolResolvedTrees,
                program: &mut TypedTrees,
            ) -> Result<(), Diagnostic> {
                let mut updates = Vec::new();
                let mut collect = |owner: SymbolHandle,
                                   parameters: &[StateParameter],
                                   contracts: &[SignatureContract]|
                 -> Result<(), Diagnostic> {
                    for contract in contracts {
                        for handle in contract.facts.handles() {
                            let ProofFact::Membership(membership) = program.proof_facts.get(handle)
                            else {
                                continue;
                            };
                            if membership.domain_symbol.is_valid() {
                                continue;
                            }
                            let Some(carrier) =
                                validation::parameter_expression_result_type_reference(
                                    program,
                                    owner,
                                    parameters,
                                    membership.value,
                                )
                            else {
                                continue;
                            };
                            let carrier =
                                program.type_reference_table.peeled_type_reference(carrier);
                            let Some(occurrence) = membership.authored_domain_selection else {
                                continue;
                            };
                            let selection = program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "contract membership lost its authored declaration occurrence",
                        )
                    })?;
                            let name = program
                                .domain_path_members
                                .span_or_empty(membership.domain)
                                .joined_path();
                            let candidates =
                                crate::type_reference::domain_constraints::select_domain_candidates(
                                    source,
                                    program,
                                    carrier,
                                    &name,
                                    Some(selection.source_span()),
                                    SymbolHandle::invalid(),
                                    program
                                        .type_reference_table
                                        .type_reference_handles(membership.domain_arguments),
                                );
                            let [domain] = candidates.as_slice() else {
                                continue;
                            };
                            if domain.alias.is_some() {
                                continue;
                            }
                            super::check_membership_argument_count(
                                program,
                                domain,
                                membership.domain_arguments.len(),
                            )?;
                            updates.push((handle, occurrence, domain.symbol));
                        }
                    }
                    Ok(())
                };
                for machine in program.machines() {
                    if let Some(entry) = program.machine_states(machine).first() {
                        collect(
                            machine.symbol,
                            program.state_parameters(entry),
                            program.machine_contracts(machine),
                        )?;
                    }
                    for state in program.machine_states(machine) {
                        collect(
                            state.symbol,
                            program.state_parameters(state),
                            program.state_contracts(state),
                        )?;
                    }
                }
                for definition in program.traits() {
                    for signature in program.trait_machine_signatures(definition).iter().chain(
                        definition
                            .refinement_clauses
                            .iter()
                            .map(|clause| &clause.signature),
                    ) {
                        collect(
                            signature.symbol,
                            program.state_signature_parameters(signature),
                            program.state_signature_contracts(signature),
                        )?;
                    }
                }
                for operator in program.operators() {
                    collect(
                        operator.symbol,
                        program.operator_parameters(operator),
                        program.operator_contracts(operator),
                    )?;
                }
                let mut selections = program.authored_declaration_selections().clone();
                for (position, (_, occurrence, symbol)) in updates.iter().enumerate() {
                    if let Some((_, _, previous)) = updates[..position]
                        .iter()
                        .find(|(_, prior, _)| prior == occurrence)
                    {
                        if previous != symbol {
                            return Err(Diagnostic::error(
                                "one contract membership occurrence selects conflicting declarations",
                            ));
                        }
                        continue;
                    }
                    selections
            .finalize_late_bound(
                *occurrence,
                AuthoredDeclarationSelectionLateBinding::CheckedDomainMembership,
                *symbol,
            )
            .map_err(|error| {
                Diagnostic::error(format!(
                    "failed to finalize contract membership declaration custody: {error:?}"
                ))
            })?;
                }
                program.retain_authored_declaration_selections(selections);
                for (handle, _, symbol) in updates {
                    if let ProofFact::Membership(membership) = program.proof_facts.get_mut(handle) {
                        membership.domain_symbol = symbol;
                    }
                }
                Ok(())
            }
        }
        use crate::type_reference::lower_type_reference_into_table;
        pub(crate) use domain_selections::normalize as normalize_membership_selections;

        use crate::expressions::expression::{
            lower_expression_handle_from_table_with_symbol_substitution, lower_fact_expression,
        };
        use crate::expressions::proposition::is_proposition_call_target;
        use crate::lowerer::Lowerer;
        use crate::lowerer::name::lower_name;
        use crate::tables::resolved_expressions;
        use crate::type_reference::domain_aliases::expand_domain_reference;
        use arena::HandleSpan;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use symbols::JoinedPath;
        use typed_trees as typed;

        pub(crate) fn lower_proof_facts(
            lowerer: &mut Lowerer,
            facts: HandleSpan<resolved::domain::ProofFact>,
        ) -> Result<HandleSpan<typed::domain::ProofFact>, Diagnostic> {
            let mut lowered = HandleSpan::empty();

            for (offset, fact) in lowerer.source_trees.proof_facts(facts).iter().enumerate() {
                let source_fact =
                    facts.handle_at(u32::try_from(offset).expect("proof fact offset overflow"));
                let source_span = lowerer.source_trees.proof_fact_source_span(source_fact);
                match fact {
                    resolved::domain::ProofFact::Expression(expression) => {
                        let fact = lower_expression_fact(lowerer, *expression)?;
                        let handle = lowerer
                            .typed_trees
                            .proof_facts
                            .append_to_span(&mut lowered, fact);
                        if let Some(source_span) = source_span {
                            lowerer
                                .typed_trees
                                .set_proof_fact_source_span(handle, source_span);
                        }
                    }
                    resolved::domain::ProofFact::Membership(membership) => {
                        let value = lower_expression_handle_from_table_with_symbol_substitution(
                            Some(lowerer.source_trees),
                            &lowerer.source_trees.tables.bodies.expressions,
                            &mut lowerer.typed_trees,
                            membership.value,
                            None,
                        )?;
                        let authored_path = lowerer
                            .source_trees
                            .domain_path_members(membership.domain)
                            .to_vec();
                        let domain_arguments =
                            lower_membership_domain_arguments(lowerer, membership, source_span)?;
                        let expanded = expand_domain_reference(
                            lowerer.source_trees,
                            membership.domain_symbol,
                            authored_path,
                        )?;
                        if !domain_arguments.is_empty() && expanded.len() != 1 {
                            let alias = lowerer
                                .source_trees
                                .domain_path_members(membership.domain)
                                .joined_path();
                            return Err(with_optional_source_span(
                                Diagnostic::error(format!(
                                    "domain alias `{alias}` does not take index arguments"
                                )),
                                source_span,
                            ));
                        }
                        for atom in expanded {
                            let mut domain = HandleSpan::empty();
                            for member in atom.path {
                                lowerer
                                    .typed_trees
                                    .domain_path_members
                                    .append_to_span(&mut domain, lower_name(&member));
                            }
                            let handle = lowerer.typed_trees.proof_facts.append_to_span(
                                &mut lowered,
                                typed::domain::ProofFact::Membership(
                                    typed::domain::ProofMembershipFact {
                                        value,
                                        domain,
                                        domain_symbol: atom.symbol,
                                        domain_arguments,
                                        semantic_domain: language_semantics::SemanticDomainId::NULL,
                                        authored_domain_selection: membership
                                            .authored_domain_selection,
                                    },
                                ),
                            );
                            if let Some(source_span) = source_span {
                                lowerer
                                    .typed_trees
                                    .set_proof_fact_source_span(handle, source_span);
                            }
                        }
                    }
                }
            }

            Ok(lowered)
        }

        fn with_optional_source_span(
            diagnostic: Diagnostic,
            source_span: Option<source::SourceSpan>,
        ) -> Diagnostic {
            match source_span {
                Some(source_span) => diagnostic.with_source_span(source_span),
                None => diagnostic,
            }
        }

        fn lower_membership_domain_arguments(
            lowerer: &mut Lowerer,
            membership: &resolved::domain::ProofMembershipFact,
            source_span: Option<source::SourceSpan>,
        ) -> Result<HandleSpan<typed::types::TypeReferenceHandle>, Diagnostic> {
            if membership.domain_arguments.is_empty() {
                return Ok(HandleSpan::empty());
            }
            let source_arguments = lowerer
                .source_trees
                .child_type_references(membership.domain_arguments)
                .to_vec();
            let mut arguments = Vec::with_capacity(source_arguments.len());
            for argument in &source_arguments {
                arguments.push(lower_type_reference_into_table(lowerer, argument)?);
            }
            if let Some(domain) = lowerer
                .typed_trees
                .domain_definition(membership.domain_symbol)
            {
                check_membership_argument_count(&lowerer.typed_trees, domain, arguments.len())
                    .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
            }
            Ok(lowerer
                .typed_trees
                .type_reference_table
                .insert_type_reference_handles(arguments))
        }

        fn check_membership_argument_count(
            program: &typed::TypedTrees,
            domain: &typed::domain::DomainDefinition,
            supplied: usize,
        ) -> Result<(), Diagnostic> {
            let index_parameters = typed::domain::index_parameters(program, domain);
            if supplied != index_parameters.len() {
                return Err(Diagnostic::error(format!(
                    "domain family `{}` requires {} closed index argument(s), but {} were supplied",
                    domain.name,
                    index_parameters.len(),
                    supplied
                )));
            }
            Ok(())
        }

        pub(crate) fn intern_proof_membership_instances(
            program: &mut typed::TypedTrees,
        ) -> Result<(), Diagnostic> {
            let mut instances = Vec::new();
            for (handle, fact) in program.proof_facts.iter() {
                let Some(membership) = fact.as_membership() else {
                    continue;
                };
                if membership.domain_arguments.is_empty() || !membership.domain_symbol.is_valid() {
                    continue;
                }
                let Some(domain) = program.domain_definition(membership.domain_symbol) else {
                    continue;
                };
                let source_span = program.proof_fact_source_span(handle);
                check_membership_argument_count(program, domain, membership.domain_arguments.len())
                    .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
                let arguments = program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments);
                let identity = typed::domain::indexed_domain_instance_name(
                    program,
                    domain,
                    typed::domain::index_parameters(program, domain),
                    arguments,
                )
                .map_err(|diagnostic| with_optional_source_span(diagnostic, source_span))?;
                instances.push((handle, identity));
            }
            for (handle, identity) in instances {
                let semantic_domain = program.semantic_domains.intern(&identity);
                if let typed::domain::ProofFact::Membership(membership) =
                    program.proof_facts.get_mut(handle)
                {
                    membership.semantic_domain = semantic_domain;
                }
            }
            Ok(())
        }

        pub(crate) fn lower_contract_kind(
            kind: &resolved::signature::SignatureContractKind,
        ) -> typed::signature::SignatureContractKind {
            match kind {
                resolved::signature::SignatureContractKind::Requires => {
                    typed::signature::SignatureContractKind::Requires
                }
                resolved::signature::SignatureContractKind::Ensures => {
                    typed::signature::SignatureContractKind::Ensures
                }
                resolved::signature::SignatureContractKind::EnsuresForResultCase {
                    result_data,
                    result_case,
                } => typed::signature::SignatureContractKind::EnsuresForResultCase {
                    result_data: *result_data,
                    result_case: *result_case,
                },
                resolved::signature::SignatureContractKind::Crashes { cause } => {
                    typed::signature::SignatureContractKind::Crashes {
                        cause: match cause {
                            resolved::signature::CrashCause::Trap => {
                                typed::signature::CrashCause::Trap
                            }
                            resolved::signature::CrashCause::Abort => {
                                typed::signature::CrashCause::Abort
                            }
                        },
                    }
                }
            }
        }

        pub(crate) fn lower_signature_contract(
            lowerer: &mut Lowerer,
            contract: &resolved::signature::SignatureContract,
        ) -> Result<typed::signature::SignatureContract, Diagnostic> {
            let facts = lower_proof_facts(lowerer, contract.facts)?;
            Ok(typed::signature::SignatureContract {
                kind: lower_contract_kind(&contract.kind),
                keyword_source_span: contract.keyword_source_span,
                binding: contract
                    .binding
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                facts,
                token_count: contract.token_count,
            })
        }

        pub(crate) fn lower_expression_fact(
            lowerer: &mut Lowerer,
            expression: resolved::expression::ExpressionHandle,
        ) -> Result<typed::domain::ProofFact, Diagnostic> {
            if let resolved::expression::ExpressionNode::Call(call) =
                resolved_expressions(lowerer.source_trees).expression(expression)
                && is_proposition_call_target(&lowerer.source_trees.symbols, call.target_symbol)
            {
                return crate::expressions::proposition::lower_proposition_application(
                    lowerer, call,
                )
                .map(typed::domain::ProofFact::Proposition);
            }
            lower_fact_expression(lowerer, expression).map(typed::domain::ProofFact::Expression)
        }
        pub(crate) fn lower_statement_fact(
            lowerer: &mut Lowerer,
            expression: resolved::expression::ExpressionHandle,
        ) -> Result<arena::Handle<typed::domain::ProofFact>, Diagnostic> {
            let span = resolved_expressions(lowerer.source_trees).source_span(expression);
            let fact = lower_expression_fact(lowerer, expression)?;
            let handle = lowerer.typed_trees.proof_facts.insert(fact);
            lowerer.typed_trees.set_proof_fact_source_span(handle, span);
            Ok(handle)
        }
    }
}

pub(crate) mod signatures {
    pub(crate) mod callable_signature {
        use crate::lowerer::Lowerer;
        use crate::lowerer::name::lower_names;
        use crate::signatures::type_parameters::lower_type_parameters;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        use crate::contracts::invocations::lower_authored_invocations;

        pub(crate) fn lower_state_signature(
            lowerer: &mut Lowerer,
            signature: &resolved::signature::StateSignature,
        ) -> Result<typed::signature::StateSignature, Diagnostic> {
            let mut typed_signature = typed::signature::StateSignature {
                symbol: signature.symbol,
                name: crate::lowerer::name::lower_name(&signature.name),
                spelling: signature.spelling,
                lifetime_parameters: lower_names(&signature.lifetime_parameters),
                type_parameters: Default::default(),
                conformance_bounds: crate::signatures::evidence::lower_conformance_bounds(
                    lowerer,
                    &signature.conformance_bounds,
                )?,
                is_default: signature.is_default,
                parameters: Default::default(),
                native_callback_parameters: signature
                    .native_callback_parameters
                    .iter()
                    .map(|parameter| typed::signature::NativeCallbackParameter {
                        name: crate::lowerer::name::lower_name(&parameter.name),
                        binder: crate::lowerer::name::lower_name(&parameter.binder),
                        native_ordinal: parameter.native_ordinal,
                    })
                    .collect(),
                return_type: crate::type_reference::lower_return_type(
                    lowerer,
                    &signature.return_type,
                )?,
                invokes: Default::default(),
                service_reach_row: signature.service_reach_row,
                service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
                suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
                blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
                suspends: signature.suspends,
                blocks: signature.blocks,
                contracts: Default::default(),
                termination_guarantee: if signature.terminates_guarantee {
                    language_semantics::TerminationGuarantee::Terminates {
                        premises: Vec::new(),
                    }
                } else {
                    language_semantics::TerminationGuarantee::NoGuarantee
                },
                declares_no_crash_route: signature.declares_no_crash_route,
            };

            typed_signature.type_parameters =
                lower_type_parameters(lowerer, signature.type_parameters)?;

            let domain_constrained_parameters =
        crate::contracts::parameter_domains::lower_state_parameters_collecting_domain_constraints(
            lowerer,
            signature.parameters,
            |typed_trees, parameter| {
                typed_trees.push_state_signature_parameter(&mut typed_signature, parameter)
            },
        )?;

            let invocations = lower_authored_invocations(
                lowerer.source_trees,
                lowerer.source_trees.signature_invokes(signature.invokes),
                lowerer.source_trees.state_parameters(signature.parameters),
                signature.name.as_str(),
            )?;
            for invocation in invocations {
                lowerer
                    .typed_trees
                    .push_state_signature_invoke(&mut typed_signature, invocation);
            }

            for contract in lowerer
                .source_trees
                .signature_contracts(signature.contracts)
            {
                let contract =
                    crate::contracts::proof_facts::lower_signature_contract(lowerer, contract)?;
                lowerer
                    .typed_trees
                    .push_state_signature_contract(&mut typed_signature, contract);
            }

            crate::contracts::parameter_domains::push_domain_membership_contracts(
                lowerer,
                domain_constrained_parameters,
                |typed_trees, contract| {
                    typed_trees.push_state_signature_contract(&mut typed_signature, contract)
                },
            );

            Ok(typed_signature)
        }
    }
    pub(crate) mod evidence {
        use crate::lowerer::Lowerer;
        use crate::lowerer::name::lower_name;
        use crate::type_reference::lower_type_reference_into_table;
        use diagnostics::Diagnostic;

        pub(crate) fn lower_conformance_bounds<'source>(
            lowerer: &mut Lowerer,
            bounds: impl IntoIterator<
                Item = &'source symbol_resolved_trees::machine::GenericConformanceBound,
            >,
        ) -> Result<Vec<typed_trees::machine::GenericConformanceBound>, Diagnostic> {
            bounds
                .into_iter()
                .map(|bound| {
                    let mut arguments = Vec::new();
                    for argument in lowerer.source_trees.child_type_references(bound.arguments) {
                        arguments.push(lower_type_reference_into_table(lowerer, argument)?);
                    }
                    Ok(typed_trees::machine::GenericConformanceBound {
                        binder: bound.binder,
                        binder_name: lower_name(&bound.binder_name),
                        trait_symbol: bound.trait_symbol,
                        trait_name: lower_name(&bound.trait_name),
                        lifetime_arguments: bound
                            .lifetime_arguments
                            .iter()
                            .map(lower_name)
                            .collect(),
                        arguments,
                    })
                })
                .collect()
        }
    }
    pub(crate) mod lexical_context {
        use crate::expressions::expression::lower_static_machine_argument;
        use diagnostics::Diagnostic;
        use language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure as SelectionExposure;
        use symbol_resolved_trees as resolved;
        use symbols::{SymbolHandle, SymbolKind};
        use typed_trees as typed;

        fn enclosing_declarations<'program>(
            program: &'program resolved::SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
        ) -> impl Iterator<Item = &'program resolved::machine::Machine> {
            let mut owners = Vec::new();
            let mut parent = machine.lexical_parent;
            while parent.is_valid() {
                let owner = program.machines.get(parent);
                owners.push(owner);
                parent = owner.lexical_parent;
            }
            owners.into_iter().rev()
        }

        pub(crate) fn enclosing_parameters<'program>(
            program: &'program resolved::SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
        ) -> Vec<&'program resolved::data::TypeParameter> {
            enclosing_declarations(program, machine)
                .flat_map(|owner| program.machine_type_parameters(owner))
                .collect()
        }

        pub(crate) fn enclosing_bounds<'program>(
            program: &'program resolved::SymbolResolvedTrees,
            machine: &resolved::machine::Machine,
        ) -> impl Iterator<Item = &'program resolved::machine::GenericConformanceBound> {
            enclosing_declarations(program, machine).flat_map(|owner| &owner.conformance_bounds)
        }

        pub(crate) fn call_arguments(
            program: Option<&resolved::SymbolResolvedTrees>,
            target: &mut typed::TypedTrees,
            exposure: Option<SelectionExposure>,
            callee: SymbolHandle,
            authored: &[resolved::expression::StaticMachineArgument],
        ) -> Result<Box<[typed::expression::StaticMachineArgument]>, Diagnostic> {
            let mut arguments = captured_arguments(program, callee);
            for argument in authored {
                arguments.push(lower_static_machine_argument(
                    program, target, exposure, argument,
                )?);
            }
            Ok(arguments.into_boxed_slice())
        }

        pub(crate) fn captured_arguments(
            program: Option<&resolved::SymbolResolvedTrees>,
            callee: SymbolHandle,
        ) -> Vec<typed::expression::StaticMachineArgument> {
            let Some(program) = program.filter(|_| callee.is_valid()) else {
                return Vec::new();
            };
            let owner = match program.symbols.get(callee).kind {
                SymbolKind::Machine => callee,
                SymbolKind::State => program.symbols.get(callee).parent,
                _ => return Vec::new(),
            };
            if !owner.is_valid() {
                return Vec::new();
            }
            let enclosing = program.symbols.get(owner).parent;
            if !enclosing.is_valid() || program.symbols.get(enclosing).kind != SymbolKind::Machine {
                return Vec::new();
            }
            let machine = program.machine(owner);
            machine.map_or_else(Vec::new, |machine| {
                enclosing_parameters(program, machine)
                    .into_iter()
                    .map(|parameter| typed::expression::StaticMachineArgument {
                        symbol: parameter.symbol,
                        path: vec![crate::lowerer::name::lower_name(&parameter.name)]
                            .into_boxed_slice(),
                        ..Default::default()
                    })
                    .chain(enclosing_bounds(program, machine).map(|bound| {
                        typed::expression::StaticMachineArgument {
                            symbol: bound.binder,
                            path: vec![crate::lowerer::name::lower_name(&bound.binder_name)]
                                .into_boxed_slice(),
                            ..Default::default()
                        }
                    }))
                    .collect()
            })
        }
    }
    pub(crate) mod parameters {
        use crate::lowerer::Lowerer;
        use crate::type_reference::lower_type_reference_into_table;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_state_parameter(
            lowerer: &mut Lowerer,
            parameter: &resolved::signature::StateParameter,
        ) -> Result<typed::signature::StateParameter, Diagnostic> {
            let type_reference =
                lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
            crate::type_reference::domain_constraints::normalize_domain_constraints_for_type(
                lowerer.source_trees,
                &mut lowerer.typed_trees,
                type_reference,
            )?;
            Ok(state_parameter(parameter, type_reference))
        }

        pub(crate) fn state_parameter(
            parameter: &resolved::signature::StateParameter,
            type_reference: typed::types::TypeReferenceHandle,
        ) -> typed::signature::StateParameter {
            typed::signature::StateParameter {
                symbol: parameter.symbol,
                name: crate::lowerer::name::lower_name(&parameter.name),
                type_reference,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                relevance: parameter.relevance,
            }
        }
    }
    pub(crate) mod type_parameters {
        use crate::lowerer::Lowerer;
        use crate::signatures::callable_signature::lower_state_signature;
        use crate::type_reference::lower_type_reference_into_table;
        use diagnostics::Diagnostic;
        use symbol_resolved_trees as resolved;
        use typed_trees as typed;

        pub(crate) fn lower_type_parameters(
            lowerer: &mut Lowerer,
            parameters: arena::HandleSpan<resolved::data::TypeParameter>,
        ) -> Result<arena::HandleSpan<typed::data::TypeParameter>, Diagnostic> {
            let parameters = lowerer
                .source_trees
                .data_type_parameters(parameters)
                .iter()
                .map(|parameter| lower_type_parameter(lowerer, parameter))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(lowerer
                .typed_trees
                .data_type_parameters
                .insert_many(parameters))
        }

        pub(crate) fn lower_type_parameter(
            lowerer: &mut Lowerer,
            parameter: &resolved::data::TypeParameter,
        ) -> Result<typed::data::TypeParameter, Diagnostic> {
            Ok(typed::data::TypeParameter {
                symbol: parameter.symbol,
                name: crate::lowerer::name::lower_name(&parameter.name),
                kind: lower_type_parameter_kind(lowerer, &parameter.kind)?,
                bounds: typed::data::DataProperties {
                    carry: parameter.bounds.carry,
                    multiplicity: parameter.bounds.multiplicity,
                },
            })
        }

        pub(crate) fn lower_type_parameter_kind(
            lowerer: &mut Lowerer,
            kind: &resolved::data::TypeParameterKind,
        ) -> Result<typed::data::TypeParameterKind, Diagnostic> {
            match kind {
                resolved::data::TypeParameterKind::Type => Ok(typed::data::TypeParameterKind::Type),
                resolved::data::TypeParameterKind::Implementation => {
                    Ok(typed::data::TypeParameterKind::Implementation)
                }
                resolved::data::TypeParameterKind::Const { type_reference } => {
                    Ok(typed::data::TypeParameterKind::Const {
                        type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
                    })
                }
                resolved::data::TypeParameterKind::Value { type_reference } => {
                    Ok(typed::data::TypeParameterKind::Value {
                        type_reference: lower_type_reference_into_table(lowerer, type_reference)?,
                    })
                }
                resolved::data::TypeParameterKind::Machine { contract } => {
                    let contract = match contract {
                        resolved::data::MachineParameterContract::RequirementIdentity => {
                            typed::data::MachineParameterContract::RequirementIdentity
                        }
                        resolved::data::MachineParameterContract::Structural(signature) => {
                            typed::data::MachineParameterContract::Structural(
                                lower_state_signature(lowerer, signature)?,
                            )
                        }
                        resolved::data::MachineParameterContract::AuthoredNominal { .. } => {
                            return Err(Diagnostic::error(
                                "an unresolved nominal machine-parameter requirement reached typed lowering",
                            ));
                        }
                        resolved::data::MachineParameterContract::Nominal {
                            trait_definition,
                            requirement,
                            authored_path,
                            trait_arguments,
                        } => {
                            let [trait_path @ .., requirement_name] = authored_path.as_slice()
                            else {
                                return Err(Diagnostic::error(
                                    "a nominal machine-parameter requirement lost its authored `Trait::requirement` path before typed lowering",
                                ));
                            };
                            if trait_path.is_empty() {
                                return Err(Diagnostic::error(
                                    "a nominal machine-parameter requirement lost its authored trait path before typed lowering",
                                ));
                            }
                            crate::type_reference::retain_path_selection(
                        &mut lowerer.typed_trees,
                        trait_path,
                        *trait_definition,
                        lowerer.type_reference_exposure,
                        language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
                        "nominal machine-parameter trait requirement",
                    )?;
                            crate::type_reference::retain_static_path_selection(
                                &mut lowerer.typed_trees,
                                std::slice::from_ref(requirement_name),
                                *requirement,
                                lowerer.type_reference_exposure,
                                "nominal machine-parameter requirement",
                            )?;
                            let mut lowered_arguments = Vec::new();
                            for argument in
                                lowerer.source_trees.child_type_references(*trait_arguments)
                            {
                                lowered_arguments
                                    .push(lower_type_reference_into_table(lowerer, argument)?);
                            }
                            typed::data::MachineParameterContract::Nominal {
                                receiving_trait: *trait_definition,
                                receiving_arguments: lowered_arguments.clone(),
                                trait_definition: *trait_definition,
                                requirement: *requirement,
                                trait_arguments: lowered_arguments,
                                instantiated: None,
                            }
                        }
                    };
                    Ok(typed::data::TypeParameterKind::Machine { contract })
                }
                resolved::data::TypeParameterKind::Proposition { contract } => {
                    let mut parameters = arena::HandleSpan::empty();
                    for parameter in lowerer.source_trees.state_parameters(contract.parameters) {
                        let parameter = crate::signatures::parameters::lower_state_parameter(
                            lowerer, parameter,
                        )?;
                        lowerer
                            .typed_trees
                            .state_parameters
                            .append_to_span(&mut parameters, parameter);
                    }
                    Ok(typed::data::TypeParameterKind::Proposition {
                        contract: typed::data::PropositionParameterSignature {
                            name: crate::lowerer::name::lower_name(&contract.name),
                            parameters,
                        },
                    })
                }
            }
        }
    }
}
