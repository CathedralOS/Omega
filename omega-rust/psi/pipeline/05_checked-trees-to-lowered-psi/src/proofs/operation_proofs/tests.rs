use super::{
    CertificateEnvelope, EvidenceIdentity, EvidenceRoute, LoweredPsi, LoweringError,
    ObligationEvidence, ProofSystemMarker, Proposition, finalize_operation_proofs,
    finalize_operation_proofs_inner, lower_machine, obligation_id,
    produce_checked_canonical_integer_proof,
};
use crate::TerminalMachineSelection;
use crate::terminal_identities::block_id;

#[test]
fn sequential_clamps_prove_exact_arithmetic_on_an_observed_field() {
    for expression in ["bounded + 70", "70 + bounded", "bounded + -70"] {
        let lowered = clamped_field_addition(expression);
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("the independent verifier reconstructs the clamp and addition obligations");
    }
}

fn clamped_field_addition(expression: &str) -> LoweredPsi {
    let checked = crate::front_end::checked_program(&format!(
        "data Main {{ seed: i32; result: i32; }}
         machine Main::run(&mut self) {{
             let low: i32 = max(self.seed, 0);
             let bounded: i32 = min(low, 60);
             self.result = {expression};
         }}",
    ));
    lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("clamp bounds justify the exact addition")
}

fn fixture(obligation_count: usize) -> LoweredPsi {
    let mut lowered = lower_source("machine root(value: u64) -> u64 { value }");
    lowered.semantic_module.machines[0].contract.ensures = (0..obligation_count)
        .map(|position| terminal_psi::ContractClause {
            obligation: obligation_id(1_000_000 + position as u64),
            proposition: Proposition::Truth,
        })
        .collect();
    lowered.proof_bundle.evidence.clear();
    lowered
}

fn lower_source(source: &str) -> LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    lower_machine(&checked, TerminalMachineSelection::Name("root")).unwrap()
}

// Retain the previous per-obligation preparation as a same-process reference.
// The fixture uses contract clauses so each pending row takes the canonical path.
fn uncached_reference(lowered: &mut LoweredPsi) -> usize {
    let validated = terminal_verifier::validate_module(&lowered.semantic_module).unwrap();
    let obligations =
        terminal_verifier::reconstruct_terminal_obligations(&lowered.semantic_module).unwrap();
    assert_eq!(
        obligations,
        terminal_verifier::reconstruct_execution_terminal_obligations(validated).unwrap()
    );
    let mut preparations = 0;
    for site in obligations.obligations() {
        if lowered
            .proof_bundle
            .evidence
            .iter()
            .any(|evidence| evidence.obligation == site.obligation.id)
        {
            continue;
        }
        let machine = validated.machine(site.owner.machine()).unwrap();
        let context = validated.value_context(machine).unwrap();
        let parameters = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect();
        preparations += 1;
        let proof = produce_checked_canonical_integer_proof(
            &context,
            &site.obligation.proposition,
            &site.requirements,
            &site.semantic_axioms,
            &parameters,
        )
        .unwrap();
        lowered.proof_bundle.evidence.push(ObligationEvidence {
            obligation: site.obligation.id,
            route: EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity: EvidenceIdentity::new(site.obligation.id.get()).unwrap(),
                proof_system_marker: ProofSystemMarker::CURRENT,
                proof,
            }),
        });
    }
    lowered
        .proof_bundle
        .evidence
        .sort_by_key(|evidence| evidence.obligation);
    crate::proofs::control_cycle_proofs::finalize(lowered).unwrap();
    preparations
}

#[test]
fn retained_evidence_needs_no_preparation_and_mutated_proof_still_rejects() {
    let mut lowered = fixture(17);
    uncached_reference(&mut lowered);
    let original = lowered.clone();
    finalize_operation_proofs_inner(&mut lowered, &|_| panic!("no pending evidence")).unwrap();
    assert_eq!(lowered, original);

    let EvidenceRoute::CertificateDerived(certificate) =
        &mut lowered.proof_bundle.evidence[0].route
    else {
        panic!("fixture certificate");
    };
    certificate.proof.conclusion = Proposition::Falsehood;
    let corrupted = lowered.proof_bundle.clone();
    finalize_operation_proofs_inner(&mut lowered, &|_| {
        panic!("retained evidence is not regenerated")
    })
    .unwrap();
    assert_eq!(lowered.proof_bundle, corrupted);
    assert!(
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .is_err()
    );
}

