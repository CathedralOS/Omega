//! Exact projection of checked semantic-dependency evidence.

use super::super::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::capture::source::ProjectedSemanticDependencyRow;
use crate::record::{
    PackageReviewSemanticDependency, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind,
};
use compiler::CheckedCompilation;
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

pub(crate) fn project_semantic_dependencies(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedSemanticDependencyRow>, Vec<Diagnostic>> {
    let derived = typed_trees_to_checked_trees::derive_checked_semantic_dependencies(
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
                checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation => {
                    PackageReviewSemanticDependencyExposure::PrivateImplementation
                }
                checked_trees::CheckedSemanticDependencyExposure::PublicInterface => {
                    PackageReviewSemanticDependencyExposure::PublicInterface
                }
            },
            kind: match checked.kind {
                checked_trees::CheckedSemanticDependencyKind::NominalIdentity => {
                    PackageReviewSemanticDependencyKind::NominalIdentity
                }
                checked_trees::CheckedSemanticDependencyKind::Layout => {
                    PackageReviewSemanticDependencyKind::Layout
                }
                checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior => {
                    PackageReviewSemanticDependencyKind::OwnershipBehavior
                }
                checked_trees::CheckedSemanticDependencyKind::AutomaticCleanup => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanup
                }
                checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine => {
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
