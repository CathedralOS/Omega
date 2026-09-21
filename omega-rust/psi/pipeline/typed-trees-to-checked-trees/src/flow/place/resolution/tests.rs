//! Case qualification selects declaration identity, not the first matching spelling.
use super::effective_member_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::name::Identifier;
use symbols::SymbolHandle;
use typed_trees::expression::{
    MatchPattern, TableCastExpression, TableIndexedExpression, TableMatchArm, TableMatchExpression,
    TableMemberExpression,
};
use typed_trees::statement::StatementNode;

fn fixture() -> (typed_trees::TypedTrees, TableMemberExpression) {
    let source = r#"
        pub data Cell { item: u64; }
        pub data Outcome { case First(c: Cell); case Second(c: Cell); }
        pub data Other { case Second(c: Cell); }
        machine observe(value: Outcome) -> u64 {
            transition value {
                Outcome::First { c } -> done(c.item)
                Outcome::Second { c } -> done(c.item)
            }
            state done(x: u64) { x }
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let member = program
        .expression_table
        .iter_expressions()
        .find_map(|(_, node)| match node {
            ExpressionNode::Member(member)
                if member
                    .case_variant
                    .as_ref()
                    .is_some_and(|name| name.as_str() == "Second") =>
            {
                Some(member.clone())
            }
            _ => None,
        })
        .expect("case pattern retains its qualified payload projection");
    (program, member)
}

fn field(program: &typed_trees::TypedTrees, type_name: &str, case_name: &str) -> SymbolHandle {
    let declaration = program
        .data_definitions()
        .iter()
        .find(|row| row.name.as_str() == type_name)
        .unwrap();
    let variant = program
        .data_members(declaration)
        .iter()
        .find_map(|row| match row {
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == case_name =>
            {
                Some(variant)
            }
            _ => None,
        })
        .unwrap();
    program.data_payload_fields(variant)[0].symbol
}

#[test]
fn case_qualified_payload_preserves_the_selected_case_identity() {
    let (program, mut member) = fixture();
    let expected = field(&program, "Outcome", "Second");
    assert_ne!(expected, field(&program, "Outcome", "First"));
    member.member_symbol = SymbolHandle::invalid();
    assert_eq!(
        effective_member_symbol(&program, member.receiver, &member),
        expected
    );
    member.member_symbol = expected;
    assert_eq!(
        effective_member_symbol(&program, member.receiver, &member),
        expected
    );
}

#[test]
fn case_qualified_payload_rejects_conflicting_retained_identity() {
    let (program, mut member) = fixture();
    for conflicting in [
        field(&program, "Outcome", "First"),
        field(&program, "Other", "Second"),
    ] {
        member.member_symbol = conflicting;
        assert!(!effective_member_symbol(&program, member.receiver, &member).is_valid());
    }
}

/// The `requires` fact `b.item.scheduler` crosses a generic application:
/// `b: &Box<Context>` binds `item`'s declared `T` to `Context`, so the
/// `scheduler` hop must resume at `Context`'s declaration rather than stop at
/// the opaque parameter — the same leaf the typed-tree lowerer and the
/// checker-side partition replay now cross.
fn generic_leaf_fixture() -> (
    typed_trees::TypedTrees,
    TableMemberExpression,
    TableMemberExpression,
) {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context { scheduler: SchedulerHandle; }
        pub data Box<T> { item: T; }
        pub domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        pub boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        pub machine wait_boxed(b: &Box<Context>)
        requires b.item.scheduler in WeakFair
        terminates;
        -> u64 { 0 }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let members: Vec<TableMemberExpression> = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) => Some(member.clone()),
            _ => None,
        })
        .collect();
    let item = members
        .iter()
        .find(|member| member.member.as_str() == "item")
        .expect("b.item member expression")
        .clone();
    let scheduler = members
        .iter()
        .find(|member| member.member.as_str() == "scheduler")
        .expect("b.item.scheduler member expression")
        .clone();
    (program, item, scheduler)
}

fn declared_field(
    program: &typed_trees::TypedTrees,
    type_name: &str,
    field_name: &str,
) -> SymbolHandle {
    let declaration = program
        .data_definitions()
        .iter()
        .find(|row| row.name.as_str() == type_name)
        .unwrap();
    program
        .data_members(declaration)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == field_name => {
                Some(field.symbol)
            }
            _ => None,
        })
        .unwrap()
}

#[test]
fn member_resolution_replays_a_generic_application_leaf() {
    let (program, item, scheduler) = generic_leaf_fixture();
    let item_symbol = declared_field(&program, "Box", "item");
    let scheduler_symbol = declared_field(&program, "Context", "scheduler");

    // `b.item` still names Box's own member through the application's
    // declaration.
    assert_eq!(
        effective_member_symbol(&program, item.receiver, &item),
        item_symbol
    );
    // `item`'s declared `T` resumes at the bound `Context` argument, so the
    // `scheduler` hop resolves `Context::scheduler` instead of stopping at an
    // opaque parameter.
    assert_eq!(
        effective_member_symbol(&program, scheduler.receiver, &scheduler),
        scheduler_symbol
    );
    assert_eq!(
        super::expression_type_symbol(&program, scheduler.receiver),
        Some(declared_data_symbol(&program, "Context"))
    );
}

#[test]
fn member_resolution_through_a_generic_leaf_still_names_a_declared_member() {
    let (program, _, scheduler) = generic_leaf_fixture();
    let mut missing = scheduler.clone();
    missing.member = Identifier::generated("missing");
    assert!(!effective_member_symbol(&program, scheduler.receiver, &missing).is_valid());
}

