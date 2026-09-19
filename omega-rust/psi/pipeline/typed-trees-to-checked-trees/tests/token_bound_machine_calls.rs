//! A resolved use of a token-bearing machine is supplied by that machine's own
//! checked body: checking binds the spelled use to an ordinary call on the
//! declaration's entry state, retains the authored operator occurrence as a
//! selection of that declaration, and rejects the token positions whose body
//! supply is not implemented instead of leaving them to builtin arithmetic.

use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
};
use typed_trees::expression::ExpressionNode;

fn check(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    typed_trees_to_checked_trees::lower_typed_trees(typed)
}

const WRAPPED_ADD: &str = "data Wrapped { value: u8; }
    machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
        (left.value as u64) + (right.value as u64)
    }
    machine by_token(left: Wrapped, right: Wrapped) -> u64 { left + right }";

#[test]
fn binary_token_use_becomes_an_ordinary_call_on_the_declaration_entry() {
    let checked = check(WRAPPED_ADD).expect("the token use binds the declaration body");
    let add = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Wrapped::add")
        .expect("token-bearing declaration");
    let add_entry = checked.machine_states(add)[0].symbol;
    let synthesized = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target_symbol == add_entry => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = synthesized.as_slice() else {
        panic!(
            "exactly one call binds the token use; found {}",
            synthesized.len()
        );
    };
    assert_eq!(
        call.operational_acknowledgement.origin,
        language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
    );
    assert_eq!(
        checked
            .expression_table
            .expression_handles(call.arguments)
            .len(),
        2,
        "both operands forward in authored order"
    );
    let remaining_additions = checked
        .expression_table
        .iter_expressions()
        .filter(|(_, expression)| {
            matches!(expression, ExpressionNode::Binary(binary)
                if binary.operator == typed_trees::expression::BinaryOperator::Add)
        })
        .count();
    assert_eq!(
        remaining_additions, 1,
        "only the builtin `u64 + u64` inside `Wrapped::add` stays a spelled operator"
    );
    let operator_selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .map(|selection| selection.target())
        .collect::<Vec<_>>();
    assert!(
        operator_selections.contains(&AuthoredDeclarationSelectionTarget::Resolved(
            language_semantics::declaration_selection::ResolvedAuthoredDeclarationSelection::new(
                add.symbol
            )
            .unwrap()
        )),
        "the authored `+` occurrence settles to the selected declaration: {operator_selections:?}"
    );
}

/// Indexed `[]` uses now bind the declaration body: `target[0u64]` rewrites
/// to an ordinary call on `Wrapped::at`'s entry state with the collection and
/// index forwarded in authored order, keeping the authored `[]` occurrence as
/// a selection of that declaration.
#[test]
fn index_token_use_binds_the_declaration_body() {
    let checked = check(
        "data Wrapped { value: u8; }
        machine [] Wrapped::at(target: Wrapped, index: u64) -> u8 { target.value }
        machine by_token(target: Wrapped) -> u8 { target[0u64] }",
    )
    .expect("an index token use binds the declaration body");
    let at = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Wrapped::at")
        .expect("token-bearing declaration");
    let at_entry = checked.machine_states(at)[0].symbol;
    let synthesized = checked
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Call(call) if call.target_symbol == at_entry => Some(call),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [call] = synthesized.as_slice() else {
        panic!(
            "exactly one call binds the token use; found {}",
            synthesized.len()
        );
    };
    assert_eq!(
        call.operational_acknowledgement.origin,
        language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
    );
    assert_eq!(
        checked
            .expression_table
            .expression_handles(call.arguments)
            .len(),
        2,
        "collection and index forward in authored order"
    );
    // `Wrapped::at`'s first parameter is an ordinary parameter, so no receiver
    // loan forms at the call edge.
    assert!(!call.receiver.is_valid());
}

/// The positions whose body supply is still unimplemented keep rejecting
/// rather than falling back to builtin indexing: an open range use has no
/// `end` bound to forward to the declaration.
#[test]
fn open_range_token_use_rejects_instead_of_falling_back() {
    let diagnostics = check(
        "data Wrapped { value: u8; }
        machine [..] Wrapped::window(target: Wrapped, start: u64, end: u64) -> u8 {
            target.value
        }
        machine by_token(target: Wrapped) -> u8 { target[0u64..] }",
    )
    .expect_err("an open range use has no implemented body supply");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("the range use is open or inclusive")),
        "{diagnostics:?}"
    );
}

const ADDITIVE_QUANTITY: &str = "data Quantity { value: i32; }
    domain Quantity::Additive requires self.value >= 0;
    machine + Quantity::Additive::add(left: Quantity, right: Quantity) -> Quantity {
        Quantity { value: ((left.value as i32 in Wrapping) + (right.value as i32 in Wrapping)) as i32 }
    }";

fn calls_targeting(checked: &checked_trees::CheckedTrees, machine_name: &str) -> usize {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("declaration");
    let entry = checked.machine_states(machine)[0].symbol;
    checked
        .expression_table
        .iter_expressions()
        .filter(|(_, expression)| {
            matches!(expression, ExpressionNode::Call(call) if call.target_symbol == entry)
        })
        .count()
}

#[test]
fn domain_homed_token_use_binds_the_body_only_where_the_domain_is_selected() {
    // A signature `requires` selects the domain for `left`, so `left + right`
    // is the domain-family meaning and binds `Quantity::Additive::add`'s body.
    let selected = check(&format!(
        "{ADDITIVE_QUANTITY}
        machine combine(left: Quantity, right: Quantity) -> Quantity
        requires left in Quantity::Additive
        {{ left + right }}"
    ))
    .expect("a selected domain meaning binds the declaration body");
    assert_eq!(calls_targeting(&selected, "Quantity::Additive::add"), 1);

    // Nothing selects the domain: the data operands have no meaning at all,
    // which rejects instead of quietly reaching the body or a builtin.
    let diagnostics = check(&format!(
        "{ADDITIVE_QUANTITY}
        machine combine(left: Quantity, right: Quantity) -> Quantity {{ left + right }}"
    ))
    .expect_err("an unselected domain meaning is inadmissible for data operands");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no admissible meaning")),
        "{diagnostics:?}"
    );
}

#[test]
fn unselected_domain_binding_over_a_builtin_carrier_keeps_the_builtin_meaning() {
    // Declaring `machine + i32::Degrees::add` does not replace ordinary `i32`
    // addition: `i32 in Wrapping` operands select no `Degrees` binding, so
    // the use stays a builtin operator and no call to the body exists.
    let checked = check(
        "domain i32::Degrees requires self >= 0;
        machine + i32::Degrees::add(left: i32, right: i32) -> i32 {
            ((left as i32 in Wrapping) + (right as i32 in Wrapping)) as i32
        }
        machine rotate(value: i32 in Wrapping, delta: i32 in Wrapping) -> i32 in Wrapping {
            value + delta
        }",
    )
    .expect("builtin wrapping addition stays selected");
    assert_eq!(calls_targeting(&checked, "i32::Degrees::add"), 0);
}
