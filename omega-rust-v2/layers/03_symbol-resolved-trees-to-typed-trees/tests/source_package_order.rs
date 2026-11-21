#[cfg(feature = "diagnostic-origins")]
#[path = "support/diagnostic_origin.rs"]
mod diagnostic_origin;

use omega_lexer::SourceSpan;
use omega_resolver::{ParsedSource, StageSource};
use omega_typer::{NodeSemantics, TypeError, TypedTrees, ValueType, resolve_source_package, type_source_package};

const FILE_ORDERS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
const DRAW: &str = include_str!("fixtures/overloaded_calls/draw.omg");
const RENDER: &str = include_str!("fixtures/overloaded_calls/render.omg");
const WRONG_RENDER: &str = include_str!("fixtures/overloaded_calls/wrong_result.omg");
const READ: &str = "machine read(items: @[bool], i: u64) -> bool { items[i] }";
const WRONG_READ: &str = "machine read(items: @[bool], i: u64) -> u8 { items[i] }";

fn type_package<'text>(
    render: &'text str,
    read: &'text str,
    order: [usize; 3],
) -> Result<TypedTrees<'text>, TypeError> {
    let parsed = [("draw.omg", DRAW), ("render.omg", render), ("view.omg", read)].map(|(path, text)| {
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        (path, text, tokens, tree)
    });
    let sources = order.map(|index| {
        let (path, text, tokens, tree) = &parsed[index];
        StageSource {
            package: "app",
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        }
    });
    let resolved = resolve_source_package(&sources).expect("all three files resolve, including the negative twins");
    drop(parsed);
    let result = type_source_package(&resolved);
    #[cfg(feature = "diagnostic-origins")]
    if let Err(error) = &result {
        diagnostic_origin::report_return(error, "", None);
    }
    result
}

#[test]
fn source_package_preserves_selections_and_obligations_in_every_file_order() {
    let expected = type_package(RENDER, READ, FILE_ORDERS[0]).unwrap();
    let [draw] = expected.calls.as_slice() else {
        panic!("expected one source call");
    };
    assert_eq!(draw.machine.path, "draw");
    assert_eq!(draw.machine.operands, ["bool"]);
    assert_eq!(draw.result, ValueType::Bool);
    let [read] = expected.collection_accesses.as_slice() else {
        panic!("expected one indexed read");
    };
    assert_eq!(read.machine.path, "indexing\\read");
    assert!(
        expected
            .obligations
            .iter()
            .any(|obligation| obligation.site == read.site
                && matches!(&obligation.kind, omega_typer::ObligationKind::CallPrecondition(machine) if *machine == read.machine))
    );

    for order in FILE_ORDERS {
        assert_eq!(type_package(RENDER, READ, order).unwrap(), expected, "order {order:?}");
    }
}

#[test]
fn source_package_reports_the_same_first_error_in_every_file_order() {
    for (render, read, file, text, expression, message) in [
        (WRONG_RENDER, READ, "render.omg", WRONG_RENDER, "draw(flag)", "expected u32, found bool"),
        (RENDER, WRONG_READ, "view.omg", WRONG_READ, "items[i]", "expected u8, found bool"),
        (WRONG_RENDER, WRONG_READ, "render.omg", WRONG_RENDER, "draw(flag)", "expected u32, found bool"),
    ] {
        let start = text.find(expression).unwrap();
        let expected = TypeError::new("app", file, SourceSpan::new(start, start + expression.len()), message);
        for order in FILE_ORDERS {
            assert_eq!(type_package(render, read, order).unwrap_err(), expected, "order {order:?}");
        }
    }
}

