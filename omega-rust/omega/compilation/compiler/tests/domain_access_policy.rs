//! A domain selects the policy that governs it with `access Policy`
//! (`wiki/spec/language/domains.md`, "Domain-owned read and write
//! authority"). The parser reads the clause, name resolution selects the one
//! named implementation it names, and the typed trees carry that selection on
//! the domain, so a reader of `as +D` asks the declaration who may issue it.
//!
//! Two programs pin it from source: one whose policy is declared, and its
//! twin whose policy is declared nowhere.

use typed_trees::domain::DomainIssuance;

// Compilation shares process-wide counters; run one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/fixtures")
        .join(name)
        .join("main.omg")
}

/// Psi 00 through 04 on one fixture, without its Build.
fn typed_frontier(name: &str) -> Result<typed_trees::FrozenTypedTrees, Vec<String>> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let main = fixture(name);
    compiler::CompilationSession::default()
        .typed_frontier_without_build(
            compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            None,
        )
        .map(|frontier| frontier.typed)
        .map_err(|failure| {
            failure
                .into_diagnostics()
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect()
        })
}

#[test]
fn a_domain_carries_the_policy_its_access_clause_selects() {
    let typed = &typed_frontier("pass/domains/access_policy_selected")
        .unwrap_or_else(|diagnostics| panic!("Psi 00 through 04 must complete: {diagnostics:?}"));
    let domain = |name: &str| {
        typed
            .domain_definitions
            .iter()
            .map(|(_, domain)| domain)
            .find(|domain| domain.name.as_str() == name)
            .unwrap_or_else(|| panic!("domain `{name}` is declared"))
    };

    let controlled = domain("Cell::Controlled");
    let DomainIssuance::Planned(policy) = controlled.issuance() else {
        panic!(
            "a domain with an access policy has planned issuance, not {:?}",
            controlled.issuance()
        );
    };
    let implementation = typed.symbols.get(policy.implementation());
    assert_eq!(implementation.kind, symbols::SymbolKind::Conformance);
    assert_eq!(typed.symbols.name(policy.implementation()), "CellAccess");
    // The policy is named bare, so nothing about it is left unserved.
    assert!(typed.deferred_refusal(controlled.symbol).is_none());

    // The same domain without the clause restricts nobody.
    assert_eq!(
        domain("Cell::Plain").issuance(),
        DomainIssuance::Unrestricted
    );
}