#[test]
fn invalid_module_rejects_before_preparation() {
    let mut lowered = fixture(17);
    lowered.semantic_module.machines[0].entry = block_id(u64::MAX);
    assert!(matches!(
        finalize_operation_proofs_inner(&mut lowered, &|_| panic!("invalid module")),
        Err(LoweringError::InvalidTerminalModule(_))
    ));
}

// The `as u8` cast obligation on `48 + (remaining % 10)` needs the wrapping
// chain `sum = WrapAdd(48, remainder)` to carry the remainder's synthesized
// `<= 9` bound into `sum <= 57`, then relax through transitivity to 255.
// Before operand-derived seeds joined the wrapping transport the machine
// declined with OperationProofUnavailable.
#[test]
fn wrapping_add_chain_carries_remainder_bound_to_cast_obligation() {
    let checked = crate::front_end::checked_program(
        r#"
        data Text {
            bytes: [u8; 16];
            len: u64;
        }
        machine Text::put_digits_reversed(&mut self, remaining: u64, digit_index: u64) {
            transition digit_index < 4 {
                true -> store_digit(remaining, digit_index)
                false -> done()
            }
            state store_digit(&mut self, remaining: u64, digit_index: u64) {
                self.bytes[digit_index] = (((48 as u64 in Wrapping) + ((remaining % 10) as u64 as u64 in Wrapping)) as u64) as u8;
                self.len = (((digit_index as u64 in Wrapping) + (1 as u64 in Wrapping)) as u64);
                transition remaining >= 10 {
                    true -> self.put_digits_reversed(remaining / 10, (((digit_index as u64 in Wrapping) + (1 as u64 in Wrapping)) as u64))
                    false -> done()
                }
            }
            state done(&mut self) {
            }
        }
        "#,
    );
    let lowered = lower_machine(
        &checked,
        TerminalMachineSelection::Name("Text::put_digits_reversed"),
    )
    .unwrap();
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
}

#[test]
fn case_payload_structural_result_is_an_owned_return() {
    let checked = crate::front_end::checked_program(
        r#"
        data MemoryAlignment [copy] {
            case Alignment1;
            case Alignment2;
            case Alignment4;
            case Alignment8;
        }
        data AlignmentParseResult [copy] {
            case Invalid;
            case Parsed(alignment: MemoryAlignment);
        }
        machine MemoryAlignment::get_size_in_bytes(&self) -> u64 [1..=8] {
            transition self {
                MemoryAlignment::Alignment1 -> (1)
                MemoryAlignment::Alignment2 -> (2)
                MemoryAlignment::Alignment4 -> (4)
                MemoryAlignment::Alignment8 -> (8)
            }
        }
        machine AlignmentParseResult::make() -> AlignmentParseResult {
            AlignmentParseResult::Parsed { alignment: MemoryAlignment::Alignment1 }
        }
        "#,
    );
    for machine in [
        "MemoryAlignment::get_size_in_bytes",
        "AlignmentParseResult::make",
    ] {
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine)).unwrap();
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap();
    }
}

#[test]
fn parallel_failures_keep_the_first_pending_obligation_and_publish_nothing() {
    let mut lowered = fixture(17);
    for position in [3, 12] {
        lowered.semantic_module.machines[0].contract.ensures[position].proposition =
            Proposition::Falsehood;
    }
    let first = lowered.semantic_module.machines[0].contract.ensures[3].obligation;
    let original = lowered.proof_bundle.clone();
    assert!(
        matches!(finalize_operation_proofs(&mut lowered), Err(LoweringError::OperationProofUnavailable(obligation)) if obligation == first)
    );
    assert_eq!(lowered.proof_bundle, original);
}
