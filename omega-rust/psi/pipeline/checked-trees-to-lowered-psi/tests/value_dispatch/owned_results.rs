//! Fixtures shared by the owned result tests: the owned match sources and
//! the membership helpers.

#[path = "owned_results/call_product_arms.rs"]
mod call_product_arms;
#[path = "owned_results/folded_index_projection.rs"]
mod folded_index_projection;
#[path = "owned_results/interleaved_and_projected_sources.rs"]
mod interleaved_and_projected_sources;
#[path = "owned_results/linear_child_carriers.rs"]
mod linear_child_carriers;
#[path = "owned_results/membership_subjects.rs"]
mod membership_subjects;
#[path = "owned_results/owned_match_records_and_selections.rs"]
mod owned_match_records_and_selections;

use crate::value_dispatch::check_source;

const SOURCE: &str =
    include_str!("../../../../../../tests/omega/pass/expressions/owned_match_values/main.omg");

const MIXED_SOURCE: &str = include_str!(
    "../../../../../../tests/omega/pass/expressions/owned_match_mixed_values/main.omg"
);

const RECORD_SOURCE: &str = include_str!(
    "../../../../../../tests/omega/pass/expressions/owned_match_record_values/main.omg"
);

const INTERLEAVED_SOURCE: &str = include_str!(
    "../../../../../../tests/omega/pass/expressions/owned_match_interleaved_values/main.omg"
);

const PROJECTED_FIELD_SOURCE: &str = include_str!(
    "../../../../../../tests/omega/pass/expressions/owned_match_projected_field/main.omg"
);

const PARAMETER_SOURCE: &str = include_str!(
    "../../../../../../tests/omega/pass/expressions/owned_match_parameter_values/main.omg"
);

const CALL_VALUE_SOURCE: &str =
    include_str!("../../../../../../tests/omega/pass/expressions/owned_match_call_values/main.omg");

const MEMBERSHIP_TYPES: &str = "data Kind { case Missing; case Other; }
    data Choice { case Empty; case Full; }";

fn membership_case(
    module: &terminal_psi::TerminalModule,
    type_name: &str,
    case_name: &str,
) -> semantic_vocabulary::StructuralCaseId {
    module
        .structural_types
        .iter()
        .find_map(|declaration| {
            let terminal_psi::StructuralTypeShape::Sum { cases } = &declaration.shape else {
                return None;
            };
            cases
                .iter()
                .find(|case| {
                    declaration.identity.contains(&format!("({type_name})"))
                        && case.identity == case_name
                })
                .map(|case| case.id)
        })
        .unwrap_or_else(|| {
            panic!(
                "{type_name}::{case_name} is declared: {:#?}",
                module
                    .structural_types
                    .iter()
                    .map(|declaration| match &declaration.shape {
                        terminal_psi::StructuralTypeShape::Sum { cases } => (
                            declaration.identity.as_str(),
                            cases
                                .iter()
                                .map(|case| case.identity.as_str())
                                .collect::<Vec<_>>(),
                        ),
                        _ => (declaration.identity.as_str(), Vec::new(),),
                    })
                    .collect::<Vec<_>>()
            )
        })
}

fn verify_membership_source(
    source: &str,
) -> (checked_trees::CheckedTrees, lowered_psi::LoweredPsi) {
    let checked =
        check_source(source).unwrap_or_else(|errors| panic!("checking {source}: {errors:#?}"));
    let machine = checked
        .machines()
        .iter()
        .next()
        .expect("membership machine")
        .symbol;
    let lowered = checked_trees_to_lowered_psi::lower_machine_by_symbol(&checked, machine)
        .unwrap_or_else(|error| panic!("lowering {source}: {error:#?}"));
    let semantic_bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics");
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("encode proof");
    let module = terminal_codec::decode_module(&semantic_bytes).expect("decode semantics");
    let proof = terminal_codec::decode_proof_bundle(&proof_bytes).expect("decode proof");
    terminal_verifier::verify_module(&module, &proof, &super::AdmissionProfile::default())
        .unwrap_or_else(|error| panic!("verifying {source}: {error:#?}"));
    (checked, lowered)
}
