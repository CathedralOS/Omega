//! Project ordinary public trait contracts from checked declarations.

use super::declarations::{ContractScope, unavailable};
use super::input::PolicySource;
use diagnostics::Diagnostic;
use review_evidence::record::{
    PackagePolicyTermination, PackagePolicyTraitRequirement, PackagePolicyTraitShape,
    PackageReviewBorrowOrigin, PackageReviewParameterQualification,
    PackageReviewReturnedBorrowOrigin, PackageReviewSemanticDependencyExposure,
    PackageReviewSignatureParameter, PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;

impl PolicySource<'_> {
    pub(super) fn public_conformances(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<review_evidence::record::PackagePolicyConformanceShape>, Vec<Diagnostic>> {
        use review_evidence::record::{
            PackagePolicyConformanceShape, PackageReviewEvidenceInterface,
            PackageReviewEvidenceRequirement,
        };
        use source_compiler::representations::source_state::trait_definition::AppliedTraitScope;
        let program = self.typed();
        let mut rows = Vec::new();
        for c in program.conformances().iter().filter(|c| c.is_public) {
            if program.symbols.symbol_package_identity(c.symbol) != Some(package) {
                continue;
            }
            let root = AppliedTraitScope::from_conformance(program, c)
                .ok_or_else(|| unavailable("a public conformance's checked schema application"))?;
            let mut scopes = Vec::new();
            root.try_visit(program, &mut |scope| {
                scopes.push(scope.clone());
                Ok(())
            })
            .map_err(unavailable)?;
            let arguments = program
                .type_reference_table
                .type_reference_handles(c.arguments)
                .iter()
                .map(|r| self.declared_type(c.symbol, *r))
                .collect::<Result<Vec<_>, _>>()?;
            let mut requirements = Vec::new();
            for scope in scopes {
                let is_root = scope.declaration.symbol == root.declaration.symbol;
                if !is_root
                    && (!scope.declaration.type_parameters.is_empty()
                        || !scope.declaration.lifetime_parameters.is_empty())
                {
                    return Err(unavailable(
                        "inherited evidence with forwarded generic arguments",
                    ));
                }
                for signature in program.trait_machine_signatures(scope.declaration) {
                    requirements.push(PackageReviewEvidenceRequirement {
                        declaring_trait: self.nominal(scope.declaration.symbol)?,
                        declaring_trait_lifetime_arguments: if is_root {
                            c.trait_lifetime_arguments.clone()
                        } else {
                            Vec::new()
                        },
                        declaring_trait_arguments: if is_root {
                            arguments.clone()
                        } else {
                            Vec::new()
                        },
                        requirement: self.nominal(signature.symbol)?,
                    });
                }
            }
            requirements.sort();
            requirements.dedup();
            rows.push(PackagePolicyConformanceShape {
                identity: self.nominal(c.symbol)?,
                lifetime_parameter_count: c.lifetime_parameters.len(),
                type_parameters: self.type_parameters(c.symbol)?,
                interface: PackageReviewEvidenceInterface {
                    trait_identity: self.nominal(c.trait_symbol)?,
                    lifetime_arguments: c.trait_lifetime_arguments.clone(),
                    arguments,
                    requirements,
                },
            });
        }
        rows.sort_by(|a, b| a.identity.cmp(&b.identity));
        Ok(rows)
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
            || !signature.type_parameters.is_empty()
            || !signature.conformance_bounds.is_empty()
            || !signature.native_callback_parameters.is_empty()
            || signature.spelling.is_some()
        {
            return Err(unavailable(
                "the requirement's complete static/default/callback contract",
            ));
        }
        let formals = typed.state_signature_parameters(signature);
        if formals.len() != metadata.parameter_types.len() {
            return Err(unavailable(
                "the public requirement's exact checked formal namespace",
            ));
        }
        let exposure = if declaration.is_public {
            PackageReviewSemanticDependencyExposure::PublicInterface
        } else {
            PackageReviewSemanticDependencyExposure::PrivateImplementation
        };
        for formal in &formals {
            self.require_type_dependencies(signature.symbol, formal.type_reference, exposure)?;
        }
        if signature.return_type.is_valid() {
            self.require_type_dependencies(signature.symbol, signature.return_type, exposure)?;
        }
        let parameters = formals
            .iter()
            .zip(&metadata.parameter_types)
            .map(|(formal, canonical)| PackageReviewSignatureParameter {
                name: formal.name.as_str().to_owned(),
                type_identity: PackageReviewTypeIdentity {
                    canonical: canonical.clone(),
                },
                is_const: formal.is_const,
                is_mutable: formal.is_mutable,
            })
            .collect();
        let contracts = self.signature_contracts(
            typed.state_signature_contracts(signature),
            ContractScope {
                owner: signature.symbol,
                subject: None,
                statics: Default::default(),
                parameters: formals,
                result: None,
            },
            signature.return_type,
        )?;
        let mut service_reach = metadata
            .service_reach
            .as_ref()
            .map(|services| {
                services
                    .iter()
                    .map(|symbol| self.nominal(*symbol))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;
        if let Some(services) = &mut service_reach {
            services.sort();
            services.dedup();
        }
        let mut qualifications = metadata
            .parameter_qualifications
            .iter()
            .map(|(structural_index, domain)| {
                self.nominal(*domain)
                    .map(|domain| PackageReviewParameterQualification {
                        structural_index: *structural_index,
                        domain,
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        qualifications.sort();
        qualifications.dedup();
        Ok(PackagePolicyTraitRequirement {
            identity: self.nominal(signature.symbol)?,
            spelling: None,
            has_default_realization: false,
            lifetime_parameter_count: metadata.lifetime_parameter_count,
            type_parameters: Vec::new(),
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
            published_crash: (!metadata.crash_is_open).then(Vec::new),
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
                        Invocation::Service(symbol) => self.nominal(*symbol).map(Reviewed::Service),
                    })
                    .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
                invocations.sort();
                invocations.dedup();
                invocations
            },
            suspends: metadata.suspends,
            blocks: metadata.blocks,
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
