use super::SymbolHandle;
use crate::checks::borrows::persistent::StaticPersistentPath;

use super::{
    StaticPersistentSegment, rebase_static_paths_for_transition,
    static_persistent_segments_may_overlap,
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
