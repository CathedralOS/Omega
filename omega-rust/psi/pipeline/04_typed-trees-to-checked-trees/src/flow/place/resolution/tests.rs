//! Case qualification selects declaration identity, not the first matching spelling.
use super::effective_member_symbol;
use crate::tests::front_end::{checked_program, typed_program};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::name::Identifier;
use symbols::SymbolHandle;
use typed_trees::expression::TableMemberExpression;
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
    let program = typed_program(source);
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
    checked_program(source);
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
    let program = typed_program(source);
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
