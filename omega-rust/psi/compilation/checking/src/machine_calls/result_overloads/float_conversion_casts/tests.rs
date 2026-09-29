//! Float-to-integer policy casts bind exactly the conversion overload their
//! result qualification selects, or stay casts.

use super::bind_float_conversion_casts;
use crate::machine_calls::result_overloads::resolve_named_result_overloads;
use language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionKind, AuthoredDeclarationSelectionTarget,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;

const CONVERSIONS: &str = "
    pub data I32 {}
    pub boundary operator I32::from_f64(value: f64) -> i32
    requires value == value && value > -2147483649.0 && value < 2147483648.0;
    pub boundary operator I32::from_f64(value: f64) -> i32 in Trapping;
    pub boundary operator I32::from_f64(value: f64) -> i32 in Saturating;
    pub boundary operator I32::round_from_f64(value: f64) -> i32 in Saturating;
";

fn bound(source: &str) -> (TypedTrees, usize) {
    let mut program = crate::front_end::typed_program(source);
    resolve_named_result_overloads(&mut program).expect("result overloads resolve");
    let count = bind_float_conversion_casts(&mut program).expect("casts bind");
    (program, count)
}

/// The initializer of `machine value`'s local `name`.
fn initializer(program: &TypedTrees, name: &str) -> ExpressionHandle {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "value")
        .expect("machine value");
    program
        .statement_table
        .statements(program.machine_states(machine)[0].statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == name => {
                Some(local.initial_value)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("local {name}"))
}

/// The conversion overload whose result carries `policy` (empty for the
/// unqualified overload), on the exact `I32::from_f64` path.
fn overload(program: &TypedTrees, member: &str, policy: &str) -> SymbolHandle {
    program
        .operators()
        .iter()
        .find(|operator| {
            program
                .operator_path_members(operator.name)
                .last()
                .is_some_and(|last| last.as_str() == member)
                && program
                    .display_type_reference_with_constraints(operator.return_type)
                    .contains(policy)
                && (!policy.is_empty()
                    || !program
                        .display_type_reference_with_constraints(operator.return_type)
                        .contains(" in "))
        })
        .map(|operator| operator.symbol)
        .unwrap_or_else(|| panic!("{member} {policy} overload"))
}

/// Strip `( .. ) as i32` erasures down to the node the policy cast became.
fn conversion(program: &TypedTrees, mut expression: ExpressionHandle) -> ExpressionHandle {
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Atomic(atomic) => expression = atomic.value,
            ExpressionNode::Cast(cast)
                if cast.domain == numerics::arithmetic::ArithmeticDomain::Exact =>
            {
                expression = cast.value;
            }
            _ => return expression,
        }
    }
}

fn call_target(program: &TypedTrees, expression: ExpressionHandle) -> SymbolHandle {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        panic!(
            "the policy cast became a conversion call: {:?}",
            program.expression_table.expression(expression)
        );
    };
    call.target_symbol
}

#[test]
fn policy_casts_bind_the_overload_their_result_qualification_selects() {
    let (program, count) = bound(&format!(
        "{CONVERSIONS}
        machine value(input: f64) -> i32 {{
            let clamped: i32 = (input as i32 in Saturating) as i32;
            let trapped: i32 = (input as i32 in Trapping) as i32;
            let qualified: i32 in Saturating = input as i32 in Saturating;
            clamped
        }}"
    ));
    assert_eq!(count, 3);
    let saturating = overload(&program, "from_f64", "Saturating");
    let trapping = overload(&program, "from_f64", "Trapping");
    for (local, selected) in [
        ("clamped", saturating),
        ("trapped", trapping),
        ("qualified", saturating),
    ] {
        let call = conversion(&program, initializer(&program, local));
        assert_eq!(call_target(&program, call), selected, "{local}");
        // The cast's own span carries exactly one resolved Call selection
        // of the same overload: the custody an explicit spelling would keep.
        let selections = program
            .expression_table
            .authored_selection_occurrences(call)
            .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
            .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Call)
            .collect::<Vec<_>>();
        let [selection] = selections.as_slice() else {
            panic!("{local}: one Call selection, found {}", selections.len());
        };
        assert!(
            matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
                if target.selected_symbol() == selected),
            "{local}"
        );
        assert_eq!(
            selection.source_span(),
            program.expression_table.source_span(call),
            "{local}"
        );
    }
    // Binding is idempotent: a bound call is no longer a cast.
    let mut again = program.clone();
    assert_eq!(bind_float_conversion_casts(&mut again).unwrap(), 0);
}

