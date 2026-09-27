//! Exact source, qualification, and claim joins remain independent of cleanup.

use crate::tests::fixtures::dominance::refresh_function_derivatives;
use crate::tests::support::id;
use abstract_operations::AbstractOperation;
use semantic_vocabulary::ClaimId;

use super::fixtures::{field, mixed_unit};
use crate::unit_validation::operation_contracts::validate_internal_claim_transfers;
use crate::validate_psi_optimization_unit;

#[test]
fn general_partial_affine_retains_claim_transfer_joins() {
    let baseline = mixed_unit();
    let AbstractOperation::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &baseline.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    assert!(validate_internal_claim_transfers(
        &baseline.functions[0],
        &baseline.functions[1],
        structural_arguments,
        claim_transfers,
    ));
    let mut caller = baseline.functions[0].clone();
    caller
        .entry_claim_declarations
        .push(terminal_psi::EntryClaim {
            claim: id(1, ClaimId::new),
            input: caller.structural_parameters[0].place,
            path: structural_arguments[0].path.clone(),
        });
    assert!(
        !validate_internal_claim_transfers(
            &caller,
            &baseline.functions[1],
            structural_arguments,
            claim_transfers,
        ),
        "a claimed source projection cannot join a claim-free disposer"
    );
    let mut callee = baseline.functions[1].clone();
    callee
        .entry_claim_declarations
        .push(terminal_psi::EntryClaim {
            claim: id(2, ClaimId::new),
            input: callee.structural_parameters[0].place,
            path: Vec::new(),
        });
    assert!(
        !validate_internal_claim_transfers(&caller, &callee, structural_arguments, claim_transfers,),
        "matching paths still require an exact transfer row"
    );
}

#[test]
fn general_partial_affine_rejects_claimed_live_residuals() {
    let mut changed = mixed_unit();
    let claim = id(1, ClaimId::new);
    let root = changed.functions[0].structural_parameters[0].place;
    changed.functions[0]
        .entry_claim_declarations
        .push(terminal_psi::EntryClaim {
            claim,
            input: root,
            path: vec![field("tail")],
        });
    changed.functions[0].entry_claims.insert(claim);
    refresh_function_derivatives(&mut changed, 0);
    assert!(
        validate_psi_optimization_unit(&changed).is_err(),
        "claim-free cleanup cannot erase custody on an untouched residual"
    );
}
