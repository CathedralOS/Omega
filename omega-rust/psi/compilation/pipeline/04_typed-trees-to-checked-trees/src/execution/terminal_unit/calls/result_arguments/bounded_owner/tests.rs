//! A bounded owner local lends whole to a static machine, and a successor
//! state receives a qualified shared loan only by exact forwarding.
use crate::tests::front_end::checked_program_result;
use checked_trees::{
    CheckedStructuralAccess, CheckedTrees, CheckedUnitStructuralArgumentSourcePlan,
};

const DOMAIN: &str = "domain [u8; 8]::Utf8 requires valid_utf8(self);";

const CALLEE: &str = r#"
    machine forwarded_length(text: &[u8; 8] in Utf8) -> u64 {
        transition { _ -> measure(text) }

        state measure(text: &[u8; 8] in Utf8) -> u64 {
            transition { _ -> (text.len) }
        }
    }
"#;

fn checked(source: &str) -> CheckedTrees {
    checked_program_result(&format!("{DOMAIN}\n{CALLEE}\n{source}"))
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

/// `forwarded_length(&text)` lends the literal-built owner whole: the
/// argument names the local's result binding under a shared loan, and the
/// callee's successor state receives the same qualified loan it forwards.
#[test]
fn a_bounded_owner_local_lends_whole_and_its_loan_forwards_qualified() {
    let checked = checked(
        r#"
        machine main() -> u64 {
            let text: [u8; 8] in Utf8 = "150";
            let length: u64 = forwarded_length(&text);
            transition { _ -> (length) }
        }
    "#,
    );
    let flow = &checked.facts.flow.terminal_unit_effects;
    let entry = flow
        .for_machine(machine_named(&checked, "main"))
        .expect("the lending entry plans");
    let loans = entry
        .operations
        .iter()
        .filter_map(|operation| operation.call())
        .flat_map(|call| &call.structural_arguments)
        .map(|argument| {
            (
                argument.source.clone(),
                argument.access,
                argument.path.len(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        loans,
        vec![(
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal: 0 },
            CheckedStructuralAccess::SharedBorrow,
            0
        )]
    );
    let callee = flow
        .composed_for_machine(machine_named(&checked, "forwarded_length"))
        .expect("the forwarding callee plans as a state graph");
    let successor = &callee.states[1].structural_parameters;
    assert!(
        successor.len() == 1
            && successor[0].access == CheckedStructuralAccess::SharedBorrow
            && !successor[0].qualifications.is_empty(),
        "the successor keeps the owner loan's qualifications: {successor:?}"
    );
}

/// A fresh `&text` loan into a qualified successor state is not a forwarded
/// loan: nothing on that edge carries the qualification, so the state graph
/// declines rather than granting the successor its declared membership.
#[test]
fn a_fresh_local_loan_cannot_enter_a_qualified_successor_state() {
    let checked = checked(
        r#"
        machine main() -> u64 {
            let text: [u8; 8] in Utf8 = "150";
            transition { _ -> measure(&text) }

            state measure(text: &[u8; 8] in Utf8) -> u64 {
                transition { _ -> (text.len) }
            }
        }
    "#,
    );
    assert!(
        checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_for_machine(machine_named(&checked, "main"))
            .is_none()
    );
}
