/*
    An authored token binding whose operands name no declared type or domain
    joins a primitive operator family, which only the core semantic-home owner
    publishes. The spelled path is not a home: use sites select by operand
    types alone. Every introducer answers the same rule.
*/

use crate::resolution::frontend_fixture::Lexer;
use crate::resolution::{ResolutionRequest, resolve};

const PRIMITIVE_FAMILY: &str = "joins a primitive operator family";

fn diagnostics(source_text: &str) -> Vec<String> {
    let tokens = Lexer::new(source_text).tokenize().unwrap();
    let mut sources = crate::source_inputs::SourceMap::default();
    let source_id = sources
        .add("primitive-family/main.omg".into(), source_text.to_owned())
        .source_id;
    let syntax =
        crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens)
            .unwrap();
    match resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    }) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect(),
    }
}

fn rejects_as_primitive_family(source_text: &str) -> bool {
    diagnostics(source_text)
        .iter()
        .any(|message| message.contains(PRIMITIVE_FAMILY))
}

#[test]
fn every_introducer_over_primitive_operands_joins_the_primitive_family() {
    for declaration in [
        "boundary operator - Mine::replacement(left: u32, right: u32) -> u32;",
        "boundary operator - u32::replacement(left: u32, right: u32) -> u32;",
        "boundary machine == Comparison::equal(left: i32, right: i32) -> bool;",
        "boundary machine == Float::equal(left: f64, right: f64) -> bool;",
        "operator + Mine::add(left: i32, right: i32) -> i32;",
        "machine + add(left: u8, right: u8) -> u8 { left }",
    ] {
        assert!(rejects_as_primitive_family(declaration), "{declaration}");
    }
}

#[test]
fn a_compiler_policy_qualifier_is_not_a_declared_home() {
    assert!(rejects_as_primitive_family(
        "boundary operator - Mine::replacement(left: u32 in Wrapping, right: u32 in Wrapping) -> u32 in Wrapping;"
    ));
}

/*
    `Wrapping` names core's policy only where no declared domain of that name
    is visible. A package's own `domain u8::Wrapping;` is a declared home for
    its `+`; the same `+` without the declaration joins the primitive family.
*/
#[test]
fn a_packages_own_wrapping_domain_is_a_declared_home() {
    let binding = "machine + add(x: u8 in Wrapping, y: u8) -> u8 in Wrapping { x }";
    assert_eq!(
        diagnostics(&format!("domain u8::Wrapping;\n{binding}")),
        Vec::<String>::new()
    );
    assert!(rejects_as_primitive_family(binding));

    // A recast takes a declared domain but never core's policy.
    let recast = "machine view(value: @u64) -> @u8 { value as @u8 in Wrapping }";
    assert!(
        diagnostics(recast)
            .iter()
            .any(|message| message.contains("takes no arithmetic domain")),
        "{:?}",
        diagnostics(recast)
    );
}

#[test]
fn a_declared_data_or_domain_operand_is_a_home_whatever_the_path() {
    for source_text in [
        "data Meter { value: u32; }\nboundary machine + Float::add(left: Meter, right: Meter) -> Meter;",
        "domain u32::Mine;\nboundary machine - Comparison::subtract(left: u32 in Mine, right: u32 in Mine) -> u32;",
    ] {
        assert_eq!(
            diagnostics(source_text),
            Vec::<String>::new(),
            "{source_text}"
        );
    }
}

type ScopedSource<'a> = (
    &'a str,
    &'a str,
    source_provenance::DependencyScope,
    source_admission::SourceOrigin,
);

fn scoped_sources(
    units: &[ScopedSource<'_>],
) -> Result<crate::source_resolved::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    scoped_sources_with_imports(units, &[])
}

fn scoped_sources_with_imports(
    units: &[ScopedSource<'_>],
    imports: &[(&str, &str, &str)],
) -> Result<crate::source_resolved::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = crate::source_inputs::SourceMap::default();
    let mut syntax = syntax_trees::SyntaxTrees::default();
    for (path, text, scope, origin) in units {
        let path = std::path::PathBuf::from(path);
        let root = path.parent().unwrap().to_path_buf();
        let source = sources
            .add_checked_instance(
                path,
                *text,
                root,
                None,
                *origin,
                source_admission::SourceResolutionStratum::Base,
                *scope,
            )
            .source_id;
        let tokens = Lexer::new(text).tokenize().unwrap();
        let unit = crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source, &tokens)
            .unwrap();
        syntax.extend_from(&unit);
    }
    let top_level_bindings = crate::resolution::fixture_file_bindings(&sources, imports);
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings,
        retained_base: None,
    })
}

