//! L1 -> L2 handoff. Each test feeds a parsed file to the public resolver and checks
//! one specific use that L2 makes of the syntax tree promised by L1's GRAMMAR.md.
use omega_lexer::{SourceSpan, lex};
use omega_parser::parse_file;
use omega_resolver::{
    AssemblyVocabulary, DomainCarrier, NameRole, ParsedSource, StageSource, Symbol, SymbolResolvedTree, SymbolResolvedTrees, resolve_stage_for_target,
};
use omega_syntax::{NodeKind, TerminationClause};

mod support;

/// Resolve the files in both input orders and check provenance, as the stage tests do.
fn resolve_for<'text>(
    files: &[(&'text str, &'text str, &'text str)],
    assembly: &AssemblyVocabulary<'_>,
) -> SymbolResolvedTrees<'text> {
    let parses: Vec<_> = files
        .iter()
        .map(|&(_, _, text)| {
            let tokens = lex(text.as_bytes()).unwrap();
            let tree = parse_file(text.as_bytes(), &tokens).unwrap();
            (tokens, tree)
        })
        .collect();
    let mut sources: Vec<_> = files
        .iter()
        .zip(&parses)
        .map(|(&(package, logical_path, text), (tokens, tree))| StageSource {
            package,
            source: ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    let forward = resolve_stage_for_target(&sources, &[], assembly);
    if let Ok(stage) = &forward {
        support::check_origins(stage, &sources);
    }
    sources.reverse();
    assert_eq!(forward, resolve_stage_for_target(&sources, &[], assembly));
    forward.unwrap_or_else(|error| panic!("{error:?}"))
}

fn resolve<'text>(files: &[(&'text str, &'text str, &'text str)]) -> SymbolResolvedTrees<'text> {
    resolve_for(files, &AssemblyVocabulary::default())
}

fn at(
    text: &str,
    needle: &str,
) -> usize {
    text.find(needle)
        .unwrap_or_else(|| panic!("{needle:?} is not in {text:?}"))
}

fn text_of(
    text: &str,
    span: SourceSpan,
) -> &str {
    &text[span.get_start_offset()..span.get_end_offset()]
}

fn describe(symbol: &Symbol) -> String {
    match symbol {
        Symbol::Declaration(id) => format!("{:?} {}", id.kind, id.path),
        Symbol::Binding { kind, span, .. } => format!("{kind:?}@{}", span.get_start_offset()),
        Symbol::Builtin(name) => format!("builtin {name}"),
        Symbol::Member(name) => format!("member {name}"),
        Symbol::TargetInstruction(name) => format!("instruction {name}"),
        Symbol::TargetRegister(name) => format!("register {name}"),
        Symbol::ScopedNames { names, .. } => names.iter().map(describe).collect::<Vec<_>>().join(" | "),
        other => format!("{other:?}"),
    }
}

fn tree<'s>(
    stage: &'s SymbolResolvedTrees<'_>,
    path: &str,
) -> SymbolResolvedTree<'s> {
    stage
        .trees()
        .find(|tree| tree.file.path == path)
        .unwrap_or_else(|| panic!("no tree for {path}"))
}

/// Every ordinary reference spelled exactly `spelling` in `path`, in syntax order.
fn references(
    stage: &SymbolResolvedTrees<'_>,
    path: &str,
    text: &str,
    spelling: &str,
) -> Vec<String> {
    tree(stage, path)
        .occurrences()
        .filter(|u| u.role == NameRole::Reference && u.span.slice(text.as_bytes()) == Some(spelling.as_bytes()))
        .map(|u| describe(&u.target))
        .collect()
}

fn nodes(
    tree: SymbolResolvedTree<'_>,
    kind: NodeKind,
) -> Vec<usize> {
    (0..tree.file.nodes.len())
        .filter(|&n| tree.file.nodes[n].kind == kind)
        .collect()
}

fn signature(id: &omega_resolver::DeclarationId) -> String {
    let mut text = format!("{}({})", id.path, id.operands.join(", "));
    if !id.result_domains.is_empty() {
        text += &format!(" in {}", id.result_domains.join(" & "));
    }
    if let Some(owner) = &id.enclosing_machine {
        text += &format!(" within {}", owner.path);
    }
    text
}

