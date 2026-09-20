use super::SymbolHandle;
use crate::checks::borrows::persistent::StaticPersistentPath;
use crate::checks::borrows::persistent::retain_static_paths_across_call_frame;
use std::cell::Cell;

#[test]
fn empty_static_provenance_does_not_request_a_call_frame() {
    let queries = Cell::new(0);
    let mut paths = Vec::new();
    let retired = retain_static_paths_across_call_frame(
        &typed_trees::TypedTrees::default(),
        &typed_trees::state::State::default(),
        &[],
        &mut paths,
        &[],
        || {
            queries.set(queries.get() + 1);
            Some(Vec::new())
        },
    );
    assert!(!retired);
    assert!(paths.is_empty());
    assert_eq!(
        queries.get(),
        0,
        "there are no facts for the frame to invalidate"
    );
}

#[test]
fn either_static_frontier_still_requires_invalidation() {
    let field = SymbolHandle::from_parts(1, 1);
    let local = crate::flow::canonical_place_from_symbol(field).expect("local marker");
    for (mut paths, local_places) in [
        (
            vec![StaticPersistentPath {
                field,
                segments: Vec::new(),
            }],
            Vec::new(),
        ),
        (Vec::new(), vec![local]),
    ] {
        let queried = Cell::new(false);
        let retired = retain_static_paths_across_call_frame(
            &typed_trees::TypedTrees::default(),
            &typed_trees::state::State::default(),
            &[],
            &mut paths,
            &local_places,
            || {
                queried.set(true);
                None
            },
        );
        assert!(queried.get());
        assert!(
            retired,
            "an opaque call retires local canonical markers too"
        );
        assert!(paths.is_empty());
    }
}

use super::{
    StaticPersistentSegment, immutable_state_index_symbol, rebase_static_paths_for_transition,
    stable_index_origin_symbol, static_persistent_segments_may_overlap,
};
use arena::HandleSpan;
use numerics::literals::IntegerLiteral;
use typed_trees::expression::{ExpressionNode, TableNamePath};
use typed_trees::name::Identifier;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::TypeReferenceNode;

const FIELD_ITEMS: u32 = 40;
const PARAM_BOUNDARY: u32 = 41;
const PARAM_OTHER: u32 = 42;
const PARAM_RENAMED: u32 = 43;
const PARAM_RENAMED_OTHER: u32 = 44;
const PARAM_MUTABLE: u32 = 45;
const LOCAL_COPY: u32 = 46;
const LOCAL_ALIAS: u32 = 47;
const LOCAL_MUTABLE: u32 = 48;
const LOCAL_COMPUTED: u32 = 49;

fn sym(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn name_expression(
    program: &mut typed_trees::TypedTrees,
    symbol: SymbolHandle,
    name: &'static str,
) -> typed_trees::expression::ExpressionHandle {
    let members = program.expression_table.reserve_name_path_members(1);
    program.expression_table.set_name_path_member_at_offset(
        members,
        0,
        Identifier::generated_static(name),
    );
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            member_symbols: HandleSpan::empty(),
            head_symbol: symbol,
            symbol,
        }))
}

fn parameter(
    type_reference: typed_trees::types::TypeReferenceHandle,
    symbol: u32,
    name: &'static str,
    is_mutable: bool,
) -> StateParameter {
    StateParameter {
        symbol: sym(symbol),
        name: Identifier::generated_static(name),
        type_reference,
        is_mutable,
        ..Default::default()
    }
}

