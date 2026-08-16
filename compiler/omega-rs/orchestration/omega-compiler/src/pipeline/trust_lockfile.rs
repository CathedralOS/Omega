//! GR4 (the chapter-10 carrier's lockfile): "the build lockfile -- the same
//! machine-written lockfile that pins package resolution; one receipt file,
//! not two -- records the statement hash automatically; a statement that
//! drifts under a grant fails the build until re-approved."
//!
//! v1 scope (packages do not exist yet): the lockfile holds only TRUST
//! RECEIPTS -- one row per root grant, `<fnv1a hex>  <commitment>` -- and
//! lives beside the project's build.omg (`omega.lock`, machine-written; it
//! must persist ACROSS builds to see drift). A project with no grants gets
//! no lockfile. Domains and unmatched grants retain their FNV-1a statement
//! identity; provider plans retain selected-plan identity; generic accepted
//! axioms retain universal template identity; and non-generic accepted axioms
//! defer to the exact checked machine-contract fingerprint. Re-approval v1:
//! delete the stale row (or the file); the error names it. The `defer`-tooling
//! item owns the one-command re-approve UX.

use crate::pipeline::compile_options::CompileOptions;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use std::collections::BTreeMap;

pub(super) struct PreparedTrustLock {
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

fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn domain_commitment_statement(typed: &TypedTrees, grant: &str) -> Option<(String, String)> {
    for domain in typed.domain_definitions() {
        if !domain.semantic_id.is_valid() {
            continue;
        }
        let leaf = domain
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(domain.name.as_str());
        if grant == domain.name.as_str() || grant == leaf {
            let mut statement = format!("domain {}", domain.name.as_str());
            for fact in typed.proof_facts.span_or_empty(domain.facts) {
                if let psi_typed_trees::domain::ProofFact::Expression(expression) = fact {
                    statement.push_str("; ");
                    statement.push_str(&typed.expression_table.display_name(*expression));
                }
            }
            return Some((
                format!("domain introduction: {}", domain.name.as_str()),
                statement,
            ));
        }
    }
    None
}

fn commitment_statement(typed: &TypedTrees, grant: &str) -> (String, String) {
    if let Some(domain) = domain_commitment_statement(typed, grant) {
        return domain;
    }
    (format!("accepted fact: {grant}"), grant.to_owned())
}

pub(super) fn prepare_trust_lockfile(
    options: &CompileOptions,
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<PreparedTrustLock, Vec<Diagnostic>> {
    let Some(project_dir) = options.root_path.parent() else {
        return Ok(PreparedTrustLock {
            lock_path: None,
            rows: Vec::new(),
        });
    };
    let lock_path = project_dir.join("omega.lock");

    // Current receipts.
    let mut rows: Vec<PreparedTrustReceipt> = Vec::new();
    for grant in root_grants {
        // A grant naming a DERIVED PROVIDER PLAN (by plan name or boundary
        // slot) pins the SELECTED plan's NORMALIZED IDENTITY. Slot grants use
        // a slot-stable commitment key so changing the selected provider is
        // itself trust drift rather than a silent replacement lock row.
        if let Some(plan) = crate::pipeline::provider_plans::selected_provider_plan_for_grant(
            provider_plans,
            selected_provider_plans,
            grant,
        )
        .map_err(|diagnostic| vec![diagnostic])?
        {
            let commitment = if grant == &plan.name {
                format!("provider plan: {}", plan.name)
            } else {
                format!("provider slot: {}", plan.schema.trait_name)
            };
            if !rows.iter().any(|row| row.commitment == commitment) {
                rows.push(PreparedTrustReceipt {
                    commitment,
                    identity: PreparedTrustIdentity::Ready(plan.identity_fingerprint()),
                });
            }
            continue;
        }
        // MP5: a generic accepted axiom is granted ONCE at its universal
        // normalized template. Every concrete specialization references this
        // receipt; none creates another grant row. The template identity
        // includes its machine-parameter requirements, so changing a `where
        // machine` contract drifts the existing receipt before any instance
        // can reuse it.
        let accepted_machine = typed.machines().iter().find(|machine| {
            machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
                && (grant == machine.name.as_str()
                    || Some(grant.as_str()) == machine.name.as_str().rsplit("::").next())
        });
        if let Some((machine, identity)) = accepted_machine.and_then(|machine| {
            psi_typed_trees_to_checked_trees::generic_machine_template_fingerprint(
                typed,
                machine.symbol,
            )
            .map(|identity| (machine, identity))
        }) {
            let commitment = format!("accepted fact: {}", machine.name.as_str());
            if !rows.iter().any(|row| row.commitment == commitment) {
                rows.push(PreparedTrustReceipt {
                    commitment,
                    identity: PreparedTrustIdentity::Ready(identity),
                });
            }
            continue;
        }
        if let Some((commitment, statement)) = domain_commitment_statement(typed, grant) {
            if !rows.iter().any(|row| row.commitment == commitment) {
                rows.push(PreparedTrustReceipt {
                    commitment,
                    identity: PreparedTrustIdentity::Ready(fnv1a(&statement)),
                });
            }
            continue;
        }
        if let Some(machine) = accepted_machine {
            let commitment = format!("accepted fact: {}", machine.name.as_str());
            if !rows.iter().any(|row| row.commitment == commitment) {
                rows.push(PreparedTrustReceipt {
                    commitment,
                    identity: PreparedTrustIdentity::AcceptedMachine(machine.symbol),
                });
            }
            continue;
        }
        let (commitment, statement) = commitment_statement(typed, grant);
        if !rows.iter().any(|row| row.commitment == commitment) {
            rows.push(PreparedTrustReceipt {
                commitment,
                identity: PreparedTrustIdentity::Ready(fnv1a(&statement)),
            });
        }
    }

    Ok(PreparedTrustLock {
        lock_path: Some(lock_path),
        rows,
    })
}

pub(super) fn enforce_trust_lockfile(
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
    if let Some(existing) = existing {
        let pinned = parse_trust_lock(&existing, &lock_path)?;
        validate_complete_receipt_set(&pinned, &rows, &lock_path)?;
    } else if rows.is_empty() {
        return Ok(());
    }

    let output = render_trust_lock(&rows);
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

    use super::{
        PreparedTrustIdentity, PreparedTrustLock, PreparedTrustReceipt, enforce_trust_lockfile,
        parse_trust_lock, render_trust_lock, resolve_receipts, validate_complete_receipt_set,
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
}
