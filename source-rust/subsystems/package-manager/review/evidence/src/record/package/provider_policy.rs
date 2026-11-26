// Receipt-free selected provider meaning, carrying no acceptance or execution
// authority.

mod binding_validation;
mod methods;
mod plans;
mod validation;

use crate::record::accessors::field_projection_accessors;
use crate::record::package::callable_policy::validation::service_set;
use crate::record::package::callable_policy::validation::signature::{Scope, owned_pair};
use crate::record::public_policy::validation::scope;
use crate::record::public_policy::validation::signatures::parameters;
use crate::record::validation::ordered;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyTypeParameter,
    PackageReviewCompilerIntrinsicExecution, PackageReviewForeignLocator,
    PackageReviewNominalIdentity, PackageReviewProviderFamilyCoverage,
    PackageReviewProviderSelectionAuthority, PackageReviewSelectedInstallationReach,
    PackageReviewServiceIdentity, PackageReviewSignatureParameter,
    PackageReviewSynchronousInvocation, PackageReviewTypeIdentity,
};
pub use methods::PackagePolicyServiceMethod;
pub use plans::{PackagePolicyProviderPlan, PackagePolicySelectedProviders};
use semantic_vocabulary::PackageKeyIdentity;
use service_surface::{
    ServiceProgressEstablishmentRouteKind, ServiceProgressSubject, ServiceSynchronousInvocation,
};
use std::slice::from_ref;
use target::TargetProfile;

pub(crate) fn validate_native_service_method(
    method: &PackagePolicyServiceMethod,
    target: target::TargetProfile,
) -> Result<(), &'static str> {
    validate_service_methods(from_ref(method), &method.requirement_owner, target)
}

// Source-qualified operational authority records for service methods, kept independent of readable schemas.

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyServiceAuthority {
    pub service_reach: Vec<PackageReviewServiceIdentity>,
    pub synchronous_invocations: Vec<PackageReviewSynchronousInvocation>,
    pub progress_premises: Vec<PackagePolicyServiceProgressPremise>,
}