#[test]
fn requires_fact_through_a_generic_leaf_resolves_at_call_sites() {
    // The caller's own entry requirement lands `boxed.item.scheduler` on the
    // exact field path; the call must instantiate `b.item.scheduler` onto the
    // same canonical place to discharge `wait_boxed`'s requires.
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context { scheduler: SchedulerHandle; }
        pub data Box<T> { item: T; }
        pub domain SchedulerHandle::WeakFair
        satisfies ProgressProfile
        established by SchedulerAdmission::grant;
        pub boundary trait SchedulerAdmission {
            machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
        }
        pub machine wait_boxed(b: &Box<Context>)
        requires b.item.scheduler in WeakFair
        terminates;
        -> u64 { 0 }
        machine caller(boxed: &Box<Context>)
        requires boxed.item.scheduler in WeakFair
        {
            let dropped: u64 = wait_boxed(boxed);
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(program, &crate::CheckingRequest::settled())
        .expect("a requires fact through a generic leaf discharges at the call site");
}

/// `values[i].item.scheduler` crosses an indexed leaf before the generic
/// application: `values: &[Box<Context>]` projects the index hop onto the
/// bound `Box<Context>` argument, whose `item: T` then resumes at `Context`.
/// Dropping the collection's retained position at the index hop would stop
/// `scheduler` at the unbound `T`, so this is the same replay one shape later.
fn indexed_generic_leaf_fixture() -> (
    typed_trees::TypedTrees,
    TableMemberExpression,
    TableMemberExpression,
) {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context { scheduler: SchedulerHandle; }
        pub data Box<T> { item: T; }
        machine hold(values: &[Box<Context>]) {
            let i: u64 = 1;
            let s: SchedulerHandle = values[0].item.scheduler;
            let t: SchedulerHandle = values[i].item.scheduler;
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let members: Vec<TableMemberExpression> = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) => Some(member.clone()),
            _ => None,
        })
        .collect();
    let item = members
        .iter()
        .find(|member| member.member.as_str() == "item")
        .expect("values[0].item member expression")
        .clone();
    let scheduler = members
        .iter()
        .find(|member| member.member.as_str() == "scheduler")
        .expect("values[0].item.scheduler member expression")
        .clone();
    (program, item, scheduler)
}

#[test]
fn member_resolution_replays_an_indexed_generic_leaf() {
    let (program, item, scheduler) = indexed_generic_leaf_fixture();
    let item_symbol = declared_field(&program, "Box", "item");
    let scheduler_symbol = declared_field(&program, "Context", "scheduler");

    // `values[0].item` still names Box's own member; the index hop keeps the
    // reaching `&[Box<Context>]` position instead of dropping it.
    assert_eq!(
        effective_member_symbol(&program, item.receiver, &item),
        item_symbol
    );
    // `item`'s declared `T` resumes at the bound `Context` argument across the
    // indexed leaf, so `scheduler` resolves `Context::scheduler`.
    assert_eq!(
        effective_member_symbol(&program, scheduler.receiver, &scheduler),
        scheduler_symbol
    );
    assert_eq!(
        super::expression_type_symbol(&program, scheduler.receiver),
        Some(declared_data_symbol(&program, "Context"))
    );
}

#[test]
fn member_resolution_replays_a_runtime_indexed_generic_leaf() {
    let (program, _, _) = indexed_generic_leaf_fixture();
    let runtime_scheduler = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) if member.member.as_str() == "scheduler" => {
                Some(member.clone())
            }
            _ => None,
        })
        .find(|member| {
            let ExpressionNode::Member(item) = program.expression_table.expression(member.receiver)
            else {
                return false;
            };
            let ExpressionNode::Indexed(indexed) =
                program.expression_table.expression(item.receiver)
            else {
                return false;
            };
            !program
                .expression_table
                .constant_integer_value(indexed.index)
                .is_some()
        })
        .expect("values[i].item.scheduler member expression");
    assert_eq!(
        effective_member_symbol(&program, runtime_scheduler.receiver, &runtime_scheduler),
        declared_field(&program, "Context", "scheduler")
    );
}

#[test]
fn member_resolution_through_an_indexed_leaf_still_names_a_declared_member() {
    let (program, _, scheduler) = indexed_generic_leaf_fixture();
    let mut missing = scheduler.clone();
    missing.member = Identifier::generated("missing");
    assert!(!effective_member_symbol(&program, scheduler.receiver, &missing).is_valid());
}

fn declared_data_symbol(program: &typed_trees::TypedTrees, type_name: &str) -> SymbolHandle {
    program
        .data_definitions()
        .iter()
        .find(|row| row.name.as_str() == type_name)
        .unwrap()
        .symbol
}

/// Drop every retained symbol along a member receiver's place chain — name
/// path member/head/leaf symbols and intermediate member symbols — so only
/// the contextual place walk can still identify the demanded member. This is
/// the gap `resolve_member_symbol_from_place` exists for; with the receiver's
/// symbols present the expression route answers first.
fn strip_receiver_symbols(program: &mut typed_trees::TypedTrees, receiver: ExpressionHandle) {
    let mut cursor = receiver;
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            ExpressionNode::Borrow(borrow) => cursor = borrow.target,
            ExpressionNode::Member(member) => {
                let next = member.receiver;
                let ExpressionNode::Member(member) =
                    program.expression_table.expression_mut(cursor)
                else {
                    unreachable!();
                };
                member.member_symbol = SymbolHandle::invalid();
                cursor = next;
            }
            ExpressionNode::Name(path) => {
                let member_symbols = path.member_symbols;
                let count = program
                    .expression_table
                    .name_path_member_symbols(member_symbols)
                    .len();
                for offset in 0..count {
                    program
                        .expression_table
                        .set_name_path_member_symbol_at_offset(
                            member_symbols,
                            offset as u32,
                            SymbolHandle::invalid(),
                        );
                }
                let ExpressionNode::Name(path) = program.expression_table.expression_mut(cursor)
                else {
                    unreachable!();
                };
                path.head_symbol = SymbolHandle::invalid();
                path.symbol = SymbolHandle::invalid();
                break;
            }
            _ => break,
        }
    }
}

