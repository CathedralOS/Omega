//! Calls inside a target sibling's body bind to the same target's siblings,
//! both for a free helper only that target declares and for an attached
//! method reached through `self`.
use crate::{ResolutionRequest, resolve};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolHandle;

fn resolve_source(source: &str) -> SymbolResolvedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    resolve(ResolutionRequest::new(syntax)).expect("siblings resolve")
}

fn resolve_sources(sources: &[(&str, &str)]) -> SymbolResolvedTrees {
    let mut source_map = source::SourceMap::default();
    let mut syntax = syntax_trees::SyntaxTrees::default();
    for (path, text) in sources {
        let source_id = source_map.add((*path).into(), (*text).into()).source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokens");
        tokens_to_syntax_trees::parse(&mut syntax, source_id, &tokens).expect("syntax");
    }
    // Model the caller's canonical-body selection, retaining authored_target
    // so resolution can bind the exact family before retargeting its calls.
    for handle in syntax.root_item_handles().to_vec() {
        let syntax_trees::item::Item::Machine(machine) = syntax.root_item(handle) else {
            continue;
        };
        if machine
            .target
            .as_ref()
            .is_some_and(|target| target.as_str() == "base_target")
        {
            let mut machine = machine.clone();
            machine.target = None;
            syntax
                .items
                .replace_item(handle, syntax_trees::item::Item::Machine(machine));
        }
    }
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(source_map)),
        top_level_bindings: Vec::new(),
    })
    .expect("module-qualified calls resolve")
}

fn machine_symbol(program: &SymbolResolvedTrees, name: &str) -> SymbolHandle {
    program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("machine {name} is lowered"))
        .symbol
}

/// The callee machine of the first `let` initializer call in `machine`'s
/// first state.
fn first_local_call_callee(program: &SymbolResolvedTrees, machine: &str) -> SymbolHandle {
    let target = first_local_call_target(program, machine);
    assert!(
        target.is_valid(),
        "local initializer call inside {machine} stays unbound"
    );
    program.symbols.get(target).parent
}

fn first_local_call_target(program: &SymbolResolvedTrees, machine: &str) -> SymbolHandle {
    let record = program
        .machines
        .iter()
        .find(|candidate| candidate.name.as_str() == machine)
        .unwrap_or_else(|| panic!("machine {machine} is lowered"));
    let state = program.machine_state(program.machine_state_handles(record.states)[0]);
    let statements = state.statement_nodes;
    for offset in 0..statements.count() {
        let handle = arena::Handle::from_parts(
            statements.start().arena_index() + offset,
            statements.start().generation(),
        );
        let symbol_resolved_trees::statement::StatementNode::LocalData(local) =
            program.tables.bodies.statements.statement(handle)
        else {
            continue;
        };
        let symbol_resolved_trees::expression::ExpressionNode::Call(call) = program
            .tables
            .bodies
            .expressions
            .expression(local.initial_value)
        else {
            continue;
        };
        return call.target_symbol;
    }
    panic!("{machine} has no local initializer call");
}

const SOURCE: &str = "data Helper { seed: i32; }
demo_target machine free_helper(clock_id: u32) -> u64 { transition { _ -> 5 } }
demo_target machine Helper::perf(&mut self) -> u64 {
    let v: u64 = free_helper(6);
    transition { _ -> v }
}
demo_target machine Helper::internal(&self) -> i32 { transition { _ -> 63 } }
demo_target machine Helper::entry_point(&self) -> i32 {
    let inner: i32 = self.internal();
    transition { _ -> inner }
}";

