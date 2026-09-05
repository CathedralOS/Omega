use super::*;
use psi_language_semantics::declaration_selection::{
    AuthoredDeclarationSelectionExposure, AuthoredDeclarationSelectionTarget,
};

const DECLARATIONS: &str = "data Secret { value: i32; }\npub data Envelope<T> { value: T; }\n";

fn check(
    source: &str,
) -> Result<omega_compiler::CheckedCompilation, Vec<psi_diagnostics::Diagnostic>> {
    let tree = TempTree::new();
    let root = tree.package("generic-visibility");
    TempTree::write(root.join("main.omg"), source);
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "generic-visibility",
            root.clone(),
        )],
        Vec::new(),
    )
    .unwrap();
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
}

fn rejects_private_secret(source: &str, public_owner: &str) {
    let diagnostics = check(source).expect_err("public use must not expose the private argument");
    let owner_start = source.find(public_owner).unwrap();
    let argument_start = owner_start + source[owner_start..].find("Secret").unwrap();
    let visibility_errors = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .message
                .contains("public interface selects private data `Secret`")
        })
        .collect::<Vec<_>>();
    assert!(
        visibility_errors.len() == 1,
        "unexpected diagnostics: {diagnostics:#?}"
    );
    assert!(
        visibility_errors
            .iter()
            .all(|diagnostic| diagnostic
                .source_span
                .is_some_and(|span| span.span.start == argument_start
                    && span.span.end == argument_start + "Secret".len())),
        "only the authored public occurrence may expose Secret: {diagnostics:#?}"
    );
}

#[test]
fn private_generic_application_keeps_the_argument_private() {
    let checked = check(&format!(
        "{DECLARATIONS}machine consume(value: &Envelope<Secret>) {{}}\n"
    ))
    .expect("a private use of a public generic does not publish its type argument");
    let selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.display_path(target.selected_symbol(), "::") == "Secret"
            )
        })
        .collect::<Vec<_>>();
    assert!(
        selections.len() == 1,
        "the single authored argument must remain one selection occurrence"
    );
    assert!(selections.iter().all(|selection| selection.exposure()
        == AuthoredDeclarationSelectionExposure::PrivateImplementation));
}

#[test]
fn public_machine_generic_argument_rejects_a_private_type() {
    rejects_private_secret(
        &format!("{DECLARATIONS}pub machine expose(value: &Envelope<Secret>) {{}}\n"),
        "pub machine expose",
    );
}

#[test]
fn boundary_generic_argument_rejects_a_private_type() {
    rejects_private_secret(
        &format!("{DECLARATIONS}boundary machine expose(value: &Envelope<Secret>);\n"),
        "boundary machine expose",
    );
}

#[test]
fn public_field_generic_argument_rejects_a_private_type() {
    rejects_private_secret(
        &format!("{DECLARATIONS}pub data Api {{ value: Envelope<Secret>; }}\n"),
        "pub data Api",
    );
}

#[test]
fn a_shared_instance_preserves_private_then_public_occurrences() {
    rejects_private_secret(
        &format!(
            "{DECLARATIONS}machine consume(value: &Envelope<Secret>) {{}}\npub machine expose(value: &Envelope<Secret>) {{}}\n"
        ),
        "pub machine expose",
    );
}

#[test]
fn a_shared_instance_preserves_public_then_private_occurrences() {
    rejects_private_secret(
        &format!(
            "{DECLARATIONS}pub machine expose(value: &Envelope<Secret>) {{}}\nmachine consume(value: &Envelope<Secret>) {{}}\n"
        ),
        "pub machine expose",
    );
}

#[test]
fn a_public_generic_template_cannot_hide_an_authored_private_field() {
    rejects_private_secret(
        "data Secret { value: i32; }\npub data Envelope<T> { value: T; secret: Secret; }\nmachine consume(value: &Envelope<i32>) {}\n",
        "secret: Secret",
    );
}

#[test]
fn private_nested_generic_applications_do_not_publish_the_leaf() {
    check(&format!(
        "{DECLARATIONS}machine consume(value: &Envelope<Envelope<Secret>>) {{}}\n"
    ))
    .expect("nested derivations preserve the original private application");
}

#[test]
fn public_nested_generic_applications_reject_the_private_leaf() {
    rejects_private_secret(
        &format!("{DECLARATIONS}pub machine expose(value: &Envelope<Envelope<Secret>>) {{}}\n"),
        "pub machine expose",
    );
}

#[test]
fn public_generic_application_still_requires_a_public_template() {
    let source = "pub data Payload { value: i32; }\ndata Internal<T> { value: T; }\npub machine expose(value: &Internal<Payload>) {}\n";
    let diagnostics =
        check(source).expect_err("public arguments cannot make a private template public");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("public interface selects private data")
            && diagnostic.message.contains("Internal")),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn an_imported_public_generic_keeps_the_consumers_payload_private() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let library = tree.package("library");
    TempTree::write(
        library.join("envelopes.omg"),
        "pub data Envelope<T> { value: T; }\n",
    );
    TempTree::write(
        root.join("main.omg"),
        "use library::envelopes;\ndata Secret { value: i32; }\nmachine consume(value: &Envelope<Secret>) {}\n",
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "library", library),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "library",
            identity(2),
        )],
    )
    .unwrap();
    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("a library template does not publish a consumer's private type argument");
    let selections = checked
        .authored_declaration_selections()
        .iter()
        .filter(|selection| {
            matches!(selection.target(), AuthoredDeclarationSelectionTarget::Resolved(target)
            if checked.symbols.display_path(target.selected_symbol(), "::") == "Secret")
        })
        .collect::<Vec<_>>();
    assert_eq!(selections.len(), 1);
    assert_eq!(
        selections[0].exposure(),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    );
    assert!(
        checked
            .symbols
            .source_file(selections[0].source_span())
            .is_some_and(|file| file.package_identity == Some(identity(1)))
    );
}

#[test]
fn a_public_attached_template_method_does_not_publish_private_instances() {
    check(&format!(
        "{DECLARATIONS}pub machine Envelope::stored<T>(&self) -> T {{ self.value }}\nmachine consume(value: &Envelope<Secret>) {{}}\n"
    ))
    .expect("a generated attached method does not publish a private instance argument");
}

#[test]
fn an_authored_public_attached_method_still_rejects_a_private_type() {
    rejects_private_secret(
        &format!(
            "{DECLARATIONS}pub machine Envelope::expose<T>(&self, value: &Secret) {{}}\nmachine consume(value: &Envelope<i32>) {{}}\n"
        ),
        "pub machine Envelope::expose",
    );
}

#[test]
fn a_public_generic_free_machine_keeps_its_private_call_argument_private() {
    check(
        "data Secret { value: i32; }\npub machine identity<T>(value: T) -> T { value }\nmachine read() -> i32 { let secret: Secret = identity<Secret>(Secret { value: 7 }); secret.value }\n",
    )
    .expect("typed free-machine specialization does not publish a private call's argument");
}
