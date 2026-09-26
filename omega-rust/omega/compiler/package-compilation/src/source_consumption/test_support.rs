//! Custody-substitution machinery for the source-consumption commitment
//! matrix.
//!
//! `derive_source_consumption_commitment` is the independent checker: it
//! replays the retained unit roster and reconciled package graph into the
//! domain-separated `PackageSourceConsumptionCommitment`. A legitimate
//! substitution either keeps the roster's strict canonical order and moves
//! the derived commitment, or cannot keep the order and rejects at
//! derivation. Build-purpose edges are compilation-local nameability and are
//! deliberately absent from the durable product closure — the slack leg that
//! proves they cannot diverge the commitment stays in the test itself, since
//! it is a non-substitution invariant rather than a custody axis.
//!
//! Three inventories cover the family's substitution lanes:
//! `ConsumedSourceUnitFieldForTest` for the representable fields of one
//! `ConsumedSourceUnit` row, `ConsumedSourceRosterAxisForTest` for the
//! roster-shape axes (omission, duplication, reordering, forged insert), and
//! `PackageGraphConsumptionFieldForTest` for the reconciled package-graph
//! axes the commitment hashes (root, role, package roster, and product-edge
//! coordinates). Every leg is consumed by the shared
//! `run_one_field_substitution_matrix` driver.

use super::super::{
    BuildDeclarationKind, PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use super::tests::canonical_row_fixture;
use super::{
    ConsumedSourceUnit, ConsumedSourceUnitKind, PackageSourceConsumptionCommitment,
    canonical_consumed_unit_bytes, derive_source_consumption_commitment,
};
use optimization_core::MutationOutcome;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::PathBuf;

optimization_core::custody_field_inventory! {
    /// One substitutable field of a `ConsumedSourceUnit` row in the retained
    /// canonical roster. The matrix substitutes exactly one field per leg so
    /// a rejection attributes to that claim alone; `byte_count` and
    /// `content_digest` take fixed non-identity alternates and the
    /// handle-valued fields take foreign package/namespace values.
    /// `source_id` and the physical cache root are compilation-local and are
    /// deliberately not retained fields. The independent checker is
    /// `derive_source_consumption_commitment`.
    pub enum ConsumedSourceUnitFieldForTest {
        Kind,
        PackageForeign,
        PackageCleared,
        ToolchainNamespaceForeign,
        ToolchainNamespaceCleared,
        RelativePathComponent,
        RelativePathExtended,
        RelativePathDropped,
        ByteCount,
        ContentDigest,
    }
}

optimization_core::custody_field_inventory! {
    /// One roster-shape axis of the canonical `ConsumedSourceUnit` roster.
    /// Omission and forged insertion keep a well-formed, still-ordered roster
    /// whose derived commitment diverges; emptiness, duplication, and
    /// reordering break the canonical joins and reject at derivation.
    pub enum ConsumedSourceRosterAxisForTest {
        RowDropped,
        RosterEmptied,
        RowDuplicated,
        RowsSwapped,
        RowsReversed,
        ForgedRowInserted,
    }
}

optimization_core::custody_field_inventory! {
    /// One reconciled package-graph axis the source-consumption commitment
    /// hashes: the root package identity, the root's declared role, the
    /// package roster, and each product-edge coordinate (alias, requester,
    /// roster extent). Substitutions keep the substituted graph well-formed,
    /// so the independent checker replays to a divergent commitment.
    pub enum PackageGraphConsumptionFieldForTest {
        RootIdentity,
        RootRole,
        PackageRosterDropped,
        PackageIdentity,
        EdgeAlias,
        EdgeRequester,
        EdgeRosterExtended,
    }
}

/// The checker result classification for a mutated source-consumption case.
///
/// `derive_source_consumption_commitment` reports `Vec<Diagnostic>`; the
/// matrix needs an exact per-leg expectation, so the check classifies the
/// first diagnostic:
///
/// - `MissingUnits` -- the roster is empty; derivation requires retained
///   frontend source metadata.
/// - `NonCanonicalRoster` -- the mutated roster is not strictly ordered and
///   unique; derivation refuses it before hashing.
/// - `Unclassified` -- a rejection the matrix does not name; declaring it in
///   `outcome` is never correct, so producing it always fails the leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceConsumptionRejection {
    MissingUnits,
    NonCanonicalRoster,
    Unclassified,
}

