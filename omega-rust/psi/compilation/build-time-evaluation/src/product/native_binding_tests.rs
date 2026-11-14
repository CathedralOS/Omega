//! Requirement preparation uses actual Build requests and Product declarations.
//! The fixture's plain locator record exercises source preparation only; it
//! does not stand in for core's native locator publication or host execution.

use super::provider_selection_tests::Fixture;
use language_semantics::declaration_selection::{BuildOperation, NativeBindingFormation};
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees_to_typed_trees::{PreparedProductRequirement, prepare_product_requirement_operand};

const PRODUCT: &str = r#"
public boundary trait Raw { machine read() reaches none crashes none; }
public boundary trait Other { machine read() reaches none crashes none; }
public trait Wrong { machine read() reaches none crashes none; }
"#;

fn build(requirement: &str) -> String {
    format!(
        r#"
machine build(builder: &mut Build) {{ install(builder); }}
machine install(builder: &mut Build) {{
    Build::bind<{requirement}>(builder, linux_x86_64, NativeBinding {{ number: 9u64 }});
}}
"#
    )
}

fn formation(fixture: &Fixture) -> (NativeBindingFormation, SymbolHandle) {
    let occurrences =
        typed_trees::product_operand::collect_product_selection_occurrences(&fixture.typed);
    let mut native = occurrences
        .into_iter()
        .filter(|occurrence| occurrence.request.operation == BuildOperation::NativeBinding);
    let first = native
        .next()
        .expect("an actual native binding source request");
    let author = fixture.typed.symbols.authored_origin(first.machine);
    // Preparation and commit carry one request and the same lexical author.
    for occurrence in native {
        assert!(
            fixture
                .typed
                .same_product_request_source(&first.request, &occurrence.request)
        );
        assert_eq!(
            fixture.typed.symbols.authored_origin(occurrence.machine),
            author
        );
    }
    assert_ne!(author, fixture.activation);
    assert_eq!(
        fixture
            .typed
            .machine_by_symbol(author)
            .unwrap()
            .name
            .as_str(),
        "install"
    );
    let formation = typed_trees::product_operand::native_binding_formation(
        &fixture.typed,
        author,
        Some(fixture.activation),
        Arc::clone(&first.request),
    )
    .expect("the actual Build author retains native formation custody");
    assert!(formation.root_scope);
    assert_eq!(
        formation.author,
        fixture
            .typed
            .symbols
            .symbol_provenance_source_span(author)
            .unwrap()
    );
    (formation, author)
}

fn requirement(fixture: &Fixture, owner: &str) -> SymbolHandle {
    let product = fixture.typed.product_declarations();
    let declaration = product
        .traits()
        .iter()
        .find(|declaration| declaration.name.as_str() == owner)
        .unwrap();
    product
        .trait_machine_signatures(declaration)
        .iter()
        .find(|signature| signature.name.as_str() == "read")
        .unwrap()
        .symbol
}

fn prepare(
    fixture: &mut Fixture,
) -> Result<PreparedProductRequirement, Vec<diagnostics::Diagnostic>> {
    let (formation, author) = formation(fixture);
    let [operand] = formation.request.operands.as_ref() else {
        panic!("the authored native bind has one requirement operand");
    };
    prepare_product_requirement_operand(&mut fixture.typed, operand, author, None)
}

#[test]
fn a_native_requirement_prepares_the_exact_designated_sibling() {
    let mut fixture = Fixture::new(&build("Raw::read"), PRODUCT, None);
    let expected = requirement(&fixture, "Raw");
    let other = requirement(&fixture, "Other");
    assert_ne!(expected, other);
    let prepared = prepare(&mut fixture).expect("the exact public boundary requirement prepares");
    assert_eq!(prepared.requirement, expected);
    assert_eq!(prepared.closed_requirement, Some(expected));
}

#[test]
fn a_native_method_keeps_its_own_static_binders_for_concrete_calls() {
    let mut fixture = Fixture::new(
        &build("Raw::read"),
        "public boundary trait Raw { machine read<const Count: u64>(bytes: &[u8; Count]) requires Count > 0 reaches none crashes none; }",
        None,
    );
    let expected = requirement(&fixture, "Raw");
    let prepared = prepare(&mut fixture).expect("Build binds the whole requirement schema");
    assert_eq!(prepared.requirement, expected);
    assert_eq!(prepared.closed_requirement, None);
    let declaration = fixture
        .typed
        .traits()
        .iter()
        .find(|row| row.name.as_str() == "Raw")
        .unwrap();
    let signature = fixture
        .typed
        .trait_machine_signature_by_symbol(declaration, expected)
        .unwrap();
    let parameters = fixture.typed.state_signature_type_parameters(signature);
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name.as_str(), "Count");
    assert!(!signature.contracts.is_empty());
}

#[test]
fn an_unapplied_generic_trait_does_not_become_a_closed_native_requirement() {
    let mut fixture = Fixture::new(
        &build("Raw::read"),
        "public boundary trait Raw<const Count: u64> { machine read(bytes: &[u8; Count]) reaches none crashes none; }",
        None,
    );
    let refused = prepare(&mut fixture).unwrap_err();
    assert!(
        refused
            .iter()
            .any(|row| row.message.contains("complete compile-time application"))
    );
}

#[test]
fn an_ordinary_trait_machine_is_not_a_native_boundary_requirement() {
    let mut fixture = Fixture::new(&build("Wrong::read"), PRODUCT, None);
    assert!(prepare(&mut fixture).is_err());
}

#[test]
fn a_dependency_exposes_its_public_native_requirement_and_keeps_its_private_sibling() {
    let dependency = r#"
public boundary trait Raw { machine read() reaches none crashes none; }
boundary trait PrivateRaw { machine read() reaches none crashes none; }
"#;
    let mut public = Fixture::new(&build("dep::Raw::read"), "", Some(dependency));
    let expected = requirement(&public, "Raw");
    let prepared = prepare(&mut public).expect("the dependency's public requirement is visible");
    assert_eq!(prepared.requirement, expected);

    let mut private = Fixture::new(&build("dep::PrivateRaw::read"), "", Some(dependency));
    assert!(prepare(&mut private).is_err());
}
