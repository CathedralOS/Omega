//! Trusted-surface inventory for Terminal Psi verification.
//!
//! This ledger is the structured enumeration the
//! [verification contract](../../../../../wiki/spec/terminal-psi/verification.md)
//! requires of the trusted implementation surface: primitive judgments,
//! certificate checker rules, reconstructed fact kinds, normalization and
//! conversion, premise scope, write invalidation, and call/cycle composition,
//! plus the shared formation rules those rules stand on.
//!
//! Each [`TrustedSurfaceEntry`] binds:
//!
//! - `premises` / `conclusion`: the exact rule shape the implementation is
//!   trusted to check, stated at the level the code enforces.
//! - `dependencies`: other ledger entries whose own status this rule needs;
//!   a proved row may not hide an unproved composition theorem.
//! - `implementation`: registered [`ImplementationSite`] paths whose recorded
//!   content digest pins the implementing code. Changing an implementation
//!   changes its digest and fails coverage until the entry's justification is
//!   revalidated; a stable enum tag is not enough.
//! - `soundness`: `Proved`, `ExplicitlyTrusted`, or `Unfinished`. A `Proved`
//!   entry names checked evidence and its assumptions — currently a total
//!   certifying procedure whose every emitted fact the certificate checker
//!   re-decides before it may join a roster — while its dependencies keep the
//!   trusted checker rules it stands on explicit. `Unfinished` entries cannot
//!   establish an independent claim merely because the implementation returns
//!   success.
//!
//! ## Mechanical coverage
//!
//! Coverage is a ratchet, not a hand-maintained manifest:
//!
//! - Each dispatchable enum (`PrimitiveJudgment`, `ProofRule`, `EvidenceRoute`,
//!   `ReconstructedTerminalObligationOwner`, `OperationSemanticTag`,
//!   `Terminator`) has an exhaustive map in this module with no wildcard arm.
//!   A new accepted dispatch or fact kind fails compilation before
//!   publication; the coverage test additionally requires a bijection between
//!   variants and dispatch-bound entries.
//! - Every non-test Rust source under the three trusted crates (`proof-
//!   admission`, `terminal-semantics`, `terminal-verifier`) must be a
//!   registered site or a declared test-only module, so a new file cannot
//!   join the accepted surface unobserved.
//! - Site digests are recomputed from the working tree; any implementation
//!   edit fails the witnessed test and names the entries to revalidate.
//!   Inventory-machinery files cannot digest themselves and bind by path only.
//!
//! Coverage establishes inventory completeness, not rule soundness.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

mod checker;
mod operations;
mod reconstruction;
mod shared;
mod sites;

pub use checker::{
    ENTRIES as CHECKER_ENTRIES, checker_rule_entry, evidence_route_entry, primitive_judgment_entry,
};
pub use operations::{ENTRIES as OPERATION_ENTRIES, operation_schema_entry};
pub use reconstruction::{
    ENTRIES as RECONSTRUCTION_ENTRIES, obligation_owner_entry, terminator_fact_entry,
};
pub use shared::ENTRIES as SHARED_ENTRIES;
pub use sites::{IMPLEMENTATION_SITES, TEST_ONLY_SOURCES, TRUSTED_SOURCE_ROOTS, TestOnlySource};

/// The kind of trusted responsibility one ledger entry inventories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LedgerFamily {
    /// Total closed judgments re-decided by the kernel.
    PrimitiveJudgment,
    /// `ProofRule` dispatch arms of the certificate checker.
    CheckerRule,
    /// `EvidenceRoute` arms accepting a fact for an obligation.
    EvidenceRoute,
    /// `ReconstructedTerminalObligationOwner` kinds and their producers.
    ObligationOwner,
    /// `OperationSemanticTag` rows: leaf denotations and call compositions.
    OperationSchema,
    /// Reconstructed axiom/fact introduction categories and terminator facts.
    ReconstructedFactKind,
    /// Normalization, conversion, and bounded witness-form machinery.
    NormalizationConversion,
    /// Premise availability: dominance, merges, entry scope, substitution.
    PremiseScope,
    /// Write and invalidating-event rules ending a fact's validity.
    WriteInvalidation,
    /// Call instantiation and imported-contract composition machinery.
    CallComposition,
    /// Recursive-component and control-cycle question composition.
    CycleComposition,
    /// Shared validation and formation rules the surface stands on.
    SharedFormation,
    /// The inventory mechanism itself.
    Inventory,
}