#[test]
fn via_calls_and_nested_arguments_bind_to_the_declarations_authored_target() {
    let program = resolve_sources(&[(
        "bindings.omg",
        "data Foreign {} data Provider {}
         pub boundary requirement Foreign::value() -> u64;
         base_target machine binding() -> u64 { 1 }
         demo_target machine binding() -> u64 { 2 }
         machine wrap(value: u64) -> u64 { value }
         base_target machine Provider::leaf() -> u64
             satisfies Foreign::value via wrap(binding());
         demo_target machine Provider::leaf() -> u64
             satisfies Foreign::value via wrap(binding());",
    )]);
    let leaves = program
        .machines
        .iter()
        .filter(|machine| machine.name.as_str() == "Provider::leaf")
        .collect::<Vec<_>>();
    assert_eq!(leaves.len(), 2, "both target-authored leaves are checked");
    for machine in leaves {
        let conformance = &program.machine_trait_conformances(machine.satisfies)[0];
        let expressions = &program.tables.bodies.expressions;
        let symbol_resolved_trees::expression::ExpressionNode::Call(wrapper) =
            expressions.expression(conformance.via_expression)
        else {
            panic!("via retains its wrapper call");
        };
        assert_eq!(
            program.symbols.get(wrapper.target_symbol).parent,
            machine_symbol(&program, "wrap"),
            "an ordinary helper is not a member of the target family"
        );
        let argument = expressions.expression_handles(wrapper.arguments)[0];
        let symbol_resolved_trees::expression::ExpressionNode::Call(binding) =
            expressions.expression(argument)
        else {
            panic!("via retains its nested binding call");
        };
        let expected = program
            .machines
            .iter()
            .find(|candidate| {
                candidate.name.as_str() == "binding" && candidate.target == machine.target
            })
            .expect("each authored target has a binding producer");
        assert_eq!(
            program.symbols.get(binding.target_symbol).parent,
            expected.symbol
        );
    }
}

#[test]
fn canonical_body_uses_its_authored_target_not_the_callee_familys_canonical_target() {
    let text = "base_target machine helper() -> u64 { 1 }
        demo_target machine helper() -> u64 { 2 }
        demo_target machine caller() -> u64 { let value: u64 = helper(); value }";
    let tokens = Lexer::new(text).tokenize().expect("tokens");
    let mut syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    for handle in syntax.root_item_handles().to_vec() {
        let syntax_trees::item::Item::Machine(machine) = syntax.root_item(handle) else {
            continue;
        };
        if machine.name.as_str() == "caller"
            || machine
                .target
                .as_ref()
                .is_some_and(|target| target.as_str() == "base_target")
        {
            let mut machine = machine.clone();
            machine.target = None;
            syntax
                .items
                .replace_item(handle, syntax_trees::item::Item::Machine(machine));
        }
    }
    let program = resolve(ResolutionRequest::new(syntax)).expect("resolve target-authored callers");
    let target_helper = program
        .machines
        .iter()
        .find(|machine| {
            machine.name.as_str() == "helper"
                && machine
                    .target
                    .as_ref()
                    .is_some_and(|target| target.as_str() == "demo_target")
        })
        .expect("demo helper");
    assert_eq!(
        first_local_call_callee(&program, "caller"),
        target_helper.symbol
    );
}

