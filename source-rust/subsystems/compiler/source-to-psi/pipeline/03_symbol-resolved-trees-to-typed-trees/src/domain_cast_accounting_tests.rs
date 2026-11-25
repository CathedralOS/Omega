//! Casts retain their exact selected domain and read declaration-owned values.

use super::*;
use crate::source_state::statement::{StatementNode, TransitionTargetNode};
use crate::typing_queries::{declared_callable_value_type, declared_place_type};
use std::sync::Arc;

fn source_result(
    text: &str,
) -> (
    crate::source_resolved::SymbolResolvedTrees,
    Result<TypedTrees, diagnostics::Diagnostic>,
) {
    let mut sources = crate::source_inputs::SourceMap::default();
    let id = sources
        .add("cast_contexts/main.omg".into(), text.to_owned())
        .source_id;
    let tokens = crate::frontend_fixture::Lexer::new(text)
        .tokenize()
        .unwrap();
    let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let mut request = crate::resolution::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = crate::resolution::resolve(request).unwrap();
    let typed = crate::lower_symbol_resolved_trees(&resolved);
    (resolved, typed)
}

fn source(text: &str) -> (crate::source_resolved::SymbolResolvedTrees, TypedTrees) {
    let (resolved, typed) = source_result(text);
    (resolved, typed.unwrap())
}

fn machine(typed: &TypedTrees, name: &str) -> SymbolHandle {
    typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .symbol
}

#[test]
fn a_domain_cast_reads_a_named_requirements_declared_result() {
    let (_, typed) = source(
        r#"
boundary trait Offset {
    machine value() -> u64 reaches none crashes none;
}
machine read() -> u64 in Wrapping {
    Offset::value() as u64 +Wrapping
}
"#,
    );
    assert!(
        typed.deferred_refusals.is_empty(),
        "{:?}",
        typed.deferred_refusals
    );
    assert!(
        typed
            .expression_table
            .expression_entries()
            .any(|(_, node)| {
                matches!(node, ExpressionNode::Cast(cast) if cast.form.entry().is_some()
            && cast.result_type.is_valid())
            })
    );
}

#[test]
fn a_carrier_cast_cannot_drop_a_named_requirements_result_domain() {
    let (_, result) = source_result(
        r#"
domain u64::Tagged;
boundary trait Offset {
    machine value() -> u64 in Tagged reaches none crashes none;
}
machine read() -> u64 in Wrapping {
    Offset::value() as u64 +Wrapping
}
"#,
    );
    let error = result.expect_err("the carrier cast must account for Tagged");
    assert!(
        error.message.contains("carrier cast") && error.message.contains("Tagged"),
        "{}",
        error.message
    );
}

#[test]
fn a_borrowed_collection_cast_keeps_its_declared_field_type() {
    let (_, typed) = source(
        r#"
domain [u8; 2]::Marked;
data Packet { bytes: [u8; 2] in Marked; }
machine update(packet: @mutable Packet) {
    (@mutable packet.bytes as -Marked)[0] = 1;
}
"#,
    );
    let owner = machine(&typed, "update");
    let (handle, borrow) = typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Borrow(borrow) => Some((handle, borrow)),
            _ => None,
        })
        .unwrap();
    let reference = typed
        .expression_table
        .constructed_value_type_reference(handle);
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = typed.type_reference_table.type_reference(reference)
    else {
        panic!("the field borrow retains its reference type");
    };
    assert_eq!(*access, language_core::ReferenceAccess::Mutable);
    let declared = declared_callable_value_type(&typed, owner, borrow.target).unwrap();
    assert_eq!(
        typed.normalized_type_identity(*referee),
        typed.normalized_type_identity(declared)
    );
    assert!(!matches!(
        typed.type_reference_table.type_reference(declared),
        TypeReferenceNode::Unit
    ));
    let cast = typed
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| match node {
            ExpressionNode::Cast(cast) if cast.form.entry().is_some() => Some(cast),
            _ => None,
        })
        .unwrap();
    let TypeReferenceNode::Reference {
        referee: plain,
        access: after_access,
        ..
    } = typed.type_reference_table.type_reference(cast.result_type)
    else {
        panic!("domain removal keeps a reference result");
    };
    assert_eq!(*after_access, *access);
    let domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("Marked"))
        .unwrap();
    assert_eq!(cast.semantic_domain_symbol, domain.symbol);
    assert_ne!(
        cast.semantic_domain_id,
        semantic_vocabulary::SemanticDomainId::NULL
    );
    assert_eq!(
        typed.normalized_type_identity(flatten(&typed, declared).0),
        typed.normalized_type_identity(*plain)
    );
    assert!(
        !typed
            .deferred_refusals_of(owner)
            .any(|row| row.request == DeferredRequest::DomainCastThroughReference)
    );
}

