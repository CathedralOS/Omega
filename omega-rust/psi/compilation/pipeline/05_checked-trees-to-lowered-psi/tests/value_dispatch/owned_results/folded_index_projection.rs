//! Owned indexed leaves lower the same way a literal index spells them:
//! `items[1]` and `items[0 + 1]` plan the same checked `FixedIndex` leaf, so
//! the widened fold adds no downstream divergence. Both locals are built from
//! a fixed-array literal of records, which establishes as zeroed storage plus
//! one authored store per leaf; the selected element then observes its field.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::{StructuralPathSegment, Terminator};

const LITERAL_SOURCE: &str = "data Cell { tag: u64; }
    data Pack { items: [Cell; 2]; }
    machine choose(flag: bool) -> u64 {
        let left: Pack = Pack { items: [Cell { tag: 41 }, Cell { tag: 42 }] };
        let right: Pack = Pack { items: [Cell { tag: 43 }, Cell { tag: 44 }] };
        let picked: Cell = match flag { true -> left.items[1], false -> right.items[0] };
        picked.tag
    }";

const FOLDED_SOURCE: &str = "data Cell { tag: u64; }
    data Pack { items: [Cell; 2]; }
    machine choose(flag: bool) -> u64 {
        let left: Pack = Pack { items: [Cell { tag: 41 }, Cell { tag: 42 }] };
        let right: Pack = Pack { items: [Cell { tag: 43 }, Cell { tag: 44 }] };
        let picked: Cell = match flag { true -> left.items[0 + 1], false -> right.items[0] };
        picked.tag
    }";

/// Both spellings check — the fold produces exactly the checked `FixedIndex`
/// leaf a literal does — and both lower to verified modules that move the
/// same selected element out of the same record member.
#[test]
fn folded_index_selects_the_literal_element() {
    let moved = |label: &str, source: &str| {
        let checked = crate::front_end::checked_program_result(source)
            .unwrap_or_else(|errors| panic!("{label}: {errors:#?}"));
        let lowered = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("choose"),
        )
        .unwrap_or_else(|error| panic!("{label}: {error:?}"));
        let module = terminal_codec::decode_module(
            &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
        )
        .unwrap();
        let proof = terminal_codec::decode_proof_bundle(
            &terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
                .unwrap(),
        )
        .unwrap();
        terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
            .unwrap_or_else(|error| panic!("{label}: {error:?}"));
        module.machines[0]
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator {
                Terminator::Jump {
                    structural_arguments,
                    ..
                } => Some(structural_arguments),
                _ => None,
            })
            .flatten()
            .map(|argument| argument.path.clone())
            .filter(|path| !path.is_empty())
            .collect::<Vec<_>>()
    };
    let item = |index| {
        vec![
            StructuralPathSegment::Field("items".to_owned()),
            StructuralPathSegment::FixedIndex(index),
        ]
    };
    let literal = moved("literal", LITERAL_SOURCE);
    assert_eq!(
        literal,
        [item(1), item(0)],
        "each arm moves its selected element"
    );
    assert_eq!(moved("folded", FOLDED_SOURCE), literal);
}
