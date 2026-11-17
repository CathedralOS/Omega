//! `x == S::C` is a tag test only when `S` is a payload-free sum; `x in S::C`
//! is a tag test on any sum.
//!
//! `wiki/spec/language/conformances.md`: "Primitives and payload-less sums
//! acquire equality implicitly. Records and payload-bearing sums require an
//! explicit named conformance. ... Domain membership uses its own tag/domain
//! rules, not `Equatable`." `data_and_literals.md` forbids "silently changing
//! tag equality into field equality". The typed-tree stage replaces a `==` it
//! can type on such data with its selected `Equatable` call; it cannot type a
//! `match` result, so `(match ...) == Cmd::Stop` used to reach Terminal Psi
//! reading as a bare `==` and was read as a tag test. These twins pin that the
//! comparison is now refused there, while membership and payload-free
//! equality spelled the same way still compile.
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/joined_product_execution.rs"]
mod joined_product_execution;

use terminal_psi_to_terminal_psi::{RouteVerdict, judge};
use typed_trees_to_terminal_psi::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const DATA: &str = "data Cmd [copy] {\n    case Stop;\n    case Move(dx: i32);\n}\n\
                    data Kind [copy] {\n    case A;\n    case B;\n}\n";

/// The route's verdict on `source` beside the two sums and an empty `main`.
fn verdict(name: &str, source: &str) -> RouteVerdict {
    let source = format!("{DATA}{source}\nmachine main() {{}}\n");
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let frontier = unbuilt_frontier::staged_frontier(name, &source);
    frontier_verdict(name, &frontier)
}

fn frontier_verdict(name: &str, frontier: &compiler::TypedFrontier) -> RouteVerdict {
    judge(elaborate(
        &frontier.typed,
        Compilation {
            calling: frontier
                .normalization
                .calling
                .borrow(&frontier.typed)
                .selection(),
            providers: frontier.provider_selections(),
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
            supplies: &[],
        },
    ))
    .unwrap_or_else(|defect| panic!("{name}: the route must reach a verdict: {defect:?}"))
}