#[test]
fn a_declared_receiving_type_still_constrains_a_cast_initializer() {
    let (_, result) = source_result(
        r#"
domain [u8; 2]::Marked;
data Packet { bytes: [u8; 2] in Marked; }
machine update(packet: @mutable Packet) {
    let wrong: bool = @mutable packet.bytes as -Marked;
}
"#,
    );
    let error = match result {
        Ok(_) => panic!("a declared Boolean cannot become an inferred byte reference"),
        Err(error) => error,
    };
    assert!(
        error.message.contains("Marked") && error.message.contains("bool"),
        "{error:?}"
    );
}

#[test]
fn policy_entries_keep_the_written_domain_and_independent_result_type() {
    let mut text = String::new();
    let mut expected = Vec::new();
    for policy in [
        ArithmeticDomain::Wrapping,
        ArithmeticDomain::Saturating,
        ArithmeticDomain::Trapping,
    ] {
        for (verb, kind) in [
            ("add", CastEntryKind::Add),
            ("keep", CastEntryKind::Keep),
            ("remove", CastEntryKind::Remove),
        ] {
            let name = format!("{verb}_{}", policy.name());
            let qualified = format!("u32 in {}", policy.name());
            let input = if kind == CastEntryKind::Add {
                "u32"
            } else {
                &qualified
            };
            let result = if kind == CastEntryKind::Remove {
                "u32"
            } else {
                &qualified
            };
            text.push_str(&format!(
                "machine {name}(value: {input}) -> {result} {{ value as {}{} }}\n",
                kind.sign(),
                policy.name()
            ));
            expected.push((name, policy, kind));
        }
    }
    let (_, typed) = source(&text);
    assert!(
        typed.deferred_refusals.is_empty(),
        "{:?}",
        typed.deferred_refusals
    );
    for (name, policy, kind) in expected {
        let owner = typed.machine_by_symbol(machine(&typed, &name)).unwrap();
        let state = typed.machine_states(owner).first().unwrap();
        let StatementNode::Transition(transition) = typed
            .statement_table
            .statement(state.statement_nodes.start())
        else {
            panic!("one authored return transition");
        };
        let TransitionTargetNode::Value(expression) =
            typed.statement_table.transition_target(transition.target)
        else {
            panic!("the transition returns the authored cast");
        };
        let ExpressionNode::Cast(cast) = typed.expression_table.expression(*expression) else {
            panic!("the authored entry survives accounting");
        };
        assert_eq!(cast.form.entry().unwrap().kind, kind);
        assert_eq!(cast.domain, policy, "{name} lost its source policy");
        assert!(cast.result_type.is_valid());
        assert_eq!(
            typed.normalized_type_identity(cast.result_type),
            typed.normalized_type_identity(state.return_type),
            "{name} changed the resulting qualification"
        );
    }
}

#[test]
fn typed_scalar_literals_keep_their_carriers_during_domain_accounting() {
    // A suffix fixes the operand before the cast chooses a domain. The two
    // same-named domains must follow that carrier, not a receiving context.
    let (_, typed) = source(
        r#"
domain u64::Marked;
domain u8::Marked;
domain bool::Ready;
machine wide() -> u64 in Marked { 42u64 as +Marked }
machine narrow() -> u8 in Marked { 42u8 as +Marked }
machine ready() -> bool in Ready { true as +Ready }
"#,
    );
    assert!(
        typed.deferred_refusals.is_empty(),
        "{:?}",
        typed.deferred_refusals
    );
    let owner = machine(&typed, "wide");
    let mut casts = 0;
    for (_, expression) in typed.expression_table.expression_entries() {
        let ExpressionNode::Cast(cast) = expression else {
            continue;
        };
        let operand = declared_callable_value_type(&typed, owner, cast.value).unwrap();
        let domain = typed
            .domain_definition(cast.semantic_domain_symbol)
            .unwrap();
        assert_eq!(
            typed.normalized_type_identity(operand),
            typed.normalized_type_identity(domain.target_type)
        );
        assert_eq!(
            typed.primitive_type_reference(operand),
            typed.primitive_type_reference(cast.result_type)
        );
        casts += 1;
    }
    assert_eq!(casts, 3);
}

