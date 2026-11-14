/* Operator-home selection must use the same scoped domain identities as lookup. */
use crate::preparation::declaration_selection::ConstantSelection;
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use std::sync::Arc;
use symbol_resolved_trees::name::DiagnosticName;
use symbol_resolved_trees::operator::OperatorDefinition;

fn select_home(units: &[(&str, &str)], explicit: Option<&str>) -> Result<Vec<usize>, String> {
    select_home_with_base(units, explicit, false)
}

fn select_home_with_base(
    units: &[(&str, &str)],
    explicit: Option<&str>,
    seeded: bool,
) -> Result<Vec<usize>, String> {
    let mut sources = source::SourceMap::default();
    let mut syntax = syntax_trees::SyntaxTrees::default();
    let mut base_syntax = syntax_trees::SyntaxTrees::default();
    for (position, (path, text)) in units.iter().enumerate() {
        let path = std::path::PathBuf::from(path);
        let root = path.parent().unwrap().to_path_buf();
        let source = sources
            .add_with_metadata(
                path,
                (*text).to_owned(),
                root,
                None,
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let unit = tokens_to_syntax_trees::parse_syntax_trees_with_id(source, &tokens).unwrap();
        if seeded && position + 1 < units.len() {
            base_syntax.extend_from(&unit);
        } else {
            syntax.extend_from(&unit);
        }
    }
    let sources = Arc::new(sources);
    let base = seeded.then(|| {
        Arc::new(
            crate::resolution::resolve(crate::resolution::ResolutionRequest {
                syntax: base_syntax,
                sources: Some(Arc::clone(&sources)),
                top_level_bindings: Vec::new(),
                retained_base: None,
            })
            .unwrap(),
        )
    });
    let selection = ConstantSelection::with_shared_base(
        &syntax,
        Some(Arc::clone(&sources)),
        Vec::new(),
        base.clone(),
    )
    .unwrap();
    let mut lowerer = Lowerer::new(Some(sources), Vec::new());
    if let Some(base) = base {
        lowerer.seed_resolved_base(base).unwrap();
    }
    lowerer.constant_selection = Some(selection);
    lowerer.defer_machine_bodies = true;
    crate::lowering::lower_items(&mut lowerer, &syntax).unwrap();

    // Exercise the retained operator-signature representation without authoring
    // the retired `operator` introducer. Its operand shape comes from a machine.
    let program = &mut lowerer.symbol_resolved_trees;
    let machine = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .unwrap();
    let reference = machine.name.source_span();
    let entry = program.machine_state_handles(machine.states)[0];
    let parameters = program.machine_state(entry).parameters;
    let mut name = HandleSpan::empty();
    for member in explicit.unwrap_or("operation").split("::") {
        program
            .tables
            .declarations
            .operator_path_members
            .append_to_span(&mut name, DiagnosticName::new(member, reference));
    }
    program.operators.push(OperatorDefinition {
        name,
        parameters,
        ..Default::default()
    });
    super::select_operator_homes(&mut lowerer).map_err(|errors| {
        errors
            .iter()
            .map(|error| error.message.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    if seeded {
        let retained_domain = lowerer.symbol_resolved_trees.domain_definitions[0].symbol;
        crate::symbols::assign_declarations(&mut lowerer).unwrap();
        let program = &lowerer.symbol_resolved_trees;
        let domain = &program.domain_definitions[0];
        assert_eq!(domain.symbol, retained_domain);
        let [operator] = program.operator_definitions(domain.operators) else {
            panic!("expected the new operator on the retained domain");
        };
        assert!(
            operator.symbol.is_valid(),
            "homed operator was not published"
        );
        assert_eq!(program.symbols.get(operator.symbol).parent, domain.symbol);
        assert_eq!(
            program.symbols.display_path(operator.symbol, "::"),
            "units::Item::Mark::operation"
        );
    }
    Ok(lowerer
        .symbol_resolved_trees
        .domain_definitions
        .iter()
        .enumerate()
        .filter_map(|(position, domain)| (!domain.operators.is_empty()).then_some(position))
        .collect())
}

#[test]
fn domain_exposure_operator_home_rejoins_retained_base_domains() {
    assert_eq!(
        select_home_with_base(
            &[
                (
                    "app/units.omg",
                    "module units; public data Item {} public domain Item::Mark;"
                ),
                (
                    "app/main.omg",
                    "use units; machine probe(value: units::Item in units::Item::Mark) -> u64 { 0 }"
                ),
            ],
            None,
            true
        )
        .unwrap(),
        [0]
    );
}

#[test]
fn domain_exposure_operator_home_accepts_module_and_exact_imports() {
    for import in ["units", "units::u64::Mark"] {
        let caller = format!("use {import}; machine probe(value: u64 in u64::Mark) -> u64 {{ 0 }}");
        assert_eq!(
            select_home(
                &[
                    ("app/units.omg", "module units; public domain u64::Mark;"),
                    ("app/main.omg", &caller),
                ],
                None
            )
            .unwrap(),
            [0]
        );
    }
}

#[test]
fn domain_exposure_operator_home_does_not_borrow_sibling_imports() {
    assert!(
        select_home(
            &[
                (
                    "app/units.omg",
                    "module units; public domain u64::Mark; machine helper() -> u64 { 0 }"
                ),
                ("app/sibling.omg", "use units;"),
                (
                    "app/main.omg",
                    "use units::helper; machine probe(value: u64 in u64::Mark) -> u64 { 0 }"
                ),
            ],
            None
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn domain_exposure_operator_home_rejects_foreign_unimported_root() {
    assert!(
        select_home(
            &[
                ("foreign/units.omg", "domain u64::Mark;"),
                (
                    "app/main.omg",
                    "machine probe(value: u64 in Mark) -> u64 { 0 }"
                ),
            ],
            None
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn domain_exposure_operator_home_preserves_ambiguity_and_local_precedence() {
    let domains = [
        ("app/one.omg", "module one; public domain u64::Mark;"),
        ("app/two.omg", "module two; public domain u64::Mark;"),
    ];
    let error = select_home(
        &[
            domains[0],
            domains[1],
            (
                "app/main.omg",
                "use one; use two; machine probe(value: u64 in u64::Mark) -> u64 { 0 }",
            ),
        ],
        None,
    )
    .unwrap_err();
    assert!(
        error.contains("more than one possible domain home"),
        "{error}"
    );
    assert_eq!(
        select_home(
            &[
                domains[0],
                domains[1],
                (
                    "app/main.omg",
                    "module one; use two; machine probe(value: u64 in Mark) -> u64 { 0 }"
                ),
            ],
            None
        )
        .unwrap(),
        [0]
    );
}

#[test]
fn domain_exposure_operator_home_keeps_carrier_identity() {
    assert!(
        select_home(
            &[
                (
                    "app/one.omg",
                    "module one; public data Item {} public domain Item::Mark;"
                ),
                (
                    "app/main.omg",
                    "use one; data Item {} machine probe(value: Item in one::Item::Mark) -> u64 { 0 }"
                ),
            ],
            None
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn domain_exposure_explicit_owner_uses_the_same_scope() {
    assert_eq!(
        select_home(
            &[
                ("app/units.omg", "module units; public domain u64::Mark;"),
                (
                    "app/main.omg",
                    "use units; machine probe(value: u64) -> u64 { 0 }"
                ),
            ],
            Some("u64::Mark::operation")
        )
        .unwrap(),
        [0]
    );
    assert!(
        select_home(
            &[
                ("foreign/units.omg", "module units; domain u64::Mark;"),
                ("app/main.omg", "machine probe(value: u64) -> u64 { 0 }"),
            ],
            Some("units::u64::Mark::operation")
        )
        .unwrap()
        .is_empty()
    );
}
