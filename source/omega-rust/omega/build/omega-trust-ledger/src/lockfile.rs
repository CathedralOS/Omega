//! GR4 (the chapter-10 carrier's lockfile): "the build lockfile -- the same
//! machine-written lockfile that pins package resolution; one receipt file,
//! not two -- records the statement hash automatically; a statement that
//! drifts under a grant fails the build until re-approved."
//!
//! Legacy standalone scope: the lockfile holds only TRUST RECEIPTS -- one row
//! per exact selected-provider or accepted-machine grant,
//! `<identity hex>  <commitment>` -- and
//! lives beside the project's build.omg (`omega.lock`, machine-written; it
//! must persist ACROSS builds to see drift). A project with no grants gets
//! no lockfile. Provider plans retain selected-plan identity; generic accepted
//! axioms retain universal template identity; and non-generic accepted axioms
//! defer to the exact checked machine-contract fingerprint. Domain names and
//! unmatched strings cannot manufacture receipts. Package-aware compilation
//! rejects individual accepted-machine grants because package claims require
//! complete package-level admission. Re-approval remains legacy standalone
//! behavior: delete the stale row (or file); the error names it.

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use std::collections::BTreeMap;

pub struct PreparedTrustLock {
    lock_path: Option<std::path::PathBuf>,
    rows: Vec<PreparedTrustReceipt>,
}

struct PreparedTrustReceipt {
    commitment: String,
    identity: PreparedTrustIdentity,
}

enum PreparedTrustIdentity {
    Ready(u64),
    AcceptedMachine(psi_symbols::SymbolHandle),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NonProviderTrustGrant {
    AcceptedMachine(psi_symbols::SymbolHandle),
}

struct NonProviderTrustGrantCandidate<'name> {
    subject: NonProviderTrustGrant,
    kind: &'static str,
    name: &'name str,
}

pub(super) fn resolve_non_provider_trust_grant(
    typed: &TypedTrees,
    grant: &str,
) -> Result<NonProviderTrustGrant, Diagnostic> {
    let candidates = typed
        .machines()
        .iter()
        .filter(|machine| grantable_accepted_machine(typed, machine))
        .map(|machine| NonProviderTrustGrantCandidate {
            subject: NonProviderTrustGrant::AcceptedMachine(machine.symbol),
            kind: "accepted machine",
            name: machine.name.as_str(),
        })
        .collect::<Vec<_>>();
    let exact = candidates
        .iter()
        .filter(|candidate| candidate.name == grant)
        .collect::<Vec<_>>();
    match exact.as_slice() {
        [candidate] => return validate_grant_subject(grant, candidate),
        [] => {}
        _ => return Err(ambiguous_grant(grant, &exact)),
    }
    if !grant.contains("::") {
        let leaf = candidates
            .iter()
            .filter(|candidate| candidate.name.rsplit("::").next() == Some(grant))
            .collect::<Vec<_>>();
        match leaf.as_slice() {
            [candidate] => return validate_grant_subject(grant, candidate),
            [] => {}
            _ => return Err(ambiguous_grant(grant, &leaf)),
        }
    }
    Err(Diagnostic::error(format!(
        "root grant `{grant}` does not name an exact accepted machine or selected provider plan; domain and arbitrary-string trust grants are unsupported",
    )))
}

fn grantable_accepted_machine(
    typed: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
) -> bool {
    machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
        && !typed.machine_specializations.iter().any(|specialization| {
            specialization.accepted_template_commitment.is_some()
                && specialization.instance == machine.symbol
                && specialization.instance != specialization.template
        })
}

fn validate_grant_subject(
    grant: &str,
    candidate: &NonProviderTrustGrantCandidate<'_>,
) -> Result<NonProviderTrustGrant, Diagnostic> {
    let NonProviderTrustGrant::AcceptedMachine(symbol) = candidate.subject;
    if !symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "root grant `{grant}` resolves to {} `{}` with no valid exact symbol",
            candidate.kind, candidate.name,
        )));
    }
    Ok(candidate.subject)
}

