//! Project ordinary public trait contracts from checked declarations.

use super::declarations::{ContractScope, unavailable};
use super::input::PolicySource;
use crate::record::{
    PackagePolicyTermination, PackagePolicyTraitRequirement, PackagePolicyTraitShape,
    PackageReviewBorrowOrigin, PackageReviewParameterQualification,
    PackageReviewReturnedBorrowOrigin, PackageReviewSignatureParameter, PackageReviewTypeIdentity,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;

impl PolicySource<'_> {
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
            if source_compiler::source_state::source_subjects::contains(
                typed,
                declaration.symbol,
                &[Some(package)],
                declaration_contracts::source_identity::DependencyScope::Build,
            )
            .map_err(unavailable)?
            {
                return Err(unavailable("Build-scope public trait declarations"));
            }
            if !self.owns(declaration.symbol, package)? {
                continue;
            }
            if !declaration.type_parameters.is_empty()
                || !declaration.lifetime_parameters.is_empty()
                || !declaration.conformance_bounds.is_empty()
                || !typed.trait_requirements(declaration).is_empty()
                || declaration.refines.is_some()
                || !declaration.refinement_clauses.is_empty()
            {
                return Err(unavailable(
                    "the ordinary public trait's complete generic/parent/refinement contract",
                ));
            }
            let mut requirements = Vec::new();
            for signature in typed.trait_machine_signatures(declaration) {
                let metadata = self
                    .accepted()
                    .requirement_review(signature.symbol)
                    .map_err(|reason| unavailable(&reason))?;
                if signature.is_default
                    || !signature.type_parameters.is_empty()
                    || !signature.conformance_bounds.is_empty()
                    || !signature.native_callback_parameters.is_empty()
                    || signature.spelling.is_some()
                    || signature.service_reach_is_installation_bound
                {
                    return Err(unavailable(
                        "the ordinary public requirement's complete static/default/callback contract",
                    ));
                }
                let formals = typed.state_signature_parameters(signature);
                if formals.len() != metadata.parameter_types.len() {
                    return Err(unavailable(
                        "the public requirement's exact checked formal namespace",
                    ));
                }
                // Type identities come from the shared whole-signature
                // normalizer. Validate all source dependency ownership too.
                for formal in &formals {
                    self.require_type_dependencies(
                        formal.type_reference,
                        crate::record::PackageReviewSemanticDependencyExposure::PublicInterface,
                    )?;
                }
                if signature.return_type.is_valid() {
                    self.require_type_dependencies(
                        signature.return_type,
                        crate::record::PackageReviewSemanticDependencyExposure::PublicInterface,
                    )?;
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
                let service_reach = metadata
                    .service_reach
                    .as_ref()
                    .map(|services| {
                        services
                            .iter()
                            .map(|symbol| self.nominal(*symbol))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .transpose()?;
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
                requirements.push(PackagePolicyTraitRequirement {
                    identity: self.nominal(signature.symbol)?,
                    spelling: None,
                    has_default_realization: false,
                    lifetime_parameter_count: metadata.lifetime_parameter_count,
                    type_parameters: Vec::new(),
                    conformance_bounds: Vec::new(),
                    parameters,
                    return_type: metadata.result_type.as_ref().map(|canonical| {
                        PackageReviewTypeIdentity {
                            canonical: canonical.clone(),
                        }
                    }),
                    contracts,
                    published_crash: (!metadata.crash_is_open).then(Vec::new),
                    service_reach,
                    service_reach_is_installation_bound: false,
                    synchronous_invocations: Vec::new(),
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
                });
            }
            requirements.sort_by(|left, right| left.identity.cmp(&right.identity));
            rows.push(PackagePolicyTraitShape {
                identity: self.nominal(declaration.symbol)?,
                is_boundary: false,
                lifetime_parameter_count: 0,
                type_parameters: Vec::new(),
                conformance_bounds: Vec::new(),
                parents: Vec::new(),
                requirements,
            });
        }
        rows.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(rows)
    }
}