#[test]
fn exact_casts_and_unselectable_policies_stay_casts() {
    // Exact float-to-integer casts keep their own finite-and-range proof.
    // A policy without its own overload is never served by another one:
    // neither the unqualified conversion nor a same-signature Saturating
    // conversion on another path substitutes for `I32::from_f64 in Saturating`.
    let (program, count) = bound(
        "pub data I32 {}
        pub boundary operator I32::from_f64(value: f64) -> i32
        requires value == value && value > -2147483649.0 && value < 2147483648.0;
        pub boundary operator I32::from_f64(value: f64) -> i32 in Trapping;
        pub boundary operator I32::round_from_f64(value: f64) -> i32 in Saturating;
        machine value(input: f64 [0.0..=100.0]) -> i32 {
            let exact: i32 = input as i32;
            let clamped: i32 = (input as i32 in Saturating) as i32;
            exact
        }",
    );
    assert_eq!(count, 0);
    for local in ["exact", "clamped"] {
        let root = initializer(&program, local);
        let node = conversion(&program, root);
        assert!(
            !matches!(
                program.expression_table.expression(node),
                ExpressionNode::Call(_)
            ),
            "{local} stays a cast"
        );
    }
}

#[test]
fn exact_float_widenings_select_the_exact_source_and_destination() {
    for (destination, owner, other_float) in [("f32", "F32", "f64"), ("f64", "F64", "f32")] {
        for source in [
            "i8",
            "i16",
            "i32",
            "i64",
            "u8",
            "u16",
            "u32",
            "u64",
            other_float,
        ] {
            let (mut program, count) = bound(&format!(
                "pub data {owner} {{}}
                pub boundary operator {owner}::from_{source}(input: {source}) -> {destination};
                machine value(input: {source}) -> {destination} {{
                    let converted: {destination} = input as {destination};
                    converted
                }}"
            ));
            let exact = matches!(
                (source, destination),
                ("i8" | "i16" | "u8" | "u16", "f32" | "f64") | ("i32" | "u32" | "f32", "f64")
            );
            assert_eq!(count, usize::from(exact), "{source} -> {destination}");
            let expression = initializer(&program, "converted");
            if !exact {
                assert!(matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Cast(_)
                ));
                continue;
            }
            assert_eq!(
                call_target(&program, expression),
                overload(&program, &format!("from_{source}"), "")
            );
            assert_eq!(bind_float_conversion_casts(&mut program).unwrap(), 0);
        }
    }
}

#[test]
fn widening_does_not_erase_a_destination_range_obligation() {
    let (program, count) = bound(
        "pub data F64 {}
         pub boundary operator F64::from_i32(input: i32) -> f64;
         machine value() -> f64 {
             let converted: f64 = 42i32 as f64 [0.0..=10.0];
             converted
         }",
    );
    assert_eq!(count, 0);
    assert!(matches!(
        program
            .expression_table
            .expression(initializer(&program, "converted")),
        ExpressionNode::Cast(_)
    ));
}

#[test]
fn loaded_but_unimported_conversion_is_not_a_candidate() {
    let declarations = "pub data F64 {} pub boundary operator F64::from_i32(input: i32) -> f64;";
    let caller =
        "machine value(input: i32) -> f64 { let converted: f64 = input as f64; converted }";
    let mut sources = source::SourceMap::default();
    let owner = sources
        .add_with_metadata(
            "owner/main.omg".into(),
            declarations.to_owned(),
            "owner".into(),
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap()),
            source::SourceOrigin::User,
        )
        .source_id;
    let requester = sources
        .add_with_metadata(
            "caller/main.omg".into(),
            caller.to_owned(),
            "caller".into(),
            Some(semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]).unwrap()),
            source::SourceOrigin::User,
        )
        .source_id;
    let mut program = crate::front_end::typed_program_from_source_map(
        sources,
        &[(owner, declarations), (requester, caller)],
    );
    assert_eq!(bind_float_conversion_casts(&mut program).unwrap(), 0);
    assert!(matches!(
        program
            .expression_table
            .expression(initializer(&program, "converted")),
        ExpressionNode::Cast(_)
    ));
}

#[test]
fn float_cast_does_not_select_a_missing_family_or_same_format_operator() {
    for source in [
        "machine value(input: i32) -> f64 { let converted: f64 = input as f64; converted }",
        "pub data F64 {}
         pub boundary operator F64::from_f64(input: f64) -> f64;
         machine value(input: f64) -> f64 { let converted: f64 = input as f64; converted }",
    ] {
        let (program, count) = bound(source);
        assert_eq!(count, 0);
        assert!(matches!(
            program
                .expression_table
                .expression(initializer(&program, "converted")),
            ExpressionNode::Cast(_)
        ));
    }
}
