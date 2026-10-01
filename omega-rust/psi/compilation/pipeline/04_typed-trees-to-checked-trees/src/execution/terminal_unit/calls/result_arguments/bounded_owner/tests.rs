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
        .unwrap_or_else(|| {
            panic!(
                "missing machine `{name}`; available machines: {:?}",
                checked
                    .machines()
                    .iter()
                    .map(|machine| machine.name.as_str())
                    .collect::<Vec<_>>()
            )
        })
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

/// `text[0]` of a loaned bounded owner is bounded by the live length, not
/// the capacity: the read stays a runtime-indexed byte read with a literal
/// selector instead of a fixed-index projection into the capacity.
#[test]
fn a_literal_index_reads_a_bounded_owner_against_its_live_length() {
    let checked = checked(
        r#"
        machine first(text: &[u8; 8] in Utf8) -> u8 {
            transition 0 < text.len {
                true -> read(text)
                _ -> (0)
            }

            state read(text: &[u8; 8] in Utf8) -> u8 {
                let byte: u8 = text[0];
                transition { _ -> (byte) }
            }
        }
    "#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "first")
        .expect("reader machine");
    let read = &checked.machine_states(machine)[1];
    let expression = checked
        .facts
        .values
        .scalar_expressions
        .expression_at(
            read.symbol,
            0,
            checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
        )
        .expect("the byte local retains its read");
    assert!(
        matches!(
            expression,
            checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
                path,
                index,
                ..
            } if path.is_empty()
                && matches!(**index, checked_trees::CheckedScalarExpression::IntegerLiteral { .. })
        ),
        "{expression:?}"
    );
}

/// A single-state reader's `text in Utf8` restatement rides the parameter
/// declaration; it is not a runtime entry requirement the body must evaluate.
#[test]
fn a_bounded_owner_membership_restatement_is_no_runtime_requirement() {
    let checked = checked(
        r#"
        machine live_length(text: &[u8; 8] in Utf8) -> u64 {
            transition { _ -> (text.len) }
        }
    "#,
    );
    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine_named(&checked, "live_length"))
        .expect("the reader publishes a contract");
    assert_eq!(
        contract
            .crash
            .structural_runtime_requirements()
            .map(|requirements| requirements.len()),
        Some(0)
    );
}

const OWNED_BOUNDARY_SOURCE: &str = r#"
    boundary trait Output { machine write(bytes: &[u8]) reaches Output; }
    data Root {}
    machine Root::emit() reaches Output {
        let mut text: [u8; 8] in Utf8 = "abc";
        text[1] = 65;
        Output::write(text);
    }
"#;

#[test]
fn a_mutated_bounded_owner_lends_its_live_bytes_to_a_shared_boundary_view() {
    for source in [
        OWNED_BOUNDARY_SOURCE.to_owned(),
        OWNED_BOUNDARY_SOURCE.replace("write(text)", "write(&text)"),
    ] {
        let checked = checked(&source);
        let machine = machine_named(&checked, "Root::emit");
        let plans = &checked.facts.flow.terminal_unit_effects;
        let plan = plans.for_machine(machine).unwrap_or_else(|| {
            panic!(
                "the established owner supplies an exact shared byte view: {:#?}\nsource:\n{source}",
                plans.omission_for_machine(machine)
            )
        });
        let literal = plan
            .operations
            .iter()
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::EstablishByteSequence {
                    result,
                    qualifications,
                    ..
                } => Some((result, qualifications)),
                _ => None,
            })
            .unwrap();
        assert_eq!(literal.1.len(), 1, "the owner keeps its declared invariant");
        let argument = plan
            .operations
            .iter()
            .filter_map(|operation| operation.call())
            .filter(|call| call.is_boundary())
            .flat_map(|call| &call.structural_arguments)
            .next()
            .expect("boundary structural argument");
        assert_eq!(
            argument.source_structural_result_binding_ordinal(),
            Some(literal.0.binding_ordinal)
        );
        assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
        assert!(argument.path.is_empty());
        assert_ne!(
            argument.type_identity, literal.0.type_identity,
            "view presentation does not change the owner's carrier"
        );
    }
}

#[test]
fn a_bounded_owner_shared_view_requires_its_exact_captured_read_access() {
    let checked = checked(OWNED_BOUNDARY_SOURCE);
    let machine = machine_named(&checked, "Root::emit");
    let plans = &checked.facts.flow.terminal_unit_effects;
    let plan = plans.for_machine(machine).unwrap_or_else(|| {
        panic!(
            "owner boundary plan: {:#?}",
            plans.omission_for_machine(machine)
        )
    });
    let borrow_state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| state.machine_symbol == machine && state.state_symbol == plan.state)
        .unwrap();
    let call = checked
        .facts
        .borrow
        .calls
        .span_or_empty(borrow_state.calls)
        .iter()
        .find(|call| call.statement_index == 2)
        .unwrap();
    let accesses = call.accesses;
    assert!(
        !checked
            .facts
            .borrow
            .argument_accesses
            .span_or_empty(accesses)
            .is_empty()
    );
    for kind in [
        checked_trees::BorrowAccessKind::Mutable,
        checked_trees::BorrowAccessKind::WriteOnly,
    ] {
        let mut facts = checked.facts.clone();
        for access in facts.borrow.argument_accesses.span_mut(accesses).unwrap() {
            access.kind = kind.clone();
        }
        let rebuilt = crate::execution::terminal_unit::build_checked_unit_effect_plans(
            &checked.typed,
            &facts,
            crate::execution::terminal_unit::ScalarCalleePlans {
                boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                structural_returns: &facts.flow.terminal_structural_scalar_returns,
            },
        );
        assert!(
            rebuilt.for_machine(machine).is_none(),
            "a different captured access cannot authorize this shared presentation"
        );
    }
}