/// A dispatchable surface whose every variant maps to a ledger entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CoveredSurface {
    PrimitiveJudgments,
    ProofRules,
    EvidenceRoutes,
    ObligationOwners,
    OperationTags,
    Terminators,
}

/// How an entry's coverage is ratcheted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryBinding {
    /// Produced by exactly one arm of that surface's exhaustive map.
    DispatchOn(CoveredSurface),
    /// Bound through implementation sites and file coverage only.
    Procedural,
}

/// One entry's soundness state under the verification contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundnessStatus {
    /// Checked evidence exists and names its assumptions.
    Proved {
        /// The checked derivation or theorem binding this entry.
        evidence: &'static str,
    },
    /// Implemented and accepted under a named trust root with a stated
    /// rationale; not proved.
    ExplicitlyTrusted {
        /// A [`TRUST_ROOTS`] identity accepting this implementation.
        root: &'static str,
        /// Why this implementation is accepted in that role.
        rationale: &'static str,
    },
    /// A known gap: success of the implementation establishes nothing here.
    Unfinished {
        /// What remains unestablished.
        gap: &'static str,
    },
}

/// One registered implementation identity: a source file whose recorded
/// digest binds the code an entry names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImplementationSite {
    /// Repository-relative source path.
    pub path: &'static str,
    /// Lowercase hex SHA-256 of the file contents. `None` only for inventory
    /// machinery, which cannot carry its own digest.
    pub sha256: Option<&'static str>,
    /// Set only on `trusted_surface` module files themselves.
    pub inventory_machinery: bool,
}

/// One row of the trusted-surface ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustedSurfaceEntry {
    /// Stable ledger identity (`family:rule-name` convention).
    pub id: &'static str,
    pub family: LedgerFamily,
    pub binding: EntryBinding,
    /// Exact licensed premises for this rule.
    pub premises: &'static str,
    /// Exact conclusion this rule may establish or accept.
    pub conclusion: &'static str,
    /// Other entry or [`TRUST_ROOTS`] identities this rule depends on.
    pub dependencies: &'static [&'static str],
    /// [`IMPLEMENTATION_SITES`] paths implementing this entry.
    pub implementation: &'static [&'static str],
    pub soundness: SoundnessStatus,
}

/// A registered terminal of the trust-dependency graph. Dependencies must end
/// at an entry or at one of these roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrustRoot {
    pub id: &'static str,
    /// What kind of authority the root carries.
    pub kind: &'static str,
    /// The semantic subject the root accepts.
    pub subject: &'static str,
    /// The owner whose policy accepts it.
    pub owner: &'static str,
    pub rationale: &'static str,
}

/// The only places an `ExplicitlyTrusted` status may point.
pub static TRUST_ROOTS: &[TrustRoot] = &[
    TrustRoot {
        id: "root:rust-reference-verifier",
        kind: "implementation",
        subject: "the trusted Rust implementation of the Terminal Psi verifier and certificate checker",
        owner: "psi",
        rationale: "bounded rules executed as trusted code under the verification contract; no lower-rung derivation currently discharges them",
    },
    TrustRoot {
        id: "root:verifier-regression-corpus",
        kind: "witness",
        subject: "terminal-verifier, proof-admission, and canary suites witnessing each accepted and rejected boundary",
        owner: "tests",
        rationale: "coverage witnesses establish that implemented rules fire and refuse; they are evidence of behavior, not of soundness",
    },
    TrustRoot {
        id: "root:verification-contract",
        kind: "specification",
        subject: "wiki/spec/terminal-psi/verification.md canonical-ledger and trusted-surface obligations",
        owner: "docs",
        rationale: "the contract this inventory enumerates; it assigns responsibilities, not discharged theorems",
    },
];

/// Every ledger entry across all families.
pub fn all_entries() -> impl Iterator<Item = &'static TrustedSurfaceEntry> {
    CHECKER_ENTRIES
        .iter()
        .chain(OPERATION_ENTRIES)
        .chain(RECONSTRUCTION_ENTRIES)
        .chain(SHARED_ENTRIES)
}