// GRAMMAR "file, items": a file is `item*` and its namespace comes from its directory.
#[test]
fn each_item_of_a_file_becomes_one_declaration_in_its_directory_namespace() {
    use NodeKind::*;
    let items = "const ORIGIN: u8 = 0; data Point { x: u8; } domain u8::Small; trait Shape { machine area(p: Point) -> u8; } Square satisfies Shape {} machine scale(p: Point) -> Point { p } asm machine stub {} let twice(n: u8): u8 = n;";
    let stage = resolve(&[
        ("app", "geo/shapes.omg", items),
        ("app", "quiet/empty.omg", ""),
        ("app", "quiet/notes.omg", "// note\n/* outer /* nested */ */\n"),
    ]);
    let declared: Vec<_> = stage
        .declarations
        .iter()
        .map(|id| (id.kind, id.package.as_str(), id.path.as_str()))
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-file-items/consumer",
        assert_eq!(
            declared,
            [
                (Const, "app", "geo\\ORIGIN"),
                (Data, "app", "geo\\Point"),
                (Field, "app", "geo\\Point::x"),
                (Domain, "app", "geo\\u8::Small"),
                (Trait, "app", "geo\\Shape"),
                (Machine, "app", "geo\\Shape::area"),
                (Conformance, "app", "geo\\Square"),
                (Machine, "app", "geo\\scale"),
                (AsmMachine, "app", "geo\\stub"),
                (MathLet, "app", "geo\\twice"),
            ]
        )
    );
    // Every root child is an item, and every item of these files declares one name.
    let per_file: Vec<_> = stage
        .trees()
        .map(|tree| {
            let top_level = tree
                .declaration_nodes()
                .filter(|&(node, _)| tree.file.nodes[node].parent == Some(0))
                .count();
            (tree.file.path, tree.file.nodes[0].children.len(), top_level)
        })
        .collect();
    assert_eq!(per_file, [("geo/shapes.omg", 8, 8), ("quiet/empty.omg", 0, 0), ("quiet/notes.omg", 0, 0)]);
    // Files without items still contribute their directory as an importable module.
    let modules: Vec<_> = stage.compiled_interface().modules.into_iter().collect();
    assert_eq!(modules, [("app".to_owned(), "geo".to_owned()), ("app".to_owned(), "quiet".to_owned())]);
}

// GRAMMAR "data": members, cases, case fields, nested and inline data, `dropped by`,
// `requires` before the body and the alias form.
#[test]
fn declarations_at_the_parser_limit_resolve_under_the_complete_owner_chain() {
    let names: Vec<_> = (0..128).map(|i| format!("N{i}")).collect();
    let source = names.iter().map(|name| format!("data {name} {{")).collect::<String>() + &"}".repeat(names.len());
    let stage = resolve(&[("app", "main.omg", &source)]);
    let actual: Vec<_> = stage.declarations.iter().map(|id| (id.kind, id.path.as_str())).collect();
    let paths: Vec<_> = (1..=names.len()).map(|end| names[..end].join("::")).collect();
    let expected: Vec<_> = paths.iter().map(|path| (NodeKind::Data, path.as_str())).collect();
    spec_corpus::observe_handoff!("nesting-limit/resolved-owners", assert_eq!(actual, expected));
}

#[test]
fn data_members_nest_under_their_authored_owner_and_its_clauses_bind() {
    use NodeKind::*;
    let source = "machine release(b: Buffer) {} data Buffer dropped by release requires self.len > 0 { len: u8; op: data Operation { case Read; case Write(count: u8); }; data Header { tag: u8; } } data Words { case: u8; data: [u8; 4]; } data Bytes = Buffer;";
    let stage = resolve(&[("app", "main.omg", source)]);
    let declared: Vec<_> = stage
        .declarations
        .iter()
        .map(|id| (id.kind, id.path.as_str()))
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-data/consumer",
        assert_eq!(
            declared,
            [
                (Machine, "release"),
                (Data, "Buffer"),
                (Field, "Buffer::len"),
                (Field, "Buffer::op"),
                (Data, "Buffer::Operation"),
                (Case, "Buffer::Operation::Read"),
                (Case, "Buffer::Operation::Write"),
                (CaseField, "Buffer::Operation::Write::count"),
                (Data, "Buffer::Header"),
                (Field, "Buffer::Header::tag"),
                (Data, "Words"),
                (Field, "Words::case"),
                (Field, "Words::data"),
                (Data, "Bytes"),
            ]
        )
    );
    assert_eq!(references(&stage, "main.omg", source, "release"), ["Machine release"]);
    assert_eq!(references(&stage, "main.omg", source, "self"), ["builtin self"]);
    assert_eq!(references(&stage, "main.omg", source, "Buffer"), ["Data Buffer", "Data Buffer"]);
}

