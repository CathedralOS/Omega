//! Project ordinary public trait contracts from checked declarations.

use super::declarations::{ContractScope, unavailable};
use super::input::PolicySource;
use super::telescope::SignatureScope;
use diagnostics::Diagnostic;
use review_evidence::record::{
    PackagePolicyConformanceShape, PackagePolicyTermination, PackagePolicyTraitRequirement,
    PackagePolicyTraitShape, PackageReviewBorrowOrigin, PackageReviewEvidenceInterface,
    PackageReviewEvidenceRequirement, PackageReviewParameterQualification,
    PackageReviewReturnedBorrowOrigin, PackageReviewSemanticDependencyExposure,
    PackageReviewSignatureParameter, PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_state::trait_definition::AppliedTraitScope;

impl PolicySource<'_> {
    pub(super) fn public_conformances(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<PackagePolicyConformanceShape>, Vec<Diagnostic>> {
        let program = self.typed();
        let mut rows = Vec::new();
        for conformance in program
            .conformances()
            .iter()
            .filter(|conformance| conformance.is_public)
        {
            if program.symbols.symbol_package_identity(conformance.symbol) != Some(package) {
                continue;
            }
            let root = AppliedTraitScope::from_conformance(program, conformance)
                .ok_or_else(|| unavailable("a public conformance's checked schema application"))?;
            let mut scopes = Vec::new();
            root.try_visit(program, &mut |scope| {
                scopes.push(scope.clone());
                Ok(())
            })
            .map_err(unavailable)?;
            let binder_scope = SignatureScope::declaration(self, &[conformance.symbol])?;
            let mut applications = scopes
                .iter()
                .map(|scope| self.conformance_interface(scope, &binder_scope));
            let mut interface = applications
                .next()
                .ok_or_else(|| unavailable("the public conformance's target trait"))??;
            for parent in applications {
                interface.requirements.append(&mut parent?.requirements);
            }
            interface.requirements.sort();
            interface.requirements.dedup();
            rows.push(PackagePolicyConformanceShape {
                identity: self.nominal(conformance.symbol)?,
                lifetime_parameter_count: conformance.lifetime_parameters.len(),
                type_parameters: self.scoped_parameters(conformance.symbol, &binder_scope)?,
                interface,
            });
        }
        rows.sort_by(|a, b| a.identity.cmp(&b.identity));
        Ok(rows)
    }

    fn conformance_interface(
        &self,
        scope: &AppliedTraitScope<'_>,
        binders: &SignatureScope,
    ) -> Result<PackageReviewEvidenceInterface, Vec<Diagnostic>> {
        use source_compiler::representations::source_state::type_identity::{
            LifetimeParameterIdentity, apply_resolved_lifetime_parameter_identity,
        };
        let program = self.typed();
        scope.with_identity_scopes(program, |frames| {
            let lifetime_arguments: Vec<u32> = (0..scope.declaration.lifetime_parameters.len())
                .map(|ordinal| {
                    let formal = LifetimeParameterIdentity {
                        declaration: scope.declaration.symbol,
                        ordinal,
                    };
                    apply_resolved_lifetime_parameter_identity(program, formal, frames)
                        .filter(|(region, _)| region.declaration == scope.argument_owner)
                        .and_then(|(region, _)| u32::try_from(region.ordinal).ok())
                        .ok_or_else(|| unavailable("the checked conformance lifetime argument"))
                })
                .collect::<Result<_, _>>()?;
            let arguments: Vec<_> = program
                .trait_type_parameters(scope.declaration)
                .iter()
                .map(|parameter| {
                    let (reference, outer) = scope
                        .supplied_parameter_application(program, parameter.symbol)
                        .ok_or_else(|| unavailable("the checked conformance static argument"))?;
                    self.applied_scoped_type(
                        scope.argument_owner,
                        reference,
                        binders,
                        frames.get(..outer).ok_or_else(|| {
                            unavailable("the conformance argument's supplying scope")
                        })?,
                    )
                })
                .collect::<Result<_, _>>()?;
            let trait_identity = self.nominal(scope.declaration.symbol)?;
            let signatures = program.trait_machine_signatures(scope.declaration);
            if signatures.len() != scope.declaration.machines.len() {
                return Err(unavailable(
                    "the declaring trait's complete requirement roster",
                ));
            }
            let requirements = signatures
                .iter()
                .map(|signature| {
                    Ok(PackageReviewEvidenceRequirement {
                        declaring_trait: trait_identity.clone(),
                        declaring_trait_lifetime_arguments: lifetime_arguments.clone(),
                        declaring_trait_arguments: arguments.clone(),
                        requirement: self.nominal(signature.symbol)?,
                    })
                })
                .collect::<Result<_, Vec<Diagnostic>>>()?;
            Ok(PackageReviewEvidenceInterface {
                trait_identity,
                lifetime_arguments,
                arguments,
                requirements,
            })
        })
    }

    pub(super) fn public_traits(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<PackagePolicyTraitShape>, Vec<Diagnostic>> {
        let typed = self.typed();
        let mut rows = Vec::new();
        for declaration in typed
            .traits()
            .iter()
            .filter(|row| row.is_public && !row.is_boundary)
        {
            if source_compiler::representations::source_state::source_subjects::contains(
                typed,
                declaration.symbol,
                &[Some(package)],
                source_provenance::DependencyScope::Build,
            )
            .map_err(unavailable)?
            {
                return Err(unavailable("Build-scope public trait declarations"));
            }
            if !self.owns(declaration.symbol, package)? {
                continue;
            }
            let mut requirements = Vec::new();
            for signature in typed.trait_machine_signatures(declaration) {
                requirements.push(self.requirement(declaration, signature)?);
            }
            requirements.sort_by(|left, right| left.identity.cmp(&right.identity));
            rows.push(self.trait_shape(declaration, requirements)?);
        }
        rows.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(rows)
    }
    pub(super) fn trait_shape(
        &self,
        declaration: &source_compiler::representations::source_state::trait_definition::TraitDefinition,
        requirements: Vec<PackagePolicyTraitRequirement>,
    ) -> Result<PackagePolicyTraitShape, Vec<Diagnostic>> {
        use review_evidence::record::{
            PackageReviewTraitCompositionKind, PackageReviewTraitParent,
        };
        use source_compiler::representations::source_state::data::TypeParameterKind;
        if declaration.refines.is_some() || !declaration.refinement_clauses.is_empty() {
            return Err(unavailable("public trait refinement evidence"));
        }
        let source = self.typed();
        let mut parents = Vec::new();
        for parent in source.trait_requirements(declaration) {
            let owner = source
                .trait_definition_by_symbol(parent.symbol)
                .ok_or_else(|| unavailable("a checked parent trait declaration"))?;
            let actual = source
                .type_reference_table
                .type_reference_handles(parent.arguments);
            let formals = source.trait_type_parameters(owner);
            if actual.len() != formals.len()
                || parent.lifetime_arguments.len() != owner.lifetime_parameters.len()
            {
                return Err(unavailable("the complete parent trait telescope"));
            }
            let mut arguments = Vec::new();
            for (argument, formal) in actual.iter().zip(formals) {
                if formal.kind == TypeParameterKind::Implementation {
                    let operand = source
                        .static_operand_from_type_reference(*argument)
                        .map_err(unavailable)?;
                    let symbol = operand.symbol;
                    if !source.symbols.symbols().nodes().is_valid(symbol)
                        || !(source.symbols.get(symbol).kind
                            == symbols::SymbolKind::ConformanceParameter
                            || source
                                .conformance_by_symbol(symbol)
                                .is_some_and(|definition| {
                                    definition.type_parameters.is_empty()
                                        && definition.lifetime_parameters.is_empty()
                                }))
                    {
                        return Err(unavailable(
                            "the checked application of a parent trait implementation argument",
                        ));
                    }
                }
                arguments.push(self.declared_type(declaration.symbol, *argument)?);
            }
            let lifetime_arguments = parent
                .lifetime_arguments
                .iter()
                .map(|argument| {
                    declaration
                        .lifetime_parameters
                        .iter()
                        .position(|parameter| parameter == argument)
                        .and_then(|position| u32::try_from(position).ok())
                        .ok_or_else(|| unavailable("a parent's declared lifetime binder"))
                })
                .collect::<Result<_, _>>()?;
            parents.push(PackageReviewTraitParent {
                kind: if owner.is_boundary {
                    PackageReviewTraitCompositionKind::ServiceReach
                } else {
                    PackageReviewTraitCompositionKind::Policy
                },
                identity: self.nominal(owner.symbol)?,
                lifetime_arguments,
                arguments,
            });
        }
        Ok(PackagePolicyTraitShape {
            identity: self.nominal(declaration.symbol)?,
            is_boundary: declaration.is_boundary,
            lifetime_parameter_count: declaration.lifetime_parameters.len(),
            type_parameters: self.type_parameters(declaration.symbol)?,
            conformance_bounds: self.conformance_bounds(
                declaration.symbol,
                &declaration.lifetime_parameters,
                &declaration.conformance_bounds,
            )?,
            parents,
            requirements,
        })
    }

    pub(super) fn scoped_requirement_parameters(
        &self,
        signature: &source_compiler::representations::source_state::signature::StateSignature,
        parameter_types: &[String],
        exposure: PackageReviewSemanticDependencyExposure,
    ) -> Result<Vec<PackageReviewSignatureParameter>, Vec<Diagnostic>> {
        let formals = self.typed().state_signature_parameters(signature);
        if formals.len() != parameter_types.len() {
            return Err(unavailable(
                "the public requirement's exact checked formal namespace",
            ));
        }
        for formal in &formals {
            self.require_type_dependencies(signature.symbol, formal.type_reference, exposure)?;
        }
        if signature.return_type.is_valid() {
            self.require_type_dependencies(signature.symbol, signature.return_type, exposure)?;
        }
        Ok(formals
            .iter()
            .zip(parameter_types)
            .map(|(formal, canonical)| PackageReviewSignatureParameter {
                name: formal.name.as_str().to_owned(),
                type_identity: PackageReviewTypeIdentity {
                    canonical: canonical.clone(),
                },
                is_const: formal.is_const,
                is_mutable: formal.is_mutable,
            })
            .collect())
    }

    pub(super) fn requirement(
        &self,
        declaration: &source_compiler::representations::source_state::trait_definition::TraitDefinition,
        signature: &source_compiler::representations::source_state::signature::StateSignature,
    ) -> Result<PackagePolicyTraitRequirement, Vec<Diagnostic>> {
        let typed = self.typed();
        let metadata = self
            .accepted()
            .requirement_review(signature.symbol)
            .map_err(|reason| unavailable(&reason))?;
        if signature.is_default
            || !signature.conformance_bounds.is_empty()
            || !signature.native_callback_parameters.is_empty()
            || signature.spelling.is_some()
        {
            return Err(unavailable(
                "the requirement's complete conformance, spelling, default, or callback contract",
            ));
        }
        let exposure = if declaration.is_public {
            PackageReviewSemanticDependencyExposure::PublicInterface
        } else {
            PackageReviewSemanticDependencyExposure::PrivateImplementation
        };
        let parameters =
            self.scoped_requirement_parameters(signature, &metadata.parameter_types, exposure)?;
        let formals = typed.state_signature_parameters(signature);
        let scope = super::telescope::SignatureScope::declaration(
            self,
            &[declaration.symbol, signature.symbol],
        )?;
        let type_parameters = self.scoped_parameters(signature.symbol, &scope)?;
        let mut statics = typed.trait_type_parameters(declaration).to_vec();
        statics.extend(
            typed
                .state_signature_type_parameters(signature)
                .iter()
                .cloned(),
        );
        let clause_scope = ContractScope {
            owner: signature.symbol,
            subject: None,
            statics: statics.as_slice().into(),
            parameters: formals,
            result: None,
        };
        let contracts = self.signature_contracts(
            typed.state_signature_contracts(signature),
            clause_scope,
            signature.return_type,
        )?;
        let mut service_reach = metadata
            .service_reach
            .as_ref()
            .map(|services| {
                services
                    .iter()
                    .map(|service| self.reviewed_requirement_service(service))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        if let Some(services) = &mut service_reach {
            services.sort();
            services.dedup();
        }
        let mut qualifications = Vec::new();
        for (position, domain) in &metadata.parameter_qualifications {
            if let Ok(index) = metadata.structural_parameter_positions.binary_search(position) {
                qualifications.push(PackageReviewParameterQualification {
                    structural_index: u32::try_from(index)
                        .map_err(|_| unavailable("a requirement's structural parameter position"))?,
                    domain: self.nominal(domain.declaration)?,
                });
            }
        }
        qualifications.sort();
        qualifications.dedup();
        Ok(PackagePolicyTraitRequirement {
            identity: self.nominal(signature.symbol)?,
            spelling: None,
            has_default_realization: false,
            lifetime_parameter_count: metadata.lifetime_parameter_count,
            type_parameters,
            conformance_bounds: Vec::new(),
            parameters,
            structural_parameter_positions: Some(metadata.structural_parameter_positions.clone()),
            return_type: metadata
                .result_type
                .as_ref()
                .map(|canonical| PackageReviewTypeIdentity {
                    canonical: canonical.clone(),
                }),
            contracts,
            published_crash: metadata
                .published_crash_routes
                .as_deref()
                .map(|checked| {
                    super::callables::published_source_crash_routes(checked, || {
                        self.signature_crash_routes(
                            typed.state_signature_contracts(signature),
                            clause_scope,
                        )
                    })
                })
                .transpose()?,
            service_reach,
            service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
            synchronous_invocations: {
                use review_evidence::record::PackageReviewSynchronousInvocation as Reviewed;
                use source_compiler::publication::RequirementInvocation as Invocation;
                let mut invocations = metadata
                    .synchronous_invocations
                    .iter()
                    .map(|invocation| match invocation {
                        Invocation::Parameter(position) => Ok(Reviewed::Parameter(*position)),
                        Invocation::Service(service) => self
                            .reviewed_requirement_service(service)
                            .map(Reviewed::Service),
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                invocations.sort();
                invocations.dedup();
                invocations
            },
            suspends: metadata.suspends,
            blocks: metadata.blocks,
            admitted_same_stack_bytes: metadata.admitted_same_stack_bytes,
            termination: if metadata.terminates {
                PackagePolicyTermination::Terminates {
                    premises: Vec::new(),
                }
            } else {
                PackagePolicyTermination::NoGuarantee
            },
            returned_borrow_origins: metadata
                .returned_parameters
                .iter()
                .map(|index| PackageReviewReturnedBorrowOrigin {
                    origin: PackageReviewBorrowOrigin::Parameter { index: *index },
                })
                .collect(),
            parameter_qualification_preconditions: qualifications,
        })
    }
}