/// The retained custody view a substitution must move: the canonical row
/// bytes, the reconciled package-graph inputs, and the commitment the record
/// claims. The first two change with the substitution itself; the reference
/// commitment is the claim the checker's honest replay must diverge from.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceConsumptionCustody {
    pub rows: Vec<Vec<u8>>,
    pub inputs: PackageCompilationInputs,
    pub reference: PackageSourceConsumptionCommitment,
}

/// The record one substitution leg mutates: the retained unit roster, the
/// reconciled package-graph inputs, and the baseline commitment derived over
/// the honest pair at case construction.
pub struct SourceConsumptionCustodyCase {
    pub units: Vec<ConsumedSourceUnit>,
    pub inputs: PackageCompilationInputs,
    pub reference: PackageSourceConsumptionCommitment,
}

/// The canonical graph of three package source roots (`root`, `middle`,
/// `leaf`) bound under one temporary directory, with the honest product
/// dependency edges `root -> middle` and `middle -> leaf`.
pub(super) fn canonical_source_consumption_inputs()
-> (PackageKeyIdentity, PackageCompilationInputs, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "omega-source-consumption-substitution-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    for name in ["root", "middle", "leaf"] {
        std::fs::create_dir_all(root.join(name)).expect("create package source root");
    }
    let binding = |identity: PackageKeyIdentity, name: &'static str| {
        PackageSourceBinding::new(identity, name, root.join(name))
    };
    let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
    let middle = PackageKeyIdentity::from_digest([8; 32]).expect("middle identity");
    let leaf = PackageKeyIdentity::from_digest([9; 32]).expect("leaf identity");
    let inputs = PackageCompilationInputs::new(
        package,
        BuildDeclarationKind::Package,
        vec![
            binding(package, "root"),
            binding(middle, "middle"),
            binding(leaf, "leaf"),
        ],
        vec![
            PackageDependencyBinding::new(package, "middle", middle),
            PackageDependencyBinding::new(middle, "leaf", leaf),
        ],
    )
    .expect("three-package chain graph");
    (package, inputs, root)
}

/// Honestly derive the claim a case retains.
fn derive_reference(
    units: &[ConsumedSourceUnit],
    inputs: &PackageCompilationInputs,
) -> PackageSourceConsumptionCommitment {
    derive_source_consumption_commitment(units, inputs)
        .expect("honest custody must derive a commitment")
}

/// The honestly produced case: the canonical four-row roster over the
/// canonical three-package graph.
pub fn honest_source_consumption_custody_case() -> SourceConsumptionCustodyCase {
    let units = canonical_row_fixture();
    let (_package, inputs, _root) = canonical_source_consumption_inputs();
    let reference = derive_reference(&units, &inputs);
    SourceConsumptionCustodyCase {
        units,
        inputs,
        reference,
    }
}

/// An authentic foreign case of the same family: a well-formed roster over
/// the same graph whose last row carries foreign bytes, so the honestly
/// derived claim diverges from the honest case's.
pub fn foreign_source_consumption_donor() -> SourceConsumptionCustodyCase {
    let mut case = honest_source_consumption_custody_case();
    case.units[3].content_digest = [0xa6; 32];
    case.reference = derive_reference(&case.units, &case.inputs);
    case
}

/// The retained custody view of a case: canonical row bytes plus the
/// reconciled inputs plus the claimed commitment.
pub fn source_consumption_custody(case: &SourceConsumptionCustodyCase) -> SourceConsumptionCustody {
    SourceConsumptionCustody {
        rows: case
            .units
            .iter()
            .map(canonical_consumed_unit_bytes)
            .collect(),
        inputs: case.inputs.clone(),
        reference: case.reference,
    }
}

