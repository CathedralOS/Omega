//! Case qualification selects declaration identity, not the first matching spelling.
use super::effective_member_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::name::Identifier;
use symbols::SymbolHandle;
use typed_trees::expression::TableMemberExpression;
use typed_trees::statement::StatementNode;

fn fixture() -> (typed_trees::TypedTrees, TableMemberExpression) {
    let source = r#"
        data Outcome { case First(count: u64); case Second(count: u64); }
        data Other { case Second(count: u64); }
        machine observe(value: Outcome) {
            transition value {
                Outcome::Second { count } -> done(count)
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
    crate::lower_typed_trees(program)
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