// GRAMMAR "domain": attached heads, nested domains, compositions, `requires` and indexed heads.
#[test]
fn domain_heads_compositions_and_indices_bind_their_written_parts() {
    let source = "public data Socket {} public domain Socket::Connected; public domain Socket::Authenticated; public domain Socket::Usable = Socket::Connected & Socket::Authenticated; public domain u8::Positive requires self > 0; public domain u8::Positive::Small requires self < 10; public domain<T, const U: u8> T::Quantity<U>;";
    let stage = resolve(&[("app", "domains.omg", source)]);
    let carriers: Vec<_> = stage
        .compiled_interface()
        .declarations
        .iter()
        .filter(|export| export.id.kind == NodeKind::Domain)
        .map(|export| {
            let carrier = match &export.domain_carrier {
                Some(DomainCarrier::Named(id)) => format!("named {}", id.path),
                Some(DomainCarrier::Builtin(name)) => format!("builtin {name}"),
                Some(DomainCarrier::Generic) => "generic".to_owned(),
                other => format!("{other:?}"),
            };
            (export.id.path.clone(), carrier)
        })
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-domain/consumer",
        assert_eq!(
            carriers,
            [
                ("Socket::Connected".to_owned(), "named Socket".to_owned()),
                ("Socket::Authenticated".to_owned(), "named Socket".to_owned()),
                ("Socket::Usable".to_owned(), "named Socket".to_owned()),
                ("u8::Positive".to_owned(), "builtin u8".to_owned()),
                ("u8::Positive::Small".to_owned(), "named u8::Positive".to_owned()),
                ("T::Quantity".to_owned(), "generic".to_owned()),
            ]
        )
    );
    let file = "domains.omg";
    assert_eq!(references(&stage, file, source, "Socket::Connected"), ["Domain Socket::Connected"]);
    assert_eq!(references(&stage, file, source, "Socket::Authenticated"), ["Domain Socket::Authenticated"]);
    assert_eq!(references(&stage, file, source, "Socket"), ["Data Socket"; 3]);
    assert_eq!(references(&stage, file, source, "self"), ["builtin self"; 2]);
    assert_eq!(references(&stage, file, source, "T"), [format!("TypeParam@{}", at(source, "T, const"))]);
    assert_eq!(references(&stage, file, source, "U"), [format!("ConstParam@{}", at(source, "U: u8"))]);
    let definitions: Vec<_> = tree(&stage, file)
        .occurrences()
        .filter(|u| u.role == NameRole::Definition)
        .map(|u| (text_of(source, u.span), describe(&u.target)))
        .collect();
    assert_eq!(definitions, [("Quantity", "Domain T::Quantity".to_owned())]);
}

// GRAMMAR "trait, conformance": a supertrait, a conformance's trait, and a selection row
// whose two paths keep their own spans.
#[test]
fn trait_parents_and_selection_rows_bind_each_written_path() {
    use NodeKind::*;
    let source = "trait Logger { machine write(x: u8); } trait Audit: Logger { machine flush(); } machine log_number(x: u8) {} Sink satisfies Audit { Logger::write = log_number; }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let declared: Vec<_> = stage
        .declarations
        .iter()
        .map(|id| (id.kind, id.path.as_str()))
        .collect();
    assert_eq!(
        declared,
        [
            (Trait, "Logger"),
            (Machine, "Logger::write"),
            (Trait, "Audit"),
            (Machine, "Audit::flush"),
            (Machine, "log_number"),
            (Conformance, "Sink"),
        ]
    );
    let audit = stage.declarations.iter().find(|id| id.path == "Audit").unwrap();
    assert_eq!(stage.trait_ancestors(audit).collect::<Vec<_>>(), [("app", "Logger")]);
    assert_eq!(references(&stage, "main.omg", source, "Logger"), ["Trait Logger"]);
    assert_eq!(references(&stage, "main.omg", source, "Audit"), ["Trait Audit"]);
    let tree = tree(&stage, "main.omg");
    let rows = nodes(tree, SelectionRow);
    let [row] = rows[..] else {
        panic!("one selection row")
    };
    let sides: Vec<_> = tree.file.nodes[row]
        .children
        .iter()
        .map(|&side| (text_of(source, tree.file.span(side)), tree.symbol(side).map(describe)))
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-trait-conformance/consumer",
        assert_eq!(
            sides,
            [
                ("Logger::write", Some("Machine Logger::write".to_owned())),
                ("log_number", Some("Machine log_number".to_owned())),
            ]
        )
    );
}

