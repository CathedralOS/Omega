//! `match` on a scalar-payload sum subject executes as ordered membership
//! selections end to end: lowered, verified, and interpreted.
use super::checked_source;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;

const CHOICE: &str = "data Choice { case Empty; case Some(value: u32); }";

fn interpret_integer(checked: &checked_trees::CheckedTrees, name: &str, expected: i64) {
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        terminal_production::TerminalMachineSelection::Name(name),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap_or_else(|error| panic!("{name} produces a terminal artifact: {error:?}"))
    .into_artifact();
    terminal_verifier::verify_module(
        &terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap_or_else(|_| panic!("{name} verifies"));
    let result = terminal_interpreter::interpret_terminal_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
    )
    .unwrap_or_else(|_| panic!("{name} interprets"));
    assert_eq!(
        result,
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: semantic_vocabulary::IntegerType::new(
                    semantic_vocabulary::IntegerSign::Signed,
                    64,
                )
                .unwrap(),
                value: semantic_vocabulary::IntegerValue::Signed(i128::from(expected)),
            }
        ),
        "{name} selects the exact arm value"
    );
}

#[test]
fn local_sum_match_executes_membership_selections() {
    let checked = checked_source(&format!(
        "{CHOICE}
         data Tag {{ case First; case Second; }}
         machine empty() -> i64 {{
             let c: Choice = Choice::Empty;
             match c {{ Choice::Empty -> 11, _ -> 33 }}
         }}
         machine fallback() -> i64 {{
             let c: Choice = Choice::Some {{ value: 9 }};
             match c {{ Choice::Empty -> 11, _ -> 33 }}
         }}
         machine second() -> i64 {{
             let t: Tag = Tag::Second;
             match t {{ Tag::First -> 11, Tag::Second -> 22, _ -> 33 }}
         }}"
    ));
    for (name, expected) in [("empty", 11), ("fallback", 33), ("second", 22)] {
        interpret_integer(&checked, name, expected);
    }
}

#[test]
fn self_field_sum_match_verifies_as_parameter_membership() {
    let checked = checked_source(&format!(
        "{CHOICE}
         data Holder {{ res: Choice }}
         machine Holder::classify(&mut self) -> i64 {{
             match self.res {{ Choice::Empty -> 1, _ -> 0 }}
         }}"
    ));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Holder::classify"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("attached-data field match produces a terminal artifact")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the membership selections verify");
    let observations = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::StructuralCaseMembership { .. }
            )
        })
        .count();
    assert_eq!(observations, 1, "the case arm observes the field's tag");
}