/// `access Policy<..>` keeps its selected implementation and bound arguments.
/// Policy application judgment stays deferred on the domain until demanded.
#[test]
fn a_policy_applied_at_static_arguments_is_deferred_on_its_domain() {
    let typed = &typed_frontier("pass/domains/access_policy_applied")
        .unwrap_or_else(|diagnostics| panic!("Psi 00 through 04 must complete: {diagnostics:?}"));
    let controlled = typed
        .domain_definitions
        .iter()
        .map(|(_, domain)| domain)
        .find(|domain| domain.name.as_str() == "Cell::Controlled")
        .expect("the domain is declared");
    let DomainIssuance::Planned(policy) = controlled.issuance() else {
        panic!("the applied policy retains its issuance request");
    };
    assert_eq!(typed.symbols.name(policy.implementation()), "CellAccess");
    let application = policy
        .selection
        .application
        .as_ref()
        .expect("the domain keeps the policy's actual static arguments");
    assert!(application.lifetime_arguments.is_empty());
    assert!(!application.member.is_valid());
    let [argument] = application.arguments.as_ref() else {
        panic!("the policy has exactly its authored Cell argument");
    };
    let cell = typed
        .data_definitions
        .iter()
        .map(|(_, data)| data)
        .find(|data| data.name.as_str() == "Cell")
        .expect("the actual type argument is declared");
    assert_eq!(argument.symbol, cell.symbol);
    let references = typed_trees::references::collect_selected_program_references(
        typed,
        &typed_trees::references::DeclarationSelection::from_symbols([controlled.symbol]),
    );
    assert!(references.symbols.contains(&cell.symbol));
    assert!(references.symbols.contains(&policy.implementation()));
    assert!(
        !references
            .symbols
            .contains(&symbols::SymbolHandle::invalid())
    );
    // Cell must also be retained when it is only a policy argument, not
    // already the domain's own subject type.
    let other = typed
        .domain_definitions
        .iter()
        .map(|(_, domain)| domain)
        .find(|domain| domain.name.as_str() == "Other::Controlled")
        .expect("the domain with a different subject is declared");
    let other_policy = other
        .access_policy
        .as_ref()
        .expect("the policy is selected");
    assert_eq!(
        other_policy
            .selection
            .application
            .as_ref()
            .unwrap()
            .arguments[0]
            .symbol,
        cell.symbol
    );
    let other_references = typed_trees::references::collect_selected_program_references(
        typed,
        &typed_trees::references::DeclarationSelection::from_symbols([other.symbol]),
    );
    assert!(other_references.symbols.contains(&cell.symbol));
    let [name] = argument.path.as_ref() else {
        panic!("the policy retains its one authored Cell path member");
    };
    assert_eq!(name.as_str(), "Cell");
    let source = typed
        .symbols
        .source_file(policy.source_span)
        .expect("the policy selection keeps its actual source file");
    assert_eq!(source.text_at(policy.source_span.span), "CellAccess");
    let refusal = typed
        .deferred_refusal(controlled.symbol)
        .expect("the applied policy is recorded as unserved");
    assert_eq!(
        refusal.request,
        typed_trees::deferred_refusals::DeferredRequest::AccessPolicyApplication
    );
}

/// A literal is part of the selected application but does not name a
/// declaration. Keeping it must not create an invalid dependency.
#[test]
fn a_numeric_policy_argument_keeps_its_value_without_a_declaration_dependency() {
    let typed = &typed_frontier("pass/domains/access_policy_applied")
        .unwrap_or_else(|diagnostics| panic!("Psi 00 through 04 must complete: {diagnostics:?}"));
    let counted = typed
        .domain_definitions
        .iter()
        .map(|(_, domain)| domain)
        .find(|domain| domain.name.as_str() == "Cell::Counted")
        .expect("the domain is declared");
    let DomainIssuance::Planned(policy) = counted.issuance() else {
        panic!("the numeric policy retains its issuance request");
    };
    assert_eq!(typed.symbols.name(policy.implementation()), "NumericAccess");
    let application = policy
        .selection
        .application
        .as_ref()
        .expect("the numeric policy retains its application");
    let [argument] = application.arguments.as_ref() else {
        panic!("the policy retains exactly its authored numeric argument");
    };
    assert_eq!(argument.symbol, symbols::SymbolHandle::invalid());
    assert_eq!(
        argument
            .const_literal
            .as_ref()
            .and_then(|literal| literal.value_u64()),
        Some(7)
    );
    let references = typed_trees::references::collect_selected_program_references(
        typed,
        &typed_trees::references::DeclarationSelection::from_symbols([counted.symbol]),
    );
    assert!(references.symbols.contains(&policy.implementation()));
    assert!(
        !references
            .symbols
            .contains(&symbols::SymbolHandle::invalid())
    );
    let refusal = typed
        .deferred_refusal(counted.symbol)
        .expect("the applied numeric policy remains unserved");
    assert_eq!(
        refusal.request,
        typed_trees::deferred_refusals::DeferredRequest::AccessPolicyApplication
    );
}

#[test]
fn an_access_policy_that_names_no_implementation_is_refused() {
    let Err(diagnostics) = typed_frontier("fail/domains/access_policy_unresolved") else {
        panic!("a policy declared nowhere stops name resolution");
    };
    assert!(
        diagnostics.iter().any(|message| message.contains(
            "domain `Cell::Controlled` access policy `CellAccess` names no implementation"
        )),
        "{diagnostics:?}"
    );
}