// GRAMMAR "machine" and the indexed-tree promise: ordered parameter annotations, the result
// annotation, operator tokens and nested machines.
#[test]
fn machine_signatures_supply_operator_tokens_ordered_operands_and_nested_owners() {
    let source = "domain u8::Even; data Grid { cells: u8; } machine [] read(grid: Grid, at: u8) -> u8 { at } machine + add(left: u8, right: u8) -> u8 { left } machine half(value: u8) -> u8 in Even { value } machine outer(count: u8) -> u8 { twice(count) machine twice(value: u8) -> u8 { value } }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let signatures: Vec<_> = stage
        .declarations
        .iter()
        .filter(|id| id.kind == NodeKind::Machine)
        .map(signature)
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-machine/consumer",
        assert_eq!(signatures, ["read(Grid, u8)", "add(u8, u8)", "half(u8) in Even", "outer(u8)", "outer::twice(u8) within outer"])
    );
    let tree = tree(&stage, "main.omg");
    let operators: Vec<_> = stage
        .operator_declarations
        .iter()
        .map(|op| (op.token.as_str(), op.machine.path.as_str(), text_of(source, tree.file.span(op.node.node))))
        .collect();
    assert_eq!(operators, [("[]", "read", "[]"), ("+", "add", "+")]);
    assert_eq!(references(&stage, "main.omg", source, "at"), [format!("Param@{}", at(source, "at: u8"))]);
    assert_eq!(references(&stage, "main.omg", source, "left"), [format!("Param@{}", at(source, "left: u8"))]);
    assert_eq!(references(&stage, "main.omg", source, "count"), [format!("Param@{}", at(source, "count: u8"))]);
    assert_eq!(references(&stage, "main.omg", source, "twice"), ["Machine outer::twice"]);
}

// GRAMMAR "mathematical let": parameters, result annotation and optional defining expression.
#[test]
fn a_mathematical_let_body_reads_its_own_parameters() {
    let source = "let greater_than(limit: i32, value: i32): bool = value > limit; boundary let choose(present: bool): bool; machine check(x: i32) -> bool { greater_than(0, x) }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let parts: Vec<_> = nodes(tree, NodeKind::MathLet)
        .into_iter()
        .map(|node| {
            let binding = tree.file.math_binding(node).unwrap();
            (
                text_of(source, tree.file.span(binding.parameters)),
                text_of(source, tree.file.span(binding.result)),
                binding.body.map(|body| text_of(source, tree.file.span(body))),
            )
        })
        .collect();
    assert_eq!(parts, [("(limit: i32, value: i32)", "bool", Some("value > limit")), ("(present: bool)", "bool", None)]);
    let body_reads = [
        references(&stage, "main.omg", source, "value"),
        references(&stage, "main.omg", source, "limit"),
    ];
    spec_corpus::observe_handoff!(
        "syntax-mathematical-let/consumer",
        assert_eq!(
            body_reads,
            [
                vec![format!("Param@{}", at(source, "value: i32"))],
                vec![format!("Param@{}", at(source, "limit: i32"))],
            ]
        )
    );
    let signatures: Vec<_> = stage
        .declarations
        .iter()
        .filter(|id| id.kind == NodeKind::MathLet)
        .map(signature)
        .collect();
    assert_eq!(signatures, ["greater_than(i32, i32)", "choose(bool)"]);
    assert_eq!(references(&stage, "main.omg", source, "greater_than"), ["MathLet greater_than"]);
}

// GRAMMAR "generic parameters": every binder form, in source order.
#[test]
fn each_generic_binder_form_binds_its_uses_and_numbers_the_operand_identity() {
    use NodeKind::*;
    let source = "machine every<T, Count: u32, const N: u64, machine Step, some S>(a: T, b: S) -> u32 { Step(a); N + Count }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let machines = nodes(tree, Machine);
    let [machine] = machines[..] else {
        panic!("one machine")
    };
    let binders: Vec<_> = tree
        .file
        .generic_parameters(machine)
        .map(|(leaf, _, kind)| (leaf.text, kind))
        .collect();
    assert_eq!(binders, [("T", TypeParam), ("Count", KindedParam), ("N", ConstParam), ("Step", MachineParam), ("S", SomeParam)]);
    let uses: Vec<_> = ["T", "S", "Step", "N", "Count", "a"]
        .into_iter()
        .map(|name| references(&stage, "main.omg", source, name))
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-generic-parameters/consumer",
        assert_eq!(
            uses,
            [
                vec![format!("TypeParam@{}", at(source, "T, Count"))],
                vec![format!("SomeParam@{}", at(source, "S>("))],
                vec![format!("MachineParam@{}", at(source, "Step, some"))],
                vec![format!("ConstParam@{}", at(source, "N: u64"))],
                vec![format!("KindedParam@{}", at(source, "Count: u32"))],
                vec![format!("Param@{}", at(source, "a: T"))],
            ]
        )
    );
    let every = stage.declarations.iter().find(|id| id.path == "every").unwrap();
    assert_eq!(every.operands, ["#TypeParam0", "#SomeParam1"]);
}