#[test]
fn sibling_bodies_lower_with_their_target_and_authored_name() {
    let program = resolve_source(SOURCE);
    let targets = program
        .machines
        .iter()
        .map(|machine| {
            (
                machine.name.as_str().to_owned(),
                machine
                    .target
                    .as_ref()
                    .map(|target| target.as_str().to_owned()),
                program.symbols.name(machine.symbol).to_owned(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        targets,
        [
            (
                "free_helper".to_owned(),
                Some("demo_target".to_owned()),
                "free_helper::demo_target".to_owned()
            ),
            (
                "Helper::perf".to_owned(),
                Some("demo_target".to_owned()),
                "Helper::perf::demo_target".to_owned()
            ),
            (
                "Helper::internal".to_owned(),
                Some("demo_target".to_owned()),
                "Helper::internal::demo_target".to_owned()
            ),
            (
                "Helper::entry_point".to_owned(),
                Some("demo_target".to_owned()),
                "Helper::entry_point::demo_target".to_owned()
            ),
        ]
    );
}

#[test]
fn free_call_inside_a_sibling_binds_to_the_same_targets_sibling() {
    let program = resolve_source(SOURCE);
    assert_eq!(
        first_local_call_callee(&program, "Helper::perf"),
        machine_symbol(&program, "free_helper")
    );
}

#[test]
fn self_method_call_inside_a_sibling_binds_to_the_same_targets_sibling() {
    let program = resolve_source(SOURCE);
    assert_eq!(
        first_local_call_callee(&program, "Helper::entry_point"),
        machine_symbol(&program, "Helper::internal")
    );
}

#[test]
fn resolved_call_to_another_module_is_not_replaced_by_a_same_named_sibling() {
    let program = resolve_sources(&[
        (
            "ordinary.omg",
            "module ordinary; pub machine helper() -> u64 { 5 }",
        ),
        (
            "platform.omg",
            "module platform;
             demo_target machine helper() -> u64 { 9 }
             demo_target machine caller() -> u64 {
                 let value: u64 = ordinary::helper();
                 value
             }",
        ),
    ]);
    let ordinary_helper = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "helper" && machine.target.is_none())
        .expect("ordinary helper")
        .symbol;
    assert_eq!(first_local_call_callee(&program, "caller"), ordinary_helper);
}

#[test]
fn resolved_call_switches_only_within_the_selected_modules_target_family() {
    let program = resolve_sources(&[
        (
            "other.omg",
            "module other;
             base_target machine helper() -> u64 { 9 }
             demo_target machine helper() -> u64 { 11 }",
        ),
        (
            "requested.omg",
            "module requested;
             pub base_target machine helper() -> u64 { 5 }
             pub demo_target machine helper() -> u64 { 6 }",
        ),
        (
            "caller.omg",
            "demo_target machine caller() -> u64 {
                 let value: u64 = requested::helper();
                 value
             }",
        ),
    ]);
    let requested_sibling = program
        .machines
        .iter()
        .find(|machine| {
            machine.name.source_span().source_id == source::SourceId(1)
                && machine
                    .target
                    .as_ref()
                    .is_some_and(|target| target.as_str() == "demo_target")
        })
        .expect("requested sibling");
    assert_eq!(
        first_local_call_callee(&program, "caller"),
        requested_sibling.symbol
    );
    let canonical = program
        .machines
        .iter()
        .find(|machine| {
            machine.name.source_span().source_id == source::SourceId(1) && machine.target.is_none()
        })
        .expect("requested canonical body");
    assert_eq!(requested_sibling.target_family, canonical.symbol);
}

#[test]
fn unmoduled_target_families_keep_their_package_identity() {
    let source = "base_target machine helper() -> u64 { 5 }
                  demo_target machine helper() -> u64 { 6 }";
    let program = resolve_sources(&[
        ("first/helpers.omg", source),
        ("second/helpers.omg", source),
    ]);
    for source_id in [source::SourceId(0), source::SourceId(1)] {
        let canonical = program
            .machines
            .iter()
            .find(|machine| {
                machine.name.source_span().source_id == source_id && machine.target.is_none()
            })
            .expect("package canonical body");
        for machine in program
            .machines
            .iter()
            .filter(|machine| machine.name.source_span().source_id == source_id)
        {
            assert_eq!(machine.target_family, canonical.symbol);
        }
    }
}

#[test]
fn unresolved_sibling_call_uses_its_own_module_not_the_first_matching_name() {
    let program = resolve_sources(&[
        (
            "other.omg",
            "module other; demo_target machine helper() -> u64 { 9 }",
        ),
        (
            "local.omg",
            "module local;
             demo_target machine helper() -> u64 { 5 }
             demo_target machine caller() -> u64 {
                 let value: u64 = helper();
                 value
             }",
        ),
    ]);
    let local_helper = program
        .machines
        .iter()
        .find(|machine| {
            machine.name.as_str() == "helper"
                && machine.name.source_span().source_id == source::SourceId(1)
        })
        .expect("local helper");
    assert_eq!(
        first_local_call_callee(&program, "caller"),
        local_helper.symbol
    );
}

#[test]
fn unresolved_sibling_call_cannot_import_a_helper_by_matching_its_target() {
    let program = resolve_sources(&[
        (
            "other.omg",
            "module other; demo_target machine helper() -> u64 { 9 }",
        ),
        (
            "local.omg",
            "module local;
             demo_target machine caller() -> u64 {
                 let value: u64 = helper();
                 value
             }",
        ),
    ]);
    assert!(!first_local_call_target(&program, "caller").is_valid());
}
