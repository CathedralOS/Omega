//! Source-produced scalar equations retain their meaning under kernel transport.

use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

use compiler::{CheckedCompileRequest, compile_to_checked};
use proof_admission::{
    Budget, Term, certificate_assumption_closure,
    verify_bounded_certificate_with_machine_parameters, verify_mathematical_certificate,
};
use semantic_vocabulary::{Proposition, ScalarTerm};
use terminal_psi::{EvidenceRoute, ProofRule};

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
boundary trait Host {
    machine measure(value: bool) -> bool reaches Host;
    machine finish(value: bool) reaches Host;
}

data Scalar {}
machine Scalar::measure(marker: u16, spare: bool, value: bool, other: bool, last: bool) -> bool
requires value == value
ensures result == ((value == spare) == (other == last))
reaches Host
{
    Host::finish(false);
    (value == spare) == (other == last)
}

data Main {}
machine Main::main(&mut self) reaches Host {
    let result: bool = Scalar::measure(9u16, false, true, false, true);
    Host::finish(result);
}
"#;

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "omega-kernel-equality-transport-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("main.omg"), source).unwrap();
        Self(directory)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn source_nested_boolean_equations_have_kernel_transport() {
    let project = Project::new(SOURCE);
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.0.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    drop(checked);
    drop(project);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let bundle = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let validated = terminal_verifier::validate_module(&module).unwrap();
    let questions =
        terminal_verifier::reconstruct_execution_terminal_obligations(validated).unwrap();
    let evidence = bundle
        .evidence
        .iter()
        .find(|evidence| {
            matches!(&evidence.route,
                EvidenceRoute::CertificateDerived(certificate)
                    if matches!(certificate.proof.rule, ProofRule::ValueEqualityTransport { .. })
            )
        })
        .expect("nested Boolean return retains explicit equation transport");
    let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        unreachable!()
    };
    let target = &certificate.proof;
    let ProofRule::ValueEqualityTransport {
        premise,
        equalities,
    } = &target.rule
    else {
        unreachable!()
    };
    assert_eq!(premise.conclusion, Proposition::Truth);
    assert_eq!(equalities.len(), 4);
    let site = questions
        .obligations()
        .iter()
        .find(|site| site.obligation.id == evidence.obligation)
        .unwrap();
    assert!(matches!(
        site.owner,
        terminal_verifier::ReconstructedTerminalObligationOwner::ContractEnsures { .. }
    ));
    assert_eq!(target.conclusion, site.obligation.proposition);
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == site.owner.machine())
        .unwrap();
    let context = validated.value_context(machine).unwrap();
    let parameters = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect();
    // Every equation comes from the decoded executable's reconstructed
    // operation/return facts, not from an authored replacement proof.
    for equality in equalities {
        let ProofRule::SemanticAxiom { index } = equality.rule else {
            panic!("source transport carries reconstructed value equations")
        };
        assert_eq!(equality.conclusion, site.semantic_axioms[index]);
    }
    let Proposition::Equal(_, ScalarTerm::BooleanEqual { left, right }) = &target.conclusion else {
        panic!("nested Boolean equality result contract")
    };
    assert!(matches!(left.as_ref(), ScalarTerm::BooleanEqual { .. }));
    assert!(matches!(right.as_ref(), ScalarTerm::BooleanEqual { .. }));

    let mut budget = Budget::default();
    let available_steps = budget.remaining();
    let denoted = verify_bounded_certificate_with_machine_parameters(
        &context,
        &site.obligation.proposition,
        &site.requirements,
        &site.semantic_axioms,
        &parameters,
        target,
        &mut budget,
    )
    .unwrap();
    for declaration in &denoted.certificate.signature {
        if declaration.body.is_some() {
            continue;
        }
        let mut conclusion = declaration.ty;
        while let Term::Pi { codomain, .. } = denoted.arena.get(conclusion) {
            conclusion = codomain;
        }
        assert!(
            !denoted
                .arena
                .structurally_equal(conclusion, denoted.certificate.expected),
            "the source transport conclusion must be derived rather than an instance assumption"
        );
    }
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. }
        ),
        "the retained source equations derive the contract through identity elimination"
    );
    let closure = certificate_assumption_closure(&denoted.arena, &denoted.certificate);
    eprintln!(
        "source transport closure={closure:?}; receipt={:?}; steps={}",
        denoted.receipt(),
        available_steps - budget.remaining()
    );
    let bytes =
        terminal_codec::encode_mathematical_certificate(&denoted.arena, &denoted.certificate)
            .unwrap();
    eprintln!("source transport mathematical wire bytes={}", bytes.len());
    let mut decoded = terminal_codec::decode_mathematical_certificate(&bytes).unwrap();
    verify_mathematical_certificate(
        &mut decoded.arena,
        &decoded.certificate,
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(
        certificate_assumption_closure(&decoded.arena, &decoded.certificate),
        closure
    );
    assert_eq!(
        terminal_codec::encode_mathematical_certificate(&decoded.arena, &decoded.certificate)
            .unwrap(),
        bytes
    );
    let Term::IdElim {
        motive,
        base,
        endpoint: _,
        proof,
    } = decoded.arena.get(decoded.certificate.term)
    else {
        panic!("source-free proof retains identity transport");
    };
    let wrong_endpoint = decoded.arena.insert(Term::TwoZero);
    decoded.certificate.term = decoded.arena.insert(Term::IdElim {
        motive,
        base,
        endpoint: wrong_endpoint,
        proof,
    });
    assert!(
        verify_mathematical_certificate(
            &mut decoded.arena,
            &decoded.certificate,
            &mut Budget::default()
        )
        .is_err()
    );

    for omitted in 0..equalities.len() {
        let mut missing = bundle.clone();
        let changed = missing
            .evidence
            .iter_mut()
            .find(|changed| changed.obligation == evidence.obligation)
            .unwrap();
        let EvidenceRoute::CertificateDerived(changed) = &mut changed.route else {
            unreachable!()
        };
        let ProofRule::ValueEqualityTransport { equalities, .. } = &mut changed.proof.rule else {
            unreachable!()
        };
        equalities.remove(omitted);
        assert!(
            terminal_verifier::verify_module(
                &module,
                &missing,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "every reconstructed equation is necessary, including position {omitted}"
        );
    }
    let mut substituted = module.clone();
    let machine = substituted
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == machine.id)
        .unwrap();
    let operation = machine
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanEqual { .. }
            )
        })
        .unwrap();
    let terminal_psi::OperationKind::BooleanEqual { left, right } = &mut operation.kind else {
        unreachable!()
    };
    assert_ne!(left, right);
    *right = *left;
    assert!(
        terminal_verifier::verify_module(
            &substituted,
            &bundle,
            &proof_admission::AdmissionProfile::default()
        )
        .is_err(),
        "the original proof cannot justify an operand substituted in the executable"
    );
    assert!(
        verify_bounded_certificate_with_machine_parameters(
            &context,
            &site.obligation.proposition,
            &site.requirements,
            &[],
            &parameters,
            target,
            &mut Budget::default(),
        )
        .is_err()
    );
    assert!(
        verify_bounded_certificate_with_machine_parameters(
            &context,
            &Proposition::Falsehood,
            &site.requirements,
            &site.semantic_axioms,
            &parameters,
            target,
            &mut Budget::default(),
        )
        .is_err()
    );
}