#[test]
fn a_consumer_walks_both_typed_arms_without_parser_inputs() {
    use omega_parser::NodeKind;
    use omega_typer::ObligationKind;
    let source = "machine choose(b:bool,items:@[bool])->bool {match b {true->draw(b),false->items[0],}}";
    let typed = type_package(source, READ, FILE_ORDERS[2]).unwrap();
    let tree = typed
        .trees()
        .find(|t| t.resolved().file.path == "render.omg")
        .unwrap();
    let file = tree.resolved().file;
    // The tail and the match occupy the same bytes, but only the match is typed.
    let tail = file
        .nodes
        .iter()
        .position(|node| node.kind == NodeKind::Tail)
        .unwrap();
    let match_node = file.nodes[tail].children[0];
    assert_eq!(file.span(tail), file.span(match_node));
    assert_eq!(tree.semantics_at(tail).count(), 0);
    assert!(
        tree.semantics_at(match_node)
            .any(|fact| matches!(fact, NodeSemantics::Expression(e) if e.ty == ValueType::Bool))
    );
    let dispatch = file
        .nodes
        .iter()
        .find(|node| node.kind == NodeKind::Match)
        .unwrap();
    let [_, first_arm, second_arm] = *dispatch.children.as_slice() else {
        panic!("expected two arms")
    };
    let first = file.nodes[first_arm].children[1];
    let second = file.nodes[second_arm].children[1];
    assert_eq!(file.nodes[first].kind, NodeKind::Call);
    assert_eq!(file.nodes[second].kind, NodeKind::Index);
    let callee = file.nodes[first].children[0];
    assert!(matches!(tree.resolved().symbol(callee), Some(omega_resolver::Symbol::Candidates(ids)) if ids.len() == 2));
    let selected = tree
        .semantics_at(first)
        .find_map(|fact| match fact {
            NodeSemantics::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    spec_corpus::observe_handoff!("node-identity-and-call-operands/producer", assert_eq!(selected.machine.operands, ["bool"]));
    assert_eq!(
        file.span(selected.operands[0].node.node)
            .slice(source.as_bytes()),
        Some(b"b".as_slice())
    );
    assert_eq!(selected.site.span.slice(source.as_bytes()), Some(b"draw(b)".as_slice()));
    assert!(
        tree.semantics_at(second)
            .any(|fact| matches!(fact, NodeSemantics::Expression(e) if e.ty == ValueType::Bool))
    );
    assert!(tree.semantics_at(second).any(|fact| matches!(fact,
        NodeSemantics::Obligation(o) if o.node == file.node_id(second)
            && matches!(&o.kind, ObligationKind::CallPrecondition(id) if id.path == "indexing\\read")
    )));
    let view = typed
        .trees()
        .find(|t| t.resolved().file.path == "view.omg")
        .unwrap();
    let file = view.resolved().file;
    let tail = file
        .nodes
        .iter()
        .position(|n| n.kind == NodeKind::Tail)
        .unwrap();
    let index = file.nodes[tail].children[0];
    assert_eq!(file.nodes[index].kind, NodeKind::Index);
    assert_eq!(file.span(tail), file.span(index));
    let read_precondition = |fact| {
        matches!(fact, NodeSemantics::Obligation(o)
        if matches!(&o.kind, ObligationKind::CallPrecondition(id) if id.path == "indexing\\read"))
    };
    spec_corpus::observe_handoff!("obligations-and-assembly-at-the-producing-node/producer", assert!(view.semantics_at(index).any(read_precondition)));
    assert!(!view.semantics_at(tail).any(read_precondition));
}

#[test]
fn imported_library_obligations_belong_to_the_library_tree() {
    use omega_typer::ObligationKind;
    let typed = type_package(RENDER, READ, FILE_ORDERS[0]).unwrap();
    let library = typed.library.unwrap();
    let obligation = typed
        .obligations
        .iter()
        .find(|obligation| matches!(obligation.kind, ObligationKind::VerifyLibraryImplementation(_)))
        .unwrap();
    let defining_tree = library.trees().nth(obligation.node.file).unwrap();
    assert!(
        defining_tree
            .semantics_at(obligation.node.node)
            .any(|fact| matches!(fact, NodeSemantics::Obligation(found) if found == obligation))
    );
    for tree in typed.trees() {
        let file = tree.resolved().file;
        for node in 0..file.nodes.len() {
            for fact in tree.semantics_at(node) {
                if let NodeSemantics::Obligation(found) = fact {
                    assert_eq!((&*found.site.package, &*found.site.file), (file.package, file.path));
                    assert_eq!(found.node, file.node_id(node));
                }
            }
        }
    }
}

#[test]
fn typed_selection_nodes_keep_the_machine_and_the_applied_requirement() {
    let texts = [
        ("a.omg", "const first=(left satisfies Slot<u8>::run);"),
        ("b.omg", "const second=(right satisfies Slot<bool>::run);"),
        (
            "schema.omg",
            "trait Slot<T>{machine run(x:T)->T;} machine left(x:u8)->u8{x} machine right(x:bool)->bool{x}",
        ),
    ];
    let parsed = texts.map(|(path, text)| {
        let tokens = omega_lexer::lex(text.as_bytes()).unwrap();
        let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap();
        (path, text, tokens, tree)
    });
    let sources = parsed.each_ref().map(|(path, text, tokens, tree)| StageSource {
        package: "app",
        source: ParsedSource {
            logical_path: path,
            text,
            tokens,
            tree,
        },
    });
    let resolved = resolve_source_package(&sources).unwrap();
    drop(parsed);
    let typed = type_source_package(&resolved).unwrap();
    for (path, machine, ty) in [
        ("a.omg", "left", ValueType::Integer { signed: false, width: Some(8) }),
        ("b.omg", "right", ValueType::Bool),
    ] {
        let tree = typed.trees().find(|t| t.resolved().file.path == path).unwrap();
        let file = tree.resolved().file;
        let node = file
            .nodes
            .iter()
            .position(|n| n.kind == omega_parser::NodeKind::Selection)
            .unwrap();
        let mut bindings = tree.semantics_at(node).filter_map(|entry| {
            if let NodeSemantics::RequirementBinding(row) = entry {
                Some(row)
            } else {
                None
            }
        });
        let row = bindings.next().unwrap();
        assert!(bindings.next().is_none());
        assert_eq!(row.node, file.node_id(node));
        assert_eq!(row.site.span, file.span(node));
        assert_eq!(row.implementation.path, machine);
        assert_eq!(row.requirement.declaration.path, "Slot::run");
        assert_eq!(row.requirement.trait_arguments, [ty]);
    }
}

#[test]
fn field_projections_keep_the_selected_declaration_and_its_applied_type() {
    let source = "data Box<T>{value:T;} data Outer{value:Box<bool>;} machine read_box(x:Outer)->bool{(x.value).value}";
    let typed = type_package(source, READ, FILE_ORDERS[0]).unwrap();
    let tree = typed
        .trees()
        .find(|tree| tree.resolved().file.path == "render.omg")
        .unwrap();
    let file = tree.resolved().file;
    for (expr, owner_path, expected) in [
        ("x.value", "Outer", "app\\Box<bool>"),
        ("(x.value).value", "Box", "bool"),
    ] {
        let n = file
            .nodes
            .iter()
            .enumerate()
            .find(|(i, node)| node.kind == omega_parser::NodeKind::FieldAccess && file.span(*i).slice(source.as_bytes()) == Some(expr.as_bytes()))
            .unwrap()
            .0;
        let declared = tree
            .semantics_at(n)
            .find_map(|fact| match fact {
                NodeSemantics::Field(id) => Some(id),
                _ => None,
            })
            .unwrap();
        let (owner, fields) = typed
            .record_fields
            .iter()
            .find(|(id, _)| id.package == "app" && id.path == owner_path)
            .unwrap();
        assert_eq!(declared, &fields[0].declaration);
        assert_eq!(
            fields[0].site.span.slice(source.as_bytes()),
            Some(if owner_path == "Box" {
                b"value:T;".as_slice()
            } else {
                b"value:Box<bool>;".as_slice()
            })
        );
        assert!(
            tree.semantics_at(n)
                .any(|fact| matches!(fact,NodeSemantics::Expression(e) if e.ty.to_string() == expected))
        );
        if owner_path == "Box" {
            assert!(matches!(&fields[0].ty, ValueType::Parameter {owner: binder, index: 0} if binder.as_ref() == owner));
        }
    }
    let (case, fields) = typed
        .record_fields
        .iter()
        .find(|(id, _)| id.path == "language\\core\\reflect\\CallableInfo::Requirement")
        .unwrap();
    assert_eq!(case.kind, omega_parser::NodeKind::Case);
    assert_eq!(
        fields[0].declaration.path,
        "language\\core\\reflect\\CallableInfo::Requirement::declaration_identity"
    );
    assert_eq!(fields[0].declaration.kind, omega_parser::NodeKind::CaseField);
}

#[test]
fn collection_metadata_nodes_record_the_selected_operation() {
    let source = "data Record{len:u64;} machine observations(view:@[u8],record:Record)->u64{_ = view.address; view.len + record.len}";
    let typed = type_package(source, READ, FILE_ORDERS[0]).unwrap();
    let tree = typed
        .trees()
        .find(|tree| tree.resolved().file.path == "render.omg")
        .unwrap();
    let file = tree.resolved().file;
    let node = |expression: &str| {
        file.nodes
            .iter()
            .enumerate()
            .find(|(i, syntax)| syntax.kind == omega_parser::NodeKind::FieldAccess && file.span(*i).slice(source.as_bytes()) == Some(expression.as_bytes()))
            .unwrap()
            .0
    };
    let byte = ValueType::Integer { signed: false, width: Some(8) };
    let view = ValueType::Reference(omega_typer::Access::Shared, Box::new(ValueType::Slice(Box::new(byte))));
    for (expression, token, expected) in [
        (
            "view.len",
            ".len",
            ValueType::Integer {
                signed: false,
                width: Some(64),
            },
        ),
        ("view.address", ".address", ValueType::Address),
    ] {
        let occurrence = node(expression);
        let records: Vec<_> = tree
            .semantics_at(occurrence)
            .filter_map(|record| match record {
                NodeSemantics::Operation(operation) => Some(operation),
                _ => None,
            })
            .collect();
        let [operation] = records.as_slice() else {
            panic!("expected one operation for {expression}")
        };
        assert_eq!(operation.node, file.node_id(occurrence));
        assert_eq!(operation.token, token);
        assert_eq!(operation.operands.as_slice(), std::slice::from_ref(&view));
        assert_eq!(operation.result, expected);
        assert!(
            tree.semantics_at(occurrence)
                .all(|record| !matches!(record, NodeSemantics::Field(_)))
        );
    }

    let record = node("record.len");
    assert!(
        tree.semantics_at(record)
            .any(|record| matches!(record, NodeSemantics::Field(field) if field.path == "Record::len"))
    );
    assert!(
        tree.semantics_at(record)
            .all(|record| !matches!(record, NodeSemantics::Operation(_)))
    );
}

#[test]
fn reflected_members_preserve_their_application_and_callable_role() {
    use omega_typer::DeclarationSelection;
    let declarations = "data Buffer<T>{unused:bool; used:T;} trait Writer{machine write();} machine update(){}";
    let source = r"use omega\language\core\reflect;
        machine inspect(){let f=reflect\field(Buffer::<u32>::used);
            let calls=[reflect\callable(update),reflect\callable(Writer::write)];}";
    let trees = type_package(declarations, source, FILE_ORDERS[0]).unwrap();
    for order in FILE_ORDERS.iter().skip(1) {
        let reordered = type_package(declarations, source, *order).unwrap();
        assert_eq!(trees.declaration_selections, reordered.declaration_selections);
    }
    assert_eq!(trees.declaration_selections.len(), 3);
    let mut selections = trees.declaration_selections.values();
    let Some(DeclarationSelection::Field { owner: ValueType::Applied(owner, args), field, field_type, position: 1, case: None }) = selections.next()
        else { panic!("expected the second field of Buffer<u32>") };
    assert_eq!(owner.path, "Buffer");
    assert_eq!(field.path, "Buffer::used");
    assert_eq!(args.as_slice(), [ValueType::Integer { signed: false, width: Some(32) }]);
    assert_eq!(field_type, &args[0]);
    assert!(matches!(selections.next(), Some(DeclarationSelection::Machine(m)) if m.machine.path == "update"));
    assert!(matches!(selections.next(), Some(DeclarationSelection::Requirement { owner, application })
        if owner.path == "Writer" && application.machine.path == "Writer::write"));
}

#[test]
fn reflection_reports_the_invalid_generic_application_at_its_selector() {
    let defs = "data Buffer<T>{used:T;} machine keep<T>(value:T)->T{value}";
    for (query, site, reason) in [
        (r"reflect\field(Buffer::used)", "Buffer::used", "Buffer expects 1 type arguments, found 0"),
        (r"reflect\callable(keep)", "keep", "keep expects 1 machine arguments, found 0"),
        (r"reflect\field(Buffer::<u32>::used::<u8>)", "Buffer::<u32>::used::<u8>", "type arguments do not belong to the selected carrier"),
    ] {
        let source = format!(r"use omega\language\core\reflect; machine inspect(){{let x={query};}}");
        let error = type_package(defs, &source, FILE_ORDERS[0]).unwrap_err();
        assert_eq!(error.message, reason);
        assert_eq!(error.file, "view.omg");
        assert_eq!(error.span.slice(source.as_bytes()), Some(site.as_bytes()));
    }
}

#[test]
fn reflected_callable_keeps_receiver_and_member_arguments_and_its_result_lifetime() {
    use omega_typer::DeclarationSelection;
    let defs = "trait Writer<T>{machine write<U>(a:T,b:U);}";
    let source = r"use omega\language\core\reflect;
        machine describe<'r>()->reflect\CallableInfo<'r>{reflect\callable(Writer::<u32>::write::<bool>)}";
    let trees = type_package(defs, source, FILE_ORDERS[0]).unwrap();
    let (&node, selection) = trees.declaration_selections.first_key_value().unwrap();
    let DeclarationSelection::Requirement { owner, application } = selection else {panic!("expected a trait requirement")};
    assert_eq!(application.arguments, [ValueType::Bool]);
    assert_eq!(application.enclosing_arguments, [(owner.clone(), vec![ValueType::Integer { signed: false, width: Some(32) }])]);
    let describe = trees.signatures.iter().find(|s| s.machine.path == "describe").unwrap();
    assert!(trees.expressions.iter().any(|e| e.node == node && e.ty == describe.result));
}

#[test]
fn reflection_preserves_a_case_fields_owner_and_position() {
    use omega_typer::DeclarationSelection;
    let defs = "data Choice<T>{case Empty; case Some(flag:bool,value:T);}";
    let source = r"use omega\language\core\reflect; machine inspect(){let field=reflect\field(Choice::<u8>::Some::value);}";
    let trees = type_package(defs, source, FILE_ORDERS[0]).unwrap();
    let Some(DeclarationSelection::Field { case: Some(case), field, position: 1, field_type, owner: ValueType::Applied(owner, args) }) =
        trees.declaration_selections.values().next() else { panic!("expected a case field") };
    assert_eq!(case.path, "Choice::Some");
    assert_eq!(field.path, "Choice::Some::value");
    assert_eq!(owner.path, "Choice");
    assert_eq!(args.as_slice(), std::slice::from_ref(field_type));
    assert_eq!(field_type, &ValueType::Integer { signed: false, width: Some(8) });
}

#[test]
fn reflected_requirement_checks_callback_arguments_against_the_applied_trait() {
    let defs = "data Filter<T> = machine(value:T)->bool;
        trait Writer<T>{machine write<const Accept:Filter<T>>();}
        machine matching(value:u32)->bool{true} machine wrong(value:bool)->bool{true}";
    let selector = r"reflect\callable(Writer::<u32>::write::<matching>)";
    let source = format!(r"use omega\language\core\reflect; machine describe(){{let descriptor={selector};}}");
    let result = type_package(defs, &source, FILE_ORDERS[0]).unwrap();
    let Some(omega_typer::DeclarationSelection::Requirement { application, .. }) = result.declaration_selections.values().next() else {
        panic!("expected an applied trait requirement");
    };
    assert!(matches!(application.arguments.as_slice(), [ValueType::MachineSelection(callback)] if callback.machine.path == "matching"));
    let wrong = source.replace("matching", "wrong");
    let error = type_package(defs, &wrong, FILE_ORDERS[0]).unwrap_err();
    assert_eq!(error.file, "view.omg");
    assert_eq!(error.message, "selected machine has incompatible callable signature");
    assert_eq!(error.span.slice(wrong.as_bytes()), Some(b"Writer::<u32>::write::<wrong>".as_slice()));
}