#[test]
fn domain_token_homes_keep_distinct_build_and_product_instances() {
    use source_admission::SourceOrigin;
    use source_provenance::DependencyScope;
    let text = "domain<T, const U: u64> T::Quantity<U>;\n\
            machine + Quantity::add(left: u32 in Quantity<1>, right: u32 in Quantity<1>) \
            -> u32 in Quantity<1> { left }";
    let program = scoped_sources(&[
        (
            "core/units.omg",
            text,
            DependencyScope::Build,
            SourceOrigin::Toolchain,
        ),
        (
            "core/units.omg",
            text,
            DependencyScope::Product,
            SourceOrigin::Toolchain,
        ),
    ])
    .unwrap();
    let domains = program.domain_definitions.iter().collect::<Vec<_>>();
    assert_eq!(domains.len(), 2);
    assert_ne!(domains[0].symbol, domains[1].symbol);
    assert_eq!(program.machines.len(), 2);
    let source_of = |symbol| program.symbols.symbol_source_span(symbol).unwrap();
    let sources = program.symbols.source_map().unwrap();
    for scope in [DependencyScope::Build, DependencyScope::Product] {
        let domain = domains
            .iter()
            .find(|domain| {
                sources
                    .file_at(source_of(domain.symbol))
                    .unwrap()
                    .dependency_scope
                    == scope
            })
            .expect("each checked scope declares a domain");
        let source = source_of(domain.symbol).source_id;
        let mut bindings = program
            .machines
            .iter()
            .filter(|binding| source_of(binding.symbol).source_id == source);
        let binding = bindings
            .next()
            .expect("each checked instance declares a binding");
        assert!(bindings.next().is_none());
        assert_eq!(binding.attached_domain_symbol, domain.symbol);
        assert_eq!(domain.semantic_roles.denotation_dimension, None);
    }
}

