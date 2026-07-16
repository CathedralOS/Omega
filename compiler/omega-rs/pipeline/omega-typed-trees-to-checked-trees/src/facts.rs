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
    )
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
        // STR4 slice 3: the honest declaration-free direct summary.
        let inferred_direct = intern_set(machine_effects.body_observed);
        let inferred_transitive = intern_set(machine_effects.transitive);
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
