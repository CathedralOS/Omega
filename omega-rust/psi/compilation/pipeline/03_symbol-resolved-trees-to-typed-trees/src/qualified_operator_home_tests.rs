//! A token binding keeps its resolved semantic home without changing its lexical owner.

use std::sync::Arc;

fn typed(units: &[(&str, &str)]) -> typed_trees::TypedTrees {
    checked(units).unwrap()
}

fn checked(units: &[(&str, &str)]) -> Result<typed_trees::TypedTrees, String> {
    packages(
        &units
            .iter()
            .map(|(path, text)| (31, *path, *text))
            .collect::<Vec<_>>(),
        &[],
        &[],
    )
}

fn packages(
    units: &[(u8, &str, &str)],
    dependencies: &[(u8, &str, u8)],
    imports: &[(usize, usize, &str, usize)],
) -> Result<typed_trees::TypedTrees, String> {
    let mut sources = source::SourceMap::default();
    let mut source_ids = Vec::with_capacity(units.len());
    let mut parsed_units = Vec::with_capacity(units.len());
    let identity = |key| semantic_vocabulary::PackageKeyIdentity::from_digest([key; 32]).unwrap();
    for (package, path, text) in units {
        let root = std::path::PathBuf::from(format!("package-{package}"));
        let id = sources
            .add_with_metadata(
                root.join(path),
                (*text).to_owned(),
                root,
                Some(identity(*package)),
                source::SourceOrigin::User,
            )
            .source_id;
        source_ids.push(id);
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let parsed = tokens_to_syntax_trees::parse_syntax_trees_with_id(id, &tokens).unwrap();
        parsed_units.push(parsed);
    }
    sources.retain_product_dependency_scope(
        dependencies
            .iter()
            .map(|(from, alias, to)| (identity(*from), *alias, identity(*to))),
    );
    let syntax = parsed_units
        .into_iter()
        .reduce(|mut combined, unit| {
            combined.extend_from(&unit);
            combined
        })
        .unwrap();
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    // Package-qualified imports enter resolution with the loader's exact
    // source/alias association, alongside the authored `use` declaration.
    request.top_level_bindings = imports
        .iter()
        .map(|(reference, declaration, path, prefix)| {
            symbols::SourceScopedTopLevelBinding::logical_module_import(
                source_ids[*reference],
                source_ids[*declaration],
                path,
                *prefix,
            )
        })
        .collect();
    request.sources = Some(Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| error.message)
            .collect::<Vec<_>>()
            .join("; ")
    })?;
    crate::lower_symbol_resolved_trees(&resolved).map_err(|error| error.message)
}

const DOMAIN: &str = "module units; public domain u32::Distance;";
const OPERATOR: &str = "public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { ((left as -Distance) +% (right as -Distance)) as +Distance }";

#[test]
fn imported_domain_operator_keeps_its_exact_home_and_lexical_module() {
    let program = typed(&[
        (
            "package/units.omg",
            "module units; public domain u32::Distance;",
        ),
        (
            "package/main.omg",
            "use units; public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { ((left as -Distance) +% (right as -Distance)) as +Distance }",
        ),
    ]);
    let domain = program.domain_definitions().first().unwrap();
    let binding = program.machine_token_bindings().first().unwrap();
    assert_eq!(binding.home_domain, domain.symbol);
    assert!(!program.symbols.symbol_module(binding.symbol).is_valid());
    assert!(domain.semantic_roles.denotation_dimension.is_none());
}

#[test]
fn operator_can_be_declared_beside_the_domain_or_in_another_owning_module() {
    for declaration in [
        format!("{DOMAIN} {OPERATOR}"),
        format!("module operations; use units; {OPERATOR}"),
    ] {
        let units = if declaration.starts_with(DOMAIN) {
            vec![("units.omg", declaration.as_str())]
        } else {
            vec![
                ("units.omg", DOMAIN),
                ("operations.omg", declaration.as_str()),
            ]
        };
        let program = typed(&units);
        assert_eq!(
            program.machine_token_bindings()[0].home_domain,
            program.domain_definitions()[0].symbol
        );
    }
}

#[test]
fn unknown_and_ambiguous_homes_reject_without_any_operator_use() {
    let unknown = format!(
        "use units; {}",
        OPERATOR.replace("Distance::add", "Missing::add")
    );
    let error = checked(&[("units.omg", DOMAIN), ("main.omg", &unknown)])
        .err()
        .unwrap();
    assert!(error.contains("Missing"), "{error}");

    let ambiguous = format!(
        "use one; use two; {}",
        OPERATOR.replace("u32 in Distance", "u32 in one::u32::Distance")
    );
    assert!(
        checked(&[
            ("one.omg", "module one; public domain u32::Distance;"),
            ("two.omg", "module two; public domain u32::Distance;"),
            ("main.omg", &ambiguous),
        ])
        .is_err()
    );
}

