//! A domain selects the policy that governs it with `access Policy`
//! (`wiki/spec/language/domains.md`, "Domain-owned read and write
//! authority"). The parser reads the clause, name resolution selects the one
//! implementation it names, and the typed trees carry that selection on
//! the domain, so a reader of `as +D` asks the declaration who may issue it.
//!
//! Two programs pin it from source: one whose policy is declared, and its
//! twin whose policy is declared nowhere. A demanded domain also carries the
//! plan its policy returned (`build_time_evaluation`, `domain_access_plans`).

use typed_trees::domain::{AccessPlanJudgment, CheckedIssuance, DomainIssuance};

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
    typed_frontier_of(&fixture(name))
}

fn typed_frontier_of(main: &std::path::Path) -> Result<typed_trees::FrozenTypedTrees, Vec<String>> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    compiler::CompilationSession::default()
        .typed_frontier_without_build(
            compiler::CheckedCompileRequest::new(main, Some("linux_x86_64")),
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
    let policy = controlled
        .access_policy
        .as_ref()
        .expect("a domain with an access clause keeps its policy");
    let implementation = typed.symbols.get(policy.implementation());
    assert_eq!(implementation.kind, symbols::SymbolKind::Conformance);
    assert_eq!(typed.symbols.name(policy.implementation()), "CellAccess");
    // The policy is named bare, so nothing about it is left unserved.
    assert!(typed.deferred_refusal(controlled.symbol).is_none());
    // Checking the package demands the domain, so the Build ran its plan.
    // `Issuance::Only { routes: [] }` permits nobody; it is not read as
    // unrestricted.
    let AccessPlanJudgment::Checked(plan) = &policy.plan else {
        panic!(
            "the demanded domain's plan is checked, not {:?}",
            policy.plan
        );
    };
    assert_eq!(typed.symbols.name(plan.plan_machine), "CellAccess::plan");
    assert_eq!(plan.issuance, CheckedIssuance::Only(Vec::new()));
    assert_eq!(controlled.issuance(), DomainIssuance::Only(&[]));
    assert!(!controlled.access_unjudged());

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

/// A plan the Build cannot judge is kept with its reason and shows nobody to
/// be an issuer. `let plan: AccessPlan; plan` returns the zero plan, whose
/// schema is not the subject's.
#[test]
fn a_plan_that_describes_another_schema_is_left_unjudged() {
    let root = std::env::temp_dir().join(format!(
        "omega-domain-access-unjudged-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("staging directory");
    let main = root.join("main.omg");
    std::fs::write(
        &main,
        "\
use omega::language::core::AccessPolicy;
use omega::language::core::AccessPlan;
use omega::language::core::reflect::TypeSchema;

data Seal { mark: i32; }
SealAccess satisfies AccessPolicy {
    machine plan(schema: TypeSchema) -> AccessPlan reaches none crashes none terminates {
        let plan: AccessPlan;
        plan
    }
}
domain Seal::Issued access SealAccess;
machine main() { }
",
    )
    .expect("staged source");
    let typed = typed_frontier_of(&main);
    let _ = std::fs::remove_dir_all(&root);
    let typed = typed
        .unwrap_or_else(|diagnostics| panic!("Psi 00 through 04 must complete: {diagnostics:?}"));
    let issued = typed
        .domain_definitions
        .iter()
        .map(|(_, domain)| domain)
        .find(|domain| domain.name.as_str() == "Seal::Issued")
        .expect("the domain is declared");
    let policy = issued
        .access_policy
        .as_ref()
        .expect("the policy is selected");
    let AccessPlanJudgment::Unjudged(reason) = &policy.plan else {
        panic!("a plan of another schema is not checked: {:?}", policy.plan);
    };
    assert!(reason.contains("another schema"), "{reason}");
    assert!(matches!(issued.issuance(), DomainIssuance::Planned(_)));
    assert!(issued.access_unjudged());
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

/// Result promises demand the selected plans just like displayed qualifications.
#[test]
fn result_membership_promises_demand_their_access_plans() {
    let typed = typed_frontier("run/domains/normal_result_membership")
        .unwrap_or_else(|diagnostics| panic!("typed preparation must complete: {diagnostics:?}"));
    let expected = [
        "Ticket::Stamped",
        "u32::WordStamped",
        "u64::Ticked",
        "addr::Allocated",
    ];
    let mut observed = Vec::new();
    for (_, domain) in typed.domain_definitions.iter() {
        if !expected.contains(&domain.name.as_str()) {
            continue;
        }
        let subject = typed.type_reference_table.type_symbol(domain.target_type);
        if let Some(atom) = typed.symbols.builtin_type_atom(subject) {
            eprintln!(
                "{} schema application: {:?}; declaration name: {:?}; full type: {:?}",
                domain.name.as_str(),
                typed_trees::reflection::construction::schema_type_identity(
                    &typed,
                    domain.target_type,
                    subject
                ),
                language_semantics::type_identity::builtin_type_name(atom),
                language_semantics::type_identity::builtin_type_identity(atom)
            );
        }
        let state = match domain.access_policy.as_ref().map(|policy| &policy.plan) {
            Some(AccessPlanJudgment::Checked(_)) => Ok(()),
            Some(AccessPlanJudgment::Unjudged(reason)) => Err(reason.as_str()),
            Some(AccessPlanJudgment::Unevaluated) => Err("plan was not evaluated"),
            None => Err("selected policy was lost"),
        };
        eprintln!("{}: {state:?}", domain.name.as_str());
        observed.push((domain.name.as_str(), state));
    }
    assert_eq!(observed.len(), expected.len(), "{observed:?}");
    assert!(
        observed.iter().all(|(_, state)| state.is_ok()),
        "{observed:?}"
    );
}

/// Scalar catalogs must not publish permissions they cannot retain and check.
#[test]
fn scalar_access_rules_are_not_erased_during_publication() {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let root = std::env::temp_dir().join(format!(
        "omega-scalar-access-publication-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let main = root.join("main.omg");
    std::fs::write(&main, r#"
use omega::language::core::AccessPolicy;
use omega::language::core::AccessPlan;
use omega::language::core::AccessRegion;
use omega::language::core::Issuance;
use omega::language::core::access;
use omega::language::core::reflect;
use omega::language::core::reflect::TypeSchema;
ScalarAccess satisfies AccessPolicy {
    machine plan(schema: TypeSchema) -> AccessPlan reaches none crashes none terminates {
        let mut plan: AccessPlan = access::new_plan(&schema, Issuance::Only { routes: [reflect::callable(issue)] });
        access::allow_readers(&mut plan, AccessRegion::Subject { schema: schema }, [reflect::callable(issue)]);
        plan
    }
}
domain u64::Restricted access ScalarAccess;
machine issue(value: u64) -> u64 in Restricted { value as +Restricted }
machine main() {
    let held: u64 in Restricted = issue(29u64);
    let copied: u64 in Restricted = held;
}
"#).unwrap();
    let observations = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            let frontier = compiler::CompilationSession::default()
                .typed_frontier_without_build(
                    compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
                    None,
                )
                .unwrap_or_else(|failure| {
                    panic!("typed preparation failed: {:?}", failure.into_diagnostics())
                });
            let controlled = frontier
                .typed
                .domain_definitions
                .iter()
                .map(|(_, row)| row)
                .find(|row| row.name.as_str() == "u64::Restricted")
                .unwrap();
            assert!(
                matches!(controlled.access_policy.as_ref().map(|policy| &policy.plan),
                Some(AccessPlanJudgment::Checked(plan)) if plan.normalized.has_operation_rules()),
                "{:?}",
                controlled.access_policy
            );
            let prepared = typed_trees_to_terminal_psi::prepare_compilation(
                &frontier.typed,
                typed_trees_to_terminal_psi::Compilation {
                    declaration_metadata: &[],
                    entries: &frontier.entries,
                    subjects: &[frontier.subject],
                    purpose: source::DependencyScope::Product,
                    selected: &frontier.selected,
                    calling: frontier
                        .normalization
                        .calling
                        .borrow(&frontier.typed)
                        .selection(),
                    providers: frontier.provider_selections(),
                    bindings: &frontier.bindings,
                    supplies: &frontier.supplies,
                },
                terminal_codec::TerminalPublication::Portable,
            );
            terminal_psi_to_terminal_psi::judge_compilation(prepared.program)
                .into_iter()
                .map(|row| {
                    let refused = matches!(&row,
                        Ok(terminal_psi_to_terminal_psi::RouteVerdict::FallBack(refusal))
                        if refusal.detail == "declaration scalar domain access rules need retained operation checking");
                    (refused, format!("{row:?}"))
                })
                .collect::<Vec<_>>()
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let _ = std::fs::remove_dir_all(root);
    for (_, observation) in &observations {
        eprintln!("{observation}");
    }
    assert!(
        observations.iter().any(|(refused, _)| *refused),
        "{observations:?}"
    );
}
