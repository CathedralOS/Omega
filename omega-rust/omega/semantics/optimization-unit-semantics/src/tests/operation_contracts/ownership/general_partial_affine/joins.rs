//! Exact source, qualification, and claim joins remain independent of cleanup.

use abstract_operations::AbstractOperation;
use semantic_vocabulary::{ClaimId, StructuralDomainId};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralPathQualification};

use super::fixtures::*;
use crate::tests::{id, refresh_function_derivatives};
use crate::{
    StructuralProjectionPolicy, structural_arguments_match, validate_internal_claim_transfers,
    validate_psi_optimization_unit,
};

#[test]
fn general_partial_affine_retains_exact_access_and_qualification_joins() {
    let baseline = mixed_unit();
    let types = baseline
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &baseline.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    let matches = |caller: &optimization_unit::PsiOptimizationFunction, parameters: &[_]| {
        structural_arguments_match(
            caller,
            structural_arguments,
            parameters,
            &types,
            StructuralProjectionPolicy::Unit,
            false,
        )
    };
    let domain = id(1, StructuralDomainId::new);
    assert!(matches(
        &baseline.functions[0],
        &baseline.functions[1].structural_parameters
    ));
    for mutation in 0..4 {
        let mut caller = baseline.functions[0].clone();
        let mut parameters = baseline.functions[1].structural_parameters.clone();
        match mutation {
            0 => caller.structural_parameters[0].access = StructuralAccess::SharedBorrow,
            1 => caller.structural_parameters[0].multiplicity = StructuralMultiplicity::Linear,
            2 => caller.structural_parameters[0].qualifications.push(domain),
            3 => parameters[0].qualifications.push(domain),
            _ => unreachable!(),
        }
        assert!(
            !matches(&caller, &parameters),
            "source/qualification mutation {mutation}"
        );
    }
    let mut caller = baseline.functions[0].clone();
    let mut parameters = baseline.functions[1].structural_parameters.clone();
    parameters[0].qualifications.push(domain);
    caller.structural_parameters[0]
        .projected_qualifications
        .push(StructuralPathQualification {
            path: structural_arguments[0].path.clone(),
            domain,
        });
    assert!(
        matches(&caller, &parameters),
        "the general matcher still joins exact projected qualification"
    );
    caller.structural_parameters[0].projected_qualifications[0].path[1] = index(1);
    assert!(
        !matches(&caller, &parameters),
        "another subtree cannot supply the qualification"
    );
}

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
