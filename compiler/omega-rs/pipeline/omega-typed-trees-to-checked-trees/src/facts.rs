use crate::borrow::build_borrow_facts;
use crate::capabilities::build_capability_facts;
use crate::flow::{build_domain_facts, build_flow_facts};
use crate::invariants::build_invariant_facts;
use crate::operators::{build_operator_facts, select_pending_domain_operator_meanings};
use crate::proof::build_proof_facts_with_operators;
use crate::semantic::build_semantic_facts;
use crate::values::build_value_facts;
use omega_checked_trees::CheckFacts;
use omega_effects::EffectPlan;
use omega_proof::obligations::ProofPlan;
use omega_typed_trees::TypedTrees;

pub(crate) fn build_check_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
    effects: EffectPlan,
) -> CheckFacts {
    let borrow = build_borrow_facts(program);
    let values = build_value_facts(program);
    let mut operators = build_operator_facts(program, &values);
    let proof = build_proof_facts_with_operators(program, proof_plan, &borrow, &operators);
    let invariants = build_invariant_facts(program);
    let mut semantic = build_semantic_facts(program, &proof);
    let domains = build_domain_facts(program, &semantic);
    let flow = build_flow_facts(program, &borrow, &proof, &mut semantic, &domains, &effects);
    // Domain-owned meanings are selected only from declarations, mints, and
    // signature `requires`; the selector accepts no flow/fact environment.
    select_pending_domain_operator_meanings(program, &mut operators);
    let capabilities = build_capability_facts(program, &effects, &flow);
    // TPR3 slice 4: the checker-established termination summaries (built
    // from the same pure functions the termination CHECK uses -- facts and
    // diagnostics cannot disagree).
    let termination = crate::checks::termination::build_termination_facts(program);
    // STR4 slice 2: kinded effect rows (published ceiling vs inferred).
    let effect_rows = build_effect_row_facts(program, &effects);
    // STR4 checked plans, slice 2: semantic-domain commitments per machine.
    let qualifications = build_qualification_facts(program);
    // STR4 checked plans: the normalized machine contracts (published
    // halves + fingerprint; prover-independent by construction).
    let contract_plans = build_contract_plans(program, &effect_rows);
    // CRY1: materialize the effective structural policy once in the checked
    // fact layer; authored clauses remain minimum promises on typed data.
    let carry = build_carry_facts(program);

    CheckFacts::with_roots(
        semantic,
        borrow,
        proof,
        values,
        invariants,
        domains,
        operators,
        effects,
        capabilities,
        flow,
        termination,
        effect_rows,
        qualifications,
        contract_plans,
        carry,
    )
}

fn build_carry_facts(program: &TypedTrees) -> omega_checked_trees::CarryFacts {
    let data = program
        .data_definitions()
        .iter()
        .map(|definition| omega_checked_trees::DataCarryFact {
            data: definition.symbol,
            declared: definition.properties.carry,
            effective: omega_validation::effective_data_carry_policy(program, definition),
        })
        .collect();
    omega_checked_trees::CarryFacts {
        data,
        suspension_crossings: Vec::new(),
        asynchronous_preemption: Vec::new(),
    }
}