/// One coverage or well-formedness failure. `Display` text names the offending
/// entry or site so a failing run is self-describing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LedgerFailure {
    DuplicateEntryId(&'static str),
    DuplicateSitePath(&'static str),
    DuplicateTrustRoot(&'static str),
    SiteWithoutDigest {
        path: &'static str,
    },
    DigestOnMachinerySite {
        path: &'static str,
    },
    MachinerySiteOutsideInventory {
        path: &'static str,
    },
    MissingSiteFile {
        path: &'static str,
    },
    UnknownImplementationSite {
        entry: &'static str,
        path: &'static str,
    },
    UnknownDependency {
        entry: &'static str,
        dependency: &'static str,
    },
    EmptyField {
        entry: &'static str,
        field: &'static str,
    },
    DependencyCycle {
        entry: &'static str,
    },
    TrustedEntryMissingRoot {
        entry: &'static str,
    },
    UnfinishedEntryMissingGap {
        entry: &'static str,
    },
    ProvedEntryMissingEvidence {
        entry: &'static str,
    },
    ProceduralEntryWithoutSites {
        entry: &'static str,
    },
    DispatchEntryNotProduced {
        entry: &'static str,
        surface: CoveredSurface,
    },
    DispatchVariantSharedEntry {
        surface: CoveredSurface,
        entry: &'static str,
    },
    DigestMismatch {
        path: &'static str,
        expected: &'static str,
        actual: String,
        dependents: Vec<&'static str>,
    },
    UnclaimedSourceFile {
        path: String,
    },
    TestOnlySourceNotGated {
        path: &'static str,
    },
}

impl std::fmt::Display for LedgerFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateEntryId(id) => write!(formatter, "duplicate ledger entry id `{id}`"),
            Self::DuplicateSitePath(path) => {
                write!(formatter, "duplicate implementation site `{path}`")
            }
            Self::DuplicateTrustRoot(id) => write!(formatter, "duplicate trust root `{id}`"),
            Self::SiteWithoutDigest { path } => write!(
                formatter,
                "implementation site `{path}` records no digest; only inventory machinery may bind by path alone"
            ),
            Self::DigestOnMachinerySite { path } => write!(
                formatter,
                "inventory machinery site `{path}` must not record a digest of itself"
            ),
            Self::MachinerySiteOutsideInventory { path } => write!(
                formatter,
                "site `{path}` claims inventory-machinery status outside the trusted_surface module"
            ),
            Self::MissingSiteFile { path } => {
                write!(formatter, "implementation site `{path}` does not exist")
            }
            Self::UnknownImplementationSite { entry, path } => write!(
                formatter,
                "ledger entry `{entry}` cites unregistered implementation site `{path}`"
            ),
            Self::UnknownDependency { entry, dependency } => write!(
                formatter,
                "ledger entry `{entry}` depends on unknown id `{dependency}`; dependencies must end at an entry or trust root"
            ),
            Self::EmptyField { entry, field } => {
                write!(formatter, "ledger entry `{entry}` has an empty `{field}`")
            }
            Self::DependencyCycle { entry } => write!(
                formatter,
                "ledger entry `{entry}` participates in a dependency cycle; the trust graph must be acyclic"
            ),
            Self::TrustedEntryMissingRoot { entry } => write!(
                formatter,
                "ledger entry `{entry}` is ExplicitlyTrusted but names no registered trust root"
            ),
            Self::UnfinishedEntryMissingGap { entry } => write!(
                formatter,
                "ledger entry `{entry}` is Unfinished but names no gap"
            ),
            Self::ProvedEntryMissingEvidence { entry } => write!(
                formatter,
                "ledger entry `{entry}` is Proved but binds no checked evidence"
            ),
            Self::ProceduralEntryWithoutSites { entry } => write!(
                formatter,
                "procedural ledger entry `{entry}` binds no implementation site"
            ),
            Self::DispatchEntryNotProduced { entry, surface } => write!(
                formatter,
                "ledger entry `{entry}` is dispatch-bound on {surface:?} but no coverage-map arm produces it"
            ),
            Self::DispatchVariantSharedEntry { surface, entry } => write!(
                formatter,
                "coverage map for {surface:?} produces entry `{entry}` for more than one variant"
            ),
            Self::DigestMismatch {
                path,
                expected,
                actual,
                dependents,
            } => write!(
                formatter,
                "implementation `{path}` changed (expected sha256 {expected}, found {actual}); revalidate the justification of entries {dependents:?} and update the recorded digest"
            ),
            Self::UnclaimedSourceFile { path } => write!(
                formatter,
                "source file `{path}` under a trusted root is not a registered implementation site or a declared test-only module"
            ),
            Self::TestOnlySourceNotGated { path } => write!(
                formatter,
                "declared test-only source `{path}` is not gated by #[cfg(test)] in its parent module"
            ),
        }
    }
}

impl std::error::Error for LedgerFailure {}

fn entry_index() -> BTreeMap<&'static str, &'static TrustedSurfaceEntry> {
    let mut index = BTreeMap::new();
    for entry in all_entries() {
        index.insert(entry.id, entry);
    }
    index
}

