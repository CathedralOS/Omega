//! Canonical toolchain-settled `FilesystemHost` provider plan.
//!
//! `source/library/std/filesystem_host.omg` declares the canonical raw
//! filesystem boundary as a requirement-only surface: packages accept the
//! exact binding (`AcceptedSemanticBindingRole::FilesystemHostService`), but
//! no package may author `satisfies` conformances or `select_provider` rows
//! for the slot — slot resolution derives only from conformance provenance,
//! so an authored conformance would never even resolve the boundary slot.
//! Provider settlement therefore mints the canonical realization plan itself,
//! per selected target, from the toolchain's own reviewed realization table.
//!
//! The minted plan deliberately covers only the leaves the toolchain can bind
//! honestly on that target: scalar-only requirements whose realization is one
//! positional kernel syscall, further narrowed to the methods the accepted
//! service schema actually declares — a consumer-declared schema names its
//! own demanded subset, and a leaf outside that subset has no requirement to
//! realize. A method whose signature carries `in Path`,
//! slice carriers, or host-dependent argument choreography is not guessable
//! and earns no row. Demand-completeness never fabricates a broad union: a
//! demanded leaf with no minted row still resolves to zero selected rows at
//! closure review, and ordinary-release cohort leaves (`close`,
//! `find_close`, `close_handle`) bind their syscall but mint no
//! classification row — they still require the retained occurrence-specific
//! release contract owned by FILESYSTEM-RELEASE-CONTRACT.
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

use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use package_compilation::AcceptedSemanticBinding;

/// Nominal provider identity for toolchain-settled canonical-host plans.
///
/// Readable slot identity only: the plan is produced by the toolchain's own
/// settlement table, not a package conformance closure, so provider-type and
/// origin package identities stay `None` and consumers must not infer
/// ownership from this name.
const TOOLCHAIN_FILESYSTEM_HOST_PROVIDER: &str = "omega::toolchain::filesystem_host";

/// One canonical `FilesystemHost` requirement realized by one positional
/// kernel syscall on the selected target.
type ToolchainFilesystemLeaf = super::ToolchainSettledLeaf;

/// The reviewed linux_x86_64 realization table: only requirements whose
/// checked signature is all scalar and whose honest realization is one
/// positional syscall. Ordinary-release cohort members (`close`,
/// `find_close`, `close_handle`) are bound here — the binding is honest —
/// while their classification row still waits on the occurrence-bound release
/// contract. Leaves needing path/spelling adaptation, slice carriers, error
/// reading (`errno`, `get_last_error`), or the `get_osfhandle` host seam have
/// no honest positional syscall and stay uncovered on purpose.
const fn filesystem_leaf(method: &'static str, number: i64) -> ToolchainFilesystemLeaf {
    ToolchainFilesystemLeaf { method, number }
}

const LINUX_X86_64_FILESYSTEM_LEAVES: &[ToolchainFilesystemLeaf] = &[
    filesystem_leaf("close", 3),
    filesystem_leaf("find_close", 3),
    filesystem_leaf("close_handle", 3),
    filesystem_leaf("seek", 8),
    filesystem_leaf("duplicate", 32),
    filesystem_leaf("lock_file", 73),
    filesystem_leaf("sync", 74),
    filesystem_leaf("sync_data", 75),
    filesystem_leaf("set_len", 77),
    filesystem_leaf("set_file_permissions", 91),
    filesystem_leaf("change_file_owner", 93),
];

/// Per-target canonical realization tables. A target with no reviewed table
/// earns no plan, leaving the closure-review stop honest rather than
/// fabricating guessed realization.
fn toolchain_filesystem_leaves(target_name: &str) -> &'static [ToolchainFilesystemLeaf] {
    match target_name {
        "linux_x86_64" => LINUX_X86_64_FILESYSTEM_LEAVES,
        _ => &[],
    }
}

/// The toolchain-settled plan and the exact symbols receiver admission
/// needs to join it: the boundary trait (fused erasure + provenance schema
/// declaration) and the row-aligned requirement machine symbols.

/// Mint the canonical `FilesystemHost` provider plan for this build, or `Ok(None)`
/// when the build accepts no filesystem binding, selects no target,
/// selects a target with no reviewed realization table, or an
/// already-selected plan covers the slot. The returned plan is
/// candidate-shaped but toolchain-settled: the caller joins it into the
/// selected facts, the fused-service erasure binding, and the
/// post-derivation review provenance, and deliberately excludes it from
/// candidate plans and pre-selection provenance replay.
pub(crate) fn mint_canonical_filesystem_host_plan(
    services: ServiceCatalog<'_>,
    calling_plans: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    accepted_binding: Option<&AcceptedSemanticBinding>,
    target_name: Option<&'static str>,
    already_selected: &[ProviderPlan],
) -> Result<Option<crate::DerivedProviderPlan>, Vec<Diagnostic>> {
    let leaves: &[super::ToolchainSettledLeaf] =
        target_name.map_or(&[], toolchain_filesystem_leaves);
    super::mint_toolchain_settled_plan(
        services,
        calling_plans,
        accepted_binding,
        target_name,
        already_selected,
        leaves,
        TOOLCHAIN_FILESYSTEM_HOST_PROVIDER,
        "FilesystemHost",
    )
}
