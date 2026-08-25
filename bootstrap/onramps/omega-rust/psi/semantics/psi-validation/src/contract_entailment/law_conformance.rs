use super::*;

/// A checked Omega body satisfying an ordinary or boundary operator is a
/// software provider, not an accepted leaf. Its own machine contract is proved
/// by the ordinary entailment pass above; this gate then checks that the proved
/// contract covers the selected operator contract.
///
/// The first checked-software rung deliberately admits the contract language
/// that is already load-bearing for boundary operators: equality facts and
/// exact `&&` conjunctions. Provider `requires` may only repeat requirement
/// premises (asking less is valid); every required operator `ensures` conjunct
/// must appear in the provider's proved ensures. Operator parameters are
/// substituted positionally onto provider parameters, and the reserved
/// `result` binder maps only to itself, so renaming is harmless while swapping
/// two parameter roles is not.
pub(crate) fn check_operator_contract_conformance(
    program: &TypedTrees,
    machine: &Machine,
    operator: &psi_typed_trees::operator::OperatorDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(entry_state) = program.machine_states(machine).first() else {
        return; // exact signature validation already reports the missing entry
    };
    let operator_identity =
        psi_typed_trees::operator::boundary_operator_requirement_identity(program, operator);
    let mut requirement_requires = Vec::new();
    let mut requirement_ensures = Vec::new();
    let mut provider_requires = Vec::new();
    let mut provider_ensures = Vec::new();
    let mut unsupported_requirement = false;
    let mut unsupported_provider_requires = false;

    let collect = |contracts: &[psi_typed_trees::signature::SignatureContract],
                   requires: &mut Vec<ExpressionHandle>,
                   ensures: &mut Vec<ExpressionHandle>,
                   unsupported_requires: &mut bool,
                   unsupported_ensures: &mut bool| {
        for contract in contracts {
            let destination = match &contract.kind {
                SignatureContractKind::Requires => &mut *requires,
                SignatureContractKind::Ensures => &mut *ensures,
                SignatureContractKind::Boundary | SignatureContractKind::Crashes { .. } => continue,
            };
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Expression(expression) = fact else {
                    match &contract.kind {
                        SignatureContractKind::Requires => *unsupported_requires = true,
                        SignatureContractKind::Ensures => *unsupported_ensures = true,
                        SignatureContractKind::Boundary | SignatureContractKind::Crashes { .. } => {
                        }
                    }
                    continue;
                };
                if !is_equality_conjunction(program, *expression) {
                    match &contract.kind {
                        SignatureContractKind::Requires => *unsupported_requires = true,
                        SignatureContractKind::Ensures => *unsupported_ensures = true,
                        SignatureContractKind::Boundary | SignatureContractKind::Crashes { .. } => {
                        }
                    }
                }
                collect_equality_conjuncts(program, *expression, destination);
            }
        }
    };
    let mut _unsupported_requirement_requires = false;
    collect(
        program.operator_contracts(operator),
        &mut requirement_requires,
        &mut requirement_ensures,
        &mut _unsupported_requirement_requires,
        &mut unsupported_requirement,
    );
    let mut _unsupported_provider_ensures = false;
    collect(
        program.machine_contracts(machine),
        &mut provider_requires,
        &mut provider_ensures,
        &mut unsupported_provider_requires,
        &mut _unsupported_provider_ensures,
    );

    if unsupported_requirement {
        diagnostics.push(Diagnostic::error(format!(
            "checked machine `{}` satisfies operator `{operator_identity}`, whose ensures contract is outside checked operator-contract entailment's equality/`&&` rung",
            machine.name,
        )));
        return;
    }
    if unsupported_provider_requires {
        diagnostics.push(Diagnostic::error(format!(
            "checked operator provider `{}` adds a non-equality requires fact while satisfying `{operator_identity}`; checked providers may not ask more than the operator requirement",
            machine.name,
        )));
        return;
    }

    let mut name_map: Vec<(SymbolHandle, String, SymbolHandle, String)> = program
        .operator_parameters(operator)
        .iter()
        .zip(program.state_parameters(entry_state))
        .map(|(requirement, provider)| {
            (
                requirement.symbol,
                requirement.name.as_str().to_owned(),
                provider.symbol,
                provider.name.as_str().to_owned(),
            )
        })
        .collect();
    name_map.push((
        SymbolHandle::invalid(),
        RESULT_BINDER.to_owned(),
        SymbolHandle::invalid(),
        RESULT_BINDER.to_owned(),
    ));

    let matches = |requirement_fact: ExpressionHandle, provider_fact: ExpressionHandle| {
        let ExpressionNode::Binary(requirement) =
            program.expression_table.expression(requirement_fact)
        else {
            return false;
        };
        let ExpressionNode::Binary(provider) = program.expression_table.expression(provider_fact)
        else {
            return false;
        };
        [
            (provider.left, provider.right),
            (provider.right, provider.left),
        ]
        .into_iter()
        .any(|(left, right)| {
            operator_contract_expressions_match(program, requirement.left, left, &name_map)
                && operator_contract_expressions_match(program, requirement.right, right, &name_map)
        })
    };

    for provider_requires_fact in &provider_requires {
        if !requirement_requires
            .iter()
            .any(|required| matches(*required, *provider_requires_fact))
        {
            diagnostics.push(Diagnostic::error(format!(
                "checked operator provider `{}` requires `{}`, which operator requirement `{operator_identity}` does not require",
                machine.name,
                program.expression_table.display_name(*provider_requires_fact),
            )));
        }
    }
    for requirement_ensures_fact in &requirement_ensures {
        if !provider_ensures
            .iter()
            .any(|provided| matches(*requirement_ensures_fact, *provided))
        {
            diagnostics.push(Diagnostic::error(format!(
                "checked operator provider `{}` proves no ensures matching operator requirement `{operator_identity}` contract `{}`",
                machine.name,
                program.expression_table.display_name(*requirement_ensures_fact),
            )));
        }
    }
}