// GRAMMAR "generic arguments": `<...>` in types and `::<...>` in expressions. A type-shaped
// argument can still name a value or a machine; L2 binds it and names the list's owner.
#[test]
fn both_argument_spellings_name_their_owner_and_type_shaped_values_bind() {
    let source = "const SIZE: u32 = 8; data Buffer<const N: u32> {} machine set_time() {} machine apply<const F: machine>() {} machine f(b: Buffer<SIZE>) { apply::<set_time>(); }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let applications: Vec<_> = tree
        .applications()
        .map(|(section, name)| {
            let arguments: Vec<_> = tree.file.nodes[section]
                .children
                .iter()
                .map(|&argument| tree.file.nodes[argument].kind)
                .collect();
            (text_of(source, tree.file.span(section)), tree.file.nodes[section].kind, arguments, describe(&name.target))
        })
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-generic-arguments/consumer",
        assert_eq!(
            applications,
            [
                ("<SIZE>", NodeKind::TypeArgs, vec![NodeKind::PathType], "Data Buffer".to_owned()),
                ("::<set_time>", NodeKind::ExpressionArgs, vec![NodeKind::PathType], "Machine apply".to_owned()),
            ]
        )
    );
    assert_eq!(references(&stage, "main.omg", source, "SIZE"), ["Const SIZE"]);
    assert_eq!(references(&stage, "main.omg", source, "set_time"), ["Machine set_time"]);
}

// GRAMMAR "types": references, slices, arrays with a length expression, qualified types,
// module-prefixed paths and function types.
#[test]
fn every_component_of_a_written_type_binds_in_source_order() {
    let source = "domain u8::Small; domain u8::Even; const LEN: u64 = 4; data Cell { v: u8; } machine f(a: @mut [u8], b: [Cell; LEN], c: u8 in Small & Even, d: @@Cell, e: units\\Km, g: u8 -> Cell) {}";
    let stage = resolve(&[("app", "main.omg", source), ("app", "units/km.omg", "data Km {}")]);
    let params = "(a: @mut [u8], b: [Cell; LEN], c: u8 in Small & Even, d: @@Cell, e: units\\Km, g: u8 -> Cell)";
    let region = at(source, params)..at(source, params) + params.len();
    let bound: Vec<_> = tree(&stage, "main.omg")
        .occurrences()
        .filter(|u| u.role == NameRole::Reference && region.contains(&u.span.get_start_offset()))
        .map(|u| (text_of(source, u.span), describe(&u.target)))
        .collect();
    let expected = [
        ("u8", "builtin u8"),
        ("Cell", "Data Cell"),
        ("LEN", "Const LEN"),
        ("u8", "builtin u8"),
        ("Small", "Domain u8::Small"),
        ("Even", "Domain u8::Even"),
        ("Cell", "Data Cell"),
        ("units\\Km", "Data units\\Km"),
        ("u8", "builtin u8"),
        ("Cell", "Data Cell"),
    ]
    .map(|(spelling, target)| (spelling, target.to_owned()));
    spec_corpus::observe_handoff!("syntax-types/consumer", assert_eq!(bound, expected));
}

// GRAMMAR "header clauses": the termination clause's separate measure, order and bound,
// `result` in `ensures`, `none` after `reaches`, a crash cause with its guard, and a
// clause word in operand position.
#[test]
fn header_clause_parts_bind_in_their_own_roles() {
    let source = "data Down {} machine count(x: u8, limit: u8, reaches: u8) -> u8 requires reaches < limit ensures result >= x reaches none crashes Trap x == 0 terminates by x -> Down in limit { x }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let clauses = nodes(tree, NodeKind::Terminates);
    let [terminates] = clauses[..] else {
        panic!("one termination clause")
    };
    let Some(TerminationClause::By {
        measure,
        order: Some(order),
        bound: Some(bound),
    }) = tree.file.nodes[terminates].termination_clause()
    else {
        panic!("a measured clause with an order and a bound")
    };
    let parts: Vec<_> = [measure, order, bound]
        .into_iter()
        .map(|node| tree.symbol(node).map(describe))
        .collect();
    let x = format!("Param@{}", at(source, "x: u8"));
    let limit = format!("Param@{}", at(source, "limit: u8"));
    spec_corpus::observe_handoff!(
        "syntax-header-clauses/consumer",
        assert_eq!(parts, [Some(x.clone()), Some("Data Down".to_owned()), Some(limit.clone())])
    );
    let file = "main.omg";
    assert_eq!(references(&stage, file, source, "reaches"), [format!("Param@{}", at(source, "reaches: u8"))]);
    assert_eq!(references(&stage, file, source, "result"), ["builtin result"]);
    assert_eq!(references(&stage, file, source, "none"), ["builtin none"]);
    assert_eq!(references(&stage, file, source, "Trap"), ["builtin Trap"]);
    assert_eq!(references(&stage, file, source, "x"), [x.clone(), x.clone(), x.clone(), x]);
    assert_eq!(references(&stage, file, source, "limit"), [limit.clone(), limit]);
}

