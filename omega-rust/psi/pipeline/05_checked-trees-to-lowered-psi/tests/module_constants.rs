use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source::SourceMap;
use std::path::PathBuf;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

#[test]
fn module_constants_publish_exact_values_without_producer_state() {
    let mut sources = SourceMap::default();
    let mut texts = Vec::new();
    for (path, source) in [
        (
            "combat.omg",
            "module combat; const DAMAGE: u32 = 7; machine value() -> u32\nrequires 7u32 == 7u32\nensures 7u32 == 7u32\n{ DAMAGE }",
        ),
        (
            "rooms.omg",
            "module rooms; const DAMAGE: u32 = 9; machine value() -> u32\nrequires 9u32 == 9u32\nensures 9u32 == 9u32\n{ DAMAGE }",
        ),
        (
            "main.omg",
            "use combat::DAMAGE; use rooms; machine imported() -> u32\nrequires 7u32 == 7u32\nensures 7u32 == 7u32\n{ DAMAGE } machine qualified() -> u32\nrequires 9u32 == 9u32\nensures 9u32 == 9u32\n{ rooms::DAMAGE }",
        ),
    ] {
        let source_id = sources
            .add(PathBuf::from(path), source.to_owned())
            .source_id;
        texts.push((source_id, source));
    }
    let checked = crate::front_end::checked_program_from_source_map(sources, &texts);
    let mut artifacts = Vec::new();
    for (qualified, expected) in [
        ("combat::value", 7u128),
        ("rooms::value", 9u128),
        ("imported", 7u128),
        ("qualified", 9u128),
    ] {
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name(qualified),
        )
        .expect("lower constant consumer");
        artifacts.push((
            encode_module(&lowered.semantic_module).expect("encode semantics"),
            encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
                .expect("encode proof"),
            expected,
        ));
    }
    drop(checked);
    for (semantics, proof, expected) in artifacts {
        let result =
            interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), &[])
                .expect("verify and execute independent constant artifact");
        assert_eq!(
            result,
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"),
                value: IntegerValue::Unsigned(expected),
            })
        );
    }
}