fn ambiguous_grant(grant: &str, candidates: &[&NonProviderTrustGrantCandidate<'_>]) -> Diagnostic {
    let mut names = candidates
        .iter()
        .map(|candidate| format!("{} `{}`", candidate.kind, candidate.name))
        .collect::<Vec<_>>();
    names.sort();
    Diagnostic::error(format!(
        "root grant `{grant}` is ambiguous across non-provider trust subjects: {}",
        names.join(", "),
    ))
}

fn accepted_machine(
    typed: &TypedTrees,
    symbol: psi_symbols::SymbolHandle,
) -> Result<&psi_typed_trees::machine::Machine, Diagnostic> {
    let machines = typed
        .machines()
        .iter()
        .filter(|machine| grantable_accepted_machine(typed, machine) && machine.symbol == symbol)
        .collect::<Vec<_>>();
    let [machine] = machines.as_slice() else {
        return Err(Diagnostic::error(match machines.len() {
            0 => {
                format!("granted accepted-machine symbol {symbol:?} has no exact typed definition")
            }
            count => format!(
                "granted accepted-machine symbol {symbol:?} has {count} exact typed definitions"
            ),
        }));
    };
    Ok(*machine)
}

pub fn reject_package_non_provider_grants(
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    for grant in root_grants {
        if provider_grants
            .iter()
            .any(|provider_grant| provider_grant.selector == *grant)
        {
            continue;
        }
        match resolve_non_provider_trust_grant(typed, grant)
            .map_err(|diagnostic| vec![diagnostic])?
        {
            NonProviderTrustGrant::AcceptedMachine(_) => {
                return Err(vec![Diagnostic::error(format!(
                    "package-aware compilation cannot admit individual accepted machine `{grant}`; package claims require complete package-level review",
                ))]);
            }
        }
    }
    Ok(())
}

pub fn prepare_trust_lockfile(
    root_path: &std::path::Path,
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
    accepted_template_classifications: &crate::AcceptedTemplateClassifications,
    package_aware: bool,
) -> Result<PreparedTrustLock, Vec<Diagnostic>> {
    if package_aware {
        reject_package_non_provider_grants(
            typed,
            root_grants,
            provider_plans,
            selected_provider_plans,
        )?;
    }
    let Some(project_dir) = root_path.parent() else {
        return Ok(PreparedTrustLock {
            lock_path: None,
            rows: Vec::new(),
        });
    };
    let lock_path = project_dir.join("omega.lock");

    // Current receipts.
    let mut rows: Vec<PreparedTrustReceipt> = Vec::new();
    let provider_grants = crate::resolve_selected_provider_grants(
        provider_plans,
        selected_provider_plans,
        root_grants,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    for grant in root_grants {
        // A grant naming a DERIVED PROVIDER PLAN (by plan name or boundary
        // slot) pins the SELECTED plan's NORMALIZED IDENTITY. Slot grants use
        // a slot-stable commitment key so changing the selected provider is
        // itself trust drift rather than a silent replacement lock row.
        if let Some(provider_grant) = provider_grants
            .iter()
            .find(|provider_grant| provider_grant.selector == *grant)
        {
            let commitment = provider_grant.commitment();
            if !rows.iter().any(|row| row.commitment == commitment) {
                rows.push(PreparedTrustReceipt {
                    commitment,
                    identity: PreparedTrustIdentity::Ready(provider_grant.selected_plan_identity),
                });
            }
            continue;
        }
        let (commitment, identity) = match resolve_non_provider_trust_grant(typed, grant)
            .map_err(|diagnostic| vec![diagnostic])?
        {
            NonProviderTrustGrant::AcceptedMachine(symbol) => {
                // MP5: a generic accepted axiom is granted ONCE at its universal
                // normalized template. Every concrete specialization references this
                // receipt; none creates another grant row. The template identity
                // includes its machine-parameter requirements, so changing a `where
                // machine` contract drifts the existing receipt before any instance
                // can reuse it.
                let machine =
                    accepted_machine(typed, symbol).map_err(|diagnostic| vec![diagnostic])?;
                let identity = accepted_template_classifications
                    .for_machine(machine.symbol, machine.name.as_str())
                    .map_err(|diagnostic| vec![diagnostic])?
                    .map(PreparedTrustIdentity::Ready)
                    .unwrap_or(PreparedTrustIdentity::AcceptedMachine(machine.symbol));
                (
                    format!("accepted fact: {}", machine.name.as_str()),
                    identity,
                )
            }
        };
        if !rows.iter().any(|row| row.commitment == commitment) {
            rows.push(PreparedTrustReceipt {
                commitment,
                identity,
            });
        }
    }

    Ok(PreparedTrustLock {
        lock_path: Some(lock_path),
        rows,
    })
}

pub fn enforce_trust_lockfile(
    prepared: PreparedTrustLock,
    checked: &psi_checked_trees::CheckedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let Some(lock_path) = prepared.lock_path else {
        return Ok(());
    };
    let rows = resolve_receipts(prepared.rows, checked)?;

    let existing = match std::fs::read_to_string(&lock_path) {
        Ok(existing) => Some(existing),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                lock_path.display()
            ))]);
        }
    };
    if let Some(existing) = existing.as_deref() {
        let pinned = parse_trust_lock(&existing, &lock_path)?;
        validate_complete_receipt_set(&pinned, &rows, &lock_path)?;
    } else if rows.is_empty() {
        return Ok(());
    }

    let output = render_trust_lock(&rows);
    if existing.as_deref() == Some(output.as_str()) {
        return Ok(());
    }
    std::fs::write(&lock_path, output).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to write {}: {error}",
            lock_path.display()
        ))]
    })
}

