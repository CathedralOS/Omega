use super::semantics::declarations::{nominal_identity, reviewed_package_owns};
use super::semantics::types::review_type_identity_with_binders;
use crate::capture::source::locations::project_nested_declaration_source_location;
use crate::capture::source::{
    ProjectedNestedSourceLocation, ProjectedReviewRow, ProjectedSemanticDependencyRow,
};
use crate::record::{
    PackageReviewBoundaryCallingPolicy, PackageReviewBoundaryShape,
    PackageReviewBoundaryShapeClass, PackageReviewBoundaryShapeField,
    PackageReviewBoundaryShapeGraph, PackageReviewBoundaryValueClass,
    PackageReviewBoundaryValueLocation, PackageReviewBoundaryValuePlacement,
    PackageReviewBoundaryValueShape, PackageReviewConformanceShape,
    PackageReviewConformanceSubject, PackageReviewIndirectPointerLocation,
    PackageReviewMachineRegister, PackageReviewOpaqueRepresentationApplicationOrigin,
    PackageReviewOpaqueRepresentationCopyDisposition,
    PackageReviewOpaqueRepresentationLifecycleDisposition,
    PackageReviewOpaqueRepresentationMovementRole, PackageReviewOpaqueRepresentationOccurrence,
    PackageReviewOpaqueRepresentationPathElement, PackageReviewRepresentationArchitecture,
    PackageReviewRepresentationObjectFormat, PackageReviewRepresentationTarget,
    PackageReviewRepresentationTargetProfile, PackageReviewRepresentationTcb,
    PackageReviewRepresentationTcbKind, PackageReviewSemanticDependency,
    PackageReviewSemanticDependencyExposure, PackageReviewSemanticDependencyKind,
    PackageReviewSourceLocationRole, PackageReviewSystemVEightbyteClass,
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

    let selections =
        if let Some(first_selection) = compilation.opaque_representation_selections().first() {
            omega_representation_planning::rederive_opaque_representation_selections(
                &compilation.typed,
                Some(first_selection.selecting_machine()),
                compilation.opaque_representation_selections(),
            )?
        } else {
            Vec::new()
        };

    if let Some(first_selection) = selections.first() {
        let selecting_machine = nominal_identity(compilation, first_selection.selecting_machine())?;
        let selection_owned_by_package = reviewed_package_owns(&selecting_machine, package)?;
        for selection in selections.iter().filter(|selection| {
            selection_owned_by_package
                && selection.copy_disposition()
                    == omega_representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        }) {
            let opaque_definitions = compilation
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == selection.opaque())
                .collect::<Vec<_>>();
            let [opaque_definition] = opaque_definitions.as_slice() else {
                return Err(vec![Diagnostic::error(format!(
                    "selected opaque copy receipt maps to {} declarations; expected one",
                    opaque_definitions.len(),
                ))]);
            };
            let declaration = nominal_identity(compilation, opaque_definition.symbol)?;
            if selection.schema_version()
                != omega_representation_planning::OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION
                || selection.selected_application_commitment() == [0; 32]
                || selection.selected_application_commitment()
                    != selection.rederived_selected_application_commitment()
            {
                return Err(vec![Diagnostic::error(
                    "selected opaque copy receipt has stale application custody",
                )]);
            }
            let conformance = nominal_identity(compilation, selection.application().declaration)?;
            let carrier = nominal_identity(compilation, selection.carrier())?;
            let mut nested_source_locations = vec![ProjectedNestedSourceLocation {
                source_span: selection.source_span(),
                role: PackageReviewSourceLocationRole::RepresentationSelection,
            }];
            nested_source_locations.push(project_nested_declaration_source_location(
                compilation,
                selection.opaque(),
                PackageReviewSourceLocationRole::Declaration,
                "opaque copy receipt declaration",
            )?);
            nested_source_locations.push(project_nested_declaration_source_location(
                compilation,
                selection.application().declaration,
                PackageReviewSourceLocationRole::Declaration,
                "opaque copy receipt conformance",
            )?);
            nested_source_locations.push(project_nested_declaration_source_location(
                compilation,
                selection.carrier(),
                PackageReviewSourceLocationRole::Declaration,
                "opaque copy receipt carrier",
            )?);
            rows.push(ProjectedReviewRow {
                row: PackageReviewRepresentationTcb {
                    declaration,
                    kind: PackageReviewRepresentationTcbKind::SelectedCopyReceipt {
                        conformance,
                        carrier,
                        representation_schema_version: selection.schema_version(),
                        origin: match selection.origin() {
                            omega_representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance => PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
                        },
                        lifecycle: match selection.lifecycle() {
                            omega_representation_planning::OpaqueRepresentationLifecycleDisposition::Inert => PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
                        },
                        copy_disposition:
                            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy,
                        conformance_application_commitment: selection
                            .application()
                            .commitment
                            .as_bytes(),
                        selected_application_commitment: selection
                            .selected_application_commitment(),
                    },
                },
                declaration: selection.selecting_machine(),
                nested_source_locations,
            });
        }
    }

    for realization in compilation.boundary_calling_plan_realizations() {
        let uses = realization
            .materialized_signature
            .opaque_representation_uses();
        if uses.is_empty() {
            continue;
        }
        let target = project_representation_target(compilation)?;
        let (validated, replayed_report, replayed_commitment) = realization
            .replayed_validated_application()
            .map_err(|reason| {
                vec![
                    Diagnostic::error(format!(
                        "representation demand cannot replay its boundary plan: {reason}"
                    ))
                    .with_source_span(realization.relationship_span),
                ]
            })?;
        if replayed_report != realization.report_fingerprint
            || replayed_commitment != realization.commitment
            || replayed_commitment.is_zero()
        {
            return Err(vec![Diagnostic::error(
                "representation demand disagrees with retained boundary-plan application custody",
            )
            .with_source_span(realization.relationship_span)]);
        }
        if realization.materialized_signature.native_target()
            != compilation.selected_native_target().ok_or_else(|| {
                vec![Diagnostic::error(
                    "representation demand has no selected native target",
                )]
            })?
        {
            return Err(vec![
                Diagnostic::error(
                    "representation demand target disagrees with the checked compilation",
                )
                .with_source_span(realization.relationship_span),
            ]);
        }

        let boundary_trait = nominal_identity(compilation, realization.boundary_trait)?;
        let boundary_arguments = realization
            .boundary_arguments
            .iter()
            .map(|argument| review_type_identity_with_binders(compilation, *argument, &[]))
            .collect::<Result<Vec<_>, _>>()?;
        let requirement = nominal_identity(compilation, realization.requirement_machine)?;
        let shape_graph = project_boundary_shape_graph(&realization.materialized_signature);
        let calling_policy = project_calling_policy(validated.plan().call.policy);

        let mut opaque_symbols = uses
            .iter()
            .map(|representation| representation.opaque())
            .collect::<Vec<_>>();
        opaque_symbols.sort_by_key(|symbol| symbol.arena_index());
        opaque_symbols.dedup();
        for opaque in opaque_symbols {
            let matching_selections = selections
                .iter()
                .filter(|selection| selection.opaque() == opaque)
                .collect::<Vec<_>>();
            let [selection] = matching_selections.as_slice() else {
                return Err(vec![
                    Diagnostic::error(format!(
                        "representation demand maps to {} selected representations; expected one",
                        matching_selections.len(),
                    ))
                    .with_source_span(realization.relationship_span),
                ]);
            };
            let selecting_machine = nominal_identity(compilation, selection.selecting_machine())?;
            if !reviewed_package_owns(&selecting_machine, package)? {
                continue;
            }
            let matching_uses = uses
                .iter()
                .filter(|representation| representation.opaque() == opaque)
                .collect::<Vec<_>>();
            if matching_uses.iter().any(|representation| {
                representation.conformance() != selection.application().declaration
                    || representation.carrier() != selection.carrier()
                    || representation.application_report_fingerprint()
                        != selection.application().report_fingerprint
                    || representation.conformance_application_commitment()
                        != selection.application().commitment.as_bytes()
                    || representation.representation_schema_version() != selection.schema_version()
                    || representation.origin() != selection.origin()
                    || representation.lifecycle() != selection.lifecycle()
                    || representation.copy_disposition() != selection.copy_disposition()
                    || representation.selected_application_commitment()
                        != selection.selected_application_commitment()
            }) {
                return Err(vec![Diagnostic::error(
                    "representation demand disagrees with independently rederived selection custody",
                )
                .with_source_span(realization.relationship_span)]);
            }

            let mut occurrences = matching_uses
                .into_iter()
                .map(|representation| {
                    let movement = realization
                        .materialized_signature
                        .opaque_representation_movement(representation, &validated)
                        .map_err(|reason| {
                            vec![Diagnostic::error(format!(
                                "representation demand cannot rejoin physical movement: {reason}"
                            ))
                            .with_source_span(realization.relationship_span)]
                        })?;
                    Ok(PackageReviewOpaqueRepresentationOccurrence {
                        carrier_shape_root: representation.shape_root(),
                        role: match movement.role() {
                            omega_provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationMovementRole::Parameter {
                                formal_ordinal,
                                native_ordinal,
                            } => PackageReviewOpaqueRepresentationMovementRole::Parameter {
                                formal_ordinal,
                                native_ordinal,
                            },
                            omega_provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationMovementRole::Result => {
                                PackageReviewOpaqueRepresentationMovementRole::Result
                            }
                        },
                        path: movement
                            .path()
                            .iter()
                            .map(|element| match element {
                                omega_provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationPathElement::FixedArrayElement => {
                                    PackageReviewOpaqueRepresentationPathElement::FixedArrayElement
                                }
                                omega_provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationPathElement::RecordField { ordinal } => {
                                    PackageReviewOpaqueRepresentationPathElement::RecordField {
                                        ordinal: *ordinal,
                                    }
                                }
                            })
                            .collect(),
                        placement: project_value_placement(movement.placement()),
                    })
                })
                .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            occurrences.sort();
            if occurrences.is_empty() {
                return Err(vec![
                    Diagnostic::error("representation demand has no by-value occurrence")
                        .with_source_span(realization.relationship_span),
                ]);
            }

            let declaration = nominal_identity(compilation, opaque)?;
            let mut nested_source_locations = vec![
                ProjectedNestedSourceLocation {
                    source_span: selection.source_span(),
                    role: PackageReviewSourceLocationRole::RepresentationSelection,
                },
                ProjectedNestedSourceLocation {
                    source_span: realization.relationship_span,
                    role: PackageReviewSourceLocationRole::TraitParent,
                },
            ];
            for (symbol, role, subject) in [
                (
                    realization.boundary_trait,
                    PackageReviewSourceLocationRole::Declaration,
                    "representation demand boundary trait",
                ),
                (
                    realization.requirement_machine,
                    PackageReviewSourceLocationRole::TraitRequirement,
                    "representation demand requirement",
                ),
                (
                    opaque,
                    PackageReviewSourceLocationRole::Declaration,
                    "representation demand opaque declaration",
                ),
                (
                    selection.application().declaration,
                    PackageReviewSourceLocationRole::Declaration,
                    "representation demand conformance",
                ),
                (
                    selection.carrier(),
                    PackageReviewSourceLocationRole::Declaration,
                    "representation demand carrier",
                ),
            ] {
                nested_source_locations.push(project_nested_declaration_source_location(
                    compilation,
                    symbol,
                    role,
                    subject,
                )?);
            }
            rows.push(ProjectedReviewRow {
                row: PackageReviewRepresentationTcb {
                    declaration,
                    kind: PackageReviewRepresentationTcbKind::ConsumerDemand {
                        boundary_trait: boundary_trait.clone(),
                        boundary_arguments: boundary_arguments.clone(),
                        requirement: requirement.clone(),
                        requirement_identity: realization
                            .materialized_signature
                            .owner_requirement_identity()
                            .to_owned(),
                        target,
                        conformance: nominal_identity(
                            compilation,
                            selection.application().declaration,
                        )?,
                        carrier: nominal_identity(compilation, selection.carrier())?,
                        representation_schema_version: selection.schema_version(),
                        origin: project_representation_origin(selection.origin()),
                        lifecycle: project_representation_lifecycle(selection.lifecycle()),
                        copy_disposition: project_representation_copy_disposition(
                            selection.copy_disposition(),
                        ),
                        shape_graph: shape_graph.clone(),
                        occurrences,
                        calling_policy,
                        conformance_application_commitment: selection
                            .application()
                            .commitment
                            .as_bytes(),
                        selected_application_commitment: selection
                            .selected_application_commitment(),
                        boundary_plan_commitment: replayed_commitment.as_bytes(),
                    },
                },
                declaration: selection.selecting_machine(),
                nested_source_locations,
            });
        }
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

fn project_representation_origin(
    origin: omega_representation_planning::OpaqueRepresentationApplicationOrigin,
) -> PackageReviewOpaqueRepresentationApplicationOrigin {
    match origin {
        omega_representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance => {
            PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance
        }
    }
}

fn project_representation_lifecycle(
    lifecycle: omega_representation_planning::OpaqueRepresentationLifecycleDisposition,
) -> PackageReviewOpaqueRepresentationLifecycleDisposition {
    match lifecycle {
        omega_representation_planning::OpaqueRepresentationLifecycleDisposition::Inert => {
            PackageReviewOpaqueRepresentationLifecycleDisposition::Inert
        }
    }
}

fn project_representation_copy_disposition(
    disposition: omega_representation_planning::OpaqueRepresentationCopyDisposition,
) -> PackageReviewOpaqueRepresentationCopyDisposition {
    match disposition {
        omega_representation_planning::OpaqueRepresentationCopyDisposition::PlacementOnly => {
            PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly
        }
        omega_representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy => {
            PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        }
    }
}

fn project_representation_target(
    compilation: &CheckedCompilation,
) -> Result<PackageReviewRepresentationTarget, Vec<Diagnostic>> {
    let profile = compilation.selected_target_profile().ok_or_else(|| {
        vec![Diagnostic::error(
            "representation demand requires a selected target profile",
        )]
    })?;
    let native = compilation.selected_native_target().ok_or_else(|| {
        vec![Diagnostic::error(
            "representation demand requires a selected native target",
        )]
    })?;
    if profile.native_target() != native {
        return Err(vec![Diagnostic::error(
            "representation demand target profile disagrees with its native target",
        )]);
    }
    Ok(PackageReviewRepresentationTarget {
        profile: match profile {
            omega_target::TargetProfile::LinuxArm64 => {
                PackageReviewRepresentationTargetProfile::LinuxArm64
            }
            omega_target::TargetProfile::LinuxX64 => {
                PackageReviewRepresentationTargetProfile::LinuxX64
            }
            omega_target::TargetProfile::MacosArm64 => {
                PackageReviewRepresentationTargetProfile::MacosArm64
            }
            omega_target::TargetProfile::WindowsX64 => {
                PackageReviewRepresentationTargetProfile::WindowsX64
            }
            omega_target::TargetProfile::UefiX64 => {
                PackageReviewRepresentationTargetProfile::UefiX64
            }
            omega_target::TargetProfile::CrossPlatformCli => {
                PackageReviewRepresentationTargetProfile::CrossPlatformCli
            }
            omega_target::TargetProfile::LocalUnchecked => {
                PackageReviewRepresentationTargetProfile::LocalUnchecked
            }
        },
        architecture: match native.architecture {
            omega_target::Architecture::Aarch64 => PackageReviewRepresentationArchitecture::Aarch64,
            omega_target::Architecture::X86_64 => PackageReviewRepresentationArchitecture::X86_64,
        },
        object_format: match native.object_format {
            omega_target::ObjectFormat::Elf => PackageReviewRepresentationObjectFormat::Elf,
            omega_target::ObjectFormat::MachO => PackageReviewRepresentationObjectFormat::MachO,
            omega_target::ObjectFormat::Coff => PackageReviewRepresentationObjectFormat::Coff,
        },
        pointer_size: u16::try_from(native.pointer_size).map_err(|_| {
            vec![Diagnostic::error(
                "representation demand pointer size exceeds canonical evidence",
            )]
        })?,
        pointer_alignment: u16::try_from(native.pointer_alignment).map_err(|_| {
            vec![Diagnostic::error(
                "representation demand pointer alignment exceeds canonical evidence",
            )]
        })?,
    })
}

fn project_boundary_shape_graph(
    signature: &omega_provider_planning::calling_policy_plans::MaterializedBoundarySignature,
) -> PackageReviewBoundaryShapeGraph {
    PackageReviewBoundaryShapeGraph {
        shapes: signature
            .shapes()
            .iter()
            .map(|shape| PackageReviewBoundaryShape {
                class: match shape.class() {
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Integer => {
                        PackageReviewBoundaryShapeClass::Integer
                    }
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Float => {
                        PackageReviewBoundaryShapeClass::Float
                    }
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Reference => {
                        PackageReviewBoundaryShapeClass::Reference
                    }
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::FixedArray {
                        element,
                        length,
                    } => PackageReviewBoundaryShapeClass::FixedArray { element, length },
                    omega_provider_planning::calling_policy_plans::BoundaryValueClass::Record {
                        first_field,
                        field_count,
                    } => PackageReviewBoundaryShapeClass::Record {
                        first_field,
                        field_count,
                    },
                },
                byte_size: shape.byte_size(),
                alignment: shape.alignment(),
            })
            .collect(),
        fields: signature
            .fields()
            .iter()
            .map(|field| PackageReviewBoundaryShapeField {
                shape: field.shape(),
                byte_offset: field.byte_offset(),
            })
            .collect(),
        parameters: signature.parameters().to_vec(),
        result: signature.result(),
    }
}

fn project_calling_policy(
    policy: omega_calling_conventions::CallingPolicy,
) -> PackageReviewBoundaryCallingPolicy {
    match policy {
        omega_calling_conventions::CallingPolicy::MicrosoftX64 => {
            PackageReviewBoundaryCallingPolicy::MicrosoftX64
        }
        omega_calling_conventions::CallingPolicy::SystemVAMD64 => {
            PackageReviewBoundaryCallingPolicy::SystemVAMD64
        }
        omega_calling_conventions::CallingPolicy::Aapcs64 => {
            PackageReviewBoundaryCallingPolicy::Aapcs64
        }
        omega_calling_conventions::CallingPolicy::LinuxSyscallX86_64 => {
            PackageReviewBoundaryCallingPolicy::LinuxSyscallX86_64
        }
        omega_calling_conventions::CallingPolicy::LinuxSyscallAarch64 => {
            PackageReviewBoundaryCallingPolicy::LinuxSyscallAarch64
        }
    }
}

fn project_value_placement(
    placement: &omega_calling_conventions::ValuePlacement,
) -> PackageReviewBoundaryValuePlacement {
    PackageReviewBoundaryValuePlacement {
        shape: PackageReviewBoundaryValueShape {
            class: match placement.shape.class {
                omega_calling_conventions::ValueClass::Integer => {
                    PackageReviewBoundaryValueClass::Integer
                }
                omega_calling_conventions::ValueClass::Float => {
                    PackageReviewBoundaryValueClass::Float
                }
                omega_calling_conventions::ValueClass::HomogeneousFloatAggregate { members } => {
                    PackageReviewBoundaryValueClass::HomogeneousFloatAggregate { members }
                }
                omega_calling_conventions::ValueClass::SystemVAggregate { first, second } => {
                    PackageReviewBoundaryValueClass::SystemVAggregate {
                        first: project_system_v_class(first),
                        second: project_system_v_class(second),
                    }
                }
            },
            byte_size: placement.shape.byte_size,
            alignment: placement.shape.alignment,
        },
        locations: placement
            .locations
            .iter()
            .map(|location| match *location {
                omega_calling_conventions::ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => PackageReviewBoundaryValueLocation::Register {
                    register: project_machine_register(register),
                    value_byte_offset,
                    byte_size,
                },
                omega_calling_conventions::ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    alignment,
                },
                omega_calling_conventions::ValueLocation::Indirect {
                    pointer,
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                } => PackageReviewBoundaryValueLocation::Indirect {
                    pointer: match pointer {
                        omega_calling_conventions::IndirectPointerLocation::Register(register) => {
                            PackageReviewIndirectPointerLocation::Register(
                                project_machine_register(register),
                            )
                        }
                        omega_calling_conventions::IndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        } => PackageReviewIndirectPointerLocation::Stack {
                            stack_byte_offset,
                            alignment,
                        },
                    },
                    copy_stack_byte_offset,
                    byte_size,
                    alignment,
                },
            })
            .collect(),
    }
}

