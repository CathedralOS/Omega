/*
    Contract checks: each call's `requires`, each exit's `ensures`, each
    self-transition arrival requirement and each write into a domain-refined
    field must be proved from the checked facts at that point.

    Two entries, both called by
    `checks::check_checked_facts_recording`:
    `bind_call_evidence_arguments` runs first among the checks and binds the
    evidence arguments of contract expressions and calls into the proof facts
    (`evidence`); `check_flow_call_contracts` runs after the borrow check.

    `check_flow_call_contracts` builds the content conservation plans and the
    declared field requirements of nominal parameters (`nominal_inputs`),
    checks every state's `asm` requires and ensures assertions (`assembly`),
    and prepares the cyclic header invariants (`exits`). The checks entry owns
    the exit entailment cache (`entailment`), shared with content composition
    so both consumers use the same preserved positive judgments. This check
    then walks each state's flow:

    1. for each call: the scalar tail result domains (`exits`) and the call's
       requirements (`calls`, which also checks the nominal inputs);
    2. for each exit: the scalar result domains, the `ensures` clauses, the
       result field domains and the field domains of readable `&mut`
       referents (`exits`);
    3. writes into domain-refined fields (`writes`).

    The other children are not steps. `prover` decides whether fact contexts
    prove a Boolean expression or a contract fact, and reaches `direct`,
    `domains` and `evaluator`. `calls` also consults `call_bounds`,
    `guard_operands`, `intervals` and `reference_domains`; `exits` consults
    `content_preservation` and `integer_embeddings`; both consult
    `entailment`. `labels`, `places` and `return_values` are shared
    vocabulary: contract labels, place matching and exit return occurrences.
*/

/*
    The steps: evidence binding, and the checks `check_flow_call_contracts`
    runs.
*/
mod argument_intervals {
    /*
        Call requirements proved from the arithmetic walk's argument intervals.

        Validation's arithmetic walk checks every call argument in the branch-local
        environment it is evaluated in, where an `&&` or `||` operand and refuted
        earlier transition arms already narrow the values. A requirement that
        compares one integer parameter with a literal (`requires index <= 4`) holds
        at a call when every value the walk allows for that argument satisfies it:
        `sum(values, index + 1, ..)` after a refuted `index >= 4 ->` arm passes
        `index + 1` in `[1, 4]`. Conjunctions of such comparisons hold when each
        does. Every other requirement keeps its existing provers; this is a client
        of the one interval engine rather than another private one. The call capture
        owner supplies the exact completed actual for each demanded formal; this
        consumer never reconstructs source arguments or evaluates them again.
    */
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

    pub(super) fn proves(
        program: &dyn ::validation::CheckingInput,
        requirement: ExpressionHandle,
        argument_for: &impl Fn(ExpressionHandle) -> Option<ExpressionHandle>,
    ) -> bool {
        let Some(binary) = program.expression_table.binary_node(requirement) else {
            return false;
        };
        if binary.operator == BinaryOperator::And {
            return proves(program, binary.left, argument_for)
                && proves(program, binary.right, argument_for);
        }
        let literal = |side: ExpressionHandle| match program.expression_table.expression(side) {
            ExpressionNode::Integer(value) => value.value_i64(),
            _ => None,
        };
        let (parameter, operator, bound) = if let Some(bound) = literal(binary.right) {
            (binary.left, binary.operator, bound)
        } else if let Some(bound) = literal(binary.left) {
            let mirrored = binary.operator.mirrored();
            (binary.right, mirrored, bound)
        } else {
            return false;
        };
        let Some(argument) = argument_for(parameter) else {
            return false;
        };
        let Some(interval) = program.expression_interval(argument) else {
            return false;
        };
        match operator {
            BinaryOperator::Less => interval.high.is_some_and(|high| high < bound),
            BinaryOperator::LessOrEqual => interval.high.is_some_and(|high| high <= bound),
            BinaryOperator::Greater => interval.low.is_some_and(|low| low > bound),
            BinaryOperator::GreaterOrEqual => interval.low.is_some_and(|low| low >= bound),
            BinaryOperator::Equal => interval.low == Some(bound) && interval.high == Some(bound),
            BinaryOperator::NotEqual => {
                interval.high.is_some_and(|high| high < bound)
                    || interval.low.is_some_and(|low| low > bound)
            }
            _ => false,
        }
    }

    /*
        A captured integer actual can constrain an exact deferred contract recipe.
        Retention is conditional evidence: every recorded relation must discharge
        under the selected target before Terminal production. A partial conjunction
        publishes nothing, and reference contents never use an argument-time hull.
    */
    pub(super) fn recipe_requirements(
        program: &dyn ::validation::CheckingInput,
        owner: symbols::SymbolHandle,
        recipe_owner: symbols::SymbolHandle,
        requirement: ExpressionHandle,
        argument_for: &impl Fn(ExpressionHandle) -> Option<ExpressionHandle>,
        description: &str,
    ) -> Option<Vec<checked_trees::ConstantRecipeObligation>> {
        use checked_trees::{ConstantRecipeObligation, ConstantRecipeRequirement};
        use numerics::bignum::BigInt;
        if matches!(
            program.expression_table.expression(requirement),
            ExpressionNode::Boolean(true)
        ) || proves(program, requirement, argument_for)
        {
            return Some(Vec::new());
        }
        let binary = program.expression_table.binary_node(requirement)?;
        if binary.operator == BinaryOperator::And {
            let mut left = recipe_requirements(
                program,
                owner,
                recipe_owner,
                binary.left,
                argument_for,
                description,
            )?;
            left.extend(recipe_requirements(
                program,
                owner,
                recipe_owner,
                binary.right,
                argument_for,
                description,
            )?);
            return Some(left);
        }
        let right_recipe = program.constant_recipe_for_contract_expression(binary.right);
        let left_recipe = program.constant_recipe_for_contract_expression(binary.left);
        let (recipe, subject, operator) = if right_recipe.is_valid() {
            (right_recipe, binary.left, binary.operator)
        } else if left_recipe.is_valid() {
            (left_recipe, binary.right, binary.operator.mirrored())
        } else {
            return None;
        };
        let interval = {
            let argument = argument_for(subject)?;
            program.expression_interval(argument)?
        };
        let relation = match operator {
            BinaryOperator::Less => ConstantRecipeRequirement::AtLeast(
                BigInt::from_i64(interval.high?).add(&BigInt::from_i64(1)),
            ),
            BinaryOperator::LessOrEqual => {
                ConstantRecipeRequirement::AtLeast(BigInt::from_i64(interval.high?))
            }
            BinaryOperator::Greater => ConstantRecipeRequirement::AtMost(
                BigInt::from_i64(interval.low?).sub(&BigInt::from_i64(1)),
            ),
            BinaryOperator::GreaterOrEqual => {
                ConstantRecipeRequirement::AtMost(BigInt::from_i64(interval.low?))
            }
            BinaryOperator::Equal => {
                return Some(vec![
                    ConstantRecipeObligation::retain(
                        owner,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        recipe,
                        ConstantRecipeRequirement::AtLeast(BigInt::from_i64(interval.high?)),
                        program.expression_table.source_span(requirement),
                        description.to_owned(),
                    )
                    .with_recipe_owner(recipe_owner),
                    ConstantRecipeObligation::retain(
                        owner,
                        semantic_vocabulary::ScalarConditionRole::Prerequisite,
                        recipe,
                        ConstantRecipeRequirement::AtMost(BigInt::from_i64(interval.low?)),
                        program.expression_table.source_span(requirement),
                        description.to_owned(),
                    )
                    .with_recipe_owner(recipe_owner),
                ]);
            }
            _ => return None,
        };
        Some(vec![
            ConstantRecipeObligation::retain(
                owner,
                semantic_vocabulary::ScalarConditionRole::Prerequisite,
                recipe,
                relation,
                program.expression_table.source_span(requirement),
                description.to_owned(),
            )
            .with_recipe_owner(recipe_owner),
        ])
    }
}
mod assembly {
    use checked_trees::expression::{ExpressionHandle, ExpressionNode};
    use checked_trees::{CheckFacts, FlowStateFact};
    use diagnostics::Diagnostic;
    use symbols::JoinedPath;
    use typed_trees::statement::{AssemblyFactKind, StatementNode};
    use validation::CanonicalPlace;

    use super::prover::{
        live_contexts_prove_boolean_expression, semantic_contexts_prove_boolean_expression,
    };
    use crate::published::labels::machine_name;
    use crate::published::places::place_at;

    /*
        Discharge each assembly assertion against ordinary invalidation-adjusted
        flow contexts. Entry assertions precede instruction evaluation. Exit
        assertions follow the instructions, including argument evaluation on an
        enclosing machine edge. Assertions consume facts; they never add premises.
    */
    pub(super) fn check_assembly_fact_contracts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for (_, state_flow) in facts.flow.control.states.iter() {
            check_state_assembly_facts(program, facts, state_flow, diagnostics);
        }
    }

    fn check_state_assembly_facts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some((machine, state)) = validation::machine_state_by_symbols(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        ) else {
            return;
        };

        let predicates = validation::StatementPredicateObservations::new(program, machine, state);
        let statements = program.statement_table.statements(state.statement_nodes);
        for (statement_index, statement) in statements.iter().enumerate() {
            let Some(fact) = statement.as_assembly_fact() else {
                continue;
            };
            let Some(statement_flow) = facts.flow.state_statement(state_flow, statement_index)
            else {
                continue;
            };
            let invalidated_inside_block = fact.kind == AssemblyFactKind::Ensures
                && asm_block_writes_fact_place(
                    program,
                    state.symbol,
                    statements,
                    statement_index,
                    fact.fact,
                );
            let transfer = (fact.kind == AssemblyFactKind::Ensures)
                .then(|| enclosing_transfer_for_fact(program, statements, statement_index))
                .flatten();
            let observations: Vec<_> = if let Some(transfer) = transfer {
                facts
                    .flow
                    .control
                    .outgoing
                    .span_or_empty(state_flow.outgoing)
                    .iter()
                    .filter(|edge| edge.statement_index == transfer)
                    .map(|edge| (transfer, edge.constraints))
                    .collect()
            } else {
                vec![(statement_index, statement_flow.entry_constraints)]
            };
            if !observations.is_empty()
                && observations.iter().all(|(observation, constraints)| {
                    let contexts = facts
                        .flow
                        .semantic_constraint_contexts(*constraints)
                        .collect::<Vec<_>>();
                    assembly_fact_is_proven(
                        program,
                        facts,
                        state,
                        *observation,
                        &contexts,
                        fact.fact,
                        !invalidated_inside_block,
                    ) || matches!(
                        program.proof_facts.get(fact.fact),
                        typed_trees::domain::ProofFact::Expression(expression)
                            if predicates.proves(*observation, *expression)
                    )
                })
            {
                continue;
            }

            let (kind, point) = match fact.kind {
                AssemblyFactKind::Requires => ("requires", "block entry"),
                AssemblyFactKind::Ensures => ("ensures", "block exit"),
            };
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove asm `{kind}` fact at {point} in {} state `{}` (statement {statement_index}): {}",
                machine_name(program, state_flow.machine_symbol),
                state.name,
                assembly_fact_label(program, fact.fact),
            )));
        }
    }

    /*
        The enclosing machine transfer is retained as the label's final ordinary
        instruction. Local assembly jumps have no argument evaluation of their own.
    */
    fn enclosing_transfer_for_fact(
        program: &dyn ::validation::CheckingInput,
        statements: &[StatementNode],
        fact: usize,
    ) -> Option<usize> {
        use typed_trees::statement::{AssemblyControlPoint, AssemblyExit};
        let (entry, graph) =
            statements[..fact]
                .iter()
                .enumerate()
                .rev()
                .find_map(|(index, statement)| {
                    let StatementNode::Assembly(AssemblyControlPoint::Entry(graph)) = statement
                    else {
                        return None;
                    };
                    Some((index, program.statement_table.assembly(*graph)))
                })?;
        let offset = fact.checked_sub(entry + 1)?;
        let block = program
            .statement_table
            .assembly_blocks(graph.blocks)
            .iter()
            .find(|block| {
                (block.start as usize..block.start as usize + block.count as usize)
                    .contains(&offset)
            })?;
        matches!(block.exit, AssemblyExit::EnclosingTransition)
            .then_some(entry + block.start as usize + block.count as usize)
    }

    /*
        The general flow engine invalidates domain-derived facts across assignments,
        but direct boolean constraints can still be present in an exit context. An
        asm postcondition must not reuse such a stale entry constraint after an
        instruction wrote one of the places it reads.
    */
    fn asm_block_writes_fact_place(
        program: &dyn ::validation::CheckingInput,
        state_symbol: symbols::SymbolHandle,
        statements: &[StatementNode],
        ensures_index: usize,
        proof_fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> bool {
        let mut instruction_end = ensures_index;
        while instruction_end > 0
            && matches!(
                statements[instruction_end - 1],
                StatementNode::AssemblyFact(ref fact) if fact.kind == AssemblyFactKind::Ensures
            )
        {
            instruction_end -= 1;
        }

        let mut instruction_start = instruction_end;
        while instruction_start > 0 {
            match &statements[instruction_start - 1] {
                StatementNode::Assembly(_) => break,
                StatementNode::AssemblyFact(fact) if fact.kind == AssemblyFactKind::Requires => {
                    break;
                }
                StatementNode::AssemblyFact(_) => return false,
                _ => instruction_start -= 1,
            }
        }

        statements[instruction_start..instruction_end]
            .iter()
            .enumerate()
            .filter_map(|(offset, statement)| {
                let assignment = statement.as_assignment()?;
                place_at(
                    program,
                    state_symbol,
                    instruction_start + offset,
                    assignment.target,
                )
            })
            .any(|written_place| {
                program
                    .tables
                    .proof_fact_expression_roots(proof_fact)
                    .into_iter()
                    .any(|expression| {
                        expression_reads_overlapping_place(
                            program,
                            state_symbol,
                            ensures_index,
                            expression,
                            &written_place,
                        )
                    })
            })
    }

    pub(super) fn expression_reads_overlapping_place(
        program: &dyn ::validation::CheckingInput,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        written_place: &CanonicalPlace,
    ) -> bool {
        use typed_trees::SubtreeVisit;
        let edges = typed_trees::SubtreeEdges {
            pattern_dependencies: true,
            ..typed_trees::SubtreeEdges::VALUES
        };
        program.subtree_any(expression, edges, |handle, node| {
            let overlaps = || {
                expression_place_may_overlap(
                    program,
                    state_symbol,
                    statement_index,
                    handle,
                    written_place,
                )
            };
            match node {
                ExpressionNode::SavedSubject(saved) => program
                    .saved_value_source(saved)
                    .map_or(SubtreeVisit::Found, SubtreeVisit::Instead),
                ExpressionNode::Name(_) | ExpressionNode::Member(_) if overlaps() => {
                    SubtreeVisit::Found
                }
                ExpressionNode::Name(_) | ExpressionNode::Member(_) => SubtreeVisit::Prune,
                ExpressionNode::Indexed(_) if overlaps() => SubtreeVisit::Found,
                ExpressionNode::Indexed(indexed) => SubtreeVisit::Instead(indexed.index),
                _ => SubtreeVisit::Descend,
            }
        })
    }

    fn expression_place_may_overlap(
        program: &dyn ::validation::CheckingInput,
        state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        written_place: &CanonicalPlace,
    ) -> bool {
        place_at(program, state_symbol, statement_index, expression).is_some_and(|read_place| {
            ::validation::storage_path_relations::canonical_places_may_overlap(
                program,
                &read_place,
                written_place,
            )
        })
    }

    fn assembly_fact_label(
        program: &dyn validation::CheckingInput,
        handle: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> String {
        match program.proof_facts.get(handle) {
            typed_trees::domain::ProofFact::Expression(expression) => {
                program.expression_table.display_name(*expression)
            }
            typed_trees::domain::ProofFact::Proposition(application) => program
                .normalize_proposition_application(application, None)
                .map(|formula| formula.identity_label())
                .unwrap_or_else(|| application.name.to_string()),
            typed_trees::domain::ProofFact::Membership(membership) => format!(
                "{} in {}",
                program.expression_table.display_name(membership.value),
                program.domain_path_members(membership.domain).joined_path()
            ),
        }
    }
    fn assembly_fact_is_proven(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        statement: usize,
        contexts: &[facts::FactContextHandle],
        handle: arena::Handle<typed_trees::domain::ProofFact>,
        entry_preserved: bool,
    ) -> bool {
        let Some(machine) = program.machine_holding_state(state.symbol) else {
            return false;
        };
        match program.proof_facts.get(handle) {
            typed_trees::domain::ProofFact::Expression(expression) => {
                live_contexts_prove_boolean_expression(
                    program,
                    facts,
                    state,
                    statement,
                    contexts,
                    *expression,
                ) || (entry_preserved
                    && semantic_contexts_prove_boolean_expression(
                        program,
                        &facts.semantic,
                        contexts,
                        facts::ProgramPoint::Statement {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            statement_index: statement,
                        },
                        *expression,
                    ))
            }
            typed_trees::domain::ProofFact::Proposition(application) => {
                (entry_preserved
                    && program
                        .normalize_proposition_application(application, None)
                        .is_some_and(|formula| {
                            super::prover::contexts_prove_proposition_label(
                                &facts.semantic,
                                program,
                                contexts,
                                &formula.identity_label(),
                            )
                        }))
                    || super::prover::instruction_effect_proven(
                        program,
                        facts,
                        state.symbol,
                        statement,
                        contexts,
                        application,
                    )
                    || super::prover::live_contexts_prove_proposition_application(
                        program,
                        facts,
                        state,
                        statement,
                        contexts,
                        application,
                    )
            }
            typed_trees::domain::ProofFact::Membership(membership) => {
                let Some(place) = place_at(program, state.symbol, statement, membership.value)
                else {
                    return false;
                };
                if membership.domain_arguments.is_empty() {
                    return super::prover::prove_domain_at_place(
                        program,
                        &facts.semantic,
                        contexts,
                        &place,
                        membership.domain_symbol,
                    );
                }
                membership.semantic_domain.is_valid() && contexts.iter().any(|context| facts.semantic.context_view(facts.semantic.contexts.get(*context)).facts().any(|candidate| {
                    matches!(candidate.payload, facts::domain_membership_pattern!(domain_symbol, semantic_domain) if domain_symbol == membership.domain_symbol && semantic_domain == membership.semantic_domain)
                    && candidate.place.as_place().and_then(|handle| crate::flow::canonical_place_from_semantic_place(program, &facts.semantic, facts.semantic.places.get(handle))).is_some_and(|candidate| candidate == place)
                }))
            }
        }
    }
}
#[path = "checks_contracts_calls.rs"]
pub(crate) mod calls;
pub(crate) mod contextual_cleanup {
    /*
        Bound generic cleanup inputs are ordinary call obligations. Their origin is
        the retained generic death row, not an authored `requires` clause; only the
        actual's evaluated operand facts can discharge the instantiated predicate.
    */
    use crate::flow::expressions;
    use validation::CanonicalPlace;

    use checked_trees::{
        CheckFacts, CheckedBooleanExpression, ContextualCleanupEntryRequirement, FlowCallFact,
        FlowStateFact,
    };
    use diagnostics::Diagnostic;
    use facts::FactContextHandle;

    pub(crate) use super::call_values::target_machine;

    pub(crate) fn actual_place(
        program: &dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        call: &FlowCallFact,
        requirement: &ContextualCleanupEntryRequirement,
    ) -> Option<CanonicalPlace> {
        if target_machine(program, call)? != requirement.machine {
            return None;
        }
        let predicate = crate::published::facts::boolean_predicates::bind_entry(
            program,
            requirement.machine,
            requirement.consequence.clone(),
        )?;
        let observation = predicate.bindings.first()?;
        let actual = super::call_values::observation_actual(program, state, call, observation)?;
        crate::published::semantic::facts::projected_argument_place(
            program,
            state.state_symbol,
            call.statement_index,
            actual.argument?,
            actual.parameter.type_reference,
            &actual.path,
        )
    }

    pub(crate) fn entry_consequence(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        requirement: &ContextualCleanupEntryRequirement,
    ) -> Option<CheckedBooleanExpression> {
        super::call_values::entry_predicate(
            program,
            facts,
            state,
            call,
            requirement.machine,
            requirement.consequence.clone(),
        )
    }

    /* Preserve the selector as a formula over authenticated caller entry values. */
    pub(crate) fn entry_antecedent(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        requirement: &ContextualCleanupEntryRequirement,
    ) -> Option<CheckedBooleanExpression> {
        super::call_values::entry_predicate(
            program,
            facts,
            state,
            call,
            requirement.machine,
            requirement.antecedent.clone(),
        )
    }

    pub(crate) fn proves(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[FactContextHandle],
        requirement: &ContextualCleanupEntryRequirement,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        super::call_values::proves_entry_predicate(
            program,
            facts,
            state,
            call,
            contexts,
            requirement.machine,
            requirement.predicate(),
            frames,
        )
    }

    pub(super) fn check(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[FactContextHandle],
        frames: Option<&validation::CallFrameResolver<'_>>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let requirements = &facts.contextual_cleanup.entry_requirements;
        if requirements.rows.is_empty() {
            return;
        }
        let Some(target) = target_machine(program, call) else {
            return;
        };
        for (_, requirement) in requirements
            .rows
            .iter()
            .filter(|(_, row)| row.machine == target)
        {
            if proves(program, facts, state, call, contexts, requirement, frames) {
                continue;
            }
            let subject = actual_place(program, state, call, requirement)
                .map(|place| place.label(program))
                .unwrap_or_else(|| "selected owned value".to_owned());
            let proposition = requirement
                .direct_boolean_subject()
                .map(|(_, _, expected)| format!("{subject} == {expected} required by"))
                .unwrap_or_else(|| format!("{subject} does not establish"));
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove contextual cleanup prerequisite for call {} from {}: {} {} clause `{}`",
                crate::published::labels::call_target_label(program, call.target_symbol),
                crate::published::labels::machine_name(program, state.machine_symbol),
                proposition,
                crate::published::labels::machine_name(program, requirement.hook_machine),
                expressions(program).display_name(requirement.hook_clause),
            )));
        }
    }
}
mod evidence {
    mod runtime_calls {
        /*
            Erased lanes on ordinary runtime calls share the proof-output invocation
            owner. Checked source terms remain separate from executable arguments; a
            scalar operand does not lose its proof inputs merely because it has no
            source statement that binds proof outputs. Ignored outputs retain the whole
            callee lane and introduce no caller-local term or runtime storage.
        */
        use checked_trees::{
            ContractCallFact, ContractProofFactKind, ContractProofFactOwner, ProofFacts,
            ProofOutputCallFact, ProofOutputEvidenceArgumentFact, ProofOutputFact,
            ProofOutputRuntimeCallFact,
        };

        pub(super) fn retain(
            program: &dyn validation::CheckingInput,
            proof: &mut ProofFacts,
            call: &ContractCallFact,
            site: &validation::CallSite<'_>,
            arguments: Vec<ProofOutputEvidenceArgumentFact>,
        ) -> Result<(), &'static str> {
            let mut matching = proof
                .proof_output_calls
                .iter()
                .map(|(_, row)| row)
                .filter(|row| {
                    row.caller_machine_symbol == call.caller_machine_symbol
                        && row.caller_state_symbol == call.caller_state_symbol
                        && row.runtime_call.is_some_and(|runtime| {
                            runtime.statement_index == call.statement_index
                                && runtime.call_ordinal == call.call_ordinal
                        })
                });
            if let Some(existing) = matching.next() {
                if existing.target_machine_symbol != call.target_machine_symbol
                    || existing.target_state_symbol != call.target_state_symbol
                    || existing.evidence_arguments != arguments
                    || matching.next().is_some()
                {
                    return Err(
                        "ordinary evidence call disagrees with its exact proof-output invocation",
                    );
                }
                return Ok(());
            }
            let mut outputs = proof
                .contract_fact_refs
                .span_or_empty(call.ensures)
                .iter()
                .filter_map(|reference| proof.contract_facts.get(reference.fact).evidence_term)
                .collect::<Vec<_>>();
            if arguments.is_empty() && outputs.is_empty() {
                return Ok(());
            }
            let owner = if call.signature.is_valid() {
                if !crate::published::proof::contracts::call_signatures::source_is_current(
                    program, proof, call, site,
                ) {
                    return Err(
                        "ordinary evidence call lost its exact current invocation contract",
                    );
                }
                crate::published::proof::contracts::call_signatures::owner(
                    &proof.call_contract_signatures,
                    call.signature,
                )
                .ok_or("ordinary evidence call lost its retained invocation-owned evidence lane")?
            } else {
                ContractProofFactOwner::Machine {
                    machine_symbol: call.target_machine_symbol,
                }
            };
            if arguments.iter().any(|argument| {
                let term = proof.evidence_terms.get(argument.callee_input);
                term.owner != owner || term.kind != ContractProofFactKind::Requires
            }) || outputs.iter().any(|output| {
                let term = proof.evidence_terms.get(*output);
                term.owner != owner || term.kind != ContractProofFactKind::Ensures
            }) {
                return Err(
                    "ordinary evidence call requires its exact invocation-owned contract lane",
                );
            }
            outputs.sort_by_key(|output| proof.evidence_terms.get(*output).lane_position);
            let mut dispositions = Vec::with_capacity(outputs.len());
            for (position, output) in outputs.into_iter().enumerate() {
                if proof.evidence_terms.get(output).lane_position != position {
                    return Err("ordinary evidence call has no complete ordered output lane");
                }
                let (proposition, label) =
                    super::instantiated_call_proposition(program, proof, call, site, output, false)
                        .ok_or("ordinary evidence call lost its exact output substitution")?;
                dispositions.push(ProofOutputFact {
                    output_position: position,
                    callee_output: output,
                    instantiated_proposition: proposition,
                    instantiated_label: label,
                    output: None,
                });
            }
            for application in arguments
                .iter()
                .map(|row| &row.instantiated_proposition)
                .chain(dispositions.iter().map(|row| &row.instantiated_proposition))
            {
                if !proof
                    .proposition_vocabulary
                    .applications
                    .contains(application)
                {
                    proof
                        .proposition_vocabulary
                        .applications
                        .push(application.clone());
                }
            }
            proof.proof_output_calls.append(ProofOutputCallFact {
                caller_machine_symbol: call.caller_machine_symbol,
                caller_state_symbol: call.caller_state_symbol,
                statement_index: call.statement_index,
                source_statement_index: call.statement_index,
                runtime_call: Some(ProofOutputRuntimeCallFact {
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                }),
                target_machine_symbol: call.target_machine_symbol,
                target_state_symbol: call.target_state_symbol,
                static_requirement_dispatch: site.static_requirement_dispatch().cloned(),
                evidence_arguments: arguments,
                outputs: dispositions,
            });
            Ok(())
        }
    }

    use crate::flow::expressions;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        CheckFacts, CheckedEvidenceTerm, CheckedPropositionApplication, ContractEvidenceArgument,
        ContractProofFactKind, ContractProofFactOwner,
    };
    use diagnostics::Diagnostic;
    use typed_trees::proposition::PropositionLabels;
    use validation::call_site_evidence_arguments;
    use validation::call_target_parameters;
    use validation::find_call_site;

    pub(super) fn bind_contract_expression_evidence_arguments(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut calls = Vec::new();
        let mut static_conformance_applications = Vec::new();
        let mut owners = Vec::new();
        for (_, contract) in facts.proof.contract_facts.iter() {
            if owners
                .iter()
                .any(|(owner, fact)| *owner == contract.owner && *fact == contract.fact)
            {
                continue;
            }
            owners.push((contract.owner, contract.fact));
        }

        for (owner, fact) in owners {
            let mut expressions = Vec::new();
            validation::append_fact_expression_roots(
                program,
                program.proof_facts.get(fact),
                &mut expressions,
            );
            let mut visited = std::collections::HashSet::new();
            while let Some(expression) = expressions.pop() {
                if !expression.is_valid() || !visited.insert(expression.arena_index()) {
                    continue;
                }
                let node = program.expression_table.expression(expression);
                program.subtree_children(node, typed_trees::SubtreeEdges::STRUCTURAL, |child| {
                    expressions.push(child);
                    false
                });
                let typed_trees::expression::ExpressionNode::Call(call) = node else {
                    continue;
                };
                for (static_argument_position, argument) in
                    call.machine_arguments.iter().enumerate()
                {
                    if program.symbols.get(argument.symbol).kind != symbols::SymbolKind::Conformance
                    {
                        continue;
                    }
                    match validation::conformance_applications::close_conformance_application(
                        program, argument,
                    ) {
                        Ok(application) => static_conformance_applications.push(
                            checked_trees::ContractExpressionStaticConformanceApplicationFact {
                                owner,
                                fact,
                                expression,
                                static_argument_position,
                                application,
                            },
                        ),
                        Err(diagnostic) => diagnostics.push(diagnostic),
                    }
                }
                if call.evidence_arguments.is_empty() || call.quotient_operation.is_some() {
                    continue;
                }
                let call_site = validation::CallSite::Expression { expression, call };
                let (
                    target_machine_symbol,
                    target_state_symbol,
                    target_parameters,
                    parameters,
                    runtime_contract,
                ) = if call.runtime_call.is_some() {
                    let (binding, contract) = match validation::call_contract_source(
                        program,
                        symbols::SymbolHandle::invalid(),
                        &call_site,
                    ) {
                        Ok(Some(validation::CallContractSource::Runtime { binding, contract })) => {
                            (binding, contract)
                        }
                        Ok(_) => {
                            diagnostics.push(Diagnostic::error(
                                "proof-expression runtime call has no exact contract row",
                            ));
                            continue;
                        }
                        Err(reason) => {
                            diagnostics.push(Diagnostic::error(reason));
                            continue;
                        }
                    };
                    (
                        symbols::SymbolHandle::invalid(),
                        symbols::SymbolHandle::invalid(),
                        program.state_signature_parameters(&contract.signature),
                        exact_runtime_evidence_parameters(facts, binding.contract),
                        Some(binding.contract),
                    )
                } else {
                    let Some(target) = call.target_symbol.is_valid().then_some(call.target_symbol)
                    else {
                        diagnostics.push(Diagnostic::error(
                            "proof-expression evidence call has no exact nominal target",
                        ));
                        continue;
                    };
                    if call.static_requirement_dispatch.is_some() {
                        continue;
                    }
                    let Some((machine, state)) =
                        crate::published::proof::contract_target_from_state_symbol(program, target)
                    else {
                        diagnostics.push(Diagnostic::error(
                            "proof-expression evidence call has no exact checked target owner",
                        ));
                        continue;
                    };
                    let Some(parameters) = call_target_parameters(program, state) else {
                        diagnostics.push(Diagnostic::error("proof-expression evidence call has no exact target parameter telescope"));
                        continue;
                    };
                    (
                        machine,
                        state,
                        parameters,
                        exact_target_evidence_parameters(facts, machine, state),
                        None,
                    )
                };
                if call.evidence_arguments.len() != parameters.len() {
                    diagnostics.push(Diagnostic::error(format!(
                        "proof-expression call supplies {} erased evidence argument{} but its named requires lane has {}",
                        call.evidence_arguments.len(),
                        if call.evidence_arguments.len() == 1 { "" } else { "s" },
                        parameters.len(),
                    )));
                    continue;
                }

                let mut bindings = Vec::with_capacity(parameters.len());
                let mut invalid = false;
                for (lane_position, (authored, parameter)) in
                    call.evidence_arguments.iter().zip(parameters).enumerate()
                {
                    if facts.proof.evidence_terms.get(parameter).lane_position != lane_position {
                        diagnostics.push(Diagnostic::error(
                            "proof-expression evidence call has no complete ordered input lane",
                        ));
                        invalid = true;
                        continue;
                    }
                    let sources =
                        exact_visible_evidence_sources(facts, owner, fact, authored.as_str());
                    let [source] = sources.as_slice() else {
                        diagnostics.push(Diagnostic::error(format!(
                            "proof-expression call resolves incoming evidence term `{authored}` to {} exact contract rows; expected one",
                            sources.len(),
                        )));
                        invalid = true;
                        continue;
                    };
                    let source = *source;
                    let Some(instantiated_proposition) = instantiate_proof_expression_parameter(
                        program,
                        facts,
                        &call_site,
                        target_parameters,
                        parameter,
                    ) else {
                        diagnostics.push(Diagnostic::error(format!(
                            "proof-expression call cannot instantiate erased requires position {lane_position}",
                        )));
                        invalid = true;
                        continue;
                    };
                    if facts.proof.evidence_terms.get(source).proposition
                        != instantiated_proposition
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "evidence term `{authored}` does not inhabit erased requires position {lane_position} of proof-expression call",
                        )));
                        invalid = true;
                        continue;
                    }
                    bindings.push(checked_trees::ContractExpressionEvidenceArgumentFact {
                        source,
                        parameter,
                        lane_position,
                        instantiated_proposition,
                    });
                }
                if !invalid {
                    calls.push(checked_trees::ContractExpressionEvidenceCallFact {
                        owner,
                        fact,
                        expression,
                        target_machine_symbol,
                        target_state_symbol,
                        runtime_contract,
                        evidence_arguments: bindings,
                    });
                }
            }
        }

        facts.proof.contract_expression_evidence_calls = calls;
        facts
            .proof
            .contract_expression_static_conformance_applications = static_conformance_applications;
    }

    fn evidence_term_belongs_to_target(
        owner: ContractProofFactOwner,
        target_machine_symbol: symbols::SymbolHandle,
        target_state_symbol: symbols::SymbolHandle,
    ) -> bool {
        matches!(
            owner,
            ContractProofFactOwner::Machine { machine_symbol }
                if machine_symbol == target_machine_symbol
        ) || matches!(
            owner,
            ContractProofFactOwner::MachineState {
                machine_symbol,
                state_symbol,
            } if machine_symbol == target_machine_symbol && state_symbol == target_state_symbol
        ) || matches!(
            owner,
            ContractProofFactOwner::StateSignature {
                owner_symbol,
                state_symbol,
            } if owner_symbol == target_machine_symbol && state_symbol == target_state_symbol
        )
    }

    pub(crate) fn exact_runtime_evidence_parameters(
        facts: &CheckFacts,
        contract: typed_trees::callable_values::RuntimeCallableContractHandle,
    ) -> Vec<Handle<CheckedEvidenceTerm>> {
        let owner = ContractProofFactOwner::RuntimeCallableContract { contract };
        let mut parameters = facts
            .proof
            .contract_facts
            .iter()
            .filter_map(|(_, fact)| {
                let term = fact.evidence_term?;
                let checked = facts.proof.evidence_terms.get(term);
                (fact.owner == owner
                    && fact.kind == ContractProofFactKind::Requires
                    && checked.owner == owner
                    && checked.kind == fact.kind)
                    .then_some(term)
            })
            .collect::<Vec<_>>();
        parameters
            .sort_by_key(|parameter| facts.proof.evidence_terms.get(*parameter).lane_position);
        parameters
    }

    pub(crate) fn exact_target_evidence_parameters(
        facts: &CheckFacts,
        target_machine_symbol: symbols::SymbolHandle,
        target_state_symbol: symbols::SymbolHandle,
    ) -> Vec<Handle<CheckedEvidenceTerm>> {
        let mut parameters = facts
            .proof
            .contract_facts
            .iter()
            .filter_map(|(_, contract)| {
                let parameter = contract.evidence_term?;
                let checked = facts.proof.evidence_terms.get(parameter);
                (contract.kind == ContractProofFactKind::Requires
                    && evidence_term_belongs_to_target(
                        contract.owner,
                        target_machine_symbol,
                        target_state_symbol,
                    )
                    && checked.owner == contract.owner
                    && checked.kind == contract.kind)
                    .then_some(parameter)
            })
            .collect::<Vec<_>>();
        parameters
            .sort_by_key(|parameter| facts.proof.evidence_terms.get(*parameter).lane_position);
        parameters
    }

    pub(crate) fn instantiate_contract_expression_evidence_parameter(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        expression: typed_trees::expression::ExpressionHandle,
        target_state_symbol: symbols::SymbolHandle,
        parameter: Handle<CheckedEvidenceTerm>,
    ) -> Option<CheckedPropositionApplication> {
        let typed_trees::expression::ExpressionNode::Call(call) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        if let Some(binding) = call.runtime_call {
            if target_state_symbol.is_valid()
                || facts.proof.evidence_terms.get(parameter).owner
                    != (ContractProofFactOwner::RuntimeCallableContract {
                        contract: binding.contract,
                    })
            {
                return None;
            }
            let site = validation::CallSite::Expression { expression, call };
            let validation::CallContractSource::Runtime { contract, .. } =
                validation::call_contract_source(program, symbols::SymbolHandle::invalid(), &site)
                    .ok()??
            else {
                return None;
            };
            return instantiate_proof_expression_parameter(
                program,
                facts,
                &site,
                program.state_signature_parameters(&contract.signature),
                parameter,
            );
        }
        if validation::call_target_entry_symbol(program, call.target_symbol) != target_state_symbol
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
        {
            return None;
        }
        let target_parameters = call_target_parameters(program, target_state_symbol)?;
        instantiate_proof_expression_parameter(
            program,
            facts,
            &validation::CallSite::Expression { expression, call },
            target_parameters,
            parameter,
        )
    }

    fn exact_visible_evidence_sources(
        facts: &CheckFacts,
        owner: ContractProofFactOwner,
        current_fact: Handle<typed_trees::domain::ProofFact>,
        name: &str,
    ) -> Vec<Handle<CheckedEvidenceTerm>> {
        // A requires operand forms before that clause introduces its binding.
        // The producer appends each owner's facts in its retained clause order.
        let current_requires = facts
            .proof
            .contract_facts
            .iter()
            .filter_map(|(handle, contract)| {
                (contract.owner == owner
                    && contract.fact == current_fact
                    && contract.kind == ContractProofFactKind::Requires)
                    .then_some(handle.arena_index())
            })
            .min();
        facts
            .proof
            .contract_facts
            .iter()
            .filter_map(|(handle, contract)| {
                let term = contract.evidence_term?;
                let checked = facts.proof.evidence_terms.get(term);
                (contract.kind == ContractProofFactKind::Requires
                    && (contract.owner != owner
                        || current_requires.is_none_or(|current| handle.arena_index() < current))
                    && evidence_term_visible_from_owner(contract.owner, owner)
                    && checked.owner == contract.owner
                    && checked.kind == contract.kind
                    && checked.name == name)
                    .then_some(term)
            })
            .collect()
    }

    fn evidence_term_visible_from_owner(
        term_owner: ContractProofFactOwner,
        fact_owner: ContractProofFactOwner,
    ) -> bool {
        term_owner == fact_owner
            || matches!(
                (term_owner, fact_owner),
                (
                    ContractProofFactOwner::Machine { machine_symbol: term_machine },
                    ContractProofFactOwner::MachineState { machine_symbol: fact_machine, .. },
                ) if term_machine == fact_machine
            )
    }

    fn instantiate_proof_expression_parameter(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        call_site: &validation::CallSite<'_>,
        target_parameters: &[typed_trees::signature::StateParameter],
        parameter: Handle<CheckedEvidenceTerm>,
    ) -> Option<CheckedPropositionApplication> {
        let contract = facts
            .proof
            .contract_facts
            .iter()
            .map(|(_, contract)| contract)
            .find(|contract| contract.evidence_term == Some(parameter))?;
        let application = program.proof_facts.get(contract.fact).as_proposition()?;
        let (binder_labels, argument_handles) =
            crate::published::labels::proposition_label_sources(application, program);
        let argument_labels = argument_handles
            .iter()
            .map(|argument| {
                super::labels::instantiate_call_contract_expression_label(
                    program,
                    symbols::SymbolHandle::invalid(),
                    0,
                    call_site,
                    target_parameters,
                    *argument,
                )
            })
            .collect::<Vec<_>>();
        program
            .normalize_nominal_proposition_application(
                application,
                Some(PropositionLabels {
                    binder_labels: &binder_labels,
                    argument_labels: &argument_labels,
                }),
            )
            .map(crate::published::proof::lower_checked_proposition_application)
    }

    pub(super) fn bind_call_evidence_arguments(
        program: &dyn ::validation::CheckingInput,
        facts: &mut CheckFacts,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut bindings = arena::Arena::default();

        let call_handles = facts
            .proof
            .contract_calls
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for call_handle in call_handles {
            let call = facts.proof.contract_calls.get(call_handle).clone();
            let Some(call_site) = find_call_site(
                program,
                call.caller_machine_symbol,
                call.caller_state_symbol,
                call.statement_index,
                call.call_ordinal,
            ) else {
                continue;
            };
            let authored = call_site_evidence_arguments(&call_site);
            let is_state_transfer =
                call_site.is_internal_state_transfer(program, call.caller_machine_symbol);
            let mut parameters = facts
                .proof
                .contract_fact_refs
                .span_or_empty(call.requires)
                .iter()
                .filter_map(|fact_ref| {
                    let fact = facts.proof.contract_facts.get(fact_ref.fact);
                    if is_state_transfer
                        && matches!(fact.owner, ContractProofFactOwner::Machine { .. })
                    {
                        return None;
                    }
                    fact.evidence_term
                })
                .collect::<Vec<_>>();
            parameters
                .sort_by_key(|parameter| facts.proof.evidence_terms.get(*parameter).lane_position);

            if authored.len() != parameters.len() {
                diagnostics.push(Diagnostic::error(format!(
                    "call `{}` supplies {} erased evidence argument{} but its named requires lane has {}",
                    call_target_name(program, call.target_state_symbol),
                    authored.len(),
                    if authored.len() == 1 { "" } else { "s" },
                    parameters.len(),
                )));
                continue;
            }

            let mut span = HandleSpan::empty();
            let mut runtime_arguments = Vec::with_capacity(authored.len());
            for (lane_position, (name, parameter)) in authored.iter().zip(parameters).enumerate() {
                if facts.proof.evidence_terms.get(parameter).lane_position != lane_position {
                    diagnostics.push(Diagnostic::error(
                        "ordinary evidence call has no complete ordered input lane",
                    ));
                    continue;
                }
                let Some(source) = source_term_by_name(
                    &facts.proof.evidence_terms,
                    &facts.proof.outcome_specific_arms,
                    call.caller_machine_symbol,
                    call.caller_state_symbol,
                    call.statement_index,
                    name.as_str(),
                ) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "unknown incoming evidence term `{}` in call `{}`; erased arguments must name an explicit requires binding",
                        name,
                        call_target_name(program, call.target_state_symbol),
                    )));
                    continue;
                };

                let selected_row = facts
                    .proof
                    .outcome_specific_arms
                    .iter()
                    .flat_map(|(_, arm)| &arm.rows)
                    .find(|row| row.selected_term == Some(source));
                let expected = instantiated_call_proposition(
                    program,
                    &facts.proof,
                    &call,
                    &call_site,
                    parameter,
                    selected_row.is_some(),
                );
                let source_term = facts.proof.evidence_terms.get(source);
                let source_proposition = if let Some(row) = selected_row {
                    row.captured_argument_identities.as_ref().map(|captures| {
                        let mut proposition = source_term.proposition.clone();
                        for (argument, capture) in proposition.arguments.iter_mut().zip(captures) {
                            if let Some(identity) = capture {
                                *argument = identity.clone();
                            }
                        }
                        proposition
                    })
                } else {
                    Some(source_term.proposition.clone())
                };
                if expected.as_ref().map(|(proposition, _)| proposition)
                    != source_proposition.as_ref()
                    || expected.is_none()
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "evidence term `{}` does not inhabit erased requires position {} of call `{}`",
                        name,
                        lane_position,
                        call_target_name(program, call.target_state_symbol),
                    )));
                    continue;
                }

                let Some((instantiated_proposition, instantiated_label)) =
                    instantiated_call_proposition(
                        program,
                        &facts.proof,
                        &call,
                        &call_site,
                        parameter,
                        false,
                    )
                else {
                    diagnostics.push(Diagnostic::error(
                        "ordinary evidence call lost its canonical actual substitution",
                    ));
                    continue;
                };
                runtime_arguments.push(checked_trees::ProofOutputEvidenceArgumentFact {
                    input_position: lane_position,
                    callee_input: parameter,
                    source,
                    instantiated_proposition,
                    instantiated_label,
                });
                bindings.append_to_span(
                    &mut span,
                    ContractEvidenceArgument {
                        source,
                        parameter,
                        lane_position,
                    },
                );
            }
            facts
                .proof
                .contract_calls
                .get_mut(call_handle)
                .evidence_arguments = span;
            if !is_state_transfer
                && runtime_arguments.len() == authored.len()
                && let Err(message) = runtime_calls::retain(
                    program,
                    &mut facts.proof,
                    &call,
                    &call_site,
                    runtime_arguments,
                )
            {
                diagnostics.push(Diagnostic::error(message));
            }
        }

        facts.proof.contract_evidence_arguments = bindings;
    }

    fn source_term_by_name(
        terms: &arena::Arena<CheckedEvidenceTerm>,
        arms: &arena::Arena<checked_trees::OutcomeSpecificArmFact>,
        caller_machine_symbol: symbols::SymbolHandle,
        caller_state_symbol: symbols::SymbolHandle,
        statement_index: usize,
        name: &str,
    ) -> Option<Handle<CheckedEvidenceTerm>> {
        arms.iter()
            .filter(|(_, arm)| {
                arm.caller_machine_symbol == caller_machine_symbol
                    && arm.caller_state_symbol == caller_state_symbol
                    && arm.statement_index == statement_index
            })
            .flat_map(|(_, arm)| arm.rows.iter().filter_map(|row| row.selected_term))
            .find(|term| terms.get(*term).name == name)
            .or_else(|| {
                terms.iter().find_map(|(handle, term)| {
            let owner_matches = matches!(
                term.owner,
                ContractProofFactOwner::Machine { machine_symbol }
                    if machine_symbol == caller_machine_symbol
            ) || matches!(
                term.owner,
                ContractProofFactOwner::MachineState {
                    machine_symbol,
                    state_symbol,
                } if machine_symbol == caller_machine_symbol && state_symbol == caller_state_symbol
            );
            (owner_matches && term.kind == ContractProofFactKind::Requires && term.name == name)
                .then_some(handle)
        })
            })
    }

    fn instantiated_call_proposition(
        program: &dyn ::validation::CheckingInput,
        proof: &checked_trees::ProofFacts,
        call: &checked_trees::ContractCallFact,
        call_site: &validation::CallSite<'_>,
        parameter: Handle<CheckedEvidenceTerm>,
        captured_outcome: bool,
    ) -> Option<(CheckedPropositionApplication, String)> {
        let parameter_term = proof.evidence_terms.get(parameter);
        let lane = match parameter_term.kind {
            ContractProofFactKind::Requires => call.requires,
            ContractProofFactKind::Ensures => call.ensures,
        };
        let contract = proof
            .contract_fact_refs
            .span_or_empty(lane)
            .iter()
            .map(|fact_ref| proof.contract_facts.get(fact_ref.fact))
            .find(|contract| contract.evidence_term == Some(parameter))?;
        let application = program.proof_facts.get(contract.fact).as_proposition()?;
        let target_parameters = if call.signature.is_valid() {
            if !crate::published::proof::contracts::call_signatures::source_is_current(
                program, proof, call, call_site,
            ) {
                return None;
            }
            let signature = crate::published::proof::contracts::call_signatures::signature(
                program,
                proof,
                parameter_term.owner,
            )?;
            program.state_signature_parameters(signature)
        } else if let Some(dispatch) = call_site.static_requirement_dispatch() {
            program
                .trait_definition_by_symbol(dispatch.declaring_trait)
                .and_then(|definition| {
                    program
                        .trait_machine_signature_by_symbol(definition, dispatch.contract_symbol())
                })
                .map(|requirement| program.state_signature_parameters(requirement))?
        } else {
            call_target_parameters(program, call.target_state_symbol)?
        };
        let argument_labels = expressions(program)
            .expression_handles(application.arguments)
            .iter()
            .map(|argument| {
                if captured_outcome
                    && validation::owned_contract_subject(program, target_parameters, *argument)
                        .is_some()
                {
                    return validation::captured_call_subject_identity(
                        program,
                        call.caller_state_symbol,
                        call.statement_index,
                        *call_site,
                        target_parameters,
                        *argument,
                    );
                }
                Some(super::labels::instantiate_call_contract_expression_label(
                    program,
                    call.caller_state_symbol,
                    call.statement_index,
                    call_site,
                    target_parameters,
                    *argument,
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        let mut proposition = parameter_term.proposition.clone();
        let (binder_labels, _) =
            crate::published::labels::proposition_label_sources(application, program);
        let label =
            program.normalized_identity_label(application, &binder_labels, &argument_labels)?;
        proposition.arguments = argument_labels;
        Some((proposition, label))
    }

    fn call_target_name(
        program: &dyn ::validation::CheckingInput,
        target: symbols::SymbolHandle,
    ) -> String {
        crate::published::labels::call_target_label(program, target)
    }
}
#[path = "checks_contracts_exits.rs"]
mod exits;

mod nominal_inputs {
    /*
        The declared field facts assumed by a nominal parameter are obligations on
        each incoming actual. Reuse the exact semantic declaration rows; do not
        recover lost membership merely from an unchanged nominal type annotation.
        Completed operand projections are the only actual correspondence: copies
        keep completion evidence and views consult invocation facts on their frozen
        referents. A historical local-origin replay is not a second proof route.
        Conditional payload validity does not select an active case; only the
        authored source access carries that separate selection obligation.
    */
    use crate::flow::CapturedResultMemberships;
    use crate::flow::OperandContextSelection;
    use crate::flow::OperandContextSelections;
    use crate::flow::captured_observation_places;
    use crate::flow::captured_value_places;
    use crate::flow::literal_value_path_is_inactive;
    use crate::published::labels::call_target_label;
    use crate::published::labels::machine_name;
    use crate::published::labels::symbol_name;
    use crate::shared::collections::UniqueRoster;
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use diagnostics::Diagnostic;
    use facts::{FactOrigin, FactPayload, FactPlan, PlaceRoot, PlaceSegment, ProgramPoint};
    use symbols::SymbolHandle;

    pub(super) struct DeclaredFieldRequirements {
        rows: symbols::SymbolKeyMap<SymbolHandle, UniqueRoster<FieldRequirement>>,
    }

    type FieldRequirement = (
        Vec<PlaceSegment>,
        SymbolHandle,
        language_semantics::SemanticDomainId,
    );

    impl DeclaredFieldRequirements {
        pub(super) fn new(semantic: &FactPlan) -> Self {
            let mut rows: symbols::SymbolKeyMap<SymbolHandle, UniqueRoster<FieldRequirement>> =
                symbols::SymbolKeyMap::default();
            for (_, fact) in semantic.facts.iter() {
                let FactPayload::DomainMembership {
                    domain_symbol,
                    semantic_domain,
                    ..
                } = fact.payload
                else {
                    continue;
                };
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let place = semantic.places.get(place);
                if place.segments.is_empty() {
                    continue;
                }
                let owner = match fact.origin {
                    FactOrigin::StateParameterDomain {
                        machine_symbol,
                        state_symbol,
                    } if fact.point
                        == (ProgramPoint::State {
                            machine_symbol,
                            state_symbol,
                        }) =>
                    {
                        let Some(symbol) = place.root.as_symbol() else {
                            continue;
                        };
                        symbol
                    }
                    _ => continue,
                };
                let requirement = (
                    semantic
                        .place_segments
                        .span_or_empty(place.segments)
                        .to_vec(),
                    domain_symbol,
                    semantic_domain,
                );
                rows.entry(owner).or_default().push(requirement);
            }
            Self { rows }
        }

        fn requirements(&self, owner: SymbolHandle) -> &[FieldRequirement] {
            self.rows.get(&owner).map_or(&[], UniqueRoster::as_slice)
        }
    }

    pub(super) fn check(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        requirements: &DeclaredFieldRequirements,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if requirements.rows.is_empty() {
            return;
        }
        let Some(parameters) = validation::call_target_parameters(program, call.target_symbol)
        else {
            return;
        };
        if !parameters
            .iter()
            .any(|parameter| !requirements.requirements(parameter.symbol).is_empty())
        {
            return;
        }
        let Some(site) = validation::find_call_site(
            program,
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        ) else {
            return;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        let mut operand_contexts = OperandContextSelections::new(
            program,
            &facts.flow,
            &facts.semantic,
            state.machine_symbol,
        );
        let mut result_memberships = CapturedResultMemberships::new(facts);
        let receiver_offset = usize::from(
            matches!(&site, validation::CallSite::Expression { call, .. } if call.receiver.is_valid()),
        );
        for (position, parameter) in parameters.iter().enumerate() {
            let argument = arguments.get(position).copied();
            let operand_position = position + receiver_offset;
            for (segments, domain_symbol, semantic_domain) in
                requirements.requirements(parameter.symbol)
            {
                if argument.is_some_and(|argument| {
                    literal_value_path_is_inactive(
                        program,
                        argument,
                        parameter.type_reference,
                        segments,
                    )
                }) {
                    continue;
                }
                let captured_actual = argument.map(|arg| super::call_values::Actual {
                    parameter,
                    argument: Some(arg),
                    path: segments.clone(),
                    operand_position,
                });
                let mut failed_capture = None;
                let satisfied = captured_actual
                    .as_ref()
                    .and_then(|actual| {
                        super::call_values::captured_values(program, facts, state, call, actual)
                    })
                    .is_some_and(|projections| {
                        !projections.is_empty()
                            && projections.iter().all(|captured| {
                                if ::validation::trace_flags::check_sites() {
                                    eprintln!("CHECKSITE nominal-field-capture state={:?} statement={} target={:?} parameter={:?} required_path={segments:?} domain={domain_symbol:?} observation={:?} observation_type={:?} observation_projection={:?} observation_subjects={:?} producer={:?} producer_type={:?} producer_projection={:?} producer_subjects={:?}", state.state_symbol, call.statement_index, call.target_symbol, parameter.symbol, *captured.observation, captured.observation_type, captured.observation_projection.remaining, captured_observation_places(program, facts.flow.view(), captured).map(|places| places.into_iter().map(|place| (place.storage, place.projected)).collect::<Vec<_>>()), *captured.operand, captured.type_reference, captured.projection.remaining, captured_value_places(program, facts.flow.view(), captured).map(|places| places.into_iter().map(|place| (place.storage, place.projected)).collect::<Vec<_>>()));
                                }
                                let invocation = facts
                                    .flow
                                    .control
                                    .operands
                                    .span_or_empty(call.operands)
                                    .last();
                                if invocation.is_some_and(|observation| {
                                    observation.storage_snapshot == call.invocation_storage
                                        && result_memberships.contains(
                                            program,
                                            captured,
                                            observation,
                                            *domain_symbol,
                                            *semantic_domain,
                                        )
                                }) {
                                    return true;
                                }
                                let mut route_refusals = [None, None];
                                let satisfied_by_sources = [
                                    (
                                        captured.observation,
                                        captured.observation_type,
                                        captured_observation_places(program, facts.flow.view(), captured),
                                        call.entry_constraints,
                                        (state.state_symbol, call.statement_index),
                                    ),
                                    (
                                        captured.operand,
                                        captured.type_reference,
                                        captured_value_places(program, facts.flow.view(), captured),
                                        captured.producer_invocation(call.entry_constraints),
                                        captured.producer_frontier(
                                            state.state_symbol,
                                            call.statement_index,
                                        ),
                                    ),
                                ]
                                .into_iter()
                                .enumerate()
                                .any(
                                    |(route, (
                                        operand,
                                        reference,
                                        subjects,
                                        invocation,
                                        (source_state, source_statement),
                                    ))| {
                                        let Some(selection) = operand_contexts.for_operand(
                                            operand,
                                            reference,
                                            invocation,
                                            source_state,
                                            source_statement,
                                        ) else {
                                            route_refusals[route] = Some("operand context unavailable");
                                            if ::validation::trace_flags::check_sites() {
                                                eprintln!("CHECKSITE nominal-field-context-unavailable state={:?} statement={} target={:?} expression={:?} reference={reference:?} source_state={source_state:?} source_statement={source_statement} invocation={invocation:?}", state.state_symbol, call.statement_index, call.target_symbol, operand.expression);
                                            }
                                            return false;
                                        };
                                        if subjects.as_ref().is_none_or(Vec::is_empty) {
                                            route_refusals[route] = Some("operand projection unavailable or empty");
                                        }
                                        subjects.is_some_and(|subjects| {
                                            !subjects.is_empty()
                                                && subjects.iter().all(|place| {
                                                    if pointwise_field_is_empty(
                                                        program,
                                                        &operand,
                                                        &place.projected,
                                                    ) {
                                                        return true;
                                                    }
                                                    let cases_selected = selection.cases_are_selected(&place.storage);
                                                    let satisfied = cases_selected
                                                        && (selection
                                                            .path_is_inactive(&place.projected)
                                                            || prove_declared_field_domain(
                                                                program,
                                                                facts,
                                                                selection,
                                                                &place.projected,
                                                                *domain_symbol,
                                                                *semantic_domain,
                                                            ));
                                                    if !satisfied {
                                                        route_refusals[route] = Some(if cases_selected {
                                                            "declared field domain not proved"
                                                        } else {
                                                            "source case not selected"
                                                        });
                                                    }
                                                    if !satisfied && ::validation::trace_flags::check_sites() {
                                                        eprintln!("CHECKSITE nominal-field-proof-refused state={:?} statement={} target={:?} expression={:?} reference={reference:?} subject={:?} storage={:?} source_state={source_state:?} source_statement={source_statement} contexts={:?} constraints={:?}", state.state_symbol, call.statement_index, call.target_symbol, operand.expression, place.projected, place.storage, selection.contexts, facts.flow.contexts.constraint_refs.span_or_empty(invocation));
                                                    }
                                                    satisfied
                                                })
                                        })
                                    },
                                );
                                if !satisfied_by_sources {
                                    failed_capture = Some(route_refusals);
                                }
                                satisfied_by_sources
                            })
                    });
                if !satisfied {
                    let detail = failed_capture
                        .and_then(|routes| routes.into_iter().flatten().next())
                        .map(|reason| format!(" ({reason})"))
                        .unwrap_or_default();
                    diagnostics.push(Diagnostic::error(format!(
                        "cannot prove default-domain field requirement for call {} from {}::{}: parameter {} requires {}{detail}",
                        call_target_label(program, call.target_symbol),
                        machine_name(program, state.machine_symbol),
                        symbol_name(program, state.state_symbol),
                        facts::canonical_place_label_from_parts(program, PlaceRoot::Symbol(parameter.symbol), segments),
                        symbol_name(program, *domain_symbol),
                    )));
                }
            }
        }
    }

    /* Empty pointwise ancestors contain no qualified field occurrences. */
    fn pointwise_field_is_empty(
        program: &dyn ::validation::CheckingInput,
        operand: &checked_trees::FlowOperandFact,
        place: &validation::CanonicalPlace,
    ) -> bool {
        place
            .segments
            .iter()
            .enumerate()
            .any(|(position, segment)| {
                let PlaceSegment::FixedRange { start, end } = segment else {
                    return false;
                };
                if position + 1 == place.segments.len() {
                    return false;
                }
                if start == end {
                    return true;
                }
                if *start != 0 || *end != usize::MAX {
                    return false;
                }
                let prefix = validation::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..position].to_vec(),
                };
                validation::canonical_place_type_reference(
                    program,
                    operand.state_symbol,
                    operand.statement_index,
                    &prefix,
                )
                .is_some_and(|reference| {
                    validation::fixed_array_type_length(program, reference) == Some(0)
                })
            })
    }

    /*
        A field qualification carries its complete domain application. Predicate
        proof may discharge an atomic domain; it cannot select an indexed instance
        or manufacture establishment provenance. Both exact and candidate referents
        use this same judgment.
    */
    fn prove_declared_field_domain(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        selection: &OperandContextSelection<'_>,
        subject: &validation::CanonicalPlace,
        domain_symbol: SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
    ) -> bool {
        {
            if validation::field_domains::domain_requires_provenance(program, domain_symbol)
                || !typed_trees::domain::has_symbol_only_proof(program, domain_symbol)
            {
                selection.has_membership(subject, domain_symbol, semantic_domain)
            } else {
                super::prover::prove_domain_at_place(
                    program,
                    &facts.semantic,
                    &selection.contexts,
                    subject,
                    domain_symbol,
                )
            }
        }
    }
}
#[path = "checks_contracts_writes.rs"]
mod writes;

/* The fact-context prover and the provers it reaches. */
mod direct {
    use facts::FactPlace;
    use language_semantics::declaration_selection::CollectionMeasure;
    use symbols::SymbolHandle;

    use super::labels::{ContractTargetParameters, instantiate_call_contract_expression_label};
    use super::places::{expression_is_boolean_place_like, expression_place_matches};
    use crate::flow::expressions;
    use crate::published::labels::semantic_boolean_fact_label;
    use crate::published::places::place_at;

    mod guard_values {
        use crate::flow::expressions;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };

        pub(super) mod denotations {
            /*
                Compare guard observations in the caller's current namespace. A captured
                local expands only through its exact selected producer and a complete frame
                preserving both the destination and every observed backing place. A reference
                actual — a local loan or a returned loan — denotes that loan's storage, so
                a field fact on the storage proves the formal's field requirement. Several
                exact storages do not: the callee may receive any of them. Expansion
                establishes denotation; it never evaluates a user call or assumes its purity.
            */
            use crate::flow::state_statements;
            use checked_trees::{
                CheckFacts, CheckedScalarComputationKind as Computation,
                CheckedScalarExpressionRole,
            };
            use symbols::SymbolHandle;
            use typed_trees::{
                expression::{ExpressionHandle, ExpressionNode},
                statement::StatementNode,
                types::PrimitiveType,
            };

            enum LoanStorage {
                One(validation::CanonicalPlace),
                Many,
                Unknown,
            }

            pub(in super::super) struct Denotations<'a> {
                pub program: &'a dyn ::validation::CheckingInput,
                pub facts: &'a CheckFacts,
                pub caller_state_symbol: SymbolHandle,
                pub target_state_symbol: SymbolHandle,
                pub statement_index: usize,
                pub call_frames: Option<&'a validation::CallFrameResolver<'a>>,
                pub arguments: &'a [ExpressionHandle],
            }

            impl Denotations<'_> {
                pub fn equal(&self, left: ExpressionHandle, right: ExpressionHandle) -> bool {
                    self.compare(left, right, true, &mut 4096, 0)
                }

                fn compare(
                    &self,
                    left: ExpressionHandle,
                    right: ExpressionHandle,
                    formal: bool,
                    remaining: &mut usize,
                    depth: usize,
                ) -> bool {
                    if depth >= 64 || *remaining == 0 {
                        return false;
                    }
                    *remaining -= 1;
                    let table = &self.program.expression_table;
                    if formal
                        && let ExpressionNode::Name(path) = table.expression(right)
                        && table.name_path_is_single_member(path)
                        && let Some(position) = validation::call_contract_parameter_position(
                            self.program,
                            self.target_state_symbol,
                            path.symbol,
                        )
                    {
                        return self.arguments.get(position).is_some_and(|actual| {
                            self.compare(left, *actual, false, remaining, depth + 1)
                        });
                    }
                    if let Some(source) = self.captured(left, remaining) {
                        return self.compare(source, right, formal, remaining, depth + 1);
                    }
                    if !formal && let Some(source) = self.captured(right, remaining) {
                        return self.compare(left, source, false, remaining, depth + 1);
                    }
                    if !formal {
                        if let ExpressionNode::Borrow(borrow) = table.expression(left) {
                            return self.compare(borrow.target, right, false, remaining, depth + 1);
                        }
                        if let ExpressionNode::Borrow(borrow) = table.expression(right) {
                            return self.compare(left, borrow.target, false, remaining, depth + 1);
                        }
                    }
                    if formal
                        && let Some(required) =
                            crate::flow::projected_leaf_place(self.program, right)
                        && let Some(root) = required.root.as_symbol()
                        && let Some(position) = validation::call_contract_parameter_position(
                            self.program,
                            self.target_state_symbol,
                            root,
                        )
                        && let Some(actual) = self.arguments.get(position)
                        && let Some(observed) =
                            crate::flow::projected_leaf_place(self.program, left)
                    {
                        if let Some(mut syntactic) =
                            crate::flow::projected_leaf_place(self.program, *actual)
                        {
                            syntactic.extend_segments(&required.segments);
                            if syntactic == observed {
                                return true;
                            }
                        }
                        if self.expression_is_reference_loan(*actual) {
                            match self.exact_loan_storage(*actual) {
                                LoanStorage::One(mut storage) => {
                                    storage.extend_segments(&required.segments);
                                    return storage == observed;
                                }
                                LoanStorage::Many => return false,
                                LoanStorage::Unknown => {}
                            }
                        }
                    }
                    let equal = |left, right, remaining: &mut usize| {
                        self.compare(left, right, formal, remaining, depth + 1)
                    };
                    match (table.expression(left), table.expression(right)) {
                        (ExpressionNode::Unary(a), ExpressionNode::Unary(b)) => {
                            a.operator == b.operator
                                && self.builtin(left)
                                && self.builtin(right)
                                && equal(a.operand, b.operand, remaining)
                        }
                        (ExpressionNode::Binary(a), ExpressionNode::Binary(b)) => {
                            a.operator == b.operator
                                && self.same_binary_meaning(left, right, formal)
                                && equal(a.left, b.left, remaining)
                                && equal(a.right, b.right, remaining)
                        }
                        (ExpressionNode::Member(a), ExpressionNode::Member(b)) => {
                            a.member == b.member
                                && a.case_variant == b.case_variant
                                && self.same_member_selection(a, b)
                                && equal(a.receiver, b.receiver, remaining)
                        }
                        (ExpressionNode::Indexed(a), ExpressionNode::Indexed(b)) => {
                            equal(a.collection, b.collection, remaining)
                                && equal(a.index, b.index, remaining)
                        }
                        (ExpressionNode::Borrow(a), ExpressionNode::Borrow(b)) => {
                            a.access == b.access && equal(a.target, b.target, remaining)
                        }
                        (ExpressionNode::Call(a), ExpressionNode::Call(b)) => {
                            if a.is_ordinary_positional_call()
                                && b.is_ordinary_positional_call()
                                && a.target_symbol.is_valid()
                                && a.target_symbol == b.target_symbol
                            {
                                let target = validation::find_state(self.program, a.target_symbol)
                                    .or_else(|| {
                                        let machine =
                                            self.program.machine_by_symbol(a.target_symbol)?;
                                        Some((
                                            machine,
                                            self.program.machine_states(machine).first()?,
                                        ))
                                    });
                                let call_plans =
                                    validation::operand_call_plans(self.program, self.call_frames);
                                let (operational, service_reaches) = call_plans.as_ref();
                                if target.is_some_and(|(machine, state)| {
                                    validation::checked_value_call_candidate(
                                        self.program,
                                        machine,
                                        state,
                                        operational,
                                        service_reaches,
                                    )
                                    .is_ok()
                                }) {
                                    let first = table.expression_handles(a.arguments);
                                    let second = table.expression_handles(b.arguments);
                                    return first.len() == second.len()
                                        && first.iter().zip(second).all(|(first, second)| {
                                            equal(*first, *second, remaining)
                                        });
                                }
                            }
                            match (
                                typed_trees::byte_predicates::byte_predicate_call(
                                    self.program,
                                    left,
                                ),
                                typed_trees::byte_predicates::byte_predicate_call(
                                    self.program,
                                    right,
                                ),
                            ) {
                                (Some((first, left)), Some((second, right))) => {
                                    first == second && equal(left, right, remaining)
                                }
                                _ => false,
                            }
                        }
                        (ExpressionNode::Name(_), ExpressionNode::Name(_))
                        | (ExpressionNode::Boolean(_), ExpressionNode::Boolean(_))
                        | (ExpressionNode::Integer(_), ExpressionNode::Integer(_))
                        | (ExpressionNode::Float(_), ExpressionNode::Float(_))
                        | (ExpressionNode::String(_), ExpressionNode::String(_)) => {
                            table.expressions_structurally_equal(left, right)
                        }
                        _ => false,
                    }
                }

                /*
                    A declared field selects its member symbol. A collection measure such as
                    `fields.len` has no field symbol: it denotes the same value exactly when
                    both sides apply the same measure to collections whose receivers already
                    compare equal, so `context.fields.len` in a caller premise matches the
                    callee's `context.fields.len`. A field and a measure never match.
                */
                fn same_member_selection(
                    &self,
                    left: &typed_trees::expression::TableMemberExpression,
                    right: &typed_trees::expression::TableMemberExpression,
                ) -> bool {
                    if left.member_symbol.is_valid() || right.member_symbol.is_valid() {
                        return left.member_symbol == right.member_symbol;
                    }
                    let measure = |member: &typed_trees::expression::TableMemberExpression| {
                        let receiver = validation::expression_place_type_reference(
                            self.program,
                            member.receiver,
                            &[],
                        )?;
                        validation::collection_operations::collection_measure_member(
                            self.program,
                            member,
                            validation::collection_operations::MeasureReceiver::Declared(receiver),
                        )
                    };
                    measure(left).is_some_and(|measure_left| measure(right) == Some(measure_left))
                }

                fn expression_is_reference_loan(&self, expression: ExpressionHandle) -> bool {
                    match self.program.expression_table.expression(expression) {
                        ExpressionNode::Call(_) => true,
                        ExpressionNode::Name(path) => {
                            self.name_is_reference_local(path.symbol, path.head_symbol)
                        }
                        _ => false,
                    }
                }

                fn name_is_reference_local(
                    &self,
                    symbol: SymbolHandle,
                    head_symbol: SymbolHandle,
                ) -> bool {
                    if !symbol.is_valid() || symbol != head_symbol {
                        return false;
                    }
                    let Some((_, state)) =
                        validation::find_state(self.program, self.caller_state_symbol)
                    else {
                        return false;
                    };
                    let Some((_, local)) = self.program.state_local_declaration(state, symbol)
                    else {
                        return false;
                    };
                    crate::checks::type_reference_is_reference(self.program, local.type_reference)
                }

                /*
                    One exact referent of a reference actual at this call. `Many` means the
                    loan may name more than one storage, so one field fact cannot prove it.
                */
                fn exact_loan_storage(&self, expression: ExpressionHandle) -> LoanStorage {
                    let Some(frames) = self.call_frames else {
                        return LoanStorage::Unknown;
                    };
                    let Some((machine, state)) =
                        validation::find_state(self.program, self.caller_state_symbol)
                    else {
                        return LoanStorage::Unknown;
                    };
                    let Some(statement) = self
                        .program
                        .statement_table
                        .statement_at(state, self.statement_index)
                    else {
                        return LoanStorage::Unknown;
                    };
                    let Some(origins) = frames.reference_expression_origins_before_statement(
                        machine, statement, expression,
                    ) else {
                        return LoanStorage::Unknown;
                    };
                    let mut storages = Vec::new();
                    for (root, segments) in origins {
                        let place = validation::CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(root),
                            segments,
                        };
                        crate::shared::collections::push_unique(&mut storages, place);
                    }
                    match storages.len() {
                        0 => LoanStorage::Unknown,
                        1 => LoanStorage::One(storages.remove(0)),
                        _ => LoanStorage::Many,
                    }
                }

                fn same_binary_meaning(
                    &self,
                    left: ExpressionHandle,
                    right: ExpressionHandle,
                    formal: bool,
                ) -> bool {
                    if self.builtin(left) && self.builtin(right) {
                        return true;
                    }
                    let selected = |expression, state| {
                        let (machine, state) = validation::find_state(self.program, state)
                            .or_else(|| {
                                let machine = self.program.machine_by_symbol(state)?;
                                Some((machine, self.program.machine_states(machine).first()?))
                            })?;
                        let binary = self.program.expression_table.binary_node(expression)?;
                        validation::sealed_float_comparison_declaration(
                            self.program,
                            machine,
                            Some(state),
                            expression,
                            binary,
                        )
                    };
                    selected(left, self.caller_state_symbol)
                        .zip(selected(
                            right,
                            if formal {
                                self.target_state_symbol
                            } else {
                                self.caller_state_symbol
                            },
                        ))
                        .is_some_and(|(left, right)| left == right)
                }

                fn builtin(&self, expression: ExpressionHandle) -> bool {
                    crate::shared::authored_selections::typed_operator_has_no_authored_selection(
                        self.program,
                        expression,
                    )
                }

                fn captured(
                    &self,
                    expression: ExpressionHandle,
                    remaining: &mut usize,
                ) -> Option<ExpressionHandle> {
                    let path = self.program.expression_table.name_path_node(expression)?;
                    if !self
                        .program
                        .expression_table
                        .name_path_is_single_member(path)
                    {
                        return None;
                    }
                    let (machine, state) =
                        validation::find_state(self.program, self.caller_state_symbol)?;
                    let statements = state_statements(self.program, state.statement_nodes);
                    let (definition, local) =
                        self.program.state_local_declaration(state, path.symbol)?;
                    if definition >= self.statement_index
                        || local.is_mutable
                        || self.program.primitive_type_reference(local.type_reference)
                            != Some(PrimitiveType::Bool)
                        || !local.initial_value.is_valid()
                    {
                        return None;
                    }
                    let binding_ordinal = statements[..definition]
                        .iter()
                        .filter(|node| {
                            matches!(node,
                        StatementNode::LocalData(local) if !local.is_mutable && local.initial_value.is_valid()
                            && self.program.primitive_type_reference(local.type_reference).is_some())
                        })
                        .count();
                    let role = CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: u32::try_from(binding_ordinal).ok()?,
                    };
                    let plans = &self.facts.values.scalar_expressions;
                    let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
                        binding.state == state.symbol
                            && binding.statement_ordinal == definition as u32
                            && binding.role == role
                    });
                    if let Some((_, binding)) = bindings.next() {
                        if bindings.next().is_some()
                            || binding.expression != local.initial_value
                            || binding.destination != local.symbol
                        {
                            return None;
                        }
                        let mut selected = plans.expressions.iter().filter(|plan| {
                            plan.state == state.symbol
                                && plan.statement_ordinal == definition as u32
                                && plan.role == role
                        });
                        selected.next()?;
                        if selected.next().is_some() {
                            return None;
                        }
                    } else {
                        let plans = &self.facts.values.scalar_computations;
                        let root =
                            plans.root_at(state.symbol, u32::try_from(definition).ok()?, role)?;
                        if root.machine != machine.symbol
                            || !plans.nodes.is_valid(root.root)
                            || plans.nodes.get(root.root).authored_root != local.initial_value
                            || !self.selected_pure(root.root, remaining, 0)
                        {
                            return None;
                        }
                    }
                    let frames = self.call_frames?;
                    let mut later_writes = Vec::new();
                    for argument in self.arguments {
                        later_writes.extend(
                            frames
                                .expression_write_frame(machine, *argument)
                                .into_complete_paths()?,
                        );
                    }
                    if !crate::flow::saved_guard_observations(
                        self.program,
                        machine,
                        state,
                        self.statement_index,
                        expression,
                        true,
                        &later_writes,
                        Some(frames),
                    )
                    .iter()
                    .any(|observation| {
                        observation.current_expression == local.initial_value && observation.value
                    }) {
                        return None;
                    }
                    Some(local.initial_value)
                }

                fn selected_pure(
                    &self,
                    handle: checked_trees::CheckedScalarComputationHandle,
                    remaining: &mut usize,
                    depth: usize,
                ) -> bool {
                    let plans = &self.facts.values.scalar_computations;
                    if depth >= 64 || *remaining == 0 || !plans.nodes.is_valid(handle) {
                        return false;
                    }
                    *remaining -= 1;
                    match &plans.nodes.get(handle).kind {
                        Computation::Value(_) => plans.nodes.get(handle).value_source.is_valid(),
                        Computation::ByteSequencePredicate {
                            predicate,
                            source_expression,
                            ..
                        } => typed_trees::byte_predicates::byte_predicate_call(
                            self.program,
                            *source_expression,
                        )
                        .is_some_and(|(selected, _)| selected == *predicate),
                        Computation::StructuralField {
                            source_expression,
                            receiver_value,
                            ..
                        } => {
                            !receiver_value.is_valid()
                                && *source_expression == plans.nodes.get(handle).authored_root
                                && crate::flow::projected_leaf_place(
                                    self.program,
                                    *source_expression,
                                )
                                .is_some()
                        }
                        Computation::SelectedComparison {
                            operator_use,
                            left,
                            right,
                        } => {
                            let Some(_) = self
                                .facts
                                .operators
                                .selected_float_comparison(self.program, *operator_use)
                            else {
                                return false;
                            };
                            let selected = self.facts.operators.uses.get(*operator_use);
                            let Some((machine, state)) =
                                validation::find_state(self.program, self.caller_state_symbol)
                            else {
                                return false;
                            };
                            let Some(binary) = self
                                .program
                                .expression_table
                                .binary_node(selected.expression)
                            else {
                                return false;
                            };
                            selected.expression == plans.nodes.get(handle).authored_root
                                && validation::sealed_float_comparison_declaration(
                                    self.program,
                                    machine,
                                    Some(state),
                                    selected.expression,
                                    binary,
                                ) == Some(selected.selected_operator_symbol)
                                && self.selected_pure(*left, remaining, depth + 1)
                                && self.selected_pure(*right, remaining, depth + 1)
                        }
                        Computation::Apply { operands, .. } => {
                            plans.operands.span(*operands).is_some_and(|operands| {
                                operands.iter().all(|operand| {
                                    self.selected_pure(*operand, remaining, depth + 1)
                                })
                            })
                        }
                        _ => false,
                    }
                }
            }
        }

        /*
            Branch truth is structural evidence, not a display-label substitution. This
            deliberately shares the expression table's symbol-aware equality relation.
        */
        pub(super) fn proves(
            program: &dyn ::validation::CheckingInput,
            guard: ExpressionHandle,
            guard_value: bool,
            required: ExpressionHandle,
            required_value: bool,
        ) -> bool {
            proves_with(
                program,
                guard,
                guard_value,
                required,
                required_value,
                &|left, right| expressions(program).expressions_structurally_equal(left, right),
            )
        }

        pub(in crate::checks::contracts) fn proves_with(
            program: &dyn ::validation::CheckingInput,
            guard: ExpressionHandle,
            guard_value: bool,
            required: ExpressionHandle,
            required_value: bool,
            equal: &impl Fn(ExpressionHandle, ExpressionHandle) -> bool,
        ) -> bool {
            macro_rules! prove_guard {
                ($guard:expr, $guard_value:expr, $required:expr, $required_value:expr $(,)?) => {
                    proves_with(
                        program,
                        $guard,
                        $guard_value,
                        $required,
                        $required_value,
                        equal,
                    )
                };
            }
            if let ExpressionNode::Boolean(value) = program.expression_table.expression(guard)
                && *value != guard_value
            {
                return true;
            }
            if let Some(value) = program.expression_table.boolean_node(required) {
                return *value == required_value;
            }
            if let ExpressionNode::Unary(unary) = program.expression_table.expression(guard)
                && unary.operator == UnaryOperator::LogicalNot
                && crate::shared::authored_selections::typed_operator_has_no_authored_selection(
                    program, guard,
                )
            {
                return prove_guard!(unary.operand, !guard_value, required, required_value);
            }
            if let ExpressionNode::Unary(unary) = program.expression_table.expression(required)
                && unary.operator == UnaryOperator::LogicalNot
                && crate::shared::authored_selections::typed_operator_has_no_authored_selection(
                    program, required,
                )
            {
                return prove_guard!(guard, guard_value, unary.operand, !required_value);
            }
            if let Some((operand, value)) = boolean_comparison(program, guard, guard_value) {
                return prove_guard!(operand, value, required, required_value);
            }
            if let Some((operand, value)) = boolean_comparison(program, required, required_value) {
                return prove_guard!(guard, guard_value, operand, value);
            }
            if equal(guard, required) {
                return guard_value == required_value;
            }
            if let Some(binary) = program.expression_table.binary_node(guard)
                && ((binary.operator == BinaryOperator::And && guard_value)
                    || (binary.operator == BinaryOperator::Or && !guard_value))
            {
                return prove_guard!(binary.left, guard_value, required, required_value)
                    || prove_guard!(binary.right, guard_value, required, required_value);
            }
            if let Some(binary) = program.expression_table.binary_node(required) {
                let left = || prove_guard!(guard, guard_value, binary.left, required_value);
                let right = || prove_guard!(guard, guard_value, binary.right, required_value);
                match (binary.operator, required_value) {
                    (BinaryOperator::And, true) | (BinaryOperator::Or, false) => {
                        return left() && right();
                    }
                    (BinaryOperator::Or, true) | (BinaryOperator::And, false) => {
                        return left() || right();
                    }
                    _ => {}
                }
            }
            false
        }

        fn boolean_comparison(
            program: &dyn ::validation::CheckingInput,
            expression: ExpressionHandle,
            value: bool,
        ) -> Option<(ExpressionHandle, bool)> {
            let binary = program.expression_table.binary_node(expression)?;
            if !matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            ) || !crate::shared::authored_selections::typed_operator_has_no_authored_selection(
                program, expression,
            ) {
                return None;
            }
            for (operand, literal) in [(binary.left, binary.right), (binary.right, binary.left)] {
                if let Some(literal) = program.expression_table.boolean_node(literal) {
                    return Some((
                        operand,
                        (*literal == value) == (binary.operator == BinaryOperator::Equal),
                    ));
                }
            }
            None
        }
    }
    mod match_patterns {
        /*
            Discharge direct comparison obligations from exact dispatch observations.
            Substitution maps formal leaves to caller operands; display labels never
            establish equality between different symbols or mutable storage locations.
        */
        use crate::flow::expressions;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };

        #[allow(clippy::too_many_arguments)]
        pub(super) fn proves(
            program: &dyn ::validation::CheckingInput,
            subject: ExpressionHandle,
            pattern: ExpressionHandle,
            matched: bool,
            required: ExpressionHandle,
            required_value: bool,
            substitute: &impl Fn(ExpressionHandle) -> ExpressionHandle,
        ) -> bool {
            proves_scoped(
                program,
                subject,
                pattern,
                matched,
                required,
                required_value,
                substitute,
                false,
            )
        }

        #[allow(clippy::too_many_arguments)]
        fn proves_scoped(
            program: &dyn ::validation::CheckingInput,
            subject: ExpressionHandle,
            pattern: ExpressionHandle,
            matched: bool,
            required: ExpressionHandle,
            required_value: bool,
            substitute: &impl Fn(ExpressionHandle) -> ExpressionHandle,
            caller_scope: bool,
        ) -> bool {
            let replacement = if caller_scope {
                required
            } else {
                substitute(required)
            };
            let caller_scope = caller_scope || replacement != required;
            let required = replacement;
            if matches!(
                program.expression_table.expression(required),
                ExpressionNode::Binary(_) | ExpressionNode::Unary(_)
            ) && !crate::shared::authored_selections::typed_operator_has_no_authored_selection(
                program, required,
            ) {
                return false;
            }
            let equal =
                |left, right| expressions(program).expressions_structurally_equal(left, right);
            if let ExpressionNode::Boolean(value) = program.expression_table.expression(pattern)
                && equal(subject, required)
            {
                return (*value == matched) == required_value;
            }
            match program.expression_table.expression(required) {
                ExpressionNode::Boolean(value) => *value == required_value,
                ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                    proves_scoped(
                        program,
                        subject,
                        pattern,
                        matched,
                        unary.operand,
                        !required_value,
                        substitute,
                        caller_scope,
                    )
                }
                ExpressionNode::Binary(binary) => match binary.operator {
                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                        let left = if caller_scope {
                            binary.left
                        } else {
                            substitute(binary.left)
                        };
                        let right = if caller_scope {
                            binary.right
                        } else {
                            substitute(binary.right)
                        };
                        ((equal(subject, left) && equal(pattern, right))
                            || (equal(subject, right) && equal(pattern, left)))
                            && (matched == (binary.operator == BinaryOperator::Equal))
                                == required_value
                    }
                    BinaryOperator::And | BinaryOperator::Or => {
                        let left = || {
                            proves_scoped(
                                program,
                                subject,
                                pattern,
                                matched,
                                binary.left,
                                required_value,
                                substitute,
                                caller_scope,
                            )
                        };
                        let right = || {
                            proves_scoped(
                                program,
                                subject,
                                pattern,
                                matched,
                                binary.right,
                                required_value,
                                substitute,
                                caller_scope,
                            )
                        };
                        if (binary.operator == BinaryOperator::And) == required_value {
                            left() && right()
                        } else {
                            left() || right()
                        }
                    }
                    _ => false,
                },
                _ => false,
            }
        }
    }
    pub(super) use guard_values::proves_with as guard_value_proves_with;

    pub(super) fn direct_context_proves_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        semantic: &facts::FactPlan,
        context: &facts::FactContext,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        let required_label = program.expression_table.display_name(expression);

        semantic.context_view(context).facts().any(|fact| {
            if let Some((subject, pattern, matched)) = fact.payload.match_pattern_comparison(program) {
                return match_patterns::proves(
                    program,
                    subject,
                    pattern,
                    matched,
                    expression,
                    true,
                    &|value| value,
                );
            }
            if let facts::FactPayload::BooleanValue {
                expression: guard,
                value,
            } = fact.payload
            {
                return guard_values::proves(program, guard, value, expression, true);
            }
            let candidate_label = semantic_boolean_fact_label(program, semantic, fact).or_else(|| {
                semantic
                    .proposition_fact_label(program, fact)
                    .and_then(|label| label.strip_prefix("boolean:").map(str::to_owned))
            });
            let Some(candidate_label) = candidate_label else {
                return false;
            };

            candidate_label == required_label
                || (expression_is_boolean_place_like(program, expression)
                    && matches!(fact.place, FactPlace::Place(candidate_place)
                        if expression_place_matches(program, semantic, expression, candidate_place)))
        })
    }

    pub(super) fn direct_context_proves_instantiated_boolean_expression(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        call_flow: &checked_trees::FlowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        context: &facts::FactContext,
        contexts: &[facts::FactContextHandle],
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_site: &validation::CallSite<'_>,
        target_state: &(impl ContractTargetParameters + ?Sized),
        target_state_symbol: SymbolHandle,
        expression: typed_trees::expression::ExpressionHandle,
        assignments: &mut crate::values::RootAssignmentObservations<'_>,
    ) -> bool {
        let semantic = &facts.semantic;
        let parameters = target_state.contract_parameters(program);
        let arguments = validation::call_site_argument_expressions(program, call_site);
        let substitute = |expression| {
            let typed_trees::expression::ExpressionNode::Name(path) =
                program.expression_table.expression(expression)
            else {
                return expression;
            };
            if !path.symbol.is_valid()
                || expressions(program).name_path_members(path.members).len() != 1
            {
                return expression;
            }
            typed_trees::signature::state_parameter_position(parameters, path.symbol)
                .and_then(|position| arguments.get(position).copied())
                .unwrap_or(expression)
        };
        if semantic.context_view(context).facts().any(|fact| {
            fact.payload.match_pattern_comparison(program).is_some_and(
                |(subject, pattern, matched)| {
                    match_patterns::proves(
                        program,
                        subject,
                        pattern,
                        matched,
                        expression,
                        true,
                        &substitute,
                    )
                },
            )
        }) {
            return true;
        }
        if instantiated_live_value_proves(
            program,
            facts,
            call_flow,
            context,
            contexts,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            expression,
            assignments,
        ) {
            return true;
        }
        let required_label = instantiate_call_contract_expression_label(
            program,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            expression,
        );

        let denotations = guard_values::denotations::Denotations {
            program,
            facts,
            caller_state_symbol,
            target_state_symbol,
            statement_index,
            call_frames,
            arguments,
        };
        semantic.context_view(context).facts().any(|fact| {
            let supplied = match fact.payload {
                facts::FactPayload::BooleanValue { expression, value } => Some((expression, value)),
                facts::FactPayload::BooleanExpression(expression) => Some((expression, true)),
                facts::FactPayload::ContractBooleanExpression {
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => Some((expression, true)),
                _ => None,
            };
            if let Some((guard, value)) = supplied {
                let proven = guard_values::proves_with(
                    program,
                    guard,
                    value,
                    expression,
                    true,
                    &|left, right| denotations.equal(left, right),
                );
                if proven || matches!(fact.payload, facts::FactPayload::BooleanValue { .. }) {
                    return proven;
                }
            }
            let candidate_label =
                semantic_boolean_fact_label(program, semantic, fact).or_else(|| {
                    semantic
                        .proposition_fact_label(program, fact)
                        .and_then(|label| label.strip_prefix("boolean:").map(str::to_owned))
                });
            let Some(candidate_label) = candidate_label else {
                return false;
            };

            candidate_label == required_label
                || (expression_is_boolean_place_like(program, expression)
                    && matches!(fact.place, FactPlace::Place(candidate_place)
                        if instantiate_call_contract_expression_label(
                            program,
                            caller_state_symbol,
                            statement_index,
                            call_site,
                            target_state,
                            expression,
                    ) == semantic.place_label(program, candidate_place)))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn instantiated_live_value_proves(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        call_flow: &checked_trees::FlowCallFact,
        context: &facts::FactContext,
        contexts: &[facts::FactContextHandle],
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_site: &validation::CallSite<'_>,
        target: &(impl ContractTargetParameters + ?Sized),
        expression: typed_trees::expression::ExpressionHandle,
        assignments: &mut crate::values::RootAssignmentObservations<'_>,
    ) -> bool {
        use super::prover::{ScalarValue, evaluate_scalar};
        use typed_trees::expression::ExpressionNode;

        let semantic = &facts.semantic;
        let parameters = target.contract_parameters(program);
        let arguments = validation::call_site_argument_expressions(program, call_site);
        evaluate_scalar(
            program,
            expression,
            &mut |formal| {
                let argument = match program.expression_table.expression(formal) {
                    ExpressionNode::Name(path)
                        if path.symbol.is_valid()
                            && expressions(program).name_path_members(path.members).len() == 1 =>
                    {
                        let position = typed_trees::signature::state_parameter_position(
                            parameters,
                            path.symbol,
                        )?;
                        *arguments.get(position)?
                    }
                    _ => {
                        return projected_formal_leaf_value(
                            program,
                            facts,
                            call_flow,
                            contexts,
                            caller_state_symbol,
                            statement_index,
                            parameters,
                            arguments,
                            formal,
                            assignments,
                        );
                    }
                };
                evaluate_scalar(
                    program,
                    argument,
                    &mut |leaf| {
                        if let Some(place) =
                            place_at(program, caller_state_symbol, statement_index, leaf)
                            && let Some(value) = scalar_value_at_observation(
                                program,
                                caller_state_symbol,
                                statement_index,
                                validation::expression_result_type_reference(
                                    program,
                                    program.machine_holding_state(caller_state_symbol)?,
                                    program.state_by_symbol(caller_state_symbol)?,
                                    leaf,
                                )?,
                                &place,
                                assignments,
                            )
                        {
                            return Some(value);
                        }
                        semantic.context_view(context).facts().find_map(|fact| {
                            let facts::FactPayload::BooleanValue {
                                expression: guard,
                                value,
                            } = fact.payload
                            else {
                                return None;
                            };
                            [true, false]
                                .into_iter()
                                .find(|required| {
                                    guard_values::proves(program, guard, value, leaf, *required)
                                })
                                .map(ScalarValue::Boolean)
                        })
                    },
                    &|_, _| true,
                )
            },
            &|_, _| true,
        ) == Some(ScalarValue::Boolean(true))
    }

    /*
        Read a callee projection's value on the actual referent. `room.count` under
        `requires room.count < bound` resolves to the argument's storage plus the
        retained member suffix (`rooms[i].count`); the live snapshot at that exact
        place is the only evidence this route accepts, and a runtime index segment
        narrows to its currently recorded element or the read refuses.
    */
    #[allow(clippy::too_many_arguments)]
    fn projected_formal_leaf_value(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        call_flow: &checked_trees::FlowCallFact,
        contexts: &[facts::FactContextHandle],
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        parameters: &[typed_trees::signature::StateParameter],
        arguments: &[typed_trees::expression::ExpressionHandle],
        leaf: typed_trees::expression::ExpressionHandle,
        assignments: &mut crate::values::RootAssignmentObservations<'_>,
    ) -> Option<facts::ScalarValue> {
        let semantic = &facts.semantic;
        let length_receiver = expressions(program)
            .member_node(leaf)
            .filter(|member| {
                member.case_variant.is_none()
                    && !member.member_symbol.is_valid()
                    && CollectionMeasure::from_authored_spelling(member.member.as_str())
                        == Some(CollectionMeasure::Length)
            })
            .map(|member| member.receiver);
        let leaf = length_receiver.unwrap_or(leaf);
        let mut required = crate::flow::projected_leaf_place(program, leaf)?;
        let formal = required.root.as_symbol()?;
        let position = typed_trees::signature::state_parameter_position(parameters, formal)?;
        let argument = *arguments.get(position)?;
        let observed_type = validation::project_type_reference_from_segments(
            program,
            parameters[position].type_reference,
            &required.segments,
        )?;
        let mut actual = place_at(program, caller_state_symbol, statement_index, argument)?;
        let actual_prefix_length = actual.segments.len();
        actual.segments.append(&mut required.segments);
        for segment in &mut actual.segments {
            let facts::PlaceSegment::Index { expression } = *segment else {
                continue;
            };
            let Some(selector) =
                place_at(program, caller_state_symbol, statement_index, expression)
            else {
                continue;
            };
            let Some(facts::ScalarValue::Integer(value)) = scalar_value_at_observation(
                program,
                caller_state_symbol,
                statement_index,
                validation::expression_result_type_reference(
                    program,
                    program.machine_holding_state(caller_state_symbol)?,
                    program.state_by_symbol(caller_state_symbol)?,
                    expression,
                )?,
                &selector,
                assignments,
            ) else {
                continue;
            };
            if let Some(index) = value.to_u64().and_then(|value| usize::try_from(value).ok()) {
                *segment = facts::PlaceSegment::FixedIndex { index };
            }
        }
        if length_receiver.is_some() {
            if let Some(length) = validation::fixed_array_type_length(program, observed_type) {
                return Some(facts::ScalarValue::Integer(
                    numerics::bignum::BigInt::from_u64(u64::try_from(length).ok()?),
                ));
            }
            if !observation_carriers_agree(
                program,
                caller_state_symbol,
                statement_index,
                observed_type,
                &actual,
            ) {
                return None;
            }
            let capture = call_flow
                .argument_operands()
                .handle_at(u32::try_from(position).ok()?);
            let length = validation::completed_storage::descriptor_extent(
                program,
                &facts.flow.control,
                capture,
                &actual.segments[actual_prefix_length..],
                call_flow.invocation_storage,
            )
            .or_else(|| {
                let literal = crate::values::literal_at_place(
                    program,
                    semantic,
                    contexts
                        .iter()
                        .map(|context| semantic.contexts.get(*context)),
                    &actual,
                )?;
                u64::try_from(typed_trees::byte_predicates::literal_bytes(program, literal)?.len())
                    .ok()
            })?;
            return Some(facts::ScalarValue::Integer(
                numerics::bignum::BigInt::from_u64(length),
            ));
        }
        scalar_value_at_observation(
            program,
            caller_state_symbol,
            statement_index,
            observed_type,
            &actual,
            assignments,
        )
    }

    /*
        Backing coordinates do not establish the scalar observation's interpretation.
        A recast needs its own checked value transform; matching address or width
        cannot import the backing carrier's live scalar facts.
    */
    fn scalar_value_at_observation(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        observed_type: typed_trees::types::TypeReferenceHandle,
        place: &validation::CanonicalPlace,
        assignments: &mut crate::values::RootAssignmentObservations<'_>,
    ) -> Option<facts::ScalarValue> {
        if !observation_carriers_agree(program, state, statement_index, observed_type, place) {
            return None;
        }
        let mut place = place.clone();
        crate::flow::narrow_pinned_index_segments_with_values(
            program,
            state,
            statement_index,
            &mut place.segments,
            |selector| assignments.scalar_value(selector),
        );
        assignments.scalar_value(&place)
    }

    /*
        A stored observation retains its declaration's interpretation across the
        exact actual/formal substitution. Equal addresses or widths do not suffice.
    */
    fn observation_carriers_agree(
        program: &dyn ::validation::CheckingInput,
        state: SymbolHandle,
        statement_index: usize,
        observed_type: typed_trees::types::TypeReferenceHandle,
        place: &validation::CanonicalPlace,
    ) -> bool {
        let Some(stored_type) =
            validation::canonical_place_type_reference(program, state, statement_index, place)
        else {
            return false;
        };
        let referee = |reference| {
            validation::reference_shell(program, reference).map_or(reference, |shell| shell.referee)
        };
        program.normalized_type_identity(referee(stored_type))
            == program.normalized_type_identity(referee(observed_type))
    }
}
mod domains {
    use crate::flow::canonical_place_from_semantic_place;
    use crate::published::facts::boolean_predicates::CasePremiseAvailability;
    use facts::FactPayload;
    use symbols::SymbolHandle;

    use super::labels::{
        ContractTargetParameters, domain_proves_expression_label,
        instantiate_call_contract_expression_label,
    };

    pub(super) fn prove_boolean_expression_via_context_domain_membership(
        program: &dyn ::validation::CheckingInput,
        semantic: &facts::FactPlan,
        context: &facts::FactContext,
        contexts: &[facts::FactContextHandle],
        point: facts::ProgramPoint,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        let candidate_label = program.expression_table.display_name(expression);
        let availability = CasePremiseAvailability::new(program, semantic, contexts, point);
        context_domain_proves_label(program, semantic, context, &candidate_label, &availability)
    }

    pub(super) fn prove_instantiated_boolean_expression_via_context_domain_membership<'premises>(
        program: &'premises dyn ::validation::CheckingInput,
        semantic: &'premises facts::FactPlan,
        context: &facts::FactContext,
        contexts: &'premises [facts::FactContextHandle],
        caller_state_symbol: SymbolHandle,
        statement_index: usize,
        call_site: &validation::CallSite<'_>,
        target_state: &(impl ContractTargetParameters + ?Sized),
        expression: typed_trees::expression::ExpressionHandle,
        availability: &std::cell::OnceCell<Option<CasePremiseAvailability<'premises>>>,
    ) -> bool {
        let candidate_label = instantiate_call_contract_expression_label(
            program,
            caller_state_symbol,
            statement_index,
            call_site,
            target_state,
            expression,
        );

        let Some(availability) = availability
            .get_or_init(|| {
                let machine = program.machine_holding_state(caller_state_symbol)?;
                Some(CasePremiseAvailability::new(
                    program,
                    semantic,
                    contexts,
                    facts::ProgramPoint::Statement {
                        machine_symbol: machine.symbol,
                        state_symbol: caller_state_symbol,
                        statement_index,
                    },
                ))
            })
            .as_ref()
        else {
            return false;
        };
        context_domain_proves_label(program, semantic, context, &candidate_label, availability)
    }

    /*
        The context-domain scan both membership proofs run once `candidate_label`
        is known: a `Place` fact's canonical or display base label must prove the
        label through the domain's own label rule.
    */
    fn context_domain_proves_label(
        program: &dyn ::validation::CheckingInput,
        semantic: &facts::FactPlan,
        context: &facts::FactContext,
        candidate_label: &str,
        availability: &CasePremiseAvailability<'_>,
    ) -> bool {
        semantic.context_view(context).facts().any(|fact| {
            let (domain_symbol, value, place) = match fact.payload {
                FactPayload::DomainMembership {
                    domain_symbol,
                    value,
                    ..
                }
                | FactPayload::ContractDomainMembership {
                    domain_symbol,
                    value,
                    ..
                } => {
                    let Some(place) = fact.place.as_place() else {
                        return false;
                    };
                    (domain_symbol, value, place)
                }
                _ => return false,
            };
            let Some(subject) =
                canonical_place_from_semantic_place(program, semantic, semantic.places.get(place))
            else {
                return false;
            };
            if !availability.allows(&subject) {
                return false;
            }
            let canonical_base_label = semantic.place_label(program, place);
            let display_base_label = program.expression_table.display_name(value);
            domain_proves_expression_label(
                program,
                domain_symbol,
                &canonical_base_label,
                candidate_label,
            ) || domain_proves_expression_label(
                program,
                domain_symbol,
                &display_base_label,
                candidate_label,
            )
        })
    }
}
mod evaluator {
    /*
        Direct evaluation of a callee's Boolean contract expression at one call
        site. The callee's parameters are replaced by the call's argument
        expressions, and the value is computed from literals, constant integers,
        declared integer ranges, known collection lengths and earlier immutable
        local initializers, without reading the flow fact contexts.

        Two entries build one `ContractExpressionEvaluator` for the call and ask
        it for `boolean_value`:

        - `call_site_proves_boolean_contract_expression`, which
          `prover::call_entry_contexts_prove_boolean_contract_expression` tries
          when the entry contexts do not prove a `requires` expression. It passes
          the checked operator facts, which Boolean equality needs, and an unknown
          value counts as not proved.
        - `call_site_boolean_contract_expression_value`, re-exported from
          `contracts` for `facts::crash_calls::route_substitution`, which refines
          published crash routes before operator facts exist. It returns `None`
          for an unknown value, distinct from `false`, because a crash route may
          be discarded only when its guard is proved false.

        The children extend the evaluator: `booleans` evaluates `&&`, `||`, `!`
        and comparisons, `integers` computes constant values and bounds,
        `collections` computes collection lengths, and `resolution` follows names
        to call arguments, local initializers, and array and struct literal
        members. Name and integer evaluation record the expressions in progress
        and give up on a cycle, which a call to the machine's own states can
        create.
    */
    use checked_trees::{FlowCallFact, FlowStateFact};
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::machine::Machine;
    use typed_trees::signature::StateParameter;
    use typed_trees::state::State;

    mod booleans {
        use crate::checks::contracts::prover::ScalarValue;
        use crate::checks::contracts::prover::closed_boolean_value;
        use crate::checks::contracts::prover::evaluate_scalar;
        use crate::checks::contracts::prover::has_builtin_operators;
        use typed_trees::expression::{
            BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
        };

        use super::{BooleanExpressionOwner as ExpressionOwner, ContractExpressionEvaluator};

        impl ContractExpressionEvaluator<'_, '_> {
            pub(super) fn boolean_value(&mut self, expression: ExpressionHandle) -> Option<bool> {
                self.boolean_value_in_owner(expression, ExpressionOwner::Contract)
            }

            fn boolean_value_in_owner(
                &mut self,
                expression: ExpressionHandle,
                owner: ExpressionOwner,
            ) -> Option<bool> {
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Boolean(value) => Some(*value),
                    ExpressionNode::Binary(binary) => match binary.operator {
                        BinaryOperator::And => Some(
                            self.boolean_value_in_owner(binary.left, owner)?
                                && self.boolean_value_in_owner(binary.right, owner)?,
                        ),
                        BinaryOperator::Or => Some(
                            self.boolean_value_in_owner(binary.left, owner)?
                                || self.boolean_value_in_owner(binary.right, owner)?,
                        ),
                        BinaryOperator::Equal => {
                            if let Some(value) =
                                self.checked_boolean_comparison_value(expression, owner)
                            {
                                Some(value)
                            } else if let (Some(left), Some(right)) = (
                                self.integer_value(binary.left),
                                self.integer_value(binary.right),
                            ) {
                                Some(left == right)
                            } else {
                                let (left_min, left_max) = self.integer_bounds(binary.left)?;
                                let (right_min, right_max) = self.integer_bounds(binary.right)?;
                                (left_min == left_max
                                    && right_min == right_max
                                    && left_min == right_min)
                                    .then_some(true)
                            }
                        }
                        BinaryOperator::Greater => {
                            if let (Some(left), Some(right)) = (
                                self.integer_value(binary.left),
                                self.integer_value(binary.right),
                            ) {
                                Some(left > right)
                            } else {
                                let (left_min, _) = self.integer_bounds(binary.left)?;
                                let (_, right_max) = self.integer_bounds(binary.right)?;
                                (left_min > right_max).then_some(true)
                            }
                        }
                        BinaryOperator::GreaterOrEqual => {
                            if let (Some(left), Some(right)) = (
                                self.integer_value(binary.left),
                                self.integer_value(binary.right),
                            ) {
                                Some(left >= right)
                            } else {
                                let (left_min, _) = self.integer_bounds(binary.left)?;
                                let (_, right_max) = self.integer_bounds(binary.right)?;
                                (left_min >= right_max).then_some(true)
                            }
                        }
                        BinaryOperator::Less => {
                            if let (Some(left), Some(right)) = (
                                self.integer_value(binary.left),
                                self.integer_value(binary.right),
                            ) {
                                Some(left < right)
                            } else {
                                let (_, left_max) = self.integer_bounds(binary.left)?;
                                let (right_min, _) = self.integer_bounds(binary.right)?;
                                (left_max < right_min).then_some(true)
                            }
                        }
                        BinaryOperator::LessOrEqual => {
                            if let (Some(left), Some(right)) = (
                                self.integer_value(binary.left),
                                self.integer_value(binary.right),
                            ) {
                                Some(left <= right)
                            } else {
                                let (_, left_max) = self.integer_bounds(binary.left)?;
                                let (right_min, _) = self.integer_bounds(binary.right)?;
                                (left_max <= right_min).then_some(true)
                            }
                        }
                        BinaryOperator::NotEqual => {
                            if let Some(value) =
                                self.checked_boolean_comparison_value(expression, owner)
                            {
                                Some(value)
                            } else if let (Some(left), Some(right)) = (
                                self.integer_value(binary.left),
                                self.integer_value(binary.right),
                            ) {
                                Some(left != right)
                            } else {
                                let (left_min, left_max) = self.integer_bounds(binary.left)?;
                                let (right_min, right_max) = self.integer_bounds(binary.right)?;
                                (left_max < right_min || right_max < left_min).then_some(true)
                            }
                        }
                        BinaryOperator::Add
                        | BinaryOperator::BitwiseAnd
                        | BinaryOperator::BitwiseOr
                        | BinaryOperator::BitwiseXor
                        | BinaryOperator::Divide
                        | BinaryOperator::Modulo
                        | BinaryOperator::Multiply
                        | BinaryOperator::ShiftLeft
                        | BinaryOperator::ShiftRight
                        | BinaryOperator::Subtract
                        | BinaryOperator::CaseMembership => None,
                    },
                    ExpressionNode::Name(_) => {
                        if self.operators.is_some() {
                            let (resolved, resolved_owner) =
                                self.resolved_boolean_expression(expression, owner)?;
                            return self.boolean_value_in_owner(resolved, resolved_owner);
                        }
                        let resolved = self.resolved_expression(expression)?;
                        (resolved != expression)
                            .then(|| self.boolean_value(resolved))
                            .flatten()
                    }
                    ExpressionNode::Borrow(inner) => {
                        self.boolean_value_in_owner(inner.target, owner)
                    }
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                        Some(!self.boolean_value_in_owner(unary.operand, owner)?)
                    }
                    _ => None,
                }
            }

            fn checked_boolean_comparison_value(
                &mut self,
                expression: ExpressionHandle,
                owner: ExpressionOwner,
            ) -> Option<bool> {
                self.guarding_cycles(super::ExpressionWork::Evaluation, expression, |evaluator| {
                    evaluator.checked_boolean_comparison_value_without_cycle(expression, owner)
                })
            }

            fn checked_boolean_comparison_value_without_cycle(
                &mut self,
                expression: ExpressionHandle,
                owner: ExpressionOwner,
            ) -> Option<bool> {
                let operators = self.operators?;
                if !has_builtin_operators(self.program, operators, expression)
                    || !self.builtin_boolean_meaning(expression, owner)
                {
                    return None;
                }
                if let Some(value) = closed_boolean_value(self.program, operators, expression) {
                    return Some(value);
                }
                let value = evaluate_scalar(
                    self.program,
                    expression,
                    &mut |leaf| {
                        let (resolved, resolved_owner) =
                            self.resolved_boolean_expression(leaf, owner)?;
                        self.boolean_value_in_owner(resolved, resolved_owner)
                            .map(ScalarValue::Boolean)
                    },
                    &|_, _| true,
                )?;
                let ScalarValue::Boolean(value) = value else {
                    return None;
                };
                Some(value)
            }

            fn builtin_boolean_meaning(
                &mut self,
                expression: ExpressionHandle,
                owner: ExpressionOwner,
            ) -> bool {
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Boolean(_) => true,
                    ExpressionNode::Name(path) => self
                        .program
                        .expression_table
                        .name_path_is_single_member(path),
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                        self.builtin_boolean_meaning(unary.operand, owner)
                    }
                    ExpressionNode::Binary(binary) => {
                        let selected_builtin = match binary.operator {
                            BinaryOperator::And | BinaryOperator::Or => true,
                            BinaryOperator::Equal | BinaryOperator::NotEqual => {
                                let Some(left_type) = self.boolean_operand_type(binary.left, owner)
                                else {
                                    return false;
                                };
                                let Some(right_type) =
                                    self.boolean_operand_type(binary.right, owner)
                                else {
                                    return false;
                                };
                                let owner_symbol = match owner {
                                    ExpressionOwner::Caller => self.caller_machine.symbol,
                                    ExpressionOwner::Contract => {
                                        let Some((target_machine, _)) = validation::find_state(
                                            self.program,
                                            self.target_symbol,
                                        ) else {
                                            return false;
                                        };
                                        target_machine.symbol
                                    }
                                };
                                typed_trees::operator::has_builtin_spelled_expression_meaning(
                                    self.program,
                                    owner_symbol,
                                    expression,
                                    if binary.operator == BinaryOperator::Equal {
                                        language_core::OperatorSpelling::Equal
                                    } else {
                                        language_core::OperatorSpelling::NotEqual
                                    },
                                    &[Some(left_type), Some(right_type)],
                                )
                            }
                            _ => false,
                        };
                        selected_builtin
                            && self.builtin_boolean_meaning(binary.left, owner)
                            && self.builtin_boolean_meaning(binary.right, owner)
                    }
                    _ => false,
                }
            }

            fn boolean_operand_type(
                &mut self,
                expression: ExpressionHandle,
                owner: ExpressionOwner,
            ) -> Option<typed_trees::types::TypeReferenceHandle> {
                use typed_trees::types::{PrimitiveType, TypeReferenceNode};
                let reference = match self.program.expression_table.expression(expression) {
                    ExpressionNode::Name(path) => {
                        if !self
                            .program
                            .expression_table
                            .name_path_is_single_member(path)
                        {
                            return None;
                        }
                        let parameters = match owner {
                            ExpressionOwner::Contract => self.target_parameters,
                            ExpressionOwner::Caller => {
                                self.program.state_parameters(self.caller_state)
                            }
                        };
                        parameters
                            .iter()
                            .find(|parameter| parameter.symbol == path.symbol)
                            .map(|parameter| parameter.type_reference)
                            .or_else(|| match owner {
                                ExpressionOwner::Contract => None,
                                ExpressionOwner::Caller => self
                                    .program
                                    .state_local_declaration(self.caller_state, path.symbol)
                                    .filter(|&(ordinal, _)| ordinal < self.statement_index)
                                    .map(|(_, local)| local.type_reference),
                            })?
                    }
                    ExpressionNode::Boolean(_)
                    | ExpressionNode::Unary(_)
                    | ExpressionNode::Binary(_) => {
                        let mut reference = self
                            .target_parameters
                            .iter()
                            .chain(self.program.state_parameters(self.caller_state))
                            .find(|parameter| {
                                self.program
                                    .primitive_type_reference(parameter.type_reference)
                                    == Some(PrimitiveType::Bool)
                            })?
                            .type_reference;
                        while let TypeReferenceNode::Constrained { base_type, .. } =
                            self.program.type_reference_table.type_reference(reference)
                        {
                            reference = *base_type;
                        }
                        reference
                    }
                    _ => return None,
                };
                (self.program.primitive_type_reference(reference) == Some(PrimitiveType::Bool))
                    .then_some(reference)
            }
        }
    }
    mod collections {
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};

        use super::ContractExpressionEvaluator;

        impl<'program> ContractExpressionEvaluator<'program, '_> {
            pub(super) fn collection_length(
                &mut self,
                expression: ExpressionHandle,
            ) -> Option<usize> {
                let resolved = self.resolved_expression(expression).unwrap_or(expression);
                match self.program.expression_table.expression(resolved) {
                    ExpressionNode::ArrayLiteral(values) => Some(values.count() as usize),
                    ExpressionNode::Borrow(inner) => self.collection_length(inner.target),
                    _ => validation::declared_collection_extent(
                        self.program,
                        self.caller_machine,
                        Some(self.caller_state),
                        resolved,
                    ),
                }
            }
        }
    }
    mod integers {
        use language_semantics::declaration_selection::CollectionMeasure;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};

        use super::ContractExpressionEvaluator;

        impl ContractExpressionEvaluator<'_, '_> {
            /*
                The strongest inexpensive interval known for an integer expression at
                this call site. Contract parameters first rebind to the caller's actual
                argument; an Exact declared range on that argument is a store-enforced
                invariant and may therefore discharge a callee comparison even when the
                argument's initializer is not constant-foldable.
            */
            pub(super) fn integer_bounds(
                &mut self,
                expression: ExpressionHandle,
            ) -> Option<(i64, i64)> {
                if let Some(value) = self.integer_value(expression) {
                    return Some((value, value));
                }

                let path = self.program.expression_table.name_path_node(expression)?;
                let name = self
                    .program
                    .expression_table
                    .name_path_members(path.members)
                    .last()
                    .map(|name| name.as_str());
                let argument = self.argument_for_parameter(path.head_symbol, path.symbol, name)?;

                if let Some(value) = self.integer_value(argument) {
                    return Some((value, value));
                }
                crate::checks::ranges::expression_enforced_declared_range(
                    self.program,
                    self.caller_machine,
                    self.caller_state,
                    argument,
                )
            }

            pub(super) fn integer_value(&mut self, expression: ExpressionHandle) -> Option<i64> {
                self.guarding_cycles(super::ExpressionWork::Evaluation, expression, |evaluator| {
                    evaluator.integer_value_of_resolved(expression)
                })
            }

            fn integer_value_of_resolved(&mut self, expression: ExpressionHandle) -> Option<i64> {
                let expression = self.resolved_expression(expression).unwrap_or(expression);
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Binary(binary) => {
                        let left = self.integer_value(binary.left)?;
                        let right = self.integer_value(binary.right)?;
                        crate::checks::ranges::folded_integer_binary(left, binary.operator, right)
                    }
                    ExpressionNode::Integer(value) => value.value_i64(),
                    ExpressionNode::Member(member)
                        if CollectionMeasure::from_authored_spelling(member.member.as_str())
                            == Some(CollectionMeasure::Length) =>
                    {
                        self.collection_length(member.receiver)
                            .and_then(|length| i64::try_from(length).ok())
                    }
                    ExpressionNode::Borrow(inner) => self.integer_value(inner.target),
                    _ => None,
                }
            }
        }
    }
    mod resolution {
        use crate::flow::expressions;
        use crate::flow::state_statements;
        use symbols::SymbolHandle;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode};

        use super::{BooleanExpressionOwner, ContractExpressionEvaluator};

        impl ContractExpressionEvaluator<'_, '_> {
            /*
                Substitute a contract formal once, then retain caller ownership through
                local resolution. Recursive calls share symbols but not these namespaces.
            */
            pub(super) fn resolved_boolean_expression(
                &mut self,
                expression: ExpressionHandle,
                owner: BooleanExpressionOwner,
            ) -> Option<(ExpressionHandle, BooleanExpressionOwner)> {
                let path = self.program.expression_table.name_path_node(expression)?;
                if !self
                    .program
                    .expression_table
                    .name_path_is_single_member(path)
                {
                    return None;
                }
                let resolved = match owner {
                    BooleanExpressionOwner::Contract => {
                        self.argument_for_parameter(path.head_symbol, path.symbol, None)?
                    }
                    BooleanExpressionOwner::Caller => {
                        self.local_initializer(path.head_symbol, path.symbol, path.members)?
                    }
                };
                Some((resolved, BooleanExpressionOwner::Caller))
            }

            pub(super) fn resolved_expression(
                &mut self,
                expression: ExpressionHandle,
            ) -> Option<ExpressionHandle> {
                self.guarding_cycles(super::ExpressionWork::Resolution, expression, |evaluator| {
                    evaluator.resolved_expression_followed(expression)
                })
            }

            fn resolved_expression_followed(
                &mut self,
                expression: ExpressionHandle,
            ) -> Option<ExpressionHandle> {
                match self.program.expression_table.expression(expression) {
                    ExpressionNode::Borrow(inner) => self
                        .resolved_expression(inner.target)
                        .or(Some(inner.target)),
                    ExpressionNode::Name(path) => {
                        let name = self
                            .program
                            .expression_table
                            .name_path_members(path.members)
                            .last()
                            .map(|name| name.as_str());
                        self.argument_for_parameter(path.head_symbol, path.symbol, name)
                            .or_else(|| {
                                self.local_initializer(path.head_symbol, path.symbol, path.members)
                            })
                            .and_then(|resolved| {
                                if resolved == expression {
                                    return Some(resolved);
                                }
                                self.resolved_expression(resolved).or(Some(resolved))
                            })
                    }
                    ExpressionNode::Indexed(indexed) => {
                        let collection = self.resolved_expression(indexed.collection)?;
                        let index = usize::try_from(self.integer_value(indexed.index)?).ok()?;
                        let values = self
                            .program
                            .expression_table
                            .array_literal_node(collection)?;
                        expressions(self.program)
                            .expression_handles(*values)
                            .get(index)
                            .copied()
                    }
                    ExpressionNode::Member(member) => {
                        let receiver = self.resolved_expression(member.receiver)?;
                        let struct_literal = self
                            .program
                            .expression_table
                            .struct_literal_node(receiver)?;
                        expressions(self.program)
                            .struct_fields(struct_literal.fields)
                            .iter()
                            .find(|field| field.name == member.member)
                            .map(|field| field.value)
                    }
                    _ => None,
                }
            }

            pub(super) fn argument_for_parameter(
                &mut self,
                head_symbol: SymbolHandle,
                symbol: SymbolHandle,
                name: Option<&str>,
            ) -> Option<ExpressionHandle> {
                let arguments = crate::shared::call_formals::arguments_aligned_to_formals(
                    self.program,
                    self.call_site.target_symbol(self.program),
                    self.target_parameters.len(),
                    validation::call_site_argument_expressions(self.program, self.call_site),
                );
                let mut argument_index = 0usize;

                for parameter in self.target_parameters {
                    let has_resolved_symbol = head_symbol.is_valid() || symbol.is_valid();
                    let parameter_matches = if has_resolved_symbol {
                        (head_symbol.is_valid() && head_symbol == parameter.symbol)
                            || (symbol.is_valid() && symbol == parameter.symbol)
                    } else {
                        name.is_some_and(|name| name == parameter.name.as_str())
                    };

                    let argument = arguments.get(argument_index).copied();
                    argument_index = argument_index.saturating_add(1);
                    if parameter_matches {
                        return argument;
                    }
                }

                None
            }

            fn local_initializer(
                &mut self,
                head_symbol: SymbolHandle,
                symbol: SymbolHandle,
                members: arena::HandleSpan<typed_trees::name::Identifier>,
            ) -> Option<ExpressionHandle> {
                let name = self
                    .program
                    .expression_table
                    .name_path_members(members)
                    .last()?;
                state_statements(self.program, self.caller_state.statement_nodes)
                    .iter()
                    .take(self.statement_index)
                    .find_map(|statement| {
                        let local = statement.as_local_data()?;
                        let has_resolved_symbol = head_symbol.is_valid() || symbol.is_valid();
                        let local_matches = if has_resolved_symbol {
                            (head_symbol.is_valid() && head_symbol == local.symbol)
                                || (symbol.is_valid() && symbol == local.symbol)
                        } else {
                            local.name == *name
                        };
                        let initializer = local.ordinary_value_initializer()?;
                        (local_matches && !local.is_mutable && self.closed_value(initializer))
                            .then_some(initializer)
                    })
            }

            fn closed_value(&mut self, expression: ExpressionHandle) -> bool {
                if !self
                    .program
                    .expression_table
                    .expression_is_valid(expression)
                {
                    return false;
                }
                matches!(
                    self.program.expression_table.expression(expression),
                    ExpressionNode::Boolean(_)
                        | ExpressionNode::Integer(_)
                        | ExpressionNode::Float(_)
                        | ExpressionNode::String(_)
                )
            }
        }
    }

    #[derive(Clone, Copy)]
    enum BooleanExpressionOwner {
        Contract,
        Caller,
    }

    pub(super) fn call_site_proves_boolean_contract_expression(
        program: &dyn ::validation::CheckingInput,
        operators: &checked_trees::CheckedOperatorFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        call_site: &validation::CallSite<'_>,
        target_symbol: symbols::SymbolHandle,
        target_parameters: &[StateParameter],
        expression: ExpressionHandle,
    ) -> bool {
        call_site_boolean_contract_expression_value(
            program,
            Some(operators),
            state_flow,
            call_flow,
            call_site,
            target_symbol,
            target_parameters,
            expression,
        )
        .unwrap_or(false)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn call_site_boolean_contract_expression_value<'program>(
        program: &'program dyn ::validation::CheckingInput,
        operators: Option<&'program checked_trees::CheckedOperatorFacts>,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        call_site: &validation::CallSite<'program>,
        target_symbol: symbols::SymbolHandle,
        target_parameters: &'program [StateParameter],
        expression: ExpressionHandle,
    ) -> Option<bool> {
        let caller_state = validation::find_state_in_machine(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        )?;
        let caller_machine = program.machine_by_symbol(state_flow.machine_symbol)?;

        ContractExpressionEvaluator {
            program,
            operators,
            caller_machine,
            caller_state,
            statement_index: call_flow.statement_index,
            call_site,
            target_symbol,
            target_parameters,
            active_evaluations: Vec::new(),
            active_resolutions: Vec::new(),
        }
        .boolean_value(expression)
    }

    pub(super) struct ContractExpressionEvaluator<'program, 'call> {
        program: &'program dyn ::validation::CheckingInput,
        /*
            Source call checking has selected operator facts. Earlier crash-fact
            construction does not, and cannot gain Boolean equality authority here.
        */
        operators: Option<&'program checked_trees::CheckedOperatorFacts>,
        caller_machine: &'program Machine,
        caller_state: &'program State,
        statement_index: usize,
        call_site: &'call validation::CallSite<'program>,
        target_symbol: symbols::SymbolHandle,
        target_parameters: &'program [StateParameter],
        /*
            Expressions whose integer evaluation is in progress on the call
            stack. Following a SELF call site maps a parameter back to an
            argument that mentions that same parameter (`n` resolves to
            `n - 1`, whose `n` resolves to `n - 1` again), so the constant
            walk would otherwise re-enter the same expression handle forever.
        */
        active_evaluations: Vec<ExpressionHandle>,
        /*
            Expressions whose name resolution is in progress on the call
            stack; catches resolution-only cycles (an argument swap such as
            `self.step(b, a)` makes parameter `a` resolve to `b` and back)
            that never pass through integer evaluation.
        */
        active_resolutions: Vec<ExpressionHandle>,
    }

    #[derive(Clone, Copy)]
    enum ExpressionWork {
        Evaluation,
        Resolution,
    }

    impl ContractExpressionEvaluator<'_, '_> {
        /*
            Runs `evaluate` with `expression` marked active in `active`. If the
            expression is already active, the walk has looped back into an
            expression it is still working on -- a cycle through call-site
            argument following -- so stand down with None. Unknown never proves
            a contract and never falsely rejects one: the caller simply falls
            back to the other provers (arm facts, caller requires).
        */
        fn guarding_cycles<T>(
            &mut self,
            work: ExpressionWork,
            expression: ExpressionHandle,
            evaluate: impl FnOnce(&mut Self) -> Option<T>,
        ) -> Option<T> {
            let active = match work {
                ExpressionWork::Evaluation => &mut self.active_evaluations,
                ExpressionWork::Resolution => &mut self.active_resolutions,
            };
            if active.contains(&expression) {
                return None;
            }
            active.push(expression);
            let value = evaluate(self);
            match work {
                ExpressionWork::Evaluation => self.active_evaluations.pop(),
                ExpressionWork::Resolution => self.active_resolutions.pop(),
            };
            value
        }
    }
}
#[path = "checks_contracts_prover.rs"]
pub(in crate::checks) mod prover;
pub(crate) use prover::call_guarantees::captured_place as captured_guarantee_place;
pub(crate) use prover::call_guarantees::current_predicates as current_call_guarantee_predicates;
pub(crate) use prover::call_guarantees::{GuaranteeUse, proves_captured_predicates};
pub(crate) use prover::contexts_prove_proposition_label;
pub(crate) use prover::{BooleanValueQueries, boolean_value_at_place};

/* Provers the call requirement and exit checks consult directly. */
mod call_bounds {
    /*
        Call requirements over standing integer bounds and live immutable captures.
    */
    use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus, FlowCallFact, FlowStateFact};
    use language_core::OperatorSpelling;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

    mod context {
        /*
            Integer comparison entailment from surviving call-entry facts.

            The arithmetic engine receives current parameter atoms, landed literals,
            and independently selected Exact addition/subtraction/multiplication trees.
            Caller storage and earlier initializers are never replayed.
            View metadata uses the selected arrival state's formal coordinates for
            both machine entries and named-state transfers. Only surviving caller
            facts are premises; an arrival's requirements remain obligations.
        */
        use super::{
            BinaryOperator, CheckFacts, ExpressionHandle, ExpressionNode, FlowCallFact,
            FlowStateFact, OperatorSpelling,
        };
        use crate::checks::termination::proves_ranked_entry_requirement;
        use crate::flow::shared_call_frames_or;
        use crate::published::facts::contract_occurrences::append_expression_occurrences;
        use crate::values::scalar_expression_type;
        use symbols::SymbolHandle;
        use typed_trees::signature::StateParameter;
        use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
        use validation::{
            StrictArithmeticBindingValue, StrictArithmeticImplicationJudgment,
            StrictArithmeticSymbolBinding,
        };

        mod arguments {
            /*
                Exact integer actuals admitted to mathematical call-requirement substitution.

                This is meaning custody, not an overflow proof. Normal source formation and
                Terminal operation obligations still independently discharge Exact arithmetic.
            */
            use super::{
                CheckFacts, ExpressionHandle, ExpressionNode, PrimitiveType, StateParameter,
                SymbolHandle, TypeReferenceHandle,
            };
            use crate::checks::arithmetic_spelling;
            use crate::checks::contracts::call_bounds::context::comparison_is_admitted;
            use crate::checks::contracts::call_bounds::context::direct_parameter;
            use crate::checks::contracts::call_bounds::context::fixed_parameter_type;
            use crate::checks::contracts::prover::has_builtin_operators;
            use crate::flow::frame_storage_writes;
            use crate::published::facts::contract_occurrences::append_expression_occurrences;
            use crate::values::scalar_expression_type;
            use numerics::arithmetic::ArithmeticDomain;

            pub(super) fn primitive_type(
                program: &dyn ::validation::CheckingInput,
                facts: &CheckFacts,
                owner: SymbolHandle,
                parameters: &[StateParameter],
                expression: ExpressionHandle,
            ) -> Option<PrimitiveType> {
                meaning(program, facts, owner, parameters, expression)
                    .map(|meaning| meaning.primitive)
            }

            /*
                A numeric actual is evaluated before later operands. Current call-entry
                atoms represent that captured value only if those operands preserve it.
            */
            pub(super) fn capture_is_current<'program>(
                program: &'program dyn ::validation::CheckingInput,
                facts: &CheckFacts,
                machine: &'program typed_trees::machine::Machine,
                state: &typed_trees::state::State,
                statement_index: usize,
                argument: ExpressionHandle,
                later_arguments: &[ExpressionHandle],
                frames: Option<&validation::CallFrameResolver<'program>>,
            ) -> bool {
                let parameters = program.state_parameters(state);
                let mut occurrences = Vec::new();
                append_expression_occurrences(program, argument, &mut occurrences);
                let captured = occurrences
                    .iter()
                    .filter_map(|expression| {
                        direct_parameter(program, parameters, *expression)
                            .filter(|parameter| parameter.is_mutable)
                            .map(|parameter| facts::PlaceRoot::Symbol(parameter.symbol))
                    })
                    .collect::<Vec<_>>();
                if captured.is_empty() || later_arguments.is_empty() {
                    return true;
                }
                let Some(frames) = frames else {
                    return false;
                };
                later_arguments.iter().all(|expression| {
                    if !operand_effects_are_known(
                        program,
                        facts,
                        machine.symbol,
                        parameters,
                        *expression,
                    ) {
                        return false;
                    }
                    let frame = frames.expression_write_frame(machine, *expression);
                    frame_storage_writes(
                        program,
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        &frame,
                        Some(frames),
                    )
                    .is_some_and(|writes| {
                        writes.iter().all(|write| {
                            let root = write.root;
                            !captured.contains(&root)
                        })
                    })
                })
            }

            /*
                Call write frames do not describe implicit selected-operator effects.
                Admit ordinary calls and independently builtin scalar operands only;
                other forms need their own complete expression-effect interpretation.
            */
            fn operand_effects_are_known(
                program: &dyn ::validation::CheckingInput,
                facts: &CheckFacts,
                owner: SymbolHandle,
                parameters: &[StateParameter],
                expression: ExpressionHandle,
            ) -> bool {
                use typed_trees::SubtreeVisit;
                let known =
                    |expression| primitive_type(program, facts, owner, parameters, expression);
                program.expression_table.expression_is_valid(expression)
                    && !program.subtree_any(
                        expression,
                        typed_trees::SubtreeEdges::VALUES,
                        |handle, node| match node {
                            ExpressionNode::Name(_)
                            | ExpressionNode::Integer(_)
                            | ExpressionNode::Boolean(_)
                            | ExpressionNode::String(_)
                            | ExpressionNode::Float(_)
                            | ExpressionNode::ZeroValue(_)
                            | ExpressionNode::Borrow(_)
                            | ExpressionNode::Call(_) => SubtreeVisit::Descend,
                            ExpressionNode::Cast(_) if known(handle).is_some() => {
                                SubtreeVisit::Prune
                            }
                            ExpressionNode::Binary(_)
                                if known(handle).is_some()
                                    || comparison_is_admitted(
                                        program, facts, owner, parameters, handle,
                                    ) =>
                            {
                                SubtreeVisit::Prune
                            }
                            _ => SubtreeVisit::Found,
                        },
                    )
            }

            struct Meaning {
                primitive: PrimitiveType,
                domain: ArithmeticDomain,
                type_reference: Option<TypeReferenceHandle>,
            }

            fn meaning(
                program: &dyn ::validation::CheckingInput,
                facts: &CheckFacts,
                owner: SymbolHandle,
                parameters: &[StateParameter],
                expression: ExpressionHandle,
            ) -> Option<Meaning> {
                match program.expression_table.expression(expression) {
                    ExpressionNode::Name(_) => {
                        let parameter = direct_parameter(program, parameters, expression)?;
                        Some(Meaning {
                            primitive: fixed_parameter_type(program, parameter)?,
                            domain: program
                                .arithmetic_domain_for_type_reference(parameter.type_reference),
                            type_reference: Some(parameter.type_reference),
                        })
                    }
                    ExpressionNode::Member(_) => {
                        let machine = program.machine_by_symbol(owner)?;
                        match validation::canonical_place_from_expression(program, expression) {
                            Some(place)
                                if place.root == facts::PlaceRoot::Symbol(machine.symbol) => {}
                            _ => return None,
                        }
                        let type_reference = validation::declared_place_type_raw(
                            program,
                            machine,
                            program.machine_states(machine).first(),
                            expression,
                        )?;
                        let primitive = program.primitive_type_reference(type_reference)?;
                        primitive.is_fixed_integer().then_some(Meaning {
                            primitive,
                            domain: program.arithmetic_domain_for_type_reference(type_reference),
                            type_reference: Some(type_reference),
                        })
                    }
                    ExpressionNode::Integer(literal) => {
                        let primitive = scalar_expression_type(
                            &checked_trees::CheckedScalarExpression::IntegerLiteral {
                                literal: literal.clone(),
                            },
                        )?;
                        primitive.is_fixed_integer().then_some(Meaning {
                            primitive,
                            domain: literal.landing()?.domain,
                            type_reference: None,
                        })
                    }
                    ExpressionNode::Cast(cast) => {
                        if !validation::plain_exact_widening_cast(program, cast) {
                            return None;
                        }
                        let source = meaning(program, facts, owner, parameters, cast.value)?;
                        let primitive = program.primitive_type_reference(cast.target_type)?;
                        validation::integer_widen_is_total(source.primitive, primitive).then_some(
                            Meaning {
                                primitive,
                                domain: ArithmeticDomain::Exact,
                                type_reference: Some(cast.target_type),
                            },
                        )
                    }
                    ExpressionNode::Binary(binary) => {
                        let spelling = arithmetic_spelling(binary.operator)?;
                        let left = meaning(program, facts, owner, parameters, binary.left);
                        let right = meaning(program, facts, owner, parameters, binary.right);
                        let anonymous = |primitive| Meaning {
                            primitive,
                            domain: ArithmeticDomain::Exact,
                            type_reference: None,
                        };
                        let (left, right) = match (left, right) {
                            (Some(left), Some(right)) => (left, right),
                            (Some(left), None) if is_unlanded_integer(program, binary.right) => {
                                let right = anonymous(left.primitive);
                                (left, right)
                            }
                            (None, Some(right)) if is_unlanded_integer(program, binary.left) => {
                                let left = anonymous(right.primitive);
                                (left, right)
                            }
                            _ => return None,
                        };
                        if left.primitive != right.primitive
                            || left.domain != ArithmeticDomain::Exact
                            || right.domain != ArithmeticDomain::Exact
                            || !has_builtin_operators(program, &facts.operators, expression)
                            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                                program,
                                owner,
                                expression,
                                spelling,
                                &[left.type_reference, right.type_reference],
                            )
                        {
                            return None;
                        }
                        Some(Meaning {
                            primitive: left.primitive,
                            domain: ArithmeticDomain::Exact,
                            type_reference: None,
                        })
                    }
                    _ => None,
                }
            }

            pub(super) fn is_unlanded_integer(
                program: &dyn ::validation::CheckingInput,
                expression: ExpressionHandle,
            ) -> bool {
                program.expression_table.expression_is_valid(expression)
                    && matches!(program.expression_table.expression(expression),
                        ExpressionNode::Integer(literal) if literal.landing().is_none())
            }
        }

        /*
            Every surviving fact across the context roster, minus the call's own
            requires/ensures — those are the obligation, not the premise.
        */
        fn roster_premise_facts<'f>(
            semantic: &'f facts::FactPlan,
            contexts: &'f [facts::FactContextHandle],
        ) -> impl Iterator<Item = &'f facts::Fact> + 'f {
            contexts
                .iter()
                .flat_map(|context| {
                    semantic
                        .context_view(semantic.contexts.get(*context))
                        .facts()
                })
                .filter(|fact| {
                    !matches!(
                        fact.origin,
                        facts::FactOrigin::CallRequires | facts::FactOrigin::CallEnsures
                    )
                })
        }

        pub(in crate::checks::contracts) fn proves(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &FlowStateFact,
            call: &FlowCallFact,
            contexts: &[facts::FactContextHandle],
            goal: ExpressionHandle,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> bool {
            prove(program, facts, caller, call, contexts, goal, call_frames).unwrap_or(false)
        }

        fn prove(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &FlowStateFact,
            call: &FlowCallFact,
            contexts: &[facts::FactContextHandle],
            goal: ExpressionHandle,
            call_frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Option<bool> {
            let site = validation::find_call_site(
                program,
                caller.machine_symbol,
                caller.state_symbol,
                call.statement_index,
                call.call_ordinal,
            )?;
            let ordinary_call = match &site {
                validation::CallSite::Expression { call: source, .. } => {
                    validation::call_target_entry_symbol(
                        program,
                        validation::call_target_symbol(program, source),
                    ) == call.target_symbol
                        && (!source.receiver.is_valid()
                            || validation::canonical_place_from_expression(
                                program,
                                source.receiver,
                            )
                            .is_some()
                            || matches!(
                                program.expression_table.expression(source.receiver),
                                ExpressionNode::Name(path)
                                    if is_data_namespace(program, path.symbol)
                                        && (path.head_symbol == path.symbol
                                            || !path.head_symbol.is_valid())
                            ))
                        && source.static_requirement_dispatch.is_none()
                        && source.quotient_operation.is_none()
                        && source.private_layout_operation.is_none()
                        && source.machine_arguments.is_empty()
                        && source.evidence_arguments.is_empty()
                }
                validation::CallSite::Statement(source) => {
                    let admissible_receiver = (source.receiver.is_empty()
                        && !source.receiver_symbol.is_valid()
                        && !source.receiver_root_symbol.is_valid())
                        || (is_data_namespace(program, source.receiver_symbol)
                            && (source.receiver_root_symbol == source.receiver_symbol
                                || !source.receiver_root_symbol.is_valid()))
                        || (!source.receiver.is_empty()
                            && source.receiver_root_symbol.is_valid()
                            && validation::find_state_in_machine(
                                program,
                                caller.machine_symbol,
                                caller.state_symbol,
                            )
                            .is_some_and(|caller_state| {
                                validation::projected_statement_receiver_place(
                                    program,
                                    caller_state,
                                    call.statement_index,
                                    source,
                                )
                                .is_some()
                            }));
                    validation::call_target_entry_symbol(program, source.target_symbol)
                        == call.target_symbol
                        && admissible_receiver
                        && source.static_requirement_dispatch.is_none()
                        && source.machine_arguments.is_empty()
                        && source.evidence_arguments.is_empty()
                }
                validation::CallSite::TransitionNamed {
                    path,
                    evidence_arguments,
                    ..
                } => {
                    validation::call_target_state(program, path.symbol)
                        .map_or(path.symbol, |state| state.symbol)
                        == call.target_symbol
                        && (path.head_symbol == path.symbol
                            || path.head_symbol == caller.machine_symbol
                            || is_data_namespace(program, path.head_symbol))
                        && evidence_arguments.is_empty()
                }
            };
            if !ordinary_call {
                return None;
            }
            let caller_machine = program.machine_by_symbol(caller.machine_symbol)?;
            let caller_state = validation::find_state_in_machine(
                program,
                caller.machine_symbol,
                caller.state_symbol,
            )?;
            let head = validation::find_machine_head(program, call.target_symbol);
            let (callee, entry) = match head {
                Some(head) => head,
                None => {
                    validation::find_state(program, call.target_symbol).filter(|(machine, _)| {
                        machine.symbol == caller.machine_symbol
                            && matches!(site, validation::CallSite::TransitionNamed { .. })
                    })?
                }
            };
            let parameters = program.state_parameters(entry);
            if head.is_some()
                && caller.machine_symbol == callee.symbol
                && matches!(site, validation::CallSite::TransitionNamed { .. })
                && proves_ranked_entry_requirement(
                    program,
                    callee,
                    goal,
                    &facts.termination.rank_arrivals,
                )
            {
                return Some(true);
            }
            let arguments = validation::call_site_argument_expressions(program, &site);
            let explicit_parameters = parameters.iter();
            if explicit_parameters.clone().count() != arguments.len() {
                return None;
            }
            let arithmetic_hypotheses = roster_premise_facts(&facts.semantic, contexts)
                .filter_map(|fact| match fact.payload {
                    facts::FactPayload::ContractBooleanExpression {
                        kind: facts::ContractFactKind::Requires,
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => Some((expression, true)),
                    facts::FactPayload::BooleanValue { expression, value } => {
                        Some((expression, value))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if validation::prove_arithmetic_call_requirement(
                program,
                caller_machine,
                caller_state,
                callee,
                entry,
                &arithmetic_hypotheses,
                goal,
                arguments,
                &facts.termination.rank_arrivals,
            ) {
                return Some(true);
            }
            if super::super::call_values::proves_captured_arithmetic_requirement(
                program, facts, caller, call, goal,
            ) {
                return Some(true);
            }
            let caller_parameters = program.state_parameters(caller_state);
            let mut owned_frames = None;
            let frames = caller_parameters
                .iter()
                .any(|parameter| parameter.is_mutable)
                .then(|| shared_call_frames_or(call_frames, program, &mut owned_frames))
                .flatten();
            let bindings = caller_parameters
                .iter()
                .filter_map(|parameter| {
                    let primitive = fixed_parameter_type(program, parameter)?;
                    Some(binding(parameter.symbol, parameter.symbol, primitive))
                })
                .collect::<Vec<_>>();
            if !comparison_is_admitted(program, facts, callee.symbol, parameters, goal) {
                return None;
            }
            let mut goal_occurrences = Vec::new();
            append_expression_occurrences(program, goal, &mut goal_occurrences);
            let mut argument_bindings = Vec::new();
            for (position, (parameter, argument)) in explicit_parameters.zip(arguments).enumerate()
            {
                if !goal_occurrences.iter().any(|occurrence| {
                    direct_parameter(program, parameters, *occurrence)
                        .is_some_and(|demanded| demanded.symbol == parameter.symbol)
                }) {
                    continue;
                }
                let Some(primitive) = program.primitive_type_reference(parameter.type_reference)
                else {
                    continue;
                };
                if !primitive.is_fixed_integer() {
                    continue;
                }
                if fixed_parameter_type(program, parameter) != Some(primitive) {
                    return None;
                }
                if arguments::primitive_type(
                    program,
                    facts,
                    caller_machine.symbol,
                    caller_parameters,
                    *argument,
                ) != Some(primitive)
                    && !(arguments::is_unlanded_integer(program, *argument)
                        && validation::land_anonymous_integer_expression(
                            program,
                            *argument,
                            primitive,
                            &[],
                            |_| false,
                        )
                        .is_some())
                {
                    return None;
                }
                if !arguments::capture_is_current(
                    program,
                    facts,
                    caller_machine,
                    caller_state,
                    call.statement_index,
                    *argument,
                    &arguments[position + 1..],
                    frames,
                ) {
                    return None;
                }
                argument_bindings.push(validation::StrictArithmeticExpressionBinding {
                    symbol: parameter.symbol,
                    expression: *argument,
                });
            }
            let hypotheses = roster_premise_facts(&facts.semantic, contexts)
                .filter_map(|fact| match fact.payload {
                    facts::FactPayload::ContractBooleanExpression {
                        kind: facts::ContractFactKind::Requires,
                        expression,
                        instantiated,
                        ..
                    } if !instantiated.is_valid() => Some(expression),
                    facts::FactPayload::BooleanValue {
                        expression,
                        value: true,
                    } => Some(expression),
                    _ => None,
                })
                .flat_map(|expression| {
                    true_conjuncts(program, facts, caller.machine_symbol, expression)
                })
                .filter(|expression| {
                    comparison_is_admitted(
                        program,
                        facts,
                        caller.machine_symbol,
                        caller_parameters,
                        *expression,
                    )
                })
                .collect::<Vec<_>>();
            Some(
                validation::strict_arithmetic_expression_implication(
                    program,
                    caller_machine,
                    &hypotheses,
                    goal,
                    &bindings,
                    &argument_bindings,
                ) == StrictArithmeticImplicationJudgment::Proven,
            )
        }

        /*
            Every conjunct of a true builtin `&&` is itself a true fact. A guard
            such as `work > 1 && members[row].vector == at` keeps its arithmetic
            conjunct even though the other lies outside this vocabulary.
        */
        fn true_conjuncts(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            owner: SymbolHandle,
            expression: ExpressionHandle,
        ) -> Vec<ExpressionHandle> {
            let mut pending = vec![expression];
            let mut conjuncts = Vec::new();
            while let Some(expression) = pending.pop() {
                let Some(expression) =
                    positive_comparison_expression(program, facts, owner, expression)
                else {
                    continue;
                };
                if let Some(binary) = program.expression_table.binary_node(expression)
                    && binary.operator == BinaryOperator::And
                    && !facts.operators.uses.iter().any(|(_, operator)| {
                        operator.expression == expression
                            && operator.status
                                != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                    })
                {
                    pending.extend([binary.right, binary.left]);
                    continue;
                }
                conjuncts.push(expression);
            }
            conjuncts
        }

        /*
            Boolean-pattern transition guards retain `comparison == true`. Peel only
            positively asserted builtin wrappers; a false arm supplies no positive fact.
        */
        fn positive_comparison_expression(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            owner: SymbolHandle,
            mut expression: ExpressionHandle,
        ) -> Option<ExpressionHandle> {
            for _ in 0..64 {
                let Some(binary) = program.expression_table.binary_node(expression) else {
                    return Some(expression);
                };
                let spelling = match binary.operator {
                    BinaryOperator::Equal => OperatorSpelling::Equal,
                    BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
                    _ => return Some(expression),
                };
                let wrapper = [(binary.left, binary.right), (binary.right, binary.left)]
                    .into_iter()
                    .find_map(|(operand, literal)| {
                        match program.expression_table.expression(literal) {
                            ExpressionNode::Boolean(value) => Some((operand, *value)),
                            _ => None,
                        }
                    });
                let Some((operand, value)) = wrapper else {
                    return Some(expression);
                };
                if value != (binary.operator == BinaryOperator::Equal)
                    || !super::super::prover::has_builtin_operators(
                        program,
                        &facts.operators,
                        expression,
                    )
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        owner,
                        expression,
                        spelling,
                        &[None, None],
                    )
                {
                    return None;
                }
                expression = operand;
            }
            None
        }

        fn is_data_namespace(
            program: &dyn ::validation::CheckingInput,
            symbol: SymbolHandle,
        ) -> bool {
            symbol.is_valid() && program.data_definition_by_symbol(symbol).is_some()
        }

        fn binding(
            formal: SymbolHandle,
            actual: SymbolHandle,
            primitive: PrimitiveType,
        ) -> StrictArithmeticSymbolBinding {
            StrictArithmeticSymbolBinding {
                symbol: formal,
                value: StrictArithmeticBindingValue::Atom {
                    identity: format!("\0call-parameter:{actual:?}"),
                    unsigned: matches!(
                        primitive,
                        typed_trees::unsigned_integer_primitive_pattern!()
                    ),
                },
            }
        }

        fn fixed_parameter_type(
            program: &dyn ::validation::CheckingInput,
            parameter: &StateParameter,
        ) -> Option<PrimitiveType> {
            if !parameter.symbol.is_valid() || parameter.is_const {
                return None;
            }
            program
                .primitive_type_reference(parameter.type_reference)
                .filter(|primitive| primitive.is_fixed_integer())
        }

        fn direct_parameter<'a>(
            program: &dyn ::validation::CheckingInput,
            parameters: &'a [StateParameter],
            expression: ExpressionHandle,
        ) -> Option<&'a StateParameter> {
            let symbol = ::validation::single_segment_name_symbol(program, expression)?;
            parameters
                .iter()
                .find(|parameter| parameter.symbol == symbol)
        }

        fn comparison_is_admitted(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            owner: SymbolHandle,
            parameters: &[StateParameter],
            expression: ExpressionHandle,
        ) -> bool {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return false;
            };
            if binary.operator == BinaryOperator::And {
                return super::super::prover::has_builtin_operators(
                    program,
                    &facts.operators,
                    expression,
                ) && comparison_is_admitted(program, facts, owner, parameters, binary.left)
                    && comparison_is_admitted(program, facts, owner, parameters, binary.right);
            }
            let Some(spelling) =
                typed_trees::operator::comparison_operator_spelling(binary.operator)
            else {
                return false;
            };
            let operand =
                |expression| -> Option<(Option<PrimitiveType>, Option<TypeReferenceHandle>)> {
                    if arguments::is_unlanded_integer(program, expression) {
                        return Some((None, None));
                    }
                    if let Some(literal) = program.expression_table.integer_node(expression) {
                        let primitive = scalar_expression_type(
                            &checked_trees::CheckedScalarExpression::IntegerLiteral {
                                literal: literal.clone(),
                            },
                        )?;
                        return primitive
                            .is_fixed_integer()
                            .then_some((Some(primitive), None));
                    }
                    let primitive =
                        arguments::primitive_type(program, facts, owner, parameters, expression)?;
                    let type_reference = direct_parameter(program, parameters, expression)
                        .map(|parameter| parameter.type_reference);
                    Some((Some(primitive), type_reference))
                };
            let Some((left, left_reference)) = operand(binary.left) else {
                return false;
            };
            let Some((right, right_reference)) = operand(binary.right) else {
                return false;
            };
            (left == right || left.is_none() || right.is_none())
                && super::super::prover::has_builtin_operators(
                    program,
                    &facts.operators,
                    expression,
                )
                && typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    owner,
                    expression,
                    spelling,
                    &[left_reference, right_reference],
                )
        }
    }
    mod guard_implication {
        /*
            A strict selected-arm comparison implies its non-strict counterpart only
            over the same substituted values. This is an identity judgment, not interval
            evaluation; the caller separately checks intervening operand writes.
        */
        use super::super::prover::has_builtin_operators;
        use checked_trees::CheckFacts;
        use language_core::OperatorSpelling;
        use typed_trees::{
            expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
            machine::Machine,
            signature::StateParameter,
            state::State,
            types::{PrimitiveType, TypeReferenceNode},
        };

        #[derive(PartialEq, Eq)]
        struct Operand {
            place: validation::CanonicalPlace,
            meaning: Meaning,
        }

        #[derive(PartialEq, Eq)]
        enum Meaning {
            CollectionLength,
            Integer(PrimitiveType),
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn proves(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            caller: &Machine,
            caller_state: &State,
            callee: &Machine,
            callee_state: &State,
            parameters: &[StateParameter],
            arguments: &[ExpressionHandle],
            goal: ExpressionHandle,
            guard: ExpressionHandle,
        ) -> bool {
            let Some(required) = program.expression_table.binary_node(goal) else {
                return false;
            };
            let (left, right, spelling) = match required.operator {
                BinaryOperator::LessOrEqual => {
                    (required.left, required.right, OperatorSpelling::LessEqual)
                }
                BinaryOperator::GreaterOrEqual => (
                    required.right,
                    required.left,
                    OperatorSpelling::GreaterEqual,
                ),
                _ => return false,
            };
            if !builtin(program, facts, callee, callee_state, goal, spelling) {
                return false;
            }
            let goal_operand = |expression| {
                let (expression, meaning) = observation(program, callee, callee_state, expression)?;
                let (actual, suffix) =
                    crate::published::semantic::places::call_contract_argument_projection(
                        program, parameters, arguments, expression,
                    )?;
                if !suffix.is_empty() || !has_builtin_operators(program, &facts.operators, actual) {
                    return None;
                }
                let actual_meaning = match meaning {
                    Meaning::CollectionLength => {
                        let reference = validation::declared_place_type_raw(
                            program,
                            caller,
                            Some(caller_state),
                            actual,
                        )?;
                        let reference = validation::unwrapped_type_reference(program, reference)?;
                        if !matches!(
                            program.type_reference_table.type_reference(reference),
                            TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. }
                        ) {
                            return None;
                        }
                        Meaning::CollectionLength
                    }
                    Meaning::Integer(_) => observation(program, caller, caller_state, actual)?.1,
                };
                if actual_meaning != meaning {
                    return None;
                }
                let mut place = validation::canonical_place_from_expression(program, actual)?;
                place.segments.extend(suffix);
                exact_place(place).map(|place| Operand { place, meaning })
            };
            let (Some(left), Some(right)) = (goal_operand(left), goal_operand(right)) else {
                return false;
            };
            let guard_operand = |expression| {
                let (expression, meaning) = observation(program, caller, caller_state, expression)?;
                let place = validation::canonical_place_from_expression(program, expression)?;
                exact_place(place).map(|place| Operand { place, meaning })
            };
            let mut pending = vec![guard];
            while let Some(expression) = pending.pop() {
                let Some(binary) = program.expression_table.binary_node(expression) else {
                    continue;
                };
                match binary.operator {
                    BinaryOperator::And
                        if has_builtin_operators(program, &facts.operators, expression) =>
                    {
                        pending.extend([binary.right, binary.left]);
                    }
                    BinaryOperator::Equal
                        if builtin(
                            program,
                            facts,
                            caller,
                            caller_state,
                            expression,
                            OperatorSpelling::Equal,
                        ) =>
                    {
                        if matches!(
                            program.expression_table.expression(binary.right),
                            ExpressionNode::Boolean(true)
                        ) {
                            pending.push(binary.left);
                        } else if matches!(
                            program.expression_table.expression(binary.left),
                            ExpressionNode::Boolean(true)
                        ) {
                            pending.push(binary.right);
                        }
                    }
                    BinaryOperator::Less | BinaryOperator::Greater => {
                        let (first, second, spelling) = if binary.operator == BinaryOperator::Less {
                            (binary.left, binary.right, OperatorSpelling::Less)
                        } else {
                            (binary.right, binary.left, OperatorSpelling::Greater)
                        };
                        if builtin(program, facts, caller, caller_state, expression, spelling)
                            && guard_operand(first).as_ref() == Some(&left)
                            && guard_operand(second).as_ref() == Some(&right)
                        {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }

        fn builtin(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
            spelling: OperatorSpelling,
        ) -> bool {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return false;
            };
            let operands = [binary.left, binary.right].map(|operand| {
                validation::declared_place_type_raw(program, machine, Some(state), operand)
            });
            has_builtin_operators(program, &facts.operators, expression)
                && typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    machine.symbol,
                    expression,
                    spelling,
                    &operands,
                )
        }

        fn observation(
            program: &dyn ::validation::CheckingInput,
            machine: &Machine,
            state: &State,
            expression: ExpressionHandle,
        ) -> Option<(ExpressionHandle, Meaning)> {
            if let Some(receiver) =
                validation::collection_length_receiver(program, machine, Some(state), expression)
            {
                return Some((receiver, Meaning::CollectionLength));
            }
            let reference =
                validation::declared_place_type_raw(program, machine, Some(state), expression)?;
            if !matches!(
                program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { .. }
            ) {
                return None;
            }
            let primitive = program.primitive_type_reference(reference)?;
            primitive
                .is_fixed_integer()
                .then_some((expression, Meaning::Integer(primitive)))
        }

        fn exact_place(place: validation::CanonicalPlace) -> Option<validation::CanonicalPlace> {
            (matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
                && place.segments.iter().all(
                    |segment| matches!(segment, facts::PlaceSegment::Field { symbol } if symbol.is_valid()),
                ))
            .then_some(place)
        }
    }
    pub(super) use context::proves as proves_in_context;

    pub(super) fn proves(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        expression: ExpressionHandle,
        guard: Option<ExpressionHandle>,
    ) -> bool {
        if guard.is_some_and(|guard| {
            !super::prover::has_builtin_operators(program, &facts.operators, guard)
        }) {
            return false;
        }
        prove(program, facts, caller, call, contexts, expression, guard).unwrap_or(false)
    }

    fn prove(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        caller: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        expression: ExpressionHandle,
        guard: Option<ExpressionHandle>,
    ) -> Option<bool> {
        let binary = program.expression_table.binary_node(expression)?;
        if facts.operators.uses.iter().any(|(_, operator)| {
            operator.expression == expression
                && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
        }) {
            return None;
        }
        if binary.operator == BinaryOperator::And {
            return Some(
                proves(program, facts, caller, call, contexts, binary.left, guard)
                    && proves(program, facts, caller, call, contexts, binary.right, guard),
            );
        }
        if binary.operator == BinaryOperator::Or {
            return Some(
                proves(program, facts, caller, call, contexts, binary.left, guard)
                    || proves(program, facts, caller, call, contexts, binary.right, guard),
            );
        }
        let spelling = validation::comparison_operator_spelling(binary.operator)?;
        let site = validation::find_call_site(
            program,
            caller.machine_symbol,
            caller.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let (callee, callee_state) = match &site {
            validation::CallSite::Expression {
                call: source_call, ..
            } => {
                if validation::call_target_entry_symbol(
                    program,
                    validation::call_target_symbol(program, source_call),
                ) != call.target_symbol
                    || (source_call.receiver.is_valid()
                        && validation::canonical_place_from_expression(
                            program,
                            source_call.receiver,
                        )
                        .is_none())
                    || source_call.static_requirement_dispatch.is_some()
                {
                    return None;
                }
                validation::find_state(program, call.target_symbol).filter(|(machine, state)| {
                    program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == state.symbol)
                })?
            }
            validation::CallSite::TransitionNamed {
                path,
                evidence_arguments,
                ..
            } => {
                let target = validation::call_target_state(program, path.symbol)
                    .map_or(path.symbol, |state| state.symbol);
                if target != call.target_symbol || !evidence_arguments.is_empty() {
                    return None;
                }
                validation::find_state(program, call.target_symbol)
                    .filter(|(machine, _)| machine.symbol == caller.machine_symbol)?
            }
            validation::CallSite::Statement(_) => return None,
        };
        let transition = matches!(site, validation::CallSite::TransitionNamed { .. });
        let parameters = program.state_parameters(callee_state);
        let explicit = parameters.iter().collect::<Vec<_>>();
        let arguments = validation::call_site_argument_expressions(program, &site);
        if arguments.len() != explicit.len() {
            return None;
        }
        let machine = program.machine_by_symbol(caller.machine_symbol)?;
        let state =
            validation::find_state_in_machine(program, caller.machine_symbol, caller.state_symbol)?;
        if validation::instantiated_value_call_requires_proven(
            program,
            machine,
            state,
            call.statement_index,
            site,
            callee_state.symbol,
            parameters,
            arguments,
            expression,
        ) {
            return Some(true);
        }
        if guard.is_some_and(|guard| {
            guard_implication::proves(
                program,
                facts,
                machine,
                state,
                callee,
                callee_state,
                parameters,
                arguments,
                expression,
                guard,
            )
        }) {
            return Some(true);
        }
        if carrier_extent_discharge(program, machine, state, &explicit, arguments, binary) {
            return Some(true);
        }
        let operand = |expression| match program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) => literal
                .value_i64()
                .map(|value| ((Some(value), Some(value)), None)),
            ExpressionNode::Name(path)
                if path.symbol.is_valid() && path.head_symbol == path.symbol =>
            {
                let position = explicit
                    .iter()
                    .position(|parameter| parameter.symbol == path.symbol)?;
                let parameter = explicit[position];
                if parameter.is_mutable || parameter.is_const {
                    return None;
                }
                let argument = arguments[position];
                if !super::prover::has_builtin_operators(program, &facts.operators, argument) {
                    return None;
                }
                let standing =
                    immutable_capture_bounds(program, &facts.semantic, contexts, state, argument)
                        .or_else(|| {
                            validation::immutable_integer_expression_bounds(
                                program, machine, state, argument,
                            )
                        })
                        .map(|(low, high)| (Some(low), Some(high)))
                        .or_else(|| {
                            validation::stored_integer_bounds(program, machine, state, argument)
                        });
                let guarded = guard.and_then(|guard| {
                    guarded_update_bounds(program, machine, state, argument, guard)
                });
                let (mut low, mut high) = match (standing, guarded) {
                    (Some((low, high)), Some((guard_low, guard_high))) => (
                        low.into_iter().chain(guard_low).max(),
                        high.into_iter().chain(guard_high).min(),
                    ),
                    (bounds, None) | (None, bounds) => bounds?,
                };
                if let Some(guard_interval) = guard.and_then(|guard| {
                    super::intervals::guard_interval_for_label(
                        program,
                        guard,
                        &program.expression_table.display_name(argument),
                    )
                }) {
                    (low, high) = super::intervals::narrow_i64_bounds(low, high, &guard_interval);
                }
                Some(((low, high), Some(parameter.type_reference)))
            }
            ExpressionNode::Member(_)
                if transition && receiver_rooted(program, callee, expression) =>
            {
                let bounds =
                    validation::stored_integer_bounds(program, callee, callee_state, expression)?;
                let place_type = validation::declared_place_type_raw(
                    program,
                    callee,
                    Some(callee_state),
                    expression,
                );
                Some((bounds, place_type))
            }
            ExpressionNode::Member(_) => loan_field_bounds(
                program,
                facts,
                state,
                call,
                contexts,
                &explicit,
                arguments,
                callee,
                callee_state,
                expression,
            ),
            _ => None,
        };
        let ((left_low, left_high), left_type) = operand(binary.left)?;
        let ((right_low, right_high), right_type) = operand(binary.right)?;
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            callee.symbol,
            expression,
            spelling,
            &[left_type, right_type],
        ) {
            return None;
        }
        Some(match binary.operator {
            BinaryOperator::Equal => {
                left_low? == left_high? && right_low? == right_high? && left_low == right_low
            }
            BinaryOperator::NotEqual => left_high? < right_low? || right_high? < left_low?,
            BinaryOperator::Less => left_high? < right_low?,
            BinaryOperator::LessOrEqual => left_high? <= right_low?,
            BinaryOperator::Greater => left_low? > right_high?,
            BinaryOperator::GreaterOrEqual => left_low? >= right_high?,
            _ => return None,
        })
    }

    fn loan_field_bounds(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        call: &FlowCallFact,
        contexts: &[facts::FactContextHandle],
        explicit: &[&typed_trees::signature::StateParameter],
        arguments: &[typed_trees::expression::ExpressionHandle],
        callee: &typed_trees::machine::Machine,
        callee_state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<(
        (Option<i64>, Option<i64>),
        Option<typed_trees::types::TypeReferenceHandle>,
    )> {
        let required = crate::flow::projected_leaf_place(program, expression)?;
        let formal = required.root.as_symbol()?;
        let position = explicit
            .iter()
            .position(|parameter| parameter.symbol == formal)?;
        let argument = *arguments.get(position)?;
        let places = crate::flow::reference_expression_storage_places_including_shared(
            program,
            state.symbol,
            call.statement_index,
            argument,
            None,
        )?;
        let [storage] = places.as_slice() else {
            return None;
        };
        let mut actual = storage.clone();
        actual.extend_segments(&required.segments);
        let bounds = crate::values::integer_bounds_at_place(
            program,
            &facts.semantic,
            contexts
                .iter()
                .map(|context| facts.semantic.contexts.get(*context)),
            &actual,
        )?;
        let place_type =
            validation::declared_place_type_raw(program, callee, Some(callee_state), expression);
        Some((
            (bounds.minimum.to_i64(), bounds.maximum.to_i64()),
            place_type,
        ))
    }

    /*
        Narrow storage reads before transferring their bounds through arithmetic.
        The arithmetic owner decides the actual Exact, wrapping or saturating law;
        this reader does not reinterpret an update as mathematical addition. The
        selected-arm caller separately checks guard and operand write ordering.
    */
    fn guarded_update_bounds(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
        guard: ExpressionHandle,
    ) -> Option<(Option<i64>, Option<i64>)> {
        match program.expression_table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                let operand = match binary.operator {
                    BinaryOperator::Add
                        if matches!(
                            program.expression_table.expression(binary.left),
                            ExpressionNode::Integer(_)
                        ) =>
                    {
                        binary.right
                    }
                    BinaryOperator::Add | BinaryOperator::Subtract => binary.left,
                    _ => return None,
                };
                let (lower, upper) =
                    guarded_update_bounds(program, machine, state, operand, guard)?;
                validation::builtin_monotonic_integer_update_bounds(
                    program, machine, state, expression, lower, upper,
                )
            }
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                let (mut lower, mut upper) =
                    validation::stored_integer_bounds(program, machine, state, expression)?;
                if let Some(guard_interval) = super::intervals::guard_interval_for_label(
                    program,
                    guard,
                    &program.expression_table.display_name(expression),
                ) {
                    (lower, upper) =
                        super::intervals::narrow_i64_bounds(lower, upper, &guard_interval);
                }
                Some((lower, upper))
            }
            _ => None,
        }
    }

    /*
        Read the value captured when an immutable scalar local was established.
        Looking through its initializer here would incorrectly use the *current*
        values of operands that may since have changed. Mutable locals need an
        argument-evaluation snapshot, not this call-entry storage query.
    */
    fn immutable_capture_bounds(
        program: &dyn ::validation::CheckingInput,
        semantic: &facts::FactPlan,
        contexts: &[facts::FactContextHandle],
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> Option<(i64, i64)> {
        let path = program.expression_table.name_path_node(expression)?;
        if !program.expression_table.name_path_is_single_member(path) {
            return None;
        }
        let (_, local) = program.state_local_declaration(state, path.symbol)?;
        if local.is_mutable
            || program
                .primitive_type_reference(local.type_reference)
                .is_none()
        {
            return None;
        }
        let place = crate::flow::canonical_place_from_symbol(path.symbol)?;
        let bounds = crate::values::integer_bounds_at_place(
            program,
            semantic,
            contexts
                .iter()
                .map(|context| semantic.contexts.get(*context)),
            &place,
        )?;
        Some((bounds.minimum.to_i64()?, bounds.maximum.to_i64()?))
    }

    /*
        Whether one conjunct is vacuous under the substituted operand's carrier:
        the comparison's subject resolves to a call argument whose type's full
        extent already lies inside the bound the literal states. Only the same
        literal-vs-parameter surface the operand read below resolves qualifies --
        two literals belong to the closed evaluator and two subjects have no
        carrier extent to quote.
    */
    fn carrier_extent_discharge(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        explicit: &[&typed_trees::signature::StateParameter],
        arguments: &[ExpressionHandle],
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> bool {
        let left_literal = validation::closed_integer_range_bound(program, binary.left);
        let right_literal = validation::closed_integer_range_bound(program, binary.right);
        let (subject, subject_on_left, bound) = match (left_literal, right_literal) {
            (Some(value), None) => (binary.right, false, value),
            (None, Some(value)) => (binary.left, true, value),
            _ => return false,
        };
        let Some(path) = program.expression_table.name_path_node(subject) else {
            return false;
        };
        if !(path.symbol.is_valid() && path.head_symbol == path.symbol) {
            return false;
        }
        let Some(argument) = explicit
            .iter()
            .position(|parameter| parameter.symbol == path.symbol)
            .and_then(|position| arguments.get(position))
        else {
            return false;
        };
        let maximum = crate::checks::ranges::types::expression_integer_carrier_maximum(
            program, machine, state, *argument,
        )
        .map(numerics::bignum::BigInt::from_u64);
        let minimum = crate::checks::ranges::types::expression_integer_carrier_minimum(
            program, machine, state, *argument,
        );
        match (binary.operator, subject_on_left) {
            (BinaryOperator::Less, true) | (BinaryOperator::Greater, false) => {
                maximum.is_some_and(|maximum| bound > maximum)
            }
            (BinaryOperator::LessOrEqual, true) | (BinaryOperator::GreaterOrEqual, false) => {
                maximum.is_some_and(|maximum| bound >= maximum)
            }
            (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
                minimum.is_some_and(|minimum| bound < minimum)
            }
            (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::LessOrEqual, false) => {
                minimum.is_some_and(|minimum| bound <= minimum)
            }
            _ => false,
        }
    }

    /* Whether a member chain reads a field of the machine's own receiver. */
    fn receiver_rooted(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        mut expression: ExpressionHandle,
    ) -> bool {
        loop {
            match program.expression_table.expression(expression) {
                ExpressionNode::Member(member) if member.case_variant.is_none() => {
                    expression = member.receiver;
                }
                ExpressionNode::Name(path) => {
                    return path.symbol == machine.symbol && path.head_symbol == machine.symbol;
                }
                _ => return false,
            }
        }
    }
}
mod captured_predicates {
    /*
        Preserve predicates of completed copies across writes and selections.
        Bindings keep each possible value and all of its aliases together. Producer
        contexts never mix: selection coverage decides which transported conclusions
        hold for the whole observed value.
    */
    use crate::published::facts::boolean_predicates::{BoundObservation, BoundPredicate};
    use checked_trees::{CheckFacts, FlowOperandFact, FlowStateFact};
    use validation::CanonicalPlace;

    mod bindings {
        /*
            One possible completed value and its simultaneous authenticated aliases.
            Alternatives remain separate obligations, including candidates with the same
            selection condition. Geometry is useful only with matching type and epoch.
        */
        use super::{BoundObservation, CanonicalPlace, CheckFacts, FlowOperandFact};
        use crate::flow::{CapturedValue, CapturedValueChoice};
        use facts::BooleanPredicateOperand;
        use typed_trees::types::TypeReferenceHandle;
        use validation::CheckingInput;
        use validation::canonical_place_type_reference;
        use validation::owned_projection_type_reference;
        use validation::project_type_reference_from_segments;
        use validation::reference_shell;

        pub(in crate::checks::contracts) struct Alias<'facts> {
            pub place: CanonicalPlace,
            pub epoch: &'facts FlowOperandFact,
        }

        pub(in crate::checks::contracts) struct CapturedBinding<'facts> {
            pub position: usize,
            pub choices: Vec<CapturedValueChoice>,
            pub aliases: Vec<Alias<'facts>>,
        }

        impl<'facts> CapturedBinding<'facts> {
            pub fn unresolved(position: usize, choices: Vec<CapturedValueChoice>) -> Self {
                Self {
                    position,
                    choices,
                    aliases: Vec::new(),
                }
            }

            pub fn observation(
                position: usize,
                place: CanonicalPlace,
                epoch: &'facts FlowOperandFact,
            ) -> Self {
                Self {
                    position,
                    choices: Vec::new(),
                    aliases: vec![Alias { place, epoch }],
                }
            }
        }

        pub(in crate::checks::contracts) fn retain_bindings<'facts>(
            program: &dyn CheckingInput,
            facts: &'facts CheckFacts,
            captured: &CapturedValue<'facts>,
            epoch: &'facts FlowOperandFact,
            bindings: &[BoundObservation],
            first_position: usize,
            retained: &mut Vec<CapturedBinding<'facts>>,
        ) {
            for (offset, binding) in bindings.iter().enumerate() {
                let producer = producer_alias(program, facts, captured, epoch, binding);
                let mut candidate = CapturedBinding::observation(
                    first_position + offset,
                    binding.place.clone(),
                    epoch,
                );
                candidate.choices = captured.choices.clone();
                if let Some((observed, alias, source_type)) = &producer
                    && binding.place == *observed
                    && binding.compatible_type(program, *source_type)
                {
                    candidate.aliases.push(Alias {
                        place: alias.place.clone(),
                        epoch: alias.epoch,
                    });
                }
                retained.push(candidate);
            }
        }

        fn producer_alias<'facts>(
            program: &dyn CheckingInput,
            facts: &'facts CheckFacts,
            captured: &CapturedValue<'facts>,
            epoch: &'facts FlowOperandFact,
            binding: &BoundObservation,
        ) -> Option<(CanonicalPlace, Alias<'facts>, TypeReferenceHandle)> {
            let stable_copy =
                |reference, projection: &crate::flow::LiteralExpressionProjection| match &binding
                    .operand
                {
                    BooleanPredicateOperand::Scalar(_) => project_type_reference_from_segments(
                        program,
                        reference,
                        &projection.remaining,
                    )
                    .is_some_and(|selected| {
                        crate::flow::operand_binds_stable_copy(program, selected)
                    }),
                    BooleanPredicateOperand::CaseSubject(_) => {
                        owned_projection_type_reference(program, reference, &projection.remaining)
                            .is_some_and(|selected| {
                                reference_shell(program, selected).is_none()
                                    && program.closed_data_definition(selected).is_some()
                            })
                    }
                };
            if !stable_copy(captured.observation_type, &captured.observation_projection)
                || (captured.producer_epoch.is_none()
                    && !stable_copy(captured.type_reference, &captured.projection))
            {
                return None;
            }
            let observed =
                crate::flow::captured_observation_places(program, facts.flow.view(), captured)?;
            let produced =
                crate::flow::captured_value_places(program, facts.flow.view(), captured)?;
            let ([observed], [produced]) = (observed.as_slice(), produced.as_slice()) else {
                return None;
            };
            let producer_epoch = captured.producer_epoch.unwrap_or(captured.operand);
            if crate::flow::operand_binds_live_view(program, captured.type_reference)
                && captured.producer_epoch.is_none()
                && !crate::flow::operand_observations_agree(
                    program,
                    facts.flow.view(),
                    captured.operand.row(),
                    captured.producer_epoch.map_or(epoch, |read| read.row()),
                    &produced.projected,
                )
            {
                return None;
            }
            let prefixes = super::super::capture_coordinates::borrowed_bindings(
                program,
                captured.operand.row(),
                &captured.projection.remaining,
            )?;
            if prefixes.iter().any(|prefix| {
                if let Some(read) = captured.producer_epoch {
                    !crate::flow::operand_bindings_agree(
                        program,
                        facts.flow.view(),
                        captured.operand.row(),
                        read.row(),
                        prefix,
                    )
                } else {
                    !crate::flow::operand_entry_binding_origin_is_stable(
                        program,
                        &facts.flow,
                        captured.operand.row(),
                        prefix,
                    )
                }
            }) {
                return None;
            }
            let source_type = canonical_place_type_reference(
                program,
                captured.operand.state_symbol,
                captured.operand.statement_index,
                &produced.projected,
            )?;
            Some((
                observed.projected.clone(),
                Alias {
                    place: produced.projected.clone(),
                    epoch: producer_epoch.row(),
                },
                source_type,
            ))
        }

        /*
            One read of an alternative of a completed selection: the read epoch that
            detached its contents and the storage place it observed.
        */
        pub(in crate::checks::contracts) struct AlternativeRead<'facts> {
            choice: CapturedValueChoice,
            epoch: &'facts FlowOperandFact,
            place: CanonicalPlace,
        }

        /*
            Record every single-choice read among these completed values. Values whose
            contents were not detached by a read, or that span several selections, add
            nothing.
        */
        pub(in crate::checks::contracts) fn collect_alternative_reads<'facts>(
            program: &dyn CheckingInput,
            facts: &'facts CheckFacts,
            values: &[CapturedValue<'facts>],
            reads: &mut Vec<AlternativeRead<'facts>>,
        ) {
            for value in values {
                let (Some(epoch), [choice]) = (value.producer_epoch, value.choices.as_slice())
                else {
                    continue;
                };
                let Some(places) =
                    crate::flow::captured_value_places(program, facts.flow.view(), value)
                else {
                    continue;
                };
                for place in places {
                    reads.push(AlternativeRead {
                        choice: *choice,
                        epoch: epoch.row(),
                        place: place.projected,
                    });
                }
            }
        }

        /*
            A binding on current storage equals, on each predecessor, the one read that
            predecessor reached when every path reaches a read of an alternative of one
            selection and preserves its contents (`fact_validity.md`: each predecessor
            supplies its own reaching value). The binding then has one capture per
            alternative under that alternative's condition; coverage decides the rest.
            Several reads under one alternative, or reads from different selections,
            leave the binding unconditional.
        */
        pub(in crate::checks::contracts) fn retain_alternative_reads<'facts>(
            program: &dyn CheckingInput,
            facts: &'facts CheckFacts,
            storage: arena::Handle<checked_trees::FlowStorageSnapshot>,
            bindings: &[BoundObservation],
            reads: &[AlternativeRead<'facts>],
            retained: &mut Vec<CapturedBinding<'facts>>,
        ) {
            for (position, binding) in bindings.iter().enumerate() {
                let mut own = retained
                    .iter()
                    .filter(|capture| capture.position == position);
                if own.clone().next().is_none() || own.any(|capture| !capture.choices.is_empty()) {
                    continue;
                }
                let candidates = reads
                    .iter()
                    .filter(|read| read.place == binding.place)
                    .collect::<Vec<_>>();
                let Some(first) = candidates.first() else {
                    continue;
                };
                if candidates
                    .iter()
                    .any(|read| read.choice.selection != first.choice.selection)
                    || candidates.iter().enumerate().any(|(index, read)| {
                        candidates[..index]
                            .iter()
                            .any(|earlier| earlier.choice.alternative == read.choice.alternative)
                    })
                {
                    continue;
                }
                let cuts = candidates
                    .iter()
                    .map(|read| read.epoch.storage_snapshot)
                    .collect::<Vec<_>>();
                if !validation::completed_storage::contents_preserved_from(
                    program,
                    &facts.flow.control,
                    storage,
                    &binding.place,
                    &cuts,
                    validation::completed_storage::StoredObservation::Contents,
                ) {
                    continue;
                }
                retained.retain(|capture| capture.position != position);
                retained.extend(candidates.into_iter().map(|read| CapturedBinding {
                    position,
                    choices: vec![read.choice],
                    aliases: vec![Alias {
                        place: binding.place.clone(),
                        epoch: read.epoch,
                    }],
                }));
            }
        }
    }
    mod case_evidence {
        /*
            Typed case truths retain all completed-value alternatives.
        */
        use super::transport::ConditionalPredicate;
        use super::{BoundPredicate, CapturedBinding, CheckFacts, FlowStateFact};
        use crate::flow::{SelectionCondition, SelectionConditions};

        /*
            A tag truth belongs to one completed observation and its alternatives.
            Unknown or disagreeing suppliers subtract support rather than disappearing.
        */
        pub(super) fn predicates(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            required: &BoundPredicate,
            captures: &[CapturedBinding<'_>],
            selections: &[SelectionCondition],
            conditions: &mut SelectionConditions,
        ) -> Vec<ConditionalPredicate> {
            let mut result = Vec::new();
            for (position, binding) in required.bindings.iter().enumerate() {
                let facts::BooleanPredicateOperand::CaseSubject(subject) = &binding.operand else {
                    continue;
                };
                let Some(data) = program.closed_data_definition(binding.type_reference) else {
                    continue;
                };
                for member in program.data_members(data) {
                    let typed_trees::data::DataMember::Variant(variant) = member else {
                        continue;
                    };
                    let case = variant.member_identity();
                    let mut yes = conditions.never();
                    let mut no = conditions.never();
                    let mut unknown = conditions.never();
                    for (capture, &selection) in captures
                        .iter()
                        .zip(selections)
                        .filter(|(capture, _)| capture.position == position)
                    {
                        let mut truth = None;
                        let mut conflict = false;
                        for alias in &capture.aliases {
                            let contexts = facts
                                .flow
                                .semantic_constraint_contexts(alias.epoch.constraints)
                                .collect::<Vec<_>>();
                            let value = crate::flow::CaseEvidence::prepare(
                                program,
                                &facts.semantic,
                                &contexts,
                                state.machine_symbol,
                                alias.epoch.state_symbol,
                                alias.epoch.statement_index,
                            )
                            .and_then(|evidence| {
                                evidence.case_truth(&alias.place, binding.type_reference, &case)
                            });
                            if let Some(value) = value {
                                conflict |= truth.is_some_and(|known| known != value);
                                truth = Some(value);
                            }
                        }
                        match (conflict, truth) {
                            (false, Some(true)) => yes = conditions.any(yes, selection),
                            (false, Some(false)) => no = conditions.any(no, selection),
                            _ => unknown = conditions.any(unknown, selection),
                        }
                    }
                    for (positive, good, bad) in [(true, yes, no), (false, no, yes)] {
                        let bad = conditions.any(bad, unknown);
                        let not_bad = conditions.negate(&facts.flow.control, bad);
                        let condition = conditions.all(good, not_bad);
                        if !conditions.possible(&facts.flow.control, condition) {
                            continue;
                        }
                        let atom =
                            checked_trees::CheckedBooleanExpression::StructuralCaseMembership {
                                subject: subject.clone(),
                                case: case.clone(),
                            };
                        result.push(ConditionalPredicate {
                            condition,
                            predicate: BoundPredicate {
                                machine: required.machine,
                                entry_state: required.entry_state,
                                expression: if positive {
                                    atom
                                } else {
                                    checked_trees::CheckedBooleanExpression::Not(Box::new(atom))
                                },
                                bindings: vec![binding.clone()],
                            },
                        });
                    }
                }
            }
            result
        }
    }
    mod cleanup {
        /*
            Cleanup contracts consume the same captured predicate judgment.
        */
        use super::{
            BoundObservation, BoundPredicate, CapturedBinding, CheckFacts, FlowStateFact,
            PredicateTransport, retain_bindings, transport,
        };

        /*
            Rejoin a constructed value's scalar observation with its typed completion. The
            active destination token owns availability; its original place/point names
            one actual child capture even after an outer projection kept only a loan.
            Neither source syntax nor an initializer can replace a missing capture.
        */
        pub(crate) fn cleanup_predicates(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            contexts: &[facts::FactContextHandle],
            required: &mut BoundPredicate,
            premises: &[BoundPredicate],
            mut stable: impl FnMut(&BoundObservation) -> bool,
        ) -> PredicateTransport {
            let mut captures = Vec::new();
            for (position, binding) in required.bindings.iter().enumerate() {
                if !stable(binding) {
                    continue;
                }
                let mut origin = None;
                let mut ambiguous = false;
                let mut origin_depth = 0;
                for fact in contexts.iter().flat_map(|context| {
                    facts
                        .semantic
                        .context_view(facts.semantic.contexts.get(*context))
                        .facts()
                }) {
                    let facts::FactPayload::AssignedValueCapture { source, point } = fact.payload
                    else {
                        continue;
                    };
                    let Some(destination) = fact.place.as_place().and_then(|place| {
                        crate::flow::canonical_place_from_semantic_place(
                            program,
                            &facts.semantic,
                            facts.semantic.places.get(place),
                        )
                    }) else {
                        continue;
                    };
                    if destination.root != binding.place.root {
                        continue;
                    }
                    let Some(relative) = binding
                        .place
                        .segments
                        .strip_prefix(destination.segments.as_slice())
                    else {
                        continue;
                    };
                    let depth = destination.segments.len();
                    if origin.is_some() && depth < origin_depth {
                        continue;
                    }
                    if depth > origin_depth {
                        origin = None;
                        ambiguous = false;
                    }
                    origin_depth = depth;
                    let candidate = (source, point, relative.to_vec());
                    if origin
                        .as_ref()
                        .is_some_and(|previous| *previous != candidate)
                    {
                        ambiguous = true;
                    }
                    origin = Some(candidate);
                }
                let Some((
                    source,
                    facts::ProgramPoint::Statement {
                        machine_symbol,
                        state_symbol,
                        statement_index,
                    },
                    relative,
                )) = origin
                else {
                    continue;
                };
                if ambiguous || machine_symbol != state.machine_symbol {
                    continue;
                }
                let source = facts.semantic.places.get(source);
                let Some(parent) = source.root.as_expression() else {
                    continue;
                };
                let path = facts.semantic.place_segments.span_or_empty(source.segments);
                let mut matching =
                    facts
                        .flow
                        .control
                        .operand_children
                        .iter()
                        .filter_map(|(_, child)| {
                            let operand = crate::flow::CapturedOperand::recorded(
                                &facts.flow.control,
                                child.value,
                            )?;
                            (child.parent == parent
                                && operand.state_symbol == state_symbol
                                && operand.statement_index == statement_index
                                && facts
                                    .flow
                                    .control
                                    .operand_child_paths
                                    .span_or_empty(child.path)
                                    == path)
                                .then_some((child, operand))
                        });
                let Some((child, operand)) = matching.next() else {
                    continue;
                };
                let selected = validation::project_type_reference_from_segments(
                    program,
                    child.type_reference,
                    &relative,
                );
                if matching.any(|(other, _)| {
                    other.value != child.value || other.type_reference != child.type_reference
                }) || selected.and_then(|reference| program.primitive_type_reference(reference))
                    != binding.operand.primitive_type()
                {
                    continue;
                }
                let Some(mut values) = crate::flow::captured_operand_values(
                    program,
                    facts.flow.view(),
                    &facts.semantic,
                    state,
                    operand,
                    child.type_reference,
                    &relative,
                ) else {
                    continue;
                };
                for captured in values.drain(..) {
                    let observed = crate::flow::captured_observation_places(
                        program,
                        facts.flow.view(),
                        &captured,
                    );
                    let observed = observed.as_deref().and_then(|places| {
                        if let [place] = places {
                            Some(&place.projected)
                        } else {
                            None
                        }
                    });
                    let coordinates =
                        validation::find_state(program, captured.observation.state_symbol)
                            .is_some_and(|(machine, body)| {
                                validation::value_has_builtin_coordinates(
                                    program,
                                    machine,
                                    body,
                                    captured.observation_projection.expression,
                                )
                            });
                    let Some(observed) = observed.filter(|_| coordinates) else {
                        captures.push(CapturedBinding::unresolved(
                            position,
                            captured.choices.clone(),
                        ));
                        continue;
                    };
                    let mut copied = binding.clone();
                    copied.place = observed.clone();
                    retain_bindings(
                        program,
                        facts,
                        &captured,
                        operand.row(),
                        &[copied],
                        position,
                        &mut captures,
                    );
                }
            }
            if captures.is_empty() {
                return PredicateTransport::default();
            }
            let frames = validation::CallFrameResolver::new(program);
            transport(
                program,
                facts,
                state,
                required,
                &captures,
                premises,
                frames.as_ref(),
            )
        }
    }
    mod coverage {
        /*
            Selection support is a proof obligation, not a list of convenient cases.
            A good candidate cannot hide an incompatible candidate under the same
            choice. Shared condition DAGs retain correlation without actual products.
        */
        use super::super::prover::call_guarantees::proves_captured_predicates;
        use super::bindings::Alias;
        use super::transport::ConditionalPredicate;
        use super::{BoundPredicate, CapturedBinding, CheckFacts, PredicateTransport};
        use crate::flow::operand_observations_agree;
        use crate::flow::{SelectionCondition, SelectionConditions};

        pub(super) fn implies(
            facts: &CheckFacts,
            conditions: &mut SelectionConditions,
            premise: SelectionCondition,
            conclusion: SelectionCondition,
        ) -> bool {
            let missing = conditions.negate(&facts.flow.control, conclusion);
            let counterexample = conditions.all(premise, missing);
            !conditions.possible(&facts.flow.control, counterexample)
        }

        pub(super) fn dependency(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            captures: &[CapturedBinding<'_>],
            selections: &[SelectionCondition],
            conditions: &mut SelectionConditions,
            source: &Alias<'_>,
            position: usize,
        ) -> SelectionCondition {
            let mut good = conditions.never();
            let mut bad = conditions.never();
            for (candidate, &selection) in captures.iter().zip(selections) {
                if candidate.position != position {
                    continue;
                }
                let agrees = candidate
                    .aliases
                    .iter()
                    .any(|alias| aliases_agree(program, facts, source, alias));
                if agrees {
                    good = conditions.any(good, selection);
                } else {
                    bad = conditions.any(bad, selection);
                }
            }
            let not_bad = conditions.negate(&facts.flow.control, bad);
            conditions.all(good, not_bad)
        }

        fn aliases_agree(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            left: &Alias<'_>,
            right: &Alias<'_>,
        ) -> bool {
            left.place == right.place
                && operand_observations_agree(
                    program,
                    facts.flow.view(),
                    left.epoch,
                    right.epoch,
                    &left.place,
                )
        }

        pub(super) fn join_identities(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            required: &mut BoundPredicate,
            captures: &[CapturedBinding<'_>],
            selections: &[SelectionCondition],
            conditions: &mut SelectionConditions,
        ) {
            for right in 0..required.bindings.len() {
                for left in 0..right {
                    let source = required.bindings[right].clone();
                    let destination = required.bindings[left].clone();
                    if source.place == destination.place
                        || !source.compatible_with(program, &destination)
                    {
                        continue;
                    }
                    let mut good = conditions.never();
                    let mut bad = conditions.never();
                    for (first, &first_condition) in captures
                        .iter()
                        .zip(selections)
                        .filter(|(capture, _)| capture.position == left)
                    {
                        for (second, &second_condition) in captures
                            .iter()
                            .zip(selections)
                            .filter(|(capture, _)| capture.position == right)
                        {
                            let selection = conditions.all(first_condition, second_condition);
                            if !conditions.possible(&facts.flow.control, selection) {
                                continue;
                            }
                            let agrees = first.aliases.iter().any(|one| {
                                second
                                    .aliases
                                    .iter()
                                    .any(|two| aliases_agree(program, facts, one, two))
                            });
                            if agrees {
                                good = conditions.any(good, selection);
                            } else {
                                bad = conditions.any(bad, selection);
                            }
                        }
                    }
                    let not_bad = conditions.negate(&facts.flow.control, bad);
                    let identity = conditions.all(good, not_bad);
                    if conditions.covers(&facts.flow.control, identity) {
                        for binding in &mut required.bindings {
                            if binding.place == source.place
                                && binding.compatible_with(program, &source)
                            {
                                binding.place = destination.place.clone();
                            }
                        }
                    }
                }
            }
        }

        pub(super) fn qualify(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            required: &BoundPredicate,
            premises: &[BoundPredicate],
            evidence: Vec<ConditionalPredicate>,
            conditions: &mut SelectionConditions,
        ) -> PredicateTransport {
            let mut result = PredicateTransport::default();
            for proposition in &evidence {
                if conditions.covers(&facts.flow.control, proposition.condition) {
                    result.predicates.push(proposition.predicate.clone());
                }
            }
            if evidence
                .iter()
                .all(|proposition| conditions.covers(&facts.flow.control, proposition.condition))
            {
                return result;
            }
            let mut proven = conditions.never();
            let mut supports = Vec::new();
            for support in &evidence {
                if supports.contains(&support.condition)
                    || !conditions.possible(&facts.flow.control, support.condition)
                {
                    continue;
                }
                supports.push(support.condition);
                let mut hypotheses = premises.to_vec();
                for proposition in &evidence {
                    if implies(facts, conditions, support.condition, proposition.condition) {
                        hypotheses.push(proposition.predicate.clone());
                    }
                }
                if proves_captured_predicates(
                    program,
                    &facts.semantic,
                    &[],
                    &hypotheses,
                    required,
                    |_| None,
                ) {
                    proven = conditions.any(proven, support.condition);
                    if conditions.covers(&facts.flow.control, proven) {
                        break;
                    }
                }
            }
            result.proves_required = conditions.covers(&facts.flow.control, proven);
            result
        }
    }
    mod transport {
        /*
            Rebind one producer's predicates at its own read epoch. Every dependency
            needs an alias valid for all destination candidates under that producer's
            condition. Conditions combine; source premise contexts never do.
        */
        use super::bindings::Alias;
        use super::{BoundObservation, BoundPredicate, CapturedBinding, CheckFacts, FlowStateFact};
        use crate::flow::{SelectionCondition, SelectionConditions};

        pub(super) struct ConditionalPredicate {
            pub predicate: BoundPredicate,
            pub condition: SelectionCondition,
        }

        pub(super) fn predicates(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            state: &FlowStateFact,
            required: &BoundPredicate,
            captures: &[CapturedBinding<'_>],
            selections: &[SelectionCondition],
            conditions: &mut SelectionConditions,
            frames: Option<&validation::CallFrameResolver<'_>>,
        ) -> Vec<ConditionalPredicate> {
            let mut transported = super::case_evidence::predicates(
                program, facts, state, required, captures, selections, conditions,
            );
            for (capture, &selection) in captures.iter().zip(selections) {
                for alias in &capture.aliases {
                    let contexts = facts
                        .flow
                        .semantic_constraint_contexts(alias.epoch.constraints)
                        .collect::<Vec<_>>();
                    let mut predicates = crate::published::facts::boolean_predicates::retained(
                        program,
                        &facts.operators,
                        &facts.semantic,
                        &contexts,
                        facts::ProgramPoint::Statement {
                            machine_symbol: state.machine_symbol,
                            state_symbol: alias.epoch.state_symbol,
                            statement_index: alias.epoch.statement_index,
                        },
                    );
                    if let (Some(frames), Some(origin)) = (
                        frames,
                        facts
                            .flow
                            .control
                            .state(state.machine_symbol, alias.epoch.state_symbol),
                    ) {
                        predicates.extend(
                            super::super::prover::call_guarantees::current_predicates(
                                program,
                                facts,
                                origin,
                                super::super::prover::call_guarantees::GuaranteeUse::StatementEntry(
                                    alias.epoch.statement_index + 1,
                                ),
                                &contexts,
                                frames,
                            ),
                        );
                    }
                    let cases = crate::flow::CaseEvidence::prepare(
                        program,
                        &facts.semantic,
                        &contexts,
                        state.machine_symbol,
                        alias.epoch.state_symbol,
                        alias.epoch.statement_index,
                    );
                    while let Some(mut predicate) = predicates.pop() {
                        crate::published::facts::boolean_predicates::fold_cases(
                            &mut predicate,
                            |binding, case| {
                                cases.as_ref()?.case_truth(
                                    &binding.place,
                                    binding.type_reference,
                                    case,
                                )
                            },
                        );
                        if let checked_trees::CheckedBooleanExpression::And { left, right } =
                            &predicate.expression
                        {
                            predicates.push(BoundPredicate {
                                expression: *left.clone(),
                                ..predicate.clone()
                            });
                            predicates.push(BoundPredicate {
                                expression: *right.clone(),
                                ..predicate
                            });
                            continue;
                        }
                        let source_bindings = std::mem::take(&mut predicate.bindings);
                        let complete =
                            crate::published::facts::boolean_predicates::map_observations(
                                &mut predicate.expression,
                                &mut |operand| {
                                    let observation = source_bindings
                                        .iter()
                                        .find(|binding| binding.operand == *operand)?;
                                    let mut destination = None;
                                    for (position, binding) in required.bindings.iter().enumerate()
                                    {
                                        if !binding.compatible_with(program, observation) {
                                            continue;
                                        }
                                        let support = super::coverage::dependency(
                                            program,
                                            facts,
                                            captures,
                                            selections,
                                            conditions,
                                            &Alias {
                                                place: observation.place.clone(),
                                                epoch: alias.epoch,
                                            },
                                            position,
                                        );
                                        if super::coverage::implies(
                                            facts, conditions, selection, support,
                                        ) {
                                            destination = Some(position);
                                            break;
                                        }
                                    }
                                    let Some(position) = destination else {
                                        let value = crate::values::scalar_value_at_place(
                                            program,
                                            &facts.semantic,
                                            contexts.iter().map(|context| {
                                                facts.semantic.contexts.get(*context)
                                            }),
                                            &observation.place,
                                        )?;
                                        *operand = super::super::call_values::observed_literal(
                                            value,
                                            observation.operand.primitive_type()?,
                                        )?
                                        .into();
                                        return Some(());
                                    };
                                    let binding = &required.bindings[position];
                                    *operand = binding.operand.clone();
                                    let rebound = BoundObservation {
                                        operand: operand.clone(),
                                        place: binding.place.clone(),
                                        type_reference: binding.type_reference,
                                    };
                                    if !predicate.bindings.contains(&rebound) {
                                        predicate.bindings.push(rebound);
                                    }
                                    Some(())
                                },
                            );
                        if complete.is_none() {
                            continue;
                        }
                        predicate.machine = required.machine;
                        predicate.entry_state = required.entry_state;
                        if let Some(known) = transported
                            .iter_mut()
                            .find(|known| known.predicate == predicate)
                        {
                            known.condition = conditions.any(known.condition, selection);
                        } else {
                            transported.push(ConditionalPredicate {
                                predicate,
                                condition: selection,
                            });
                        }
                    }
                }
            }
            transported
        }
    }
    pub(super) use bindings::{
        CapturedBinding, collect_alternative_reads, retain_alternative_reads, retain_bindings,
    };
    pub(crate) use cleanup::cleanup_predicates;

    #[derive(Default)]
    pub(crate) struct PredicateTransport {
        pub predicates: Vec<BoundPredicate>,
        pub proves_required: bool,
    }

    pub(super) fn transport(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        required: &mut BoundPredicate,
        captures: &[CapturedBinding<'_>],
        premises: &[BoundPredicate],
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> PredicateTransport {
        let mut conditions = crate::flow::SelectionConditions::new();
        let Some(selections) = captures
            .iter()
            .map(|capture| conditions.choices(&facts.flow.control, &capture.choices))
            .collect::<Option<Vec<_>>>()
        else {
            return PredicateTransport::default();
        };
        coverage::join_identities(
            program,
            facts,
            required,
            captures,
            &selections,
            &mut conditions,
        );
        let evidence = transport::predicates(
            program,
            facts,
            state,
            required,
            captures,
            &selections,
            &mut conditions,
            frames,
        );
        coverage::qualify(
            program,
            facts,
            required,
            premises,
            evidence,
            &mut conditions,
        )
    }
}
pub(crate) use captured_predicates::cleanup_predicates;
pub(crate) mod call_values {
    /*
        Bind invocation predicates to actual values captured in source order.

        Detached copies keep operand-time observations; reference operands use the

        invocation's live referents. Cleanup requirements and concrete write checks
        share this substitution law. Neither may reread an initializer or substitute
        a later value for an operand already evaluated.
    */
    mod predicate_subjects {
        /*
            Predicate subjects bind completed values without performing their reads.
            Owned paths preserve conditional payload coordinates; actual reference
            crossings keep the ordinary captured-read and live-memory obligations.
        */
        use super::Actual;
        use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};

        /*
            A formula's owned payload projections are conditional coordinates, not
            eagerly performed reads. Retain the already-completed carrier and its typed
            path even when another case is active. Reference crossings still use the
            ordinary observed-read route and cannot freeze pointee contents here.
        */
        pub(super) fn values<'facts>(
            program: &dyn validation::CheckingInput,
            facts: &'facts CheckFacts,
            state: &FlowStateFact,
            call: &FlowCallFact,
            actual: &Actual<'_>,
        ) -> Option<Vec<crate::flow::CapturedValue<'facts>>> {
            if !actual.path.is_empty()
                && actual
                    .path
                    .iter()
                    .all(crate::flow::place_segment_has_static_identity)
                && validation::reference_shell(program, actual.parameter.type_reference).is_none()
                && validation::owned_projection_type_reference(
                    program,
                    actual.parameter.type_reference,
                    &actual.path,
                )
                .is_some_and(|selected| validation::reference_shell(program, selected).is_none())
            {
                let root = Actual {
                    parameter: actual.parameter,
                    argument: actual.argument,
                    path: Vec::new(),
                    operand_position: actual.operand_position,
                };
                let mut values = super::captured_values(program, facts, state, call, &root)?;
                for value in &mut values {
                    value
                        .observation_projection
                        .remaining
                        .extend_from_slice(&actual.path);
                    value.projection.remaining.extend_from_slice(&actual.path);
                }
                Some(values)
            } else {
                super::captured_values(program, facts, state, call, actual)
            }
        }

        /*
            Bind a structural tag observation to the completed actual's existing
            namespace. This publishes no selected case or payload read permission.
        */
        pub(super) fn current_case(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            captured: &crate::flow::CapturedValue<'_>,
            bindings: &mut Vec<crate::published::facts::boolean_predicates::BoundObservation>,
        ) -> Option<facts::BooleanPredicateOperand> {
            let places =
                crate::flow::captured_observation_places(program, facts.flow.view(), captured)?;
            let [observed] = places.as_slice() else {
                return None;
            };
            let reference = validation::project_type_reference_from_segments(
                program,
                captured.observation_type,
                &captured.observation_projection.remaining,
            )
            .and_then(|reference| {
                validation::field_domains::readable_type_reference(program, reference)
            })?;
            let data = program.closed_data_definition(reference)?;
            if !program
                .data_members(data)
                .iter()
                .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
            {
                return None;
            }
            let operand = facts::BooleanPredicateOperand::CaseSubject(
                checked_trees::CheckedStructuralCaseSubject {
                    root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                        position: u32::try_from(bindings.len()).ok()?,
                    },
                    path: Vec::new(),
                },
            );
            bindings.push(
                crate::published::facts::boolean_predicates::BoundObservation {
                    operand: operand.clone(),
                    place: observed.projected.clone(),
                    type_reference: reference,
                },
            );
            Some(operand)
        }

        /*
            Preserve the carrier observations actually used by an owned payload read.
            They are proof dependencies even when the callee consumes only its scalar.
            The shared completed-storage owner authenticates this receiver/projection;
            equal scalar contents or a printed place prefix cannot create the relation.
        */
        pub(super) fn retain_owned_cases<'facts>(
            program: &dyn validation::CheckingInput,
            facts: &'facts CheckFacts,
            state: &FlowStateFact,
            operand: crate::flow::CapturedOperand<'facts>,
            predicate: &mut crate::published::facts::boolean_predicates::BoundPredicate,
            epochs: &mut Vec<&'facts checked_trees::FlowOperandFact>,
            captures: &mut Vec<super::captured_predicates::CapturedBinding<'facts>>,
        ) -> Option<()> {
            let Some((receiver, path)) =
                validation::completed_storage::owned_observation_projection(
                    program,
                    &facts.flow.control,
                    operand.recorded_in(&facts.flow.control)?,
                    None,
                )?
            else {
                return Some(());
            };
            let receiver = crate::flow::CapturedOperand::recorded(&facts.flow.control, receiver)?;
            for (position, segment) in path.iter().enumerate() {
                if !matches!(segment, facts::PlaceSegment::Case { .. }) {
                    continue;
                }
                let prefix = &path[..position];
                let reference = validation::owned_projection_type_reference(
                    program,
                    receiver.type_reference,
                    prefix,
                )?;
                if validation::reference_shell(program, reference).is_some() {
                    return None;
                }
                let values = crate::flow::captured_operand_values(
                    program,
                    facts.flow.view(),
                    &facts.semantic,
                    state,
                    receiver,
                    receiver.type_reference,
                    prefix,
                )?;
                let first = values.first()?;
                if values.iter().any(|value| {
                    value.observation.recorded_in(&facts.flow.control)
                        != first.observation.recorded_in(&facts.flow.control)
                        || value.observation_type != first.observation_type
                        || value.observation_projection.remaining
                            != first.observation_projection.remaining
                }) {
                    return None;
                }
                let start = predicate.bindings.len();
                current_case(program, facts, first, &mut predicate.bindings)?;
                let binding = &predicate.bindings[start];
                for (previous, previous_epoch) in
                    predicate.bindings[..start].iter().zip(epochs.iter())
                {
                    if previous.place == binding.place
                        && !crate::flow::operand_observations_agree(
                            program,
                            facts.flow.view(),
                            previous_epoch,
                            first.observation.row(),
                            &binding.place,
                        )
                    {
                        return None;
                    }
                }
                for value in &values {
                    super::captured_predicates::retain_bindings(
                        program,
                        facts,
                        value,
                        first.observation.row(),
                        &predicate.bindings[start..],
                        start,
                        captures,
                    );
                }
                epochs.push(first.observation.row());
            }
            Some(())
        }
    }

    use crate::published::facts::boolean_predicates;
    use crate::published::facts::entry_parameter_place_before_statement;
    use crate::published::places::place_at;
    use validation::CanonicalPlace;

    use checked_trees::{
        CheckFacts, CheckedBooleanExpression, CheckedScalarExpression, FlowCallFact, FlowStateFact,
    };
    use facts::FactContextHandle;
    use symbols::SymbolHandle;
    use typed_trees::{expression::ExpressionHandle, signature::StateParameter};

    use super::captured_predicates;

    pub(crate) fn target_machine(
        program: &dyn ::validation::CheckingInput,
        call: &FlowCallFact,
    ) -> Option<SymbolHandle> {
        program
            .machine_by_symbol(call.target_symbol)
            .map(|machine| machine.symbol)
            .or_else(|| {
                validation::find_machine_by_entry_state(program, call.target_symbol)
                    .map(|(machine, _)| machine.symbol)
            })
    }

    /*
        Entry predicates use the performing body's formal identities while source
        captures keep the called declaration's telescope. Only an exact checked
        family member or a satisfier of the called requirement shares that
        positional correspondence; other mismatches refuse.
    */
    fn invocation_body_parameters<'program>(
        program: &'program dyn validation::CheckingInput,
        call: &FlowCallFact,
        body: SymbolHandle,
    ) -> Option<&'program [StateParameter]> {
        let target = target_machine(program, call);
        if target != Some(body) {
            let family_member = target
                .and_then(|target| program.machine_by_symbol(target))
                .and_then(|interface| validation::target_family_members(program, interface))
                .is_some_and(|members| members.iter().any(|member| member.machine == body));
            let implementation =
                validation::requirement_backing_machines(program, call.target_symbol)
                    .contains(&body);
            (family_member || implementation).then_some(())?;
        }
        let machine = program.machine_by_symbol(body)?;
        Some(program.state_parameters(program.machine_states(machine).first()?))
    }

    fn body_observation_actual<'program>(
        program: &'program dyn validation::CheckingInput,
        state: &FlowStateFact,
        call: &FlowCallFact,
        body: SymbolHandle,
        observation: &boolean_predicates::BoundObservation,
    ) -> Option<Actual<'program>> {
        let parameters = invocation_body_parameters(program, call, body)?;
        let position = parameters.iter().position(|parameter| {
            observation.place.root == facts::PlaceRoot::Symbol(parameter.symbol)
        })?;
        actual_at(
            program,
            state,
            call,
            position,
            observation.place.segments.clone(),
        )
    }

    pub(crate) struct Actual<'program> {
        pub(crate) parameter: &'program StateParameter,
        pub(crate) argument: Option<ExpressionHandle>,
        pub(crate) path: Vec<facts::PlaceSegment>,
        pub(crate) operand_position: usize,
    }

    pub(crate) fn actual_at<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        call: &FlowCallFact,
        position: usize,
        path: Vec<facts::PlaceSegment>,
    ) -> Option<Actual<'program>> {
        let site = validation::find_call_site(
            program,
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let parameter = parameters.get(position)?;
        let receiver_offset = usize::from(
            matches!(&site, validation::CallSite::Expression { call, .. } if call.receiver.is_valid()),
        );
        let ordinal = parameters[..position].len();
        let operand_position = { ordinal + receiver_offset };
        let argument = {
            let ordinal = parameters[..position].len();
            Some(*validation::call_site_argument_expressions(program, &site).get(ordinal)?)
        };
        Some(Actual {
            parameter,
            argument,
            path,
            operand_position,
        })
    }

    /*
        Bind a callee observation to its actual operand, without assuming that the
        formal's declaration position is a dense scalar position.
    */
    pub(crate) fn observation_actual<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state: &FlowStateFact,
        call: &FlowCallFact,
        observation: &boolean_predicates::BoundObservation,
    ) -> Option<Actual<'program>> {
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let position = parameters.iter().position(|parameter| {
            observation.place.root == facts::PlaceRoot::Symbol(parameter.symbol)
        })?;
        actual_at(
            program,
            state,
            call,
            position,
            observation.place.segments.clone(),
        )
    }

    pub(in crate::checks::contracts) fn captured_operand<'facts>(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        actual: &Actual<'_>,
    ) -> Option<crate::flow::CapturedOperand<'facts>> {
        let operands =
            crate::flow::captured_call_operands(program, facts.flow.view(), state, call)?.0;
        operands.get(actual.operand_position)?;
        crate::flow::CapturedOperand::recorded(
            &facts.flow.control,
            call.operands
                .handle_at(u32::try_from(actual.operand_position).ok()?),
        )
    }

    /*
        The complete call telescope in exact captured source order. Static namespace
        receivers are evaluation positions but not explicit argument parameters;
        retain the same receiver offset used by individual captured observations.
    */
    pub(crate) fn captured_argument_expressions(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
    ) -> Option<Vec<ExpressionHandle>> {
        if !call.authored_source_custody_valid {
            return None;
        }
        let (operands, receiver_offset) =
            crate::flow::captured_call_operands(program, facts.flow.view(), state, call)?;
        Some(
            operands[receiver_offset..]
                .iter()
                .map(|operand| operand.expression)
                .collect(),
        )
    }

    pub(crate) fn captured_values<'facts>(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        actual: &Actual<'_>,
    ) -> Option<Vec<crate::flow::CapturedValue<'facts>>> {
        let operands =
            crate::flow::captured_call_operands(program, facts.flow.view(), state, call)?.0;
        operands.get(actual.operand_position)?;
        let operand = crate::flow::CapturedOperand::recorded(
            &facts.flow.control,
            call.operands
                .handle_at(u32::try_from(actual.operand_position).ok()?),
        )?;
        let epoch = operands.last()?;
        let values = crate::flow::captured_operand_values(
            program,
            facts.flow.view(),
            &facts.semantic,
            state,
            operand,
            actual.parameter.type_reference,
            &actual.path,
        )?;
        Some(crate::flow::observe_captured_values(
            program,
            &facts.flow,
            &facts.semantic,
            state,
            values,
            epoch,
        ))
    }

    fn captured_value<'facts>(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        actual: &Actual<'_>,
    ) -> Option<crate::flow::CapturedValue<'facts>> {
        let mut values = captured_values(program, facts, state, call, actual)?;
        if values.len() != 1 {
            return None;
        }
        values.pop()
    }

    /*
        Complete ordered operand evidence supplies closed arithmetic observations
        before any source-substitution tier can reinterpret an earlier descriptor.
    */
    pub(crate) fn proves_captured_arithmetic_requirement(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        expression: ExpressionHandle,
    ) -> bool {
        let Some((machine, body)) = validation::find_state(program, call.target_symbol)
            .or_else(|| validation::find_machine_head(program, call.target_symbol))
        else {
            return false;
        };
        if captured_argument_expressions(program, facts, state, call).is_none() {
            return false;
        }
        validation::prove_captured_arithmetic_call_requirement(
            program,
            machine,
            body,
            expression,
            |formal, observation| {
                captured_arithmetic_observation(program, facts, state, call, formal, observation)
            },
        )
    }

    /*
        A requirement observes only its demanded formal coordinate. The complete
        ordered call roster remains authenticated by `captured_call_operands`; this
        projection does not suppress evaluation or availability of another actual.
        Scalar values use the established copy/live-referent epoch. Descriptor
        geometry closes in its own completed operand epoch, not in later invocation
        facts about the selector or endpoints that built that descriptor.
    */
    pub(crate) fn captured_arithmetic_observation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        formal: SymbolHandle,
        observation: validation::ArithmeticCallObservation<'_>,
    ) -> Option<numerics::bignum::BigInt> {
        if !call.authored_source_custody_valid {
            return None;
        }
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        let position = parameters
            .iter()
            .position(|parameter| parameter.symbol == formal)?;
        let path = match observation {
            validation::ArithmeticCallObservation::Value(path)
            | validation::ArithmeticCallObservation::CollectionLength(path) => path
                .iter()
                .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                .collect(),
        };
        let actual = actual_at(program, state, call, position, path)?;
        let captured = captured_value(program, facts, state, call, &actual)?;
        if captured.observation.state_symbol != state.state_symbol
            || captured.observation.statement_index != call.statement_index
            || captured.operand.state_symbol != state.state_symbol
            || captured.operand.statement_index > call.statement_index
        {
            return None;
        }
        let (machine, body) = validation::machine_state_by_symbols(
            program,
            state.machine_symbol,
            state.state_symbol,
        )?;
        match observation {
            validation::ArithmeticCallObservation::Value(_) => {
                let reference = validation::project_type_reference_from_segments(
                    program,
                    actual.parameter.type_reference,
                    &actual.path,
                )?;
                let primitive = program.primitive_type_reference(reference)?;
                if !primitive.is_fixed_integer() {
                    return None;
                }
                let contexts = actual_contexts(program, facts, state, call, &actual);
                let value = current_actual(
                    program,
                    facts,
                    state,
                    call,
                    &captured,
                    primitive,
                    &contexts,
                    true,
                    &mut Vec::new(),
                )?;
                match crate::values::evaluate_checked_scalar(&value, &mut |_| None)? {
                    facts::ScalarValue::Integer(value) => Some(value),
                    _ => None,
                }
            }
            validation::ArithmeticCallObservation::CollectionLength(_) => {
                if let Some(length) = crate::checks::collection_extents::from_captured_value(
                    program,
                    &facts.borrow,
                    &facts.flow,
                    &captured,
                    facts
                        .flow
                        .contexts
                        .constraint_refs
                        .span_or_empty(call.entry_constraints),
                ) {
                    return Some(numerics::bignum::BigInt::from_u64(
                        u64::try_from(length).ok()?,
                    ));
                }
                if !captured.projection.remaining.is_empty() {
                    return None;
                }
                let contexts = facts
                    .flow
                    .semantic_constraint_contexts(captured.operand.constraints)
                    .collect::<Vec<_>>();
                let length = validation::captured_collection_length(
                    program,
                    machine,
                    body,
                    captured.projection.expression,
                    |_observation, place| {
                        let reference = validation::canonical_place_type_reference(
                            program,
                            state.state_symbol,
                            captured.operand.statement_index,
                            place,
                        )?;
                        if !program
                            .primitive_type_reference(reference)?
                            .is_fixed_integer()
                        {
                            return None;
                        }
                        match scalar_at(
                            program,
                            facts,
                            &contexts,
                            facts::ProgramPoint::Statement {
                                machine_symbol: state.machine_symbol,
                                state_symbol: captured.operand.state_symbol,
                                statement_index: captured.operand.statement_index,
                            },
                            place,
                        )? {
                            facts::ScalarValue::Integer(value) => Some(value),
                            _ => None,
                        }
                    },
                )?;
                Some(numerics::bignum::BigInt::from_u64(length))
            }
        }
    }

    #[derive(Default)]
    pub(super) struct MembershipCase {
        pub contexts: Vec<FactContextHandle>,
        pub referents: Option<Vec<CanonicalPlace>>,
    }

    pub(super) struct MembershipEvidence {
        /*
            Each possible completed value owns its own evidence epoch. Contexts and
            referents from different selections are never a joint premise set.
        */
        pub cases: Vec<MembershipCase>,
        /*
            Exact completed operand for a whole-formal membership. A projected
            requirement cannot qualify the enclosing operand.
        */
        pub whole_argument: Option<ExpressionHandle>,
    }

    impl Default for MembershipEvidence {
        fn default() -> Self {
            Self {
                cases: vec![MembershipCase::default()],
                whole_argument: None,
            }
        }
    }

    /*
        Bind the subject and evidence epoch of a declared membership's exact formal. A
        copied argument retains completion facts even when a later operand writes
        its source; borrowed arguments retain the ordinary invocation-live rules.
        The declaration's subject and its substituted place must agree with the
        same argument occurrence. A missing capture supplies no evidence.
    */
    pub(super) fn membership_evidence(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        requirement: &facts::Fact,
    ) -> Option<MembershipEvidence> {
        let facts::FactPayload::ContractDomainMembership {
            value,
            fact: source_fact,
            ..
        } = requirement.payload
        else {
            return None;
        };
        let declared = validation::canonical_place_from_expression(program, value)?;
        let symbol = declared.root.as_symbol()?;
        let Some(contract_call) = facts.proof.contract_call_at(&(
            state.machine_symbol,
            state.state_symbol,
            call.statement_index,
            call.call_ordinal,
        )) else {
            return Some(MembershipEvidence::default());
        };
        let mut position = None;
        for reference in facts
            .proof
            .contract_fact_refs
            .span_or_empty(contract_call.requires)
        {
            let contract = facts.proof.contract_facts.get(reference.fact);
            if contract.fact != source_fact {
                continue;
            }
            let bound = match crate::published::semantic::facts::contract_parameter_position(
                program,
                &facts.proof,
                contract,
                contract_call.target_state_symbol,
                symbol,
            ) {
                Ok(Some(bound)) => bound,
                Ok(None) => return None,
                Err(()) => return Some(MembershipEvidence::default()),
            };
            if position.is_some_and(|position| position != bound) {
                return Some(MembershipEvidence::default());
            }
            position = Some(bound);
        }
        let Some(position) = position else {
            return Some(MembershipEvidence::default());
        };
        let Some(actual) = actual_at(program, state, call, position, declared.segments) else {
            return Some(MembershipEvidence::default());
        };
        let Some(place) = requirement.place.as_place() else {
            return Some(MembershipEvidence::default());
        };
        if let Some((_, _, domain_symbol, semantic_domain)) =
            requirement.payload.domain_membership()
        {
            let erased = (actual.path.is_empty()
                && actual.argument.is_some_and(|argument| {
                    validation::field_domains::expression_erases_membership(
                        program,
                        argument,
                        domain_symbol,
                        semantic_domain,
                    )
                }))
                || u32::try_from(actual.operand_position)
                    .ok()
                    .map(|pos| call.operands.handle_at(pos))
                    .is_some_and(|operand_handle| {
                        operand_erases_membership(
                            program,
                            &facts.flow.control,
                            operand_handle,
                            &actual.path,
                            domain_symbol,
                            semantic_domain,
                            &mut Vec::new(),
                        )
                    });
            if erased {
                return Some(MembershipEvidence {
                    cases: Vec::new(),
                    whole_argument: None,
                });
            }
        }
        let Some(required) = crate::flow::canonical_place_from_semantic_place(
            program,
            &facts.semantic,
            facts.semantic.places.get(place),
        ) else {
            return Some(MembershipEvidence::default());
        };
        let Some(expected) = actual.argument.and_then(|argument| {
            crate::published::semantic::facts::projected_argument_place(
                program,
                state.state_symbol,
                call.statement_index,
                argument,
                actual.parameter.type_reference,
                &actual.path,
            )
        }) else {
            return Some(MembershipEvidence::default());
        };
        if required.root != expected.root
            || required.segments.len() != expected.segments.len()
            || !required
                .segments
                .iter()
                .zip(&expected.segments)
                .all(|(left, right)| {
                    crate::flow::canonical_place_segments_equal(program, *left, *right)
                })
        {
            return Some(MembershipEvidence::default());
        }
        let Some(values) = captured_values(program, facts, state, call, &actual) else {
            return Some(MembershipEvidence::default());
        };
        let mut cases = values
            .iter()
            .map(|captured| {
                let contexts = crate::flow::operand_contexts(
                    program,
                    &facts.flow.contexts,
                    &facts.flow.control,
                    &facts.semantic,
                    captured.observation.row(),
                    captured.observation_type,
                    call.entry_constraints,
                );
                let referents =
                    crate::flow::operand_binds_live_view(program, captured.type_reference).then(
                        || {
                            if captured.operand.referents.is_empty() {
                                return Vec::new();
                            }
                            crate::flow::captured_value_places(program, facts.flow.view(), captured)
                                .unwrap_or_default()
                                .into_iter()
                                .map(|place| place.projected)
                                .collect()
                        },
                    );
                MembershipCase {
                    contexts,
                    referents,
                }
            })
            .collect::<Vec<_>>();
        if cases.is_empty() {
            cases.push(MembershipCase::default());
        }
        Some(MembershipEvidence {
            cases,
            whole_argument: actual.path.is_empty().then_some(actual.argument).flatten(),
        })
    }

    /*
        A projected field observes its own completed child. A stored aggregate
        retains its whole-value capture; its initializer is never replayed.
    */
    pub(crate) fn actual_contexts(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        actual: &Actual<'_>,
    ) -> Vec<FactContextHandle> {
        let Some(captured) = captured_value(program, facts, state, call, actual) else {
            return Vec::new();
        };
        crate::flow::operand_contexts(
            program,
            &facts.flow.contexts,
            &facts.flow.control,
            &facts.semantic,
            captured.observation.row(),
            captured.observation_type,
            call.entry_constraints,
        )
    }

    /*
        Rebind a selected actual expression to exact current storage. Local slots
        denote stored values; their initializer expressions are never replayed.
    */
    #[allow(clippy::too_many_arguments)]
    fn current_actual(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        captured: &crate::flow::CapturedValue<'_>,
        primitive: typed_trees::types::PrimitiveType,
        contexts: &[FactContextHandle],
        fold_observed_values: bool,
        bindings: &mut Vec<boolean_predicates::BoundObservation>,
    ) -> Option<CheckedScalarExpression> {
        let (machine, body) = validation::machine_state_by_symbols(
            program,
            state.machine_symbol,
            state.state_symbol,
        )?;
        let capture = captured.observation.row();
        let projection = &captured.observation_projection;
        let expression = projection.expression;
        let observed_type = validation::project_type_reference_from_segments(
            program,
            captured.observation_type,
            &projection.remaining,
        )
        .and_then(|reference| {
            validation::field_domains::readable_type_reference(program, reference)
        })?;
        if matches!(
            capture.completion,
            checked_trees::FlowOperandCompletion::Copied { .. }
                | checked_trees::FlowOperandCompletion::Selected(_)
        ) {
            let places =
                crate::flow::captured_observation_places(program, facts.flow.view(), captured)?;
            let [place] = places.as_slice() else {
                return None;
            };
            return bind_observation(
                program,
                facts,
                state,
                call,
                place.projected.clone(),
                observed_type,
                primitive,
                contexts,
                fold_observed_values,
                bindings,
            );
        }
        if projection.remaining.is_empty()
            && validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(body),
                expression,
            )
            && let Some(value) = crate::values::lower_unit_scalar_argument_for_places(
                program,
                &facts.operators,
                body,
                capture.statement_index,
                expression,
                primitive,
                |place, primitive| {
                    bind_current(
                        program,
                        facts,
                        state,
                        call,
                        place.clone(),
                        primitive,
                        contexts,
                        fold_observed_values,
                        bindings,
                    )
                },
            )
        {
            return Some(value);
        }
        let mut place = place_at(
            program,
            capture.state_symbol,
            capture.statement_index,
            expression,
        )?;
        place.extend_segments(&projection.remaining);
        bind_observation(
            program,
            facts,
            state,
            call,
            place,
            observed_type,
            primitive,
            contexts,
            fold_observed_values,
            bindings,
        )
    }

    fn bind_current(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        place: CanonicalPlace,
        primitive: typed_trees::types::PrimitiveType,
        contexts: &[FactContextHandle],
        fold_observed_values: bool,
        bindings: &mut Vec<boolean_predicates::BoundObservation>,
    ) -> Option<CheckedScalarExpression> {
        let type_reference = validation::canonical_place_type_reference(
            program,
            state.state_symbol,
            call.statement_index,
            &place,
        )?;
        bind_observation(
            program,
            facts,
            state,
            call,
            place,
            type_reference,
            primitive,
            contexts,
            fold_observed_values,
            bindings,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_observation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        place: CanonicalPlace,
        type_reference: typed_trees::types::TypeReferenceHandle,
        primitive: typed_trees::types::PrimitiveType,
        contexts: &[FactContextHandle],
        fold_observed_values: bool,
        bindings: &mut Vec<boolean_predicates::BoundObservation>,
    ) -> Option<CheckedScalarExpression> {
        if program.primitive_type_reference(type_reference) != Some(primitive) {
            return None;
        }
        if fold_observed_values
            && let Some(value) = scalar_at(
                program,
                facts,
                contexts,
                facts::ProgramPoint::Statement {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                },
                &place,
            )
            && let Some(literal) = observed_literal(value, primitive)
        {
            return Some(literal);
        }
        let operand = CheckedScalarExpression::Parameter {
            position: bindings.len(),
            primitive_type: primitive,
        };
        bindings.push(boolean_predicates::BoundObservation {
            operand: operand.clone().into(),
            place,
            type_reference,
        });
        Some(operand)
    }

    fn scalar_at(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        point: facts::ProgramPoint,
        place: &CanonicalPlace,
    ) -> Option<facts::ScalarValue> {
        super::prover::boolean_value_at_place(
            program,
            &facts.operators,
            &facts.semantic,
            contexts,
            point,
            place,
        )
        .map(facts::ScalarValue::Boolean)
        .or_else(|| {
            crate::values::scalar_value_at_place(
                program,
                &facts.semantic,
                contexts
                    .iter()
                    .map(|handle| facts.semantic.contexts.get(*handle)),
                place,
            )
        })
    }

    pub(super) fn observed_literal(
        value: facts::ScalarValue,
        primitive: typed_trees::types::PrimitiveType,
    ) -> Option<CheckedScalarExpression> {
        match value {
            facts::ScalarValue::Boolean(value)
                if primitive == typed_trees::types::PrimitiveType::Bool =>
            {
                Some(CheckedScalarExpression::Boolean(Box::new(
                    CheckedBooleanExpression::Constant(value),
                )))
            }
            facts::ScalarValue::Integer(value) => {
                let text = value.to_string();
                let digits = text.strip_prefix('-').unwrap_or(&text);
                let literal = numerics::literals::IntegerLiteral::from_parts(
                    text.starts_with('-'),
                    numerics::literals::IntegerRadix::Decimal,
                    digits,
                )
                .ok()?
                .with_landing(numerics::literals::IntegerLanding {
                    landed_type: crate::values::landed_for_primitive(primitive)?,
                    domain: numerics::arithmetic::ArithmeticDomain::Exact,
                });
                Some(CheckedScalarExpression::IntegerLiteral { literal })
            }
            _ => None,
        }
    }

    pub(crate) fn proves_entry_predicate(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[FactContextHandle],
        target: SymbolHandle,
        expression: CheckedBooleanExpression,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(predicate) = boolean_predicates::bind_entry(program, target, expression) else {
            return false;
        };
        if invocation_body_parameters(program, call, target).is_none() {
            return false;
        }
        proves_bound_predicate(
            program,
            facts,
            state,
            call,
            contexts,
            &predicate,
            frames,
            &|observation| body_observation_actual(program, state, call, target, observation),
        )
    }

    /*
        The source row owns its formal-to-actual correspondence. Entry cleanup,
        ordinary declarations, named states and inherited rows share one captured
        Boolean algebra after supplying that exact correspondence.
    */
    pub(crate) fn proves_bound_predicate<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[FactContextHandle],
        predicate: &boolean_predicates::BoundPredicate,
        frames: Option<&validation::CallFrameResolver<'_>>,
        actual: &impl Fn(&boolean_predicates::BoundObservation) -> Option<Actual<'program>>,
    ) -> bool {
        proves_bound_predicate_mode(
            program, facts, state, call, contexts, predicate, frames, actual, true,
        )
    }

    fn proves_bound_predicate_mode<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        contexts: &[FactContextHandle],
        predicate: &boolean_predicates::BoundPredicate,
        frames: Option<&validation::CallFrameResolver<'_>>,
        actual: &impl Fn(&boolean_predicates::BoundObservation) -> Option<Actual<'program>>,
        fold_observed_values: bool,
    ) -> bool {
        let mut alternative_reads = Vec::new();
        for observation in &predicate.bindings {
            if let Some(actual) = actual(observation)
                && let Some(values) = captured_values(program, facts, state, call, &actual)
            {
                captured_predicates::collect_alternative_reads(
                    program,
                    facts,
                    &values,
                    &mut alternative_reads,
                );
            }
        }
        let mut selected_contexts: Option<Vec<FactContextHandle>> = None;
        let mut binding_epochs: Vec<&checked_trees::FlowOperandFact> = Vec::new();
        let mut captures = Vec::new();
        let mut observed_operands = Vec::new();
        let mut current = predicate.clone();
        current.bindings.clear();
        if boolean_predicates::map_observations(&mut current.expression, &mut |operand| {
            let observation = predicate.binding(operand)?;
            let actual = actual(observation)?;
            if fold_observed_values
                && let Some(primitive) = operand.primitive_type()
                && let Some(frames) = frames
                && let Some(capture) = captured_operand(program, facts, state, call, &actual)
                && let Some(value) = super::prover::captured_scalar_at_path(
                    program,
                    facts,
                    state,
                    capture.row(),
                    &actual.path,
                    call.statement_index.checked_add(1)?,
                    call.entry_constraints,
                    frames,
                )
                && let Some(literal) = observed_literal(value, primitive)
            {
                *operand = literal.into();
                return Some(());
            }
            if let Some(operand) = captured_operand(program, facts, state, call, &actual)
                && !observed_operands
                    .iter()
                    .any(|known: &crate::flow::CapturedOperand<'_>| {
                        known.recorded_in(&facts.flow.control)
                            == operand.recorded_in(&facts.flow.control)
                    })
            {
                observed_operands.push(operand);
            }
            let values = predicate_subjects::values(program, facts, state, call, &actual)?;
            let captured = values.first()?;
            let observation = captured.observation.recorded_in(&facts.flow.control)?;
            if !values.iter().all(|value| {
                value.observation.recorded_in(&facts.flow.control) == Some(observation)
                    && value.observation_type == captured.observation_type
                    && value.observation_projection.expression
                        == captured.observation_projection.expression
                    && value.observation_projection.remaining
                        == captured.observation_projection.remaining
            }) {
                return None;
            }
            let capture = captured.observation.row();
            let capture_type = captured.observation_type;
            let operand_contexts = crate::flow::operand_contexts(
                program,
                &facts.flow.contexts,
                &facts.flow.control,
                &facts.semantic,
                capture,
                capture_type,
                call.entry_constraints,
            );
            if let Some(selected) = &mut selected_contexts {
                selected.retain(|context| operand_contexts.contains(context));
            } else {
                selected_contexts = Some(operand_contexts.clone());
            }
            let epoch = if crate::flow::operand_binds_live_view(program, capture_type) {
                facts.flow.control.operands.span(call.operands)?.last()?
            } else {
                capture
            };
            let first_binding = current.bindings.len();
            *operand = match operand {
                facts::BooleanPredicateOperand::Scalar(scalar) => {
                    let primitive = scalar.primitive_type()?;
                    current_actual(
                        program,
                        facts,
                        state,
                        call,
                        captured,
                        primitive,
                        &operand_contexts,
                        fold_observed_values,
                        &mut current.bindings,
                    )?
                    .into()
                }
                facts::BooleanPredicateOperand::CaseSubject(_) => predicate_subjects::current_case(
                    program,
                    facts,
                    captured,
                    &mut current.bindings,
                )?,
            };
            for binding in &current.bindings[first_binding..] {
                for (previous, previous_epoch) in current.bindings[..first_binding]
                    .iter()
                    .zip(&binding_epochs)
                {
                    if previous.place == binding.place
                        && !crate::flow::operand_observations_agree(
                            program,
                            facts.flow.view(),
                            previous_epoch,
                            epoch,
                            &binding.place,
                        )
                    {
                        return None;
                    }
                }
            }
            binding_epochs.resize(current.bindings.len(), epoch);
            for value in &values {
                captured_predicates::retain_bindings(
                    program,
                    facts,
                    value,
                    epoch,
                    &current.bindings[first_binding..],
                    first_binding,
                    &mut captures,
                );
            }
            Some(())
        })
        .is_none()
        {
            return false;
        }
        for operand in observed_operands {
            if predicate_subjects::retain_owned_cases(
                program,
                facts,
                state,
                operand,
                &mut current,
                &mut binding_epochs,
                &mut captures,
            )
            .is_none()
            {
                return false;
            }
        }
        captured_predicates::retain_alternative_reads(
            program,
            facts,
            call.invocation_storage,
            &current.bindings,
            &alternative_reads,
            &mut captures,
        );
        let selected_contexts = selected_contexts.unwrap_or_else(|| contexts.to_vec());
        let mut retained = boolean_predicates::retained(
            program,
            &facts.operators,
            &facts.semantic,
            &selected_contexts,
            facts::ProgramPoint::Statement {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                statement_index: call.statement_index,
            },
        );
        if let Some(frames) = frames {
            retained.extend(super::prover::call_guarantees::current_predicates(
                program,
                facts,
                state,
                super::prover::call_guarantees::GuaranteeUse::StatementEntry(
                    call.statement_index + 1,
                ),
                &selected_contexts,
                frames,
            ));
        }
        let transported = captured_predicates::transport(
            program,
            facts,
            state,
            &mut current,
            &captures,
            &retained,
            frames,
        );
        if fold_observed_values && transported.proves_required {
            return true;
        }
        retained.extend(transported.predicates);
        if !fold_observed_values {
            return super::calls::selected_guard_predicate(program, facts, state, call, frames)
                .and_then(|guard| boolean_predicates::guarded_requirement(&guard, &current))
                .is_some_and(|required| {
                    boolean_predicates::proves(&retained, &required, |_| None)
                });
        }
        let proves = |required: &boolean_predicates::BoundPredicate| {
            super::prover::call_guarantees::proves_captured_predicates(
                program,
                &facts.semantic,
                &selected_contexts,
                &retained,
                required,
                |place| {
                    scalar_at(
                        program,
                        facts,
                        &selected_contexts,
                        facts::ProgramPoint::Statement {
                            machine_symbol: state.machine_symbol,
                            state_symbol: state.state_symbol,
                            statement_index: call.statement_index,
                        },
                        place,
                    )
                },
            )
        };
        if proves(&current) {
            return true;
        }
        let Some(guard) =
            super::calls::selected_guard_predicate(program, facts, state, call, frames)
        else {
            return false;
        };
        boolean_predicates::guarded_requirement(&guard, &current)
            .is_some_and(|required| proves(&required))
            || proves_bound_predicate_mode(
                program, facts, state, call, contexts, predicate, frames, actual, false,
            )
    }

    /*
        Transport only entry-custodied observations. A callee selector remains a
        formula; it never becomes an unconditional caller cleanup demand.
    */
    pub(crate) fn entry_predicate(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        target: SymbolHandle,
        predicate: CheckedBooleanExpression,
    ) -> Option<CheckedBooleanExpression> {
        invocation_body_parameters(program, call, target)?;
        let predicate = boolean_predicates::bind_entry(program, target, predicate)?;
        let mut expression = predicate.expression.clone();
        boolean_predicates::map_observations(&mut expression, &mut |operand| {
            let observation = predicate.binding(operand)?;
            let actual = body_observation_actual(program, state, call, target, observation)?;
            let primitive = operand.primitive_type()?;
            let captured = captured_value(program, facts, state, call, &actual)?;
            let capture = captured.operand.row();
            let (machine, body) = validation::machine_state_by_symbols(
                program,
                state.machine_symbol,
                state.state_symbol,
            )?;
            if captured.projection.remaining.is_empty()
                && let Some(value) = crate::values::lower_entry_scalar(
                    program,
                    &facts.operators,
                    machine,
                    body,
                    capture.statement_index,
                    captured.projection.expression,
                    primitive,
                    &|place| {
                        crate::flow::operand_entry_origin_is_stable(
                            program,
                            facts.flow.view(),
                            capture,
                            place,
                        )
                        .then_some(())?;
                        entry_parameter_place_before_statement(
                            program,
                            state.machine_symbol,
                            state.state_symbol,
                            capture.statement_index,
                            place,
                        )
                    },
                )
            {
                *operand = value.into();
                return Some(());
            }
            let mut place = place_at(
                program,
                capture.state_symbol,
                capture.statement_index,
                captured.projection.expression,
            )?;
            place.extend_segments(&captured.projection.remaining);
            crate::flow::operand_entry_origin_is_stable(
                program,
                facts.flow.view(),
                capture,
                &place,
            )
            .then_some(())?;
            let (position, path) = entry_parameter_place_before_statement(
                program,
                state.machine_symbol,
                state.state_symbol,
                capture.statement_index,
                &place,
            )?;
            *operand = boolean_predicates::entry_scalar(
                program,
                state.machine_symbol,
                position,
                path,
                primitive,
            )?
            .into();
            Some(())
        })?;
        Some(expression)
    }

    pub(crate) fn operand_erases_membership(
        program: &dyn ::validation::CheckingInput,
        control: &checked_trees::FlowControlFacts,
        operand_handle: arena::Handle<checked_trees::FlowOperandFact>,
        path: &[facts::PlaceSegment],
        domain_symbol: symbols::SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
        active: &mut Vec<arena::Handle<checked_trees::FlowOperandFact>>,
    ) -> bool {
        if !control.operands.is_valid(operand_handle) || active.contains(&operand_handle) {
            return false;
        }
        active.push(operand_handle);
        let operand = control.operands.get(operand_handle);

        if path.is_empty()
            && validation::field_domains::expression_erases_membership(
                program,
                operand.expression,
                domain_symbol,
                semantic_domain,
            )
        {
            active.pop();
            return true;
        }

        if let checked_trees::FlowOperandCompletion::Constructed(children) = operand.completion
            && let Some(children) = control.operand_children.span(children)
        {
            for child in children {
                let child_path = control.operand_child_paths.span_or_empty(child.path);
                if path.starts_with(child_path)
                    && operand_erases_membership(
                        program,
                        control,
                        child.value,
                        &path[child_path.len()..],
                        domain_symbol,
                        semantic_domain,
                        active,
                    )
                {
                    active.pop();
                    return true;
                }
            }
        }

        if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion
            && let Some(rows) = control.operand_alternatives.span(alternatives)
        {
            let any_erased = rows.iter().any(|row| {
                operand_erases_membership(
                    program,
                    control,
                    row.value,
                    path,
                    domain_symbol,
                    semantic_domain,
                    active,
                )
            });
            if any_erased {
                active.pop();
                return true;
            }
        }

        if let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } =
            operand.completion
            && control.copied_values.is_valid(copy)
        {
            let copied = control.copied_values.get(copy);
            let mut combined = control.operand_child_paths.span_or_empty(prefix).to_vec();
            combined.extend_from_slice(path);
            if operand_erases_membership(
                program,
                control,
                copied.source,
                &combined,
                domain_symbol,
                semantic_domain,
                active,
            ) {
                active.pop();
                return true;
            }
        }

        if operand.backing.is_valid() {
            let mut combined = control
                .operand_child_paths
                .span_or_empty(
                    operand
                        .address_projection
                        .map_or_else(Default::default, |projection| projection.segments()),
                )
                .to_vec();
            combined.extend_from_slice(path);
            if operand_erases_membership(
                program,
                control,
                operand.backing,
                &combined,
                domain_symbol,
                semantic_domain,
                active,
            ) {
                active.pop();
                return true;
            }
            let stored = validation::completed_storage::reference_sources_at_path(
                program,
                control,
                operand.backing,
                &combined,
                operand.storage_snapshot,
            );
            if stored_operand_erases_membership(
                program,
                control,
                &stored,
                domain_symbol,
                semantic_domain,
                active,
            ) {
                active.pop();
                return true;
            }
        } else if !path.is_empty() {
            let stored = validation::completed_storage::reference_sources_at_path(
                program,
                control,
                operand_handle,
                path,
                operand.storage_snapshot,
            );
            if stored_operand_erases_membership(
                program,
                control,
                &stored,
                domain_symbol,
                semantic_domain,
                active,
            ) {
                active.pop();
                return true;
            }
        }

        if path.is_empty() && control.operand_retentions.is_valid(operand.retention) {
            let profile = control.operand_retentions.get(operand.retention);
            if let Some(suppliers) = control.operand_retained_suppliers.span(profile.suppliers) {
                for supplier in suppliers {
                    let supp_path = control
                        .operand_retained_paths
                        .span_or_empty(supplier.projection);
                    if operand_erases_membership(
                        program,
                        control,
                        supplier.operand,
                        supp_path,
                        domain_symbol,
                        semantic_domain,
                        active,
                    ) {
                        active.pop();
                        return true;
                    }
                }
            }
        }

        if operand.value_source.is_valid()
            && operand_erases_membership(
                program,
                control,
                operand.value_source,
                path,
                domain_symbol,
                semantic_domain,
                active,
            )
        {
            active.pop();
            return true;
        }

        active.pop();
        false
    }

    fn stored_operand_erases_membership(
        program: &dyn ::validation::CheckingInput,
        control: &checked_trees::FlowControlFacts,
        stored: &validation::completed_storage::StoredOperand,
        domain_symbol: symbols::SymbolHandle,
        semantic_domain: language_semantics::SemanticDomainId,
        active: &mut Vec<arena::Handle<checked_trees::FlowOperandFact>>,
    ) -> bool {
        match stored {
            validation::completed_storage::StoredOperand::Alternatives(alts) => {
                alts.iter().any(|alt| {
                    stored_operand_erases_membership(
                        program,
                        control,
                        alt,
                        domain_symbol,
                        semantic_domain,
                        active,
                    )
                })
            }
            validation::completed_storage::StoredOperand::Value {
                capture,
                projection,
                ..
            } => operand_erases_membership(
                program,
                control,
                *capture,
                projection,
                domain_symbol,
                semantic_domain,
                active,
            ),
            validation::completed_storage::StoredOperand::Incoming(_)
            | validation::completed_storage::StoredOperand::Unavailable => false,
        }
    }
}
mod capture_coordinates {
    /*
        Stored source coordinates depend on reference bindings independently of payload.

        A projected scalar is read after its selector, but the collection descriptor
        was evaluated before that selector. Sharing its canonical slot therefore
        requires the descriptor prefixes to survive; a write to payload contents
        alone does not redirect the completed read. Reference construction and
        copied-value alias publication use this same typed prefix discovery.
    */
    use crate::published::places::place_at;
    use checked_trees::FlowOperandFact;
    use facts::PlaceSegment;
    use validation::expression_traversal::collect_expression_tree;
    use validation::{
        CanonicalPlace, CheckingInput, canonical_place_type_reference, find_state,
        has_builtin_subslice_meaning,
    };

    pub(super) fn borrowed_bindings(
        program: &dyn CheckingInput,
        capture: &FlowOperandFact,
        remaining: &[PlaceSegment],
    ) -> Option<Vec<CanonicalPlace>> {
        let mut sources = Vec::new();
        collect_expression_tree(program, capture.expression, &mut sources);
        let (machine, body) = find_state(program, capture.state_symbol)?;
        let produced_windows: Vec<_> = sources
            .iter()
            .filter(|source| has_builtin_subslice_meaning(program, machine, Some(body), **source))
            .filter_map(|source| {
                place_at(
                    program,
                    capture.state_symbol,
                    capture.statement_index,
                    *source,
                )
            })
            .collect();
        let mut bindings = Vec::new();
        for source in sources {
            let Some(mut place) = place_at(
                program,
                capture.state_symbol,
                capture.statement_index,
                source,
            ) else {
                continue;
            };
            if !place
                .root
                .as_symbol()
                .is_some_and(|symbol| symbol.is_valid())
            {
                continue;
            }
            if source == capture.expression {
                place.extend_segments(remaining);
            }
            if produced_windows
                .iter()
                .any(|window| crate::flow::canonical_places_agree(program, window, &place))
            {
                continue;
            }
            let reference = canonical_place_type_reference(
                program,
                capture.state_symbol,
                capture.statement_index,
                &place,
            )?;
            if crate::flow::operand_binds_live_view(program, reference)
                && !bindings.contains(&place)
            {
                bindings.push(place);
            }
        }
        Some(bindings)
    }
}
mod content_preservation {
    /*
        Congruence of content projections across a nonmutating activation.
    */
    use crate::flow::exit_parameter_origins;
    use checked_trees::{CheckFacts, FlowStateFact};
    use language_semantics::content::{
        ContentConservationOwnerKind, ContentConservationPlan, ContentConservationTerm,
        ContentPlaceRoot, ContentPlaceVersion,
    };

    mod inherited {
        /*
            Rejoin a requirement's normalized content equation to its satisfying body.

            The retained conformance selects the complete applied signature. Positional
            formal correspondence changes subjects, not projection or operator meaning.
            The resulting equation still needs proof at this exact completed return;
            inheritance supplies neither the theorem nor custody of its subjects.
        */
        use checked_trees::{
            CheckFacts, ContractProofFactKind, ContractProofFactOwner, FlowExitFact,
        };
        use language_semantics::content::{
            ContentConservationEquation, ContentConservationOwnerKind, ContentConservationPlan,
            ContentConservationTerm, ContentPlaceRoot, conservation_report_fingerprint,
        };
        use typed_trees::signature::StateParameter;
        use typed_trees::type_identity::TypeIdentityRequest;
        use typed_trees::types::TypeReferenceHandle;

        pub(super) fn proves_exit(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            exit: &FlowExitFact,
            fact: &facts::Fact,
            sources: &[validation::ContentConservationSourcePlan],
            available: &[ContentConservationPlan],
        ) -> bool {
            let facts::FactPayload::ContractBooleanExpression {
                fact: source_fact,
                expression,
                ..
            } = fact.payload
            else {
                return false;
            };
            if program.proof_facts.get(source_fact).as_expression() != Some(&expression) {
                return false;
            }
            let Some(machine) = program.machine_by_symbol(exit.machine_symbol) else {
                return false;
            };
            let Some(state) = program.state_in_machine(machine, exit.state_symbol) else {
                return false;
            };
            if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || !machine.body_is_present
                || !facts.flow.control.operands.is_valid(exit.returned_value)
            {
                return false;
            }
            let mut mapped: Option<ContentConservationPlan> = None;
            for (_, contract) in facts.proof.contract_facts.iter().filter(|(_, contract)| {
                contract.kind == ContractProofFactKind::Ensures
                    && contract.fact == source_fact
                    && contract.inherited_scope.is_some()
                    && matches!(contract.owner, ContractProofFactOwner::MachineState {
                        machine_symbol, ..
                    } if machine_symbol == machine.symbol)
            }) {
                let Some((declaring_trait, requirement, satisfier, scope)) =
                    crate::published::semantic::facts::inherited_contract_scope_parts(
                        program,
                        &facts.proof,
                        contract,
                    )
                else {
                    return false;
                };
                if satisfier.symbol != state.symbol
                    || program
                        .machine_states(machine)
                        .first()
                        .map(|state| state.symbol)
                        != Some(state.symbol)
                {
                    return false;
                }
                let Some(applied) =
                    crate::published::semantic::facts::applied_inherited_requirement(
                        program,
                        requirement,
                        scope,
                    )
                else {
                    return false;
                };
                let Some(parameters) = validation::inherited_satisfier_parameters(
                    program,
                    declaring_trait,
                    requirement,
                    satisfier,
                ) else {
                    return false;
                };
                let declared = program.state_signature_parameters(requirement);
                let selected = program.state_signature_parameters(applied);
                if !same_type(program, requirement.return_type, applied.return_type)
                    || !same_type(program, applied.return_type, satisfier.return_type)
                    || declared.len() != selected.len()
                    || selected.len() != parameters.len()
                    || !declared.iter().zip(selected).zip(parameters).all(
                        |((declared, selected), actual)| {
                            same_type(program, declared.type_reference, selected.type_reference)
                                && same_type(
                                    program,
                                    selected.type_reference,
                                    actual.type_reference,
                                )
                        },
                    )
                {
                    return false;
                }
                let mut matching = sources.iter().filter(|source| {
                    source.source_expression == expression
                        && source.plan.owner_kind == ContentConservationOwnerKind::TraitRequirement
                        && source.plan.owner == scope.declaring_trait
                        && source.plan.callable == scope.requirement
                });
                let Some(source) = matching.next() else {
                    return false;
                };
                if matching.next().is_some() {
                    return false;
                }
                let mut plan = source.plan.clone();
                let mut left = plan.equation.left().clone();
                let mut right = plan.equation.right().clone();
                for term in [&mut left, &mut right] {
                    if rebind(term, declared, parameters, program.state_parameters(state)).is_none()
                    {
                        return false;
                    }
                }
                plan.owner_kind = ContentConservationOwnerKind::Machine;
                plan.owner = machine.symbol;
                plan.callable = state.symbol;
                plan.equation = ContentConservationEquation::new(left, right);
                plan.report_fingerprint =
                    conservation_report_fingerprint(&plan.algebra, &plan.equation);
                if mapped.as_ref().is_some_and(|known| known != &plan) {
                    return false;
                }
                mapped = Some(plan);
            }
            let Some(plan) = mapped else {
                return false;
            };
            let completed = facts.flow.control.operands.get(exit.returned_value);
            completed.state_symbol == state.symbol
                && completed.statement_index == exit.statement_index
                && validation::completed_value_proves_content_conservation(
                    program,
                    machine,
                    state,
                    completed.statement_index,
                    completed.expression,
                    completed.type_reference,
                    &plan,
                    &facts.qualifications.content.plans,
                    available,
                )
        }

        fn same_type(
            program: &dyn validation::CheckingInput,
            left: TypeReferenceHandle,
            right: TypeReferenceHandle,
        ) -> bool {
            left.is_valid()
                && right.is_valid()
                && program.type_identity(TypeIdentityRequest::ordinary(left))
                    == program.type_identity(TypeIdentityRequest::ordinary(right))
        }

        fn rebind(
            term: &mut ContentConservationTerm,
            declared: &[StateParameter],
            actual: &[StateParameter],
            current: &[StateParameter],
        ) -> Option<()> {
            match term {
                ContentConservationTerm::Separate(children) => {
                    for child in children {
                        rebind(child, declared, actual, current)?;
                    }
                }
                ContentConservationTerm::Projection { subject, .. } => {
                    if let ContentPlaceRoot::Parameter {
                        position,
                        symbol,
                        name,
                    } = &mut subject.root
                    {
                        let ordinal = usize::try_from(*position).ok()?;
                        if declared.get(ordinal)?.symbol != *symbol {
                            return None;
                        }
                        let target = actual.get(ordinal)?.symbol;
                        let (ordinal, parameter) = current
                            .iter()
                            .enumerate()
                            .find(|(_, parameter)| parameter.symbol == target)?;
                        *position = u32::try_from(ordinal).ok()?;
                        *symbol = target;
                        *name = parameter.name.as_str().to_owned();
                    }
                }
            }
            Some(())
        }
    }

    pub(super) fn proves_exit(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        exit: &checked_trees::FlowExitFact,
        fact: &facts::Fact,
        content_plans: &[validation::ContentConservationSourcePlan],
        available: &[ContentConservationPlan],
    ) -> bool {
        if inherited::proves_exit(program, facts, exit, fact, content_plans, available) {
            return true;
        }
        let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
            return false;
        };
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            return false;
        };
        content_plans
            .iter()
            .filter(|source| {
                source.source_expression == expression && source.plan.owner == machine.symbol
            })
            .any(|source| unchanged_projection(program, facts, machine, &source.plan))
    }

    fn unchanged_projection(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        plan: &ContentConservationPlan,
    ) -> bool {
        let Some(mutation) = facts.mutation.for_machine(machine.symbol) else {
            return false;
        };
        if !machine.body_is_present
            || program.machine_states(machine).is_empty()
            || program.machine_states(machine).iter().any(|state| {
                !mutation.state_write_frames.iter().any(|frame| {
                    frame.state == state.symbol
                        && frame.frame.is_complete()
                        && frame.frame.paths().is_empty()
                })
            })
        {
            return false;
        }
        let ContentConservationTerm::Projection {
            domain: left_domain,
            semantic_domain: left_semantic_domain,
            projection_machine: left_projection,
            subject: left,
            ..
        } = plan.equation.left()
        else {
            return false;
        };
        let ContentConservationTerm::Projection {
            domain: right_domain,
            semantic_domain: right_semantic_domain,
            projection_machine: right_projection,
            subject: right,
            ..
        } = plan.equation.right()
        else {
            return false;
        };
        left_domain.is_valid()
            && left_projection.is_valid()
            && left_domain == right_domain
            && left_semantic_domain == right_semantic_domain
            && left_projection == right_projection
            && matches!((&left.root, &right.root),
                (ContentPlaceRoot::Parameter { symbol: left, .. },
                 ContentPlaceRoot::Parameter { symbol: right, .. })
                 if left.is_valid() && left == right)
            && left.segments == right.segments
            && matches!(
                (left.version, right.version),
                (ContentPlaceVersion::Entry, ContentPlaceVersion::Current)
                    | (ContentPlaceVersion::Current, ContentPlaceVersion::Entry)
            )
    }

    /*
        Publish read-only congruence only when the actual complete exit roster and
        each exact incoming-origin obligation check. An exitless callable supplies
        no independently observed theorem, while its ordinary partial contract is
        left intact.
    */
    pub(super) fn proves_readonly_source(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        source: &validation::ContentConservationSourcePlan,
    ) -> bool {
        if source.plan.owner_kind != ContentConservationOwnerKind::Machine {
            return false;
        }
        let Some(machine) = program.machine_by_symbol(source.plan.owner) else {
            return false;
        };
        if program
            .machine_states(machine)
            .first()
            .map(|state| state.symbol)
            != Some(source.plan.callable)
            || !unchanged_projection(program, facts, machine, &source.plan)
        {
            return false;
        }
        let mut observed = false;
        for state in program.machine_states(machine) {
            let Some(flow) = facts.flow.control.state(machine.symbol, state.symbol) else {
                return false;
            };
            for exit in facts.flow.control.exits.span_or_empty(flow.exits) {
                observed = true;
                let mut matched = false;
                for context in facts
                    .flow
                    .semantic_constraint_contexts(exit.ensures_constraints)
                {
                    for fact in facts
                        .semantic
                        .context_view(facts.semantic.contexts.get(context))
                        .facts()
                    {
                        let facts::FactPayload::ContractBooleanExpression {
                            fact: contract,
                            expression,
                            ..
                        } = fact.payload
                        else {
                            continue;
                        };
                        if expression != source.source_expression {
                            continue;
                        }
                        matched = true;
                        if exit_parameter_origins(facts.view(), exit)
                            .iter()
                            .any(|origin| {
                                origin.contract == contract
                                    && !origin.state_parameter.is_valid()
                                    && !origin.retained_referent_type.is_valid()
                            })
                        {
                            return false;
                        }
                    }
                }
                if !matched {
                    return false;
                }
            }
        }
        observed
    }
}
mod current_values {
    /*
        Contract call and exit obligations under the shared current-value transfer.
        The flow owner selects the exact operation and invocation origins; the
        arithmetic roster supplies only the values observed at that program point.
    */
    use crate::checks::current_scalar_values::CurrentScalarValues;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::{StatementNode, TransitionTargetHandle};

    /* One exit frontier is prepared once, including an explicitly unavailable one. */
    pub(crate) type ExitValueCache<'program> = Option<
        Option<(
            &'program typed_trees::machine::Machine,
            CurrentScalarValues<'program>,
            Option<ExpressionHandle>,
        )>,
    >;

    /*
        A Boolean contract's atoms consume the same completed frontier as its whole
        proposition. Case and scalar conjuncts keep their independent proof owners;
        no conjunction is assumed merely to form a result-field observation.
    */
    pub(crate) fn proves_exit_expression<'program>(
        program: &'program dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        exit: &checked_trees::FlowExitFact,
        contract: arena::Handle<typed_trees::domain::ProofFact>,
        expression: ExpressionHandle,
        frames: Option<&validation::CallFrameResolver<'program>>,
        cached_exit_values: &mut ExitValueCache<'program>,
    ) -> bool {
        let Some((machine, values, returned)) = cached_exit_values
            .get_or_insert_with(|| {
                let (machine, state) =
                    program.state_flow_site(exit.machine_symbol, exit.state_symbol)?;
                let returned = match program
                    .statement_table
                    .statement_at(state, exit.statement_index)
                {
                    Some(StatementNode::Expression(expression))
                        if !exit.transition_target.is_valid() =>
                    {
                        Some(*expression)
                    }
                    Some(StatementNode::Transition(_)) if exit.transition_target.is_valid() => {
                        match program
                            .statement_table
                            .transition_target(exit.transition_target)
                        {
                            typed_trees::statement::TransitionTargetNode::Value(expression) => {
                                Some(*expression)
                            }
                            typed_trees::statement::TransitionTargetNode::Terminal => None,
                            typed_trees::statement::TransitionTargetNode::Named { .. } => {
                                return None;
                            }
                        }
                    }
                    None if !exit.transition_target.is_valid()
                        && exit.statement_index
                            == crate::flow::state_statements(program, state.statement_nodes)
                                .len() =>
                    {
                        None
                    }
                    _ => return None,
                };
                let mut values = CurrentScalarValues::at(
                    program,
                    machine,
                    state,
                    exit.statement_index,
                    exit.transition_target,
                    frames,
                )?;
                if let Some(returned) = returned {
                    values.retire_operand_effects(returned);
                    values.prepare_exit_return(returned);
                }
                Some((machine, values, returned))
            })
            .as_ref()
        else {
            return false;
        };
        let origins = crate::flow::exit_parameter_origins(facts.view(), exit)
            .iter()
            .filter(|origin| origin.contract == contract)
            .map(|origin| (origin.entry_parameter, origin.state_parameter));
        let Some(goal) = values.postcondition(expression, *returned, origins) else {
            return false;
        };
        values.proves(program, machine, &goal)
    }

    /*
        Bind the exact call's formal roster simultaneously to current actuals.
        The same prefix transfer handles an ordinary call and a named state edge.
    */
    pub(crate) fn proves_call(
        program: &dyn ::validation::CheckingInput,
        state_flow: &checked_trees::FlowStateFact,
        call: &checked_trees::FlowCallFact,
        expression: ExpressionHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
        completed: &mut crate::checks::current_scalar_values::CompletedCallGuarantees,
    ) -> bool {
        prepare_call(program, state_flow, call, frames, completed)
            .is_some_and(|values| proves_prepared_call(&values, state_flow, call, expression))
    }

    // A contract's clauses share the values at this exact completed invocation.
    // Retire overwritten captures once, before any clause asks for a proof.
    pub(crate) fn prepare_call<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state_flow: &checked_trees::FlowStateFact,
        call: &checked_trees::FlowCallFact,
        frames: Option<&validation::CallFrameResolver<'_>>,
        completed: &mut crate::checks::current_scalar_values::CompletedCallGuarantees,
    ) -> Option<CurrentScalarValues<'program>> {
        let (machine, state) =
            program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)?;
        let site = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call.statement_index,
            call.call_ordinal,
        )?;
        let parameters = validation::call_target_parameters(program, call.target_symbol)?;
        if validation::call_site_argument_expressions(program, &site).len() != parameters.len() {
            return None;
        }
        let target = validation::transition_call_target(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        )
        .filter(|target| target.is_valid() && site.completes_transition(program, *target))
        .unwrap_or_else(TransitionTargetHandle::invalid);
        let mut values = CurrentScalarValues::at_completing(
            program,
            machine,
            state,
            call.statement_index,
            target,
            frames,
            &[],
            completed,
        )?;
        let at = program.storage_graph().and_then(|graph| {
            validation::completed_storage::versions::invocation_snapshot(
                program,
                graph,
                state.symbol,
                call.statement_index,
                call.call_ordinal,
            )
        });
        values.retire_unpreserved(at);
        Some(values)
    }

    pub(crate) fn proves_prepared_call(
        values: &CurrentScalarValues<'_>,
        state_flow: &checked_trees::FlowStateFact,
        call: &checked_trees::FlowCallFact,
        expression: ExpressionHandle,
    ) -> bool {
        if values.machine().symbol != state_flow.machine_symbol
            || values.state().symbol != state_flow.state_symbol
            || values.statement_index() != call.statement_index
        {
            return false;
        }
        let program = values.program();
        let Some(site) = validation::find_call_site(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            call.statement_index,
            call.call_ordinal,
        ) else {
            return false;
        };
        let Some(parameters) = validation::call_target_parameters(program, call.target_symbol)
        else {
            return false;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        if arguments.len() != parameters.len() {
            return false;
        }
        let Some((callee, callee_state)) = validation::find_state(program, call.target_symbol)
            .or_else(|| validation::find_machine_head(program, call.target_symbol))
        else {
            return false;
        };
        values.proves_parameter_substitution(
            callee,
            callee_state,
            expression,
            parameters,
            arguments,
            site,
        )
    }
}
mod entailment {
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;

    mod case_preservation {
        /*
            Eligibility for tag-only proof without completed value observations.

            The runtime case judge retains tags for entry parameters, saved call results
            and branch subjects. Establish preservation for the whole reachable body,
            rather than checking only the caller's outward frame: writes to an owned
            local can disappear from that frame while still invalidating its saved tag.
            Complete frames exclude unknown effects; checked stored-value types
            distinguish scalar mutation (including atomic events) from replacement of
            a case-bearing value or reference binding. This deliberately proves no
            scalar equations. A replacement instead needs the ordinary completed-value
            prefix proof: exact captures and write frames, never replay of changed
            storage. Failure here selects that owner rather than refusing its evidence.
        */
        use checked_trees::CheckFacts;
        use symbols::SymbolHandle;
        use typed_trees::expression::{BinaryOperator, ExpressionHandle};
        use typed_trees::machine::Machine;

        pub(super) fn is_case_requirement(
            program: &dyn validation::CheckingInput,
            machine: &Machine,
            state: &typed_trees::state::State,
            expression: ExpressionHandle,
        ) -> bool {
            let Some(binary) = program.expression_table.binary_node(expression) else {
                return false;
            };
            if binary.operator == BinaryOperator::And {
                return is_case_requirement(program, machine, state, binary.left)
                    && is_case_requirement(program, machine, state, binary.right);
            }
            validation::has_exact_case_membership_meaning(
                program,
                machine,
                Some(state),
                expression,
                binary,
            )
        }

        pub(super) fn body_preserves_case_tags(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            machine: &Machine,
        ) -> bool {
            let mut pending = vec![machine.symbol];
            let mut visited: Vec<SymbolHandle> = Vec::new();
            while let Some(symbol) = pending.pop() {
                if visited.contains(&symbol) {
                    continue;
                }
                visited.push(symbol);
                let Some(machine) = program.machine_by_symbol(symbol) else {
                    return false;
                };
                if !machine.supplies_checked_body() {
                    return false;
                }
                let Some(mutation) = facts.mutation.for_machine(symbol) else {
                    return false;
                };
                if program.machine_states(machine).is_empty() {
                    return false;
                }
                for state in program.machine_states(machine) {
                    if !mutation
                        .state_write_frames
                        .iter()
                        .any(|frame| frame.state == state.symbol && frame.frame.is_complete())
                    {
                        return false;
                    }
                    for statement in program.statement_table.statements(state.statement_nodes) {
                        if let Some(assignment) = statement.as_assignment() {
                            let scalar_destination = validation::assignment_stored_value_type(
                                program, machine, state, assignment,
                            )
                            .and_then(|reference| program.primitive_type_reference(reference))
                            .is_some();
                            if !scalar_destination {
                                return false;
                            }
                        }
                    }
                }
                for (_, state) in facts
                    .flow
                    .control
                    .machine_states(symbol)
                    .into_iter()
                    .map(|state| ((), state))
                {
                    for call in facts.flow.control.calls.span_or_empty(state.calls) {
                        let callee = program
                            .machine_by_symbol(call.target_symbol)
                            .or_else(|| program.machine_holding_state(call.target_symbol));
                        let Some(callee) = callee else {
                            return false;
                        };
                        pending.push(callee.symbol);
                    }
                }
            }
            true
        }
    }

    /*
        Transient, program-local results of positive entailment judgments. The
        runtime place prover need not duplicate structural or integer proof rules,
        but absence of a validation error is not evidence for any postcondition.
    */
    pub(in crate::checks) struct ProvenExitExpressions<'program, 'frames> {
        program: &'program dyn ::validation::CheckingInput,
        classification: &'program typed_trees::proof_only::ProofOnlyClassification,
        resolver: Option<&'frames validation::CallFrameResolver<'program>>,
        /* Postconditions the one judgment proved on this program, by machine. */
        validated: &'frames [(SymbolHandle, validation::ProvenContractExpressions)],
        machines: Vec<MachineEntailmentOutcome>,
        content_sources: std::sync::Arc<Vec<language_semantics::content::ContentConservationPlan>>,
    }

    pub(in crate::checks) struct MachineEntailmentOutcome {
        machine: SymbolHandle,
        pub(in crate::checks) expressions: Vec<ExpressionHandle>,
        entry_premises_preserved: bool,
    }

    impl<'program, 'frames> ProvenExitExpressions<'program, 'frames> {
        /*
            The check pass owns one resolver for the immutable program window;
            `resolver` borrows it. `None` disables the isolated-proof-value path
            exactly as an unconstructable resolver did before.
        */
        pub(in crate::checks) fn new(
            program: &'program dyn ::validation::CheckingInput,
            classification: &'program typed_trees::proof_only::ProofOnlyClassification,
            resolver: Option<&'frames validation::CallFrameResolver<'program>>,
            validated: &'frames [(SymbolHandle, validation::ProvenContractExpressions)],
        ) -> Self {
            Self {
                program,
                classification,
                resolver,
                validated,
                machines: Vec::new(),
                content_sources: std::sync::Arc::new(Vec::new()),
            }
        }

        pub(in crate::checks) fn conditional_content(
            &self,
            plan: &language_semantics::content::ContentConservationPlan,
            expression: ExpressionHandle,
        ) -> Option<&validation::ConditionalContentGuarantee> {
            self.validated
                .iter()
                .find(|(owner, _)| *owner == plan.owner)?
                .1
                .conditional_content
                .iter()
                .find(|proof| proof.expression == expression && proof.plan == *plan)
        }

        pub(in crate::checks) fn publish_content_sources(
            &mut self,
            sources: Vec<language_semantics::content::ContentConservationPlan>,
        ) {
            self.content_sources = std::sync::Arc::new(sources);
        }

        pub(in crate::checks) fn content_sources(
            &self,
        ) -> std::sync::Arc<Vec<language_semantics::content::ContentConservationPlan>> {
            std::sync::Arc::clone(&self.content_sources)
        }

        /*
            A nonmutating entry/current congruence is an independent body proof,
            including Unit-returning bodies. Do not confuse absence of an observed
            return with that proof: the existing exit/origin owner must cover all exits.
        */
        pub(in crate::checks) fn record_preserved_content_sources(
            &mut self,
            facts: &checked_trees::CheckFacts,
            sources: &[validation::ContentConservationSourcePlan],
        ) {
            for source in sources {
                if super::content_preservation::proves_readonly_source(self.program, facts, source)
                {
                    self.record_proved_normal_return(
                        facts,
                        source.plan.owner,
                        source.source_expression,
                    );
                }
            }
        }

        /*
            Add an independently established all-normal-return judgment. The caller
            must cover the actual entry's complete outgoing graph; a theorem about
            one local exit cannot bypass another state's missing origin evidence.
        */
        pub(in crate::checks) fn record_proved_normal_return(
            &mut self,
            facts: &checked_trees::CheckFacts,
            machine: SymbolHandle,
            expression: ExpressionHandle,
        ) {
            self.for_machine(facts, machine);
            if let Some(outcome) = self
                .machines
                .iter_mut()
                .find(|outcome| outcome.machine == machine)
                && !outcome.expressions.contains(&expression)
            {
                outcome.expressions.push(expression);
                let inherited = validation::matched_machine_law_guarantees(self.program, machine)
                    .into_iter()
                    .filter(|matching| {
                        matching.machine == machine
                            && matching
                                .source_expressions
                                .iter()
                                .all(|source| outcome.expressions.contains(source))
                    })
                    .map(|matching| matching.expression)
                    .collect::<Vec<_>>();
                for expression in inherited {
                    crate::shared::collections::push_unique(&mut outcome.expressions, expression);
                }
            }
        }

        /*
            The one judgment over a checked body is validation's retained row;
            this read is the prover's decision point — nothing rejudges it.
        */
        pub(in crate::checks) fn for_machine(
            &mut self,
            facts: &checked_trees::CheckFacts,
            machine_symbol: SymbolHandle,
        ) -> &MachineEntailmentOutcome {
            let position = self
                .machines
                .iter()
                .position(|outcome| outcome.machine == machine_symbol)
                .unwrap_or_else(|| {
                    let entry_premises_preserved = entry_premises_are_preserved(
                        self.program,
                        facts,
                        machine_symbol,
                        self.classification,
                        self.resolver,
                    );
                    let mut expressions = self
                        .validated
                        .iter()
                        .find(|(symbol, _)| *symbol == machine_symbol)
                        .map(|(_, proven)| {
                            let mut expressions = proven.normal_returns.clone();
                            if entry_premises_preserved {
                                expressions.extend_from_slice(&proven.entry);
                            }
                            expressions
                        })
                        .unwrap_or_default();
                    if !expressions.is_empty() {
                        let inherited = validation::matched_machine_law_guarantees(
                            self.program,
                            machine_symbol,
                        )
                        .into_iter()
                        .filter(|matching| {
                            matching.machine == machine_symbol
                                && matching
                                    .source_expressions
                                    .iter()
                                    .all(|source| expressions.contains(source))
                        })
                        .map(|matching| matching.expression)
                        .collect::<Vec<_>>();
                        expressions.extend(inherited);
                    }
                    self.machines.push(MachineEntailmentOutcome {
                        machine: machine_symbol,
                        expressions,
                        entry_premises_preserved,
                    });
                    self.machines.len() - 1
                });
            &self.machines[position]
        }
    }

    pub(super) fn transparent_proposition_proves_exit(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        outcome: &MachineEntailmentOutcome,
        state_flow: &checked_trees::FlowStateFact,
        fact: &facts::Fact,
    ) -> bool {
        if outcome.machine != state_flow.machine_symbol || !outcome.entry_premises_preserved {
            return false;
        }
        let facts::FactPayload::ContractPropositionApplication { fact: source, .. } = fact.payload
        else {
            return false;
        };
        let Some(application) = program.proof_facts.get(source).as_proposition() else {
            return false;
        };
        let Some(machine) =
            program
                .machine_by_symbol(state_flow.machine_symbol)
                .filter(|machine| {
                    machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
                })
        else {
            return false;
        };
        let Some(state) = validation::find_state_in_machine(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        ) else {
            return false;
        };
        let inherited = facts
            .proof
            .contract_facts
            .iter()
            .find(|(_, contract)| {
                contract.fact == source
                    && contract.owner
                        == checked_trees::ContractProofFactOwner::MachineState {
                            machine_symbol: state_flow.machine_symbol,
                            state_symbol: state_flow.state_symbol,
                        }
            })
            .and_then(|(_, contract)| {
                crate::published::semantic::facts::inherited_contract_scope_parts(
                    program,
                    &facts.proof,
                    contract,
                )
            })
            .map(|(trait_definition, requirement, _, scope)| {
                validation::InheritedRequirementEdge {
                    trait_definition,
                    requirement,
                    trait_arguments: &scope.trait_arguments,
                }
            });
        validation::transparent_proposition_application_entailed(
            program,
            machine,
            state,
            application,
            inherited,
        )
    }

    /*
        Entry-context proofs do not establish a mutable subject's exit revision.
        Complete empty checked frames exclude transitive writes. Direct owned
        parameter assignments must also be absent: they are not outward effects.
    */
    fn entry_premises_are_preserved(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine_symbol: SymbolHandle,
        classification: &typed_trees::proof_only::ProofOnlyClassification,
        resolver: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(machine) = program.machine_by_symbol(machine_symbol) else {
            return false;
        };
        if resolver.is_some_and(|resolver| {
            isolated_proof_values(program, facts, machine, classification, resolver)
        }) {
            return true;
        }
        let Some(mutation) = facts.mutation.for_machine(machine_symbol) else {
            return false;
        };
        !program.machine_states(machine).is_empty()
            && program.machine_states(machine).iter().all(|state| {
                !crate::flow::state_statements(program, state.statement_nodes)
                    .iter()
                    .any(|statement| {
                        matches!(
                            statement,
                            typed_trees::statement::StatementNode::Assignment(_)
                        )
                    })
                    && mutation.state_write_frames.iter().any(|frame| {
                        frame.state == state.symbol
                            && frame.frame.is_complete()
                            && frame.frame.paths().is_empty()
                    })
            })
    }

    /*
        Structural induction and call formation share one proof language, but not
        one observation point. Ask for the exact call entry, never reuse an exit
        judgment or import an unproved callee guarantee.
    */
    pub(super) fn structural_call_requirement(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        state_flow: &checked_trees::FlowStateFact,
        call_flow: &checked_trees::FlowCallFact,
        expression: ExpressionHandle,
        source_contract: Option<arena::Handle<checked_trees::ContractProofFact>>,
        resolver: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let classification = validation::proof_only_classification(program);
        let Some((machine, state)) = validation::machine_state_by_symbols(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
        ) else {
            return false;
        };
        let Some(call_site) = validation::find_call_site(
            program,
            machine.symbol,
            state.symbol,
            call_flow.statement_index,
            call_flow.call_ordinal,
        ) else {
            return false;
        };
        let callee = validation::call_target_state(program, call_flow.target_symbol);
        let callee_machine = callee.and_then(|callee| program.machine_holding_state(callee.symbol));
        let runtime_values = !classification.is_proof_machine(program, machine)
            || callee_machine
                .is_none_or(|callee| !classification.is_proof_machine(program, callee));
        let case_facts_only = runtime_values
            && callee.zip(callee_machine).is_some_and(|(state, machine)| {
                case_preservation::is_case_requirement(program, machine, state, expression)
            })
            && case_preservation::body_preserves_case_tags(program, facts, machine);
        let preserved = if runtime_values {
            true
        } else {
            entry_premises_are_preserved(program, facts, machine.symbol, &classification, resolver)
        };
        if !preserved {
            return false;
        }
        if !matches!(call_site, validation::CallSite::TransitionNamed { .. }) {
            let inherited = source_contract
                .filter(|handle| {
                    facts
                        .proof
                        .contract_facts
                        .get(*handle)
                        .inherited_scope
                        .is_some()
                })
                .map(|handle| (&facts.proof, handle));
            return validation::selected_value_call_requirement_entailed(
                program,
                machine,
                state,
                call_flow.statement_index,
                call_site,
                expression,
                case_facts_only,
                inherited,
            );
        }
        let Some(callee) = callee else {
            return false;
        };
        let arguments = validation::call_site_argument_expressions(program, &call_site);
        let in_transition_target = validation::transition_call_target(
            program,
            machine,
            state,
            call_flow.statement_index,
            call_flow.call_ordinal,
        )
        .is_some_and(|target| target.is_valid());
        validation::structural_call_requirement_entailed(
            program,
            machine,
            state.symbol,
            call_flow.statement_index,
            in_transition_target,
            callee,
            arguments,
            expression,
            case_facts_only,
        )
    }

    fn isolated_proof_values(
        program: &dyn ::validation::CheckingInput,
        facts: &checked_trees::CheckFacts,
        machine: &typed_trees::machine::Machine,
        classification: &typed_trees::proof_only::ProofOnlyClassification,
        resolver: &validation::CallFrameResolver<'_>,
    ) -> bool {
        if !classification.is_proof_machine(program, machine) {
            return false;
        }
        let isolated_parameters = |state: &typed_trees::state::State| {
            program.state_parameters(state).iter().all(|parameter| {
                !parameter.is_mutable
                    && resolver.proof_value_is_caller_isolated(parameter.type_reference)
            })
        };
        program.machine_states(machine).iter().all(|state| {
            isolated_parameters(state)
                && crate::flow::state_statements(program, state.statement_nodes)
                    .iter()
                    .all(|statement| match statement {
                        typed_trees::statement::StatementNode::Assignment(_) => false,
                        typed_trees::statement::StatementNode::LocalData(local) => {
                            if validation::is_arm_pattern_marker(statement) {
                                validation::declared_place_type_raw(
                                    program,
                                    machine,
                                    Some(state),
                                    local.initial_value,
                                )
                                .is_some_and(|reference| {
                                    resolver.proof_value_is_caller_isolated(reference)
                                })
                            } else {
                                resolver.proof_value_is_caller_isolated(local.type_reference)
                            }
                        }
                        _ => true,
                    })
        }) && facts
            .flow
            .control
            .machine_states(machine.symbol)
            .into_iter()
            .all(|state| {
                crate::flow::call_flows(&facts.flow.control, state.calls)
                    .iter()
                    .all(|call| {
                        program
                            .state_by_symbol(call.target_symbol)
                            .map(isolated_parameters)
                            .or_else(|| {
                                program
                                    .machine_parameter_signature_in(machine, call.target_symbol)
                                    .map(|signature| {
                                        program.state_signature_parameters(signature).iter().all(
                                            |parameter| {
                                                !parameter.is_mutable
                                                    && resolver.proof_value_is_caller_isolated(
                                                        parameter.type_reference,
                                                    )
                                            },
                                        )
                                    })
                            })
                            .unwrap_or(false)
                    })
            })
    }
}
pub(super) use entailment::ProvenExitExpressions;
mod float_predicate_domains {
    /*
        Predicate domains on `f32` and `f64`.

        Integer arithmetic implication does not apply: a float comparison is an
        IEEE observation, and subtraction rounds. Two observations are still
        enough to establish a predicate-only domain, and both are format-specific.

        A landed literal is one exact value. Its equality observation implies any
        weaker bound the domain asks for, so `6.0` establishes `value >= 0.01`.
        A parameter or other live place establishes the domain when an exit or
        initializer premise already observes a bound that implies the predicate.

        Builtin subtraction `left - right` establishes `value >= 0` when the same
        premises prove `left >= right` and both operands finite. The exact
        difference is then nonnegative. Default rounding keeps that sign: a tiny
        positive difference becomes `+0`, and overflow becomes `+infinity`, both
        of which compare `>= 0`. A strict `>` is not claimed, because underflow
        can produce zero. Without finiteness, `+infinity - +infinity` is NaN, so
        the ordered comparison alone is not enough.
    */
    use checked_trees::CheckFacts;
    use facts::{ContractFactKind, FactContextHandle, FactOrigin, FactPayload, FloatPredicate};
    use numerics::bignum::ExactFloat;
    use numerics::literals::FloatFormat;
    use typed_trees::domain::{DomainDefinition, domain_subject_reference};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use typed_trees::machine::Machine;
    use typed_trees::operator::{
        binary_operator_spelling, has_builtin_spelled_expression_meaning,
        primitive_float_binary_semantics, resolve_named_expression_call,
    };
    use typed_trees::state::State;
    use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
    use validation::CanonicalPlace;
    use validation::expression_result_type_reference;
    use validation::find_machine_by_entry_state;

    use crate::published::places::place_at;
    use crate::values::float_predicates::{comparison, has_ieee_meaning, literal};

    pub(in crate::checks::contracts) fn expression_establishes_domain_predicate(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        machine: &Machine,
        state: &State,
        statement_index: usize,
        value: ExpressionHandle,
        domain: &DomainDefinition,
        predicate: ExpressionHandle,
    ) -> bool {
        let Some(format) = domain_float_format(program, domain) else {
            return false;
        };
        float_predicate_holds(
            program,
            facts,
            contexts,
            machine,
            state,
            statement_index,
            value,
            domain,
            predicate,
            format,
        )
    }

    fn domain_float_format(
        program: &dyn ::validation::CheckingInput,
        domain: &DomainDefinition,
    ) -> Option<FloatFormat> {
        match program.primitive_type_reference(domain.target_type) {
            Some(PrimitiveType::F32) => Some(FloatFormat::F32),
            Some(PrimitiveType::F64) => Some(FloatFormat::F64),
            _ => None,
        }
    }

    fn float_predicate_holds(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        machine: &Machine,
        state: &State,
        statement_index: usize,
        value: ExpressionHandle,
        domain: &DomainDefinition,
        predicate: ExpressionHandle,
        format: FloatFormat,
    ) -> bool {
        let Some(binary) = program.expression_table.binary_node(predicate) else {
            return false;
        };
        if binary.operator == BinaryOperator::And {
            return float_predicate_holds(
                program,
                facts,
                contexts,
                machine,
                state,
                statement_index,
                value,
                domain,
                binary.left,
                format,
            ) && float_predicate_holds(
                program,
                facts,
                contexts,
                machine,
                state,
                statement_index,
                value,
                domain,
                binary.right,
                format,
            );
        }
        let Some(required) = required_subject_bound(program, domain, predicate, format) else {
            return false;
        };
        value_implies_bound(
            program,
            facts,
            contexts,
            machine,
            state,
            statement_index,
            value,
            format,
            required,
        )
    }

    fn required_subject_bound(
        program: &dyn ::validation::CheckingInput,
        domain: &DomainDefinition,
        predicate: ExpressionHandle,
        format: FloatFormat,
    ) -> Option<FloatPredicate> {
        let binary = comparison(program, predicate)?;
        let (operator, bound) = if domain_subject_reference(program, domain, binary.left) {
            (binary.operator, binary.right)
        } else if domain_subject_reference(program, domain, binary.right) {
            (binary.operator.mirrored(), binary.left)
        } else {
            return None;
        };
        let bound = literal(program, bound, format)?;
        let builtin = has_ieee_meaning(
            program,
            domain.symbol,
            predicate,
            operator,
            domain.target_type,
        );
        let lowered_call = program.expression_table.call_node(predicate).is_some();
        if !builtin && !lowered_call {
            return None;
        }
        FloatPredicate::comparison(format, operator, &bound)
    }

    fn value_implies_bound(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        machine: &Machine,
        state: &State,
        statement_index: usize,
        value: ExpressionHandle,
        format: FloatFormat,
        required: FloatPredicate,
    ) -> bool {
        if literal(program, value, format).is_some_and(|bound| {
            FloatPredicate::comparison(format, BinaryOperator::Equal, &bound)
                .is_some_and(|observed| observed.implies(required))
        }) {
            return true;
        }
        if premise_implies_bound(
            program,
            facts,
            contexts,
            machine,
            state,
            statement_index,
            value,
            format,
            required,
        ) {
            return true;
        }
        nonnegative_difference(
            program,
            facts,
            contexts,
            machine,
            state,
            statement_index,
            value,
            format,
        )
        .is_some_and(|observed| observed.implies(required))
    }

    fn premise_implies_bound(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        machine: &Machine,
        state: &State,
        statement_index: usize,
        value: ExpressionHandle,
        format: FloatFormat,
        required: FloatPredicate,
    ) -> bool {
        let Some(place) = place_at(program, state.symbol, statement_index, value) else {
            return false;
        };
        boolean_premises(program, facts, contexts).any(|premise| {
            let Some(binary) = comparison(program, premise) else {
                return false;
            };
            let (operator, bound) =
                if same_place(program, state, statement_index, binary.left, &place) {
                    (binary.operator, binary.right)
                } else if same_place(program, state, statement_index, binary.right, &place) {
                    (binary.operator.mirrored(), binary.left)
                } else {
                    return false;
                };
            let Some(reference) =
                expression_result_type_reference(program, machine, state, binary.left).or_else(
                    || expression_result_type_reference(program, machine, state, binary.right),
                )
            else {
                return false;
            };
            if !has_ieee_meaning(program, machine.symbol, premise, operator, reference) {
                return false;
            }
            literal(program, bound, format)
                .and_then(|bound| FloatPredicate::comparison(format, operator, &bound))
                .is_some_and(|observed| observed.implies(required))
        })
    }

    fn nonnegative_difference(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        machine: &Machine,
        state: &State,
        statement_index: usize,
        value: ExpressionHandle,
        format: FloatFormat,
    ) -> Option<FloatPredicate> {
        let binary = comparison(program, value)?;
        if binary.operator != BinaryOperator::Subtract {
            return None;
        }
        let reference = expression_result_type_reference(program, machine, state, value)?;
        let primitive = program.primitive_type_reference(reference)?;
        let value_format = match primitive {
            PrimitiveType::F32 => FloatFormat::F32,
            PrimitiveType::F64 => FloatFormat::F64,
            _ => return None,
        };
        if value_format != format {
            return None;
        }
        if !subtraction_has_ieee_meaning(program, machine, value, reference) {
            return None;
        }
        let left_finite = operand_is_finite(
            program,
            facts,
            contexts,
            state,
            statement_index,
            binary.left,
        );
        let right_finite = operand_is_finite(
            program,
            facts,
            contexts,
            state,
            statement_index,
            binary.right,
        );
        let ordered = left_is_at_least_right(
            program,
            facts,
            contexts,
            machine,
            state,
            statement_index,
            binary.left,
            binary.right,
        );
        if !left_finite || !right_finite || !ordered {
            return None;
        }
        FloatPredicate::comparison(
            format,
            BinaryOperator::GreaterOrEqual,
            &ExactFloat::from_f64(0.0),
        )
    }

    fn subtraction_has_ieee_meaning(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
    ) -> bool {
        let expected_format = match program.primitive_type_reference(reference) {
            Some(PrimitiveType::F32) => FloatFormat::F32,
            Some(PrimitiveType::F64) => FloatFormat::F64,
            _ => return false,
        };
        let Some(spelling) = binary_operator_spelling(BinaryOperator::Subtract) else {
            return false;
        };
        if has_builtin_spelled_expression_meaning(
            program,
            machine.symbol,
            expression,
            spelling,
            &[Some(reference), Some(reference)],
        ) {
            return true;
        }
        let Some(call) = program.expression_table.call_node(expression) else {
            return false;
        };
        let Some((target, _)) = find_machine_by_entry_state(program, call.target_symbol) else {
            return false;
        };
        let Some(binding) = program.machine_token_binding_by_symbol(target.symbol) else {
            return false;
        };
        primitive_float_binary_semantics(program, binding).is_some_and(|(operator, format)| {
            operator == BinaryOperator::Subtract && format == expected_format
        })
    }

    fn operand_is_finite(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        state: &State,
        statement_index: usize,
        operand: ExpressionHandle,
    ) -> bool {
        let Some(place) = place_at(program, state.symbol, statement_index, operand) else {
            return false;
        };
        boolean_premises(program, facts, contexts).any(|premise| {
            finite_call_argument(program, premise).is_some_and(|argument| {
                same_place(program, state, statement_index, argument, &place)
            }) || finite_call_argument(program, peel_true_comparison(program, premise)).is_some_and(
                |argument| same_place(program, state, statement_index, argument, &place),
            )
        })
    }

    fn left_is_at_least_right(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: &[FactContextHandle],
        machine: &Machine,
        state: &State,
        statement_index: usize,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> bool {
        let Some(left_place) = place_at(program, state.symbol, statement_index, left) else {
            return false;
        };
        let Some(right_place) = place_at(program, state.symbol, statement_index, right) else {
            return false;
        };
        boolean_premises(program, facts, contexts).any(|premise| {
            let Some(binary) = comparison(program, premise) else {
                return false;
            };
            let ordered = matches!(
                binary.operator,
                BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
            );
            let reversed = matches!(
                binary.operator,
                BinaryOperator::Less | BinaryOperator::LessOrEqual
            );
            if !has_ieee_meaning(
                program,
                machine.symbol,
                premise,
                binary.operator,
                expression_result_type_reference(program, machine, state, left).unwrap_or_default(),
            ) {
                return false;
            }
            (ordered
                && same_place(program, state, statement_index, binary.left, &left_place)
                && same_place(program, state, statement_index, binary.right, &right_place))
                || (reversed
                    && same_place(program, state, statement_index, binary.left, &right_place)
                    && same_place(program, state, statement_index, binary.right, &left_place))
        })
    }

    fn peel_true_comparison(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> ExpressionHandle {
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return expression;
        };
        if binary.operator == BinaryOperator::Equal
            && matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            )
        {
            binary.left
        } else {
            expression
        }
    }

    fn finite_call_argument(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Option<ExpressionHandle> {
        let call = program.expression_table.call_node(expression)?;
        let operator = resolve_named_expression_call(program, call)
            .or_else(|| program.operator_definition_by_symbol(call.target_symbol))?;
        let [namespace, requirement] = program.operator_path_members(operator.name) else {
            return None;
        };
        if requirement.as_str() != "is_finite" {
            return None;
        }
        if !matches!(namespace.as_str(), "F32" | "F64" | "Float") {
            return None;
        }
        let [argument] = program.expression_table.expression_handles(call.arguments) else {
            return None;
        };
        Some(*argument)
    }

    fn same_place(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        expression: ExpressionHandle,
        expected: &CanonicalPlace,
    ) -> bool {
        place_at(program, state.symbol, statement_index, expression)
            .is_some_and(|place| crate::flow::canonical_places_agree(program, &place, expected))
    }

    fn boolean_premises<'a>(
        program: &'a dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        contexts: &'a [FactContextHandle],
    ) -> impl Iterator<Item = ExpressionHandle> + 'a {
        contexts.iter().flat_map(move |context| {
            let mut pending = facts
                .semantic
                .context_view(facts.semantic.contexts.get(*context))
                .facts()
                .filter_map(|fact| premise_expression(fact.origin, fact.payload))
                .collect::<Vec<_>>();
            let mut conjuncts = Vec::new();
            while let Some(expression) = pending.pop() {
                if let Some(binary) = program.expression_table.binary_node(expression)
                    && binary.operator == BinaryOperator::And
                {
                    pending.push(binary.left);
                    pending.push(binary.right);
                } else {
                    conjuncts.push(expression);
                }
            }
            conjuncts
        })
    }

    fn premise_expression(origin: FactOrigin, payload: FactPayload) -> Option<ExpressionHandle> {
        if matches!(origin, FactOrigin::CallRequires | FactOrigin::CallEnsures) {
            return None;
        }
        match payload {
            FactPayload::ContractBooleanExpression {
                kind: ContractFactKind::Requires | ContractFactKind::Ensures,
                expression,
                instantiated,
                ..
            } if !instantiated.is_valid() => Some(expression),
            FactPayload::BooleanExpression(expression) => Some(expression),
            FactPayload::BooleanValue {
                expression,
                value: true,
            } => Some(expression),
            _ => None,
        }
    }
}
mod guard_operands {
    /*
        Whether a requirement's operands survive the jump's earlier operand writes.

        A named transition evaluates its guard first, then its operands left to
        right, then jumps. The guard routes in `calls.rs` match a requirement's
        instantiated label against a guard evaluated before any operand ran, so
        they quote the value the guard read. An operand evaluated before the
        operand the requirement reads may write that same storage —
        `done(clear(&mut self.flag), self.flag)` — after which the guard's fact no
        longer describes the value delivered. This module answers that one
        question from the checked write frames of the operand calls: it grants
        nothing and consults no source shape.
    */
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use facts::FactPayload;
    use symbols::SymbolHandle;
    use typed_trees::expression::{ExpressionHandle, ExpressionNode};
    use typed_trees::signature::StateParameter;
    use typed_trees::statement::TransitionTargetHandle;

    use super::assembly::expression_reads_overlapping_place;

    /*
        What a requirement instance reads at the jump: the argument expression a
        mentioned parameter binds, at that operand's position.
    */
    enum RequirementRead {
        Argument(ExpressionHandle),
    }

    /* One call inside this arm's operands, with the places it may write. */
    struct ArmOperandWrites {
        authored_expression: ExpressionHandle,
        writes: Vec<validation::CanonicalPlace>,
    }

    /*
        The named jump this call belongs to and every call inside its operands with
        the places each may write, or `None` when a write frame cannot be bounded
        (callers treat that as "may write anything"). `jump_arm` is `None` when this
        call is not a named jump at all: only a named jump has operands that run
        between the guard and the requirement's read, so the routes' existing
        prerequisites are unchanged.
    */
    struct ArmOperandFrame {
        jump_arm: Option<TransitionTargetHandle>,
        state_symbol: SymbolHandle,
        writes: Vec<ArmOperandWrites>,
    }

    fn arm_operand_frame(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<ArmOperandFrame> {
        let (machine, state) =
            program.state_flow_site(state_flow.machine_symbol, state_flow.state_symbol)?;
        let arm_of = |call_ordinal: usize| -> Option<TransitionTargetHandle> {
            validation::transition_call_target(
                program,
                machine,
                state,
                call_flow.statement_index,
                call_ordinal,
            )
        };
        let jump_arm = arm_of(call_flow.call_ordinal).filter(|arm| arm.is_valid());
        let Some(jump_arm) = jump_arm else {
            return Some(ArmOperandFrame {
                jump_arm: None,
                state_symbol: state.symbol,
                writes: Vec::new(),
            });
        };
        let state_flow_calls = facts
            .flow
            .control
            .state(machine.symbol, state.symbol)
            .map(|owner| facts.flow.control.calls.span_or_empty(owner.calls))?;
        let borrow_calls = facts
            .borrow
            .first_state_borrow(machine.symbol, state.symbol)
            .map(|(_, owner)| facts.borrow.calls.span_or_empty(owner.calls))
            .unwrap_or(&[]);
        let summaries = crate::flow::StateMutationSummaryCache::default();
        let mut writes = Vec::new();
        for operand_call in state_flow_calls.iter().filter(|call| {
            call.statement_index == call_flow.statement_index
                && call.call_ordinal != call_flow.call_ordinal
                && call.authored_expression.is_valid()
        }) {
            if arm_of(operand_call.call_ordinal) != Some(jump_arm) {
                continue;
            }
            let borrow_call = borrow_calls.iter().find(|candidate| {
                candidate.statement_index == operand_call.statement_index
                    && candidate.call_ordinal == operand_call.call_ordinal
            })?;
            let mut mutated = crate::flow::call_mutated_places(
                program,
                machine.symbol,
                state.symbol,
                &facts.borrow,
                borrow_call,
                &summaries,
                call_frames,
            )?;
            for write in &mut mutated {
                for segment in &mut write.segments {
                    if let facts::PlaceSegment::Index { expression } = segment {
                        *expression = ExpressionHandle::invalid();
                    }
                }
            }
            writes.push(ArmOperandWrites {
                authored_expression: operand_call.authored_expression,
                writes: mutated,
            });
        }
        Some(ArmOperandFrame {
            jump_arm: Some(jump_arm),
            state_symbol: state.symbol,
            writes,
        })
    }

    /*
        `true` when no operand call evaluated before one of the requirement's
        reads may write the storage that read names. `mentioned` lists the
        symbols the requirement spells; only those naming a target parameter are
        supplied by the jump, so only those can be read after an earlier operand.
        An operand call whose write frame the checked facts cannot bound is
        treated as writing everything.
    */
    pub(super) fn requirement_reads_survive_earlier_operand_writes(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        arguments: &[ExpressionHandle],
        target_parameters: &[StateParameter],
        mentioned: &[SymbolHandle],
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let reads = requirement_reads(arguments, target_parameters, mentioned);
        if reads.is_empty() {
            return true;
        }
        let Some(frame) = arm_operand_frame(program, facts, state_flow, call_flow, call_frames)
        else {
            return false;
        };
        if frame.jump_arm.is_none() {
            return true;
        }
        for operand in &frame.writes {
            let Some(operand_position) = arguments.iter().position(|argument| {
                expression_contains(program, *argument, operand.authored_expression)
            }) else {
                continue;
            };
            let later_reads = reads
                .iter()
                .filter(|(position, _)| *position > operand_position)
                .map(|(_, read)| read)
                .collect::<Vec<_>>();
            if later_reads.is_empty() {
                continue;
            }
            let written = operand.writes.iter().any(|write| {
                later_reads.iter().any(|read| match read {
                    RequirementRead::Argument(argument) => expression_reads_overlapping_place(
                        program,
                        frame.state_symbol,
                        call_flow.statement_index,
                        *argument,
                        write,
                    ),
                })
            });
            if written {
                return false;
            }
        }
        true
    }

    /*
        `true` when no call inside this arm's operands may write storage that a
        captured transition subject's predicate reads.

        A COMPUTED transition subject is evaluated once, before arm dispatch, and
        captured into a generated local; the arms read the capture's truth. The
        predicate's operands are read at that evaluation, before every arm operand,
        so any operand call writing one of them leaves a guard-derived requirement
        discharged from a value the jump no longer carries. The captured truth test
        mentions none of those operands, which is why the recovered predicate is
        what this check reads.
    */
    pub(super) fn premise_reads_survive_arm_operand_writes(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        call_flow: &FlowCallFact,
        premise: ExpressionHandle,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some(frame) = arm_operand_frame(program, facts, state_flow, call_flow, call_frames)
        else {
            return false;
        };
        if frame.jump_arm.is_none() {
            return true;
        }
        frame.writes.iter().all(|operand| {
            operand.writes.iter().all(|write| {
                !expression_reads_overlapping_place(
                    program,
                    frame.state_symbol,
                    call_flow.statement_index,
                    premise,
                    write,
                )
            })
        })
    }

    /* The symbols a boolean contract expression names at its roots. */
    pub(super) fn boolean_requirement_mentions(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
    ) -> Vec<SymbolHandle> {
        let mut mentioned = Vec::new();
        collect_root_mentions(program, expression, &mut mentioned);
        mentioned
    }

    /*
        The symbols a call-entry fact names: a boolean contract expression's
        roots, or a domain membership's subject expression roots. Both are
        callee-terms spellings -- a membership's PLACE was already substituted to
        the caller subject and names no target parameter, so mentioning it could
        never locate the operand position the requirement actually reads.
    */
    pub(super) fn fact_requirement_mentions(
        program: &dyn ::validation::CheckingInput,
        _facts: &CheckFacts,
        fact: &facts::Fact,
    ) -> Vec<SymbolHandle> {
        match fact.payload {
            FactPayload::ContractBooleanExpression { expression, .. } => {
                boolean_requirement_mentions(program, expression)
            }
            FactPayload::ContractDomainMembership { value, .. } => {
                boolean_requirement_mentions(program, value)
            }
            _ => Vec::new(),
        }
    }

    /*
        The caller argument expressions a requirement instance reads: one per
        mentioned target parameter. Symbols that name no target parameter are not
        supplied by the call and yield nothing.
    */
    pub(super) fn requirement_argument_reads(
        arguments: &[ExpressionHandle],
        target_parameters: &[StateParameter],
        mentioned: &[SymbolHandle],
    ) -> Vec<ExpressionHandle> {
        requirement_reads(arguments, target_parameters, mentioned)
            .into_iter()
            .map(|(_, RequirementRead::Argument(argument))| argument)
            .collect()
    }

    /*
        Each mentioned target parameter paired with the operand position at which
        the jump reads it. Symbols that name no target parameter are not supplied
        by the operands and are left to the other provers.
    */
    fn requirement_reads(
        arguments: &[ExpressionHandle],
        target_parameters: &[StateParameter],
        mentioned: &[SymbolHandle],
    ) -> Vec<(usize, RequirementRead)> {
        mentioned
            .iter()
            .filter_map(|symbol| {
                let index =
                    typed_trees::signature::state_parameter_position(target_parameters, *symbol)?;
                let argument_index = index;
                let argument = *arguments.get(argument_index)?;
                Some((argument_index, RequirementRead::Argument(argument)))
            })
            .collect()
    }

    fn collect_root_mentions(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        mentioned: &mut Vec<SymbolHandle>,
    ) {
        program.subtree_for_each(
            expression,
            typed_trees::SubtreeEdges::SAVED_SOURCES,
            |_, node| {
                if let ExpressionNode::Name(path) = node {
                    let root = if path.head_symbol.is_valid() {
                        path.head_symbol
                    } else {
                        path.symbol
                    };
                    if root.is_valid() && !mentioned.contains(&root) {
                        mentioned.push(root);
                    }
                }
            },
        );
    }

    /* Whether `needle` occurs anywhere inside `expression`. */
    fn expression_contains(
        program: &dyn ::validation::CheckingInput,
        expression: ExpressionHandle,
        needle: ExpressionHandle,
    ) -> bool {
        let edges = typed_trees::SubtreeEdges {
            pattern_dependencies: true,
            ..typed_trees::SubtreeEdges::VALUES
        };
        program.subtree_any(expression, edges, |handle, _| {
            if handle == needle {
                typed_trees::SubtreeVisit::Found
            } else {
                typed_trees::SubtreeVisit::Descend
            }
        })
    }
}
mod integer_embeddings {
    /*
        Exit-local congruence for admitted proof integer embeddings.
    */
    use super::return_values::{exit_return_expression, is_result_reference};
    use crate::flow::expressions;
    use checked_trees::{FlowExitFact, FlowStateFact};
    use typed_trees::expression::{BinaryOperator, ExpressionHandle};

    /*
        `embed(result) == 3` and `embed(result.field) == 3` when the returned
        integer is a closed length or literal. Plain equality already knows that
        value; embedding is the same mathematical integer, not a second unknown.
    */
    pub(super) fn proves_embedded_literal_exit(
        program: &dyn ::validation::CheckingInput,
        state_flow: &FlowStateFact,
        exit_flow: &FlowExitFact,
        fact: &facts::Fact,
    ) -> bool {
        let facts::FactPayload::ContractBooleanExpression { expression, .. } = fact.payload else {
            return false;
        };
        let Some(machine) = program.machine_by_symbol(state_flow.machine_symbol) else {
            return false;
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == exit_flow.state_symbol)
        else {
            return false;
        };
        if exit_flow.machine_symbol != machine.symbol || exit_flow.state_symbol != state.symbol {
            return false;
        }
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return false;
        };
        if binary.operator != BinaryOperator::Equal {
            return false;
        }
        let returned = exit_return_expression(program, exit_flow);
        if let Some((embedded, literal)) = embedded_literal(program, binary.left, binary.right)
            .or_else(|| embedded_literal(program, binary.right, binary.left))
        {
            let subject = returned_subject(program, machine, embedded, returned);
            let Some(value) = known_exit_integer(program, machine, state, subject) else {
                return false;
            };
            return value == literal;
        }
        let Some((_, left)) = validation::integer_embedding_argument(program, binary.left) else {
            return false;
        };
        let Some((_, right)) = validation::integer_embedding_argument(program, binary.right) else {
            return false;
        };
        let Some(left) = known_exit_integer(
            program,
            machine,
            state,
            returned_subject(program, machine, left, returned),
        ) else {
            return false;
        };
        let Some(right) = known_exit_integer(
            program,
            machine,
            state,
            returned_subject(program, machine, right, returned),
        ) else {
            return false;
        };
        left == right
    }

    fn embedded_literal(
        program: &dyn ::validation::CheckingInput,
        embedded: ExpressionHandle,
        literal: ExpressionHandle,
    ) -> Option<(ExpressionHandle, numerics::bignum::BigInt)> {
        let (_, argument) = validation::integer_embedding_argument(program, embedded)?;
        let literal = expressions(program).integer_node(literal)?.value_bignum()?;
        Some((argument, literal))
    }

    fn returned_subject(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        expression: ExpressionHandle,
        returned: ExpressionHandle,
    ) -> ExpressionHandle {
        if is_result_reference(program, machine, expression) {
            return returned;
        }
        let Some(member) = program.expression_table.member_node(expression) else {
            return expression;
        };
        if !is_result_reference(program, machine, member.receiver) {
            return expression;
        }
        let Some(literal) = program.expression_table.struct_literal_node(returned) else {
            return expression;
        };
        expressions(program)
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name.as_str() == member.member.as_str())
            .map(|field| field.value)
            .unwrap_or(expression)
    }

    fn known_exit_integer(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: ExpressionHandle,
    ) -> Option<numerics::bignum::BigInt> {
        if let Some(literal) = program.expression_table.integer_node(expression) {
            return literal.value_bignum();
        }
        let member = program.expression_table.member_node(expression)?;
        if member.case_variant.is_some()
            || language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
                member.member.as_str(),
            ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
        {
            return None;
        }
        let extent =
            validation::declared_collection_extent(program, machine, Some(state), member.receiver)?;
        Some(numerics::bignum::BigInt::from_u64(
            u64::try_from(extent).ok()?,
        ))
    }
}
mod intervals {
    /*
        The closed interval a guard states for one spelled place.

        Label routes discharge a requirement from a transition guard by reading the
        guard's comparisons as an interval, so `index < 16` establishes
        `requires self <= 15` without matching text. The interval is validation's
        `BigIntInterval`: an absent end is unbounded, and `&&` meets two readings
        on that lattice. A conjunct that does not name this spelling contributes
        no reading. A subject/literal pair that is not a closed comparison
        contributes the unbounded interval, so `&&` keeps the other conjunct.
        Either way the result is only ever weaker than the guard.
        An argument's own interval is not computed here; the arithmetic walk bounds
        every evaluated argument, and the call checks read that
        (`validation::expression_interval`).

        Predicate-only domain requirements still publish an unstated end as
        `i64::MIN` or `i64::MAX` (`predicate_only_domain_interval`). Containment
        against that publication treats an absent guard end as the same extreme, so
        a single comparison discharges the same requirements it did when this
        module filled those extremes itself. That adapter is not a second lattice:
        the meet and the ends live on `BigIntInterval`.
    */
    use numerics::bignum::BigInt;
    use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use validation::BigIntInterval;

    /*
        The closed interval a guard's conjuncts establish for the place spelled
        `subject_label`.
    */
    pub(super) fn guard_interval_for_label(
        program: &dyn ::validation::CheckingInput,
        guard: ExpressionHandle,
        subject_label: &str,
    ) -> Option<BigIntInterval> {
        let binary = program.expression_table.binary_node(guard)?;
        match binary.operator {
            BinaryOperator::And => {
                let left = guard_interval_for_label(program, binary.left, subject_label);
                let right = guard_interval_for_label(program, binary.right, subject_label);
                match (left, right) {
                    (Some(left), Some(right)) => Some(left.intersect(&right)),
                    (bound, None) | (None, bound) => bound,
                }
            }
            BinaryOperator::Equal
                if matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
            {
                guard_interval_for_label(program, binary.left, subject_label)
            }
            _ => comparison_bound(program, binary, subject_label),
        }
    }

    /*
        Whether `guard` is contained in the closed requirement whose unstated ends
        are still the signed extremes. An absent guard end meets a required end
        only at or beyond that extreme, matching `i64::MIN >= required_low` and
        `i64::MAX <= required_high`.
    */
    pub(super) fn establishes_closed_requirement(
        guard: &BigIntInterval,
        required_low: &BigInt,
        required_high: &BigInt,
    ) -> bool {
        let low_holds = match &guard.low {
            Some(low) => low >= required_low,
            None => required_low <= &BigInt::from_i64(i64::MIN),
        };
        let high_holds = match &guard.high {
            Some(high) => high <= required_high,
            None => required_high >= &BigInt::from_i64(i64::MAX),
        };
        low_holds && high_holds
    }

    /*
        Narrow an open `i64` lattice by a guard interval. An end that does not fit
        strictly inside `i64::MIN..i64::MAX` stays open: the lattice's own extremes
        add no stricter finite bound, and a magnitude outside `i64` cannot land in it.
    */
    pub(super) fn narrow_i64_bounds(
        mut low: Option<i64>,
        mut high: Option<i64>,
        guard: &BigIntInterval,
    ) -> (Option<i64>, Option<i64>) {
        if let Some(bound) = guard
            .low
            .as_ref()
            .and_then(BigInt::to_i64)
            .filter(|bound| *bound > i64::MIN)
        {
            low = Some(low.map_or(bound, |current| current.max(bound)));
        }
        if let Some(bound) = guard
            .high
            .as_ref()
            .and_then(BigInt::to_i64)
            .filter(|bound| *bound < i64::MAX)
        {
            high = Some(high.map_or(bound, |current| current.min(bound)));
        }
        (low, high)
    }

    /*
        The interval one comparison of `subject_label` against a closed integer
        literal states. A subject/literal pair that is not a closed comparison
        states the unbounded interval: `&&` then keeps the other conjunct, and a
        lone non-comparison does not invent a finite end.
    */
    fn comparison_bound(
        program: &dyn ::validation::CheckingInput,
        binary: &typed_trees::expression::TableBinaryExpression,
        subject_label: &str,
    ) -> Option<BigIntInterval> {
        let left_is_subject = program.expression_table.display_name(binary.left) == subject_label;
        let right_is_subject = program.expression_table.display_name(binary.right) == subject_label;
        let (literal, subject_on_left) = match (left_is_subject, right_is_subject) {
            (true, false) => (binary.right, true),
            (false, true) => (binary.left, false),
            _ => return None,
        };
        let value = validation::closed_integer_range_bound(program, literal)?;
        Some(
            BigIntInterval::from_comparison(binary.operator, subject_on_left, value)
                .unwrap_or_else(BigIntInterval::unbounded),
        )
    }
}
mod reference_domains {
    /*
        Exact live aliases and owned captures transport established qualifications.
    */
    use super::prover::segments_cover_subject;
    use crate::checks::places::place_or_root_in;
    use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};
    use facts::{Fact, FactContextHandle};

    pub(super) fn proves(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &FlowStateFact,
        call: &FlowCallFact,
        entry_contexts: &[FactContextHandle],
        required: &Fact,
        captured_referents: Option<&[validation::CanonicalPlace]>,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> bool {
        let Some((_, _, domain_symbol, _)) = required.payload.domain_membership() else {
            return false;
        };
        let Some(place) = required.place.as_place() else {
            return false;
        };
        let place = facts.semantic.places.get(place);
        let place = place_or_root_in(&facts.semantic.place_segments, place.root, place.segments);
        let Some(machine) = program.machine_by_symbol(state.machine_symbol) else {
            return false;
        };
        let mut owned_frames = None;
        let Some(frames) =
            crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames)
        else {
            return false;
        };
        let sources = if let Some(referents) = captured_referents {
            referents.to_vec()
        } else {
            match crate::flow::local_reference_storage_at_call(
                program,
                frames,
                machine,
                &facts.flow,
                state,
                call,
                place.clone(),
                crate::flow::ValueObservation::Contents,
            ) {
                Some(source) => vec![source],
                None => crate::flow::local_reference_candidate_storages_at_call(
                    program,
                    frames,
                    &facts.borrow,
                    machine,
                    &facts.flow,
                    state,
                    call,
                    place.clone(),
                )
                .unwrap_or_default(),
            }
        };
        if !sources.is_empty()
            && sources.iter().all(|source| {
                if captured_referents.is_some()
                    && !crate::flow::place_cases_are_selected(
                        program,
                        &facts.semantic,
                        entry_contexts,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        source,
                    )
                    && !(source.segments.ends_with(&place.segments)
                        && place
                            .segments
                            .iter()
                            .any(|segment| matches!(segment, facts::PlaceSegment::Case { .. }))
                        && crate::flow::place_cases_are_selected(
                            program,
                            &facts.semantic,
                            entry_contexts,
                            state.machine_symbol,
                            state.state_symbol,
                            call.statement_index,
                            &place,
                        ))
                {
                    return false;
                }
                contexts_prove_domain(
                    program,
                    facts,
                    entry_contexts.iter().copied(),
                    source,
                    domain_symbol,
                    |candidate| {
                        let candidate = crate::flow::local_reference_storage_at_call(
                            program,
                            frames,
                            machine,
                            &facts.flow,
                            state,
                            call,
                            candidate,
                            crate::flow::ValueObservation::Contents,
                        )?;
                        if captured_referents.is_some()
                            && candidate.segments.iter().zip(&source.segments).any(
                                |(candidate, source)| {
                                    matches!(source, facts::PlaceSegment::Index { .. })
                                        && !matches!(
                                            candidate,
                                            facts::PlaceSegment::FixedRange {
                                                start: 0,
                                                end: usize::MAX
                                            }
                                        )
                                },
                            )
                        {
                            return None;
                        }
                        Some(candidate)
                    },
                )
            })
        {
            return true;
        }
        if captured_referents.is_some() || !field_place(&place) {
            return false;
        }
        let Some(source) = crate::flow::value_origin_at_call(
            program,
            &facts.flow,
            machine,
            state,
            call,
            place,
            Some(frames),
            crate::flow::ValueObservation::Contents,
        ) else {
            return false;
        };
        if !field_place(&source) {
            return false;
        }
        contexts_prove_domain(
            program,
            facts,
            facts
                .flow
                .semantic_constraint_contexts(state.entry_constraints),
            &source,
            domain_symbol,
            Some,
        )
    }

    fn field_place(place: &validation::CanonicalPlace) -> bool {
        matches!(place.root, facts::PlaceRoot::Symbol(symbol) if symbol.is_valid())
            && place.segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. } | facts::PlaceSegment::Case { .. }
                )
            })
    }

    fn contexts_prove_domain(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        contexts: impl Iterator<Item = FactContextHandle>,
        source: &validation::CanonicalPlace,
        domain_symbol: symbols::SymbolHandle,
        mut resolve_candidate: impl FnMut(
            validation::CanonicalPlace,
        ) -> Option<validation::CanonicalPlace>,
    ) -> bool {
        let mut contexts = contexts;
        contexts.any(|context| {
            facts
                .semantic
                .context_view(facts.semantic.contexts.get(context))
                .facts()
                .any(|candidate| {
                    let candidate_domain = match candidate.payload {
                        facts::domain_membership_pattern!(domain_symbol) => domain_symbol,
                        _ => return false,
                    };
                    let Some(candidate) = candidate.place.as_place() else {
                        return false;
                    };
                    let candidate = facts.semantic.places.get(candidate);
                    if !validation::field_domains::domain_membership_implies(
                        program,
                        candidate_domain,
                        domain_symbol,
                    ) {
                        return false;
                    }
                    let candidate = place_or_root_in(
                        &facts.semantic.place_segments,
                        candidate.root,
                        candidate.segments,
                    );
                    let Some(candidate) = resolve_candidate(candidate) else {
                        return false;
                    };
                    (candidate.root) == (source.root)
                        && segments_cover_subject(program, &candidate.segments, &source.segments)
                })
        })
    }
}

/*
    Shared vocabulary: contract labels, place matching and exit return
    occurrences.
    `pub(super)` so the operator-`requires` discharge (checks/operators) can
    reuse the domain-derived boolean proving labels.
*/
pub(crate) mod labels {
    pub(crate) mod calls {
        use crate::flow::expressions;
        use crate::flow::reference_expression_storage_places_including_shared;
        use crate::flow::reference_route_source;
        use symbols::SymbolHandle;
        use typed_trees::proposition::ProofSubstitutions;

        /* The reserved binder naming a call's return value inside an `ensures` clause. */
        pub(crate) const RESULT_BINDER: &str = "result";

        /*
            Parameter source for an instantiated call contract. Ordinary calls use a
            concrete state; static machine-parameter calls use the authored signature
            directly while their generic body is checked.
        */
        pub(crate) trait ContractTargetParameters {
            fn contract_parameters<'program>(
                &'program self,
                program: &'program dyn ::validation::CheckingInput,
            ) -> &'program [typed_trees::signature::StateParameter];
        }

        impl ContractTargetParameters for typed_trees::state::State {
            fn contract_parameters<'program>(
                &'program self,
                program: &'program dyn ::validation::CheckingInput,
            ) -> &'program [typed_trees::signature::StateParameter] {
                program.state_parameters(self)
            }
        }

        impl ContractTargetParameters for [typed_trees::signature::StateParameter] {
            fn contract_parameters<'program>(
                &'program self,
                _program: &'program dyn ::validation::CheckingInput,
            ) -> &'program [typed_trees::signature::StateParameter] {
                self
            }
        }

        /*
            Render the label of the value a call produces, used to substitute the
            `result` binder of the callee's `ensures` clause into caller terms. This is
            the call expression itself (`receiver.target(args)` or `target(args)`), so a
            fact like `ensures result in String::Utf8` becomes a domain fact on the
            concrete call result at the call site.
        */
        fn call_result_label(
            program: &dyn ::validation::CheckingInput,
            call_site: &validation::CallSite<'_>,
        ) -> String {
            let argument_list = |arguments| {
                expressions(program)
                    .expression_handles(arguments)
                    .iter()
                    .map(|argument| {
                        program.render_proof_expression(*argument, ProofSubstitutions::None)
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            match call_site {
                validation::CallSite::Statement(call) => {
                    let arguments = argument_list(call.arguments);
                    let receiver = typed_trees::expression::display_name_path(
                        program.expression_table.name_path_members(call.receiver),
                        "::",
                    );
                    if receiver.is_empty() {
                        format!("{}({arguments})", call.target)
                    } else {
                        format!("{receiver}.{}({arguments})", call.target)
                    }
                }
                validation::CallSite::Expression { call, .. } => {
                    let arguments = argument_list(call.arguments);
                    if call.receiver.is_valid() {
                        format!(
                            "{}.{}({arguments})",
                            program
                                .render_proof_expression(call.receiver, ProofSubstitutions::None),
                            call.target
                        )
                    } else {
                        format!("{}({arguments})", call.target)
                    }
                }
                validation::CallSite::TransitionNamed { .. } => RESULT_BINDER.to_owned(),
            }
        }

        #[allow(
            clippy::only_used_in_recursion,
            reason = "call-site coordinates are deliberately threaded through recursive label rendering"
        )]
        pub(crate) fn instantiate_call_contract_expression_label(
            program: &dyn ::validation::CheckingInput,
            caller_state_symbol: SymbolHandle,
            statement_index: usize,
            call_site: &validation::CallSite<'_>,
            target_state: &(impl ContractTargetParameters + ?Sized),
            expression: typed_trees::expression::ExpressionHandle,
        ) -> String {
            let render = |value: typed_trees::expression::ExpressionHandle| {
                instantiate_call_contract_expression_label(
                    program,
                    caller_state_symbol,
                    statement_index,
                    call_site,
                    target_state,
                    value,
                )
            };
            let name_label = |path: &typed_trees::expression::TableNamePath| -> String {
                let members = program.expression_table.name_path_members(path.members);
                let first_member = members.first().map(|member| member.as_str());

                if first_member == Some(RESULT_BINDER)
                    && members.len() == 1
                    && !target_state
                        .contract_parameters(program)
                        .iter()
                        .any(|parameter| parameter.name.as_str() == RESULT_BINDER)
                {
                    return call_result_label(program, call_site);
                }

                let arguments = validation::call_site_argument_expressions(program, call_site);
                let mut argument_index = 0usize;

                for parameter in target_state.contract_parameters(program) {
                    let parameter_matches = first_member == Some(parameter.name.as_str())
                        || path.head_symbol == parameter.symbol
                        || path.symbol == parameter.symbol;

                    let argument = arguments.get(argument_index).copied();
                    argument_index = argument_index.saturating_add(1);
                    if parameter_matches {
                        return argument
                            .map(|argument| {
                                let argument = match (
                                    program
                                        .type_reference_table
                                        .type_reference(parameter.type_reference),
                                    program.expression_table.expression(argument),
                                ) {
                                    (
                                        typed_trees::types::TypeReferenceNode::Reference { .. },
                                        typed_trees::expression::ExpressionNode::Borrow(borrow),
                                    ) => borrow.target,
                                    _ => argument,
                                };
                                if let Some(storage) = reference_actual_storage_label(
                                    program,
                                    caller_state_symbol,
                                    statement_index,
                                    parameter.type_reference,
                                    argument,
                                ) {
                                    return storage;
                                }
                                program.render_proof_expression(argument, ProofSubstitutions::None)
                            })
                            .unwrap_or_else(|| parameter.name.to_string());
                    }
                }

                let machine_arguments = call_site_machine_arguments(call_site);
                if members.len() == 1 && !machine_arguments.is_empty() {
                    let type_parameters = validation::call_target_type_parameters(
                        program,
                        call_site_target_symbol(call_site),
                    );
                    for (position, parameter) in type_parameters.enumerate() {
                        let binder_matches = matches!(
                            &parameter.kind,
                            typed_trees::data::TypeParameterKind::Const { .. }
                                | typed_trees::data::TypeParameterKind::Value { .. }
                        ) && (path.head_symbol == parameter.symbol
                            || path.symbol == parameter.symbol
                            || first_member == Some(parameter.name.as_str()));
                        if binder_matches && let Some(argument) = machine_arguments.get(position) {
                            return argument.display_name();
                        }
                    }
                }

                program.render_proof_expression(expression, ProofSubstitutions::None)
            };
            super::render::expression_label(
                program,
                program.expression_table.expression(expression),
                &render,
                &name_label,
                &|literal| program.render_proof_constructor_value(literal, render),
            )
        }

        /*
            The caller's spelling of storage a reference actual names, when that
            actual is a local loan or a returned loan rather than a borrow written
            at the call. One exact referent is required: several candidates would
            let a fact about one of them discharge a requirement on all of them.
        */
        fn reference_actual_storage_label(
            program: &dyn ::validation::CheckingInput,
            caller_state_symbol: SymbolHandle,
            statement_index: usize,
            parameter_type: typed_trees::types::TypeReferenceHandle,
            argument: typed_trees::expression::ExpressionHandle,
        ) -> Option<String> {
            if !type_is_reference(program, parameter_type) {
                return None;
            }
            let state = program.state_by_symbol(caller_state_symbol)?;
            let rendered = match program.expression_table.expression(argument) {
                typed_trees::expression::ExpressionNode::Name(path)
                    if path.symbol == path.head_symbol
                        && expressions(program).name_path_members(path.members).len() == 1 =>
                {
                    let statements = program.statement_table.statements(state.statement_nodes);
                    let source = reference_route_source(
                        program,
                        state,
                        statements,
                        statement_index,
                        path.symbol,
                    )?;
                    referent_label(program, caller_state_symbol, statement_index, source)
                }
                typed_trees::expression::ExpressionNode::Call(_) => {
                    referent_label(program, caller_state_symbol, statement_index, argument)
                }
                _ => None,
            }?;
            Some(rendered)
        }

        fn referent_label(
            program: &dyn ::validation::CheckingInput,
            caller_state_symbol: SymbolHandle,
            statement_index: usize,
            expression: typed_trees::expression::ExpressionHandle,
        ) -> Option<String> {
            let mut referent = expression;
            loop {
                match program.expression_table.expression(referent) {
                    typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                        referent = borrow.target
                    }
                    typed_trees::expression::ExpressionNode::Cast(cast)
                        if cast.form.is_recast() =>
                    {
                        referent = cast.value;
                    }
                    typed_trees::expression::ExpressionNode::Call(_) => {
                        let places = reference_expression_storage_places_including_shared(
                            program,
                            caller_state_symbol,
                            statement_index,
                            referent,
                            None,
                        )?;
                        let [place] = places.as_slice() else {
                            return None;
                        };
                        return Some(place.label(program));
                    }
                    _ => {
                        return Some(
                            program.render_proof_expression(referent, ProofSubstitutions::None),
                        );
                    }
                }
            }
        }

        fn type_is_reference(
            program: &dyn ::validation::CheckingInput,
            mut reference: typed_trees::types::TypeReferenceHandle,
        ) -> bool {
            while reference.is_valid() {
                match program.type_reference_table.type_reference(reference) {
                    typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                        reference = *base_type;
                    }
                    typed_trees::types::TypeReferenceNode::Reference { .. } => return true,
                    _ => return false,
                }
            }
            false
        }

        /*
            The static generic arguments authored on the call (`bounded<K>`), empty
            for a named transition which carries none.
        */
        fn call_site_machine_arguments<'program>(
            call_site: &'program validation::CallSite<'program>,
        ) -> &'program [typed_trees::expression::StaticMachineArgument] {
            match call_site {
                validation::CallSite::Statement(call) => &call.machine_arguments,
                validation::CallSite::Expression { call, .. } => &call.machine_arguments,
                validation::CallSite::TransitionNamed { .. } => &[],
            }
        }

        /*
            The state symbol the call targets, used to reach the callee's declared
            generic telescope for binder instantiation.
        */
        fn call_site_target_symbol(call_site: &validation::CallSite<'_>) -> SymbolHandle {
            match call_site {
                validation::CallSite::Statement(call) => call.target_symbol,
                validation::CallSite::Expression { call, .. } => call.target_symbol,
                validation::CallSite::TransitionNamed { path, .. } => path.symbol,
            }
        }
    }
    mod domain {
        use symbols::SymbolHandle;

        pub(crate) fn domain_proves_expression_label(
            program: &dyn ::validation::CheckingInput,
            domain_symbol: SymbolHandle,
            base_label: &str,
            candidate_label: &str,
        ) -> bool {
            if !typed_trees::domain::has_symbol_only_proof(program, domain_symbol) {
                return false;
            }
            let Some(domain) = program.domain_definition(domain_symbol) else {
                return false;
            };

            program.proof_facts(domain).iter().any(|fact| match fact {
                typed_trees::domain::ProofFact::Expression(expression) => {
                    instantiate_domain_expression_label(program, domain, *expression, base_label)
                        == candidate_label
                }
                typed_trees::domain::ProofFact::Membership(membership) => {
                    let nested_base = instantiate_domain_expression_label(
                        program,
                        domain,
                        membership.value,
                        base_label,
                    );
                    domain_proves_expression_label(
                        program,
                        membership.domain_symbol,
                        &nested_base,
                        candidate_label,
                    )
                }
                typed_trees::domain::ProofFact::Proposition(_) => false,
            })
        }

        pub(crate) fn instantiate_domain_expression_label(
            program: &dyn ::validation::CheckingInput,
            domain: &typed_trees::domain::DomainDefinition,
            expression: typed_trees::expression::ExpressionHandle,
            base_label: &str,
        ) -> String {
            let render = |value: typed_trees::expression::ExpressionHandle| {
                instantiate_domain_expression_label(program, domain, value, base_label)
            };
            super::render::expression_label(
                program,
                program.expression_table.expression(expression),
                &render,
                &|path| {
                    let members = program.expression_table.name_path_members(path.members);
                    if typed_trees::domain::domain_subject_reference(program, domain, expression) {
                        base_label.to_owned()
                    } else {
                        typed_trees::expression::display_name_path(members, "::")
                    }
                },
                &|literal| literal.type_name.to_string(),
            )
        }
    }
    mod render {
        /*
            Expression-label arm shapes the call-contract and domain renderers share:
            every arm below renders identically given a child labeler. Arms whose
            label carries renderer-specific context (names, struct literals, the
            `result` binder) stay with their own match.
        */
        use crate::flow::expressions;
        use arena::HandleSpan;
        use typed_trees::expression::{
            ExpressionHandle, MatchPattern, TableAtomicExpression, TableBinaryExpression,
            TableBorrowExpression, TableCallExpression, TableCastExpression,
            TableIndexedExpression, TableMatchExpression, TableMemberExpression,
            TableRangeExpression, TableUnaryExpression, display_name_path,
        };

        pub(super) fn match_label(
            program: &dyn ::validation::CheckingInput,
            dispatch: &TableMatchExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            let arms = expressions(program)
                .match_arms(dispatch.arms)
                .iter()
                .map(|arm| {
                    let pattern = match arm.pattern {
                        MatchPattern::Value(value) => render(value),
                        MatchPattern::Wildcard => "_".to_owned(),
                        MatchPattern::Destructure(_) => {
                            typed_trees::expression::display_match_pattern(
                                &program.expression_table,
                                arm.pattern,
                                render,
                            )
                        }
                    };
                    format!("{pattern} -> {}", render(arm.value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("match {} {{ {arms} }}", render(dispatch.subject))
        }

        pub(super) fn atomic_label(
            atomic: &TableAtomicExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            format!("atomic[{:?}]({})", atomic.ordering, render(atomic.value))
        }

        pub(super) fn array_literal_label(
            program: &dyn ::validation::CheckingInput,
            values: HandleSpan<ExpressionHandle>,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            let values = expressions(program)
                .expression_handles(values)
                .iter()
                .map(|value| render(*value))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }

        pub(super) fn binary_label(
            binary: &TableBinaryExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            format!(
                "{} {} {}",
                render(binary.left),
                binary.operator.display_name(),
                render(binary.right)
            )
        }

        pub(super) fn cast_label(
            program: &dyn ::validation::CheckingInput,
            cast: &TableCastExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            format!(
                "{} as {}",
                render(cast.value),
                display_name_path(
                    expressions(program).name_path_members(cast.target_label),
                    "::",
                )
            )
        }

        pub(super) fn call_label(
            program: &dyn ::validation::CheckingInput,
            call: &TableCallExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            typed_trees::expression::display_call_label(&program.expression_table, call, render)
        }

        pub(super) fn indexed_label(
            indexed: &TableIndexedExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            format!("{}[{}]", render(indexed.collection), render(indexed.index))
        }

        pub(super) fn member_label(
            member: &TableMemberExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            format!("{}.{}", render(member.receiver), member.member)
        }

        pub(super) fn borrow_label(
            inner: &TableBorrowExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            let target = render(inner.target);
            match inner.access {
                language_semantics::ReferenceAccess::Shared => target,
                language_semantics::ReferenceAccess::Mutable => format!("mut {target}"),
                language_semantics::ReferenceAccess::WriteOnly => format!("write {target}"),
            }
        }

        pub(super) fn range_label(
            range: &TableRangeExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            match (range.start.is_valid(), range.end.is_valid()) {
                (true, true) => format!("{}..{}", render(range.start), render(range.end)),
                (true, false) => format!("{}..", render(range.start)),
                (false, true) => format!("..{}", render(range.end)),
                (false, false) => "..".to_owned(),
            }
        }

        pub(super) fn unary_label(
            unary: &TableUnaryExpression,
            render: &dyn Fn(ExpressionHandle) -> String,
        ) -> String {
            format!("{}{}", unary.operator.display_name(), render(unary.operand))
        }

        /*
            The full expression-label dispatcher the call-contract and domain renderers
            share: every arm renders identically except `Name` (whose binder rules
            differ by caller) and `StructLiteral`, which the caller closures supply.
        */
        pub(super) fn expression_label(
            program: &dyn ::validation::CheckingInput,
            node: &typed_trees::expression::ExpressionNode,
            render: &dyn Fn(ExpressionHandle) -> String,
            name_label: &dyn Fn(&typed_trees::expression::TableNamePath) -> String,
            struct_label: &dyn Fn(&typed_trees::expression::TableStructLiteral) -> String,
        ) -> String {
            match node {
                typed_trees::expression::ExpressionNode::SavedSubject(saved) => format!(
                    "$saved-subject#{}:{}",
                    saved.capture.arena_index(),
                    saved.capture.generation()
                ),
                typed_trees::expression::ExpressionNode::Match(dispatch) => {
                    match_label(program, dispatch, render)
                }
                typed_trees::expression::ExpressionNode::Atomic(atomic) => {
                    atomic_label(atomic, render)
                }
                typed_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                    array_literal_label(program, *values, render)
                }
                typed_trees::expression::ExpressionNode::Binary(binary) => {
                    binary_label(binary, render)
                }
                typed_trees::expression::ExpressionNode::Boolean(value) => value.to_string(),
                typed_trees::expression::ExpressionNode::Cast(cast) => {
                    cast_label(program, cast, render)
                }
                typed_trees::expression::ExpressionNode::Call(call) => {
                    call_label(program, call, render)
                }
                typed_trees::expression::ExpressionNode::Float(value) => value.to_string(),
                typed_trees::expression::ExpressionNode::UndeterminedNaN(format) => {
                    format!("<{} NaN without payload>", format.name())
                }
                typed_trees::expression::ExpressionNode::Indexed(indexed) => {
                    indexed_label(indexed, render)
                }
                typed_trees::expression::ExpressionNode::Integer(value) => value.to_string(),
                typed_trees::expression::ExpressionNode::Member(member) => {
                    member_label(member, render)
                }
                typed_trees::expression::ExpressionNode::Borrow(inner) => {
                    borrow_label(inner, render)
                }
                typed_trees::expression::ExpressionNode::Name(path) => name_label(path),
                typed_trees::expression::ExpressionNode::Range(range) => range_label(range, render),
                typed_trees::expression::ExpressionNode::StructLiteral(literal) => {
                    struct_label(literal)
                }
                typed_trees::expression::ExpressionNode::String(value) => format!("{value:?}"),
                typed_trees::expression::ExpressionNode::Unary(unary) => unary_label(unary, render),
                typed_trees::expression::ExpressionNode::TypeExpression(type_reference) => {
                    program.display_type_reference(*type_reference).to_string()
                }
                typed_trees::expression::ExpressionNode::ZeroValue(type_reference) => format!(
                    "zero_value<{}>()",
                    program.display_type_reference(*type_reference)
                ),
            }
        }
    }

    pub(crate) use calls::{ContractTargetParameters, instantiate_call_contract_expression_label};
    pub(crate) use domain::{domain_proves_expression_label, instantiate_domain_expression_label};
}
mod places {
    pub(super) fn expression_is_boolean_place_like(
        program: &dyn ::validation::CheckingInput,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Borrow(inner) => {
                expression_is_boolean_place_like(program, inner.target)
            }
            typed_trees::expression::ExpressionNode::Name(_)
            | typed_trees::expression::ExpressionNode::Member(_)
            | typed_trees::expression::ExpressionNode::Indexed(_) => true,
            _ => false,
        }
    }

    pub(super) fn expression_place_matches(
        program: &dyn ::validation::CheckingInput,
        semantic: &facts::FactPlan,
        expression: typed_trees::expression::ExpressionHandle,
        candidate_place: facts::PlaceHandle,
    ) -> bool {
        let candidate_label = semantic.place_label(program, candidate_place);
        program.expression_table.display_name(expression) == candidate_label
    }
}
mod return_values {
    /*
        Exact normal-return occurrences shared by exit proof consumers.
    */
    use checked_trees::{CheckFacts, FlowCallFact, FlowExitFact, FlowStateFact};
    use typed_trees::{
        expression::ExpressionHandle,
        machine::Machine,
        statement::{StatementNode, TransitionExit, TransitionTargetNode},
    };

    /*
        A completed opaque activation claim cannot cross its owning activation's
        return boundary. Check the actual returning occurrence, not its signature:
        crashing bodies and claim-free Rejected attempts do not return a claim.
        Aggregate/conditional custody is decided on the final executable CFG.
    */
    pub(super) fn check_activation_return(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state_flow: &FlowStateFact,
        exit: &FlowExitFact,
        diagnostics: &mut Vec<diagnostics::Diagnostic>,
    ) {
        let Some(machine) = program.machine(exit.machine_symbol) else {
            return;
        };
        let Some(state) = validation::find_state_for_exit(program, exit) else {
            return;
        };
        let returned = exit_return_expression(program, exit);
        let reference = if returned.is_valid() {
            validation::expression_result_type_reference(program, machine, state, returned)
        } else {
            exit_return_call_in_state(program, facts, state_flow, exit)
                .and_then(|call| validation::call_target_state(program, call.target_symbol))
                .map(|target| target.return_type)
        };
        let Some(symbol) = reference.and_then(|reference| {
            program
                .type_reference_table
                .type_reference(reference)
                .as_named_symbol()
        }) else {
            return;
        };
        if validation::activation_claim_byte_carrier(program, symbol).is_none() {
            return;
        }
        let mut diagnostic = diagnostics::Diagnostic::error(format!(
            "activation claim cannot escape its owning activation through return from {} state {} at statement {}",
            program.symbols.display_path(exit.machine_symbol, "::"),
            program.symbols.display_path(exit.state_symbol, "::"),
            exit.statement_index,
        ));
        if returned.is_valid() {
            let span = program.expression_table.source_span(returned);
            if span != source::SourceSpan::default() {
                diagnostic = diagnostic.with_source_span(span);
            }
        }
        diagnostics.push(diagnostic);
    }

    /*
        The completed operand belongs to this exact outgoing occurrence. An exit's
        source position alone does not establish that its expression has finished:
        same-statement call inputs still use their earlier evaluation frontier.
    */
    pub(in crate::checks::contracts) fn completed_return_operand<'facts>(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        exit: &FlowExitFact,
    ) -> Option<&'facts checked_trees::FlowOperandFact> {
        if !facts.flow.control.operands.is_valid(exit.returned_value) {
            return None;
        }
        let operand = facts.flow.control.operands.get(exit.returned_value);
        let returned = exit_return_expression(program, exit);
        if !returned.is_valid()
            || operand.expression != returned
            || operand.state_symbol != exit.state_symbol
            || operand.statement_index != exit.statement_index
        {
            return None;
        }
        let mut outgoing = facts.flow.control.outgoing.iter().filter(|(_, outgoing)| {
            outgoing.machine_symbol == exit.machine_symbol
                && outgoing.state_symbol == exit.state_symbol
                && outgoing.statement_index == exit.statement_index
                && outgoing.transition_target == exit.transition_target
                && outgoing.returned_value == exit.returned_value
        });
        outgoing.next()?;
        outgoing.next().is_none().then_some(operand)
    }

    /*
        Rejoin a returned invocation to its exact retained call occurrence, whether
        the return spells a value call or names an external tail target.
    */
    pub(super) fn exit_return_call<'facts>(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        exit: &FlowExitFact,
    ) -> Option<&'facts FlowCallFact> {
        let returned = exit_return_expression(program, exit);
        let calls = facts
            .flow
            .control
            .machine_states(exit.machine_symbol)
            .into_iter()
            .filter(|state| state.state_symbol == exit.state_symbol)
            .map(|state| ((), state))
            .flat_map(|(_, state)| facts.flow.control.calls.span_or_empty(state.calls));
        matching_exit_return_call(program, exit, returned, calls)
    }

    pub(super) fn exit_return_call_in_state<'facts>(
        program: &dyn ::validation::CheckingInput,
        facts: &'facts CheckFacts,
        state_flow: &FlowStateFact,
        exit: &FlowExitFact,
    ) -> Option<&'facts FlowCallFact> {
        let returned = exit_return_expression(program, exit);
        matching_exit_return_call(
            program,
            exit,
            returned,
            facts.flow.control.calls.span_or_empty(state_flow.calls),
        )
    }

    fn matching_exit_return_call<'facts>(
        program: &dyn ::validation::CheckingInput,
        exit: &FlowExitFact,
        returned: ExpressionHandle,
        calls: impl IntoIterator<Item = &'facts FlowCallFact>,
    ) -> Option<&'facts FlowCallFact> {
        let mut matching = calls
            .into_iter()
            .filter(|call| call.statement_index == exit.statement_index)
            .filter(|call| {
                let Some(site) = validation::find_call_site(
                    program,
                    exit.machine_symbol,
                    exit.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                ) else {
                    return false;
                };
                match site {
                    validation::CallSite::Expression {
                        expression,
                        call: selected,
                    } => {
                        let selected_target = validation::call_target_symbol(program, selected);
                        returned.is_valid()
                            && expression == returned
                            && call.authored_expression == returned
                            && call.target_symbol
                                == validation::call_target_state(program, selected_target)
                                    .map_or(selected_target, |state| state.symbol)
                    }
                    validation::CallSite::TransitionNamed { path: selected, .. }
                        if exit.transition_target.is_valid() =>
                    {
                        let TransitionTargetNode::Named { path, .. } = program
                            .statement_table
                            .transition_target(exit.transition_target)
                        else {
                            return false;
                        };
                        std::ptr::eq(path, selected)
                            && call.target_symbol
                                == validation::call_target_state(program, path.symbol)
                                    .map_or(path.symbol, |state| state.symbol)
                            && crate::published::proof::contract_target_from_state_symbol(
                                program,
                                path.symbol,
                            )
                            .is_some()
                            && !site.is_internal_state_transfer(program, exit.machine_symbol)
                    }
                    _ => false,
                }
            });
        let call = matching.next()?;
        matching.next().is_none().then_some(call)
    }

    pub(super) fn exit_return_expression(
        program: &dyn ::validation::CheckingInput,
        exit: &FlowExitFact,
    ) -> ExpressionHandle {
        let Some(state) = validation::find_state_for_exit(program, exit) else {
            return Default::default();
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        match statements.get(exit.statement_index) {
            Some(StatementNode::Expression(value))
                if !exit.transition_target.is_valid()
                    && exit.statement_index.checked_add(1) == Some(statements.len()) =>
            {
                *value
            }
            Some(StatementNode::Transition(transition))
                if exit.transition_target.is_valid()
                    && [transition.target, transition.continuation]
                        .contains(&exit.transition_target)
                    && transition.exit == TransitionExit::Ordinary =>
            {
                match program
                    .statement_table
                    .transition_target(exit.transition_target)
                {
                    TransitionTargetNode::Value(value) => *value,
                    _ => Default::default(),
                }
            }
            _ => Default::default(),
        }
    }

    /*
        The authored occurrence an exit diagnostic should point at: the arm's own
        call target or returned value, else the statement's authored span.
        Compiler-generated occurrences keep the default span, which names no
        source, so they yield `None` rather than an invented location.
    */
    pub(super) fn exit_source_span(
        program: &dyn ::validation::CheckingInput,
        exit: &FlowExitFact,
    ) -> Option<source::SourceSpan> {
        let state = validation::find_state_for_exit(program, exit)?;
        let statement = program
            .statement_table
            .statements(state.statement_nodes)
            .get(exit.statement_index)?;
        let arm = match statement {
            StatementNode::Transition(_) if exit.transition_target.is_valid() => {
                match program
                    .statement_table
                    .transition_target(exit.transition_target)
                {
                    TransitionTargetNode::Named { source_span, .. } => Some(*source_span),
                    TransitionTargetNode::Value(value) => {
                        program.expression_table.try_source_span(*value)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        arm.filter(|span| *span != source::SourceSpan::default())
            .or_else(|| validation::statement_source_span(program, statement))
    }

    pub(super) fn is_result_reference(
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(entry) = program.machine_states(machine).first() else {
            return false;
        };
        language_core::signature_reserves_result(
            program
                .state_parameters(entry)
                .iter()
                .map(|parameter| parameter.name.as_str()),
        ) && program.expression_table.names_reserved_result(expression)
    }
}

use calls::check_call_requires;
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
pub(crate) use evaluator::call_site_boolean_contract_expression_value;
pub(crate) use evidence::{
    exact_runtime_evidence_parameters, exact_target_evidence_parameters,
    instantiate_contract_expression_evidence_parameter,
};
pub(crate) use exits::exact_scalar_membership;
use exits::{CyclicHeaderInvariants, check_exit_ensures};
pub(crate) use exits::{
    exclusive_referent_type, is_readable_mutable_reference, value_provable_domain,
};
use writes::check_domain_field_writes;

pub(super) struct ContractDecisions {
    pub(super) scalar_returns: Vec<checked_trees::CheckedScalarReturnQualification>,
    pub(super) constant_obligations: Vec<checked_trees::ConstantPredicateObligation>,
    pub(super) recipe_obligations: Vec<checked_trees::ConstantRecipeObligation>,
    pub(super) integer_range_obligations: Vec<checked_trees::IntegerRangeObligation>,
    pub(super) call_requires: Vec<checked_trees::DecidedCallRequires>,
    pub(super) value_qualifications: arena::Arena<checked_trees::CheckedValueQualification>,
}

pub(super) fn check_flow_call_contracts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    incoming_guards: &crate::checks::ranges::incoming_guards::IncomingGuardIndex,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    entailment: &mut ProvenExitExpressions<'_, '_>,
) -> Result<ContractDecisions, Vec<Diagnostic>> {
    let mut diagnostics =
        crate::published::facts::contextual_cleanup::check_cleanup_requirements(program, facts);
    let mut decided = Vec::new();
    let mut scalar_returns = Vec::new();
    let mut constant_obligations = Vec::new();
    let mut recipe_obligations = Vec::new();
    let mut integer_range_obligations = Vec::new();
    let mut value_qualifications = arena::Arena::default();
    let constructor_memberships = writes::ConstructorMemberships::new(facts);
    let content_plans = validation::content_conservation_plans(program);
    let nominal_requirements = nominal_inputs::DeclaredFieldRequirements::new(&facts.semantic);
    let mut owned_call_frames = None;
    let call_frames =
        crate::flow::shared_call_frames_or(call_frames, program, &mut owned_call_frames);

    {
        assembly::check_assembly_fact_contracts(program, facts, &mut diagnostics);
    };

    let proof_only = validation::proof_only_classification(program);
    let mut cyclic_headers = CyclicHeaderInvariants::new();
    let boundary_refutations = exits::BoundaryRefutations::new(program);
    let is_proof_machine = |symbol: symbols::SymbolHandle| {
        program
            .machine_by_own_or_state_symbol(symbol)
            .is_some_and(|machine| proof_only.is_proof_machine(program, machine))
    };

    let struct_trace = ::validation::trace_flags::struct_trace();
    let has_activation_claim = program
        .data_definitions()
        .iter()
        .any(|data| validation::activation_claim_byte_carrier(program, data.symbol).is_some());

    let mut structural_values_by_state = symbols::SymbolKeyMap::<
        symbols::SymbolHandle,
        Vec<&checked_trees::CheckedStructuralValue>,
    >::default();
    for (_, value) in facts.values.structural_values.nodes.iter() {
        if matches!(
            value.kind,
            checked_trees::CheckedStructuralValueKind::ScalarArray { .. }
                | checked_trees::CheckedStructuralValueKind::FixedArray { .. }
                | checked_trees::CheckedStructuralValueKind::Record { .. }
                | checked_trees::CheckedStructuralValueKind::Case(_)
        ) {
            structural_values_by_state
                .entry(value.state)
                .or_default()
                .push(value);
        }
    }

    let state_flows: Vec<&checked_trees::FlowStateFact> = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, state_flow)| state_flow)
        .collect();

    let per_state_call_and_write_results: Vec<(
        Vec<checked_trees::DecidedCallRequires>,
        Vec<checked_trees::ConstantPredicateObligation>,
        Vec<checked_trees::ConstantRecipeObligation>,
        arena::Arena<checked_trees::CheckedValueQualification>,
        Vec<Diagnostic>,
    )> = validation::checking_thread_pool().install(|| {
        use rayon::prelude::*;
        state_flows
            .par_iter()
            .map(|&state_flow| {
                let mut state_decided = Vec::new();
                let mut state_constant_obligations = Vec::new();
                let mut state_recipe_obligations = Vec::new();
                let mut state_value_qualifications = arena::Arena::default();
                let mut state_diagnostics = Vec::new();
                let caller_is_proof = struct_trace && is_proof_machine(state_flow.machine_symbol);
                let mut guarantees =
                    crate::checks::contracts::prover::call_guarantees::StateGuarantees::new(
                        program,
                        facts.view(),
                        state_flow,
                        call_frames,
                    );
                for call_flow in facts.flow.control.calls.span_or_empty(state_flow.calls) {
                    if struct_trace {
                        eprintln!(
                            "CALLREQ caller={} proof={} target={} proof={}",
                            crate::published::labels::machine_name(
                                program,
                                state_flow.machine_symbol
                            ),
                            caller_is_proof,
                            crate::published::labels::call_target_label(
                                program,
                                call_flow.target_symbol
                            ),
                            is_proof_machine(call_flow.target_symbol),
                        );
                    }
                    exits::check_state_transfer_result_domains(
                        program,
                        state_flow,
                        call_flow,
                        &mut state_diagnostics,
                    );
                    let before = state_diagnostics.len();
                    check_call_requires(
                        program,
                        facts,
                        &mut guarantees,
                        call_flow,
                        &nominal_requirements,
                        incoming_guards.for_machine(state_flow.machine_symbol),
                        call_frames,
                        &mut state_constant_obligations,
                        &mut state_recipe_obligations,
                        &mut state_value_qualifications,
                        &mut state_diagnostics,
                    );
                    if state_diagnostics.len() == before
                        && let Ok(statement_index) = u32::try_from(call_flow.statement_index)
                    {
                        state_decided.push(checked_trees::DecidedCallRequires {
                            state: state_flow.state_symbol,
                            statement_index,
                            target: call_flow.target_symbol,
                        });
                    }
                }
                let state_structural_values = structural_values_by_state
                    .get(&state_flow.state_symbol)
                    .map_or(&[][..], Vec::as_slice);
                {
                    check_domain_field_writes(
                        program,
                        facts,
                        &constructor_memberships,
                        state_flow,
                        state_structural_values,
                        call_frames,
                        &mut state_value_qualifications,
                        &mut state_diagnostics,
                    );
                };
                (
                    state_decided,
                    state_constant_obligations,
                    state_recipe_obligations,
                    state_value_qualifications,
                    state_diagnostics,
                )
            })
            .collect()
    });

    for (
        state_flow,
        (
            state_decided,
            state_constant_obligations,
            state_recipe_obligations,
            state_value_qualifications,
            state_diagnostics,
        ),
    ) in state_flows
        .into_iter()
        .zip(per_state_call_and_write_results)
    {
        decided.extend(state_decided);
        constant_obligations.extend(state_constant_obligations);
        recipe_obligations.extend(state_recipe_obligations);
        for (_, qualification) in state_value_qualifications.iter() {
            value_qualifications.append(*qualification);
        }
        diagnostics.extend(state_diagnostics);
        for exit_flow in facts.flow.control.exits.span_or_empty(state_flow.exits) {
            if has_activation_claim {
                return_values::check_activation_return(
                    program,
                    facts,
                    state_flow,
                    exit_flow,
                    &mut diagnostics,
                );
            }
            boundary_refutations.check(program, facts, exit_flow, call_frames, &mut diagnostics);
            exits::check_scalar_result_domains(
                program,
                facts,
                state_flow,
                exit_flow,
                &mut scalar_returns,
                &mut diagnostics,
            );
            {
                let available_content = entailment.content_sources();
                check_exit_ensures(
                    program,
                    facts,
                    state_flow,
                    exit_flow,
                    entailment.for_machine(facts, state_flow.machine_symbol),
                    &content_plans,
                    &available_content,
                    call_frames,
                    &mut cyclic_headers,
                    &mut recipe_obligations,
                    &mut integer_range_obligations,
                    &mut diagnostics,
                )
            };
            let exit_diagnostics = diagnostics.len();
            exits::check_result_field_domains(
                program,
                facts,
                state_flow,
                exit_flow,
                call_frames,
                &value_qualifications,
                &mut diagnostics,
            );
            exits::check_exclusive_referent_field_domains(
                program,
                facts,
                exit_flow,
                &mut diagnostics,
            );
            let mut reported = Vec::new();
            for diagnostic in diagnostics.split_off(exit_diagnostics) {
                crate::shared::collections::push_unique(&mut reported, diagnostic);
            }
            diagnostics.extend(reported);
        }
    }

    if diagnostics.is_empty() {
        writes::check_routed_cast_coverage(program, &value_qualifications, &mut diagnostics);
    }
    if diagnostics.is_empty() {
        Ok(ContractDecisions {
            scalar_returns,
            constant_obligations,
            recipe_obligations,
            integer_range_obligations,
            call_requires: decided,
            value_qualifications,
        })
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn bind_call_evidence_arguments(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    evidence::bind_contract_expression_evidence_arguments(program, facts, &mut diagnostics);
    evidence::bind_call_evidence_arguments(program, facts, &mut diagnostics);
    crate::checks::finish(diagnostics)
}