#[test]
fn inactive_retained_token_bindings_do_not_rejoin_a_product_frontier() {
    use source_admission::SourceOrigin;
    use source_provenance::DependencyScope;
    let text = "domain<T, const U: u64> T::Quantity<U>;\n\
            machine + Quantity::add(left: u32 in Quantity<1>, right: u32 in Quantity<1>) \
            -> u32 in Quantity<1> { left }";
    let mut program = scoped_sources(&[
        (
            "core/units.omg",
            text,
            DependencyScope::Build,
            SourceOrigin::Toolchain,
        ),
        (
            "core/units.omg",
            text,
            DependencyScope::Product,
            SourceOrigin::Toolchain,
        ),
    ])
    .unwrap();
    let mut sources = program.symbols.source_map().unwrap().as_ref().clone();
    sources.retain_selected_sources(|file| file.dependency_scope == DependencyScope::Product);
    program.symbols = program
        .symbols
        .begin_source_activation(std::sync::Arc::new(sources), Vec::new())
        .unwrap()
        .finish();
    assert_eq!(program.machines.len(), 2);
    let sources = program.symbols.source_map().unwrap();
    let binding_in = |scope| {
        program
            .machines
            .iter()
            .find(|machine| {
                let span = program.symbols.symbol_source_span(machine.symbol).unwrap();
                sources.file_at(span).unwrap().dependency_scope == scope
            })
            .expect("both checked instances remain in the retained forest")
    };
    let build = binding_in(DependencyScope::Build);
    let product = binding_in(DependencyScope::Product);
    assert!(!program.symbols.symbol_source_is_selected(build.symbol));
    assert!(program.symbols.symbol_source_is_selected(product.symbol));
    assert_ne!(build.attached_domain_symbol, product.attached_domain_symbol);
    let product_home = product.attached_domain_symbol;
    crate::resolution::selections::machine_token_bindings::reject_duplicate_direct_token_bindings(
        &mut program,
    )
    .unwrap();

    // A newly selected binding still has to name its semantic home. The
    // retained inactive copy must neither reject the valid twin nor supply
    // missing qualification to the invalid one.
    for qualified in [true, false] {
        let suffix = if qualified { " in Quantity<1>" } else { "" };
        let text = format!(
            "machine - Quantity::subtract(left: u32{suffix}, right: u32{suffix}) -> u32{suffix} {{ left }}"
        );
        let mut sources = program.symbols.source_map().unwrap().as_ref().clone();
        let source = sources
            .add_checked_instance(
                "core/current.omg".into(),
                text.clone(),
                "core".into(),
                None,
                SourceOrigin::Toolchain,
                source_admission::SourceResolutionStratum::Base,
                DependencyScope::Product,
            )
            .source_id;
        let tokens = Lexer::new(&text).tokenize().unwrap();
        let syntax =
            crate::resolution::frontend_fixture::parse_syntax_trees_with_id(source, &tokens)
                .unwrap();
        let result = resolve(ResolutionRequest {
            syntax,
            sources: Some(std::sync::Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: Some(std::sync::Arc::new(program.clone())),
        });
        if qualified {
            let extension = result.unwrap();
            let binding = extension
                .machines
                .iter()
                .find(|machine| {
                    extension
                        .symbols
                        .symbol_source_span(machine.symbol)
                        .is_some_and(|span| span.source_id == source)
                })
                .expect("the new Product binding remains in the resolved extension");
            assert_eq!(binding.attached_domain_symbol, product_home);
        } else {
            let errors = result.unwrap_err();
            let [error] = errors.as_slice() else {
                panic!("expected one missing-home diagnostic: {errors:?}");
            };
            assert_eq!(error.source_span.map(|span| span.source_id), Some(source));
            assert!(
                error
                    .message
                    .contains("no operand carries its home domain `Quantity`"),
                "{error:?}"
            );
        }
    }
}

#[test]
fn an_unimported_same_leaf_domain_does_not_supply_an_operand_home() {
    use source_admission::SourceOrigin;
    use source_provenance::DependencyScope;
    let errors = scoped_sources(&[
        ("foreign/domain.omg", "public domain u32::Mine;", DependencyScope::Product, SourceOrigin::User),
        ("requester/main.omg", "boundary machine - Comparison::subtract(left: u32 in Mine, right: u32 in Mine) -> u32;", DependencyScope::Product, SourceOrigin::User),
    ]).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains(PRIMITIVE_FAMILY)),
        "{errors:?}"
    );
}

#[test]
fn only_the_declaring_domain_owns_a_core_content_projection() {
    use language_core::bundled_files::BundledFile;
    use source_admission::SourceOrigin;
    use source_provenance::DependencyScope;

    let content = std::str::from_utf8(bundled_declarations::bytes(BundledFile::CoreContent))
        .expect("bundled content.omg is UTF-8");
    let selected = |source| {
        scoped_sources_with_imports(
            &[
                (
                    "core/content.omg",
                    content,
                    DependencyScope::Product,
                    SourceOrigin::Toolchain,
                ),
                (
                    "app/main.omg",
                    source,
                    DependencyScope::Product,
                    SourceOrigin::User,
                ),
            ],
            &[("app/main.omg", "core/content.omg", "algebra")],
        )
    };
    let declaration = r#"use {omega}/language/core/content.omg as algebra;
        data Unit {}
        data Region { length: u64; }
        domain Region::Owned;
        machine Owned::measure(subject: @Region) -> |algebra|::CountedQuantity<Unit>
        satisfies |algebra|::Content<Region, |algebra|::CountedQuantity<Unit>>::project {
            |algebra|::CountedQuantity { magnitude: 0 }
        }"#;
    let program = selected(declaration).expect("a projection declared on the domain");
    let owner = program
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Region::Owned")
        .unwrap();
    let projection = program
        .machines
        .iter()
        .find(|machine| machine.name.as_str() == "Owned::measure")
        .unwrap();
    assert_eq!(projection.attached_domain_symbol, owner.symbol);
    assert_eq!(owner.content_projections, [projection.symbol]);

    let second = declaration[declaration.find("machine Owned::measure").unwrap()..]
        .replace("Owned::measure", "Owned::another");
    let twice = format!("{declaration}\n{second}");
    let errors = selected(&twice).expect_err("one exact domain may publish only one projection");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("only one core Content projection")),
        "{errors:?}"
    );
    let separate = format!(
        "{declaration}\ndomain Region::Other;\n{}",
        second.replace("Owned::another", "Other::another")
    );
    let separate =
        selected(&separate).expect("two qualifications can each publish their own projection");
    assert_eq!(
        separate
            .domain_definitions
            .iter()
            .filter(|domain| domain.content_projections.len() == 1)
            .count(),
        2
    );

    let unowned = declaration.replace("machine Owned::measure", "machine measure");
    let errors = selected(&unowned).expect_err("a free machine has no domain owner");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("a core Content projection must name a domain owned by the package")),
        "{errors:?}"
    );

    let ordinary = r#"data Region { length: u64; }
        trait Content<Owner, A> { machine project(subject: @Owner) -> A; }
        machine measure(subject: @Region) -> u64
        satisfies Content<Region, u64>::project { subject.length }"#;
    let ordinary = scoped_sources(&[(
        "app/ordinary.omg",
        ordinary,
        DependencyScope::Product,
        SourceOrigin::User,
    )])
    .expect("a user trait with the same name has its ordinary conformance rules");
    assert!(ordinary.machines.iter().all(|machine| !machine.attached_domain_symbol.is_valid()));
}