/// The statement index carrying `member_handle`, so the contextual walk sees
/// the same local-prefix window the real call site passes.
fn member_statement_index(
    program: &typed_trees::TypedTrees,
    state: &typed_trees::state::State,
    member_handle: ExpressionHandle,
) -> usize {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .position(|statement| {
            let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                return false;
            };
            let mut cursor = local.initial_value;
            loop {
                if cursor == member_handle {
                    return true;
                }
                cursor = match program.expression_table.expression(cursor) {
                    ExpressionNode::Member(member) => member.receiver,
                    ExpressionNode::Indexed(indexed) => indexed.collection,
                    ExpressionNode::Borrow(borrow) => borrow.target,
                    _ => return false,
                };
            }
        })
        .expect("the member expression belongs to a local initializer in the fixture")
}

/// `values[<lit>].item` members under `machine hold(values: &[Box<Context>])`,
/// selected by whether the index is a compile-time constant. Returns the
/// member handle, the holding state's symbol, and the statement index.
fn indexed_item_member(
    program: &typed_trees::TypedTrees,
    constant_index: bool,
) -> (ExpressionHandle, SymbolHandle, usize) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .expect("hold machine");
    let state = &program.machine_states(machine)[0];
    let member_handle = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            if member.member.as_str() != "item" {
                return None;
            }
            let ExpressionNode::Indexed(indexed) =
                program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            (program
                .expression_table
                .constant_integer_value(indexed.index)
                .is_some()
                == constant_index)
                .then_some(handle)
        })
        .expect("values[..].item member expression");
    let statement_index = member_statement_index(program, state, member_handle);
    (member_handle, state.symbol, statement_index)
}

#[test]
fn place_member_resolution_replays_an_indexed_generic_leaf() {
    let (mut program, _, _) = indexed_generic_leaf_fixture();
    let item_symbol = declared_field(&program, "Box", "item");
    let (member_handle, state_symbol, statement_index) = indexed_item_member(&program, true);
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .unwrap();
    let values_parameter = program
        .state_parameters(&program.machine_states(machine)[0])
        .iter()
        .find(|parameter| parameter.name.as_str() == "values")
        .expect("values parameter")
        .symbol;

    // The retained receiver symbols are what let `effective_member_symbol`
    // answer first; strip them so the demanded member can only be resolved by
    // replaying the place walk over the parameter's declared `&[Box<Context>]`.
    let member = match program.expression_table.expression(member_handle) {
        ExpressionNode::Member(member) => member.clone(),
        _ => unreachable!(),
    };
    let receiver = member.receiver;
    strip_receiver_symbols(&mut program, receiver);
    assert!(
        !effective_member_symbol(&program, receiver, &member).is_valid(),
        "the stripped receiver must leave the expression route unanswered"
    );

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("a parameter-rooted indexed member place resolves");
    assert_eq!(place.root, facts::PlaceRoot::Symbol(values_parameter));
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::FixedIndex { index: 0 },
            facts::PlaceSegment::Field {
                symbol: item_symbol
            },
        ]
    );
}

#[test]
fn place_member_resolution_replays_a_runtime_indexed_generic_leaf() {
    let (mut program, _, _) = indexed_generic_leaf_fixture();
    let item_symbol = declared_field(&program, "Box", "item");
    let (member_handle, state_symbol, statement_index) = indexed_item_member(&program, false);
    let ExpressionNode::Member(member) = program.expression_table.expression(member_handle) else {
        unreachable!();
    };
    let receiver = member.receiver;
    let index = match program.expression_table.expression(receiver) {
        ExpressionNode::Indexed(indexed) => indexed.index,
        _ => unreachable!(),
    };
    strip_receiver_symbols(&mut program, receiver);

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("a parameter-rooted indexed member place resolves");
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::Index { expression: index },
            facts::PlaceSegment::Field {
                symbol: item_symbol
            },
        ]
    );
}

#[test]
fn place_member_resolution_across_an_index_still_requires_a_declared_member() {
    let (mut program, _, _) = indexed_generic_leaf_fixture();
    let (member_handle, state_symbol, statement_index) = indexed_item_member(&program, true);
    let receiver = match program.expression_table.expression(member_handle) {
        ExpressionNode::Member(member) => member.receiver,
        _ => unreachable!(),
    };
    strip_receiver_symbols(&mut program, receiver);
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(member_handle)
    else {
        unreachable!();
    };
    member.member = Identifier::generated("missing");

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("the place walk still builds");
    // The index hop replays onto `Box<Context>`, whose declaration has no
    // `missing` member: the demanded field stays unresolved rather than
    // minting the collection's own identity for it.
    assert!(matches!(
        place.segments.last(),
        Some(facts::PlaceSegment::Field { symbol }) if !symbol.is_valid()
    ));
}

#[test]
fn case_qualified_payload_does_not_fall_back_when_qualification_is_missing() {
    let (program, original) = fixture();
    let mut member = original.clone();
    member.member_symbol = field(&program, "Outcome", "Second");
    member.case_variant = Some(Identifier::generated("Absent"));
    assert!(!effective_member_symbol(&program, member.receiver, &member).is_valid());
    member = original.clone();
    member.member = Identifier::generated("absent");
    assert!(!effective_member_symbol(&program, member.receiver, &member).is_valid());
    assert!(!effective_member_symbol(&program, ExpressionHandle::invalid(), &original).is_valid());
}

/// The `Second`-qualified `count` member inside `observe`'s transition, plus
/// its state symbol and the transition's statement index — the contextual
/// walk's local-prefix window. The demanded member is a destructure-bound
/// payload projection: `member.member` names the field and
/// `member.case_variant` names the case that owns it.
fn second_qualified_member(
    program: &typed_trees::TypedTrees,
) -> (ExpressionHandle, SymbolHandle, usize) {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("observe machine");
    let state = &program.machine_states(machine)[0];
    let member_handle = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            member
                .case_variant
                .as_ref()
                .is_some_and(|name| name.as_str() == "Second")
                .then_some(handle)
        })
        .expect("Second-qualified member expression");
    let statement_index = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
        .expect("the case-qualified member lives in the transition statement");
    (member_handle, state.symbol, statement_index)
}

