//! Each domain cast changes the qualification of one value on one control
//! edge. Stage 05 writes the cast as a coercion row from the value before
//! the cast to a fresh block parameter that carries exactly the domains the
//! cast leaves, and the lowered-form checker follows that chain to decide
//! what a value is known to be a member of.
//!
//! The spec rule, `wiki/spec/language/domains.md`, "Domain-only casts": a
//! cast `value as =A -B +C` keeps the carrier, and "The result carries the
//! retained and added domains".

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, Verdict};
use semantic_vocabulary::ValueId;
use terminal_psi::TerminalMachine;

fn chain(first: &str, second: &str) -> String {
    format!(
        "domain u8::A; domain u8::B; machine f(x: u8) -> u8 {{ \
         let both: u8 in A & B = x as +{first} +{second}; let only: u8 in B = both as -A =B; only as -B }}"
    )
}

/// The domain names in the qualification set of `value`, sorted.
fn domains_of(program: &Program, machine: &TerminalMachine, value: ValueId) -> Vec<String> {
    let set = machine
        .blocks
        .iter()
        .flat_map(|block| &block.parameters)
        .find(|parameter| parameter.id == value)
        .unwrap_or_else(|| panic!("{value:?} is a block parameter"))
        .qualifications;
    let catalog = &program.elaboration().module.types.scalar_qualifications;
    let mut names: Vec<_> = catalog
        .sets
        .iter()
        .find(|candidate| candidate.id == set)
        .map(|set| set.domains.clone())
        .unwrap_or_default()
        .iter()
        .map(|domain| {
            let declared = catalog
                .domains
                .iter()
                .find(|row| row.id == *domain)
                .unwrap();
            let text = program
                .elaboration()
                .module
                .identities
                .text(declared.identity)
                .unwrap()
                .to_owned();
            ["A", "B"]
                .into_iter()
                .find(|name| text.contains(&format!("path\\(u8::{name}\\)")))
                .unwrap_or_else(|| panic!("an unknown domain: {text}"))
                .to_owned()
        })
        .collect();
    names.sort();
    names
}

/// The first cast adds both domains on the same edge in either spelling.
/// Each subsequent cast consumes exactly the preceding edge's value.
#[test]
fn each_cast_edge_carries_the_previous_value_into_exactly_its_domains() {
    for (first, second) in [("A", "B"), ("B", "A")] {
        let program = Program::of(&chain(first, second));
        let f = program.machine("f");
        let x = f.contract.parameters[0].id;
        let coercions: Vec<_> = program
            .elaboration()
            .module
            .types
            .scalar_qualifications
            .coercions
            .iter()
            .filter(|row| row.machine == f.id)
            .collect();
        assert_eq!(coercions.len(), 3, "{coercions:?}");
        let mut previous = x;
        for row in &coercions {
            assert_eq!(row.source, previous, "{row:?}");
            previous = row.destination;
        }
        let sets: Vec<_> = coercions
            .iter()
            .map(|row| domains_of(&program, f, row.destination))
            .collect();
        assert_eq!(
            sets,
            [vec!["A", "B"], vec!["B"], vec![]],
            "+{first} +{second}"
        );
        program.judged(|_| {}).accepted();

        // A coercion that reads `x` again instead of the value the previous
        // cast produced.
        let skipped = coercions[1].edge;
        let verdict = program.judged(|lowered| {
            for row in &mut lowered.module.types.scalar_qualifications.coercions {
                if row.edge == skipped {
                    row.source = x;
                }
            }
        });
        let Verdict::Refused(refusal) = verdict else {
            panic!("the skipped coercion must be refused: {verdict:?}")
        };
        assert!(
            refusal
                .detail
                .contains("scalar coercion is not bound to its exact successor argument"),
            "{refusal:?}"
        );
    }
}

#[test]
fn a_simultaneous_cast_forms_the_complete_qualified_carrier() {
    for (adds, removes) in [
        ("+Selected +Positive", "-Positive -Selected"),
        ("+Positive +Selected", "-Selected -Positive"),
    ] {
        let program = Program::of(&format!(
            "domain u16::Positive requires self > 0u16;
             data Count = u16 in Positive;
             domain Count::Selected;
             machine make() -> Count in Selected {{ 7u16 as {adds} }}
             machine drop(x: Count in Selected) -> u16 {{ x as {removes} }}"
        ));
        let catalog = &program.elaboration().module.types.scalar_qualifications;
        for name in ["make", "drop"] {
            let machine = program.machine(name).id;
            assert_eq!(
                catalog
                    .coercions
                    .iter()
                    .filter(|row| row.machine == machine)
                    .count(),
                1
            );
        }
        program.judged(|_| {}).accepted();
    }
}