#[test]
fn machine_and_requirement_contracts_retain_each_domain_entry() {
    let (_, typed) = source(
        r#"
domain i64::Marked;
machine relay(value: i64 in Marked) -> i64 in Marked
requires (value as -Marked) != 0
ensures (result as -Marked) == (value as -Marked)
{ value as =Marked }
trait Service {
    machine relay(value: i64 in Marked) -> i64 in Marked
    requires (value as -Marked) != 0
    ensures (result as -Marked) == (value as -Marked);
}
boundary trait Raw {
    machine relay(value: i64 in Marked) -> i64 in Marked
    requires (value as -Marked) != 0
    ensures (result as -Marked) == (value as -Marked)
    reaches none crashes none;
}
"#,
    );
    let domain = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str().ends_with("Marked"))
        .unwrap();
    let casts = typed
        .expression_table
        .expression_entries()
        .filter_map(|(handle, expression)| match expression {
            ExpressionNode::Cast(cast) if cast.form.entry().is_some() => Some((handle, cast)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        casts.len() >= 10,
        "every source contract and the kept body entry are present"
    );
    for (handle, cast) in casts {
        assert!(cast.result_type.is_valid(), "{cast:?}");
        assert_eq!(cast.semantic_domain_symbol, domain.symbol);
        assert_eq!(cast.semantic_domain_id, domain.semantic_id);
        let selected = typed
            .expression_table
            .authored_selection_occurrences(handle)
            .filter_map(|occurrence| typed.authored_declaration_selections().get(occurrence))
            .filter_map(|selection| {
                selection
                    .target()
                    .as_resolved()
                    .map(|target| target.selected_symbol())
            })
            .collect::<Vec<_>>();
        assert_eq!(selected, [domain.symbol]);
    }
    assert!(
        typed.deferred_refusals.is_empty(),
        "{:?}",
        typed.deferred_refusals
    );
}

#[test]
fn kept_and_removed_entries_retain_their_selected_domain_arguments() {
    let (_, typed) = source(
        r#"
domain<Index> i64::Marked<Index>;
machine keep(value: i64 in Marked<u32>) -> i64 in Marked<u32> { value as =Marked<u32> }
machine remove(value: i64 in Marked<u32>) -> i64 { value as -Marked<u32> }
"#,
    );
    let mut count = 0;
    for (_, expression) in typed.expression_table.expression_entries() {
        let ExpressionNode::Cast(cast) = expression else {
            continue;
        };
        assert!(cast.result_type.is_valid());
        assert!(cast.semantic_domain_symbol.is_valid());
        assert_eq!(
            typed
                .expression_table
                .name_path_string(cast.semantic_domain),
            "Marked"
        );
        let arguments = typed
            .type_reference_table
            .type_reference_handles(cast.semantic_domain_arguments);
        assert_eq!(arguments.len(), 1);
        assert_eq!(
            typed.primitive_type_reference(arguments[0]),
            Some(crate::source_state::types::PrimitiveType::Integer(
                numerics::integer_carrier::FixedInteger::unsigned(32)
            ))
        );
        count += 1;
    }
    assert_eq!(count, 2);
}

#[test]
fn reserved_results_are_exact_callable_values_and_never_storage() {
    let (_, mut typed) = source(
        r#"
domain i64::Marked;
machine first(value: i64 in Marked) -> i64 in Marked
ensures (result as -Marked) == (value as -Marked)
{ value }
machine second(value: i64 in Marked) -> i64 in Marked { value }
machine shadow(result: i64 in Marked) -> i64 { result as -Marked }
"#,
    );
    let first = machine(&typed, "first");
    let first_state = typed.machine_states(typed.machine_by_symbol(first).unwrap())[0].clone();
    let second = machine(&typed, "second");
    let second_state = typed.machine_states(typed.machine_by_symbol(second).unwrap())[0].symbol;
    let (expression, path) = typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Name(path) if path.reserved_result_of == first_state.symbol => {
                Some((handle, *path))
            }
            _ => None,
        })
        .unwrap();
    for owner in [first, first_state.symbol] {
        assert_eq!(
            declared_callable_value_type(&typed, owner, expression).unwrap(),
            first_state.return_type
        );
    }
    assert!(declared_place_type(&typed, &first_state, expression).is_err());
    assert!(declared_callable_value_type(&typed, second, expression).is_err());
    assert!(declared_callable_value_type(&typed, SymbolHandle::invalid(), expression).is_err());
    let foreign = typed.expression_table.insert(ExpressionNode::Name(
        crate::source_state::expression::TableNamePath {
            reserved_result_of: second_state,
            ..path
        },
    ));
    assert!(declared_callable_value_type(&typed, first, foreign).is_err());

    let shadow = machine(&typed, "shadow");
    let state = &typed.machine_states(typed.machine_by_symbol(shadow).unwrap())[0];
    let formal = &typed.state_parameters(state)[0];
    let ordinary = typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Name(path) if path.resolved_symbol() == Some(formal.symbol) => {
                assert!(!path.reserved_result_of.is_valid());
                Some(handle)
            }
            _ => None,
        })
        .unwrap();
    assert_ne!(formal.type_reference, state.return_type);
    assert_eq!(
        declared_callable_value_type(&typed, shadow, ordinary).unwrap(),
        formal.type_reference
    );
    assert_eq!(
        declared_place_type(&typed, state, ordinary).unwrap(),
        formal.type_reference
    );
}