#[test]
fn place_member_resolution_scopes_a_case_qualified_member_to_its_variant() {
    let (mut program, _) = fixture();
    let expected = field(&program, "Outcome", "Second");
    let second = facts::payload_variant_for_field(&program, expected)
        .expect("Second::count's owning variant");
    let (member_handle, state_symbol, statement_index) = second_qualified_member(&program);
    let member = match program.expression_table.expression(member_handle) {
        ExpressionNode::Member(member) => member.clone(),
        _ => unreachable!(),
    };
    // `First::count` and `Second::count` share one spelling. Strip the
    // receiver's retained symbols so only the place walk can name the field,
    // and the demanded case is the only thing distinguishing them.
    strip_receiver_symbols(&mut program, member.receiver);
    assert!(
        !effective_member_symbol(&program, member.receiver, &member).is_valid(),
        "the stripped receiver must leave the expression route unanswered"
    );

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("a parameter-rooted case-qualified member place resolves");
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::Case { variant: second },
            facts::PlaceSegment::Field { symbol: expected },
        ]
    );
}

#[test]
fn place_member_resolution_rejects_a_case_qualified_member_outside_its_variant() {
    // `First` and `Second` declare different payload spellings, so demanding
    // `count` under `Second` must fail closed: the sibling case's field is
    // never the selected one.
    let source = r#"
        data Outcome { case First(count: u64); case Second(total: u64); }
        machine observe(value: Outcome) {
            transition value {
                Outcome::Second { total } -> done(total)
                Outcome::First { count } -> done(count)
            }
            state done(count: u64) {}
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let mut program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let (member_handle, state_symbol, statement_index) = second_qualified_member(&program);
    let member = match program.expression_table.expression(member_handle) {
        ExpressionNode::Member(member) => member.clone(),
        _ => unreachable!(),
    };
    strip_receiver_symbols(&mut program, member.receiver);
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(member_handle)
    else {
        unreachable!();
    };
    member.member = Identifier::generated("count");

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("the place walk still builds");
    assert!(matches!(
        place.segments.last(),
        Some(facts::PlaceSegment::Field { symbol }) if !symbol.is_valid()
    ));
}

#[test]
fn place_member_resolution_rejects_an_absent_case_qualification() {
    let (mut program, _) = fixture();
    let (member_handle, state_symbol, statement_index) = second_qualified_member(&program);
    let member = match program.expression_table.expression(member_handle) {
        ExpressionNode::Member(member) => member.clone(),
        _ => unreachable!(),
    };
    strip_receiver_symbols(&mut program, member.receiver);
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(member_handle)
    else {
        unreachable!();
    };
    member.case_variant = Some(Identifier::generated("Absent"));

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("the place walk still builds");
    // `Absent` names no variant of `Outcome`: the demanded field stays
    // unresolved rather than borrowing the first same-spelled payload field.
    assert!(matches!(
        place.segments.last(),
        Some(facts::PlaceSegment::Field { symbol }) if !symbol.is_valid()
    ));
}

/// `values[0..2][i].item.scheduler` crosses a fixed window before the indexed
/// leaf: the range hop yields a slice over the same element, so the following
/// index hop resumes at `Box<Context>` and `item`'s declared `T` still lands
/// on `Context`. A range that dropped the position would leave `scheduler`
/// unprovable even though every hop is exact.
fn ranged_indexed_generic_leaf_fixture() -> (
    typed_trees::TypedTrees,
    TableMemberExpression,
    TableMemberExpression,
    ExpressionHandle,
    SymbolHandle,
    usize,
) {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context { scheduler: SchedulerHandle; }
        pub data Box<T> { item: T; }
        machine hold(values: &[Box<Context>]) {
            let i: u64 = 1;
            let s: SchedulerHandle = values[0..2][i].item.scheduler;
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let members: Vec<TableMemberExpression> = program
        .expression_table
        .iter_expressions()
        .filter_map(|(_, node)| match node {
            ExpressionNode::Member(member) => Some(member.clone()),
            _ => None,
        })
        .collect();
    let item = members
        .iter()
        .find(|member| member.member.as_str() == "item")
        .expect("values[0..2][i].item member expression")
        .clone();
    let scheduler = members
        .iter()
        .find(|member| member.member.as_str() == "scheduler")
        .expect("values[0..2][i].item.scheduler member expression")
        .clone();
    let scheduler_handle = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            (member.member.as_str() == "scheduler").then_some(handle)
        })
        .expect("scheduler member handle");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .expect("hold machine");
    let state_symbol = program.machine_states(machine)[0].symbol;
    let statement_index = {
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "hold")
            .expect("hold machine");
        let state = &program.machine_states(machine)[0];
        member_statement_index(&program, state, scheduler_handle)
    };
    (
        program,
        item,
        scheduler,
        scheduler_handle,
        state_symbol,
        statement_index,
    )
}

#[test]
fn member_resolution_replays_a_ranged_indexed_leaf() {
    let (program, _, scheduler, ..) = ranged_indexed_generic_leaf_fixture();
    // The expression route answers from types alone: `values[0..2]` is a
    // window over `Box<Context>`, `[i]` names one element, and `item`'s
    // declared `T` resumes at the bound `Context` argument.
    assert_eq!(
        effective_member_symbol(&program, scheduler.receiver, &scheduler),
        declared_field(&program, "Context", "scheduler")
    );
    assert_eq!(
        super::expression_type_symbol(&program, scheduler.receiver),
        Some(declared_data_symbol(&program, "Context"))
    );
}

