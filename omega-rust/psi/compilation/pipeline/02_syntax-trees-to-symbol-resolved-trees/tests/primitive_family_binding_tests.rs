/*
    An authored token binding whose operands name no declared type or domain
    joins a primitive operator family, which only the core semantic-home owner
    publishes. The spelled path is not a home: use sites select by operand
    types alone. Every introducer answers the same rule.
*/

use crate::frontend_fixture::Lexer;
use crate::{ResolutionRequest, resolve};

const PRIMITIVE_FAMILY: &str = "joins a primitive operator family";

fn diagnostics(source_text: &str) -> Vec<String> {
    let tokens = Lexer::new(source_text).tokenize().unwrap();
    let mut sources = source::SourceMap::default();
    let source_id = sources
        .add("primitive-family/main.omg".into(), source_text.to_owned())
        .source_id;
    let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
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

fn scoped_sources(
    units: &[(&str, &str, source::DependencyScope, source::SourceOrigin)],
) -> Result<symbol_resolved_trees::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let mut sources = source::SourceMap::default();
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
                source::SourceResolutionStratum::Base,
                *scope,
            )
            .source_id;
        let tokens = Lexer::new(text).tokenize().unwrap();
        let unit = crate::frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
        syntax.extend_from(&unit);
    }
    resolve(ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(sources)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
}

#[test]
fn domain_token_homes_keep_distinct_build_and_product_instances() {
    use source::{DependencyScope, SourceOrigin};
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
        assert_eq!(binding.operator_home_domain, domain.symbol);
        assert_eq!(domain.semantic_roles.denotation_dimension, None);
    }
}

#[test]
fn inactive_retained_token_bindings_do_not_rejoin_a_product_frontier() {
    use source::{DependencyScope, SourceOrigin};
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
    assert_ne!(build.operator_home_domain, product.operator_home_domain);
    let product_home = product.operator_home_domain;
    crate::lowering::machine::reject_duplicate_direct_token_bindings(&mut program).unwrap();

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
                source::SourceResolutionStratum::Base,
                DependencyScope::Product,
            )
            .source_id;
        let tokens = Lexer::new(&text).tokenize().unwrap();
        let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
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
            assert_eq!(binding.operator_home_domain, product_home);
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
    use source::{DependencyScope, SourceOrigin};
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
