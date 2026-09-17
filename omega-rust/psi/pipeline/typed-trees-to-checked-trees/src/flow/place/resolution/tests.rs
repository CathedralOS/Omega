//! Case qualification selects declaration identity, not the first matching spelling.
use super::effective_member_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::name::Identifier;
use symbols::SymbolHandle;
use typed_trees::expression::TableMemberExpression;

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