#[test]
fn place_member_resolution_replays_a_ranged_indexed_leaf() {
    let (mut program, _item, scheduler, scheduler_handle, state_symbol, statement_index) =
        ranged_indexed_generic_leaf_fixture();
    let item_symbol = declared_field(&program, "Box", "item");
    let scheduler_symbol = declared_field(&program, "Context", "scheduler");

    // Strip the receiver chain's retained symbols so only the contextual
    // place walk can name either member.
    strip_receiver_symbols(&mut program, scheduler.receiver);
    let ExpressionNode::Member(item_member) =
        program.expression_table.expression(scheduler.receiver)
    else {
        unreachable!();
    };
    assert!(
        !effective_member_symbol(&program, item_member.receiver, item_member).is_valid(),
        "the stripped receiver must leave the expression route unanswered"
    );

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        scheduler_handle,
    )
    .expect("a parameter-rooted ranged member place resolves");
    let ExpressionNode::Indexed(inner) = program.expression_table.expression(item_member.receiver)
    else {
        unreachable!();
    };
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::FixedRange { start: 0, end: 2 },
            facts::PlaceSegment::Index {
                expression: inner.index
            },
            facts::PlaceSegment::Field {
                symbol: item_symbol
            },
            facts::PlaceSegment::Field {
                symbol: scheduler_symbol
            },
        ]
    );
}

#[test]
fn place_member_resolution_across_a_range_still_requires_a_declared_member() {
    let (mut program, _, scheduler, scheduler_handle, state_symbol, statement_index) =
        ranged_indexed_generic_leaf_fixture();
    strip_receiver_symbols(&mut program, scheduler.receiver);
    let ExpressionNode::Member(member) = program.expression_table.expression_mut(scheduler_handle)
    else {
        unreachable!();
    };
    member.member = Identifier::generated("missing");

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        scheduler_handle,
    )
    .expect("the place walk still builds");
    // The range hop replays onto `Context`, which has no `missing` member:
    // the demanded field stays unresolved rather than minting the window's
    // element for it.
    assert!(matches!(
        place.segments.last(),
        Some(facts::PlaceSegment::Field { symbol }) if !symbol.is_valid()
    ));
}

#[test]
fn ranged_window_members_do_not_borrow_the_elements_fields() {
    let (mut program, item, ..) = ranged_indexed_generic_leaf_fixture();
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(item.receiver)
    else {
        unreachable!();
    };
    // `values[0..2]` — the window expression itself. A member demanded
    // directly on it names a field of the slice, which declares none: the
    // window must not borrow `Box<Context>`'s `item`.
    let window = indexed.collection;
    strip_receiver_symbols(&mut program, window);
    let mut member = item.clone();
    member.member = Identifier::generated("item");
    member.member_symbol = SymbolHandle::invalid();
    member.receiver = window;
    let member_handle = program
        .expression_table
        .insert(ExpressionNode::Member(member));
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .expect("hold machine");
    let state = &program.machine_states(machine)[0];
    let statement_index = member_statement_index(&program, state, item.receiver);

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state.symbol,
        statement_index,
        member_handle,
    )
    .expect("the place walk still builds");
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::FixedRange { start: 0, end: 2 },
            facts::PlaceSegment::Field {
                symbol: SymbolHandle::invalid()
            },
        ]
    );
}

/// An atomic leaf's position is its operand's position: the checked
/// interpreter evaluates `atomic.value`, and `expression_type_reference_
/// in_state` maps the node the same way, so a member resolved against an
/// atomic receiver stands on the operand's declared type.
#[test]
fn atomic_expression_position_is_its_operand_position() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context { scheduler: SchedulerHandle; }
        machine hold(context: Context) {
            let s: SchedulerHandle = context.scheduler;
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let mut program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let context = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Name(path) = node else {
                return None;
            };
            (program.expression_table.display_name(handle) == "context"
                && program
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 1)
                .then_some(handle)
        })
        .expect("the context name expression");
    let atomic = program.expression_table.insert(ExpressionNode::Atomic(
        typed_trees::expression::TableAtomicExpression {
            value: context,
            result: ExpressionHandle::invalid(),
            ordering: language_core::atomic::AtomicOrderingPlan::Load(
                language_core::atomic::MemoryOrdering::NoOrdering,
            ),
            result_custody: language_core::atomic::AtomicExpressionResultCustody::Scalar,
        },
    ));
    assert_eq!(
        super::expression_type_symbol(&program, atomic),
        Some(declared_data_symbol(&program, "Context"))
    );
}

/// `hold(context: Context)` supplies the `context` name, the
/// `context.scheduler` member's handle, and the parameter's stored `Context`
/// reference — the leaf material the self-typed and collection leaves resume
/// at. `Context` is `[copy]` so a synthesized `[context, context]` literal or
/// a two-arm `match` needs no move custody to lower.
fn stored_type_leaf_fixture() -> (
    typed_trees::TypedTrees,
    ExpressionHandle,
    ExpressionHandle,
    typed_trees::types::TypeReferenceHandle,
) {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context [copy] { scheduler: SchedulerHandle; }
        machine hold(context: Context) {
            let s: SchedulerHandle = context.scheduler;
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let context = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Name(_) = node else {
                return None;
            };
            (program.expression_table.display_name(handle) == "context").then_some(handle)
        })
        .expect("the context name expression");
    let scheduler_member = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            (member.member.as_str() == "scheduler").then_some(handle)
        })
        .expect("the context.scheduler member expression");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "hold")
        .expect("hold machine");
    let context_type = program
        .state_parameters(&program.machine_states(machine)[0])
        .iter()
        .find(|parameter| parameter.name.as_str() == "context")
        .expect("context parameter")
        .type_reference;
    (program, context, scheduler_member, context_type)
}

