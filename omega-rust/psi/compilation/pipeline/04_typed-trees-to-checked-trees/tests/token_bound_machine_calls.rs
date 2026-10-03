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
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )?;
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
}

const WRAPPED_ADD: &str = "data Wrapped { value: u8; }
    machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
        (left.value as u64) + (right.value as u64)
    }
    machine by_token(left: Wrapped, right: Wrapped) -> u64 { left + right }";

#[test]
fn free_boundary_token_requirements_retain_scalar_signatures_without_attachment() {
    for attachment in ["", "data Float {}"] {
        let checked = check(&format!(
            "{attachment}
             boundary machine * Float::multiply(left: f64, right: f64) -> f64;
             machine convert(value: f64) -> f64 {{ value * 2.0 }}",
        ))
        .expect("a free requirement is legal source");
        let requirement = checked
            .machines()
            .iter()
            .find(|machine| machine.spelling.is_some())
            .unwrap();
        let plan = checked
            .facts
            .flow
            .terminal_unit_effects
            .boundary_machines
            .iter()
            .find(|plan| plan.machine == requirement.symbol)
            .expect("a free boundary has a requirement plan without manufacturing attached data");
        assert_eq!(
            plan.attachment_type_identity.is_none(),
            attachment.is_empty()
        );
        assert!(plan.structural_parameters.is_empty());
        assert_eq!(plan.scalar_parameters.len(), 2);
        assert!(plan.scalar_parameters.iter().all(|parameter|
            parameter.primitive_type == typed_trees::types::PrimitiveType::F64
        ));
        assert_eq!(
            plan.result.scalar(),
            Some(typed_trees::types::PrimitiveType::F64)
        );
    }
}

#[test]
fn canonical_float_comparisons_retain_exact_requirement_and_operand_identity() {
    for format in ["f32", "f64"] {
        for (token, name) in [
            ("==", "equal"),
            ("!=", "not_equal"),
            ("<", "less"),
            ("<=", "less_or_equal"),
            (">", "greater"),
            (">=", "greater_or_equal"),
        ] {
            let checked = check(&format!(
                "boundary machine {token} Float::{name}(left: {format}, right: {format}) -> bool;
                 machine compare(left: {format}, right: {format}) -> bool {{ left {token} right }}"
            ))
            .expect("selected bodyless comparison retains its computation");
            let requirement = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == format!("Float::{name}"))
                .expect("declared requirement");
            let selected = checked
                .facts
                .operators
                .uses
                .iter()
                .filter(|(_, selected)| selected.selected_operator_symbol == requirement.symbol)
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), 1, "{format} {token}");
            let (handle, selected) = selected[0];
            assert!(
                checked
                    .facts
                    .operators
                    .selected_float_comparison(&checked.typed, handle)
                    .is_some()
            );
            let ExpressionNode::Binary(binary) =
                checked.expression_table.expression(selected.expression)
            else {
                panic!("a selected comparison must not become a bodyless call");
            };
            assert_eq!(
                selected.operands(&checked.typed).unwrap(),
                [binary.left, binary.right]
            );
            assert!(
                checked
                    .facts
                    .operators
                    .boundary_applications
                    .iter()
                    .any(
                        |application| application.site == selected.application_site()
                            && application.requirement_symbol == requirement.symbol
                    )
            );
        }
    }
}

#[test]
fn noncanonical_boundary_float_tokens_remain_calls() {
    for declaration in [
        "boundary machine > Other::greater(left: f64, right: f64) -> bool;",
        "boundary machine > Float::different(left: f64, right: f64) -> bool;",
    ] {
        let checked = check(&format!(
            "{declaration}
             machine compare(left: f64, right: f64) -> bool {{ left > right }}"
        ))
        .expect("ordinary token supply");
        let requirement = checked
            .machines()
            .iter()
            .find(|machine| machine.spelling.is_some())
            .expect("token declaration");
        let entry = checked.machine_states(requirement)[0].symbol;
        assert!(checked.expression_table.iter_expressions().any(|(_, expression)|
            matches!(expression, ExpressionNode::Call(call) if call.target_symbol == entry)
        ), "{declaration}");
        assert!(checked.facts.operators.uses.iter().all(|(handle, _)| {
            checked
                .facts
                .operators
                .selected_float_comparison(&checked.typed, handle)
                .is_none()
        }));
    }
}

#[test]
fn retained_float_comparisons_still_owe_requirements_and_unique_selection() {
    for (declarations, expected) in [
        (
            "boundary machine > Float::greater(left: f64, right: f64) -> bool requires false;",
            "requires",
        ),
        (
            "boundary machine > Float::greater(left: f64, right: f64) -> bool;
          boundary machine > Other::greater(left: f64, right: f64) -> bool;",
            "ambiguous",
        ),
    ] {
        let diagnostics = check(&format!(
            "{declarations}
             machine compare(left: f64, right: f64) -> bool {{ left > right }}"
        ))
        .expect_err("retaining a comparison cannot bypass admission");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{diagnostics:?}"
        );
    }
}

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

/// The position whose body supply is still unimplemented keeps rejecting
/// rather than falling back to builtin indexing: an open range use omits its
/// end, and the omitted endpoint would be the collection's length, which a
/// declared `[..]` telescope cannot form.
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
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("the range use omits its end")),
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