#[test]
fn a_shared_cast_cannot_choose_between_two_callable_owners() {
    let (resolved, mut typed) = source(
        r#"
domain i64::Marked;
machine first(value: i64 in Marked) -> i64 { value as -Marked }
machine second(value: i64 in Marked) -> i64 { value as -Marked }
"#,
    );
    let first = machine(&typed, "first");
    let second = machine(&typed, "second");
    let first_state = typed.machine_states(typed.machine_by_symbol(first).unwrap())[0].clone();
    let second_state = typed.machine_by_symbol(second).unwrap().states.start();
    typed.machine_states.get_mut(second_state).statement_nodes = first_state.statement_nodes;
    let refused =
        crate::expression_meanings::prepare(&resolved, &mut typed, 0, 0, &[]).unwrap_err();
    assert!(
        refused
            .message
            .contains("more than one exact lexical owner"),
        "{refused:?}"
    );
}

#[test]
fn a_constant_and_its_retained_initializer_share_one_source_cast_owner() {
    let (_, typed) = source(
        r#"
domain i64::Marked;
machine plain(value: i64 in Marked) -> i64 { value as -Marked }
const VALUE: i64 = plain(7 as +Marked);
"#,
    );
    let constant = typed
        .const_declarations()
        .iter()
        .find(|row| typed.symbols.name(row.symbol) == "VALUE")
        .unwrap();
    assert!(constant.evaluated_value.is_none());
    assert!(typed.tables.machines.is_valid(constant.evaluation_entry));
    let entry = typed.tables.machines.get(constant.evaluation_entry).symbol;
    let owners = crate::source_state::references::collect_program_operand_owners(&typed);
    let shared = typed
        .expression_table
        .expression_entries()
        .filter_map(|(handle, node)| matches!(node, ExpressionNode::Cast(_)).then_some(handle))
        .any(|handle| {
            let owners = owners.expression_owners(handle).collect::<Vec<_>>();
            owners.contains(&constant.symbol) && owners.contains(&entry)
        });
    assert!(
        shared,
        "the actual retained initializer carries the shared source expression"
    );
    assert!(
        typed.deferred_refusals.is_empty(),
        "{:?}",
        typed.deferred_refusals
    );
}