// GRAMMAR "statements": `let mut` with a type, assignment to a place, discard and tail.
#[test]
fn statement_forms_read_and_write_the_local_their_let_introduces() {
    let source = "machine step(seed: u8) -> u8 { let mut total: u8 = seed; total = total + 1; _ = seed; total }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let total = format!("BindingPattern@{}", at(source, "total:"));
    let seed = format!("Param@{}", at(source, "seed:"));
    let reads = [
        references(&stage, "main.omg", source, "total"),
        references(&stage, "main.omg", source, "seed"),
    ];
    spec_corpus::observe_handoff!(
        "syntax-statements/consumer",
        assert_eq!(reads, [vec![total.clone(), total.clone(), total], vec![seed.clone(), seed]])
    );
    let tree = tree(&stage, "main.omg");
    let patterns: Vec<_> = (0..tree.file.nodes.len())
        .filter_map(|node| {
            tree.pattern_binding(node)
                .map(|span| (text_of(source, tree.file.span(node)), text_of(source, span)))
        })
        .collect();
    assert_eq!(patterns, [("total", "total")]);
}

// GRAMMAR "match, transition": one arm grammar, braced arm bodies, comma-separated arms
// and subjectless conditions.
#[test]
fn arm_bindings_and_braced_arm_locals_end_at_their_arm() {
    let source = "machine pick(code: u8, ready: bool) -> u8 { let chosen = match code { 0 -> { let port = 1; port }, port -> port }; transition { ready -> chosen, _ -> code } }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let file = "main.omg";
    let arm_port = at(source, "port -> port");
    spec_corpus::observe_handoff!(
        "syntax-match-transition/consumer",
        assert_eq!(
            references(&stage, file, source, "port"),
            [
                format!("BindingPattern@{}", at(source, "port = 1")),
                format!("BindingPattern@{arm_port}"),
            ]
        )
    );
    assert_eq!(references(&stage, file, source, "ready"), [format!("Param@{}", at(source, "ready:"))]);
    assert_eq!(references(&stage, file, source, "chosen"), [format!("BindingPattern@{}", at(source, "chosen ="))]);
    let code = format!("Param@{}", at(source, "code:"));
    assert_eq!(references(&stage, file, source, "code"), [code.clone(), code]);
}

