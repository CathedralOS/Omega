//! Casts retain their exact selected domain and read declaration-owned values.

use super::*;
use crate::source_state::statement::{StatementNode, TransitionTargetNode};
use crate::typing::typing_queries::{declared_callable_value_type, declared_place_type};
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
    let tokens = crate::typing::frontend_fixture::Lexer::new(text)
        .tokenize()
        .unwrap();
    let syntax = crate::typing::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
    let mut request = crate::resolution::ResolutionRequest::new(syntax);
    request.sources = Some(Arc::new(sources));
    let resolved = crate::resolution::resolve(request).unwrap();
    let typed = crate::typing::lower_symbol_resolved_trees(&resolved);
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
fn a_borrowed_collection_cast_keeps_its_declared_field_type() {
    let (_, typed) = source(
        r#"
domain [u8; 2]::Marked;
data Packet { bytes: [u8; 2] in Marked; }
machine update(packet: @mut Packet) {
    (@mut packet.bytes as -Marked)[0] = 1;
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
        declaration_contracts::SemanticDomainId::NULL
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
machine update(packet: @mut Packet) {
    let wrong: bool = @mut packet.bytes as -Marked;
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
        crate::typing::expression_meanings::prepare(&resolved, &mut typed, 0, 0, &[]).unwrap_err();
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
machine update(packet: @mut Packet) {
    (@mut packet.bytes as -Terminated)[packet.index as -Wrapping] = 0;
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