/// A cast stores its complete normalized result type on the node, so the
/// leaf's position is that reference outright: a member hop on the cast
/// resolves the field the result type declares instead of stopping at an
/// opaque leaf. An unresolved stored result keeps no position.
#[test]
fn cast_expression_position_is_its_stored_result_type() {
    let (mut program, context, _, context_type) = stored_type_leaf_fixture();
    let cast = program
        .expression_table
        .insert(ExpressionNode::Cast(TableCastExpression {
            value: context,
            target_type: context_type,
            result_type: context_type,
            target_label: arena::HandleSpan::empty(),
            domain: numerics::arithmetic::ArithmeticDomain::Exact,
            semantic_domain: arena::HandleSpan::empty(),
            semantic_domain_arguments: arena::HandleSpan::empty(),
            semantic_domain_symbol: SymbolHandle::invalid(),
            semantic_domain_id: language_semantics::SemanticDomainId::NULL,
            form: language_core::cast_form::CastForm::Value,
        }));
    assert_eq!(
        super::expression_type_symbol(&program, cast),
        Some(declared_data_symbol(&program, "Context"))
    );
    let member = TableMemberExpression {
        receiver: cast,
        member: Identifier::generated("scheduler"),
        member_symbol: SymbolHandle::invalid(),
        case_variant: None,
    };
    assert_eq!(
        effective_member_symbol(&program, cast, &member),
        declared_field(&program, "Context", "scheduler")
    );

    let unresolved = {
        let ExpressionNode::Cast(cast) = program.expression_table.expression(cast) else {
            unreachable!()
        };
        let mut cast = *cast;
        cast.result_type = typed_trees::types::TypeReferenceHandle::invalid();
        program.expression_table.insert(ExpressionNode::Cast(cast))
    };
    assert!(super::expression_type_symbol(&program, unresolved).is_none());
}

/// The proof-only zero value of `T` is a `T`: the node stores the exact
/// reference, so its position is that reference outright and a member hop
/// resolves the field `T` declares. An invalid stored reference keeps no
/// position.
#[test]
fn zero_value_position_is_its_stored_type() {
    let (mut program, _, _, context_type) = stored_type_leaf_fixture();
    let zero = program
        .expression_table
        .insert(ExpressionNode::ZeroValue(context_type));
    assert_eq!(
        super::expression_type_symbol(&program, zero),
        Some(declared_data_symbol(&program, "Context"))
    );
    let member = TableMemberExpression {
        receiver: zero,
        member: Identifier::generated("scheduler"),
        member_symbol: SymbolHandle::invalid(),
        case_variant: None,
    };
    assert_eq!(
        effective_member_symbol(&program, zero, &member),
        declared_field(&program, "Context", "scheduler")
    );

    let invalid = program.expression_table.insert(ExpressionNode::ZeroValue(
        typed_trees::types::TypeReferenceHandle::invalid(),
    ));
    assert!(super::expression_type_symbol(&program, invalid).is_none());
}

/// An array literal is a fixed collection over one element type: an index
/// hop resumes at the element, so `[context, context][0].scheduler` lands on
/// `Context::scheduler` — the same window contract a ranged leaf already
/// carries, since the literal declares no fields of its own.
#[test]
fn array_literal_position_is_a_window_over_its_agreed_element() {
    let (mut program, context, _, _) = stored_type_leaf_fixture();
    let elements = program
        .expression_table
        .insert_expression_handles([context, context]);
    let literal = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements));
    let index = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::zero(),
    ));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection: literal,
                index,
            }));
    assert_eq!(
        super::expression_type_symbol(&program, indexed),
        Some(declared_data_symbol(&program, "Context"))
    );
    let member = TableMemberExpression {
        receiver: indexed,
        member: Identifier::generated("scheduler"),
        member_symbol: SymbolHandle::invalid(),
        case_variant: None,
    };
    assert_eq!(
        effective_member_symbol(&program, indexed, &member),
        declared_field(&program, "Context", "scheduler")
    );
}

/// A member demanded on the literal itself names a field of the array, which
/// declares none: the window position must not lend the element's members to
/// the collection, exactly as a ranged window refuses them.
#[test]
fn array_literal_members_resolve_nothing_on_the_literal_itself() {
    let (mut program, context, _, _) = stored_type_leaf_fixture();
    let elements = program
        .expression_table
        .insert_expression_handles([context, context]);
    let literal = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements));
    let member = TableMemberExpression {
        receiver: literal,
        member: Identifier::generated("scheduler"),
        member_symbol: SymbolHandle::invalid(),
        case_variant: None,
    };
    assert!(!effective_member_symbol(&program, literal, &member).is_valid());
}

/// Every element must agree on one exact stored position: `[context,
/// context.scheduler]` mixes a `Context` leaf with a `SchedulerHandle` leaf,
/// so the literal keeps no position rather than naming one element's type
/// for the whole collection.
#[test]
fn array_literal_positions_require_every_element_to_agree() {
    let (mut program, context, scheduler_member, _) = stored_type_leaf_fixture();
    let elements = program
        .expression_table
        .insert_expression_handles([context, scheduler_member]);
    let literal = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements));
    assert!(super::expression_type_symbol(&program, literal).is_none());
    let index = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::zero(),
    ));
    let indexed =
        program
            .expression_table
            .insert(ExpressionNode::Indexed(TableIndexedExpression {
                collection: literal,
                index,
            }));
    let member = TableMemberExpression {
        receiver: indexed,
        member: Identifier::generated("scheduler"),
        member_symbol: SymbolHandle::invalid(),
        case_variant: None,
    };
    assert!(!effective_member_symbol(&program, indexed, &member).is_valid());
}

/// A match's value is whichever arm produces it: every arm must agree on one
/// exact position — the same stored reference — or the dispatch keeps no
/// position rather than letting one arm's leaf stand in for the others.
#[test]
fn match_expression_position_is_the_arms_common_position() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        pub data SchedulerHandle [copy] {}
        pub data Context [copy] { scheduler: SchedulerHandle; }
        machine pick(flag: bool, context: Context) -> Context {
            match flag { true -> context _ -> context }
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let dispatch = program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| matches!(node, ExpressionNode::Match(_)).then_some(handle))
        .expect("the match expression");
    assert_eq!(
        super::expression_type_symbol(&program, dispatch),
        Some(declared_data_symbol(&program, "Context"))
    );
    let member = TableMemberExpression {
        receiver: dispatch,
        member: Identifier::generated("scheduler"),
        member_symbol: SymbolHandle::invalid(),
        case_variant: None,
    };
    assert_eq!(
        effective_member_symbol(&program, dispatch, &member),
        declared_field(&program, "Context", "scheduler")
    );
}