/// Pure ledger well-formedness: unique identities, resolved dependencies and
/// sites, shaped statuses, and digest/machinery consistency. Filesystem-free.
pub fn check_ledger_internals() -> Vec<LedgerFailure> {
    let mut failures = Vec::new();
    let mut entry_ids = BTreeSet::new();
    let mut site_paths = BTreeSet::new();
    let mut root_ids = BTreeSet::new();
    for root in TRUST_ROOTS {
        if !root_ids.insert(root.id) {
            failures.push(LedgerFailure::DuplicateTrustRoot(root.id));
        }
    }
    for site in IMPLEMENTATION_SITES {
        if !site_paths.insert(site.path) {
            failures.push(LedgerFailure::DuplicateSitePath(site.path));
        }
        match (site.sha256, site.inventory_machinery) {
            (None, false) => failures.push(LedgerFailure::SiteWithoutDigest { path: site.path }),
            (Some(_), true) => {
                failures.push(LedgerFailure::DigestOnMachinerySite { path: site.path })
            }
            _ => {}
        }
        if site.inventory_machinery
            && !site
                .path
                .starts_with("omega-rust/psi/semantics/terminal-verifier/src/trusted_surface")
        {
            failures.push(LedgerFailure::MachinerySiteOutsideInventory { path: site.path });
        }
    }
    for entry in all_entries() {
        if !entry_ids.insert(entry.id) {
            failures.push(LedgerFailure::DuplicateEntryId(entry.id));
        }
        if entry.premises.trim().is_empty() {
            failures.push(LedgerFailure::EmptyField {
                entry: entry.id,
                field: "premises",
            });
        }
        if entry.conclusion.trim().is_empty() {
            failures.push(LedgerFailure::EmptyField {
                entry: entry.id,
                field: "conclusion",
            });
        }
        match entry.soundness {
            SoundnessStatus::Proved { evidence } => {
                if evidence.trim().is_empty() {
                    failures.push(LedgerFailure::ProvedEntryMissingEvidence { entry: entry.id });
                }
            }
            SoundnessStatus::ExplicitlyTrusted { root, rationale } => {
                if !root_ids.contains(root) || rationale.trim().is_empty() {
                    failures.push(LedgerFailure::TrustedEntryMissingRoot { entry: entry.id });
                }
            }
            SoundnessStatus::Unfinished { gap } => {
                if gap.trim().is_empty() {
                    failures.push(LedgerFailure::UnfinishedEntryMissingGap { entry: entry.id });
                }
            }
        }
        for &site in entry.implementation {
            if !site_paths.contains(site) {
                failures.push(LedgerFailure::UnknownImplementationSite {
                    entry: entry.id,
                    path: site,
                });
            }
        }
        if entry.implementation.is_empty() && entry.binding == EntryBinding::Procedural {
            failures.push(LedgerFailure::ProceduralEntryWithoutSites { entry: entry.id });
        }
    }
    let index = entry_index();
    for entry in all_entries() {
        for &dependency in entry.dependencies {
            if !index.contains_key(dependency) && !root_ids.contains(dependency) {
                failures.push(LedgerFailure::UnknownDependency {
                    entry: entry.id,
                    dependency,
                });
            }
        }
    }
    // The trust graph is closed: dependencies resolve to entries or roots and
    // entry-to-entry edges must be acyclic. A cycle exists exactly when an
    // entry can reach itself through entry dependencies.
    for entry in all_entries() {
        let mut visited = BTreeSet::new();
        let mut stack: Vec<&TrustedSurfaceEntry> = vec![entry];
        while let Some(current) = stack.pop() {
            for &dependency in current.dependencies {
                if dependency == entry.id {
                    failures.push(LedgerFailure::DependencyCycle { entry: entry.id });
                    stack.clear();
                    break;
                }
                if let Some(next) = index.get(dependency)
                    && visited.insert(next.id)
                {
                    stack.push(next);
                }
            }
        }
    }
    failures
}

/// The dispatch-bound entries one coverage map must produce, one per variant.
pub fn dispatch_bound_entries(surface: CoveredSurface) -> BTreeSet<&'static str> {
    all_entries()
        .filter(|entry| entry.binding == EntryBinding::DispatchOn(surface))
        .map(|entry| entry.id)
        .collect()
}

