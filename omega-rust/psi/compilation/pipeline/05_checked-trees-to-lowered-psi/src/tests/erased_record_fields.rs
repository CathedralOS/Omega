//! Erasure preserves semantic declarations while excluding their initializer
//! computations and cleanup from executable record construction.
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionRequest,
    TerminalProductionTimings,
};

fn execute(source: &str, expected: TerminalExecutionResult) {
    let artifact = {
        let checked = crate::front_end::checked_program(source);
        TerminalProductionRequest::new(&checked, TerminalMachineSelection::Name("run"))
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("erased fields retain checking without runtime initializers")
            .into_artifact()
    };
    let reloaded = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes())
        .expect("source-free record artifact reloads");
    let module = terminal_codec::decode_module(reloaded.semantic_bytes())
        .expect("decode the independent executable module");
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(
                operation.kind,
                terminal_psi::OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Signed(101)
                        | semantic_vocabulary::IntegerValue::Unsigned(101),
                }
            )),
        "the erased Evidence initializer must not become executable work"
    );
    let profile = proof_admission::AdmissionProfile::default();
    terminal_codec::verify_terminal_artifact_proof(&reloaded, &profile)
        .expect("independent replay admits the relevant field roster");
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            reloaded.semantic_bytes(),
            reloaded.proof_bytes(),
            &profile,
            &[],
        )
        .expect("erased initializers require no runtime storage or cleanup"),
        expected,
    );
}

#[test]
fn erased_record_fields_preserve_middle_ids_and_authored_order_in_scalar_calls() {
    execute(
        r#"
        data Evidence { tag: i32; }
        machine Evidence::drop(evidence: &mut Evidence) {}
        data Carrier { before: i32; proof [erased]: Evidence; after: i32; }
        machine inspect(carrier: &Carrier) -> bool {
            carrier.before == 17 && carrier.after == 29
        }
        machine run() -> bool {
            let carrier: Carrier = Carrier {
                after: 29, proof: Evidence { tag: 101 }, before: 17
            };
            inspect(&carrier)
        }
    "#,
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
}

#[test]
fn erased_record_fields_preserve_scalar_predicate_evidence() {
    execute(
        r#"
        domain i32::NotFive(value) requires value != 5;
        data Evidence { tag: i32; }
        machine Evidence::drop(evidence: &mut Evidence) {}
        data Carrier { before: i32; proof [erased]: Evidence; after: i32 in NotFive; }
        machine inspect(carrier: &Carrier) -> bool {
            carrier.before == 17 && carrier.after == 29
        }
        machine run() -> bool {
            let carrier: Carrier = Carrier {
                after: 29, proof: Evidence { tag: 101 }, before: 17
            };
            inspect(&carrier)
        }
    "#,
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
    );
}

#[test]
fn erased_record_fields_construct_and_retire_in_unit_bodies() {
    execute(
        r#"
        data Evidence { tag: i32; }
        machine Evidence::drop(evidence: &mut Evidence) {}
        data Carrier { before: i32; proof [erased]: Evidence; after: i32; }
        machine run() {
            let carrier: Carrier = Carrier {
                after: 29, proof: Evidence { tag: 101 }, before: 17
            };
        }
    "#,
        TerminalExecutionResult::Unit,
    );
}

#[test]
fn erased_record_fields_allow_an_empty_runtime_roster() {
    execute(
        r#"
        data Evidence { tag: i32; }
        machine Evidence::drop(evidence: &mut Evidence) {}
        data Carrier { proof [erased]: Evidence; }
        machine run() {
            let carrier: Carrier = Carrier { proof: Evidence { tag: 101 } };
        }
    "#,
        TerminalExecutionResult::Unit,
    );
}
