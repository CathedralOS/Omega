//! Canonical toolchain-settled `TimeHost` provider plan.
//!
//! `source/library/std/time_host.omg` declares the canonical raw clock
//! boundary as a requirement-only surface: packages accept the exact binding
//! (`AcceptedSemanticBindingRole::TimeHostService`), but no package may
//! author `satisfies` conformances or `select_provider` rows for the slot —
//! slot resolution derives only from conformance provenance, so an authored
//! conformance would never even resolve the boundary slot. Provider
//! settlement therefore mints the canonical realization plan itself, per
//! selected target, from the toolchain's own reviewed realization table.
//!
//! The minted plan deliberately covers only the leaves the toolchain can
//! name honestly on that target: requirements whose irreducible kernel
//! realization is one reviewed syscall. `monotonic_ticks` and
//! `wall_clock_raw` bottom out in `clock_gettime`; `sleep` bottoms out in
//! `nanosleep`. Their timespec marshalling and result composition are
//! checked-code adaptation, the same settlement-transport leg the
//! `FilesystemHost` mint pins for its own rows: a demanded leaf with no
//! transport still rejects at lowering rather than manufacturing one. The
//! three per-target constants (`monotonic_ticks_per_second`,
//! `wall_clock_units_per_second`, `wall_clock_epoch_offset_seconds`) carry
//! no kernel mechanism and stay uncovered until a constant binding kind
//! exists. Demand-completeness never fabricates a broad union.
//!
//! The plan joins the selected closure through
//! [`effects::SelectedProviderPlanFacts::with_toolchain_settled_plan`], which
//! skips only the authored-conformance full-coverage check; uniqueness,
//! nonzero-identity, and boundary-slot invariants still apply, and the facts
//! record it as toolchain-settled so the trust report labels its provenance
//! rather than reporting an ungranted dev-active package. It never enters
//! the pre-selection `selected` derivation list (provenance replay requires
//! authored conformance rows it does not have), but it does join the
//! retained `provider_plans` candidate inventory — the trust report replays
//! every selected plan against exactly one candidate, and this plan is a
//! real retained candidate for the slot even though no package authored it.
//! It enters the post-derivation review-provenance list as a
//! `UniqueCoveringCandidate` so receiver admission, fused-service erasure,
//! and downstream settlement readers see the same plan identity the
//! selected facts carry; its per-row realization symbols stay `invalid`
//! because the realization is a toolchain settlement table entry, not an
//! authored machine.

use diagnostics::Diagnostic;
use effects::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};
use package_compilation::AcceptedSemanticBinding;
use typed_trees::TypedTrees;

/// Nominal provider identity for toolchain-settled canonical-host plans.
///
/// Readable slot identity only: the plan is produced by the toolchain's own
/// settlement table, not a package conformance closure, so provider-type and
/// origin package identities stay `None` and consumers must not infer
/// ownership from this name.
const TOOLCHAIN_TIME_HOST_PROVIDER: &str = "omega::toolchain::time_host";

/// One canonical `TimeHost` requirement realized by one reviewed kernel
/// syscall on the selected target.
struct ToolchainTimeHostLeaf {
    /// Canonical requirement name; locates the method inside the accepted
    /// service schema. It is never an identity key in the emitted plan — the
    /// row carries the method's complete normalized requirement identity.
    method: &'static str,
    /// Target syscall number for the selected target profile.
    number: i64,
}

/// The reviewed linux_x86_64 realization table: only requirements whose
/// irreducible kernel realization is one named syscall. `monotonic_ticks`
/// and `wall_clock_raw` bind `clock_gettime` (228); `sleep` binds
/// `nanosleep` (35). The per-target constants carry no kernel mechanism and
/// the timespec marshalling is checked-code adaptation, so neither earns a
/// row from this table.
const LINUX_X86_64_TIME_HOST_LEAVES: &[ToolchainTimeHostLeaf] = &[
    ToolchainTimeHostLeaf {
        method: "monotonic_ticks",
        number: 228,
    },
    ToolchainTimeHostLeaf {
        method: "wall_clock_raw",
        number: 228,
    },
    ToolchainTimeHostLeaf {
        method: "sleep",
        number: 35,
    },
];

/// Per-target canonical realization tables. A target with no reviewed table
/// earns no plan, leaving the closure-review stop honest rather than
/// fabricating guessed realization.
fn toolchain_time_host_leaves(target_name: &str) -> &'static [ToolchainTimeHostLeaf] {
    match target_name {
        "linux_x86_64" => LINUX_X86_64_TIME_HOST_LEAVES,
        _ => &[],
    }
}

/// Resolve the accepted `TimeHostService` binding against the typed program:
/// the exact package-owned boundary trait, its reified service schema, and a
/// digest check identical to the post-checking rejoin in `selected-dispatch`'s
/// `resolve_accepted_service_binding`. Returns the trait definition too so the
/// caller can bind requirement symbols and fused-service erasure against it.
fn resolve_time_host_binding<'trees>(
    typed: &'trees TypedTrees,
    binding: &AcceptedSemanticBinding,
) -> Result<
    (
        &'trees typed_trees::trait_definition::TraitDefinition,
        effects::provider_plan::ServiceSchema,
    ),
    Diagnostic,
