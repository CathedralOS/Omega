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
    // Domain-owned spelled candidates can only be admitted by PROVEN domain
    // facts (chapter 8), and proof contexts exist only now that flow facts are
    // built: finalize every pending spelled binary use before the checks read
    // the operator evidence.
    select_pending_domain_operator_meanings(program, &mut operators, &mut semantic, &flow);
    let capabilities = build_capability_facts(program, &effects, &flow);
    // TPR3 slice 4: the checker-established termination summaries (built
    // from the same pure functions the termination CHECK uses -- facts and
    // diagnostics cannot disagree).
    let termination = crate::checks::termination::build_termination_facts(program);
    // STR4 slice 2: kinded effect rows (published ceiling vs inferred).
    let effect_rows = build_effect_row_facts(program, &effects);
    // STR4 checked plans, slice 2: semantic-domain commitments per machine.
    let qualifications = build_qualification_facts(program);

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
    )
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
                        for argument in
                            program.statement_table.expression_handles(call.arguments)
                        {
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
