//! Fallback toolchain-settled `TimeHost` provider plan.
//!
//! `source/library/std/time_host.omg` declares the canonical raw clock
//! boundary. Reviewed packages may supply ordinary checked implementations
//! with explicit conformance and selection; the macOS provider does so in
//! `targets/macos_arm64/time_host_impl.omg`. Those plans retain normal
//! package, schema, target and body provenance. This fallback mints a plan
//! only when no ordinary provider was selected for the accepted boundary
//! (`AcceptedSemanticBindingRole::TimeHostService`).
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
//! no kernel mechanism and stay uncovered by this table. They belong in
//! ordinary checked provider bodies, not a new constant binding kind; see
//! `wiki/spec/build/provider_selection.md`. Linux still needs those bodies
//! and marshalled syscall adapters. Demand-completeness never fabricates
//! missing implementations.
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
use effects::provider_plan::ProviderPlan;
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
type ToolchainTimeHostLeaf = super::ToolchainSettledLeaf;

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

/// The toolchain-settled plan and the exact symbols receiver admission
/// needs to join it: the boundary trait (fused erasure + provenance schema
/// declaration) and the row-aligned requirement machine symbols.
pub type MintedTimeHostPlan = super::ToolchainSettledMint;

/// Mint the canonical `TimeHost` provider plan for this build, or `Ok(None)`
/// when the build accepts no clock binding, selects no target,
/// selects a target with no reviewed realization table, or an
/// already-selected plan covers the slot. The returned plan is
/// candidate-shaped but toolchain-settled: the caller joins it into the
/// selected facts, the fused-service erasure binding, and the
/// post-derivation review provenance, and deliberately excludes it from
/// candidate plans and pre-selection provenance replay.
pub fn mint_canonical_time_host_plan(
    typed: &TypedTrees,
    accepted_binding: Option<&AcceptedSemanticBinding>,
    target_name: Option<&'static str>,
    already_selected: &[ProviderPlan],
) -> Result<Option<MintedTimeHostPlan>, Vec<Diagnostic>> {
    let leaves: &[super::ToolchainSettledLeaf] =
        target_name.map_or(&[], toolchain_time_host_leaves);
    super::mint_toolchain_settled_plan(
        typed,
        accepted_binding,
        target_name,
        already_selected,
        leaves,
        TOOLCHAIN_TIME_HOST_PROVIDER,
        "TimeHost",
        super::CanonicalHostCoverage::Exact,
    )
}
