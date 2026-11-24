//! A logical parameter supplies the whole named implication. Calling it must
//! establish its premise without selecting or executing the cited declaration.

#[path = "support/source_handoff.rs"]
mod source;

use source::{Program, Verdict};

const ORIGINAL: &str = include_str!("fixtures/proofs/positive/named_implication/main.omg");

#[test]
fn a_named_theorem_supplies_the_full_explicit_implication() {
    let program = Program::of(ORIGINAL);
    let verdict = program.judged(|elaboration| {
        assert!(
            elaboration.generic_binders.is_empty(),
            "the checked logical header leaves no open source binder at publication"
        );
        let proofs = &elaboration.module.proofs;
        assert!(
            !proofs.theorem_parameters.is_empty(),
            "the alias retains its complete logical type"
        );
        assert!(
            !proofs.theorem_applications.is_empty(),
            "Step(n) remains a real logical application"
        );
        assert!(
            !proofs.theorem_citations.is_empty(),
            "the explicit proof's source remains available to checking"
        );
        for source in proofs.theorem_citations.iter().filter_map(|row| {
            if let terminal_psi::TheoremSource::Named(source) = row.source {
                Some(source)
            } else {
                None
            }
        }) {
            assert!(
                elaboration
                    .module
                    .execution
                    .callables
                    .descriptors
                    .iter()
                    .all(|descriptor| descriptor.machine != source),
                "a theorem citation cannot create an executable callback"
            );
            assert!(
                elaboration
                    .module
                    .machines
                    .iter()
                    .flat_map(|machine| machine.module_invocations())
                    .all(|invocation| invocation.inputs.callee != source),
                "a theorem citation cannot create an executable call"
            );
        }
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn evidence_restricted_to_seven_cannot_supply_the_alias_even_at_seven() {
    let source = ORIGINAL.replace(
        "use_step::<nonzero_is_positive>(7u8)",
        "use_step::<only_seven>(7u8)",
    );
    assert_ne!(source, ORIGINAL);
    let program = Program::of(&source);
    let verdict = program.judged(|_| {});
    assert!(
        !matches!(verdict, Verdict::Accepted),
        "narrower evidence cannot be checked only at this caller's seven"
    );
    assert!(
        format!("{verdict:?}")
            .contains("explicit proof argument does not establish the complete aliased contract"),
        "{verdict:?}"
    );
}

#[test]
fn a_required_proof_argument_is_never_inferred_from_the_scalar_actual() {
    let source = ORIGINAL.replace("use_step::<nonzero_is_positive>(7u8)", "use_step(7u8)");
    assert_ne!(source, ORIGINAL);
    let refusal = match Program::try_of(&[("product/main.omg", &source)]) {
        Err(refusal) => refusal,
        Ok(_) => panic!("the header requires an explicit proof argument"),
    };
    assert!(
        refusal
            .message
            .contains("must explicitly supply the proof argument `Step`"),
        "{refusal:?}"
    );
}

#[test]
fn a_literal_does_not_supply_a_named_theorem() {
    let source = ORIGINAL.replace(
        "use_step::<nonzero_is_positive>(7u8)",
        "use_step::<7u8>(7u8)",
    );
    assert_ne!(source, ORIGINAL);
    let refusal = match Program::try_of(&[("product/main.omg", &source)]) {
        Err(refusal) => refusal,
        Ok(_) => panic!("a scalar literal is no proof citation"),
    };
    assert!(
        refusal
            .message
            .contains("proof supply needs its complete explicit theorem citation"),
        "{refusal:?}"
    );
}

#[test]
fn a_logical_use_cannot_drop_its_premise_obligation() {
    let program = Program::of(ORIGINAL);
    let verdict = program.judged(|elaboration| {
        let application = elaboration
            .module
            .proofs
            .theorem_applications
            .first_mut()
            .expect("one real logical application");
        assert!(!application.requirements.is_empty());
        application.requirements.clear();
    });
    assert!(!matches!(verdict, Verdict::Accepted));
    assert!(
        format!("{verdict:?}").contains("logical use lost a subject or premise"),
        "{verdict:?}"
    );
}

#[test]
fn a_theorem_conclusion_cannot_establish_its_own_missing_premise() {
    let source = ORIGINAL.replace(
        "machine use_step<Step: NonzeroIsPositive>(n: u8)\nrequires n != 0u8",
        "machine use_step<Step: NonzeroIsPositive>(n: u8)\nrequires true",
    );
    assert_ne!(source, ORIGINAL);
    let program = Program::of(&source);
    let verdict = program.judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted));
    assert!(
        format!("{verdict:?}").contains("TheoremRequirement"),
        "{verdict:?}"
    );
}

#[test]
fn received_logical_subjects_keep_their_real_declared_carrier() {
    let program = Program::of(ORIGINAL);
    program.judged(|elaboration| {
        let application = elaboration.module.proofs.theorem_applications[0].clone();
        let machine = elaboration
            .module
            .machines
            .iter_mut()
            .find(|machine| machine.id == application.machine)
            .unwrap();
        machine.parameters[0].scalar_type = semantic_vocabulary::ScalarType::Boolean;
        let error = terminal_codec::validate_theorem_parameters(&elaboration.module).unwrap_err();
        assert_eq!(
            error,
            "logical subject has another carrier or is unavailable before its application"
        );
    });
}

#[test]
fn a_logical_subject_cannot_be_an_out_of_carrier_literal() {
    let program = Program::of(ORIGINAL);
    program.judged(|elaboration| {
        let mut application = elaboration.module.proofs.theorem_applications[0].clone();
        let scalar_type = application.arguments[0].scalar_type().as_integer().unwrap();
        application.arguments[0] = semantic_vocabulary::ScalarTerm::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(256),
        };
        let owner = elaboration.module.machine(application.machine).unwrap();
        let error =
            terminal_semantics::theorems::application(&elaboration.module, owner, &application)
                .unwrap_err();
        assert_eq!(error, "logical subject has a malformed scalar term");
    });
}

#[test]
fn decoding_preserves_the_recorded_judgment_and_leaves_proof_checking_to_the_validator() {
    use source_compiler::test_support::checking::{RouteVerdict, judge};

    let program = Program::of(ORIGINAL);
    let RouteVerdict::Accepted(accepted) = judge(Ok(program.elaboration().clone())).unwrap() else {
        panic!("the original caller must reach publication");
    };
    let mut module = accepted.prepare_publication().unwrap().module().clone();
    let derivation = &mut module.proofs.theorem_citations[0].derivations[0];
    // This rule does not prove the recorded implication. The bytes still
    // identify the same source, subjects, invocation and conclusion.
    derivation.proof.rule =
        terminal_psi::ProofRule::Primitive(terminal_psi::PrimitiveJudgment::Truth);
    let bytes = terminal_codec::encode_module(&module).unwrap();
    let decoded = terminal_codec::decode_module(&bytes).unwrap();
    assert_eq!(
        decoded.proofs.theorem_citations,
        module.proofs.theorem_citations
    );
    assert!(
        terminal_codec::validate_theorem_evidence(
            &decoded,
            &mut proof_admission::Budget::default()
        )
        .is_err()
    );

    module.proofs.theorem_citations[0].derivations[0]
        .proof
        .conclusion = semantic_vocabulary::Proposition::Truth;
    let error = terminal_codec::decode_module(&terminal_codec::encode_module(&module).unwrap())
        .unwrap_err();
    assert!(
        matches!(
            error,
            terminal_codec::CodecError::MalformedStructuralFoundation(
                "theorem derivation lost its declared judgment or role"
            )
        ),
        "{error:?}"
    );
}
