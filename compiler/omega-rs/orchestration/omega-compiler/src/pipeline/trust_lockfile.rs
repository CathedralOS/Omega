//! GR4 (the chapter-10 carrier's lockfile): "the build lockfile -- the same
//! machine-written lockfile that pins package resolution; one receipt file,
//! not two -- records the statement hash automatically; a statement that
//! drifts under a grant fails the build until re-approved."
//!
//! v1 scope (packages do not exist yet): the lockfile holds only TRUST
//! RECEIPTS -- one row per root grant, `<fnv1a hex>  <commitment>` -- and
//! lives beside the project's build.omg (`omega.lock`, machine-written; it
//! must persist ACROSS builds to see drift). A project with no grants gets
//! no lockfile. The hash is FNV-1a over the commitment's rendered statement
//! (a domain grant hashes the domain's name + rendered facts; an
//! accepted-fact grant hashes its path until boundary-machine statements
//! carry bodies) -- implemented inline so the hash never varies across Rust
//! releases. Re-approval v1: delete the stale row (or the file); the error
//! names it. The `defer`-tooling item owns the one-command re-approve UX.

use crate::pipeline::compile_options::CompileOptions;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn commitment_statement(typed: &TypedTrees, grant: &str) -> (String, String) {
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
            return (
                format!("domain introduction: {}", domain.name.as_str()),
                statement,
            );
        }
    }
    // An ACCEPTED machine (bodyless boundary axiom, GR6d): the receipt
    // hashes the axiom's rendered ensures -- the statement the grant
    // covers; editing the claim under a grant is drift.
    for machine in typed.machines() {
        if machine.supply_mode != psi_language_semantics::MachineSupplyMode::Accepted {
            continue;
        }
        let leaf = machine
            .name
            .as_str()
            .rsplit("::")
            .next()
            .unwrap_or(machine.name.as_str());
        if grant != machine.name.as_str() && grant != leaf {
            continue;
        }
        let mut statement = format!("boundary machine {}", machine.name.as_str());
        for contract in typed.machine_contracts(machine) {
            if !matches!(
                contract.kind,
                psi_typed_trees::signature::SignatureContractKind::Ensures
            ) {
                continue;
            }
            for fact in typed.proof_facts.span_or_empty(contract.facts) {
                if let psi_typed_trees::domain::ProofFact::Expression(expression) = fact {
                    statement.push_str("; ensures ");
                    statement.push_str(&typed.expression_table.display_name(*expression));
                }
            }
        }
        return (
            format!("accepted fact: {}", machine.name.as_str()),
            statement,
        );
    }
    (format!("accepted fact: {grant}"), grant.to_owned())
}

pub(super) fn enforce_trust_lockfile(
    options: &CompileOptions,
    typed: &TypedTrees,
    root_grants: &[String],
    provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    selected_provider_plans: &[String],
) -> Result<(), Vec<Diagnostic>> {
    if root_grants.is_empty() {
        return Ok(());
    }
    let Some(project_dir) = options.root_path.parent() else {
        return Ok(());
    };
    let lock_path = project_dir.join("omega.lock");

    // Current receipts.
    let mut rows: Vec<(String, u64)> = Vec::new();
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
            if !rows.iter().any(|(existing, _)| *existing == commitment) {
                rows.push((commitment, plan.identity_fingerprint()));
            }
            continue;
        }
        // MP5: a generic accepted axiom is granted ONCE at its universal
        // normalized template. Every concrete specialization references this
        // receipt; none creates another grant row. The template identity
        // includes its machine-parameter requirements, so changing a `where
        // machine` contract drifts the existing receipt before any instance
        // can reuse it.
        if let Some(machine) = typed.machines().iter().find(|machine| {
            machine.supply_mode == psi_language_semantics::MachineSupplyMode::Accepted
                && (grant == machine.name.as_str()
                    || Some(grant.as_str()) == machine.name.as_str().rsplit("::").next())
        }) && let Some(identity) =
            psi_typed_trees_to_checked_trees::generic_machine_template_fingerprint(
                typed,
                machine.symbol,
            )
        {
            let commitment = format!("accepted fact: {}", machine.name.as_str());
            if !rows.iter().any(|(existing, _)| *existing == commitment) {
                rows.push((commitment, identity));
            }
            continue;
        }
        let (commitment, statement) = commitment_statement(typed, grant);
        if !rows.iter().any(|(existing, _)| *existing == commitment) {
            rows.push((commitment, fnv1a(&statement)));
        }
    }

    // Drift check against the existing lock.
    if let Ok(existing) = std::fs::read_to_string(&lock_path) {
        for line in existing.lines() {
            let Some((hash_text, commitment)) = line.split_once("  ") else {
                continue;
            };
            let Ok(pinned) = u64::from_str_radix(hash_text, 16) else {
                continue;
            };
            if let Some((_, current)) = rows.iter().find(|(name, _)| name == commitment)
                && *current != pinned
            {
                return Err(vec![Diagnostic::error(format!(
                    "granted statement drifted: `{commitment}` no longer matches the \
                     receipt in {} -- a statement that changes under a grant fails \
                     the build until re-approved (delete the stale row to re-approve)",
                    lock_path.display()
                ))]);
            }
        }
    }

    // Write the machine-written lock (stable order).
    let mut output = String::new();
    for (commitment, hash) in &rows {
        output.push_str(&format!("{hash:016x}  {commitment}\n"));
    }
    std::fs::write(&lock_path, output).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to write {}: {error}",
            lock_path.display()
        ))]
    })
}