#[test]
fn membership_and_payload_free_equality_are_tag_tests() {
    let mut wrong = Vec::new();
    for (name, source) in [
        // Membership never asks for `Equatable`, even on a payload-bearing
        // sum and in value position.
        (
            "membership_value_position",
            "machine stopped(c: Cmd) -> bool { c in Cmd::Stop }",
        ),
        (
            "membership_negated",
            "machine moving(c: Cmd) -> bool { (c in Cmd::Stop) == false }",
        ),
        // A payload-free sum's equality is its tag, however the operand is
        // spelled.
        (
            "payload_free_named",
            "machine is_a(k: Kind) -> bool { k == Kind::A }",
        ),
        (
            "payload_free_variables",
            "machine same(left: Kind, right: Kind) -> bool { left == right }",
        ),
        (
            "payload_free_variables_unequal",
            "machine different(left: Kind, right: Kind) -> bool { left != right }",
        ),
        (
            "payload_free_match_operand",
            "machine is_a(flag: bool) -> bool {\n\
                 (match flag { true -> Kind::A, false -> Kind::B }) == Kind::A\n\
             }",
        ),
    ] {
        let found = verdict(name, source);
        if !matches!(found, RouteVerdict::Accepted(_)) {
            wrong.push(format!("{name}: {found:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn equality_against_a_case_of_a_payload_bearing_sum_is_not_a_tag_test() {
    for (name, operator) in [("equal", "=="), ("not_equal", "!=")] {
        let source = format!(
            "machine stopped(flag: bool) -> bool {{\n    \
                 (match flag {{ true -> Cmd::Stop, false -> Cmd::Stop }}) {operator} Cmd::Stop\n\
             }}"
        );
        let found = verdict(name, &source);
        let RouteVerdict::FallBack(refusal) = &found else {
            panic!("{name}: `{operator}` on Cmd must not compile as a tag test: {found:?}");
        };
        assert!(
            refusal.detail.contains("no selected Equatable"),
            "{name}: {refusal:?}"
        );
    }
}

#[test]
fn intrinsic_enum_equality_retains_payload_and_type_refusals() {
    for (name, source, reason) in [
        (
            "payload_bearing_variables",
            "machine same(flag: bool, left: Cmd, right: Cmd) -> bool {\n\
                 (match flag { true -> left, false -> left }) == right\n\
             }",
            "no selected Equatable",
        ),
        (
            "foreign_enum_variables",
            "data Other [copy] { case A; case B; }\n\
             machine same(flag: bool, left: Kind, right: Other) -> bool {\n\
                 (match flag { true -> left, false -> left }) == right\n\
             }",
            "equality operand types differ",
        ),
        (
            "qualified_enum_variables",
            "domain Kind::Marked;\n\
             machine same(flag: bool, left: Kind in Marked, right: Kind in Marked) -> bool {\n\
                 (match flag { true -> left, false -> left }) == right\n\
             }",
            "unqualified operands",
        ),
    ] {
        let found = verdict(name, source);
        assert!(
            !matches!(found, RouteVerdict::Accepted(_)),
            "{name}: {found:?}"
        );
        assert!(format!("{found:?}").contains(reason), "{name}: {found:?}");
    }
}

#[test]
fn a_lost_selected_enum_comparison_cannot_become_intrinsic() {
    use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
    use typed_trees::statement::StatementNode;

    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = "data Key [copy] { case A; case B; }\n\
                  machine == key_equal(left: Key, right: Key) -> bool { false }\n\
                  machine same(left: Key, right: Key) -> bool { left == right }\n\
                  machine main() {}";
    let mut frontier = unbuilt_frontier::staged_frontier("selected_enum_comparison", source);
    assert!(matches!(
        frontier_verdict("selected_enum_comparison", &frontier),
        RouteVerdict::Accepted(_)
    ));
    let mut typed = frontier.typed.clone().into_inner();
    let target = unbuilt_frontier::program_machine(&typed, "key_equal").unwrap();
    let (handle, call) = typed
        .expression_table
        .iter_expressions()
        .find_map(|(handle, node)| match node {
            ExpressionNode::Call(call) if call.target_symbol == target => {
                Some((handle, call.clone()))
            }
            _ => None,
        })
        .expect("the authored operator must first become its selected call");
    let [left, right] = *typed.expression_table.expression_handles(call.arguments) else {
        panic!("equality has exactly two arguments");
    };
    let span = typed.expression_table.source_span(handle);
    // Model a broken producer that drops both the selected call and its
    // occurrence attachment while retaining the same operand identities.
    let bare = typed
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            right,
            operator: BinaryOperator::Equal,
            policy: numerics::arithmetic::ArithmeticDomain::Exact,
        }));
    typed.expression_table.set_source_span(bare, span);
    assert_eq!(
        typed
            .expression_table
            .authored_selection_occurrences(bare)
            .len(),
        0
    );
    let same = unbuilt_frontier::program_machine(&typed, "same").unwrap();
    let body = typed.machine_states(typed.machine_by_symbol(same).unwrap())[0].statement_nodes;
    let statement = typed
        .statement_table
        .iter_statements(body)
        .find_map(|(statement, node)| {
            matches!(node, StatementNode::Expression(value) if *value == handle)
                .then_some(statement)
        })
        .expect("the comparison is the machine's result expression");
    *typed.statement_table.statement_mut(statement) = StatementNode::Expression(bare);
    frontier.typed = typed.into();
    let found = frontier_verdict("lost_selected_enum_comparison", &frontier);
    assert!(!matches!(found, RouteVerdict::Accepted(_)), "{found:?}");
    assert!(
        format!("{found:?}").contains("selected operator call"),
        "{found:?}"
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn enum_comparison_observes_each_operand_once_in_native_and_psi() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = fixture_package_inputs::repo_root()
        .join("tests/fixtures/run/equality/intrinsic_enum_comparison/main.omg");
    let discovered = fixture_package_inputs::discover_fixture(&fixture, Default::default())
        .unwrap_or_else(|errors| panic!("enum fixture discovery: {errors:#?}"));
    let product = compiler::CompilationProduct::Native(products::NativeProductRequest {
        installation_assumptions: Vec::new(),
        retain_native_inputs: false,
        retain_native_stage_inputs: false,
        image: Default::default(),
        private_entry_ram: Vec::new(),
        terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: Default::default(),
    });
    let mut prepared = fixture_package_inputs::review_fixture(
        discovered,
        &[target::TargetProfile::LinuxX64],
        product,
    )
    .unwrap_or_else(|errors| panic!("enum fixture review: {errors:#?}"));
    let output = fixture_package_inputs::produce_fixture(&mut prepared, Some("linux_x86_64"))
        .unwrap_or_else(|errors| panic!("enum fixture production: {errors:#?}"));
    let report = orchestration::complete_product(output)
        .and_then(compiler::CompilationOutput::into_report)
        .expect("complete the enum comparison native product");
    let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
        "intrinsic-enum-comparison",
        false,
    )
    .unwrap();
    joined_product_execution::execute_report(report, &directory.path().join("native"));
}