impl PackagePolicyServiceAuthority {
    field_projection_accessors!(sliced service_reach: &[PackageReviewServiceIdentity]);
    field_projection_accessors!(sliced synchronous_invocations: &[PackageReviewSynchronousInvocation]);
    field_projection_accessors!(sliced progress_premises: &[PackagePolicyServiceProgressPremise]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyServiceProgressPremise {
    pub(crate) profile: PackageReviewNominalIdentity,
    pub(crate) subject: ServiceProgressSubject,
    pub(crate) subject_projections: Vec<PackageReviewNominalIdentity>,
    pub(crate) establishment_routes: Vec<PackagePolicyServiceProgressRoute>,
}

impl PackagePolicyServiceProgressPremise {
    field_projection_accessors!(borrowed profile: &PackageReviewNominalIdentity);
    field_projection_accessors!(copied subject: ServiceProgressSubject);
    field_projection_accessors!(sliced subject_projections: &[PackageReviewNominalIdentity]);
    field_projection_accessors!(sliced establishment_routes: &[PackagePolicyServiceProgressRoute]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyServiceProgressRoute {
    pub(crate) kind: ServiceProgressEstablishmentRouteKind,
    pub(crate) requirement_owner: PackageReviewNominalIdentity,
    pub(crate) requirement: PackageReviewNominalIdentity,
}

impl PackagePolicyServiceProgressRoute {}

impl self::PackagePolicyServiceMethod {
    pub(super) fn validate_authority(&self) -> Result<(), &'static str> {
        let authority = &self.authority;
        ordered(&authority.service_reach)?;
        ordered(&authority.synchronous_invocations)?;
        ordered(&authority.progress_premises)?;
        ServiceSynchronousInvocation::validate_entries(
            &self.synchronous_invocations,
            self.parameter_count,
        )?;
        let declared_parameters =
            self.synchronous_invocations
                .iter()
                .filter_map(|entry| match entry {
                    ServiceSynchronousInvocation::Parameter(position) => Some(position),
                    ServiceSynchronousInvocation::Service(_) => None,
                });
        let authority_parameters = authority
            .synchronous_invocations
            .iter()
            .filter_map(|entry| match entry {
                PackageReviewSynchronousInvocation::Parameter(position) => Some(position),
                PackageReviewSynchronousInvocation::Service(_) => None,
            });
        if !declared_parameters.eq(authority_parameters) {
            return Err("provider authority differs from its declared invocation parameters");
        }
        if (!self.terminates_guarantee && !authority.progress_premises.is_empty())
            || authority.progress_premises.len() != self.termination_premises.len()
        {
            return Err("provider authority detaches its progress premises");
        }
        service_set(&authority.service_reach)?;
        for invocation in &authority.synchronous_invocations {
            match invocation {
                PackageReviewSynchronousInvocation::Parameter(ordinal) => {
                    if *ordinal as usize >= self.parameter_count {
                        return Err("provider invocation parameter is outside its telescope");
                    }
                }
                PackageReviewSynchronousInvocation::Service(service) => {
                    service.validate_resolved()?
                }
            }
        }
        for premise in &authority.progress_premises {
            premise.profile.validate_resolved()?;
            if matches!(premise.subject, ServiceProgressSubject::Parameter(ordinal) if ordinal >= self.parameter_count)
            {
                return Err("provider progress premise has an invalid subject");
            }
            ordered(&premise.establishment_routes)?;
            for projection in &premise.subject_projections {
                projection.validate_resolved()?;
            }
            for route in &premise.establishment_routes {
                owned_pair(&route.requirement_owner, &route.requirement)?;
            }
        }
        Ok(())
    }
}

// Provider family records and coordinate projections for selected package providers.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderFamily {
    pub(crate) family_identity: PackageReviewNominalIdentity,
    pub(crate) provider_type_declaration: PackageReviewNominalIdentity,
    pub(crate) target: TargetProfile,
    pub(crate) authority: PackageReviewProviderSelectionAuthority,
    pub(crate) coverage: PackageReviewProviderFamilyCoverage,
    pub(crate) coordinates: Vec<PackagePolicyProviderFamilyCoordinate>,
}

impl PackagePolicyProviderFamily {
    field_projection_accessors!(borrowed family_identity: &PackageReviewNominalIdentity);
    field_projection_accessors!(borrowed provider_type_declaration: &PackageReviewNominalIdentity);

    field_projection_accessors!(copied target: TargetProfile);
    field_projection_accessors!(copied authority: PackageReviewProviderSelectionAuthority);
    field_projection_accessors!(copied coverage: PackageReviewProviderFamilyCoverage);
    field_projection_accessors!(sliced coordinates: &[PackagePolicyProviderFamilyCoordinate]);
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PackagePolicyProviderFamilyCoordinate {
    pub(crate) requirement_identity: String,
    pub(crate) operator_declaration: PackageReviewNominalIdentity,
    pub(crate) plan_index: u32,
}

impl PackagePolicyProviderFamilyCoordinate {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    field_projection_accessors!(borrowed operator_declaration: &PackageReviewNominalIdentity);

    field_projection_accessors!(copied plan_index: u32);
}

// Provider plan realization rows and leaf binding mechanisms (preserving evaluated producers without evaluator receipts).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderRow {
    pub method: String,
    pub requirement: PackageReviewNominalIdentity,
    pub requirement_application: Option<String>,
    pub realization: Option<PackageReviewNominalIdentity>, // Authored realization machine (`None` for toolchain-settled rows).
    pub native_build: Option<PackagePolicyEvaluatedBindingProducer>, // Actual Build producer when native supply has no realization body.
    pub requirement_lifetime_partition: Vec<u32>,
    pub admitted_same_stack_bytes: u64, // Authored same-stack demand in bytes (`0` when no `stack N bytes` clause was written).
    pub binding: PackagePolicyProviderBinding,
    pub compiler_intrinsic_execution: Option<PackageReviewCompilerIntrinsicExecution>,
    pub installation_reach: Option<PackageReviewSelectedInstallationReach>,
}

impl PackagePolicyProviderRow {
    field_projection_accessors!(borrowed requirement: &PackageReviewNominalIdentity);
    pub fn method(&self) -> &str {
        &self.method
    }

    pub fn realization(&self) -> Option<&PackageReviewNominalIdentity> {
        self.realization.as_ref()
    }

    field_projection_accessors!(sliced requirement_lifetime_partition: &[u32]);
    field_projection_accessors!(copied admitted_same_stack_bytes: u64);

    field_projection_accessors!(borrowed binding: &PackagePolicyProviderBinding);

    pub(crate) fn compiler_intrinsic_execution(
        &self,
    ) -> Option<PackageReviewCompilerIntrinsicExecution> {
        self.compiler_intrinsic_execution
    }

    pub fn installation_reach(&self) -> Option<&PackageReviewSelectedInstallationReach> {
        self.installation_reach.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyProviderEvaluatedSyscall {
    pub target: String,
    pub producer: PackagePolicyEvaluatedBindingProducer,
}

impl PackagePolicyProviderEvaluatedSyscall {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackagePolicyProviderBinding {
    FunctionField {
        target: String,
        selector: Box<semantic_vocabulary::FunctionFieldBinding>,
        producer: PackagePolicyEvaluatedBindingProducer,
    },
    Import {
        target: String,
        locator: PackageReviewForeignLocator,
        producer: PackagePolicyEvaluatedBindingProducer,
    },
    Syscall {
        number: i64,
        evaluated: Option<PackagePolicyProviderEvaluatedSyscall>,
    },
    CompilerIntrinsic {
        machine: String,
    },
    VtableSlot {
        index: i64,
    },
    VtableField {
        table: String,
        field: String,
        table_declaration: PackageReviewNominalIdentity,
        storage: Result<provider_plans::TableFieldStorage, String>,
    },
    TableFunction {
        table: String,
        field: String,
        table_declaration: PackageReviewNominalIdentity,
        storage: Result<provider_plans::TableFieldStorage, String>,
    },
    CheckedAdapter {
        machine_identity: String,
        machine_package_identity: Option<PackageKeyIdentity>,
    },
}

// Shared complete service-schema validation for inert provider policy components.

impl PackagePolicyServiceMethod {
    pub(crate) fn validate_service_structure(
        &self,
        schema: &PackageReviewNominalIdentity,
        target: TargetProfile,
    ) -> Result<(), &'static str> {
        self.validate_signature()?;
        self.validate_authority()?;
        owned_pair(&self.requirement_owner, &self.requirement)?;
        if self.name.is_empty()
            || self.parameter_count != self.parameter_type_identities.len()
            || self.parameter_type_identities.iter().any(String::is_empty)
            || self.has_result != self.result_type_identity.is_some()
            || self
                .result_type_identity
                .as_ref()
                .is_some_and(String::is_empty)
            || (!self.has_result
                && (!self.result_claims.is_empty() || !self.normal_result_memberships.is_empty()))
            || self
                .normal_result_memberships
                .iter()
                .any(|row| !row.has_exact_identity())
            || self
                .normal_result_memberships
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.entry_claims.iter().any(|claim| {
                claim.parameter_index >= self.parameter_count
                    || !claim.carrier_identity.is_valid()
                    || claim.domain.is_empty()
            })
        {
            return Err("provider method has inconsistent identity, signature, or claims");
        }
        for (position, calling) in self.calling.iter().enumerate() {
            if self.calling[..position]
                .iter()
                .any(|previous| previous.abi == calling.abi)
            {
                return Err("provider method repeats an exact declaration ABI application");
            }
            calling.validate_for_service(self)?;
            if calling.target.profile.backend_profile() != target
                || calling.boundary_trait != *schema
                || calling.requirement != self.requirement
                || calling.requirement_trait != self.requirement_owner
                || calling.semantic_parameters.len() != self.parameter_count
            {
                return Err(
                    "provider method calling policy is detached from its schema or requirement",
                );
            }
        }
        for premise in &self.termination_premises {
            if premise.profile.is_empty()
                || matches!(premise.subject, service_surface::ServiceProgressSubject::Parameter(ordinal) if ordinal >= self.parameter_count)
                || premise
                    .establishment_routes
                    .iter()
                    .any(|route| self.identities.text(route.requirement_identity).is_empty())
            {
                return Err("provider progress premise has an invalid subject or route");
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_service_methods(
    methods: &[PackagePolicyServiceMethod],
    schema: &PackageReviewNominalIdentity,
    target: TargetProfile,
) -> Result<(), &'static str> {
    schema.validate_resolved()?;
    if methods.is_empty() {
        return Err("service schema has no declared methods");
    }
    for (index, method) in methods.iter().enumerate() {
        method.validate_service_structure(schema, target)?;
        if methods[..index]
            .iter()
            .any(|prior| prior.requirement == method.requirement)
        {
            return Err("service schema repeats an exact requirement");
        }
    }
    Ok(())
}

// Target-independent source signature of a selected service application.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePolicyServiceSignature {
    pub schema_arguments: Vec<PackageReviewTypeIdentity>,
    pub schema_lifetime_parameter_count: u32,
    pub schema_lifetime_arguments: Vec<u32>,
    pub application_lifetime_parameter_count: u32,
    pub requirement_arguments: Vec<PackageReviewTypeIdentity>,
    pub requirement_lifetime_arguments: Vec<u32>,
    pub requirement_lifetime_parameter_count: u32,
    pub static_parameters: Vec<PackagePolicyTypeParameter>,
    pub parameters: Vec<PackageReviewSignatureParameter>,
    pub result: Option<PackageReviewTypeIdentity>,
}

impl PackagePolicyServiceSignature {
    field_projection_accessors!(sliced static_parameters: &[PackagePolicyTypeParameter]);
    field_projection_accessors!(sliced parameters: &[PackageReviewSignatureParameter]);
    pub fn result(&self) -> Option<&PackageReviewTypeIdentity> {
        self.result.as_ref()
    }
}

impl self::PackagePolicyServiceMethod {
    /// External completion retains its source return type without a calling result.
    pub fn calling_result_type(&self) -> Option<&PackageReviewTypeIdentity> {
        match self.completion {
            semantic_vocabulary::BoundaryCompletion::NormalReturn => self.signature.result(),
            semantic_vocabulary::BoundaryCompletion::External(_) => None,
        }
    }

    pub(super) fn validate_signature(&self) -> Result<(), &'static str> {
        let signature = &self.signature;
        let application_scope = signature
            .schema_lifetime_parameter_count
            .checked_add(signature.application_lifetime_parameter_count)
            .ok_or("provider signature lifetime telescope overflows")?;
        let lifetime_count = application_scope
            .checked_add(signature.requirement_lifetime_parameter_count)
            .ok_or("provider signature lifetime telescope overflows")?;
        if signature.parameters.len() != self.parameter_count
            || signature.result.is_some() != self.has_result
            || signature.schema_lifetime_arguments.len()
                != signature.schema_lifetime_parameter_count as usize
            || signature
                .schema_lifetime_arguments
                .iter()
                .any(|ordinal| *ordinal >= application_scope)
            || signature
                .requirement_lifetime_arguments
                .iter()
                .any(|ordinal| *ordinal >= application_scope)
            || signature.parameters.iter().any(|parameter| {
                parameter.name.is_empty() || parameter.type_identity.canonical.is_empty()
            })
            || signature
                .result
                .as_ref()
                .is_some_and(|result| result.canonical.is_empty())
        {
            return Err(
                "provider typed service signature has inconsistent parameters or lifetimes",
            );
        }
        let scope = Scope {
            parameters: signature.parameters.len(),
            result: signature.result.is_some(),
            ..scope(&signature.static_parameters, lifetime_count as usize)
        };
        parameters(&scope, 0)?;
        for calling in &self.calling {
            if signature.schema_arguments != calling.boundary_arguments
                || signature.schema_lifetime_parameter_count
                    != calling.boundary_lifetime_parameter_count
                || signature.schema_lifetime_arguments != calling.boundary_lifetime_arguments
                || signature.application_lifetime_parameter_count
                    != calling.application_lifetime_parameter_count
                || signature.requirement_arguments != calling.requirement_arguments
                || signature.requirement_lifetime_arguments
                    != calling.requirement_lifetime_arguments
                || signature.requirement_lifetime_parameter_count
                    != calling.requirement_lifetime_parameter_count
                || signature.static_parameters != calling.static_parameters
                || self.calling_result_type() != calling.semantic_result.as_ref()
                || signature.parameters.len() != calling.semantic_parameters.len()
                || signature
                    .parameters
                    .iter()
                    .zip(&calling.semantic_parameters)
                    .any(|(source, physical)| {
                        source.name != physical.name
                            || source.type_identity != physical.value_type
                            || source.is_const != physical.is_const
                            || source.is_mutable != physical.is_mutable
                    })
            {
                return Err(
                    "provider service signature is detached from its complete calling application",
                );
            }
        }
        Ok(())
    }
}