// GRAMMAR "patterns": record patterns with renamed, waived, shorthand and projected fields,
// positional rest, bare-word bindings and qualified constants.
#[test]
fn each_pattern_form_publishes_exactly_the_locals_it_introduces() {
    let source = "data Pair { left: u8; right: u8; } machine read(p: Pair, a: u8, b: u8) -> u8 { let Pair { left as slot, right } = p; let (first, ..) = (a, b); let picked = match p { Pair { left: 0, .. } -> right, Pair { left as _, right as r } -> r + slot + first }; match a { limits\\x -> picked, x -> x } }";
    let stage = resolve(&[("app", "limits/values.omg", "const x: u8 = 7;"), ("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let patterns: Vec<_> = (0..tree.file.nodes.len())
        .filter_map(|node| {
            tree.pattern_binding(node)
                .map(|span| (text_of(source, tree.file.span(node)), text_of(source, span)))
        })
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-patterns/consumer",
        assert_eq!(
            patterns,
            [
                ("left as slot", "slot"),
                ("right", "right"),
                ("first", "first"),
                ("picked", "picked"),
                ("right as r", "r"),
                ("x", "x"),
            ]
        )
    );
    let file = "main.omg";
    assert_eq!(references(&stage, file, source, "slot"), [format!("FieldPattern@{}", at(source, "slot"))]);
    assert_eq!(references(&stage, file, source, "right"), [format!("FieldPattern@{}", at(source, "right }"))]);
    assert_eq!(references(&stage, file, source, "r"), [format!("FieldPattern@{}", at(source, "right as r") + "right as ".len())]);
    assert_eq!(references(&stage, file, source, "first"), [format!("BindingPattern@{}", at(source, "first,"))]);
    assert_eq!(references(&stage, file, source, "picked"), [format!("BindingPattern@{}", at(source, "picked ="))]);
    assert_eq!(references(&stage, file, source, "limits\\x"), ["Const limits\\x"]);
    assert_eq!(references(&stage, file, source, "x"), [format!("BindingPattern@{}", at(source, "x -> x"))]);
    // `left: 0` is projected equality: it selects the member and introduces nothing.
    let projected: Vec<_> = nodes(tree, NodeKind::FieldPattern)
        .into_iter()
        .filter(|&node| text_of(source, tree.file.span(node)) == "left: 0")
        .map(|node| {
            let names: Vec<_> = tree.names(node).map(|u| (u.role, describe(&u.target))).collect();
            (names, tree.pattern_binding(node))
        })
        .collect();
    assert_eq!(projected, [(vec![(NameRole::Member, "member left".to_owned())], None)]);
}

// GRAMMAR "binary operators": the tier table decides which subtree is the right operand
// of `in`, so the domain binds as a membership target. Equality keeps both operands.
#[test]
fn operator_grouping_selects_the_membership_target_and_equality_operands() {
    let source = "domain u8::Small; machine f(a: u8, b: u8) -> bool { a + b in Small && a == b }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let grouped: Vec<_> = nodes(tree, NodeKind::Binary)
        .into_iter()
        .map(|node| {
            let [lhs, rhs] = tree.file.nodes[node].children[..] else {
                panic!("binary operands")
            };
            (
                tree.file.operator(node).unwrap().token,
                text_of(source, tree.file.span(lhs)),
                text_of(source, tree.file.span(rhs)),
                tree.symbol(rhs).map(describe),
            )
        })
        .collect();
    let a = format!("Param@{}", at(source, "a: u8"));
    let b = format!("Param@{}", at(source, "b: u8"));
    spec_corpus::observe_handoff!(
        "syntax-binary-operators/consumer",
        assert_eq!(
            grouped,
            [
                ("&&", "a + b in Small", "a == b", None),
                ("in", "a + b", "Small", Some("Domain u8::Small".to_owned())),
                ("+", "a", "b", Some(b.clone())),
                ("==", "a", "b", Some(b.clone())),
            ]
        )
    );
    assert_eq!(references(&stage, "main.omg", source, "a"), [a.clone(), a]);
}

// GRAMMAR "prefixes, postfixes, primaries": record literals with shorthand and explicit
// fields, field access by name and dereference.
#[test]
fn record_fields_and_field_access_carry_member_spellings_and_value_bindings() {
    let source = "data Pair { left: u8; right: u8; } machine make(left: u8, other: @Pair) -> Pair { let copy = $other; Pair { left, right: copy.right } }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let names_at = |kind| -> Vec<_> {
        nodes(tree, kind)
            .into_iter()
            .map(|node| {
                let names: Vec<_> = tree.names(node).map(|u| (u.role, describe(&u.target))).collect();
                (text_of(source, tree.file.span(node)), names)
            })
            .collect()
    };
    let left = format!("Param@{}", at(source, "left: u8,"));
    spec_corpus::observe_handoff!(
        "syntax-prefixes-postfixes-primaries/consumer",
        assert_eq!(
            names_at(NodeKind::FieldInit),
            [
                ("left", vec![(NameRole::Member, "member left".to_owned()), (NameRole::Reference, left)]),
                ("right: copy.right", vec![(NameRole::Member, "member right".to_owned())]),
            ]
        )
    );
    assert_eq!(names_at(NodeKind::FieldAccess), [("copy.right", vec![(NameRole::Member, "member right".to_owned())])]);
    let file = "main.omg";
    assert_eq!(references(&stage, file, source, "copy"), [format!("BindingPattern@{}", at(source, "copy ="))]);
    assert_eq!(references(&stage, file, source, "other"), [format!("Param@{}", at(source, "other:"))]);
    assert_eq!(references(&stage, file, source, "Pair"), ["Data Pair"; 3]);
}

// GRAMMAR "casts": after `as`, the carrier and every `+D`/`-D` entry belong to the cast.
#[test]
fn cast_entries_bind_as_domains_rather_than_arithmetic_operands() {
    let source = "domain u16::Small; domain u16::Even; machine widen(x: u8) -> u16 { x as u16 +Small -Even }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let tree = tree(&stage, "main.omg");
    let entries: Vec<_> = nodes(tree, NodeKind::CastEntry)
        .into_iter()
        .map(|entry| {
            let [domain] = tree.file.nodes[entry].children[..] else {
                panic!("one domain per cast entry")
            };
            (tree.file.nodes[entry].leaves[0].text, tree.symbol(domain).map(describe))
        })
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-casts/consumer",
        assert_eq!(entries, [("+", Some("Domain u16::Small".to_owned())), ("-", Some("Domain u16::Even".to_owned()))])
    );
    let casts = nodes(tree, NodeKind::Cast);
    let [cast] = casts[..] else {
        panic!("one cast")
    };
    let [operand, carrier, ..] = tree.file.nodes[cast].children[..] else {
        panic!("cast operand and carrier")
    };
    assert_eq!(tree.symbol(operand).map(describe), Some(format!("Param@{}", at(source, "x: u8"))));
    assert_eq!(tree.symbol(carrier).map(describe), Some("builtin u16".to_owned()));
}