#[test]
fn foreign_package_cannot_declare_the_imported_domains_operator() {
    let declaration = format!("use dep::units; {OPERATOR}");
    let error = packages(
        &[(31, "units.omg", DOMAIN), (32, "main.omg", &declaration)],
        &[(32, "dep", 31)],
        &[(1, 0, "dep::units", 1)],
    )
    .err()
    .unwrap();
    assert!(error.contains("another package owns"), "{error}");
}

#[test]
fn duplicate_shapes_across_owning_modules_reject_without_a_use() {
    let first = format!("module first; use units; {OPERATOR}");
    let second = format!(
        "module second; use units; {}",
        OPERATOR.replace("Distance::add", "Distance::another")
    );
    let error = checked(&[
        ("units.omg", DOMAIN),
        ("first.omg", &first),
        ("second.omg", &second),
    ])
    .err()
    .unwrap();
    assert!(error.contains("same owner and operand shape"), "{error}");
}

#[test]
fn a_domain_home_must_participate_in_the_operand_tuple() {
    let declaration =
        "use units; machine + Distance::add(left: u32, right: u32) -> u32 { left +% right }";
    let error = checked(&[("units.omg", DOMAIN), ("main.omg", declaration)])
        .err()
        .unwrap();
    assert!(
        error.contains("no operand carries its home domain"),
        "{error}"
    );
}

#[test]
fn body_imports_belong_to_the_declaring_file() {
    use typed_trees::expression::ExpressionNode;
    let body = "machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { helper((left as -Distance) +% (right as -Distance)) as +Distance }";
    let own_import = format!("use units; use helpers::helper; {body}");
    let borrowed_import = format!("use units; {body}");
    let domain = "module units; use helpers::helper; public domain u32::Distance;";
    let helpers = "module helpers; public machine helper(value: u32) -> u32 { value }";
    let own = typed(&[
        ("units.omg", domain),
        ("helpers.omg", helpers),
        ("main.omg", &own_import),
    ]);
    let borrowed = typed(&[
        ("units.omg", domain),
        ("helpers.omg", helpers),
        ("main.omg", &borrowed_import),
    ]);
    let targets = |program: &typed_trees::TypedTrees| {
        program
            .expression_table
            .iter_expressions()
            .filter_map(|(_, node)| match node {
                ExpressionNode::Call(call) if call.target.as_str() == "helper" => {
                    Some(call.target_symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let own_targets = targets(&own);
    assert!(!own_targets.is_empty());
    assert!(
        own_targets.iter().all(|target| target.is_valid()),
        "{own_targets:?}"
    );
    let borrowed_targets = targets(&borrowed);
    assert!(!borrowed_targets.is_empty());
    assert!(
        borrowed_targets.iter().all(|target| !target.is_valid()),
        "association borrowed another file's imports"
    );
}

#[test]
fn domain_family_discovery_preserves_visibility_without_importing_its_module() {
    use language_core::operator_spelling::OperatorSpelling;
    for (public, expected) in [(true, 1), (false, 0)] {
        let declaration = format!(
            "module operations; use units; {} {}",
            if public {
                OPERATOR.to_owned()
            } else {
                OPERATOR.replacen("public ", "", 1)
            },
            OPERATOR.replace("Distance", "Span"),
        );
        let program = packages(&[
            (31, "units.omg", "module units; public domain u32::Distance; public domain u32::Span;"),
            (31, "operations.omg", &declaration),
            (32, "client.omg", "use dep::units; machine sum(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { left }"),
        ], &[(32, "dep", 31)], &[(2, 0, "dep::units", 1)]).unwrap();
        let binding = &program.machine_token_bindings()[0];
        let caller = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "sum")
            .unwrap();
        let entry = program.machine_states(caller).first().unwrap();
        let operands = program
            .state_parameters(entry)
            .iter()
            .map(|parameter| Some(parameter.type_reference))
            .collect::<Vec<_>>();
        let at = program
            .symbols
            .symbol_provenance_source_span(caller.symbol)
            .unwrap();
        let candidates = typed_trees::operator::resolve_spelling_for_operands(
            &program,
            OperatorSpelling::Add,
            &operands,
            at,
        );
        assert_eq!(candidates.len(), expected, "public={public}");
        if let Some(selected) = candidates.first() {
            assert_eq!(selected.operator.symbol, binding.symbol);
            assert_eq!(selected.domain.unwrap().symbol, binding.home_domain);
        }
    }
}
