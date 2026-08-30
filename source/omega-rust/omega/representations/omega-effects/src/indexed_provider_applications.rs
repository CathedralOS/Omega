//! Final-composition closure for indexed provider-requirement applications.
//!
//! An indexed requirement is one selected provider slot even when an artifact
//! demands several concrete applications. `ResidentContentTransfer<P, T>` is
//! the motivating package-qualified two-argument schema: generic artifacts may
//! retain artifact-local symbolic `P`/`T` applications, final composition
//! substitutes reachable arguments, and the one selected provider must carry
//! an attached coverage row for the resulting concrete set.
//!
//! This module is deliberately non-authorizing. It retains structural demand
//! and provider-asserted coverage only. It does not prove provider admission,
//! bind an issuance occurrence, establish resident custody, or authorize an
//! installation or transfer.

use std::collections::{BTreeMap, BTreeSet};

use crate::SelectedProviderPlanFacts;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedProviderRequirementSchema {
    trait_name: String,
    trait_package_identity: Option<psi_core::PackageKeyIdentity>,
    application_arity: usize,
}

impl IndexedProviderRequirementSchema {
    pub fn new(
        trait_name: impl Into<String>,
        trait_package_identity: Option<psi_core::PackageKeyIdentity>,
        application_arity: usize,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        let trait_name = trait_name.into();
        if trait_name.is_empty() {
            return Err(error("indexed provider schema has an empty trait identity"));
        }
        if application_arity == 0 {
            return Err(error(
                "indexed provider schema must declare at least one application argument",
            ));
        }
        Ok(Self {
            trait_name,
            trait_package_identity,
            application_arity,
        })
    }

    pub fn resident_content_transfer(trait_package_identity: psi_core::PackageKeyIdentity) -> Self {
        Self {
            trait_name: "ResidentContentTransfer".to_owned(),
            trait_package_identity: Some(trait_package_identity),
            application_arity: 2,
        }
    }

    pub fn trait_name(&self) -> &str {
        &self.trait_name
    }

    pub const fn trait_package_identity(&self) -> Option<psi_core::PackageKeyIdentity> {
        self.trait_package_identity
    }

    pub const fn application_arity(&self) -> usize {
        self.application_arity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedProviderApplicationArtifactIdentity([u8; 32]);

impl IndexedProviderApplicationArtifactIdentity {
    pub fn from_digest(digest: [u8; 32]) -> Option<Self> {
        (digest != [0; 32]).then_some(Self(digest))
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.0
    }
}

/// One artifact-local indexed-application binder. The artifact identity keeps
/// equal binder ordinals in separately compiled artifacts distinct.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedProviderApplicationParameter {
    artifact_identity: IndexedProviderApplicationArtifactIdentity,
    binder_ordinal: u32,
}

impl IndexedProviderApplicationParameter {
    pub const fn new(
        artifact_identity: IndexedProviderApplicationArtifactIdentity,
        binder_ordinal: u32,
    ) -> Self {
        Self {
            artifact_identity,
            binder_ordinal,
        }
    }

    pub const fn artifact_identity(&self) -> &IndexedProviderApplicationArtifactIdentity {
        &self.artifact_identity
    }