fn operator_contract_expressions_match(
    program: &TypedTrees,
    requirement: ExpressionHandle,
    provider: ExpressionHandle,
    name_map: &[(SymbolHandle, String, SymbolHandle, String)],
) -> bool {
    if !requirement.is_valid() || !provider.is_valid() {
        return requirement.is_valid() == provider.is_valid();
    }
    let table = &program.expression_table;
    match (table.expression(requirement), table.expression(provider)) {
        (ExpressionNode::Name(required), ExpressionNode::Name(provided)) => {
            let required_members = table.name_path_members(required.members);
            let provided_members = table.name_path_members(provided.members);
            if let [required_name] = required_members
                && let Some((_, _, provided_symbol, provided_name)) =
                    name_map
                        .iter()
                        .find(|(candidate_symbol, candidate_name, _, _)| {
                            (required.symbol.is_valid()
                                && candidate_symbol.is_valid()
                                && required.symbol == *candidate_symbol)
                                || candidate_name == required_name.as_str()
                        })
            {
                return matches!(provided_members, [actual]
                    if (provided_symbol.is_valid()
                        && provided.symbol.is_valid()
                        && provided.symbol == *provided_symbol)
                        || actual.as_str() == provided_name);
            }
            table.expressions_structurally_equal(requirement, provider)
        }
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => left == right,
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::String(left), ExpressionNode::String(right)) => left == right,
        (ExpressionNode::Float(left), ExpressionNode::Float(right)) => left == right,
        (ExpressionNode::Borrow(left), ExpressionNode::Borrow(right)) => {
            operator_contract_expressions_match(program, left.target, right.target, name_map)
        }
        (ExpressionNode::Unary(left), ExpressionNode::Unary(right)) => {
            left.operator == right.operator
                && operator_contract_expressions_match(
                    program,
                    left.operand,
                    right.operand,
                    name_map,
                )
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && operator_contract_expressions_match(program, left.left, right.left, name_map)
                && operator_contract_expressions_match(program, left.right, right.right, name_map)
        }
        (ExpressionNode::Indexed(left), ExpressionNode::Indexed(right)) => {
            operator_contract_expressions_match(
                program,
                left.collection,
                right.collection,
                name_map,
            ) && operator_contract_expressions_match(program, left.index, right.index, name_map)
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member.as_str() == right.member.as_str()
                && left.case_variant == right.case_variant
                && operator_contract_expressions_match(
                    program,
                    left.receiver,
                    right.receiver,
                    name_map,
                )
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            let target_matches = if left.target_symbol.is_valid() && right.target_symbol.is_valid()
            {
                left.target_symbol == right.target_symbol
            } else {
                left.target.as_str() == right.target.as_str()
            };
            let left_arguments = table.expression_handles(left.arguments);
            let right_arguments = table.expression_handles(right.arguments);
            target_matches
                && left.machine_arguments == right.machine_arguments
                && left.operational_acknowledgement == right.operational_acknowledgement
                && operator_contract_expressions_match(
                    program,
                    left.receiver,
                    right.receiver,
                    name_map,
                )
                && left_arguments.len() == right_arguments.len()
                && left_arguments
                    .iter()
                    .zip(right_arguments)
                    .all(|(left, right)| {
                        operator_contract_expressions_match(program, *left, *right, name_map)
                    })
        }
        (ExpressionNode::ArrayLiteral(left), ExpressionNode::ArrayLiteral(right)) => {
            let left = table.expression_handles(*left);
            let right = table.expression_handles(*right);
            left.len() == right.len()
                && left.iter().zip(right).all(|(left, right)| {
                    operator_contract_expressions_match(program, *left, *right, name_map)
                })
        }
        (ExpressionNode::Range(left), ExpressionNode::Range(right)) => {
            left.end_inclusive == right.end_inclusive
                && operator_contract_expressions_match(program, left.start, right.start, name_map)
                && operator_contract_expressions_match(program, left.end, right.end, name_map)
        }
        (ExpressionNode::StructLiteral(left), ExpressionNode::StructLiteral(right)) => {
            let left_fields = table.struct_fields(left.fields);
            let right_fields = table.struct_fields(right.fields);
            left.type_name.as_str() == right.type_name.as_str()
                && left.case_name.as_ref().map(|name| name.as_str())
                    == right.case_name.as_ref().map(|name| name.as_str())
                && left_fields.len() == right_fields.len()
                && left_fields.iter().zip(right_fields).all(|(left, right)| {
                    left.name.as_str() == right.name.as_str()
                        && operator_contract_expressions_match(
                            program,
                            left.value,
                            right.value,
                            name_map,
                        )
                })
        }
        (ExpressionNode::Cast(left), ExpressionNode::Cast(right)) => {
            left.target_type == right.target_type
                && table.name_path_members(left.target_label)
                    == table.name_path_members(right.target_label)
                && left.domain == right.domain
                && table.name_path_members(left.semantic_domain)
                    == table.name_path_members(right.semantic_domain)
                && left.semantic_domain_arguments == right.semantic_domain_arguments
                && left.semantic_domain_symbol == right.semantic_domain_symbol
                && left.semantic_domain_id == right.semantic_domain_id
                && left.form == right.form
                && operator_contract_expressions_match(program, left.value, right.value, name_map)
        }
        (ExpressionNode::Atomic(left), ExpressionNode::Atomic(right)) => {
            left.ordering == right.ordering
                && operator_contract_expressions_match(program, left.value, right.value, name_map)
                && operator_contract_expressions_match(program, left.result, right.result, name_map)
        }
        (ExpressionNode::ZeroValue(left), ExpressionNode::ZeroValue(right)) => left == right,
        _ => false,
    }
}

