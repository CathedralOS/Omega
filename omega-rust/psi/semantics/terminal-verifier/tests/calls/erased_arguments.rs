//! Erased-argument lanes on scalar calls: the callee's `erased_scalar_formals`
//! roster admits proof-only terms, the per-call `erased_arguments` lane
//! instantiates them into the contract's `requires`, and the verifier rejects
//! rows that are missing, outside the caller's admitted closure, or
//! substituted for a different actual (PROOF-RELEVANCE-MIGRATION).

use super::{
    AdmissionProfile, ModuleError, ObligationEvidence, ProofBundle, Proposition, ScalarTerm,
    boolean_declaration, boolean_value, call_module, obligation_id, operation_id,
    reconstruct_terminal_obligations, validate_module, value_id, verify_module,
};
use proof_admission::{
    CertificateEnvelope, EvidenceRoute, ProofNode, ProofRule, ProofSystemMarker,
};
use semantic_vocabulary::EvidenceIdentity;
use terminal_psi::OperationKind;

fn erased_call_module() -> TerminalModule {
    let mut module = call_module();
    // The callee's proof-only formal carries the `requires` operand; its dense
    // parameter list is unchanged.
    module.machines[1].contract.erased_scalar_formals = vec![boolean_declaration(value_id(6))];
    module.machines[1].contract.requires = vec![Proposition::Equal(
        boolean_value(6),
        ScalarTerm::boolean(true),
    )];
    module
}

fn erased_lane_mut(module: &mut TerminalModule) -> &mut Vec<ScalarTerm> {
    let OperationKind::Call {
        erased_arguments, ..
    } = &mut module.machines[0].blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    erased_arguments
}

fn axiom_evidence(obligation: u64, conclusion: Proposition) -> ObligationEvidence {
    ObligationEvidence {
        obligation: obligation_id(obligation),
        route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(obligation).unwrap(),
            proof_system_marker: ProofSystemMarker::CURRENT,
            proof: ProofNode {
                conclusion,
                rule: ProofRule::SemanticAxiom { index: 0 },
            },
        }),
    }
}

fn callee_ensures_evidence() -> ObligationEvidence {
    axiom_evidence(2, Proposition::Equal(boolean_value(5), boolean_value(4)))
}

fn call_site_bundle(call_site_conclusion: Proposition) -> ProofBundle {
    ProofBundle {
        evidence: vec![
            axiom_evidence(1, call_site_conclusion),
            callee_ensures_evidence(),
        ],
        ..ProofBundle::default()
    }
}

use terminal_psi::TerminalModule;

#[test]
fn erased_actual_discharges_the_instantiated_requires() {
    let mut module = erased_call_module();
    *erased_lane_mut(&mut module) = vec![boolean_value(1)];

    let reconstructed = reconstruct_terminal_obligations(&module).expect("reconstructed");
    let call_site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation_id(1))
        .expect("call-site requires obligation");
    assert_eq!(
        call_site.obligation.proposition,
        Proposition::Equal(boolean_value(1), ScalarTerm::boolean(true)),
        "the erased actual instantiates the callee requires at the call site"
    );

    validate_module(&module).expect("caller-owned erased actual validates");
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "the requires obligation still needs evidence"
    );
    verify_module(
        &module,
        &call_site_bundle(Proposition::Equal(
            boolean_value(1),
            ScalarTerm::boolean(true),
        )),
        &AdmissionProfile::default(),
    )
    .expect("the instantiated requires is discharged by caller-visible evidence");

    // A proof-only literal may carry the erased lane as well.
    *erased_lane_mut(&mut module) = vec![ScalarTerm::boolean(true)];
    validate_module(&module).expect("literal erased actual validates");
    let reconstructed = reconstruct_terminal_obligations(&module).expect("reconstructed");
    let call_site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation_id(1))
        .expect("call-site requires obligation");
    assert_eq!(
        call_site.obligation.proposition,
        Proposition::Equal(ScalarTerm::boolean(true), ScalarTerm::boolean(true))
    );
}

#[test]
fn erased_actual_violating_requires_rejects_in_source_free_verification() {
    let mut module = erased_call_module();
    *erased_lane_mut(&mut module) = vec![ScalarTerm::boolean(false)];

    let reconstructed = reconstruct_terminal_obligations(&module).expect("reconstructed");
    let call_site = reconstructed
        .obligations()
        .iter()
        .find(|site| site.obligation.id == obligation_id(1))
        .expect("call-site requires obligation");
    assert_eq!(
        call_site.obligation.proposition,
        Proposition::Equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true)),
        "the violating actual is what the call site must prove"
    );

    validate_module(&module).expect("a violating literal is still structurally valid");
    assert!(
        verify_module(
            &module,
            &ProofBundle::default(),
            &AdmissionProfile::default()
        )
        .is_err(),
        "no evidence can discharge `false == true`"
    );
    assert!(
        verify_module(
            &module,
            &call_site_bundle(Proposition::Equal(
                boolean_value(1),
                ScalarTerm::boolean(true),
            )),
            &AdmissionProfile::default()
        )
        .is_err(),
        "evidence for a different instantiation does not discharge the violating actual"
    );
}

#[test]
fn erased_actual_outside_the_admitted_closure_rejects() {
    let mut module = erased_call_module();
    *erased_lane_mut(&mut module) = vec![boolean_value(999)];

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::ErasedCallArgumentUnknownValue {
            operation: operation_id(2),
        }
    );
}

#[test]
fn missing_erased_argument_row_rejects() {
    let mut module = erased_call_module();
    erased_lane_mut(&mut module).clear();

    assert_eq!(
        validate_module(&module).unwrap_err(),
        ModuleError::ErasedCallArgumentArityMismatch {
            operation: operation_id(2),
            expected: 1,
            actual: 0,
        }
    );
}

#[test]
fn substituted_erased_argument_row_rejects() {
    // A substituted actual still validates structurally, but the reconstructed
    // obligation tracks the row actually transmitted, so evidence phrased for
    // another instantiation no longer matches.
    let mut module = erased_call_module();
    *erased_lane_mut(&mut module) = vec![ScalarTerm::boolean(true)];

    validate_module(&module).expect("a substituted admitted actual validates");
    assert!(
        verify_module(
            &module,
            &call_site_bundle(Proposition::Equal(
                boolean_value(1),
                ScalarTerm::boolean(true),
            )),
            &AdmissionProfile::default()
        )
        .is_err(),
        "evidence for a different row does not discharge the substituted actual"
    );
}
