use crate::borrow::build_borrow_facts;
use crate::checks;
use crate::flow::{build_domain_facts, build_flow_facts};
use crate::invariants::build_invariant_facts;
use crate::proof::build_proof_facts;
use crate::semantic::build_semantic_facts;
use omega_checked_trees::{CheckFacts, CheckedTrees};

pub(crate) fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    omega_validation::validate_program(&program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;
    let effects = omega_effects::infer_effects(&program);
    omega_validation::validate_effect_plan(&program, &effects)?;
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let invariants = build_invariant_facts(&program);
    let mut semantic = build_semantic_facts(&program, &proof);
    let domains = build_domain_facts(&program, &semantic);
    let flow = build_flow_facts(&program, &borrow, &proof, &mut semantic, &domains, &effects);
    let facts = CheckFacts {
        semantic,
        proof,
        borrow,
        invariants,
        domains,
        effects,
        flow,
    };
    checks::check_checked_facts(&program, &facts)?;

    Ok(CheckedTrees {
        typed: program,
        facts,
    })
}