fn is_equality_conjunction(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    match binary.operator {
        BinaryOperator::Equal => true,
        BinaryOperator::And => {
            is_equality_conjunction(program, binary.left)
                && is_equality_conjunction(program, binary.right)
        }
        _ => false,
    }
}

/// LAW-CONFORMANCE (rearrange rung B, settle 2026-07-18): a trait requirement
/// carrying `ensures` is a LAW -- an obligation every satisfier proves. The
/// satisfier machine must carry a PROVEN ensures conjunct matching the
/// declared law forall-to-forall: the requirement's parameters are pattern
/// variables that must bind to DISTINCT parameters of the satisfier (a weaker
/// instance -- `add(x, x) == add(x, x)` against `add(a, b) == add(b, a)` --
/// does not license the law), and the law's op-slot applications (`add`,
/// `mul` -- the trait's own requirement names) resolve to the CARRIER's bound
/// machines first. This is the N3 shape-match machinery promoted from
/// suggestion-only to load-bearing.
pub(crate) fn check_law_conformance(
    program: &TypedTrees,
    machine: &Machine,
    conformance_alias: Option<&str>,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_trait_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The declared law conjuncts (Equal binaries; And-chains split;
    // `result`-mentioning conjuncts are functional specs, not laws -- they
    // stay outside this check, exactly like the suggestion path).
    let mut law_conjuncts: Vec<ExpressionHandle> = Vec::new();
    let mut proposition_laws = Vec::new();
    for contract in program.state_signature_contracts(requirement) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Expression(expression) => {
                    collect_equality_conjuncts(program, *expression, &mut law_conjuncts);
                }
                ProofFact::Proposition(application) => proposition_laws.push(application),
                ProofFact::Membership(_) => {}
            }
        }
    }
    if law_conjuncts.is_empty() && proposition_laws.is_empty() {
        return; // an OP requirement, not a law
    }

    let requirement_parameters: Vec<String> = program
        .state_signature_parameters(requirement)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();

    let Some(entry_state) = program.machine_states(machine).first() else {
        return; // the signature check already flagged a stateless machine
    };
    let satisfier_parameters: Vec<String> = program
        .state_parameters(entry_state)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();

    let mut proven_propositions = Vec::new();
    let mut proven_expressions = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            match fact {
                ProofFact::Proposition(application) => proven_propositions.push(application),
                ProofFact::Expression(expression) => proven_expressions.push(*expression),
                ProofFact::Membership(_) => {}
            }
        }
    }
    check_proposition_law_conformance(
        program,
        machine,
        trait_definition,
        requirement,
        explicit_trait_arguments,
        &proposition_laws,
        &proven_propositions,
        &proven_expressions,
        diagnostics,
    );
    if law_conjuncts.is_empty() {
        return;
    }

    // The CARRIER is the satisfier's first entry parameter type (law
    // requirements are Self-shaped; the signature check already bound Self
    // there), or its return type for parameterless requirements.
    let carrier = program
        .state_parameters(entry_state)
        .first()
        .map(|parameter| parameter.type_reference)
        .unwrap_or(entry_state.return_type);

    // The trait's op-slot names, and the carrier's bound machine for each.
    let slot_names: Vec<String> = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .map(|signature| signature.name.as_str().to_owned())
        .collect();
    let slot_bindings = carrier_slot_bindings(
        program,
        trait_definition,
        carrier,
        conformance_alias,
        diagnostics,
    );

    // The satisfier's own PROVEN ensures conjuncts (machine-checked by this
    // engine before this point -- compiling means proven).
    let mut proven_conjuncts: Vec<ExpressionHandle> = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                collect_equality_conjuncts(program, *expression, &mut proven_conjuncts);
            }
        }
    }

    let result_binder = RESULT_BINDER.to_owned();
    for law_conjunct in &law_conjuncts {
        let ExpressionNode::Binary(binary) = program.expression_table.expression(*law_conjunct)
        else {
            continue;
        };
        let (Some(law_left), Some(law_right)) = (
            structural_term(program, binary.left),
            structural_term(program, binary.right),
        ) else {
            continue; // out-of-language law conjunct: nothing to enforce yet
        };
        if term_mentions_variable(&law_left, &result_binder)
            || term_mentions_variable(&law_right, &result_binder)
        {
            continue; // a functional spec, not a law conjunct
        }

        // Resolve the law's op-slot applications to the carrier's machines.
        let mut missing_slots: Vec<String> = Vec::new();
        let law_left =
            rewrite_slot_applications(&law_left, &slot_names, &slot_bindings, &mut missing_slots);
        let law_right =
            rewrite_slot_applications(&law_right, &slot_names, &slot_bindings, &mut missing_slots);
        // N4 identity-law bridging: nullary CONSTANT applications
        // (`zero()`, `one()`) normalize to their constructor bodies, so
        // `add(a, zero())` and the proof's `add(a, Nat::Zero)` are one
        // term.
        let law_left = unfold_constant_applications(program, law_left);
        let law_right = unfold_constant_applications(program, law_right);
        if !missing_slots.is_empty() {
            missing_slots.sort();
            missing_slots.dedup();
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}`, whose law mentions `{}` -- but no machine \
                 satisfies that requirement for this carrier (conform the op first; the law \
                 check resolves op slots through the carrier's own conformances)",
                machine.name,
                trait_definition.name,
                requirement.name,
                missing_slots.join("`, `"),
            )));
            continue;
        }

        let matched = proven_conjuncts.iter().any(|proven| {
            let ExpressionNode::Binary(proven_binary) =
                program.expression_table.expression(*proven)
            else {
                return false;
            };
            let (Some(proven_left), Some(proven_right)) = (
                structural_term(program, proven_binary.left),
                structural_term(program, proven_binary.right),
            ) else {
                return false;
            };
            if term_mentions_variable(&proven_left, &result_binder)
                || term_mentions_variable(&proven_right, &result_binder)
            {
                return false;
            }
            let proven_left = unfold_constant_applications(program, proven_left);
            let proven_right = unfold_constant_applications(program, proven_right);
            [(&proven_left, &proven_right), (&proven_right, &proven_left)]
                .into_iter()
                .any(|(first, second)| {
                    let mut bindings: Vec<(String, StructuralTerm)> = Vec::new();
                    diagnostic_shape_match(&law_left, first, &requirement_parameters, &mut bindings)
                        && diagnostic_shape_match(
                            &law_right,
                            second,
                            &requirement_parameters,
                            &mut bindings,
                        )
                        && bindings_are_forall_general(&bindings, &satisfier_parameters)
                })
        });

        if !matched {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but proves no ensures matching the declared \
                 law `{} == {}` -- a law requirement's satisfier must carry that equation as a \
                 machine-checked ensures, general in every law parameter",
                machine.name,
                trait_definition.name,
                requirement.name,
                display_structural_term(&law_left),
                display_structural_term(&law_right),
            )));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_proposition_law_conformance(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    explicit_trait_arguments: &[psi_typed_trees::types::TypeReferenceHandle],
    proposition_laws: &[&psi_typed_trees::proposition::PropositionApplication],
    proven_propositions: &[&psi_typed_trees::proposition::PropositionApplication],
    proven_expressions: &[ExpressionHandle],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if proposition_laws.is_empty() {
        return;
    }
    let Some(entry_state) = program.machine_states(machine).first() else {
        return;
    };
    let substitutions = program
        .state_signature_parameters(requirement)
        .iter()
        .zip(program.state_parameters(entry_state))
        .map(|(required, actual)| {
            (
                required.symbol,
                required.name.as_str().to_owned(),
                actual.name.as_str().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    let trait_parameters = program.trait_type_parameters(trait_definition);

    for law in proposition_laws {
        let mut instantiated = (*law).clone();
        if program.symbols.get(law.proposition).kind
            == psi_symbols::SymbolKind::PropositionParameter
        {
            let Some(parameter_index) = trait_parameters
                .iter()
                .position(|parameter| parameter.symbol == law.proposition)
            else {
                continue;
            };
            let Some(argument) = explicit_trait_arguments.get(parameter_index) else {
                continue;
            };
            let psi_typed_trees::types::TypeReferenceNode::Named { symbol, name } =
                program.type_reference_table.type_reference(*argument)
            else {
                continue;
            };
            instantiated.proposition = *symbol;
            instantiated.name = name.clone();
        }
        let binder_labels = if instantiated.binder_arguments.is_empty() {
            match synthesize_indexed_law_binder_labels(
                program,
                law,
                &instantiated,
                requirement,
                entry_state,
            ) {
                Some(labels) => labels,
                None => {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` satisfies `{}::{}` but the selected proposition family `{}` requires an indexed binder telescope that cannot be synthesized from the law's representative parameters",
                        machine.name,
                        trait_definition.name,
                        requirement.name,
                        instantiated.name,
                    )));
                    continue;
                }
            }
        } else {
            instantiated
                .binder_arguments
                .iter()
                .map(|argument| argument.display_name())
                .collect::<Vec<_>>()
        };
        let argument_labels = program
            .expression_table
            .expression_handles(instantiated.arguments)
            .iter()
            .map(|argument| {
                program.render_proof_expression_with_parameters(*argument, &substitutions)
            })
            .collect::<Vec<_>>();
        let Some(expected_formula) = program.normalize_proposition_application_with_labels(
            &instantiated,
            &binder_labels,
            &argument_labels,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but its proposition law `{}` does not normalize after trait-family and indexed-binder substitution",
                machine.name,
                trait_definition.name,
                requirement.name,
                instantiated.name,
            )));
            continue;
        };
        let expected = expected_formula.identity_label();
        let expected_nominal = program.normalize_nominal_proposition_application_with_labels(
            &instantiated,
            &binder_labels,
            &argument_labels,
        );
        let matched = if let Some(expected_nominal) = expected_nominal {
            proven_propositions.iter().any(|proven| {
                program
                    .normalize_nominal_proposition_application(proven)
                    .is_some_and(|actual| actual == expected_nominal)
            })
        } else if let psi_typed_trees::proposition::NormalizedPropositionFormula::Boolean {
            label: expected_boolean,
        } = &expected_formula
        {
            proven_propositions.iter().any(|proven| {
                matches!(
                    program.normalize_proposition_application(proven),
                    Some(psi_typed_trees::proposition::NormalizedPropositionFormula::Boolean {
                        label,
                    }) if label == *expected_boolean
                )
            }) || proven_expressions.iter().any(|proven| {
                program.render_proof_expression_with_symbols(*proven, &[]) == *expected_boolean
            })
        } else {
            false
        };
        if !matched {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` satisfies `{}::{}` but proves no ensures matching proposition law `{expected}` after trait-family substitution",
                machine.name, trait_definition.name, requirement.name,
            )));
        }
    }
}

fn synthesize_indexed_law_binder_labels(
    program: &TypedTrees,
    authored_law: &psi_typed_trees::proposition::PropositionApplication,
    instantiated_law: &psi_typed_trees::proposition::PropositionApplication,
    requirement: &StateSignature,
    entry_state: &psi_typed_trees::state::State,
) -> Option<Vec<String>> {
    let declaration = program
        .propositions()
        .iter()
        .find(|declaration| declaration.symbol == instantiated_law.proposition)?;
    let binder_count = program.proposition_binders(declaration).len();
    if binder_count == 0 {
        return Some(Vec::new());
    }
    let required_parameters = program.state_signature_parameters(requirement);
    let actual_parameters = program.state_parameters(entry_state);
    let mut labels = Vec::with_capacity(binder_count);
    for argument in program
        .expression_table
        .expression_handles(authored_law.arguments)
    {
        let required_index =
            proposition_law_parameter_index(program, *argument, required_parameters)?;
        let actual = actual_parameters.get(required_index)?;
        let generic_arguments = generic_type_argument_handles(program, actual.type_reference)?;
        labels.extend(
            generic_arguments
                .iter()
                .map(|argument| program.display_type_reference(*argument)),
        );
    }
    (labels.len() == binder_count).then_some(labels)
}

fn proposition_law_parameter_index(
    program: &TypedTrees,
    argument: ExpressionHandle,
    parameters: &[psi_typed_trees::signature::StateParameter],
) -> Option<usize> {
    let ExpressionNode::Name(path) = program.expression_table.expression(argument) else {
        return None;
    };
    let name = program
        .expression_table
        .name_path_members(path.members)
        .last()?;
    parameters.iter().position(|parameter| {
        (path.symbol.is_valid() && parameter.symbol == path.symbol)
            || parameter.name.as_str() == name.as_str()
    })
}

fn generic_type_argument_handles(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<&[psi_typed_trees::types::TypeReferenceHandle]> {
    match program.type_reference_table.type_reference(type_reference) {
        psi_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            generic_type_argument_handles(program, *referee)
        }
        psi_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            generic_type_argument_handles(program, *base_type)
        }
        psi_typed_trees::types::TypeReferenceNode::Generic { arguments, .. } => Some(
            program
                .type_reference_table
                .type_reference_handles(*arguments),
        ),
        _ => None,
    }
}

/// Forall-to-forall sharpening: every law parameter must bind to a DISTINCT
/// plain parameter VARIABLE of the satisfier -- binding two law parameters to
/// one satisfier parameter (or to a compound term) proves only a weaker
/// instance of the law.
fn bindings_are_forall_general(
    bindings: &[(String, StructuralTerm)],
    satisfier_parameters: &[String],
) -> bool {
    let mut seen: Vec<&String> = Vec::new();
    for (_, bound) in bindings {
        let StructuralTerm::Variable(name) = bound else {
            return false;
        };
        if !satisfier_parameters
            .iter()
            .any(|parameter| parameter == name)
        {
            return false;
        }
        if seen.iter().any(|previous| *previous == name) {
            return false;
        }
        seen.push(name);
    }
    true
}

/// Split an ensures fact into its `==` conjuncts (And-chains recursively).
pub(super) fn collect_equality_conjuncts(
    program: &TypedTrees,
    expression: ExpressionHandle,
    out: &mut Vec<ExpressionHandle>,
) {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return;
    };
    match binary.operator {
        BinaryOperator::And => {
            collect_equality_conjuncts(program, binary.left, out);
            collect_equality_conjuncts(program, binary.right, out);
        }
        BinaryOperator::Equal => out.push(expression),
        _ => {}
    }
}

/// The CARRIER's op-slot bindings: for each requirement of the trait, the
/// machine conforming to it whose carrier type matches. Alias preference
/// (plural algebras): a binding sharing the checking conformance's alias
/// wins; otherwise unaliased bindings win; a remaining tie is ambiguous and
/// reported.
fn carrier_slot_bindings(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
    carrier: psi_typed_trees::types::TypeReferenceHandle,
    prefer_alias: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(String, String)> {
    let mut bindings: Vec<(String, String)> = Vec::new();

    for requirement in program.trait_machine_signatures(trait_definition) {
        // (slot machine name, alias) candidates for this carrier.
        let mut candidates: Vec<(String, Option<String>)> = Vec::new();
        for candidate in program.machines() {
            for conformance in program.machine_trait_conformances(candidate) {
                if conformance.symbol != trait_definition.symbol {
                    continue;
                }
                let bound_requirement = conformance
                    .requirement
                    .as_ref()
                    .map(|name| name.as_str().to_owned())
                    .or_else(|| {
                        candidate
                            .attached_data
                            .is_none()
                            .then(|| candidate.name.as_str().to_owned())
                    });
                if bound_requirement.as_deref() != Some(requirement.name.as_str()) {
                    continue;
                }
                let Some(candidate_entry) = program.machine_states(candidate).first() else {
                    continue;
                };
                let candidate_carrier = program
                    .state_parameters(candidate_entry)
                    .first()
                    .map(|parameter| parameter.type_reference)
                    .unwrap_or(candidate_entry.return_type);
                if !crate::type_references::type_references_match(
                    program,
                    candidate_carrier,
                    carrier,
                ) {
                    continue;
                }
                candidates.push((
                    candidate.name.as_str().to_owned(),
                    conformance
                        .alias
                        .as_ref()
                        .map(|alias| alias.as_str().to_owned()),
                ));
            }
        }

        if candidates.is_empty() {
            continue; // an unbound slot only matters if a law mentions it
        }
        let chosen = if let Some(preferred) = candidates
            .iter()
            .filter(|(_, alias)| alias.as_deref() == prefer_alias)
            .collect::<Vec<_>>()
            .split_first()
            .filter(|(_, rest)| rest.is_empty())
            .map(|(first, _)| (*first).clone())
        {
            Some(preferred)
        } else {
            let unaliased: Vec<_> = candidates
                .iter()
                .filter(|(_, alias)| alias.is_none())
                .collect();
            match unaliased.as_slice() {
                [single] => Some((*single).clone()),
                [] if candidates.len() == 1 => Some(candidates[0].clone()),
                [] => None,
                _ => None,
            }
        };
        match chosen {
            Some((machine_name, _)) => {
                bindings.push((requirement.name.as_str().to_owned(), machine_name));
            }
            None => {
                diagnostics.push(Diagnostic::error(format!(
                    "trait `{}` requirement `{}` has AMBIGUOUS satisfiers for this carrier -- \
                     name the family with `as <Alias>` on each conformance so the law check \
                     (and the judge) can pick one",
                    trait_definition.name, requirement.name,
                )));
            }
        }
    }

    bindings
}

/// Rewrite the law's op-slot applications (`add(a, b)` where `add` is a
/// requirement of the SAME trait) to the carrier's bound machine names;
/// slots with no binding are collected for the missing-slot diagnostic.
fn rewrite_slot_applications(
    term: &StructuralTerm,
    slot_names: &[String],
    slot_bindings: &[(String, String)],
    missing: &mut Vec<String>,
) -> StructuralTerm {
    match term {
        StructuralTerm::Application { machine, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| {
                    rewrite_slot_applications(argument, slot_names, slot_bindings, missing)
                })
                .collect();
            let machine = if slot_names.iter().any(|slot| slot == machine) {
                match slot_bindings
                    .iter()
                    .find(|(slot, _)| slot == machine)
                    .map(|(_, bound)| bound.clone())
                {
                    Some(bound) => bound,
                    None => {
                        missing.push(machine.clone());
                        machine.clone()
                    }
                }
            } else {
                machine.clone()
            };
            StructuralTerm::Application { machine, arguments }
        }
        StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
            data: data.clone(),
            case: case.clone(),
            fields: fields
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        rewrite_slot_applications(value, slot_names, slot_bindings, missing),
                    )
                })
                .collect(),
        },
        other => other.clone(),
    }
}

pub(super) fn term_mentions_variable(term: &StructuralTerm, variable: &String) -> bool {
    match term {
        StructuralTerm::Variable(name) => name == variable,
        StructuralTerm::Constructor { fields, .. } => fields
            .iter()
            .any(|(_, value)| term_mentions_variable(value, variable)),
        StructuralTerm::Application { arguments, .. } => arguments
            .iter()
            .any(|argument| term_mentions_variable(argument, variable)),
        StructuralTerm::Opaque(_) => false,
    }
}

/// First-order matching for the SUGGESTION diagnostic only (the proving
/// path never pattern-matches -- citations instantiate at written
/// operands): occurrences of `variables` in `pattern` bind consistently
/// against the goal's subterms; everything else must agree exactly.
pub(super) fn diagnostic_shape_match(
    pattern: &StructuralTerm,
    term: &StructuralTerm,
    variables: &[String],
    bindings: &mut Vec<(String, StructuralTerm)>,
) -> bool {
    match (pattern, term) {
        (StructuralTerm::Variable(name), _) if variables.iter().any(|v| v == name) => {
            if let Some((_, bound)) = bindings.iter().find(|(n, _)| n == name) {
                bound == term
            } else {
                bindings.push((name.clone(), term.clone()));
                true
            }
        }
        (StructuralTerm::Variable(left), StructuralTerm::Variable(right)) => left == right,
        (
            StructuralTerm::Constructor { data, case, fields },
            StructuralTerm::Constructor {
                data: data_t,
                case: case_t,
                fields: fields_t,
            },
        ) => {
            data == data_t
                && case == case_t
                && fields.len() == fields_t.len()
                && fields
                    .iter()
                    .zip(fields_t)
                    .all(|((name, value), (name_t, value_t))| {
                        name == name_t
                            && diagnostic_shape_match(value, value_t, variables, bindings)
                    })
        }
        (
            StructuralTerm::Application { machine, arguments },
            StructuralTerm::Application {
                machine: machine_t,
                arguments: arguments_t,
            },
        ) => {
            machine == machine_t
                && arguments.len() == arguments_t.len()
                && arguments
                    .iter()
                    .zip(arguments_t)
                    .all(|(argument, argument_t)| {
                        diagnostic_shape_match(argument, argument_t, variables, bindings)
                    })
        }
        (StructuralTerm::Opaque(left), StructuralTerm::Opaque(right)) => left == right,
        _ => false,
    }
}

/// Render a term back into citation-argument spelling.
pub(super) fn display_structural_term(term: &StructuralTerm) -> String {
    match term {
        StructuralTerm::Variable(name) => name.clone(),
        StructuralTerm::Constructor { data, case, fields } => {
            if fields.is_empty() {
                format!("{data}::{case}")
            } else {
                let rendered: Vec<String> = fields
                    .iter()
                    .map(|(name, value)| format!("{name}: {}", display_structural_term(value)))
                    .collect();
                format!("{data}::{case} {{ {} }}", rendered.join(", "))
            }
        }
        StructuralTerm::Application { machine, arguments } => {
            let rendered: Vec<String> = arguments.iter().map(display_structural_term).collect();
            format!("{machine}({})", rendered.join(", "))
        }
        StructuralTerm::Opaque(display) => display.clone(),
    }
}