#[test]
fn old_projects_the_exact_callable_input_type_without_becoming_storage() {
    let (_, mut typed) = source(
        r#"
domain i64::Marked;
machine first(value: i64 in Marked) -> i64
ensures result == (old(value) as -Marked)
{ value as -Marked }
machine second(value: i64 in Marked) -> i64 { value as -Marked }
"#,
    );
    let first = machine(&typed, "first");
    let second = machine(&typed, "second");
    let state = typed.machine_states(typed.machine_by_symbol(first).unwrap())[0].clone();
    let input = typed.state_parameters(&state)[0].type_reference;
    let (old, call) = typed
        .expression_table
        .expression_entries()
        .find_map(|(handle, node)| {
            let ExpressionNode::Call(call) = node else {
                return None;
            };
            (typed
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(symbols::BuiltinFunction::ContentOld))
            .then(|| (handle, call.clone()))
        })
        .unwrap();
    assert_eq!(
        declared_callable_value_type(&typed, first, old).unwrap(),
        input
    );
    assert_eq!(
        declared_callable_value_type(&typed, state.symbol, old).unwrap(),
        input
    );
    assert!(declared_callable_value_type(&typed, second, old).is_err());
    assert!(declared_callable_value_type(&typed, SymbolHandle::invalid(), old).is_err());
    assert!(declared_place_type(&typed, &state, old).is_err());

    let mut forged = call.clone();
    forged.target_symbol = second;
    let lookalike = typed.expression_table.insert(ExpressionNode::Call(forged));
    assert!(declared_callable_value_type(&typed, first, lookalike).is_err());

    let argument = typed.expression_table.expression_handles(call.arguments)[0];
    let mut receiver = call.clone();
    receiver.receiver = argument;
    let receiver = typed
        .expression_table
        .insert(ExpressionNode::Call(receiver));
    assert!(declared_callable_value_type(&typed, first, receiver).is_err());

    for arguments in [Vec::new(), vec![argument, argument]] {
        let mut malformed = call.clone();
        malformed.arguments = typed.expression_table.insert_expression_handles(arguments);
        let malformed = typed
            .expression_table
            .insert(ExpressionNode::Call(malformed));
        assert!(declared_callable_value_type(&typed, first, malformed).is_err());
    }
}

#[test]
fn a_captured_cast_index_retains_its_declared_result_type() {
    let (_, typed) = source(
        r#"
domain [u8; 8]::Terminated requires self[7] == 0;
data Packet { bytes: [u8; 8] in Terminated; index: i32 in Wrapping; }
machine update(packet: @mutable Packet) {
    (@mutable packet.bytes as -Terminated)[packet.index as -Wrapping] = 0;
}
"#,
    );
    let update = typed.machine_by_symbol(machine(&typed, "update")).unwrap();
    let mut captures = 0;
    for state in typed.machine_states(update) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            if let StatementNode::LocalData(local) = statement
                && local.is_operand_capture
            {
                captures += 1;
                assert!(
                    typed
                        .type_reference_table
                        .contains_type_reference(local.type_reference),
                    "a captured index must retain its actual cast result: {local:?}"
                );
                let ExpressionNode::Cast(cast) =
                    typed.expression_table.expression(local.initial_value)
                else {
                    panic!("the variable selector retains its domain-removal cast");
                };
                assert_eq!(
                    typed.normalized_type_identity(local.type_reference),
                    typed.normalized_type_identity(cast.result_type)
                );
            }
        }
    }
    assert_eq!(captures, 1);
}