/// The canonical package graph after substituting one axis. `package` is the
/// honest root identity; `middle` and `leaf` are the other canonical members;
/// `other` is a foreign package identity bound into the `leaf` slot for the
/// package-identity leg.
fn substituted_graph(
    field: PackageGraphConsumptionFieldForTest,
    root: &std::path::Path,
) -> PackageCompilationInputs {
    use PackageGraphConsumptionFieldForTest as Field;
    let binding = |identity: PackageKeyIdentity, name: &'static str| {
        PackageSourceBinding::new(identity, name, root.join(name))
    };
    let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
    let middle = PackageKeyIdentity::from_digest([8; 32]).expect("middle identity");
    let leaf = PackageKeyIdentity::from_digest([9; 32]).expect("leaf identity");
    let other = PackageKeyIdentity::from_digest([77; 32]).expect("other package");
    let packages = || {
        vec![
            binding(package, "root"),
            binding(middle, "middle"),
            binding(leaf, "leaf"),
        ]
    };
    match field {
        Field::RootIdentity => PackageCompilationInputs::new_package(
            middle,
            vec![binding(middle, "root"), binding(leaf, "leaf")],
            vec![PackageDependencyBinding::new(middle, "leaf", leaf)],
        )
        .expect("substituted root graph"),
        Field::RootRole => PackageCompilationInputs::new(
            package,
            BuildDeclarationKind::Application,
            packages(),
            vec![
                PackageDependencyBinding::new(package, "middle", middle),
                PackageDependencyBinding::new(middle, "leaf", leaf),
            ],
        )
        .expect("application-role graph"),
        Field::PackageRosterDropped => PackageCompilationInputs::new_package(
            package,
            vec![binding(package, "root"), binding(middle, "middle")],
            vec![PackageDependencyBinding::new(package, "middle", middle)],
        )
        .expect("leaf-dropped graph"),
        Field::PackageIdentity => PackageCompilationInputs::new_package(
            package,
            vec![
                binding(package, "root"),
                binding(middle, "middle"),
                binding(other, "leaf"),
            ],
            vec![
                PackageDependencyBinding::new(package, "middle", middle),
                PackageDependencyBinding::new(middle, "leaf", other),
            ],
        )
        .expect("substituted package graph"),
        Field::EdgeAlias => PackageCompilationInputs::new_package(
            package,
            packages(),
            vec![
                PackageDependencyBinding::new(package, "middle", middle),
                PackageDependencyBinding::new(middle, "other", leaf),
            ],
        )
        .expect("renamed-alias graph"),
        Field::EdgeRequester => PackageCompilationInputs::new_package(
            package,
            packages(),
            vec![
                PackageDependencyBinding::new(package, "middle", middle),
                PackageDependencyBinding::new(package, "leaf", leaf),
            ],
        )
        .expect("root-requested leaf graph"),
        Field::EdgeRosterExtended => PackageCompilationInputs::new_package(
            package,
            packages(),
            vec![
                PackageDependencyBinding::new(package, "middle", middle),
                PackageDependencyBinding::new(package, "leaf", leaf),
                PackageDependencyBinding::new(middle, "leaf", leaf),
            ],
        )
        .expect("extra-edge graph"),
    }
}

/// Mutate exactly one declared `ConsumedSourceUnit` row field. Foreign and
/// fixed alternates cannot equal any honest value in the canonical fixture;
/// the `Cleared` legs withdraw an optional field entirely.
pub fn corrupt_source_consumption_unit_for_test(
    case: &mut SourceConsumptionCustodyCase,
    field: ConsumedSourceUnitFieldForTest,
    donor: &SourceConsumptionCustodyCase,
) {
    use ConsumedSourceUnitFieldForTest as Field;
    let other = PackageKeyIdentity::from_digest([77; 32]).expect("other package");
    match field {
        Field::Kind => case.units[0].kind = ConsumedSourceUnitKind::ToolchainOwned,
        Field::PackageForeign => case.units[0].package = Some(other),
        Field::PackageCleared => case.units[1].package = None,
        Field::ToolchainNamespaceForeign => {
            case.units[3].toolchain_namespace = Some("std".to_owned());
        }
        Field::ToolchainNamespaceCleared => case.units[3].toolchain_namespace = None,
        Field::RelativePathComponent => {
            case.units[0].relative_path[0] = "other.omg".to_owned();
        }
        Field::RelativePathExtended => case.units[0].relative_path.push("extra.omg".to_owned()),
        Field::RelativePathDropped => {
            case.units[1].relative_path.pop();
        }
        Field::ByteCount => case.units[2].byte_count += 1,
        Field::ContentDigest => {
            // The donor's foreign content bytes are authentic evidence of the
            // same family.
            case.units[0].content_digest = donor.units[3].content_digest;
        }
    }
}

/// Mutate exactly one roster-shape axis of the retained canonical roster.
/// Drops and forged inserts keep the roster strictly ordered; emptiness,
/// duplication, and reordering break the canonical joins the checker binds.
pub fn corrupt_source_consumption_roster_for_test(
    case: &mut SourceConsumptionCustodyCase,
    axis: ConsumedSourceRosterAxisForTest,
    _donor: &SourceConsumptionCustodyCase,
) {
    use ConsumedSourceRosterAxisForTest as Axis;
    match axis {
        Axis::RowDropped => {
            case.units.remove(0);
        }
        Axis::RosterEmptied => case.units.clear(),
        Axis::RowDuplicated => case.units.insert(1, case.units[0].clone()),
        Axis::RowsSwapped => case.units.swap(0, 1),
        Axis::RowsReversed => case.units.reverse(),
        Axis::ForgedRowInserted => {
            let mut forged = case.units[0].clone();
            forged.relative_path = vec!["zzz.omg".to_owned()];
            case.units.insert(1, forged);
        }
    }
}

