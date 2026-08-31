use super::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::capture::source::locations::project_nested_declaration_source_location;
use crate::capture::source::{
    ProjectedNestedSourceLocation, ProjectedReviewRow, ProjectedSemanticDependencyRow,
};
use crate::record::{
    PackageReviewConformanceShape, PackageReviewConformanceSubject, PackageReviewRepresentationTcb,
    PackageReviewRepresentationTcbKind, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSourceLocationRole,
};
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
    public_conformances: &[ProjectedReviewRow<PackageReviewConformanceShape>],
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
                kind: PackageReviewRepresentationTcbKind::Unbound,
            },
            declaration: definition.symbol,
            nested_source_locations: Vec::new(),
        });
    }

    for conformance in compilation.conformances().iter().filter(|conformance| {
        conformance.is_public
            && omega_representation_planning::is_compiler_owned_opaque_representation_trait(
                &compilation.typed,
                conformance.trait_symbol,
            )
    }) {
        let conformance_identity = nominal_identity(compilation, conformance.symbol)?;
        if !reviewed_package_owns(&conformance_identity, package)?
            || !conformance.lifetime_parameters.is_empty()
            || !compilation
                .conformance_type_parameters(conformance)
                .is_empty()
        {
            continue;
        }
        let trait_arguments = compilation
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        let [opaque_argument] = trait_arguments else {
            return Err(vec![Diagnostic::error(format!(
                "public opaque-representation conformance `{}` does not retain one exact opaque argument",
                conformance_identity.path(),
            ))]);
        };
        let opaque_symbol = compilation
            .type_reference_table
            .type_symbol(*opaque_argument);
        let opaque_definitions = compilation
            .data_definitions()
            .iter()
            .filter(|definition| {
                definition.symbol == opaque_symbol
                    && definition.supply_mode
                        == psi_language_semantics::DataSupplyMode::BoundaryOpaque
            })
            .collect::<Vec<_>>();
        let [opaque_definition] = opaque_definitions.as_slice() else {
            continue;
        };
        let projected_conformances = public_conformances
            .iter()
            .filter(|projected| projected.row.identity() == &conformance_identity)
            .collect::<Vec<_>>();
        let [projected_conformance] = projected_conformances.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "public opaque-representation conformance `{}` maps to {} ordinary public conformance rows; expected one",
                conformance_identity.path(),
                projected_conformances.len(),
            ))]);
        };
        let PackageReviewConformanceSubject::Nominal(carrier_identity) =
            projected_conformance.row.subject()
        else {
            continue;
        };
        let carrier_definitions = compilation
            .data_definitions()
            .iter()
            .filter(|definition| definition.symbol == conformance.carrier_symbol)
            .collect::<Vec<_>>();
        let [carrier_definition] = carrier_definitions.as_slice() else {
            return Err(vec![Diagnostic::error(format!(
                "public opaque-representation conformance `{}` maps to {} carrier declarations; expected one",
                conformance_identity.path(),
                carrier_definitions.len(),
            ))]);
        };
        if carrier_definition.supply_mode != psi_language_semantics::DataSupplyMode::CheckedShape
            || !carrier_definition.is_public
        {
            continue;
        }
        let exact_carrier_identity = nominal_identity(compilation, carrier_definition.symbol)?;
        if carrier_identity != &exact_carrier_identity {
            return Err(vec![Diagnostic::error(format!(
                "public opaque-representation conformance `{}` disagrees with its ordinary public carrier row",
                conformance_identity.path(),
            ))]);
        }
        let declaration = nominal_identity(compilation, opaque_definition.symbol)?;
        let nested_source_locations = [opaque_definition.symbol, carrier_definition.symbol]
            .into_iter()
            .map(|symbol| {
                project_nested_declaration_source_location(
                    compilation,
                    symbol,
                    PackageReviewSourceLocationRole::Declaration,
                    "opaque-representation availability",
                )
            })
            .collect::<Result<Vec<ProjectedNestedSourceLocation>, _>>()?;
        rows.push(ProjectedReviewRow {
            row: PackageReviewRepresentationTcb {
                declaration,
                kind: PackageReviewRepresentationTcbKind::ProducerAvailability {
                    conformance: conformance_identity,
                    carrier: exact_carrier_identity,
                },
            },
            declaration: conformance.symbol,
            nested_source_locations,
        });
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| left.row == right.row && left.declaration == right.declaration);
    Ok(rows)
}