const OPAQUE_SOURCE: &str = r#"
boundary trait Host {
    machine measure(value: bool) -> bool reaches Host;
    machine finish(value: bool) reaches Host;
}

data Scalar {}
machine Scalar::masked(s: u16, t: u16, x: u16, w: u16)
requires
    s == t,
    s in 1..=4,
    t in 1..=4,
    x in 1..=4
{
    transition x * t >= 1 {
        true -> nonzero(x * s, w)
        _ -> dead()
    }

    state nonzero(d: u16, keep: u16) {
        let _q: u16 = keep % d;
    }

    state dead() {
    }
}

data Main {}
machine Main::main(&mut self) {
    Scalar::masked(2u16, 2u16, 3u16, 30u16);
}
"#;

/// An operation obligation whose proof must rewrite an equation INSIDE a
/// `x * s` operand. The arm-call binds `d` to `x * s` (semantic axioms
/// `d == x * s`); the divisor's nonzero fact comes from the guard
/// `x * t >= 1` and the entry equation `s == t`, so the transport's
/// conclusion only follows after the rewrite descends into the multiply.
/// Whole-term opaque constants cannot express that: they leave
/// `x * s` and `x * t` unrelated atoms, and the denotation falls back to
/// an admitted rule-instance assumption.
#[test]
fn source_opaque_operation_equations_have_kernel_transport() {
    let project = Project::new(OPAQUE_SOURCE);
    let checked = compile_to_checked(CheckedCompileRequest::new(
        &project.0.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    drop(checked);
    drop(project);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let bundle = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    let validated = terminal_verifier::validate_module(&module).unwrap();
    let questions =
        terminal_verifier::reconstruct_execution_terminal_obligations(validated).unwrap();
    let evidence = bundle
        .evidence
        .iter()
        .find(|evidence| {
            matches!(&evidence.route,
                EvidenceRoute::CertificateDerived(certificate)
                    if matches!(certificate.proof.rule, ProofRule::ValueEqualityTransport { .. }))
        })
        .expect("remainder divisor retains explicit equation transport");
    let EvidenceRoute::CertificateDerived(certificate) = &evidence.route else {
        unreachable!()
    };
    let target = &certificate.proof;
    let ProofRule::ValueEqualityTransport {
        premise,
        equalities,
    } = &target.rule
    else {
        unreachable!()
    };
    let site = questions
        .obligations()
        .iter()
        .find(|site| site.obligation.id == evidence.obligation)
        .unwrap();
    assert!(matches!(
        site.owner,
        terminal_verifier::ReconstructedTerminalObligationOwner::Operation { .. }
    ));
    assert_eq!(target.conclusion, site.obligation.proposition);

    // `1 <= d` where `d` is the state's divisor parameter; the premise is
    // the guard's `1 <= x * t` and the chain `d == v, v == x * s, s == t`
    // reaches `s` only through the multiply's operand position.
    let Proposition::LessOrEqual(_, ScalarTerm::Value { .. }) = &target.conclusion else {
        panic!("nonzero divisor obligation, got {:?}", target.conclusion)
    };
    let Proposition::LessOrEqual(
        _,
        ScalarTerm::ExactIntegerMultiply {
            right: premise_factor,
            ..
        },
    ) = &premise.conclusion
    else {
        panic!(
            "guard premise is `1 <= x * t`, got {:?}",
            premise.conclusion
        )
    };
    assert_eq!(equalities.len(), 3);
    let Proposition::Equal(
        _,
        ScalarTerm::ExactIntegerMultiply {
            right: bound_factor,
            ..
        },
    ) = &equalities[1].conclusion
    else {
        panic!(
            "arm argument binds `d` to `x * s`, got {:?}",
            equalities[1].conclusion
        )
    };
    let Proposition::Equal(
        ScalarTerm::Value { id: bound_s, .. },
        ScalarTerm::Value { id: bound_t, .. },
    ) = &equalities[2].conclusion
    else {
        panic!(
            "entry equation binds `s == t`, got {:?}",
            equalities[2].conclusion
        )
    };
    let ScalarTerm::Value {
        id: multiplied_s, ..
    } = bound_factor.as_ref()
    else {
        panic!("multiply operand is the multiplied value")
    };
    let ScalarTerm::Value { id: premise_t, .. } = premise_factor.as_ref() else {
        panic!("premise operand is the rewritten value")
    };
    assert_eq!(*bound_s, *multiplied_s);
    assert_eq!(*bound_t, *premise_t);
    for equality in equalities {
        assert!(
            matches!(
                equality.rule,
                ProofRule::SemanticAxiom { .. } | ProofRule::Assumption { .. }
            ),
            "source transport carries reconstructed value equations"
        );
    }

    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == site.owner.machine())
        .unwrap();
    let context = validated.value_context(machine).unwrap();
    let parameters = machine.parameters.iter().map(|p| p.id).collect();
    let denoted = verify_bounded_certificate_with_machine_parameters(
        &context,
        &site.obligation.proposition,
        &site.requirements,
        &site.semantic_axioms,
        &parameters,
        target,
        &mut Budget::default(),
    )
    .unwrap();
    for declaration in &denoted.certificate.signature {
        if declaration.body.is_some() {
            continue;
        }
        let mut conclusion = declaration.ty;
        while let Term::Pi { codomain, .. } = denoted.arena.get(conclusion) {
            conclusion = codomain;
        }
        assert!(
            !denoted
                .arena
                .structurally_equal(conclusion, denoted.certificate.expected),
            "the opaque-operation transport conclusion must be derived rather than an instance assumption"
        );
    }
    assert!(
        matches!(
            denoted.arena.get(denoted.certificate.term),
            Term::IdElim { .. }
        ),
        "the retained source equations derive the divisor fact through identity elimination"
    );
    for omitted in 0..equalities.len() {
        let mut missing = bundle.clone();
        let changed = missing
            .evidence
            .iter_mut()
            .find(|changed| changed.obligation == evidence.obligation)
            .unwrap();
        let EvidenceRoute::CertificateDerived(changed) = &mut changed.route else {
            unreachable!()
        };
        let ProofRule::ValueEqualityTransport { equalities, .. } = &mut changed.proof.rule else {
            unreachable!()
        };
        equalities.remove(omitted);
        assert!(
            terminal_verifier::verify_module(
                &module,
                &missing,
                &proof_admission::AdmissionProfile::default()
            )
            .is_err(),
            "every reconstructed equation is necessary, including position {omitted}"
        );
    }
}

#[test]
fn source_nested_boolean_transport_rejects_a_negated_result() {
    let changed = SOURCE.replace(
        "\n    (value == spare) == (other == last)\n",
        "\n    !((value == spare) == (other == last))\n",
    );
    assert_ne!(changed, SOURCE);
    let project = Project::new(&changed);
    let diagnostics = compile_to_checked(CheckedCompileRequest::new(
        &project.0.join("main.omg"),
        Some("linux_x86_64"),
    ))
    .expect_err("negating the nested expression cannot establish the original result contract");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .to_string()
            .contains("cannot prove ensures contract")),
        "{diagnostics:#?}"
    );
}