> {
    let matches = typed
        .traits()
        .iter()
        .filter_map(|definition| {
            if !definition.is_boundary
                || typed.symbols.symbol_package_identity(definition.symbol)
                    != Some(binding.package())
                || typed.symbols.display_path(definition.symbol, "::") != binding.declaration_path()
            {
                return None;
            }
            let schema = provider_planning::service_schema::from_typed(typed, definition)?;
            (schema.trait_package_identity == Some(binding.package())
                && package_compilation::accepted_service_schema_digest(binding.role(), &schema)
                    == binding.normalized_schema_digest())
            .then_some((definition, schema))
        })
        .collect::<Vec<_>>();
    let [resolved] = matches.as_slice() else {
        return Err(Diagnostic::error(format!(
            "accepted semantic binding {:?} resolved to {} exact package-owned boundary declarations instead of one",
            binding.role(),
            matches.len(),
        )));
    };
    Ok(resolved.clone())
}

/// The toolchain-settled plan and the exact symbols receiver admission
/// needs to join it: the boundary trait (fused erasure + provenance schema
/// declaration) and the row-aligned requirement machine symbols.
pub struct MintedTimeHostPlan {
    pub plan: ProviderPlan,
    pub trait_symbol: symbols::SymbolHandle,
    /// Row-aligned with `plan.rows`: the requirement's own machine symbol.
    /// Realization symbols deliberately stay `invalid` downstream — the
    /// toolchain settlement table, not an authored machine, realizes the row.
    pub requirement_symbols: Vec<symbols::SymbolHandle>,
}

/// Mint the canonical `TimeHost` provider plan for this build, or `Ok(None)`
/// when the build accepts no clock binding, selects no target, selects a
/// target with no reviewed realization table, or an already-selected plan
/// covers the slot (defense in depth: authored conformances for this slot
/// cannot resolve, so coverage implies a foreign source this mint must never
/// override).
///
/// The returned plan is candidate-shaped but toolchain-settled: the caller
/// joins it into the selected facts, the fused-service erasure binding, and
/// the post-derivation review provenance, and deliberately excludes it from
/// candidate plans and pre-selection provenance replay. Package review
/// replays the same mint against the consumed `TimeHostService` binding to
/// validate a retained toolchain-settled plan's exact identity.
pub fn mint_canonical_time_host_plan(
    typed: &TypedTrees,
    accepted_binding: Option<&AcceptedSemanticBinding>,
    target_name: Option<&'static str>,
    already_selected: &[ProviderPlan],
) -> Result<Option<MintedTimeHostPlan>, Vec<Diagnostic>> {
    let Some(binding) = accepted_binding else {
        return Ok(None);
    };
    let Some(target_name) = target_name else {
        return Ok(None);
    };
    let leaves = toolchain_time_host_leaves(target_name);
    if leaves.is_empty() {
        return Ok(None);
    }
    let (definition, schema) =
        resolve_time_host_binding(typed, binding).map_err(|error| vec![error])?;
    let signatures = typed.trait_machine_signatures(definition);
    if already_selected.iter().any(|selected| {
        selected.schema.trait_package_identity == schema.trait_package_identity
            && selected.schema.trait_name == schema.trait_name
    }) {
        return Ok(None);
    }
    let mut rows = Vec::with_capacity(leaves.len());
    let mut requirement_symbols = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        let Some(method) = schema
            .methods
            .iter()
            .find(|method| method.name == leaf.method)
        else {
            return Err(vec![Diagnostic::error(format!(
                "canonical `TimeHost` realization table names `{}`, which the accepted service schema does not declare",
                leaf.method,
            ))]);
        };
        let Some(signature) = signatures
            .iter()
            .find(|signature| signature.name.as_str() == method.name)
        else {
            return Err(vec![Diagnostic::error(format!(
                "canonical `TimeHost` requirement `{}` has no retained typed requirement signature",
                leaf.method,
            ))]);
        };
        requirement_symbols.push(signature.symbol);
        rows.push(ProviderPlanRow {
            method: method.name.clone(),
            requirement_identity: method.requirement_identity.clone(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::Syscall {
                number: leaf.number,
            },
        });
    }
    let plan = ProviderPlan {
        name: provider_planning::satisfies_plan_name(
            target_name,
            &schema.trait_name,
            TOOLCHAIN_TIME_HOST_PROVIDER,
        ),
        provider_type: TOOLCHAIN_TIME_HOST_PROVIDER.to_owned(),
        provider_type_package_identity: None,
        target: target_name.to_owned(),
        schema,
        rows,
        origin_package_identity: None,
        origin_package: "omega toolchain".to_owned(),
    };
    let errors = plan.validate_candidate_against_schema();
    if !errors.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "toolchain-settled `TimeHost` plan is malformed: {}",
            errors.join("; ")
        ))]);
    }
    Ok(Some(MintedTimeHostPlan {
        plan,
        trait_symbol: definition.symbol,
        requirement_symbols,
    }))
}