/// One source state `hold` carrying immutable parameters `boundary`/`other`
/// plus locals: `copy` and `alias` are immutable aliases of `boundary`
/// (`alias` copies `copy`), `mutated` is a mutable binding of the same value,
/// and `computed` binds a literal. A second state `use_held` takes two
/// immutable parameters; `use_held_mut` takes one mutable parameter.
/// Transition arguments are supplied per test.
fn forwarding_fixture() -> (typed_trees::TypedTrees, State, State, State) {
    let mut program = typed_trees::TypedTrees::default();
    let type_reference = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated_static("u64"),
        });
    let boundary_name = name_expression(&mut program, sym(PARAM_BOUNDARY), "boundary");
    let copy_name = name_expression(&mut program, sym(LOCAL_COPY), "copy");
    let zero = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));

    let mut source = State {
        parameters: program.state_parameters.insert_many([
            parameter(type_reference, PARAM_BOUNDARY, "boundary", false),
            parameter(type_reference, PARAM_OTHER, "other", false),
        ]),
        ..State::default()
    };

    for (symbol, name, initial_value, is_mutable) in [
        (LOCAL_COPY, "copy", boundary_name, false),
        (LOCAL_ALIAS, "alias", copy_name, false),
        (LOCAL_MUTABLE, "mutated", boundary_name, true),
        (LOCAL_COMPUTED, "computed", zero, false),
    ] {
        program.statement_table.push_statement(
            &mut source.statement_nodes,
            StatementNode::LocalData(TableLocalData {
                symbol: sym(symbol),
                name: Identifier::generated_static(name),
                type_reference,
                initial_value,
                is_mutable,
                ..Default::default()
            }),
        );
    }

    let target = State {
        parameters: program.state_parameters.insert_many([
            parameter(type_reference, PARAM_RENAMED, "renamed", false),
            parameter(type_reference, PARAM_RENAMED_OTHER, "renamed_other", false),
        ]),
        ..State::default()
    };

    let mutable_target = State {
        parameters: program.state_parameters.insert_many([parameter(
            type_reference,
            PARAM_MUTABLE,
            "mutated",
            true,
        )]),
        ..State::default()
    };

    (program, source, target, mutable_target)
}

#[test]
fn transition_rebases_a_stable_index_through_an_immutable_local_copy() {
    let (mut program, source, target, _) = forwarding_fixture();
    let argument = name_expression(&mut program, sym(LOCAL_ALIAS), "alias");
    let other_argument = name_expression(&mut program, sym(PARAM_OTHER), "other");
    let arguments = program
        .statement_table
        .insert_expression_handles([argument, other_argument]);
    let path = StaticPersistentPath {
        field: sym(FIELD_ITEMS),
        segments: vec![StaticPersistentSegment::StableIndex(sym(PARAM_BOUNDARY))],
    };

    let rebased =
        rebase_static_paths_for_transition(&program, &source, &target, arguments, &[path]);
    assert_eq!(
        rebased,
        vec![StaticPersistentPath {
            field: sym(FIELD_ITEMS),
            segments: vec![StaticPersistentSegment::StableIndex(sym(PARAM_RENAMED))],
        }],
        "an immutable local-copy chain must rebase onto the immutable target parameter"
    );
}

#[test]
fn transition_refuses_to_rebase_onto_a_mutable_target_parameter() {
    let (mut program, source, _, mutable_target) = forwarding_fixture();
    let argument = name_expression(&mut program, sym(PARAM_BOUNDARY), "boundary");
    let arguments = program
        .statement_table
        .insert_expression_handles([argument]);
    let path = StaticPersistentPath {
        field: sym(FIELD_ITEMS),
        segments: vec![StaticPersistentSegment::StableIndex(sym(PARAM_BOUNDARY))],
    };

    let rebased =
        rebase_static_paths_for_transition(&program, &source, &mutable_target, arguments, &[path]);
    assert!(
        rebased.is_empty(),
        "a stable index may not forward into a mutable target parameter"
    );
}

#[test]
fn transition_refuses_to_rebase_a_non_name_argument() {
    let (mut program, source, target, _) = forwarding_fixture();
    let literal = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let arguments = program
        .statement_table
        .insert_expression_handles([literal, literal]);
    let path = StaticPersistentPath {
        field: sym(FIELD_ITEMS),
        segments: vec![
            StaticPersistentSegment::FixedIndex(0),
            StaticPersistentSegment::StableIndex(sym(PARAM_BOUNDARY)),
        ],
    };

    let rebased =
        rebase_static_paths_for_transition(&program, &source, &target, arguments, &[path]);
    assert!(
        rebased.is_empty(),
        "a literal argument carries no stable index origin to rebase onto"
    );
}

#[test]
fn arity_mismatch_retires_only_the_stable_index_segments() {
    let (program, source, target, _) = forwarding_fixture();
    let indexed = StaticPersistentPath {
        field: sym(FIELD_ITEMS),
        segments: vec![StaticPersistentSegment::StableIndex(sym(PARAM_BOUNDARY))],
    };
    let fixed = StaticPersistentPath {
        field: sym(FIELD_ITEMS),
        segments: vec![StaticPersistentSegment::FixedIndex(1)],
    };

    let rebased = rebase_static_paths_for_transition(
        &program,
        &source,
        &target,
        HandleSpan::empty(),
        &[indexed.clone(), fixed.clone()],
    );
    assert_eq!(
        rebased,
        vec![fixed],
        "a malformed call arity must not discard field/fixed-index provenance"
    );
}