fn project_system_v_class(
    class: omega_calling_conventions::SystemVEightbyteClass,
) -> PackageReviewSystemVEightbyteClass {
    match class {
        omega_calling_conventions::SystemVEightbyteClass::Integer => {
            PackageReviewSystemVEightbyteClass::Integer
        }
        omega_calling_conventions::SystemVEightbyteClass::Sse => {
            PackageReviewSystemVEightbyteClass::Sse
        }
    }
}

fn project_machine_register(
    register: omega_calling_conventions::MachineRegister,
) -> PackageReviewMachineRegister {
    match register {
        omega_calling_conventions::MachineRegister::X86Rax => PackageReviewMachineRegister::X86Rax,
        omega_calling_conventions::MachineRegister::X86Rcx => PackageReviewMachineRegister::X86Rcx,
        omega_calling_conventions::MachineRegister::X86Rdx => PackageReviewMachineRegister::X86Rdx,
        omega_calling_conventions::MachineRegister::X86Rbx => PackageReviewMachineRegister::X86Rbx,
        omega_calling_conventions::MachineRegister::X86Rsp => PackageReviewMachineRegister::X86Rsp,
        omega_calling_conventions::MachineRegister::X86Rbp => PackageReviewMachineRegister::X86Rbp,
        omega_calling_conventions::MachineRegister::X86Rsi => PackageReviewMachineRegister::X86Rsi,
        omega_calling_conventions::MachineRegister::X86Rdi => PackageReviewMachineRegister::X86Rdi,
        omega_calling_conventions::MachineRegister::X86R8 => PackageReviewMachineRegister::X86R8,
        omega_calling_conventions::MachineRegister::X86R9 => PackageReviewMachineRegister::X86R9,
        omega_calling_conventions::MachineRegister::X86R10 => PackageReviewMachineRegister::X86R10,
        omega_calling_conventions::MachineRegister::X86R11 => PackageReviewMachineRegister::X86R11,
        omega_calling_conventions::MachineRegister::X86R12 => PackageReviewMachineRegister::X86R12,
        omega_calling_conventions::MachineRegister::X86R13 => PackageReviewMachineRegister::X86R13,
        omega_calling_conventions::MachineRegister::X86R14 => PackageReviewMachineRegister::X86R14,
        omega_calling_conventions::MachineRegister::X86R15 => PackageReviewMachineRegister::X86R15,
        omega_calling_conventions::MachineRegister::X86Xmm(index) => {
            PackageReviewMachineRegister::X86Xmm(index)
        }
        omega_calling_conventions::MachineRegister::Aarch64X(index) => {
            PackageReviewMachineRegister::Aarch64X(index)
        }
        omega_calling_conventions::MachineRegister::Aarch64V(index) => {
            PackageReviewMachineRegister::Aarch64V(index)
        }
    }
}