/// Check the dispatch-bound half of coverage: every produced entry is
/// dispatch-bound on that surface, no two variants share an entry, and every
/// dispatch-bound entry is produced. `produced` is one map call per variant.
pub fn check_dispatch_bijection(
    surface: CoveredSurface,
    produced: &[&'static TrustedSurfaceEntry],
) -> Vec<LedgerFailure> {
    let mut failures = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in produced {
        if entry.binding != EntryBinding::DispatchOn(surface) {
            failures.push(LedgerFailure::DispatchEntryNotProduced {
                entry: entry.id,
                surface,
            });
        }
        if !seen.insert(entry.id) {
            failures.push(LedgerFailure::DispatchVariantSharedEntry {
                surface,
                entry: entry.id,
            });
        }
    }
    for id in dispatch_bound_entries(surface) {
        if !seen.contains(id) {
            failures.push(LedgerFailure::DispatchEntryNotProduced { entry: id, surface });
        }
    }
    failures
}

/// Filesystem-backed coverage: every site's digest matches its working-tree
/// content, and every non-test source under the trusted roots is claimed.
pub fn check_source_coverage(root: &Path) -> Vec<LedgerFailure> {
    let mut failures = Vec::new();
    let index = entry_index();
    for site in IMPLEMENTATION_SITES {
        let full = root.join(site.path);
        let contents = match std::fs::read(&full) {
            Ok(contents) => contents,
            Err(_) => {
                failures.push(LedgerFailure::MissingSiteFile { path: site.path });
                continue;
            }
        };
        if let Some(expected) = site.sha256 {
            use sha2::Digest;
            let actual = format!("{:x}", sha2::Sha256::digest(&contents));
            if actual != expected {
                let dependents = index
                    .values()
                    .filter(|entry| entry.implementation.contains(&site.path))
                    .map(|entry| entry.id)
                    .collect();
                failures.push(LedgerFailure::DigestMismatch {
                    path: site.path,
                    expected,
                    actual,
                    dependents,
                });
            }
        }
    }
    let mut claimed: BTreeSet<&str> = IMPLEMENTATION_SITES
        .iter()
        .map(|site| site.path)
        .chain(TEST_ONLY_SOURCES.iter().map(|source| source.path))
        .collect();
    for trusted_root in TRUSTED_SOURCE_ROOTS {
        collect_unclaimed_sources(&root.join(trusted_root), &mut claimed, &mut failures);
    }
    let test_only: BTreeMap<&str, &TestOnlySource> = TEST_ONLY_SOURCES
        .iter()
        .map(|source| (source.path, source))
        .collect();
    for source in TEST_ONLY_SOURCES {
        if !test_source_gated(root, source, &test_only, &mut BTreeSet::new()) {
            failures.push(LedgerFailure::TestOnlySourceNotGated { path: source.path });
        }
    }
    failures
}

/// A declared test-only source is gated when its parent declares
/// `#[cfg(test)] mod <module>;`, or when that declaring parent file is itself
/// a gated test-only source: a `mod` inside a test-only subtree compiles only
/// under `cfg(test)`, so gating is transitive down the module tree.
fn test_source_gated<'a>(
    root: &Path,
    source: &'a TestOnlySource,
    sources: &BTreeMap<&'static str, &'a TestOnlySource>,
    visiting: &mut BTreeSet<&'static str>,
) -> bool {
    if !visiting.insert(source.path) {
        return false;
    }
    let gated = (|| {
        let Ok(contents) = std::fs::read_to_string(root.join(source.parent)) else {
            return false;
        };
        let lines: Vec<&str> = contents.lines().collect();
        let needle = format!("mod {};", source.module);
        let mut declared = false;
        for (index, line) in lines.iter().enumerate() {
            if line.trim() != needle {
                continue;
            }
            declared = true;
            if lines[index.saturating_sub(4)..index]
                .iter()
                .any(|prior| prior.trim() == "#[cfg(test)]")
            {
                return true;
            }
        }
        declared
            && sources
                .get(source.parent)
                .is_some_and(|parent| test_source_gated(root, parent, sources, visiting))
    })();
    visiting.remove(source.path);
    gated
}

fn collect_unclaimed_sources(
    directory: &Path,
    claimed: &mut BTreeSet<&str>,
    failures: &mut Vec<LedgerFailure>,
) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut paths: Vec<_> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            collect_unclaimed_sources(&path, claimed, failures);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            let normalized = path.to_string_lossy().replace('\\', "/");
            if let Some(position) = normalized.find("omega-rust/") {
                let repo_relative = &normalized[position..];
                if !claimed.contains(repo_relative) {
                    failures.push(LedgerFailure::UnclaimedSourceFile {
                        path: repo_relative.to_string(),
                    });
                }
            }
        }
    }
}