#[test]
fn a_static_value_types_the_captured_selector_only_in_its_own_scope() {
    let (_, typed) = source(
        "machine take<const Count: u64>(bytes: @[u8; Count]) -> u8
             requires Count > 0u64 { bytes[Count - 1u64] }
         machine other<Count: u32>() -> u32 { Count }",
    );
    let take = typed.machine_by_symbol(machine(&typed, "take")).unwrap();
    let parameters = typed.machine_type_parameters(take);
    assert_eq!(parameters.len(), 1);
    let parameter = &parameters[0];
    let crate::source_state::data::TypeParameterKind::Const { type_reference } = parameter.kind
    else {
        panic!("Count must keep its declared integer carrier");
    };
    let (use_of_count, _) = typed
        .expression_table
        .iter_backing_expressions()
        .find(|(_, node)| {
            matches!(node, ExpressionNode::Name(path)
            if path.resolved_symbol() == Some(parameter.symbol))
        })
        .expect("the body uses its own Count");
    let state = &typed.machine_states(take)[0];
    for owner in [take.symbol, state.symbol] {
        assert_eq!(
            declared_callable_value_type(&typed, owner, use_of_count).unwrap(),
            type_reference
        );
    }
    assert!(declared_callable_value_type(&typed, machine(&typed, "other"), use_of_count).is_err());
    let other = machine(&typed, "other");
    let other_binders = typed.declaration_type_parameters(other);
    let runtime_count = &other_binders[0];
    let crate::source_state::data::TypeParameterKind::Value {
        type_reference: runtime_type,
    } = runtime_count.kind
    else {
        panic!("the second Count is a runtime-capable value binder");
    };
    let (value, _) = typed
        .expression_table
        .iter_backing_expressions()
        .find(|(_, node)| {
            matches!(node, ExpressionNode::Name(path)
            if path.resolved_symbol() == Some(runtime_count.symbol))
        })
        .expect("other uses its own Count");
    assert_eq!(
        declared_callable_value_type(&typed, other, value).unwrap(),
        runtime_type
    );
    let captures: Vec<_> = typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| statement.as_local_data())
        .filter(|local| local.is_operand_capture)
        .collect();
    let [capture] = captures.as_slice() else {
        panic!("the selector must be evaluated once");
    };
    assert_eq!(
        typed.normalized_type_identity(capture.type_reference),
        typed.normalized_type_identity(type_reference),
    );
}

#[test]
fn a_selected_index_constant_keeps_its_carrier_before_evaluation() {
    let domain = "domain<const N: u8> u32::At<N>;";
    for (holder, applied) in [
        ("", "u32 in At<KEY>"),
        ("data Token<const N: u8> { value: u32; }", "Token<KEY>"),
        ("data Record { field: u32 in At<KEY>; }", "Record"),
        ("data Choice { case Value(field: u32 in At<KEY>); }", "Choice"),
    ] {
        for (carrier, value, accepted) in [("u8", "4u8", true), ("u64", "4u64", false)] {
            let source = format!(
                "{domain} {holder} machine identity(value: {carrier}) -> {carrier} {{ value }} \
                 const KEY: {carrier} = identity({value}); \
                 machine entry(value: {applied}) -> u32 {{ 0u32 }}"
            );
            let result = source_result(&source).1;
            if accepted {
                result.unwrap();
            } else {
                let Err(refusal) = result else {
                    panic!("{applied} accepted a {carrier} constant in a u8 index");
                };
                assert!(refusal.message.contains("declared carrier"), "{refusal:?}");
                let span = refusal.source_span.expect("point to the named constant").span;
                assert!(source[span.start..span.end].contains("KEY"), "{refusal:?}");
            }
        }
    }
}

#[test]
fn domain_index_literals_are_checked_against_the_selected_family() {
    use crate::source_state::types::{TypeConstraintNode, TypeReferenceNode};
    for family in [
        "domain<const N: u8> u32::At<N>;",
        "domain<T, const N: u8> T::At<N>;",
    ] {
        let check = |argument: &str| {
            source_result(&format!(
                "{family} machine entry(value: u32 in At<{argument}>) -> u32 {{ 0u32 }}"
            ))
            .1
        };
        for argument in ["4", "4u8"] {
            let typed = check(argument).unwrap();
            let mut normalized = false;
            for (_, _, constraints) in typed
                .type_reference_table
                .constrained_type_reference_sites()
            {
                for constraint in typed.type_reference_table.constraints(constraints) {
                    if let TypeConstraintNode::Domain(domain) = constraint {
                        let [index] = domain.arguments.as_slice() else {
                            continue;
                        };
                        let index = typed.type_reference_table.type_reference(*index);
                        assert!(
                            matches!(index, TypeReferenceNode::Named { name, .. } if name.as_str() == "4"),
                            "{argument}: {index:?}"
                        );
                        normalized = true;
                    }
                }
            }
            assert!(normalized);
        }
        for argument in ["256", "4u64", "true"] {
            let refusal = check(argument).expect_err("the index must fit the declared carrier");
            assert!(
                refusal.message.contains("must be a `u8`"),
                "{family} with {argument}: {refusal:?}"
            );
        }
    }
}