fn resolve_receipts(
    rows: Vec<PreparedTrustReceipt>,
    checked: &psi_checked_trees::CheckedTrees,
) -> Result<BTreeMap<String, u64>, Vec<Diagnostic>> {
    let mut resolved = BTreeMap::new();
    for row in rows {
        let identity = match row.identity {
            PreparedTrustIdentity::Ready(identity) => identity,
            PreparedTrustIdentity::AcceptedMachine(machine) => {
                let mut matches = checked
                    .facts
                    .contract_plans
                    .machines
                    .iter()
                    .filter(|plan| plan.machine == machine);
                let plan = matches.next().ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "accepted trust receipt `{}` has no exact checked machine contract plan",
                        row.commitment
                    ))]
                })?;
                if matches.next().is_some() {
                    return Err(vec![Diagnostic::error(format!(
                        "accepted trust receipt `{}` has duplicate exact checked machine contract plans",
                        row.commitment
                    ))]);
                }
                plan.fingerprint
            }
        };
        if resolved.insert(row.commitment.clone(), identity).is_some() {
            return Err(vec![Diagnostic::error(format!(
                "current trust receipt set contains duplicate commitment `{}`",
                row.commitment
            ))]);
        }
    }
    Ok(resolved)
}

fn parse_trust_lock(
    input: &str,
    lock_path: &std::path::Path,
) -> Result<BTreeMap<String, u64>, Vec<Diagnostic>> {
    if input.is_empty() {
        return Ok(BTreeMap::new());
    }
    if !input.is_empty() && !input.ends_with('\n') {
        return Err(malformed_lock_row(lock_path, input.lines().count().max(1)));
    }
    let mut rows = BTreeMap::new();
    let mut previous_commitment: Option<&str> = None;
    let body = input
        .strip_suffix('\n')
        .expect("the nonempty lock was required to end in a newline");
    for (index, line) in body.split('\n').enumerate() {
        let line_number = index + 1;
        let Some(hash_text) = line.get(..16) else {
            return Err(malformed_lock_row(lock_path, line_number));
        };
        if !hash_text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || line.get(16..18) != Some("  ")
        {
            return Err(malformed_lock_row(lock_path, line_number));
        }
        let Some(commitment) = line.get(18..) else {
            return Err(malformed_lock_row(lock_path, line_number));
        };
        if commitment.is_empty()
            || commitment.trim() != commitment
            || commitment.chars().any(char::is_control)
        {
            return Err(malformed_lock_row(lock_path, line_number));
        }
        let identity = u64::from_str_radix(hash_text, 16)
            .map_err(|_| malformed_lock_row(lock_path, line_number))?;
        if rows.contains_key(commitment) {
            return Err(vec![Diagnostic::error(format!(
                "trust lock {} contains duplicate commitment `{commitment}` on line {line_number}",
                lock_path.display()
            ))]);
        }
        if previous_commitment.is_some_and(|previous| previous >= commitment) {
            return Err(vec![Diagnostic::error(format!(
                "trust lock {} is not in canonical commitment order at line {line_number}",
                lock_path.display()
            ))]);
        }
        rows.insert(commitment.to_owned(), identity);
        previous_commitment = Some(commitment);
    }
    Ok(rows)
}