#[test]
fn match_expression_position_requires_every_arm_to_agree() {
    let (mut program, context, scheduler_member, _) = stored_type_leaf_fixture();
    // A `SchedulerHandle` arm beside a `Context` arm names no common type:
    // the dispatch keeps no position rather than borrowing one arm's leaf.
    let divergent = program.expression_table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: context,
            source_span: source::SourceSpan::default(),
        },
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: scheduler_member,
            source_span: source::SourceSpan::default(),
        },
    ]);
    let divergent = program
        .expression_table
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject: context,
            arms: divergent,
        }));
    assert!(super::expression_type_symbol(&program, divergent).is_none());

    // An opaque arm — a scalar literal with no position — fails closed the
    // same way, as does a dispatch with no arms at all.
    let literal = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::zero(),
    ));
    let opaque = program.expression_table.insert_match_arms([
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: context,
            source_span: source::SourceSpan::default(),
        },
        TableMatchArm {
            pattern: MatchPattern::Wildcard,
            value: literal,
            source_span: source::SourceSpan::default(),
        },
    ]);
    let opaque = program
        .expression_table
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject: context,
            arms: opaque,
        }));
    assert!(super::expression_type_symbol(&program, opaque).is_none());
    let empty = program
        .expression_table
        .insert(ExpressionNode::Match(TableMatchExpression {
            subject: context,
            arms: arena::HandleSpan::empty(),
        }));
    assert!(super::expression_type_symbol(&program, empty).is_none());
}

/// The `item` member demanded on the `Second`-qualified `value.c` payload
/// projection inside `observe`'s transition — a member hop one level past
/// the case leaf, where the receiver itself carries no retained symbol.
fn second_arm_item_member(
    program: &typed_trees::TypedTrees,
) -> (ExpressionHandle, TableMemberExpression) {
    program
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| {
            let ExpressionNode::Member(member) = node else {
                return None;
            };
            if member.member.as_str() != "item" {
                return None;
            }
            let ExpressionNode::Member(receiver) =
                program.expression_table.expression(member.receiver)
            else {
                return None;
            };
            receiver
                .case_variant
                .as_ref()
                .is_some_and(|name| name.as_str() == "Second")
                .then(|| (handle, member.clone()))
        })
        .expect("value.c.item member expression")
}

/// The first spelling of a member chain's root: the head name of the `Name`
/// leaf the receiver walk ends at (`self`, `b`, `value`), so fixtures with
/// several same-named members can pick the chain rooted where they mean.
fn member_chain_root(
    program: &typed_trees::TypedTrees,
    mut cursor: ExpressionHandle,
) -> Option<String> {
    loop {
        match program.expression_table.expression(cursor) {
            ExpressionNode::Member(member) => cursor = member.receiver,
            ExpressionNode::Indexed(indexed) => cursor = indexed.collection,
            ExpressionNode::Borrow(borrow) => cursor = borrow.target,
            ExpressionNode::Name(path) => {
                return program
                    .expression_table
                    .name_path_members(path.members)
                    .first()
                    .map(|member| member.as_str().to_string());
            }
            _ => return None,
        }
    }
}

/// `self.grid.cells[0].item` on `machine Board::m(&mut self)` crosses three
/// member hops and an index hop before landing on `Cell::item`: the place
/// walk must carry the position through `Board::grid -> Grid`,
/// `Grid::cells -> [Cell; 8]`, and the element projection rather than
/// stopping at the first hop. `b.cells[0].item` roots the same chain at a
/// local bound to a member-valued record. Returns the machine state symbol
/// and each chain's `item` member handle.
fn nested_member_chain_fixture() -> (
    typed_trees::TypedTrees,
    SymbolHandle,
    ExpressionHandle,
    ExpressionHandle,
) {
    let source = r#"
        pub data Cell { item: u64; }
        pub data Grid { cells: [Cell; 8]; }
        pub data Board { grid: Grid; }
        machine Board::m(&mut self) {
            let a: u64 = self.grid.cells[0].item;
            let b: Grid = self.grid;
            let d: u64 = b.cells[0].item;
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let program =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Board::m")
        .expect("Board::m machine");
    let state_symbol = program.machine_states(machine)[0].symbol;
    let mut self_item = None;
    let mut local_item = None;
    for (handle, node) in program.expression_table.iter_expressions() {
        let ExpressionNode::Member(member) = node else {
            continue;
        };
        if member.member.as_str() != "item" {
            continue;
        }
        match member_chain_root(&program, member.receiver).as_deref() {
            Some("self") => self_item = Some(handle),
            Some("b") => local_item = Some(handle),
            _ => {}
        }
    }
    (
        program,
        state_symbol,
        self_item.expect("self.grid.cells[0].item member expression"),
        local_item.expect("b.cells[0].item member expression"),
    )
}

#[test]
fn member_resolution_continues_past_a_case_qualified_payload_leaf() {
    let (program, _) = fixture();
    let item_symbol = declared_field(&program, "Cell", "item");
    let (handle, member) = second_arm_item_member(&program);

    // `c.item`'s receiver is the `Second`-qualified payload projection
    // `value.c`: the member hop must continue at `Cell`'s declaration rather
    // than stopping at the case leaf.
    assert_eq!(
        effective_member_symbol(&program, member.receiver, &member),
        item_symbol
    );
    // The nested chain's declared type is `Cell::item`'s own `u64`, replayed
    // through the payload's declared `Cell` rather than minted from the
    // sum's other same-spelled fields.
    assert_eq!(
        super::expression_type_symbol(&program, handle),
        super::symbol_type_symbol(&program, item_symbol)
    );
}

#[test]
fn place_member_resolution_replays_member_hops_past_a_case_leaf() {
    let (mut program, _) = fixture();
    let payload = field(&program, "Outcome", "Second");
    let item_symbol = declared_field(&program, "Cell", "item");
    let second =
        facts::payload_variant_for_field(&program, payload).expect("Second::c's owning variant");
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .expect("observe machine");
    let state = &program.machine_states(machine)[0];
    let state_symbol = state.symbol;
    let value_parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "value")
        .expect("value parameter")
        .symbol;
    let (member_handle, member) = second_arm_item_member(&program);
    let statement_index = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .position(|statement| matches!(statement, StatementNode::Transition(_)))
        .expect("the member chain lives in the transition statement");

    // Neither `value.c` nor `c.item` carries a retained member symbol in this
    // lowering; stripping the name path's symbols leaves the place walk as
    // the only route that can name `item` — through `Case{Second}`, the
    // payload field's declared `Cell`, and `Cell::item`.
    strip_receiver_symbols(&mut program, member.receiver);
    assert!(
        !effective_member_symbol(&program, member.receiver, &member).is_valid(),
        "the stripped receiver must leave the expression route unanswered"
    );

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("a parameter-rooted member chain past a case leaf resolves");
    assert_eq!(place.root, facts::PlaceRoot::Symbol(value_parameter));
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::Case { variant: second },
            facts::PlaceSegment::Field { symbol: payload },
            facts::PlaceSegment::Field {
                symbol: item_symbol
            },
        ]
    );
}

