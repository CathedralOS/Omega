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
fn a_named_application_names_only_a_machine_that_does_no_work() {
    // A named application emits no call, so the machine it names must have
    // nothing to run. The caller itself, or any machine with a result or
    // with operations in its body, is refused.
    let program = Program::of(ORIGINAL);
    program.judged(|elaboration| {
        let module = &elaboration.module;
        let mut application = module.proofs.theorem_applications[0].clone();
        let owner = module.machine(application.machine).unwrap();
        application.source = terminal_psi::TheoremSource::Named(owner.id);
        assert_eq!(
            terminal_semantics::theorems::application(module, owner, &application).unwrap_err(),
            "named theorem application cites its own machine"
        );
        let working = module
            .machines
            .iter()
            .find(|machine| {
                machine.id != owner.id
                    && machine
                        .blocks
                        .iter()
                        .any(|block| !block.operations.is_empty())
            })
            .expect("a machine whose body does work");
        application.source = terminal_psi::TheoremSource::Named(working.id);
        let error =
            terminal_semantics::theorems::application(module, owner, &application).unwrap_err();
        assert!(
            error.starts_with("named theorem application names a machine"),
            "{error}"
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

const RECURSIVE: &str = include_str!("fixtures/proofs/positive/recursive_forwarding/main.omg");

#[test]
fn a_recursive_call_forwards_its_theorem_through_the_cycle() {
    let program = Program::of(RECURSIVE);
    let verdict = program.judged(|elaboration| {
        assert!(
            !elaboration.module.proofs.theorem_applications.is_empty(),
            "Step(n) stays a real logical application inside the loop"
        );
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_recursive_call_cannot_hand_the_cycle_another_proof() {
    let source = RECURSIVE.replace(
        "false -> walk::<Step>(n - 1u8)",
        "false -> walk::<nonzero_is_positive>(n - 1u8)",
    );
    assert_ne!(source, RECURSIVE);
    let refusal = format!(
        "{:?}",
        Program::try_of(&[("product/main.omg", &source)]).err()
    );
    assert!(
        refusal.contains("a call around a cycle must forward each theorem formal unchanged"),
        "{refusal}"
    );
}

#[test]
fn a_recursive_call_still_owes_its_substituted_premise() {
    // With `n == 200`, the step from one reaches `walk(0)`.
    let source = RECURSIVE.replace("transition n == 1u8", "transition n == 200u8");
    assert_ne!(source, RECURSIVE);
    let verdict = Program::of(&source).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains("ArrivalInvariant"),
        "the jump to the loop head must prove `n != 0`: {verdict:?}"
    );
}

#[test]
fn a_recursive_call_still_owes_its_descent() {
    let source = RECURSIVE.replace("walk::<Step>(n - 1u8)", "walk::<Step>(n)");
    assert_ne!(source, RECURSIVE);
    let verdict = Program::of(&source).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains("removing strict rank edges leaves a control cycle"),
        "{verdict:?}"
    );
}

const RELAY: &str = "machine relay<Inner: NonzeroIsPositive>(n: u8)
requires n != 0u8
{
    use_step::<Inner>(n);
}

machine entry() -> u8 {
    relay::<nonzero_is_positive>(7u8);";

#[test]
fn a_theorem_formal_forwards_to_another_theorem_parameter() {
    let source = ORIGINAL.replace(
        "machine entry() -> u8 {\n    use_step::<nonzero_is_positive>(7u8);",
        RELAY,
    );
    assert_ne!(source, ORIGINAL);
    let verdict = Program::of(&source).judged(|elaboration| {
        assert!(
            elaboration
                .module
                .proofs
                .theorem_citations
                .iter()
                .any(|row| matches!(row.source, terminal_psi::TheoremSource::Formal(0))),
            "the relay hands on its own formal, not a new citation"
        );
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_forwarded_formal_must_state_the_same_implication() {
    // `relay`'s formal promises `n > 0` only for `n == 7`, which cannot
    // stand in for `use_step`'s alias over every nonzero `n`.
    let source = ORIGINAL
        .replace(
            "machine entry() -> u8 {\n    use_step::<nonzero_is_positive>(7u8);",
            &RELAY
                .replace("<Inner: NonzeroIsPositive>", "<Inner: OnlySeven>")
                .replace("relay::<nonzero_is_positive>", "relay::<only_seven>"),
        )
        .replace(
            "machine only_seven",
            "data OnlySeven = machine(n: u8)\n    requires n == 7u8\n    ensures n > 0u8;\n\nmachine only_seven",
        );
    assert_ne!(source, ORIGINAL);
    let verdict = Program::of(&source).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains("forwarded theorem has another complete implication"),
        "{verdict:?}"
    );
}

/// `above::<LIMIT>` proves `n > 0` for every `n` above `LIMIT`.
fn instantiated(citation: &str) -> String {
    ORIGINAL
        .replace(
            "machine only_seven",
            "machine above<const LIMIT: u8>(n: u8)\nrequires n > LIMIT\nensures n > 0u8\n{\n}\n\nmachine only_seven",
        )
        .replace("use_step::<nonzero_is_positive>(7u8)", &format!("use_step::<{citation}>(7u8)"))
}

#[test]
fn an_instantiated_theorem_supplies_the_alias() {
    let program = Program::of(&instantiated("above::<0u8>"));
    let verdict = program.judged(|elaboration| {
        let module = &elaboration.module;
        let [citation] = module.proofs.theorem_citations.as_slice() else {
            panic!("one citation: {:?}", module.proofs.theorem_citations);
        };
        let terminal_psi::TheoremSource::Named(source) = citation.source else {
            panic!("the instance is a named machine: {citation:?}");
        };
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| machine.module_invocations())
                .all(|invocation| invocation.inputs.callee != source),
            "citing the instance does not call it"
        );
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn an_instance_with_a_narrower_premise_cannot_supply_the_alias() {
    // `above::<3u8>` needs `n > 3`, which `n != 0` does not give.
    let verdict = Program::of(&instantiated("above::<3u8>")).judged(|_| {});
    assert!(
        format!("{verdict:?}")
            .contains("explicit proof argument does not establish the complete aliased contract"),
        "{verdict:?}"
    );
}

/// `lifted` proves `NonzeroIsPositive` itself, given a proof of it.
fn given_its_own_proof(citation: &str) -> String {
    ORIGINAL
        .replace(
            "machine only_seven",
            "machine lifted<Inner: NonzeroIsPositive>(n: u8)\nrequires n != 0u8\nensures n > 0u8\n{\n    Inner(n);\n}\n\nmachine only_seven",
        )
        .replace(
            "use_step::<nonzero_is_positive>(7u8)",
            &format!("use_step::<{citation}>(7u8)"),
        )
}

#[test]
fn a_theorem_given_its_own_proof_supplies_the_alias() {
    let program = Program::of(&given_its_own_proof("lifted::<nonzero_is_positive>"));
    let verdict = program.judged(|elaboration| {
        let [citation] = elaboration.module.proofs.theorem_citations.as_slice() else {
            panic!("one citation");
        };
        assert!(
            matches!(
                citation.instantiation.as_slice(),
                [terminal_psi::TheoremInstantiation {
                    source: terminal_psi::TheoremSource::Named(_),
                    ..
                }]
            ),
            "the citation keeps the proof it gives `lifted`: {citation:?}"
        );
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_theorem_given_a_narrower_proof_cannot_supply_the_alias() {
    // `only_seven` cannot stand in for `lifted`'s own `Inner`.
    let verdict = Program::of(&given_its_own_proof("lifted::<only_seven>")).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains(
            "proof given to the cited theorem does not establish its own aliased contract"
        ),
        "{verdict:?}"
    );
}

#[test]
fn a_citation_cannot_drop_the_proof_it_gives_the_cited_theorem() {
    let program = Program::of(&given_its_own_proof("lifted::<nonzero_is_positive>"));
    let verdict = program.judged(|elaboration| {
        elaboration.module.proofs.theorem_citations[0]
            .instantiation
            .clear();
    });
    assert!(
        format!("{verdict:?}")
            .contains("cited theorem needs one logical supply for each of its own proof formals"),
        "{verdict:?}"
    );
}

#[test]
fn decoding_keeps_the_proof_given_to_the_cited_theorem_and_its_check() {
    use source_compiler::test_support::checking::{RouteVerdict, judge};

    let program = Program::of(&given_its_own_proof("lifted::<nonzero_is_positive>"));
    let RouteVerdict::Accepted(accepted) = judge(Ok(program.elaboration().clone())).unwrap() else {
        panic!("the instantiated citation must reach publication");
    };
    let module = accepted.prepare_publication().unwrap().module().clone();
    let decoded =
        terminal_codec::decode_module(&terminal_codec::encode_module(&module).unwrap()).unwrap();
    assert_eq!(
        decoded.proofs.theorem_citations,
        module.proofs.theorem_citations
    );
    terminal_codec::validate_theorem_evidence(&decoded, &mut proof_admission::Budget::default())
        .unwrap();

    // The inner proof's derivation is checked by the validator too.
    let mut forged = module.clone();
    forged.proofs.theorem_citations[0].instantiation[0].derivations[0]
        .proof
        .rule = terminal_psi::ProofRule::Primitive(terminal_psi::PrimitiveJudgment::Truth);
    assert!(
        terminal_codec::validate_theorem_evidence(&forged, &mut proof_admission::Budget::default())
            .is_err()
    );

    // A changed inner judgment no longer decodes.
    forged = module;
    forged.proofs.theorem_citations[0].instantiation[0].derivations[0]
        .proof
        .conclusion = semantic_vocabulary::Proposition::Truth;
    let error = terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
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

/// `Lifted` takes a proof of `NonzeroIsPositive` in its own header, and
/// `use_lifted` gives its `Step` one when it applies it.
const CONTRACT_PROOF_PARAMETERS: &str =
    include_str!("fixtures/proofs/positive/contract_proof_parameters/main.omg");

#[test]
fn a_contract_with_its_own_proof_parameter_is_supplied_and_applied() {
    let program = Program::of(CONTRACT_PROOF_PARAMETERS);
    let verdict = program.judged(|elaboration| {
        let proofs = &elaboration.module.proofs;
        assert!(
            proofs
                .theorem_parameters
                .iter()
                .any(|row| row.contract.proof_parameters.len() == 1),
            "`Step` keeps the proof parameter of its contract"
        );
        let [citation] = proofs.theorem_citations.as_slice() else {
            panic!("one citation: {:?}", proofs.theorem_citations);
        };
        assert!(
            citation.instantiation.is_empty(),
            "`lifted` receives its proof where `Step` is applied, not where it is cited"
        );
        assert!(
            proofs.theorem_applications.iter().any(|row| matches!(
                row.instantiation.as_slice(),
                [terminal_psi::TheoremInstantiation {
                    source: terminal_psi::TheoremSource::Named(_),
                    ..
                }]
            )),
            "`Step::<nonzero_is_positive>(n)` keeps the proof it gives `Step`"
        );
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn an_application_cannot_give_its_contract_a_narrower_proof() {
    let source = CONTRACT_PROOF_PARAMETERS
        .replace("Step::<nonzero_is_positive>(n)", "Step::<only_seven>(n)");
    assert_ne!(source, CONTRACT_PROOF_PARAMETERS);
    let verdict = Program::of(&source).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains(
            "proof given to the applied theorem does not establish its own aliased contract"
        ),
        "{verdict:?}"
    );
}

#[test]
fn an_application_must_give_its_contract_every_proof_parameter() {
    let source = CONTRACT_PROOF_PARAMETERS.replace("Step::<nonzero_is_positive>(n)", "Step(n)");
    assert_ne!(source, CONTRACT_PROOF_PARAMETERS);
    let refusal = Program::try_of(&[("product/main.omg", &source)])
        .err()
        .expect("an application without its contract's proof is refused");
    assert!(
        format!("{refusal:?}").contains(
            "logical application needs one proof for each of its contract's own proof parameters"
        ),
        "{refusal:?}"
    );
}

#[test]
fn a_contract_binder_must_be_a_proof_of_a_logical_contract() {
    // `Inner: u8` would be a runtime value bound by a callable type, which no
    // contract gives a meaning.
    let source = CONTRACT_PROOF_PARAMETERS.replace(
        "data Lifted = machine<Inner: NonzeroIsPositive>(n: u8)",
        "data Lifted = machine<Inner: u8>(n: u8)",
    );
    assert_ne!(source, CONTRACT_PROOF_PARAMETERS);
    let refusal = Program::try_of(&[("product/main.omg", &source)])
        .err()
        .expect("a value binder on a callable type is refused");
    assert!(
        format!("{refusal:?}").contains(
            "a callable signature's own binder `Inner` must name a logical contract alias"
        ),
        "{refusal:?}"
    );
}

#[test]
fn a_dropped_application_proof_is_refused_by_the_checker() {
    let program = Program::of(CONTRACT_PROOF_PARAMETERS);
    let verdict = program.judged(|elaboration| {
        for row in &mut elaboration.module.proofs.theorem_applications {
            row.instantiation.clear();
        }
    });
    assert!(
        format!("{verdict:?}").contains(
            "logical use needs one proof for each of its contract's own proof parameters"
        ),
        "{verdict:?}"
    );
}

#[test]
fn a_theorem_without_the_contract_proof_parameter_cannot_supply_it() {
    // `nonzero_is_positive` declares no `Inner`, so its header is not
    // `Lifted`'s.
    let source = CONTRACT_PROOF_PARAMETERS.replace(
        "use_lifted::<lifted>(7u8)",
        "use_lifted::<nonzero_is_positive>(7u8)",
    );
    assert_ne!(source, CONTRACT_PROOF_PARAMETERS);
    let verdict = Program::of(&source).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains(
            "cited theorem's own proof formals are not exactly the proof parameters of the aliased contract"
        ),
        "{verdict:?}"
    );
}

#[test]
fn a_contract_with_a_proof_parameter_is_another_implication_when_forwarded() {
    // `Lifted` and `NonzeroIsPositive` state the same clauses, but a proof of
    // `Lifted` needs a proof of `NonzeroIsPositive` before it applies.
    let source = CONTRACT_PROOF_PARAMETERS
        .replace(
            "machine entry()",
            "machine relay<Step: Lifted>(n: u8)\nrequires n != 0u8\nensures n > 0u8\n{\n    use_step::<Step>(n);\n}\n\nmachine use_step<Step: NonzeroIsPositive>(n: u8)\nrequires n != 0u8\nensures n > 0u8\n{\n    Step(n);\n}\n\nmachine entry()",
        )
        .replace("use_lifted::<lifted>(7u8)", "relay::<lifted>(7u8)");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(
        format!("{verdict:?}").contains("forwarded theorem has another complete implication"),
        "{verdict:?}"
    );
}

#[test]
fn decoding_keeps_the_proof_an_application_gives_its_contract_and_its_check() {
    use source_compiler::test_support::checking::{RouteVerdict, judge};

    let program = Program::of(CONTRACT_PROOF_PARAMETERS);
    let RouteVerdict::Accepted(accepted) = judge(Ok(program.elaboration().clone())).unwrap() else {
        panic!("the contract with its own proof parameter must reach publication");
    };
    let module = accepted.prepare_publication().unwrap().module().clone();
    let decoded =
        terminal_codec::decode_module(&terminal_codec::encode_module(&module).unwrap()).unwrap();
    assert_eq!(decoded.proofs, module.proofs);
    terminal_codec::validate_theorem_evidence(&decoded, &mut proof_admission::Budget::default())
        .unwrap();
    let application = module
        .proofs
        .theorem_applications
        .iter()
        .position(|row| !row.instantiation.is_empty())
        .expect("`Step::<nonzero_is_positive>(n)`");

    // The applied proof's derivation is checked by the validator too.
    let mut forged = module.clone();
    forged.proofs.theorem_applications[application].instantiation[0].derivations[0]
        .proof
        .rule = terminal_psi::ProofRule::Primitive(terminal_psi::PrimitiveJudgment::Truth);
    assert!(
        terminal_codec::validate_theorem_evidence(&forged, &mut proof_admission::Budget::default())
            .is_err()
    );

    // A changed judgment no longer decodes.
    forged = module.clone();
    forged.proofs.theorem_applications[application].instantiation[0].derivations[0]
        .proof
        .conclusion = semantic_vocabulary::Proposition::Truth;
    let error = terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap())
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

    // Dropping the contract's proof parameter changes its implication.
    forged = module;
    for row in &mut forged.proofs.theorem_parameters {
        row.contract.proof_parameters.clear();
    }
    assert!(
        terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap()).is_err()
    );
}
const ONE_IS_NONZERO: &str = "data OneIsNonzero = machine(n: u8)
    requires n == 1u8
    ensures n != 0u8;

machine one_is_nonzero(n: u8)
requires n == 1u8
ensures n != 0u8
{
}

machine use_step<Step: OneIsNonzero>(n: u8)
requires n == 1u8
ensures n != 0u8
{
    Step(n);
}

machine entry() -> u8 {
    use_step::<one_is_nonzero>(1u8);
    0u8
}
";

#[test]
fn a_theorem_proved_from_an_equality_supplies_the_alias() {
    let verdict = Program::of(ONE_IS_NONZERO).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_theorem_proved_from_another_equality_cannot_supply_the_alias() {
    let source = ONE_IS_NONZERO.replacen(
        "machine one_is_nonzero(n: u8)\nrequires n == 1u8",
        "machine one_is_nonzero(n: u8)\nrequires n == 2u8",
        1,
    );
    assert_ne!(source, ONE_IS_NONZERO);
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}
fn at_an_address(source: &str) -> String {
    source
        .replace("n: u8", "n: address")
        .replace("1u8", "1")
        .replace("0u8;", "0;")
        .replace("!= 0u8", "!= 0")
}

#[test]
fn an_address_theorem_supplies_the_alias() {
    let source = at_an_address(ONE_IS_NONZERO);
    assert!(source.contains("machine(n: address)"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn an_address_theorem_from_another_equality_cannot_supply_the_alias() {
    let source = at_an_address(ONE_IS_NONZERO).replacen(
        "machine one_is_nonzero(n: address)\nrequires n == 1",
        "machine one_is_nonzero(n: address)\nrequires n == 2",
        1,
    );
    assert!(source.contains("requires n == 2"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// Every numeric address is zero or positive, so `n != 0` gives `n > 0`.
#[test]
fn a_nonzero_address_theorem_concludes_strict_positivity() {
    let source = at_an_address(ONE_IS_NONZERO)
        .replace("requires n == 1", "requires n != 0")
        .replace("ensures n != 0", "ensures n > 0")
        .replace("(1)", "(7)");
    assert!(source.contains("ensures n > 0"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// Zero is an address, so a premise that admits it gives no strict positivity.
#[test]
fn an_address_theorem_cannot_conclude_strict_positivity_without_excluding_zero() {
    let source = at_an_address(ONE_IS_NONZERO)
        .replace("requires n == 1", "requires n <= 5")
        .replace("ensures n != 0", "ensures n > 0")
        .replace("(1)", "(3)");
    assert!(source.contains("requires n <= 5"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// A record input is one subject for each of its scalar fields.
const FIRST_IS_NONZERO: &str = "data Pair { x: u8; y: u8; }

data FirstIsNonzero = machine(p: Pair)
    requires p.x == 1u8
    ensures p.x != 0u8;

machine first_is_nonzero(p: Pair)
requires p.x == 1u8
ensures p.x != 0u8
{
}

machine use_step<Step: FirstIsNonzero>(p: Pair)
requires p.x == 1u8
ensures p.x != 0u8
{
    Step(p);
}

machine entry() -> u8 {
    use_step::<first_is_nonzero>(Pair { x: 1u8, y: 2u8 });
    0u8
}
";

#[test]
fn a_theorem_over_a_record_field_supplies_its_alias() {
    let verdict = Program::of(FIRST_IS_NONZERO).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_theorem_over_another_field_of_the_record_cannot_supply_its_alias() {
    let source = FIRST_IS_NONZERO.replacen(
        "machine first_is_nonzero(p: Pair)\nrequires p.x == 1u8\nensures p.x != 0u8",
        "machine first_is_nonzero(p: Pair)\nrequires p.y == 1u8\nensures p.y != 0u8",
        1,
    );
    assert!(source.contains("requires p.y == 1u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// A field of the record names its subject where the theorem is applied:
/// the application's conclusion is about `p.x`, not about `p.y`.
#[test]
fn an_applied_record_theorem_concludes_nothing_about_another_field() {
    let source = FIRST_IS_NONZERO.replacen(
        "requires p.x == 1u8\nensures p.x != 0u8\n{\n    Step(p);",
        "requires p.x == 1u8\nensures p.y != 0u8\n{\n    Step(p);",
        1,
    );
    assert!(
        source.contains("ensures p.y != 0u8\n{\n    Step(p);"),
        "{source}"
    );
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// A field that is itself a plain record is one subject for each of its
/// own scalar fields, in place.
fn nested(source: &str) -> String {
    source
        .replace(
            "data Pair { x: u8; y: u8; }",
            "data Inner { x: u8; z: u8; }\n\ndata Pair { inner: Inner; y: u8; }",
        )
        .replace("p.x", "p.inner.x")
        .replace(
            "Pair { x: 1u8, y: 2u8 }",
            "Pair { inner: Inner { x: 1u8, z: 3u8 }, y: 2u8 }",
        )
}

#[test]
fn a_theorem_over_a_nested_record_field_supplies_its_alias() {
    let source = nested(FIRST_IS_NONZERO);
    assert!(source.contains("requires p.inner.x == 1u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_theorem_over_another_nested_field_cannot_supply_its_alias() {
    let source = nested(FIRST_IS_NONZERO).replacen(
        "machine first_is_nonzero(p: Pair)\nrequires p.inner.x == 1u8\nensures p.inner.x != 0u8",
        "machine first_is_nonzero(p: Pair)\nrequires p.inner.z == 1u8\nensures p.inner.z != 0u8",
        1,
    );
    assert!(source.contains("requires p.inner.z == 1u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// The application's conclusion is about the nested field the actual
/// record holds at that path, not about the outer record's other field.
#[test]
fn an_applied_nested_record_theorem_concludes_nothing_about_another_field() {
    let source = nested(FIRST_IS_NONZERO).replacen(
        "requires p.inner.x == 1u8\nensures p.inner.x != 0u8\n{\n    Step(p);",
        "requires p.inner.x == 1u8\nensures p.y != 0u8\n{\n    Step(p);",
        1,
    );
    assert!(
        source.contains("ensures p.y != 0u8\n{\n    Step(p);"),
        "{source}"
    );
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// A record's data invariant holds for every value of its type, so it is a
/// premise of a contract over that record: `x != 0` gives `x > 0` with no
/// stated `requires`.
const NONZERO_FIRST: &str = "data Pair requires self.x != 0u8 { x: u8; y: u8; }

data FirstIsPositive = machine(p: Pair)
    ensures p.x > 0u8;

machine first_is_positive(p: Pair)
ensures p.x > 0u8
{
}

machine use_step<Step: FirstIsPositive>(p: Pair)
ensures p.x > 0u8
{
    Step(p);
}

machine entry() -> u8 {
    use_step::<first_is_positive>(Pair { x: 1u8, y: 2u8 });
    0u8
}
";

#[test]
fn a_record_data_invariant_is_a_premise_of_its_logical_subject() {
    let verdict = stored(NONZERO_FIRST);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

#[test]
fn an_invariant_on_another_field_does_not_establish_the_conclusion() {
    let source = NONZERO_FIRST.replace("requires self.x != 0u8", "requires self.y != 0u8");
    assert_ne!(source, NONZERO_FIRST);
    let verdict = stored(&source);
    assert!(!matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// A nested record's invariant is a premise over the outer record's subjects.
#[test]
fn a_nested_record_data_invariant_is_a_premise_of_its_logical_subject() {
    let source = NONZERO_FIRST
        .replace(
            "data Pair requires self.x != 0u8 { x: u8; y: u8; }",
            "data Inner requires self.x != 0u8 { x: u8; z: u8; }\n\ndata Pair { inner: Inner; y: u8; }",
        )
        .replace("p.x", "p.inner.x")
        .replace(
            "Pair { x: 1u8, y: 2u8 }",
            "Pair { inner: Inner { x: 1u8, z: 3u8 }, y: 2u8 }",
        );
    assert!(source.contains("ensures p.inner.x > 0u8"), "{source}");
    let verdict = stored(&source);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
    let other = source.replace("requires self.x != 0u8", "requires self.z != 0u8");
    assert_ne!(other, source);
    let verdict = stored(&other);
    assert!(!matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// A constant record stores a machine in an ordinary callable field. The
/// same field is invoked under its callable contract and cited as a theorem
/// (OWNER_QUESTIONS.md#bundle-evidence-field-type).
/// A theorem over the inner record applied to `p.inner`, a field of the
/// applying machine's record formal.
const INNER_FIELD: &str = "data Inner { x: u8; z: u8; }

data Pair { inner: Inner; y: u8; }

data InnerIsNonzero = machine(i: Inner)
    requires i.x == 1u8
    ensures i.x != 0u8;

machine inner_is_nonzero(i: Inner)
requires i.x == 1u8
ensures i.x != 0u8
{
}

machine use_inner<Step: InnerIsNonzero>(p: Pair)
requires p.inner.x == 1u8
ensures p.inner.x != 0u8
{
    Step(p.inner);
}

machine entry() -> u8 {
    use_inner::<inner_is_nonzero>(Pair { inner: Inner { x: 1u8, z: 3u8 }, y: 2u8 });
    0u8
}
";

#[test]
fn a_theorem_applied_to_a_record_field_of_a_formal_concludes_about_that_field() {
    let verdict = Program::of(INNER_FIELD).judged(|elaboration| {
        let application = &elaboration.module.proofs.theorem_applications[0];
        assert!(
            application.arguments.iter().all(|argument| matches!(
                argument,
                semantic_vocabulary::ScalarTerm::IntegerField { path, .. } if path.len() == 2
            )),
            "each subject is read beneath the projected field: {application:?}"
        );
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_theorem_applied_to_a_record_field_concludes_nothing_about_the_outer_record() {
    let source = INNER_FIELD.replacen(
        "requires p.inner.x == 1u8\nensures p.inner.x != 0u8\n{\n    Step(p.inner);",
        "requires p.inner.x == 1u8\nensures p.y != 0u8\n{\n    Step(p.inner);",
        1,
    );
    assert!(source.contains("ensures p.y != 0u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_theorem_applied_to_a_record_field_owes_its_premise_for_that_field() {
    let source = INNER_FIELD.replacen(
        "requires p.inner.x == 1u8\nensures p.inner.x != 0u8\n{\n    Step(p.inner);",
        "requires p.inner.z == 1u8\nensures p.inner.x != 0u8\n{\n    Step(p.inner);",
        1,
    );
    assert!(source.contains("requires p.inner.z == 1u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// A record the applying machine establishes itself is a subject too: its
/// contents reaching the application are the subjects' values.
fn local(x: &str, ensures: &str) -> String {
    INNER_FIELD.replacen(
        "requires p.inner.x == 1u8\nensures p.inner.x != 0u8\n{\n    Step(p.inner);",
        &format!("{ensures}{{\n    let i: Inner = Inner {{ x: {x}, z: 3u8 }};\n    Step(i);"),
        1,
    )
}

#[test]
fn a_theorem_applied_to_a_record_the_machine_establishes_owes_its_premise_there() {
    let source = local("1u8", "");
    assert!(source.contains("Step(i);"), "{source}");
    let verdict = Program::of(&source).judged(|elaboration| {
        let application = &elaboration.module.proofs.theorem_applications[0];
        let machine = elaboration.module.machine(application.machine).unwrap();
        assert!(
            application.arguments.iter().all(|argument| matches!(
                argument,
                semantic_vocabulary::ScalarTerm::IntegerField { root, .. }
                    if terminal_psi::StructuralParameterDeclaration::find_by_place(
                        &machine.contract.structural_parameters,
                        *root,
                    )
                    .is_none()
            )),
            "each subject reads the established record: {application:?}"
        );
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_record_the_machine_establishes_must_meet_the_premise() {
    let source = local("0u8", "");
    assert!(source.contains("Inner { x: 0u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// The conclusion is about the established record, not the formal.
#[test]
fn a_theorem_applied_to_an_established_record_concludes_nothing_about_a_formal() {
    let source = local("1u8", "ensures p.inner.x != 0u8\n");
    assert!(
        source.contains("ensures p.inner.x != 0u8\n{\n    let i"),
        "{source}"
    );
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// Terminal Psi may name a theorem over a record directly in an
/// application. Its subjects are the record's scalar fields, as in a
/// citation, and its body must return without running anything.
fn named_over_record() -> String {
    FIRST_IS_NONZERO.replacen(
        "machine use_step",
        "machine touch(n: u8) {\n}\n\nmachine busy(p: Pair)\nrequires p.x == 1u8\nensures p.x != 0u8\n{\n    touch(p.x);\n}\n\nmachine second_is_nonzero(p: Pair)\nrequires p.y == 1u8\nensures p.y != 0u8\n{\n}\n\nmachine use_step",
        1,
    )
}

fn applied_named(program: &Program, path: &str) -> Verdict {
    let named = program.machine(path).id;
    program.judged(|elaboration| {
        let application = &mut elaboration.module.proofs.theorem_applications[0];
        assert!(matches!(
            application.source,
            terminal_psi::TheoremSource::Formal(_)
        ));
        application.source = terminal_psi::TheoremSource::Named(named);
    })
}

#[test]
fn a_named_theorem_applied_to_a_record_supplies_its_conclusion() {
    let program = Program::of(&named_over_record());
    let verdict = applied_named(&program, "first_is_nonzero");
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_named_theorem_over_another_field_does_not_supply_the_conclusion() {
    let program = Program::of(&named_over_record());
    let verdict = applied_named(&program, "second_is_nonzero");
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

#[test]
fn a_named_theorem_over_a_record_whose_body_does_work_is_refused() {
    let program = Program::of(&named_over_record());
    let verdict = applied_named(&program, "busy");
    assert!(
        format!("{verdict:?}")
            .contains("named theorem application names a machine whose body does work"),
        "{verdict:?}"
    );
}

const CALLABLE_FIELD: &str = "data NonzeroIsPositive = machine(n: u8)
    requires n != 0u8
    ensures n > 0u8
    reaches none crashes none;

machine nonzero_is_positive(n: u8)
requires n != 0u8
ensures n > 0u8
{
}

data Laws [copy] { step: NonzeroIsPositive; }

const U8_LAWS: Laws = Laws { step: nonzero_is_positive };

machine use_step<Step: NonzeroIsPositive>(n: u8)
requires n != 0u8
ensures n > 0u8
{
    Step(n);
}

machine entry() -> u8 {
    let laws: Laws = U8_LAWS;
    laws.step(7u8);
    use_step::<U8_LAWS.step>(7u8);
    0u8
}
";

fn stored(source: &str) -> Result<Verdict, String> {
    Program::try_of(&[("product/main.omg", source)])
        .map(|program| program.judged(|_| {}))
        .map_err(|refusal| format!("{refusal:?}"))
}

#[test]
fn a_theorem_stored_in_a_constant_callable_field_is_invoked_and_cited() {
    let program = Program::of(CALLABLE_FIELD);
    let verdict = program.judged(|elaboration| {
        let module = &elaboration.module;
        let cited = module
            .proofs
            .theorem_citations
            .iter()
            .filter_map(|row| match row.source {
                terminal_psi::TheoremSource::Named(source) => Some(source),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(!cited.is_empty(), "the field's selection is cited by name");
        assert!(
            !module.execution.callables.descriptors.is_empty(),
            "the stored selection remains an ordinary callable value"
        );
        for source in cited {
            assert!(
                module
                    .machines
                    .iter()
                    .flat_map(|machine| machine.module_invocations())
                    .all(|invocation| invocation.inputs.callee != source),
                "citing the field executes no callback"
            );
        }
    });
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// The field's type admits only selections proved for seven, so its
/// evidence does not cover the complete aliased implication.
#[test]
fn a_field_whose_selections_are_proved_only_for_seven_cannot_be_cited() {
    let source = CALLABLE_FIELD
        .replace(
            "data Laws [copy] { step: NonzeroIsPositive; }",
            "data OnlySeven = machine(n: u8)
    requires n == 7u8
    ensures n > 0u8
    reaches none crashes none;

machine only_seven(n: u8)
requires n == 7u8
ensures n > 0u8
{
}

data Laws [copy] { step: OnlySeven; }",
        )
        .replace(
            "Laws { step: nonzero_is_positive }",
            "Laws { step: only_seven }",
        );
    assert!(source.contains("Laws { step: only_seven }"), "{source}");
    let verdict = stored(&source);
    assert!(!matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
    assert!(
        format!("{verdict:?}")
            .contains("explicit proof argument does not establish the complete aliased contract"),
        "{verdict:?}"
    );
}

/// The selected machine recurs until `n` is nonzero, so its guarantee
/// holds only after it returns. It stays an ordinary callable value.
const RETURNING: &str = "machine positive_after_return(n: u8)
requires n != 0u8
ensures n > 0u8
{
    transition n == 0u8 {
        true -> positive_after_return(n),
        _ -> finish(n)
    }
}

machine finish(n: u8)
{
}

data Laws [copy]";

#[test]
fn a_field_whose_selection_proves_its_claim_only_after_returning_cannot_be_cited() {
    let source = CALLABLE_FIELD
        .replace("data Laws [copy]", RETURNING)
        .replace(
            "Laws { step: nonzero_is_positive }",
            "Laws { step: positive_after_return }",
        );
    assert!(
        source.contains("Laws { step: positive_after_return }"),
        "{source}"
    );
    let verdict = stored(&source);
    assert!(!matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
    assert!(
        format!("{verdict:?}").contains("its guarantee holds only after it returns"),
        "{verdict:?}"
    );
}

/// Naming such a machine directly is refused the same way: a citation runs
/// nothing, so it cannot rely on the body returning.
#[test]
fn a_named_theorem_that_proves_its_claim_only_after_returning_cannot_be_cited() {
    let source = CALLABLE_FIELD
        .replace("data Laws [copy]", RETURNING)
        .replace(
            "use_step::<U8_LAWS.step>(7u8)",
            "use_step::<positive_after_return>(7u8)",
        );
    assert!(
        source.contains("use_step::<positive_after_return>"),
        "{source}"
    );
    let verdict = stored(&source);
    assert!(
        format!("{verdict:?}").contains("its guarantee holds only after it returns"),
        "{verdict:?}"
    );
}

#[test]
fn a_field_whose_selection_cannot_be_cited_is_still_invoked() {
    let source = CALLABLE_FIELD
        .replace("data Laws [copy]", RETURNING)
        .replace(
            "Laws { step: nonzero_is_positive }",
            "Laws { step: positive_after_return }",
        )
        .replace("    use_step::<U8_LAWS.step>(7u8);\n", "");
    assert!(!source.contains("use_step::<U8_LAWS.step>"), "{source}");
    let verdict = stored(&source);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// A record formal may hold any selection its type allows, so its field
/// has no one selection whose evidence covers the citation.
#[test]
fn a_runtime_record_field_cannot_be_cited() {
    let source = CALLABLE_FIELD.replace(
        "machine entry() -> u8 {",
        "machine use_laws(laws: Laws) {
    use_step::<laws.step>(7u8);
}

machine entry() -> u8 {
    use_laws(U8_LAWS);",
    );
    assert!(source.contains("use_step::<laws.step>"), "{source}");
    let verdict = stored(&source);
    assert!(
        format!("{verdict:?}").contains("only those have one possible selection"),
        "{verdict:?}"
    );
}

/// An immutable local holds exactly its initializer, so its field has the
/// one selection that initializer names, here through a constant.
#[test]
fn a_theorem_stored_in_an_immutable_local_record_is_cited() {
    let source = CALLABLE_FIELD.replace(
        "use_step::<U8_LAWS.step>(7u8)",
        "use_step::<laws.step>(7u8)",
    );
    assert!(source.contains("use_step::<laws.step>"), "{source}");
    let verdict = stored(&source);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// The same through a record literal bound to the local.
#[test]
fn a_theorem_stored_in_a_local_record_literal_is_cited() {
    let source = CALLABLE_FIELD
        .replace(
            "let laws: Laws = U8_LAWS;",
            "let laws: Laws = Laws { step: nonzero_is_positive };",
        )
        .replace(
            "use_step::<U8_LAWS.step>(7u8)",
            "use_step::<laws.step>(7u8)",
        );
    assert!(
        source.contains("Laws { step: nonzero_is_positive };\n    laws"),
        "{source}"
    );
    let verdict = stored(&source);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// A local literal whose selection proves its claim only after returning is
/// refused at the citation, as a constant's is.
#[test]
fn a_local_record_literal_whose_selection_proves_only_after_returning_cannot_be_cited() {
    let source = CALLABLE_FIELD
        .replace("data Laws [copy]", RETURNING)
        .replace(
            "let laws: Laws = U8_LAWS;",
            "let laws: Laws = Laws { step: positive_after_return };",
        )
        .replace(
            "use_step::<U8_LAWS.step>(7u8)",
            "use_step::<laws.step>(7u8)",
        );
    assert!(
        source.contains("Laws { step: positive_after_return };"),
        "{source}"
    );
    let verdict = stored(&source);
    assert!(
        format!("{verdict:?}").contains("its guarantee holds only after it returns"),
        "{verdict:?}"
    );
}

/// A mutable local may be reassigned to another selection its type allows,
/// so its field has no one selection to cite.
#[test]
fn a_mutable_local_record_field_cannot_be_cited() {
    let source = CALLABLE_FIELD
        .replace(
            "let laws: Laws = U8_LAWS;",
            "let mutable laws: Laws = U8_LAWS;",
        )
        .replace(
            "use_step::<U8_LAWS.step>(7u8)",
            "use_step::<laws.step>(7u8)",
        );
    assert!(source.contains("let mutable laws"), "{source}");
    let verdict = stored(&source);
    assert!(!matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
    assert!(
        format!("{verdict:?}").contains("only those have one possible selection"),
        "{verdict:?}"
    );
}

/// A sum input of a logical contract is one value of the sum's inductive
/// declaration (mathematical_bindings.md, "A sum input in a logical contract
/// denotes a single mathematical value of that sum's checked inductive
/// declaration"). The alias and the cited machine are both read over that
/// one subject, and the citation's three derivations are checked over it.
const ONE_IS_POSITIVE: &str = "data Choice {
    case One(x: u8);
    case Two(y: u8);
}

data OneIsPositive = machine(c: Choice)
    requires c in Choice::One && c.x != 0u8
    ensures c.x > 0u8;

machine one_is_positive(c: Choice)
requires c in Choice::One && c.x != 0u8
ensures c.x > 0u8
{
}

machine use_step<Step: OneIsPositive>() {
}

machine entry() -> u8 {
    use_step::<one_is_positive>();
    0u8
}
";

#[test]
fn a_theorem_over_a_sum_supplies_its_alias() {
    let verdict = stored(ONE_IS_POSITIVE);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// The alias's premise names case `One`; a machine that needs case `Two`
/// is not established by it.
#[test]
fn a_theorem_needing_another_case_cannot_supply_the_alias() {
    let source = ONE_IS_POSITIVE.replace(
        "machine one_is_positive(c: Choice)\nrequires c in Choice::One && c.x != 0u8",
        "machine one_is_positive(c: Choice)\nrequires c in Choice::Two && c.x != 0u8",
    );
    assert!(source.contains("requires c in Choice::Two"), "{source}");
    let verdict = stored(&source);
    let shown = format!("{verdict:?}");
    assert!(
        shown.contains("explicit proof argument does not establish the complete aliased contract"),
        "{shown}"
    );
}

/// The alias's premise says less about the payload than the machine needs:
/// `c.x != 1u8` does not establish `c.x != 0u8`.
#[test]
fn a_theorem_needing_more_of_the_payload_cannot_supply_the_alias() {
    let source = ONE_IS_POSITIVE.replace(
        "data OneIsPositive = machine(c: Choice)\n    requires c in Choice::One && c.x != 0u8\n    ensures c.x > 0u8;",
        "data OneIsPositive = machine(c: Choice)\n    requires c in Choice::One && c.x != 1u8\n    ensures c.x != 1u8;",
    );
    assert!(
        source.contains("requires c in Choice::One && c.x != 1u8"),
        "{source}"
    );
    let verdict = stored(&source);
    let shown = format!("{verdict:?}");
    assert!(
        shown.contains("explicit proof argument does not establish the complete aliased contract"),
        "{shown}"
    );
}

/// The cited machine's conclusion `c.x > 0u8` does not reach the alias's
/// stronger `c.x > 1u8`.
#[test]
fn a_theorem_concluding_less_about_the_payload_cannot_supply_the_alias() {
    let source = ONE_IS_POSITIVE.replace(
        "    requires c in Choice::One && c.x != 0u8\n    ensures c.x > 0u8;",
        "    requires c in Choice::One && c.x != 0u8\n    ensures c.x > 1u8;",
    );
    assert!(source.contains("ensures c.x > 1u8;"), "{source}");
    let verdict = stored(&source);
    let shown = format!("{verdict:?}");
    assert!(
        shown.contains("explicit proof argument does not establish the complete aliased contract"),
        "{shown}"
    );
}

#[test]
fn a_theorem_over_a_sum_with_a_float_payload_is_refused_by_name() {
    let source = ONE_IS_POSITIVE.replace("case Two(y: u8);", "case Two(y: f32);");
    assert!(source.contains("case Two(y: f32);"), "{source}");
    let verdict = stored(&source);
    let shown = format!("{verdict:?}");
    assert!(
        shown.contains("logical contract has no well-formed mathematical implication"),
        "{shown}"
    );
}

/// Applying a theorem over a sum needs the case known at the edge, which is
/// not yet checked.
/// `Step(c)` applies the theorem to a sum formal of the applying machine:
/// its premises are owed of `c` where the edge leaves, and its conclusion
/// is known of `c` after it.
fn applied_to_a_sum(premise: &str, ensures: &str, argument: &str) -> String {
    ONE_IS_POSITIVE
        .replace(
            "machine use_step<Step: OneIsPositive>() {\n}",
            &format!(
                "machine use_step<Step: OneIsPositive>(c: Choice)\nrequires {premise}\nensures {ensures}\n{{\n    Step({argument});\n}}"
            ),
        )
        .replace(
            "use_step::<one_is_positive>();",
            "use_step::<one_is_positive>(Choice::One { x: 7u8 });",
        )
}

#[test]
fn a_theorem_over_a_sum_applies_to_a_sum_formal() {
    let source = applied_to_a_sum("c in Choice::One && c.x != 0u8", "c.x > 0u8", "c");
    assert!(source.contains("    Step(c);"), "{source}");
    let verdict = stored(&source);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// The premise `c.x != 0u8` is owed where `Step(c)` is applied; knowing only
/// the case does not establish it.
#[test]
fn an_applied_sum_theorem_owes_its_payload_premise() {
    let source = applied_to_a_sum("c in Choice::One", "true", "c");
    assert!(
        source.contains("requires c in Choice::One\nensures true"),
        "{source}"
    );
    let verdict = stored(&source);
    // Nothing reaching the application's edge establishes the premise.
    let shown = format!("{verdict:?}");
    assert!(shown.contains("Unproved(NoDerivation)"), "{shown}");
}

/// The premise `c in Choice::One` is owed too: knowing case `Two` does not
/// establish it.
#[test]
fn an_applied_sum_theorem_owes_its_case_premise() {
    let source = applied_to_a_sum("c in Choice::Two", "true", "c");
    assert!(
        source.contains("requires c in Choice::Two\nensures true"),
        "{source}"
    );
    let verdict = stored(&source);
    // Nothing reaching the application's edge establishes the premise.
    let shown = format!("{verdict:?}");
    assert!(shown.contains("Unproved(NoDerivation)"), "{shown}");
}

/// The conclusion known after `Step(c)` is the contract's, `c.x > 0u8`, and
/// no more.
#[test]
fn an_applied_sum_theorem_concludes_only_its_guarantee() {
    let source = applied_to_a_sum("c in Choice::One && c.x != 0u8", "c.x > 1u8", "c");
    assert!(source.contains("ensures c.x > 1u8"), "{source}");
    let verdict = stored(&source);
    assert!(!matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// A sum the applying machine establishes before the application is a
/// subject too: what it holds where the edge leaves.
fn applied_to_a_built_sum(payload: &str) -> String {
    applied_to_a_sum("true", "true", "c").replace(
        "machine use_step<Step: OneIsPositive>(c: Choice)\nrequires true\nensures true\n{\n    Step(c);",
        &format!(
            "machine use_step<Step: OneIsPositive>(d: Choice)\nrequires true\nensures true\n{{\n    let c: Choice = Choice::One {{ x: {payload} }};\n    Step(c);"
        ),
    )
}

#[test]
fn a_theorem_over_a_sum_applies_to_a_sum_the_machine_builds() {
    let source = applied_to_a_built_sum("7u8");
    assert!(
        source.contains("let c: Choice = Choice::One { x: 7u8 };"),
        "{source}"
    );
    let verdict = stored(&source);
    assert!(matches!(verdict, Ok(Verdict::Accepted)), "{verdict:?}");
}

/// The built sum's payload is zero, so the premise `c.x != 0u8` fails.
#[test]
fn a_built_sum_that_fails_the_premise_is_not_a_subject() {
    let source = applied_to_a_built_sum("0u8");
    assert!(
        source.contains("let c: Choice = Choice::One { x: 0u8 };"),
        "{source}"
    );
    let verdict = stored(&source);
    let shown = format!("{verdict:?}");
    assert!(shown.contains("TheoremRequirement"), "{shown}");
}

/// A constructed value with no name is not a subject.
#[test]
fn applying_a_theorem_to_an_unnamed_sum_is_refused_by_name() {
    let source = applied_to_a_sum("true", "true", "Choice::One { x: 7u8 }");
    assert!(
        source.contains("    Step(Choice::One { x: 7u8 });"),
        "{source}"
    );
    let verdict = stored(&source);
    let shown = format!("{verdict:?}");
    assert!(
        shown.contains("logical sum subject is not a named sum of the applying machine"),
        "{shown}"
    );
}

/// The cited machine's sum is its own subject too: one over another sum is
/// not the alias's subject, whatever its cases.
#[test]
fn a_theorem_over_another_sum_cannot_supply_the_alias() {
    let source = ONE_IS_POSITIVE
        .replacen(
            "data OneIsPositive",
            "data Other {\n    case One(x: u8);\n    case Two(y: u8);\n}\n\ndata OneIsPositive",
            1,
        )
        .replace(
            "machine one_is_positive(c: Choice)\nrequires c in Choice::One && c.x != 0u8",
            "machine one_is_positive(c: Other)\nrequires c in Other::One && c.x != 0u8",
        );
    assert!(
        source.contains("machine one_is_positive(c: Other)"),
        "{source}"
    );
    let verdict = stored(&source);
    let shown = format!("{verdict:?}");
    assert!(
        shown.contains("not the aliased contract's subjects and carriers"),
        "{shown}"
    );
}

/// A theorem's order premise and conclusion with the citation at `actual`.
fn ordered(premise: &str, conclusion: &str, actual: &str) -> String {
    ONE_IS_NONZERO
        .replace("requires n == 1u8", &format!("requires {premise}"))
        .replace("ensures n != 0u8", &format!("ensures {conclusion}"))
        .replace("(1u8)", &format!("({actual})"))
}

/// Integer order between literals: the complete implication `n < 9` to
/// `n <= 8` is closed over every `u8`, not only the goals at its citations.
#[test]
fn a_strict_bound_theorem_concludes_the_inclusive_bound_below_it() {
    let source = ordered("n < 9u8", "n <= 8u8", "5u8");
    assert!(source.contains("ensures n <= 8u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// `n < 9` leaves `n = 8`, so it does not give `n <= 7`.
#[test]
fn a_strict_bound_theorem_cannot_conclude_two_steps_inside_it() {
    let source = ordered("n < 9u8", "n <= 7u8", "5u8");
    assert!(source.contains("ensures n <= 7u8"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// A literal on the left moves up: `3 < n` gives `4 <= n`.
#[test]
fn a_strict_lower_bound_theorem_concludes_the_inclusive_bound_above_it() {
    let source = ordered("3u8 < n", "4u8 <= n", "5u8");
    assert!(source.contains("ensures 4u8 <= n"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// `3 < n` leaves `n = 4`, so it does not give `5 <= n`.
#[test]
fn a_strict_lower_bound_theorem_cannot_conclude_two_steps_above_it() {
    let source = ordered("3u8 < n", "5u8 <= n", "6u8");
    assert!(source.contains("ensures 5u8 <= n"), "{source}");
    let verdict = Program::of(&source).judged(|_| {});
    assert!(!matches!(verdict, Verdict::Accepted), "{verdict:?}");
}

/// The same at a signed carrier with a negative literal: `n < -3` gives
/// `n <= -4`, and not `n <= -5`.
#[test]
fn a_signed_strict_bound_theorem_concludes_only_the_adjacent_bound() {
    let signed = |conclusion: &str| ordered("n < -3i8", conclusion, "-9i8").replace("u8", "i8");
    let accepted = signed("n <= -4i8");
    let verdict = Program::of(&accepted).judged(|_| {});
    assert!(
        matches!(verdict, Verdict::Accepted),
        "{accepted}\n{verdict:?}"
    );
    let refused = signed("n <= -5i8");
    let verdict = Program::of(&refused).judged(|_| {});
    assert!(
        !matches!(verdict, Verdict::Accepted),
        "{refused}\n{verdict:?}"
    );
}