#[test]
fn every_stable_index_segment_must_rebase_or_the_path_retires() {
    let (mut program, source, target, _) = forwarding_fixture();
    // Two arguments forward `boundary` and a literal: the `other` segment has
    // no origin, so the whole path retires rather than half-rebasing.
    let boundary_name = name_expression(&mut program, sym(PARAM_BOUNDARY), "boundary");
    let literal = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(0)));
    let arguments = program
        .statement_table
        .insert_expression_handles([boundary_name, literal]);
    let path = StaticPersistentPath {
        field: sym(FIELD_ITEMS),
        segments: vec![
            StaticPersistentSegment::StableIndex(sym(PARAM_BOUNDARY)),
            StaticPersistentSegment::StableIndex(sym(PARAM_OTHER)),
        ],
    };

    let rebased =
        rebase_static_paths_for_transition(&program, &source, &target, arguments, &[path]);
    assert!(
        rebased.is_empty(),
        "a path is only as stable as its least-stable segment"
    );
}

#[test]
fn stable_index_origin_resolves_immutable_alias_chains_to_the_parameter() {
    let (program, source, _, _) = forwarding_fixture();
    let mut visiting = Vec::new();
    assert_eq!(
        stable_index_origin_symbol(&program, &source, sym(LOCAL_ALIAS), &mut visiting),
        Some(sym(PARAM_BOUNDARY)),
        "copy -> alias chains resolve to the origin parameter"
    );
    assert_eq!(
        stable_index_origin_symbol(&program, &source, sym(LOCAL_COPY), &mut Vec::new()),
        Some(sym(PARAM_BOUNDARY))
    );
    assert_eq!(
        stable_index_origin_symbol(&program, &source, sym(PARAM_BOUNDARY), &mut Vec::new()),
        Some(sym(PARAM_BOUNDARY)),
        "a parameter is its own origin"
    );
}

#[test]
fn stable_index_origin_refuses_mutable_and_non_name_origins() {
    let (program, source, _, _) = forwarding_fixture();
    assert_eq!(
        stable_index_origin_symbol(&program, &source, sym(LOCAL_MUTABLE), &mut Vec::new()),
        None,
        "a mutable binding is never a stable index origin"
    );
    assert_eq!(
        stable_index_origin_symbol(&program, &source, sym(LOCAL_COMPUTED), &mut Vec::new()),
        Some(sym(LOCAL_COMPUTED)),
        "an immutable local with a non-name initializer is its own origin"
    );
}

#[test]
fn immutable_state_index_symbol_rejects_ambiguous_or_mutable_symbols() {
    let (mut program, mut source, _, _) = forwarding_fixture();
    // A local sharing the parameter's own symbol makes the position ambiguous.
    let ambiguous = name_expression(&mut program, sym(PARAM_BOUNDARY), "boundary");
    program.statement_table.push_statement(
        &mut source.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: sym(PARAM_BOUNDARY),
            name: Identifier::generated_static("dup"),
            type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
            initial_value: ambiguous,
            is_mutable: false,
            ..Default::default()
        }),
    );
    assert_eq!(
        immutable_state_index_symbol(&program, &source, ambiguous),
        None,
        "a symbol matching both a parameter and a local is not a stable index"
    );
}

#[test]
fn segment_overlap_only_relies_on_exact_stable_index_identity() {
    let field_a = StaticPersistentSegment::Field(sym(FIELD_ITEMS));
    let field_b = StaticPersistentSegment::Field(sym(51));
    let index_zero = StaticPersistentSegment::FixedIndex(0);
    let index_one = StaticPersistentSegment::FixedIndex(1);
    let stable_boundary = StaticPersistentSegment::StableIndex(sym(PARAM_BOUNDARY));
    let stable_other = StaticPersistentSegment::StableIndex(sym(PARAM_OTHER));

    for (left, right, expected) in [
        // Distinct field or fixed-index siblings are disjoint.
        (vec![field_a], vec![field_b], false),
        (vec![index_zero], vec![index_one], false),
        // A shared prefix tail may still select the same place.
        (vec![index_zero], vec![index_zero, field_a], true),
        // Only identical stable-index symbols are a proof of sharing.
        (vec![stable_boundary], vec![stable_boundary], true),
        (vec![stable_boundary], vec![stable_other], true),
        // Mixed classes at one depth are never disjointness evidence.
        (vec![index_zero], vec![stable_boundary], true),
        (vec![field_a], vec![index_zero], true),
    ] {
        assert_eq!(
            static_persistent_segments_may_overlap(&left, &right),
            expected,
            "{left:?} vs {right:?}"
        );
    }
}