#[test]
fn a_file_import_can_name_a_domain_but_only_its_package_can_publish_the_projection() {
    use language_core::bundled_files::BundledFile;
    use source_admission::SourceOrigin;
    use source_provenance::DependencyScope;

    let content = std::str::from_utf8(bundled_declarations::bytes(BundledFile::CoreContent))
        .expect("bundled content.omg is UTF-8");
    let owner = "public data Region { length: u64; } public domain Region::Owned;";
    let projection = r#"
        data Unit {}
        machine |storage|::Region::Owned::measure(subject: @|storage|::Region)
        -> |algebra|::CountedQuantity<Unit>
        satisfies |algebra|::Content<|storage|::Region, |algebra|::CountedQuantity<Unit>>::project {
            |algebra|::CountedQuantity { magnitude: 0 }
        }"#;
    let check = |path| {
        scoped_sources_with_imports(
            &[
                (
                    "core/content.omg",
                    content,
                    DependencyScope::Product,
                    SourceOrigin::Toolchain,
                ),
                (
                    "resource/types.omg",
                    owner,
                    DependencyScope::Product,
                    SourceOrigin::User,
                ),
                (
                    path,
                    projection,
                    DependencyScope::Product,
                    SourceOrigin::User,
                ),
            ],
            &[
                (path, "resource/types.omg", "storage"),
                (path, "core/content.omg", "algebra"),
            ],
        )
    };
    let mut owned = check("resource/projection.omg")
        .expect("the owner package can define the projection in a sibling file");
    let domain = owned
        .domain_definitions
        .iter()
        .find(|domain| domain.name.as_str() == "Region::Owned")
        .expect("the domain in the imported file");
    let domain_symbol = domain.symbol;
    let [projection_symbol] = domain.content_projections.as_slice() else {
        panic!("the domain must retain its sibling file's projection")
    };
    let projection_symbol = *projection_symbol;
    assert_eq!(
        owned
            .machine(projection_symbol)
            .unwrap()
            .attached_domain_symbol,
        domain_symbol,
    );
    let mut sources = owned.symbols.source_map().unwrap().as_ref().clone();
    let projection_source = owned
        .symbols
        .symbol_source_span(projection_symbol)
        .unwrap()
        .source_id;
    sources.retain_selected_sources(|file| file.source_id != projection_source);
    owned.symbols = owned
        .symbols
        .begin_source_activation(std::sync::Arc::new(sources), Vec::new())
        .unwrap()
        .finish();
    crate::resolution::names::domain_attachments::resolve_machine_domain_homes(&mut owned)
        .expect("retained declarations keep the authored projection dependency");
    assert!(!owned.symbols.symbol_source_is_selected(projection_symbol));
    assert_eq!(
        owned
            .domain_definition_by_symbol(domain_symbol)
            .unwrap()
            .content_projections,
        [projection_symbol],
    );
    let errors = check("consumer/projection.omg")
        .expect_err("moving the same projection to another package must refuse");
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("a core Content projection must name a domain owned by the package")),
        "{errors:?}"
    );
}