// GRAMMAR "assembly": dotted mnemonics, operands as expressions, `transition -> name;`,
// nested `asm machine`, `asm` blocks, and the register clauses with `none`.
#[test]
fn assembly_lines_bind_mnemonics_registers_values_and_local_continuations() {
    let source = "asm machine start(x: u32) registers ecx = x clobbers eax { mov.w ecx, x; transition -> done; asm machine done { ret; } } machine mixed(value: u32) -> u32 { asm registers ecx = value clobbers none { add ecx, ecx; nop } value }";
    let target = AssemblyVocabulary {
        instructions: &["mov.w", "ret", "add", "nop"],
        registers: &["ecx", "eax"],
    };
    let stage = resolve_for(&[("app", "asm.omg", source)], &target);
    let observed: Vec<_> = tree(&stage, "asm.omg")
        .occurrences()
        .map(|u| (u.role, text_of(source, u.span), describe(&u.target)))
        .collect();
    let x = format!("Param@{}", at(source, "x: u32"));
    let value = format!("Param@{}", at(source, "value: u32"));
    let expected = [
        (NameRole::Reference, "u32", "builtin u32".to_owned()),
        (NameRole::Register, "ecx", "register ecx".to_owned()),
        (NameRole::Reference, "x", x.clone()),
        (NameRole::Register, "eax", "register eax".to_owned()),
        (NameRole::Reference, "mov.w", "instruction mov.w".to_owned()),
        (NameRole::Reference, "ecx", "register ecx".to_owned()),
        (NameRole::Reference, "x", x),
        (NameRole::Reference, "done", "AsmMachine start::done".to_owned()),
        (NameRole::Reference, "ret", "instruction ret".to_owned()),
        (NameRole::Reference, "u32", "builtin u32".to_owned()),
        (NameRole::Reference, "u32", "builtin u32".to_owned()),
        (NameRole::Register, "ecx", "register ecx".to_owned()),
        (NameRole::Reference, "value", value.clone()),
        (NameRole::Reference, "add", "instruction add".to_owned()),
        (NameRole::Reference, "ecx", "register ecx".to_owned()),
        (NameRole::Reference, "ecx", "register ecx".to_owned()),
        (NameRole::Reference, "nop", "instruction nop".to_owned()),
        (NameRole::Reference, "value", value),
    ];
    spec_corpus::observe_handoff!("syntax-assembly/consumer", assert_eq!(observed, expected));
}

// GRAMMAR "no reserved words": declaration words are ordinary names wherever a name is expected.
#[test]
fn declaration_words_used_as_names_bind_like_any_other_name() {
    use NodeKind::*;
    let source = "data machine { use: u8; } machine names(machine: u32, data: u32, match: u32) -> u32 { let use = machine + data; use + match }";
    let stage = resolve(&[("app", "main.omg", source)]);
    let declared: Vec<_> = stage
        .declarations
        .iter()
        .map(|id| (id.kind, id.path.as_str()))
        .collect();
    assert_eq!(declared, [(Data, "machine"), (Field, "machine::use"), (Machine, "names")]);
    let reads: Vec<_> = ["machine", "data", "match", "use"]
        .into_iter()
        .map(|word| references(&stage, "main.omg", source, word))
        .collect();
    spec_corpus::observe_handoff!(
        "syntax-no-reserved-words/consumer",
        assert_eq!(
            reads,
            [
                vec![format!("Param@{}", at(source, "machine: u32"))],
                vec![format!("Param@{}", at(source, "data: u32"))],
                vec![format!("Param@{}", at(source, "match: u32"))],
                vec![format!("BindingPattern@{}", at(source, "use ="))],
            ]
        )
    );
}

#[test]
fn a_conformance_array_extent_keeps_the_local_binder_first_in_scope() {
    let source = "const N: u64 = 99; machine width(flag:bool)->u64 { 2 } trait T {} \
                  Policy<const N: u64, const Routes: [u8; width(N > 1)]> satisfies T {}";
    let stage = resolve(&[("app", "main.omg", source)]);
    let local = source.find("Policy<const N").unwrap() + "Policy<const ".len();
    assert_eq!(references(&stage, "main.omg", source, "N"), [format!("ConstParam@{local} | Const N")]);
    assert_eq!(references(&stage, "main.omg", source, "width"), ["Machine width"]);
    assert_eq!(references(&stage, "main.omg", source, "T"), ["Trait T"]);
}