/// STR4 checked plans (machine_taxonomy.md): assemble each machine's
/// normalized contract plan from the published halves already carried on
/// the records (supply mode, effect-row ceiling, published termination),
/// with a deterministic fingerprint over them. Only DECLARED material
/// enters -- acceptance 8 (a stronger prover cannot change an exported
/// contract ID) holds by construction.
fn build_contract_plans(
    program: &TypedTrees,
    effect_rows: &omega_checked_trees::EffectRowFacts,
) -> omega_checked_trees::MachineContractPlans {
    let mut machines = Vec::new();
    let frame_resolver = omega_validation::CallFrameResolver::new(program);
    for machine in program.machines() {
        let published_effect_row = machine.effect_row;
        let members = effect_rows.rows.members(published_effect_row).to_vec();
        let published_termination = machine
            .termination_plan
            .published
            .clone()
            .unwrap_or_default();
        // Slice 2: the declared requires/ensures facts in a CANONICAL,
        // clause-order-independent encoding (each fact serializes to a
        // stable byte form; the set sorts before folding). Parameter
        // RENAMES change the identity in v1 -- positional normalization is
        // the recorded follow-up.
        let mut canonical_facts: Vec<Vec<u8>> = Vec::new();
        // The callable shape is contract identity too. A selected static
        // machine changing parameter mode/type, result type, or state surface
        // must invalidate every specialization that recorded its contract ID.
        // Encode generic binders positionally so a rename remains invisible.
        let generic_binders: Vec<(String, String)> = program
            .machine_type_parameters(machine)
            .iter()
            .enumerate()
            .map(|(index, parameter)| (parameter.name.as_str().to_owned(), format!("$G{index}")))
            .collect();
        for state in program.machine_states(machine) {
            let mut encoded = vec![0xa0];
            let state_parameters = program.state_parameters(state);
            for parameter in state_parameters {
                encoded.push(u8::from(parameter.is_self));
                encoded.push(u8::from(parameter.is_mutable));
                encoded.push(u8::from(parameter.is_const));
                encode_type_spelling(
                    &program.display_type_reference(parameter.type_reference),
                    &generic_binders,
                    &mut encoded,
                );
            }
            encoded.push(0xaf);
            encode_type_spelling(
                &program.display_type_reference(state.return_type),
                &generic_binders,
                &mut encoded,
            );
            let parameter_names = state_parameters
                .iter()
                .map(|parameter| parameter.name.as_str().to_owned())
                .collect::<Vec<_>>();
            let mut state_contracts = Vec::new();
            for contract in program.state_contracts(state) {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let mut contract_bytes = vec![0xae];
                    contract_bytes.push(match contract.kind {
                        omega_typed_trees::signature::SignatureContractKind::Requires => 1,
                        omega_typed_trees::signature::SignatureContractKind::Ensures => 2,
                        omega_typed_trees::signature::SignatureContractKind::Boundary => 3,
                    });
                    match fact {
                        omega_typed_trees::domain::ProofFact::Expression(expression) => {
                            contract_bytes.push(1);
                            encode_expression_canonical(
                                program,
                                *expression,
                                &parameter_names,
                                &mut contract_bytes,
                            );
                        }
                        omega_typed_trees::domain::ProofFact::Membership(membership) => {
                            contract_bytes.push(2);
                            encode_expression_canonical(
                                program,
                                membership.value,
                                &parameter_names,
                                &mut contract_bytes,
                            );
                            contract_bytes.push(0);
                            for member in program.domain_path_members(membership.domain) {
                                contract_bytes.extend(member.as_str().as_bytes());
                                contract_bytes.push(b':');
                            }
                        }
                    }
                    state_contracts.push(contract_bytes);
                }
            }
            state_contracts.sort();
            for contract in state_contracts {
                encoded.extend(contract);
                encoded.push(0xad);
            }
            canonical_facts.push(encoded);
        }
        // Positional parameter normalization: a contract fact naming the
        // machine's Nth parameter encodes as P<N>, so RENAMES never change
        // the identity (the substitutable contract is positional).
        let parameter_names: Vec<String> = program
            .machine_states(machine)
            .first()
            .map(|entry| {
                program
                    .state_parameters(entry)
                    .iter()
                    .map(|parameter| parameter.name.as_str().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        for contract in program.machine_contracts(machine) {
            let kind_tag: u8 = match contract.kind {
                omega_typed_trees::signature::SignatureContractKind::Requires => 1,
                omega_typed_trees::signature::SignatureContractKind::Ensures => 2,
                omega_typed_trees::signature::SignatureContractKind::Boundary => 3,
            };
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let mut encoded = vec![kind_tag];
                match fact {
                    omega_typed_trees::domain::ProofFact::Expression(expression) => {
                        encoded.push(1);
                        encode_expression_canonical(
                            program,
                            *expression,
                            &parameter_names,
                            &mut encoded,
                        );
                    }
                    omega_typed_trees::domain::ProofFact::Membership(membership) => {
                        encoded.push(2);
                        encoded.extend(
                            program
                                .expression_table
                                .display_name(membership.value)
                                .as_bytes(),
                        );
                        encoded.push(0);
                        for member in program
                            .expression_table
                            .name_path_members(membership.domain)
                        {
                            encoded.extend(member.as_str().as_bytes());
                            encoded.push(b':');
                        }
                    }
                }
                canonical_facts.push(encoded);
            }
        }
        canonical_facts.sort();
        let fingerprint = omega_checked_trees::contract_fingerprint(
            machine.supply_mode,
            published_effect_row,
            &members,
            &published_termination,
            &canonical_facts,
        );
        let inferred_write_frames = program
            .machine_states(machine)
            .iter()
            .map(|state| omega_checked_trees::StateWriteFramePlan {
                state: state.symbol,
                frame: frame_resolver
                    .as_ref()
                    .map_or_else(omega_facts::NormalizedWriteFrame::opaque, |resolver| {
                        resolver.inferred_state_write_frame(machine, state)
                    }),
            })
            .collect();
        machines.push(omega_checked_trees::MachineContractPlan {
            machine: machine.symbol,
            supply_mode: machine.supply_mode,
            published_effect_row,
            published_termination,
            inferred_write_frames,
            fingerprint,
        });
    }
    omega_checked_trees::MachineContractPlans {
        machines,
        task_activations: Vec::new(),
    }
}

fn encode_type_spelling(text: &str, binders: &[(String, String)], output: &mut Vec<u8>) {
    let mut word = String::new();
    let flush = |word: &mut String, output: &mut Vec<u8>| {
        if word.is_empty() {
            return;
        }
        if let Some((_, replacement)) = binders.iter().find(|(name, _)| name == word) {
            output.extend(replacement.as_bytes());
        } else {
            output.extend(word.as_bytes());
        }
        word.clear();
    };
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            word.push(character);
        } else {
            flush(&mut word, output);
            output.extend(character.to_string().as_bytes());
        }
    }
    flush(&mut word, output);
    output.push(0);
}