/// Mutate exactly one reconciled package-graph axis the commitment hashes.
/// Each arm rebuilds the inputs through the validating constructor so the
/// substituted graph stays well-formed; the leg's divergence comes from the
/// axis itself, never from malformed custody.
pub fn corrupt_source_consumption_graph_for_test(
    case: &mut SourceConsumptionCustodyCase,
    field: PackageGraphConsumptionFieldForTest,
    _donor: &SourceConsumptionCustodyCase,
) {
    let (_package, _inputs, root) = canonical_source_consumption_inputs();
    case.inputs = substituted_graph(field, &root);
}

/// Classify the checker's rejection: only the two named canonical-join
/// refusals are legitimate outcomes. A first diagnostic that names neither is
/// `Unclassified`, which no leg may declare.
fn classify_rejection(
    units: &[ConsumedSourceUnit],
    diagnostics: &[diagnostics::Diagnostic],
) -> SourceConsumptionRejection {
    let first = diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.as_str())
        .unwrap_or_default();
    if first.contains("strictly ordered unique consumed units") {
        // The checker only reaches the canonical-order arm when the mutated
        // roster genuinely fails the join; both must be true together.
        if units.windows(2).any(|pair| pair[0] >= pair[1]) {
            return SourceConsumptionRejection::NonCanonicalRoster;
        }
        return SourceConsumptionRejection::Unclassified;
    }
    if first.contains("requires retained frontend source metadata") {
        return SourceConsumptionRejection::MissingUnits;
    }
    SourceConsumptionRejection::Unclassified
}

/// The family's independent checker: re-derive the source-consumption
/// commitment over the substituted units and inputs.
pub fn check_source_consumption_custody(
    case: &SourceConsumptionCustodyCase,
) -> Result<SourceConsumptionCustody, SourceConsumptionRejection> {
    match derive_source_consumption_commitment(&case.units, &case.inputs) {
        Ok(commitment) => Ok(SourceConsumptionCustody {
            rows: case
                .units
                .iter()
                .map(canonical_consumed_unit_bytes)
                .collect(),
            inputs: case.inputs.clone(),
            reference: commitment,
        }),
        Err(diagnostics) => Err(classify_rejection(&case.units, &diagnostics)),
    }
}

/// Row-field substitutions keep the roster well-formed and strictly ordered
/// (only `Kind` can reorder a row, since kind is the primary sort field), so
/// the checker replays to a divergent commitment — except the `Kind` leg,
/// which collides with the toolchain-owned tail row and rejects as
/// non-canonical.
pub fn source_consumption_unit_outcome(
    field: ConsumedSourceUnitFieldForTest,
) -> MutationOutcome<SourceConsumptionRejection> {
    match field {
        ConsumedSourceUnitFieldForTest::Kind => {
            MutationOutcome::ExactError(SourceConsumptionRejection::NonCanonicalRoster)
        }
        _ => MutationOutcome::RebuiltCustodyDiffers,
    }
}

/// Roster-axis expectations: drops and forged inserts preserve the canonical
/// joins so the derived commitment diverges; emptiness, duplication, and
/// reordering reject at derivation with their named arm.
pub fn source_consumption_roster_outcome(
    axis: ConsumedSourceRosterAxisForTest,
) -> MutationOutcome<SourceConsumptionRejection> {
    use ConsumedSourceRosterAxisForTest as Axis;
    match axis {
        Axis::RowDropped | Axis::ForgedRowInserted => MutationOutcome::RebuiltCustodyDiffers,
        Axis::RosterEmptied => {
            MutationOutcome::ExactError(SourceConsumptionRejection::MissingUnits)
        }
        Axis::RowDuplicated | Axis::RowsSwapped | Axis::RowsReversed => {
            MutationOutcome::ExactError(SourceConsumptionRejection::NonCanonicalRoster)
        }
    }
}

/// Every reconciled package-graph axis binds the product-consumption
/// commitment, so a well-formed substitution always replays to a divergent
/// identity. The deliberately slack build-purpose edge is asserted in the
/// test body, outside the inventory.
pub fn source_consumption_graph_outcome(
    _field: PackageGraphConsumptionFieldForTest,
) -> MutationOutcome<SourceConsumptionRejection> {
    MutationOutcome::RebuiltCustodyDiffers
}
