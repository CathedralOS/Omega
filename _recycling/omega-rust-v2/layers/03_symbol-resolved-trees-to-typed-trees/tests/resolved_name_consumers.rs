//! L3 consumers of L2 binding decisions. Each test types a product that L2
//! accepted and inspects the typed fact that depends on the selected binding.
//! L2 refusals (transitive lookup, competing module paths, redefined builtin
//! types, a local read inside its own initializer) return no resolved trees,
//! so they have no L3 consumer.
mod support;
use omega_parser::NodeKind;
use omega_resolver::{Dependency, NameRole};
use omega_typer::{NodeSemantics, ValueType, type_stage};
use support::{check, resolve_packages};

#[test]
fn each_requester_types_the_declaration_its_own_alias_selected() {
    // The alias `dep` names lib for app and core for lib. Both expose `work` with different results.
    let files = [
        ("app", "main.omg", "machine run() -> u8 { dep\\work() }"),
        ("lib", "l.omg", "public machine work() -> u8 { 7 } machine ready() -> bool { dep\\work() }"),
        ("core", "c.omg", "public machine work() -> bool { true }"),
    ];
    let edges = [
        Dependency {
            requester: "app",
            alias: "dep",
            package: "lib",
        },
        Dependency {
            requester: "lib",
            alias: "dep",
            package: "core",
        },
    ];
    let resolved = resolve_packages(&files, &edges);
    let typed = type_stage(&resolved).unwrap();
    let mut selections: Vec<_> = typed
        .calls
        .iter()
        .filter(|call| call.machine.path == "work")
        .map(|call| {
            (
                call.site.package.as_str(),
                call.machine.package.as_str(),
                call.result.to_string(),
            )
        })
        .collect();
    selections.sort();
    spec_corpus::observe_handoff!(
        "dependency-authority/consumer",
        assert_eq!(selections, [("app", "lib", "u8".to_string()), ("lib", "core", "bool".to_string())])
    );
}

#[test]
fn a_nested_helper_types_the_package_constant_instead_of_the_enclosing_parameter() {
    // The enclosing runtime parameter is out of scope in the helper, so `port` reads the u16 constant.
    let source = "const port:u16=1; machine connect(port:u8)->u16 { default_port() machine default_port()->u16 { port } }";
    let resolved = resolve_packages(&[("game", "main.omg", source)], &[]);
    let typed = type_stage(&resolved).unwrap();
    let tree = typed.trees().next().unwrap();
    let read = source.rfind("port").unwrap();
    let node = tree
        .resolved()
        .occurrences()
        .find(|name| name.role == NameRole::Reference && name.span.get_start_offset() == read)
        .unwrap()
        .node
        .node;
    let mut facts: Vec<_> = tree
        .semantics_at(node)
        .filter_map(|record| match record {
            NodeSemantics::Constant(declaration) => Some(format!("constant {}", declaration.path)),
            NodeSemantics::Expression(expression) => Some(format!("expression {}", expression.ty)),
            _ => None,
        })
        .collect();
    facts.sort();
    facts.dedup();
    spec_corpus::observe_handoff!(
        "lexical-scope/consumer",
        assert_eq!(facts, ["constant port", "expression u16"])
    );
}

#[test]
fn a_case_payload_is_typed_with_the_outer_type_its_bare_spelling_names() {
    let source = "data Circle {r:u8;} data Shape {case Circle(c:Circle); case Empty;} machine wrap(v:Circle) -> Shape { Shape::Circle { c: v } }";
    let resolved = resolve_packages(&[("app", "main.omg", source)], &[]);
    let typed = type_stage(&resolved).unwrap();
    let outer = typed
        .resolved()
        .declarations
        .iter()
        .find(|id| id.kind == NodeKind::Data && id.path == "Circle")
        .unwrap();
    let (_, payload) = typed
        .record_fields
        .iter()
        .find(|(id, _)| id.kind == NodeKind::Case && id.path == "Shape::Circle")
        .unwrap();
    let fields: Vec<_> = payload.iter().map(|field| (field.name(), field.ty.clone())).collect();
    spec_corpus::observe_handoff!(
        "type-and-value-names/consumer",
        assert_eq!(fields, [("c", ValueType::Nominal(Box::new(outer.clone())))])
    );
}

#[test]
fn both_written_arguments_of_one_base_close_to_one_application() {
    let source = "data Buffer<const N: u32> {} machine f(x: Buffer<8>, y: Buffer<4 + 4>) {} machine g(y: Buffer<4 + 4>) -> Buffer<8> { y }";
    let stage = check(&[("main.omg", source)]).unwrap();
    let f = stage.signatures.iter().find(|s| s.machine.path == "f").unwrap();
    let [(_, written), (_, computed)] = f.parameters.as_slice() else {
        panic!("expected two parameters: {:?}", f.parameters)
    };
    spec_corpus::observe_handoff!("preserved-generic-arguments/consumer", assert_eq!(written, computed));
    assert_eq!(written.to_string(), "game\\Buffer<8>");
}