fn malformed_lock_row(lock_path: &std::path::Path, line_number: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "trust lock {} has a malformed v1 receipt row on line {line_number}",
        lock_path.display()
    ))]
}

fn validate_complete_receipt_set(
    pinned: &BTreeMap<String, u64>,
    current: &BTreeMap<String, u64>,
    lock_path: &std::path::Path,
) -> Result<(), Vec<Diagnostic>> {
    let added = current
        .keys()
        .filter(|commitment| !pinned.contains_key(*commitment))
        .cloned()
        .collect::<Vec<_>>();
    let removed = pinned
        .keys()
        .filter(|commitment| !current.contains_key(*commitment))
        .cloned()
        .collect::<Vec<_>>();
    let changed = current
        .iter()
        .filter_map(|(commitment, identity)| {
            pinned
                .get(commitment)
                .filter(|pinned_identity| *pinned_identity != identity)
                .map(|pinned_identity| {
                    format!("{commitment} ({pinned_identity:016x} -> {identity:016x})")
                })
        })
        .collect::<Vec<_>>();
    if added.is_empty() && removed.is_empty() && changed.is_empty() {
        return Ok(());
    }
    Err(vec![Diagnostic::error(format!(
        "granted statement drifted: the complete trust receipt set no longer matches {} -- added: {}; removed: {}; changed: {} -- delete the stale lock to re-approve",
        lock_path.display(),
        display_diff_entries(&added),
        display_diff_entries(&removed),
        display_diff_entries(&changed),
    ))])
}

fn display_diff_entries(entries: &[String]) -> String {
    if entries.is_empty() {
        "none".to_owned()
    } else {
        entries.join(", ")
    }
}