    pub const fn binder_ordinal(&self) -> u32 {
        self.binder_ordinal
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedProviderConcreteArgument(String);

impl IndexedProviderConcreteArgument {
    pub fn new(
        normalized_identity: impl Into<String>,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        let normalized_identity = normalized_identity.into();
        if normalized_identity.is_empty() {
            return Err(error(
                "indexed provider application has an empty concrete argument identity",
            ));
        }
        Ok(Self(normalized_identity))
    }

    pub fn normalized_identity(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IndexedProviderApplicationArgument {
    Concrete(IndexedProviderConcreteArgument),
    Parameter(IndexedProviderApplicationParameter),
}

impl IndexedProviderApplicationArgument {
    pub fn concrete(
        normalized_identity: impl Into<String>,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        Ok(Self::Concrete(IndexedProviderConcreteArgument::new(
            normalized_identity,
        )?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IndexedProviderApplicationDemand {
    schema: IndexedProviderRequirementSchema,
    arguments: Vec<IndexedProviderApplicationArgument>,
}

impl IndexedProviderApplicationDemand {
    pub fn new(
        schema: IndexedProviderRequirementSchema,
        arguments: Vec<IndexedProviderApplicationArgument>,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        validate_arity(&schema, arguments.len(), "application demand")?;
        Ok(Self { schema, arguments })
    }

    pub const fn schema(&self) -> &IndexedProviderRequirementSchema {
        &self.schema
    }

    pub fn arguments(&self) -> &[IndexedProviderApplicationArgument] {
        &self.arguments
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedProviderApplicationSubstitution {
    parameter: IndexedProviderApplicationParameter,
    concrete_argument: IndexedProviderConcreteArgument,
}

impl IndexedProviderApplicationSubstitution {
    pub fn new(
        parameter: IndexedProviderApplicationParameter,
        concrete_argument_identity: impl Into<String>,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        Ok(Self {
            parameter,
            concrete_argument: IndexedProviderConcreteArgument::new(concrete_argument_identity)?,
        })
    }

    pub const fn parameter(&self) -> &IndexedProviderApplicationParameter {
        &self.parameter
    }

    pub const fn concrete_argument(&self) -> &IndexedProviderConcreteArgument {
        &self.concrete_argument
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConcreteIndexedProviderApplication {
    schema: IndexedProviderRequirementSchema,
    arguments: Vec<IndexedProviderConcreteArgument>,
    report_fingerprint: u64,
}

impl ConcreteIndexedProviderApplication {
    pub fn new(
        schema: IndexedProviderRequirementSchema,
        arguments: Vec<IndexedProviderConcreteArgument>,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        validate_arity(&schema, arguments.len(), "concrete application")?;
        let report_fingerprint = application_report_fingerprint(&schema, &arguments);
        Ok(Self {
            schema,
            arguments,
            report_fingerprint,
        })
    }

    pub const fn schema(&self) -> &IndexedProviderRequirementSchema {
        &self.schema
    }

    pub fn arguments(&self) -> &[IndexedProviderConcreteArgument] {
        &self.arguments
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum IndexedApplicationCoverageKind {
    Generic,
    Exact(Vec<ConcreteIndexedProviderApplication>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderAssertedIndexedApplicationCoverage {
    provider_plan_report_identity: u64,
    schema: IndexedProviderRequirementSchema,
    kind: IndexedApplicationCoverageKind,
    report_fingerprint: u64,
}

impl ProviderAssertedIndexedApplicationCoverage {
    pub fn generic(
        provider_plan_report_identity: u64,
        schema: IndexedProviderRequirementSchema,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        validate_provider_plan_report_identity(provider_plan_report_identity)?;
        let kind = IndexedApplicationCoverageKind::Generic;
        let report_fingerprint =
            coverage_report_fingerprint(provider_plan_report_identity, &schema, &kind);
        Ok(Self {
            provider_plan_report_identity,
            schema,
            kind,
            report_fingerprint,
        })
    }

    pub fn exact_family(
        provider_plan_report_identity: u64,
        schema: IndexedProviderRequirementSchema,
        mut applications: Vec<ConcreteIndexedProviderApplication>,
    ) -> Result<Self, IndexedProviderApplicationClosureError> {
        validate_provider_plan_report_identity(provider_plan_report_identity)?;
        if applications.is_empty() {
            return Err(error(
                "exact indexed provider coverage family must not be empty",
            ));
        }
        if applications
            .iter()
            .any(|application| application.schema != schema)
        {
            return Err(error(
                "exact indexed provider coverage contains an application for another schema",
            ));
        }
        applications.sort();
        if applications.windows(2).any(|rows| rows[0] == rows[1]) {
            return Err(error(
                "exact indexed provider coverage contains a duplicate application",
            ));
        }
        let kind = IndexedApplicationCoverageKind::Exact(applications);
        let report_fingerprint =
            coverage_report_fingerprint(provider_plan_report_identity, &schema, &kind);
        Ok(Self {
            provider_plan_report_identity,
            schema,
            kind,
            report_fingerprint,
        })
    }

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub const fn schema(&self) -> &IndexedProviderRequirementSchema {
        &self.schema
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }

    pub const fn covers_generically(&self) -> bool {
        matches!(self.kind, IndexedApplicationCoverageKind::Generic)
    }

    pub fn exact_applications(&self) -> Option<&[ConcreteIndexedProviderApplication]> {
        match &self.kind {
            IndexedApplicationCoverageKind::Generic => None,
            IndexedApplicationCoverageKind::Exact(applications) => Some(applications),
        }
    }

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.provider_plan_report_identity.to_le_bytes());
        append_schema_bytes(&mut bytes, &self.schema);
        match &self.kind {
            IndexedApplicationCoverageKind::Generic => bytes.push(1),
            IndexedApplicationCoverageKind::Exact(applications) => {
                bytes.push(2);
                bytes.extend_from_slice(&(applications.len() as u64).to_le_bytes());
                for application in applications {
                    append_schema_bytes(&mut bytes, &application.schema);
                    bytes.extend_from_slice(&(application.arguments.len() as u64).to_le_bytes());
                    for argument in &application.arguments {
                        append_text_bytes(&mut bytes, argument.normalized_identity());
                    }
                }
            }
        }
        bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedIndexedProviderApplicationSet {
    selected_provider_closure_report_identity: u64,
    provider_plan_report_identity: u64,
    schema: IndexedProviderRequirementSchema,
    applications: Vec<ConcreteIndexedProviderApplication>,
    coverage: ProviderAssertedIndexedApplicationCoverage,
    report_fingerprint: u64,
}

impl ClosedIndexedProviderApplicationSet {
    pub const fn selected_provider_closure_report_identity(&self) -> u64 {
        self.selected_provider_closure_report_identity
    }

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub const fn schema(&self) -> &IndexedProviderRequirementSchema {
        &self.schema
    }

    pub fn applications(&self) -> &[ConcreteIndexedProviderApplication] {
        &self.applications
    }

    pub const fn coverage_report_fingerprint(&self) -> u64 {
        self.coverage.report_fingerprint()
    }

    pub const fn coverage(&self) -> &ProviderAssertedIndexedApplicationCoverage {
        &self.coverage
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedProviderApplicationClosureError(String);

impl IndexedProviderApplicationClosureError {
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IndexedProviderApplicationClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for IndexedProviderApplicationClosureError {}

/// Substitute and close every demanded application against one exact selected
/// provider plan and its attached provider-asserted coverage row. Coverage is
/// selected-closure input: callers cannot supply a companion assertion at the
/// closing boundary.
///
/// Success is structural evidence only. In particular, the returned value is
/// not an admission receipt and cannot bind provider issuance or resident
/// custody.
pub fn close_indexed_provider_applications(
    selected: &SelectedProviderPlanFacts,
    schema: &IndexedProviderRequirementSchema,
    demands: Vec<IndexedProviderApplicationDemand>,
    substitutions: Vec<IndexedProviderApplicationSubstitution>,
) -> Result<ClosedIndexedProviderApplicationSet, IndexedProviderApplicationClosureError> {
    if demands.is_empty() {
        return Err(error(
            "indexed provider application closure requires at least one demand",
        ));
    }
    let matching_plans = selected
        .plans()
        .iter()
        .filter(|plan| {
            plan.schema.trait_name == schema.trait_name
                && plan.schema.trait_package_identity == schema.trait_package_identity
        })
        .collect::<Vec<_>>();
    let [plan] = matching_plans.as_slice() else {
        return Err(error(match matching_plans.len() {
            0 => "indexed provider schema has no exact selected provider plan".to_owned(),
            count => format!(
                "indexed provider schema matches {count} selected provider plans; exactly one is required"
            ),
        }));
    };
    let plan_identity = plan.report_fingerprint();
    let matching_coverage = selected
        .indexed_provider_application_coverage()
        .iter()
        .filter(|coverage| {
            coverage.provider_plan_report_identity == plan_identity && coverage.schema == *schema
        })
        .collect::<Vec<_>>();
    let [coverage] = matching_coverage.as_slice() else {
        return Err(error(match matching_coverage.len() {
            0 => "indexed provider schema has no exact selected coverage row".to_owned(),
            count => format!(
                "indexed provider schema has {count} selected coverage rows; exactly one is required"
            ),
        }));
    };

    let mut substitution_map = BTreeMap::new();
    for substitution in substitutions {
        if substitution_map
            .insert(substitution.parameter, substitution.concrete_argument)
            .is_some()
        {
            return Err(error(
                "indexed provider application parameter is substituted more than once",
            ));
        }
    }

    let mut concrete = BTreeSet::new();
    let mut used_parameters = BTreeSet::new();
    for demand in demands {
        if demand.schema != *schema {
            return Err(error(
                "indexed provider application demand names another schema",
            ));
        }
        validate_arity(schema, demand.arguments.len(), "application demand")?;
        let arguments = demand
            .arguments
            .into_iter()
            .map(|argument| match argument {
                IndexedProviderApplicationArgument::Concrete(identity) => Ok(identity),
                IndexedProviderApplicationArgument::Parameter(parameter) => {
                    let concrete = substitution_map.get(&parameter).cloned().ok_or_else(|| {
                        error(format!(
                            "indexed provider application parameter in artifact {:?} at binder ordinal {} remains unresolved",
                            parameter.artifact_identity().digest(),
                            parameter.binder_ordinal()
                        ))
                    })?;
                    used_parameters.insert(parameter);
                    Ok(concrete)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        concrete.insert(ConcreteIndexedProviderApplication::new(
            schema.clone(),
            arguments,
        )?);
    }
    if used_parameters.len() != substitution_map.len() {
        return Err(error(
            "indexed provider application closure contains an unused substitution",
        ));
    }
    let applications = concrete.into_iter().collect::<Vec<_>>();

    if let IndexedApplicationCoverageKind::Exact(supported) = &coverage.kind {
        let supported = supported.iter().collect::<BTreeSet<_>>();
        if let Some(missing) = applications
            .iter()
            .find(|application| !supported.contains(application))
        {
            return Err(error(format!(
                "selected provider does not cover demanded indexed application {:#018x}",
                missing.report_fingerprint()
            )));
        }
    }

    let selected_provider_closure_report_identity = selected.report_fingerprint();
    let report_fingerprint = closed_set_report_fingerprint(
        selected_provider_closure_report_identity,
        plan_identity,
        schema,
        &applications,
        coverage.report_fingerprint,
    );
    Ok(ClosedIndexedProviderApplicationSet {
        selected_provider_closure_report_identity,
        provider_plan_report_identity: plan_identity,
        schema: schema.clone(),
        applications,
        coverage: (*coverage).clone(),
        report_fingerprint,
    })
}

fn validate_arity(
    schema: &IndexedProviderRequirementSchema,
    actual: usize,
    subject: &str,
) -> Result<(), IndexedProviderApplicationClosureError> {
    if actual != schema.application_arity {
        return Err(error(format!(
            "indexed provider {subject} has {actual} argument(s), expected {}",
            schema.application_arity
        )));
    }
    Ok(())
}

fn validate_provider_plan_report_identity(
    provider_plan_report_identity: u64,
) -> Result<(), IndexedProviderApplicationClosureError> {
    if provider_plan_report_identity == 0 {
        return Err(error(
            "indexed provider coverage has the reserved zero provider-plan identity",
        ));
    }
    Ok(())
}

fn error(message: impl Into<String>) -> IndexedProviderApplicationClosureError {
    IndexedProviderApplicationClosureError(message.into())
}

fn application_report_fingerprint(
    schema: &IndexedProviderRequirementSchema,
    arguments: &[IndexedProviderConcreteArgument],
) -> u64 {
    let mut hash = Fingerprint::new(b"omega.indexed-provider-application.v1");
    hash.schema(schema);
    hash.usize(arguments.len());
    for argument in arguments {
        hash.text(argument.normalized_identity());
    }
    hash.finish_nonzero()
}

fn append_schema_bytes(bytes: &mut Vec<u8>, schema: &IndexedProviderRequirementSchema) {
    append_text_bytes(bytes, schema.trait_name());
    match schema.trait_package_identity() {
        Some(identity) => {
            bytes.push(1);
            bytes.extend_from_slice(&identity.digest());
        }
        None => bytes.push(0),
    }
    bytes.extend_from_slice(&(schema.application_arity() as u64).to_le_bytes());
}

fn append_text_bytes(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn coverage_report_fingerprint(
    provider_plan_report_identity: u64,
    schema: &IndexedProviderRequirementSchema,
    kind: &IndexedApplicationCoverageKind,
) -> u64 {
    let mut hash = Fingerprint::new(b"omega.indexed-provider-coverage.v1");
    hash.u64(provider_plan_report_identity);
    hash.schema(schema);
    match kind {
        IndexedApplicationCoverageKind::Generic => hash.byte(1),
        IndexedApplicationCoverageKind::Exact(applications) => {
            hash.byte(2);
            hash.usize(applications.len());
            for application in applications {
                hash.u64(application.report_fingerprint);
            }
        }
    }
    hash.finish_nonzero()
}

fn closed_set_report_fingerprint(
    selected_provider_closure_report_identity: u64,
    provider_plan_report_identity: u64,
    schema: &IndexedProviderRequirementSchema,
    applications: &[ConcreteIndexedProviderApplication],
    coverage_report_fingerprint: u64,
) -> u64 {
    let mut hash = Fingerprint::new(b"omega.closed-indexed-provider-applications.v1");
    hash.u64(selected_provider_closure_report_identity);
    hash.u64(provider_plan_report_identity);
    hash.schema(schema);
    hash.usize(applications.len());
    for application in applications {
        hash.u64(application.report_fingerprint);
    }
    hash.u64(coverage_report_fingerprint);
    hash.finish_nonzero()
}

struct Fingerprint(u64);

impl Fingerprint {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new(domain: &[u8]) -> Self {
        let mut fingerprint = Self(Self::OFFSET);
        fingerprint.bytes(domain);
        fingerprint
    }

    fn schema(&mut self, schema: &IndexedProviderRequirementSchema) {
        self.text(&schema.trait_name);
        match schema.trait_package_identity {
            Some(identity) => {
                self.byte(1);
                self.bytes(&identity.digest());
            }
            None => self.byte(0),
        }
        self.usize(schema.application_arity);
    }

    fn text(&mut self, value: &str) {
        self.usize(value.len());
        self.bytes(value.as_bytes());
    }

    fn usize(&mut self, value: usize) {
        self.u64(value as u64);
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.byte(*byte);
        }
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn finish_nonzero(self) -> u64 {
        if self.0 == 0 { 1 } else { self.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };

    fn package(byte: u8) -> psi_core::PackageKeyIdentity {
        psi_core::PackageKeyIdentity::from_digest([byte; 32]).expect("nonzero package identity")
    }

    fn schema() -> IndexedProviderRequirementSchema {
        IndexedProviderRequirementSchema::resident_content_transfer(package(0x41))
    }

    fn selected_plan_in_package(
        name: &str,
        package_identity: psi_core::PackageKeyIdentity,
    ) -> (ProviderPlan, SelectedProviderPlanFacts) {
        let plan = ProviderPlan {
            name: name.to_owned(),
            provider_type: format!("{name}Provider"),
            provider_type_package_identity: Some(package_identity),
            target: "linux_x86_64".to_owned(),
            schema: ServiceSchema {
                trait_name: "ResidentContentTransfer".to_owned(),
                trait_package_identity: Some(package_identity),
                methods: vec![ServiceMethod {
                    name: "transfer".to_owned(),
                    requirement_owner: "ResidentContentTransfer".to_owned(),
                    requirement_owner_package_identity: Some(package_identity),
                    requirement_identity: "ResidentContentTransfer::transfer".to_owned(),
                    parameter_count: 2,
                    parameter_type_identities: vec!["P".to_owned(), "T".to_owned()],
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["ResidentContentTransfer".to_owned()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_report_fingerprint: None,
                    calling_plan_commitment: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "transfer".to_owned(),
                requirement_identity: "ResidentContentTransfer::transfer".to_owned(),
                binding: ProviderBinding::CheckedAdapter {
                    machine_identity: format!("{name}Provider::transfer"),
                    machine_package_identity: Some(package_identity),
                },
            }],
            origin_package_identity: Some(package_identity),
            origin_package: "test".to_owned(),
        };
        let selected = SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()])
            .expect("one fully covering indexed provider slot");
        (plan, selected)
    }

    fn selected_plan(name: &str) -> (ProviderPlan, SelectedProviderPlanFacts) {
        selected_plan_in_package(name, package(0x41))
    }

    fn concrete(value: &str) -> IndexedProviderApplicationArgument {
        IndexedProviderApplicationArgument::concrete(value).expect("concrete identity")
    }

    fn concrete_identity(value: &str) -> IndexedProviderConcreteArgument {
        IndexedProviderConcreteArgument::new(value).expect("concrete identity")
    }

    fn artifact(byte: u8) -> IndexedProviderApplicationArtifactIdentity {
        IndexedProviderApplicationArtifactIdentity::from_digest([byte; 32])
            .expect("nonzero artifact identity")
    }

    fn parameter(artifact_byte: u8, binder_ordinal: u32) -> IndexedProviderApplicationParameter {
        IndexedProviderApplicationParameter::new(artifact(artifact_byte), binder_ordinal)
    }

    fn demand(
        first: IndexedProviderApplicationArgument,
        second: IndexedProviderApplicationArgument,
    ) -> IndexedProviderApplicationDemand {
        IndexedProviderApplicationDemand::new(schema(), vec![first, second]).expect("demand")
    }

    fn application(first: &str, second: &str) -> ConcreteIndexedProviderApplication {
        ConcreteIndexedProviderApplication::new(
            schema(),
            vec![concrete_identity(first), concrete_identity(second)],
        )
        .expect("concrete application")
    }

    fn with_generic_coverage(
        selected: SelectedProviderPlanFacts,
        plan: &ProviderPlan,
    ) -> SelectedProviderPlanFacts {
        selected
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::generic(
                    plan.report_fingerprint(),
                    schema(),
                )
                .expect("generic coverage"),
            ])
            .expect("coverage attaches to exact selected slot")
    }

    #[test]
    fn attached_generic_coverage_closes_canonically_without_minting_slots() {
        let (plan, selected) = selected_plan("ResidentTransfer");
        let selected = with_generic_coverage(selected, &plan);
        let p = parameter(0x51, 0);
        let t = parameter(0x51, 1);
        let first_demands = vec![
            demand(concrete("StablePlan"), concrete("Header")),
            demand(
                IndexedProviderApplicationArgument::Parameter(p.clone()),
                IndexedProviderApplicationArgument::Parameter(t.clone()),
            ),
        ];
        let mut second_demands = first_demands.clone();
        second_demands.reverse();
        let first_substitutions = vec![
            IndexedProviderApplicationSubstitution::new(p.clone(), "StablePlan").unwrap(),
            IndexedProviderApplicationSubstitution::new(t.clone(), "Packet").unwrap(),
        ];
        let mut second_substitutions = first_substitutions.clone();
        second_substitutions.reverse();
        let close = |demands, substitutions| {
            close_indexed_provider_applications(&selected, &schema(), demands, substitutions)
                .unwrap()
        };
        let first = close(first_demands, first_substitutions);
        let second = close(second_demands, second_substitutions);
        assert_eq!(first.report_fingerprint(), second.report_fingerprint());
        assert_eq!(first.applications().len(), 2);
        assert!(first.coverage().covers_generically());
        assert_eq!(first.coverage().schema(), &schema());
        assert_eq!(
            first.provider_plan_report_identity(),
            plan.report_fingerprint()
        );
        assert_eq!(selected.plans().len(), 1, "applications do not mint slots");
    }

    #[test]
    fn missing_selected_coverage_rejects() {
        let (_, selected) = selected_plan("ResidentTransfer");
        let error = close_indexed_provider_applications(
            &selected,
            &schema(),
            vec![demand(concrete("StablePlan"), concrete("Packet"))],
            Vec::new(),
        )
        .expect_err("coverage must already belong to selected facts");
        assert!(error.message().contains("no exact selected coverage row"));
    }

    #[test]
    fn artifact_identity_disambiguates_equal_binder_ordinals() {
        let (plan, selected) = selected_plan("ResidentTransfer");
        let selected = with_generic_coverage(selected, &plan);
        let first = parameter(0x61, 0);
        let second = parameter(0x62, 0);
        let closed = close_indexed_provider_applications(
            &selected,
            &schema(),
            vec![demand(
                IndexedProviderApplicationArgument::Parameter(first.clone()),
                IndexedProviderApplicationArgument::Parameter(second.clone()),
            )],
            vec![
                IndexedProviderApplicationSubstitution::new(first, "FirstArtifactPlan").unwrap(),
                IndexedProviderApplicationSubstitution::new(second, "SecondArtifactType").unwrap(),
            ],
        )
        .expect("same ordinal in distinct artifacts substitutes independently");
        assert_eq!(
            closed.applications()[0]
                .arguments()
                .iter()
                .map(IndexedProviderConcreteArgument::normalized_identity)
                .collect::<Vec<_>>(),
            vec!["FirstArtifactPlan", "SecondArtifactType"]
        );
    }

    #[test]
    fn attached_exact_coverage_is_retained_in_full_and_allows_a_superset() {
        let (plan, selected) = selected_plan("ResidentTransfer");
        let family = vec![
            application("StablePlan", "Packet"),
            application("StablePlan", "Header"),
        ];
        let selected = selected
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::exact_family(
                    plan.report_fingerprint(),
                    schema(),
                    family.clone(),
                )
                .unwrap(),
            ])
            .unwrap();
        let closed = close_indexed_provider_applications(
            &selected,
            &schema(),
            vec![demand(concrete("StablePlan"), concrete("Packet"))],
            Vec::new(),
        )
        .expect("exact family may exceed the demanded set");
        assert!(!closed.coverage().covers_generically());
        let retained = closed.coverage().exact_applications().unwrap();
        let mut canonical_family = family;
        canonical_family.sort();
        assert_eq!(retained, canonical_family);
        assert_eq!(
            closed.coverage().provider_plan_report_identity(),
            plan.report_fingerprint()
        );

        let missing = SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()])
            .unwrap()
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::exact_family(
                    plan.report_fingerprint(),
                    schema(),
                    vec![application("StablePlan", "Packet")],
                )
                .unwrap(),
            ])
            .unwrap();
        let error = close_indexed_provider_applications(
            &missing,
            &schema(),
            vec![demand(concrete("StablePlan"), concrete("Header"))],
            Vec::new(),
        )
        .expect_err("exact family is fail closed");
        assert!(error.message().contains("does not cover demanded"));
    }

    #[test]
    fn coverage_rows_are_canonical_and_change_selected_closure_identity() {
        let (plan, bare) = selected_plan("ResidentTransfer");
        let bare_identity = bare.report_fingerprint();
        let generic = with_generic_coverage(bare.clone(), &plan);
        let exact = bare
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::exact_family(
                    plan.report_fingerprint(),
                    schema(),
                    vec![application("StablePlan", "Packet")],
                )
                .unwrap(),
            ])
            .unwrap();
        assert_ne!(bare_identity, generic.report_fingerprint());
        assert_ne!(generic.report_fingerprint(), exact.report_fingerprint());
        assert_eq!(generic.indexed_provider_application_coverage().len(), 1);

        let duplicate_error = SelectedProviderPlanFacts::from_selected_plans(vec![plan.clone()])
            .unwrap()
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::generic(
                    plan.report_fingerprint(),
                    schema(),
                )
                .unwrap(),
                ProviderAssertedIndexedApplicationCoverage::exact_family(
                    plan.report_fingerprint(),
                    schema(),
                    vec![application("StablePlan", "Packet")],
                )
                .unwrap(),
            ])
            .expect_err("one row per selected indexed schema");
        assert!(duplicate_error.contains("more than one indexed-application coverage row"));
    }

    #[test]
    fn coverage_and_installation_reach_compose_in_either_order() {
        let (plan, selected) = selected_plan("ResidentTransfer");
        let coverage = || {
            vec![
                ProviderAssertedIndexedApplicationCoverage::generic(
                    plan.report_fingerprint(),
                    schema(),
                )
                .unwrap(),
            ]
        };
        let reach = || {
            vec![crate::InstallationReachResolution {
                requirement_identity: "ResidentContentTransfer::transfer".to_owned(),
                provider_plan_report_identity: plan.report_fingerprint(),
                upper_bound: vec!["MemoryTransfer".to_owned()],
                resolved_row: vec!["MemoryTransfer".to_owned()],
            }]
        };
        let coverage_then_reach = selected
            .clone()
            .with_indexed_provider_application_coverage(coverage())
            .unwrap()
            .with_installation_reach_resolutions(reach())
            .unwrap();
        let reach_then_coverage = selected
            .with_installation_reach_resolutions(reach())
            .unwrap()
            .with_indexed_provider_application_coverage(coverage())
            .unwrap();

        assert_eq!(
            coverage_then_reach.report_fingerprint(),
            reach_then_coverage.report_fingerprint()
        );
        assert_eq!(
            coverage_then_reach.indexed_provider_application_coverage(),
            reach_then_coverage.indexed_provider_application_coverage()
        );
        assert_eq!(
            coverage_then_reach.installation_reach_resolutions(),
            reach_then_coverage.installation_reach_resolutions()
        );
    }

    #[test]
    fn arity_companion_row_does_not_cover_the_demanded_schema() {
        let (plan, selected) = selected_plan("ResidentTransfer");
        let arity_three = IndexedProviderRequirementSchema::new(
            "ResidentContentTransfer",
            Some(package(0x41)),
            3,
        )
        .unwrap();
        let selected = selected
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::generic(
                    plan.report_fingerprint(),
                    arity_three,
                )
                .unwrap(),
            ])
            .expect("plan schema matches independently of application arity");
        let error = close_indexed_provider_applications(
            &selected,
            &schema(),
            vec![demand(concrete("StablePlan"), concrete("Packet"))],
            Vec::new(),
        )
        .expect_err("companion row with mutated arity is not exact coverage");
        assert!(error.message().contains("no exact selected coverage row"));
    }

    #[test]
    fn same_name_from_another_package_does_not_match() {
        let (first_plan, first_selected) =
            selected_plan_in_package("ResidentTransfer", package(0x41));
        let first_selected = with_generic_coverage(first_selected, &first_plan);
        let other_schema =
            IndexedProviderRequirementSchema::resident_content_transfer(package(0x42));
        let other_demand = IndexedProviderApplicationDemand::new(
            other_schema.clone(),
            vec![concrete("StablePlan"), concrete("Packet")],
        )
        .unwrap();
        let error = close_indexed_provider_applications(
            &first_selected,
            &other_schema,
            vec![other_demand],
            Vec::new(),
        )
        .expect_err("same spelling cannot cross package identity");
        assert!(error.message().contains("no exact selected provider plan"));

        let (other_plan, _) = selected_plan_in_package("ResidentTransfer", package(0x42));
        let attach_error = SelectedProviderPlanFacts::from_selected_plans(vec![first_plan])
            .unwrap()
            .with_indexed_provider_application_coverage(vec![
                ProviderAssertedIndexedApplicationCoverage::generic(
                    other_plan.report_fingerprint(),
                    other_schema,
                )
                .unwrap(),
            ])
            .expect_err("unselected package-qualified plan");
        assert!(attach_error.contains("unselected provider plan"));
    }

    #[test]
    fn unresolved_duplicate_and_unused_substitutions_reject() {
        let (plan, selected) = selected_plan("ResidentTransfer");
        let selected = with_generic_coverage(selected, &plan);
        let p = parameter(0x71, 0);
        let symbolic = demand(
            IndexedProviderApplicationArgument::Parameter(p.clone()),
            concrete("Packet"),
        );
        assert!(
            close_indexed_provider_applications(
                &selected,
                &schema(),
                vec![symbolic.clone()],
                Vec::new(),
            )
            .expect_err("unresolved")
            .message()
            .contains("remains unresolved")
        );
        assert!(
            close_indexed_provider_applications(
                &selected,
                &schema(),
                vec![symbolic],
                vec![
                    IndexedProviderApplicationSubstitution::new(p.clone(), "StablePlan").unwrap(),
                    IndexedProviderApplicationSubstitution::new(p, "OtherPlan").unwrap(),
                ],
            )
            .expect_err("duplicate")
            .message()
            .contains("substituted more than once")
        );
        assert!(
            close_indexed_provider_applications(
                &selected,
                &schema(),
                vec![demand(concrete("StablePlan"), concrete("Packet"))],
                vec![
                    IndexedProviderApplicationSubstitution::new(parameter(0x72, 0), "OtherPlan",)
                        .unwrap()
                ],
            )
            .expect_err("unused")
            .message()
            .contains("unused substitution")
        );
    }

    #[test]
    fn malformed_arity_and_duplicate_exact_coverage_reject_early() {
        assert!(
            IndexedProviderApplicationDemand::new(schema(), vec![concrete("StablePlan")])
                .expect_err("arity mismatch")
                .message()
                .contains("expected 2")
        );
        let (plan, _) = selected_plan("ResidentTransfer");
        let row = application("StablePlan", "Packet");
        assert!(
            ProviderAssertedIndexedApplicationCoverage::exact_family(
                plan.report_fingerprint(),
                schema(),
                vec![row.clone(), row],
            )
            .expect_err("duplicate exact application")
            .message()
            .contains("duplicate application")
        );
    }
}