#[test]
fn place_member_resolution_replays_a_nested_member_indexed_chain() {
    let (mut program, state_symbol, member_handle, _) = nested_member_chain_fixture();
    let state = crate::semantic_calls::find_state(&program, state_symbol).expect("Board::m state");
    let statement_index = member_statement_index(&program, state, member_handle);
    let self_parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .expect("self parameter")
        .symbol;
    let grid_symbol = declared_field(&program, "Board", "grid");
    let cells_symbol = declared_field(&program, "Grid", "cells");
    let item_symbol = declared_field(&program, "Cell", "item");
    // Strip every retained symbol along `self.grid.cells[0]` and on the
    // demanded member itself: `item` can then only be named by replaying the
    // declared types at each hop — `&mut self` to `Board::grid`,
    // `Grid::cells`, the `[Cell; 8]` element, and `Cell::item`.
    let receiver = match program.expression_table.expression(member_handle) {
        ExpressionNode::Member(member) => member.receiver,
        _ => unreachable!(),
    };
    strip_receiver_symbols(&mut program, receiver);
    let member = match program.expression_table.expression_mut(member_handle) {
        ExpressionNode::Member(member) => {
            member.member_symbol = SymbolHandle::invalid();
            member.clone()
        }
        _ => unreachable!(),
    };
    assert!(
        !effective_member_symbol(&program, member.receiver, &member).is_valid(),
        "the stripped member must leave the expression route unanswered"
    );

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("a self-rooted nested member/index chain resolves");
    assert_eq!(place.root, facts::PlaceRoot::Symbol(self_parameter));
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::Field {
                symbol: grid_symbol
            },
            facts::PlaceSegment::Field {
                symbol: cells_symbol
            },
            facts::PlaceSegment::FixedIndex { index: 0 },
            facts::PlaceSegment::Field {
                symbol: item_symbol
            },
        ]
    );

    // The same hops answer the declared-type question: the place's type is
    // `Cell::item`'s declared `u64`, reached through the attached datum's
    // `grid` rather than a same-shaped row.
    let reference = crate::flow::canonical_place_type_reference(
        &program,
        state_symbol,
        statement_index,
        &place,
    )
    .expect("the nested chain keeps a declared type");
    assert_eq!(
        Some(program.type_reference_table.type_symbol(reference)),
        super::symbol_type_symbol(&program, item_symbol)
    );
}

#[test]
fn place_member_resolution_replays_a_local_rooted_member_indexed_chain() {
    let (mut program, state_symbol, _, member_handle) = nested_member_chain_fixture();
    let state = crate::semantic_calls::find_state(&program, state_symbol).expect("Board::m state");
    let statement_index = member_statement_index(&program, state, member_handle);
    // `b` is bound by `let b: Grid = self.grid` before the `d` statement: the
    // contextual root scan finds the local and the walk resumes at `Grid`'s
    // declared type from the local's own row.
    let local_symbol = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local_data) if local_data.name.as_str() == "b" => {
                Some(local_data.symbol)
            }
            _ => None,
        })
        .expect("b local");
    let cells_symbol = declared_field(&program, "Grid", "cells");
    let item_symbol = declared_field(&program, "Cell", "item");
    let receiver = match program.expression_table.expression(member_handle) {
        ExpressionNode::Member(member) => member.receiver,
        _ => unreachable!(),
    };
    strip_receiver_symbols(&mut program, receiver);
    let member = match program.expression_table.expression_mut(member_handle) {
        ExpressionNode::Member(member) => {
            member.member_symbol = SymbolHandle::invalid();
            member.clone()
        }
        _ => unreachable!(),
    };
    assert!(
        !effective_member_symbol(&program, member.receiver, &member).is_valid(),
        "the stripped member must leave the expression route unanswered"
    );

    let place = crate::flow::contextual_canonical_place_from_expression(
        &program,
        state_symbol,
        statement_index,
        member_handle,
    )
    .expect("a local-rooted member/index chain resolves");
    assert_eq!(place.root, facts::PlaceRoot::Symbol(local_symbol));
    assert_eq!(
        place.segments,
        [
            facts::PlaceSegment::Field {
                symbol: cells_symbol
            },
            facts::PlaceSegment::FixedIndex { index: 0 },
            facts::PlaceSegment::Field {
                symbol: item_symbol
            },
        ]
    );
}