/// STR4 checked plans, slice 2 (decision 19): collect each machine's
/// semantic-domain COMMITMENTS -- v1 walks its statements' expressions for
/// arithmetic-policy casts (`x as u8 in Saturating`; the compiler-blessed
/// closed semantic-facet subset) and normalizes the policy to its FIXED
/// SemanticDomainTable identity. Sorted + deduped; cast-free machines carry
/// no entry.
fn build_qualification_facts(program: &TypedTrees) -> omega_checked_trees::QualificationFacts {
    use omega_core::semantics::SemanticDomainTable;
    use omega_typed_trees::expression::ExpressionNode;

    fn collect_casts(
        program: &TypedTrees,
        expression: omega_typed_trees::expression::ExpressionHandle,
        committed: &mut Vec<omega_core::semantics::SemanticDomainId>,
    ) {
        if !expression.is_valid() {
            return;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Cast(cast) => {
                let policy = match cast.domain {
                    omega_core::arithmetic::ArithmeticDomain::Exact => None,
                    omega_core::arithmetic::ArithmeticDomain::Wrapping => {
                        Some(SemanticDomainTable::WRAPPING)
                    }
                    omega_core::arithmetic::ArithmeticDomain::Saturating => {
                        Some(SemanticDomainTable::SATURATING)
                    }
                    omega_core::arithmetic::ArithmeticDomain::Trapping => {
                        Some(SemanticDomainTable::TRAPPING)
                    }
                };
                if let Some(policy) = policy {
                    committed.push(policy);
                }
                // A DECLARED-domain qualification (the mint, decision 19):
                // the cast carries the domain's SHORT name; resolve it to
                // the declaration (exact or `Carrier::Name` suffix -- the
                // validation judge's rule) and take its interned identity.
                // Compiled programs only carry ACCEPTED mints -- a failed
                // mint refuses at validation.
                if cast.semantic_domain.count() > 0
                    && let Some(name) = program
                        .expression_table
                        .name_path_members(cast.semantic_domain)
                        .first()
                    && let Some(domain) = program.domain_definitions().iter().find(|domain| {
                        domain.name.as_str() == name.as_str()
                            || domain
                                .name
                                .as_str()
                                .ends_with(&format!("::{}", name.as_str()))
                    })
                {
                    if let Some(semantic_id) = domain.facets.semantic {
                        committed.push(semantic_id);
                    }
                }
                collect_casts(program, cast.value, committed);
            }
            ExpressionNode::Binary(binary) => {
                collect_casts(program, binary.left, committed);
                collect_casts(program, binary.right, committed);
            }
            ExpressionNode::Unary(unary) => collect_casts(program, unary.operand, committed),
            ExpressionNode::Member(member) => collect_casts(program, member.receiver, committed),
            ExpressionNode::Mutable(inner) => collect_casts(program, *inner, committed),
            ExpressionNode::Indexed(indexed) => {
                collect_casts(program, indexed.collection, committed);
                collect_casts(program, indexed.index, committed);
            }
            ExpressionNode::Range(range) => {
                collect_casts(program, range.start, committed);
                collect_casts(program, range.end, committed);
            }
            ExpressionNode::Call(call) => {
                collect_casts(program, call.receiver, committed);
                for argument in program.expression_table.expression_handles(call.arguments) {
                    collect_casts(program, *argument, committed);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in program.expression_table.struct_fields(literal.fields) {
                    collect_casts(program, field.value, committed);
                }
            }
            ExpressionNode::ArrayLiteral(items) => {
                for item in program.expression_table.expression_handles(*items) {
                    collect_casts(program, *item, committed);
                }
            }
            _ => {}
        }
    }

    let mut machines = Vec::new();
    for machine in program.machines() {
        let mut committed = Vec::new();
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                use omega_typed_trees::statement::StatementNode;
                match statement {
                    StatementNode::AssemblyFact(_) => {}
                    StatementNode::Assignment(assignment) => {
                        collect_casts(program, assignment.target, &mut committed);
                        collect_casts(program, assignment.value, &mut committed);
                    }
                    StatementNode::Expression(expression) => {
                        collect_casts(program, *expression, &mut committed);
                    }
                    StatementNode::LocalData(local) => {
                        collect_casts(program, local.initial_value, &mut committed);
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            collect_casts(program, *argument, &mut committed);
                        }
                    }
                    StatementNode::Transition(transition) => {
                        if let omega_typed_trees::statement::TransitionGuardNode::When(guard) =
                            &transition.guard
                        {
                            collect_casts(program, *guard, &mut committed);
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                omega_typed_trees::statement::TransitionTargetNode::Value(
                                    value,
                                ) => collect_casts(program, *value, &mut committed),
                                omega_typed_trees::statement::TransitionTargetNode::Named {
                                    arguments,
                                    ..
                                } => {
                                    for argument in
                                        program.statement_table.expression_handles(*arguments)
                                    {
                                        collect_casts(program, *argument, &mut committed);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
        committed.sort_by_key(|id| id.0);
        committed.dedup();
        if !committed.is_empty() {
            machines.push(omega_checked_trees::MachineQualifications {
                machine: machine.symbol,
                body_committed: committed,
            });
        }
    }
    omega_checked_trees::QualificationFacts { machines }
}

/// STR4 slice 2 (decision 22): build the kinded effect-row facts. The
/// typed trees' interner extends (prefix-stable, so machine `effect_row`
/// ids stay valid) with rows for the checker-INFERRED direct/transitive
/// summaries. The bit->member hop goes through the CANONICAL NAME, never
/// the bit value; the omega-effects consistency pin holds the
/// correspondence.
fn build_effect_row_facts(
    program: &TypedTrees,
    effects: &EffectPlan,
) -> omega_checked_trees::EffectRowFacts {
    let mut rows = program.effect_rows.clone();
    let mut intern_set = |set: omega_effects::EffectSet| {
        let members: Vec<omega_core::semantics::EffectMemberId> = set
            .names()
            .filter_map(omega_core::semantics::effect_member_id)
            .collect();
        rows.intern(members)
    };
    let mut machines = Vec::new();
    for machine_effects in effects.machines() {
        // STR4 slice 3 + seed rework: the honest declaration-free
        // summaries (the authored clause lives in published_ceiling; the
        // inferred rows carry only what the body observes and reaches).
        let inferred_direct = intern_set(machine_effects.body_observed);
        let inferred_transitive = intern_set(machine_effects.body_transitive);
        let published_ceiling = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_effects.symbol)
            .map(|machine| machine.effect_row)
            .unwrap_or(omega_core::semantics::EffectRowId::NULL);
        machines.push(omega_checked_trees::MachineEffectRows {
            machine: machine_effects.symbol,
            published_ceiling,
            inferred_direct,
            inferred_transitive,
        });
    }
    omega_checked_trees::EffectRowFacts { rows, machines }
}

/// A stable, spelling-independent byte encoding of a contract fact
/// expression: prefix walk with operator tags, name paths as text, integer
/// literals as text (exact at any magnitude). Deterministic across
/// programs for the same declared clause.
fn encode_expression_canonical(
    program: &TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
    parameter_names: &[String],
    out: &mut Vec<u8>,
) {
    use omega_typed_trees::expression::ExpressionNode;
    if !expression.is_valid() {
        out.push(0);
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            out.push(1);
            out.push(binary.operator as u8);
            encode_expression_canonical(program, binary.left, parameter_names, out);
            encode_expression_canonical(program, binary.right, parameter_names, out);
        }
        ExpressionNode::Unary(unary) => {
            out.push(2);
            out.push(unary.operator as u8);
            encode_expression_canonical(program, unary.operand, parameter_names, out);
        }
        ExpressionNode::Integer(value) => {
            out.push(3);
            out.extend(value.text().as_bytes());
            out.push(0);
        }
        ExpressionNode::Boolean(value) => {
            out.push(4);
            out.push(u8::from(*value));
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            // A bare parameter name normalizes to its POSITION -- renames
            // never change the contract identity.
            if let [single] = members
                && let Some(index) = parameter_names
                    .iter()
                    .position(|name| name == single.as_str())
            {
                out.push(9);
                out.extend(
                    u32::try_from(index)
                        .expect("parameter index fits u32")
                        .to_le_bytes(),
                );
                return;
            }
            out.push(5);
            for member in members {
                out.extend(member.as_str().as_bytes());
                out.push(b'.');
            }
            out.push(0);
        }
        ExpressionNode::Member(member) => {
            out.push(6);
            encode_expression_canonical(program, member.receiver, parameter_names, out);
            out.extend(member.member.as_str().as_bytes());
            out.push(0);
        }
        ExpressionNode::Call(call) => {
            out.push(7);
            out.extend(call.target.as_str().as_bytes());
            out.push(0);
            encode_expression_canonical(program, call.receiver, parameter_names, out);
            for argument in program.expression_table.expression_handles(call.arguments) {
                encode_expression_canonical(program, *argument, parameter_names, out);
            }
            out.push(0xfe);
        }
        // Anything else falls back to the display name -- stable per
        // spelling (a conservative widening; refine per-node as shapes
        // arrive in contracts).
        other => {
            let _ = other;
            out.push(8);
            out.extend(program.expression_table.display_name(expression).as_bytes());
            out.push(0);
        }
    }
}