fn render_trust_lock(rows: &BTreeMap<String, u64>) -> String {
    let mut output = String::new();
    for (commitment, identity) in rows {
        output.push_str(&format!("{identity:016x}  {commitment}\n"));
    }
    output
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use psi_checked_trees::{CheckedTrees, CrashPlan, MachineContractPlan};
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;

    use super::{
        NonProviderTrustGrant, PreparedTrustIdentity, PreparedTrustLock, PreparedTrustReceipt,
        enforce_trust_lockfile, parse_trust_lock, prepare_trust_lockfile, render_trust_lock,
        resolve_non_provider_trust_grant, resolve_receipts, validate_complete_receipt_set,
    };

    fn receipt(commitment: &str, identity: u64) -> PreparedTrustReceipt {
        PreparedTrustReceipt {
            commitment: commitment.to_owned(),
            identity: PreparedTrustIdentity::Ready(identity),
        }
    }

    fn contract(machine: SymbolHandle, fingerprint: u64) -> MachineContractPlan {
        MachineContractPlan {
            machine,
            closed_scalar_values: Default::default(),
            crash: CrashPlan::default(),
            fingerprint,
        }
    }

    fn typed_subjects(
        domains: &[(&str, u32, bool)],
        accepted_machines: &[(&str, u32)],
    ) -> TypedTrees {
        let mut typed = TypedTrees::default();
        for (name, symbol_index, retained_semantic_identity) in domains {
            let semantic_id = if *retained_semantic_identity {
                typed.semantic_domains.intern(*name)
            } else {
                psi_language_semantics::SemanticDomainId::NULL
            };
            typed.push_domain_definition(psi_typed_trees::domain::DomainDefinition {
                symbol: SymbolHandle::from_arena_index(*symbol_index),
                name: psi_typed_trees::name::Identifier::generated(*name),
                semantic_id,
                ..Default::default()
            });
        }
        for (name, symbol_index) in accepted_machines {
            typed.push_machine(psi_typed_trees::machine::Machine {
                symbol: SymbolHandle::from_arena_index(*symbol_index),
                name: psi_typed_trees::name::Identifier::generated(*name),
                supply_mode: psi_language_semantics::MachineSupplyMode::Accepted,
                ..Default::default()
            });
        }
        typed
    }

    #[test]
    fn non_provider_grants_require_one_exact_global_subject() {
        enum Expected {
            Subject(NonProviderTrustGrant),
            Error(&'static str),
        }
        let cases = [
            (
                "exact qualified precedence",
                typed_subjects(&[], &[("first::claim", 1), ("second::claim", 2)]),
                "first::claim",
                Expected::Subject(NonProviderTrustGrant::AcceptedMachine(
                    SymbolHandle::from_arena_index(1),
                )),
            ),
            (
                "unique short leaf",
                typed_subjects(&[], &[("proof::claim", 1)]),
                "claim",
                Expected::Subject(NonProviderTrustGrant::AcceptedMachine(
                    SymbolHandle::from_arena_index(1),
                )),
            ),
            (
                "ambiguous accepted-machine leaf",
                typed_subjects(&[], &[("first::claim", 1), ("second::claim", 2)]),
                "claim",
                Expected::Error("ambiguous across non-provider trust subjects"),
            ),
            (
                "unique accepted-machine leaf",
                typed_subjects(&[], &[("proof::claim", 1)]),
                "claim",
                Expected::Subject(NonProviderTrustGrant::AcceptedMachine(
                    SymbolHandle::from_arena_index(1),
                )),
            ),
            (
                "duplicate exact name",
                typed_subjects(&[], &[("proof::claim", 1), ("proof::claim", 2)]),
                "proof::claim",
                Expected::Error("ambiguous across non-provider trust subjects"),
            ),
            (
                "domain is not a trust subject",
                typed_subjects(&[("u32::Meters", 1, true)], &[]),
                "u32::Meters",
                Expected::Error("domain and arbitrary-string trust grants are unsupported"),
            ),
            (
                "qualified nonmatch does not fall back",
                typed_subjects(&[], &[("proof::claim", 1)]),
                "other::claim",
                Expected::Error("domain and arbitrary-string trust grants are unsupported"),
            ),
            (
                "unmatched",
                typed_subjects(&[], &[]),
                "ExternalClaim",
                Expected::Error("domain and arbitrary-string trust grants are unsupported"),
            ),
        ];

        for (case, typed, grant, expected) in cases {
            let actual = resolve_non_provider_trust_grant(&typed, grant);
            match expected {
                Expected::Subject(expected) => {
                    assert_eq!(actual, Ok(expected), "case: {case}");
                }
                Expected::Error(expected) => {
                    let diagnostic = actual.expect_err("ambiguous grant must reject");
                    assert!(
                        diagnostic.message.contains(expected),
                        "case: {case}; diagnostic: {diagnostic:?}",
                    );
                }
            }
        }
    }

    #[test]
    fn package_aware_compilation_rejects_individual_accepted_machine_grants() {
        let typed = typed_subjects(&[], &[("proof::claim", 1)]);
        let result = prepare_trust_lockfile(
            std::path::Path::new("/tmp/omega-package/main.omg"),
            &typed,
            &["proof::claim".to_owned()],
            &[],
            &omega_effects::SelectedProviderPlanFacts::default(),
            &crate::AcceptedTemplateClassifications::capture(&typed),
            true,
        );
        let diagnostics = match result {
            Err(diagnostics) => diagnostics,
            Ok(_) => {
                panic!("package admission must not be minted by one accepted-machine selector")
            }
        };
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("package claims require complete package-level review")
        );
    }

    #[test]
    fn deferred_accepted_receipt_fails_closed_without_exact_checked_plan() {
        let commitment = "accepted fact: admitted".to_owned();
        let machine = SymbolHandle::from_arena_index(1);
        let result = resolve_receipts(
            vec![PreparedTrustReceipt {
                commitment: commitment.clone(),
                identity: PreparedTrustIdentity::AcceptedMachine(machine),
            }],
            &CheckedTrees::default(),
        );

        let diagnostics = result.expect_err("missing exact checked plan must fail closed");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message.as_str(),
            "accepted trust receipt `accepted fact: admitted` has no exact checked machine contract plan"
        );

        let mut duplicate = CheckedTrees::default();
        duplicate
            .facts
            .contract_plans
            .machines
            .push(contract(machine, 1));
        duplicate
            .facts
            .contract_plans
            .machines
            .push(contract(machine, 2));
        let diagnostics = resolve_receipts(
            vec![PreparedTrustReceipt {
                commitment: commitment.clone(),
                identity: PreparedTrustIdentity::AcceptedMachine(machine),
            }],
            &duplicate,
        )
        .expect_err("duplicate exact checked plans must fail closed");
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("duplicate exact checked machine contract plans")
        );

        let unrelated = SymbolHandle::from_arena_index(2);
        let mut exact = CheckedTrees::default();
        exact
            .facts
            .contract_plans
            .machines
            .push(contract(machine, 0x1234));
        exact
            .facts
            .contract_plans
            .machines
            .push(contract(unrelated, 1));
        exact
            .facts
            .contract_plans
            .machines
            .push(contract(unrelated, 2));
        assert_eq!(
            resolve_receipts(
                vec![PreparedTrustReceipt {
                    commitment,
                    identity: PreparedTrustIdentity::AcceptedMachine(machine),
                }],
                &exact,
            ),
            Ok(BTreeMap::from([(
                "accepted fact: admitted".to_owned(),
                0x1234
            )]))
        );
    }

    #[test]
    fn current_receipts_reject_duplicate_commitments() {
        let diagnostics = resolve_receipts(
            vec![
                receipt("accepted fact: duplicate", 1),
                receipt("accepted fact: duplicate", 1),
            ],
            &CheckedTrees::default(),
        )
        .expect_err("duplicate current commitments must fail closed");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("duplicate commitment"));
    }

    #[test]
    fn v1_parser_accepts_only_canonical_unique_sorted_rows() {
        let path = std::path::Path::new("/tmp/omega-lock-parser/omega.lock");
        let valid =
            "0000000000000001  accepted fact: Alpha\n0000000000000002  accepted fact: Beta\n";
        assert_eq!(
            parse_trust_lock(valid, path),
            Ok(BTreeMap::from([
                ("accepted fact: Alpha".to_owned(), 1),
                ("accepted fact: Beta".to_owned(), 2),
            ]))
        );

        let malformed = [
            "0  accepted fact: Alpha\n",
            "000000000000000g  accepted fact: Alpha\n",
            "000000000000000A  accepted fact: Alpha\n",
            "0000000000000001 accepted fact: Alpha\n",
            "0000000000000001   accepted fact: Alpha\n",
            "0000000000000001  \n",
            "0000000000000001   accepted fact: Alpha\n",
            "0000000000000001  accepted fact: Alpha \n",
            "\n",
            "0000000000000001  accepted fact: Alpha\r\n",
            "0000000000000001  accepted fact: Alpha",
        ];
        for input in malformed {
            let error = parse_trust_lock(input, path)
                .expect_err("malformed v1 receipt row must fail closed");
            assert_eq!(error.len(), 1, "input: {input:?}");
            assert!(error[0].message.contains("malformed v1 receipt row"));
        }

        let duplicate =
            "0000000000000001  accepted fact: Alpha\n0000000000000001  accepted fact: Alpha\n";
        let error = parse_trust_lock(duplicate, path)
            .expect_err("duplicate pinned commitment must fail closed");
        assert!(error[0].message.contains("duplicate commitment"));

        let unsorted =
            "0000000000000002  accepted fact: Beta\n0000000000000001  accepted fact: Alpha\n";
        let error =
            parse_trust_lock(unsorted, path).expect_err("noncanonical row order must fail closed");
        assert!(error[0].message.contains("canonical commitment order"));
    }

    #[test]
    fn complete_receipt_set_reports_added_removed_and_changed_rows() {
        let path = std::path::Path::new("/tmp/omega-lock-diff/omega.lock");
        let pinned = BTreeMap::from([
            ("accepted fact: Changed".to_owned(), 1),
            ("accepted fact: Removed".to_owned(), 2),
            ("accepted fact: Stable".to_owned(), 3),
        ]);
        let current = BTreeMap::from([
            ("accepted fact: Added".to_owned(), 4),
            ("accepted fact: Changed".to_owned(), 5),
            ("accepted fact: Stable".to_owned(), 3),
        ]);
        let error = validate_complete_receipt_set(&pinned, &current, path)
            .expect_err("complete set drift must fail closed");
        assert_eq!(error.len(), 1);
        assert!(error[0].message.contains("added: accepted fact: Added"));
        assert!(error[0].message.contains("removed: accepted fact: Removed"));
        assert!(
            error[0]
                .message
                .contains("changed: accepted fact: Changed (0000000000000001 -> 0000000000000005)")
        );
        assert_eq!(
            validate_complete_receipt_set(&current, &current, path),
            Ok(())
        );
    }

    #[test]
    fn enforcement_never_rewrites_a_drifted_or_malformed_lock() {
        let root =
            std::env::temp_dir().join(format!("omega-lock-enforcement-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create lock test directory");
        let lock_path = root.join("omega.lock");

        let pinned = "0000000000000001  accepted fact: Existing\n";
        std::fs::write(&lock_path, pinned).expect("write pinned lock");
        let drift = enforce_trust_lockfile(
            PreparedTrustLock {
                lock_path: Some(lock_path.clone()),
                rows: Vec::new(),
            },
            &CheckedTrees::default(),
        )
        .expect_err("removing the final receipt must fail closed");
        assert!(
            drift[0]
                .message
                .contains("removed: accepted fact: Existing")
        );
        assert_eq!(
            std::fs::read_to_string(&lock_path).expect("read preserved lock"),
            pinned
        );

        let malformed = "not a v1 lock\n";
        std::fs::write(&lock_path, malformed).expect("write malformed lock");
        enforce_trust_lockfile(
            PreparedTrustLock {
                lock_path: Some(lock_path.clone()),
                rows: vec![receipt("accepted fact: Existing", 1)],
            },
            &CheckedTrees::default(),
        )
        .expect_err("malformed lock must fail closed");
        assert_eq!(
            std::fs::read_to_string(&lock_path).expect("read preserved malformed lock"),
            malformed
        );

        std::fs::remove_dir_all(&root).expect("remove lock test directory");
    }

    #[test]
    fn deterministic_output_sorts_rows_and_empty_first_approval_writes_nothing() {
        let rows = BTreeMap::from([
            ("accepted fact: Beta".to_owned(), 2),
            ("accepted fact: Alpha".to_owned(), 1),
        ]);
        assert_eq!(
            render_trust_lock(&rows),
            "0000000000000001  accepted fact: Alpha\n0000000000000002  accepted fact: Beta\n"
        );

        let root =
            std::env::temp_dir().join(format!("omega-lock-empty-approval-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create empty lock test directory");
        let lock_path = root.join("omega.lock");
        enforce_trust_lockfile(
            PreparedTrustLock {
                lock_path: Some(lock_path.clone()),
                rows: Vec::new(),
            },
            &CheckedTrees::default(),
        )
        .expect("empty first approval");
        assert!(!lock_path.exists());
        std::fs::remove_dir_all(&root).expect("remove empty lock test directory");
    }

    #[test]
    fn identical_valid_lock_is_not_rewritten() {
        let root =
            std::env::temp_dir().join(format!("omega-lock-identical-noop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create identical lock test directory");
        let lock_path = root.join("omega.lock");
        let pinned = "0000000000000001  accepted fact: Existing\n";
        std::fs::write(&lock_path, pinned).expect("write identical pinned lock");

        let mut permissions = std::fs::metadata(&lock_path)
            .expect("read identical lock metadata")
            .permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&lock_path, permissions).expect("make identical lock read-only");

        enforce_trust_lockfile(
            PreparedTrustLock {
                lock_path: Some(lock_path.clone()),
                rows: vec![receipt("accepted fact: Existing", 1)],
            },
            &CheckedTrees::default(),
        )
        .expect("an identical validated lock must require no write access");
        assert_eq!(
            std::fs::read_to_string(&lock_path).expect("read unchanged identical lock"),
            pinned
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&lock_path, std::fs::Permissions::from_mode(0o600))
                .expect("restore identical lock permissions");
        }
        #[cfg(windows)]
        {
            let mut permissions = std::fs::metadata(&lock_path)
                .expect("read identical lock metadata for cleanup")
                .permissions();
            permissions.set_readonly(false);
            std::fs::set_permissions(&lock_path, permissions)
                .expect("restore identical lock permissions");
        }
        std::fs::remove_dir_all(&root).expect("remove identical lock test directory");
    }
}