#[test]
fn casts_retain_the_domain_declared_on_a_qualified_alias() {
    let (_, typed) = source(
        "domain u16::Stored requires self > 0u16;
         data Kept = u16 in Stored;
         domain Kept::Side;
         machine add(x: Kept) -> Kept in Side { x as =Stored +Kept::Side }
         machine keep(x: Kept in Side) -> Kept in Side { x as =Side =Stored }
         machine remove(x: Kept in Side) -> u16 { keep(x) as -Stored -Side }",
    );
    let side = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "Kept::Side")
        .unwrap();
    let mut entries = 0;
    for (_, node) in typed.expression_table.expression_entries() {
        if let ExpressionNode::Cast(cast) = node
            && cast.form.entry().is_some()
            && typed.expression_table.name_path_string(cast.semantic_domain).ends_with("Side")
        {
            assert!(cast.result_type.is_valid());
            assert_eq!(cast.semantic_domain_symbol, side.symbol);
            assert_eq!(cast.semantic_domain_id, side.semantic_id);
            entries += 1;
        }
    }
    assert_eq!(entries, 3);
}

#[test]
fn a_qualified_domain_cast_still_requires_its_exact_data_carrier() {
    let (_, result) = source_result(
        "domain u16::Stored;
         data Kept = u16 in Stored;
         domain Kept::Side;
         machine wrong(x: u32) -> u32 { x as +Kept::Side }",
    );
    let error = result.expect_err("Side is a domain of u16");
    assert!(
        error.message.contains("names no visible domain") && error.message.contains("u32"),
        "{}",
        error.message
    );
}

#[test]
fn a_refinement_subject_supplies_the_carrier_of_its_indexed_parent() {
    let (_, typed) = source(
        "domain<const N: u8> u32::At<N>;
         domain u32::At<3>::Child;
         machine make() -> u32 in At::Child { 7u32 as +At::Child }",
    );
    let child = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "u32::At::Child")
        .unwrap();
    let parent = crate::source_state::domain::refinement_parent(&typed, child).unwrap();
    let applied = typed
        .domain_application(parent.domain_symbol, parent.semantic_domain)
        .unwrap();
    assert!(applied.is_closed);
    assert_eq!(applied.arguments.len(), 1);
    assert!(typed.deferred_refusals_of(child.symbol).next().is_none());
}

#[test]
fn casts_and_refinements_share_their_familys_typed_index() {
    const FAMILY: &str = "domain<const N: u8> u32::At<N>;";
    let (_, typed) = source(&format!(
        "{FAMILY}
        domain u32::At<3>::Child;
        machine restore(x: u32 in At<3>) -> u32 in At::Child {{ x as +At::Child }}
        machine peel(x: u32 in At::Child) -> u32 in At<3u8> {{ x as -At::Child =At<3> }}"
    ));
    let child = typed
        .domain_definitions()
        .iter()
        .find(|domain| domain.name.as_str() == "u32::At::Child")
        .unwrap();
    let parent = crate::source_state::domain::refinement_parent(&typed, child).unwrap();
    let kept = typed
        .expression_table
        .expression_entries()
        .find_map(|(_, node)| match node {
            ExpressionNode::Cast(cast)
                if cast
                    .form
                    .entry()
                    .is_some_and(|entry| entry.kind == CastEntryKind::Keep) =>
            {
                Some(cast)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        (kept.semantic_domain_symbol, kept.semantic_domain_id),
        (parent.domain_symbol, parent.semantic_domain)
    );
    assert!(typed.deferred_refusals.is_empty());

    for index in ["256", "3u64"] {
        for text in [
            format!("{FAMILY} machine wrong() -> u32 {{ 7u32 as +At<{index}> }}"),
            format!(
                "{FAMILY} domain u32::At<{index}>::Child; machine make() -> u32 in At::Child {{ 7u32 as +At::Child }}"
            ),
        ] {
            let error = source_result(&text)
                .1
                .expect_err("the argument must inhabit u8");
            assert!(error.message.contains("must be a `u8`"), "{error:?}");
            let span = error.source_span.unwrap().span;
            assert_eq!(&text[span.start..span.end], index);
        }
    }
}
