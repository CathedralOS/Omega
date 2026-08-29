use super::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::evidence::{
    PackageReviewRepresentationAbiCommitment, PackageReviewRepresentationMechanism,
    PackageReviewRepresentationTcb, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
};
use crate::projection::source::{ProjectedReviewRow, ProjectedSemanticDependencyRow};
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(crate) fn project_semantic_dependencies(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedSemanticDependencyRow>, Vec<Diagnostic>> {
    let derived = psi_typed_trees_to_checked_trees::derive_checked_semantic_dependencies(
        &compilation.typed,
        &compilation.facts,
    );
    if derived != compilation.facts.flow.semantic_dependencies {
        return Err(vec![Diagnostic::error(format!(
            "retained checked semantic-dependency evidence does not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation.facts.flow.semantic_dependencies.rows.len(),
            derived.rows.len(),
        ))]);
    }

    let mut projected: Vec<ProjectedSemanticDependencyRow> = Vec::new();
    for checked in &compilation.facts.flow.semantic_dependencies.rows {
        let consumer = nominal_identity(compilation, checked.consumer_machine)?;
        if !reviewed_package_owns(&consumer, package)? {
            continue;
        }
        let row = PackageReviewSemanticDependency {
            consumer,
            dependency: nominal_identity(compilation, checked.dependency)?,
            exposure: match checked.exposure {
                psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation => {
                    PackageReviewSemanticDependencyExposure::PrivateImplementation
                }
                psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface => {
                    PackageReviewSemanticDependencyExposure::PublicInterface
                }
            },
            kind: match checked.kind {
                psi_checked_trees::CheckedSemanticDependencyKind::NominalIdentity => {
                    PackageReviewSemanticDependencyKind::NominalIdentity
                }
                psi_checked_trees::CheckedSemanticDependencyKind::Layout => {
                    PackageReviewSemanticDependencyKind::Layout
                }
                psi_checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior => {
                    PackageReviewSemanticDependencyKind::OwnershipBehavior
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanup => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanup
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanupMachine
                }
            },
        };
        if let Some(existing) = projected.iter_mut().find(|existing| existing.row == row) {
            if !existing
                .consumer_declarations
                .contains(&checked.consumer_machine)
            {
                existing
                    .consumer_declarations
                    .push(checked.consumer_machine);
            }
            if !existing
                .dependency_declarations
                .contains(&checked.dependency)
            {
                existing.dependency_declarations.push(checked.dependency);
            }
        } else {
            projected.push(ProjectedSemanticDependencyRow {
                row,
                consumer_declarations: vec![checked.consumer_machine],
                dependency_declarations: vec![checked.dependency],
            });
        }
    }
    projected.sort_by(|left, right| left.row.cmp(&right.row));
    for row in &mut projected {
        row.consumer_declarations
            .sort_by_key(|symbol| symbol.arena_index());
        row.dependency_declarations
            .sort_by_key(|symbol| symbol.arena_index());
    }
    Ok(projected)
}

pub(crate) fn project_representation_tcb(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewRepresentationTcb>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation.data_definitions().iter().filter(|definition| {
        definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque
    }) {
        let declaration = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&declaration, package)? {
            continue;
        }
        rows.push(ProjectedReviewRow {
            row: PackageReviewRepresentationTcb {
                declaration,
                abi: PackageReviewRepresentationAbiCommitment::Unbound,
                mechanism: PackageReviewRepresentationMechanism::Unbound,
            },
            declaration: definition.symbol,
            nested_source_locations: Vec::new(),
        });
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| left.row == right.row && left.declaration == right.declaration);
    Ok(rows)
}
