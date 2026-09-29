//! A bounded byte local constructed from a literal establishes the declared
//! carrier; raw arrays and mutable locals keep their own producers.
use crate::tests::front_end::checked_program_result;
use checked_trees::{CheckedTrees, CheckedUnitEffectOperationPlan};

const DOMAIN: &str = "domain [u8; 8]::Utf8 requires valid_utf8(self);";

fn checked(source: &str) -> CheckedTrees {
    checked_program_result(&format!("{DOMAIN}\n{source}"))
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

fn machine_named(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing machine `{name}`"))
        .symbol
}

/// Every literal establishment the machine's single-body plan retains, as
/// (statement, carrier identity, octets).
fn literal_establishments(checked: &CheckedTrees, name: &str) -> Vec<(u32, String, Vec<u8>)> {
    checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(checked, name))
        .map(|plan| {
            plan.operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishByteSequenceLiteral {
                        result,
                        bytes,
                    } => Some((
                        result.statement_index,
                        result.type_identity.clone(),
                        bytes.clone(),
                    )),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A literal shorter than the capacity keeps its own octets: the plan carries
/// "150", not eight bytes, and names the same carrier identity a borrowed
/// `&[u8; 8] in Utf8` formal registers, which a later call loan must match.
#[test]
fn a_shorter_literal_establishes_the_declared_bounded_carrier() {
    let checked = checked(
        r#"
        machine hold() {
            let text: [u8; 8] in Utf8 = "150";
        }
        machine peek(text: &[u8; 8] in Utf8) {
        }
    "#,
    );
    let formal = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(machine_named(&checked, "peek"))
        .expect("a borrowed bounded formal plans")
        .structural_parameters[0]
        .type_identity
        .clone();
    assert_eq!(
        literal_establishments(&checked, "hold"),
        vec![(0, formal, b"150".to_vec())]
    );
}

/// A raw `[u8; N]` is an exact-width fixed array and a mutable bounded local
/// is replaceable storage; neither is constructed by this producer.
#[test]
fn raw_arrays_and_mutable_bounded_locals_are_not_literal_constructions() {
    let checked = checked(
        r#"
        machine raw() {
            let bytes: [u8; 3] = "abc";
        }
        machine replaceable() {
            let mut text: [u8; 8] in Utf8 = "150";
        }
    "#,
    );
    assert!(literal_establishments(&checked, "raw").is_empty());
    assert!(literal_establishments(&checked, "replaceable").is_empty());
}
