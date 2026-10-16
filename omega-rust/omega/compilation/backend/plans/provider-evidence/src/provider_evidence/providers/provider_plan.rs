//! PRV1 (design-ruled 2026-07-17): the typed **ProviderPlan** policy
//! carrier -- one value per (provider type, service schema, target), unifying
//! checked `satisfies` closures and irreducible external leaves with the
//! remaining built-in platform-lowering tables. CONSTRUCTION IS FREE: any code can build a plan; PRV2
//! validates coverage/signatures/identity, PRV3 admits semantic claims
//! through the chapter-10 grant/receipt carrier and selects by a
//! slot-owner capability. PRV4 retired authored `provides`/populate tables:
//! ordinary target packages now own checked adapters and `via Binding` leaves.
//! Trust classification is ADMISSION OUTPUT,
//! never author-selected plan data -- which is why no trust field exists
//! on these types.
//!
//! This file owns the provider plan, its rows and bindings.
//! `service_schemas.rs` carries service schemas, methods, progress
//! premises and claims, `evaluated_bindings.rs` evaluated binding usages,
//! receipts, foreign imports and syscalls, `digest_encoder.rs` the plan
//! digest encoder and `tests.rs` the plan tests.

mod digest_encoder;
pub(crate) use digest_encoder::DigestFeed;
mod evaluated_bindings;
mod identity_table;
mod service_schemas;

pub use evaluated_bindings::{
    EvaluatedBindingEvaluationDigest, EvaluatedBindingMaterializationDigest,
    EvaluatedBindingProducerClosureDigest, EvaluatedBindingReceipt, EvaluatedBindingUsage,
    EvaluatedForeignImport, EvaluatedForeignSyscall, evaluated_syscall_identity_digest,
};
pub use identity_table::{ProviderIdentityId, ProviderIdentityTable};
pub use service_schemas::BoundaryCallingPlanCommitment;
pub use service_schemas::{
    ProviderPlanDigest, ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod,
    ServiceProgressEstablishmentRoute, ServiceProgressEstablishmentRouteKind,
    ServiceProgressPremise, ServiceProgressSubject, ServiceReceiverApplication, ServiceResultClaim,
    ServiceSchema, ServiceSchemaDigest,
};

use crate::provider_evidence::providers::provider_plan::digest_encoder::ProviderPlanDigestEncoder;

/// How one method binds on one target. Instructions are checked `asm` bodies
/// whose catalog contracts contribute their obligations; they are deliberately
/// not a second, bodiless provider-binding mechanism.
///
/// The string-backed `via ForeignBinding::DllImport("library", "symbol")` bootstrap
/// has no case here. Raw foreign bytes are data, never Omega symbol names or
/// ambient lookup authority: an import binds through one evaluated
/// [`EvaluatedForeignImport`] whose target-normalized locator was produced by
/// checked Omega code. Versioned encodings that once carried the bootstrap
/// keep their tag unassigned and reject it on decode instead of
/// reinterpreting two authored strings as one physical locator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderBinding {
    /// One evaluated, target-validated physical foreign locator. Its atomic
    /// byte coordinates remain sealed together through selection and opaque
    /// executable accounting.
    Import { evaluated: EvaluatedForeignImport },
    /// Direct system call by number.
    Syscall { number: i64 },
    /// A compiler-known operation furnished by the selected target package.
    /// This is the exact normalized realization-machine overload identity;
    /// target catalog selection is derived separately from the checked
    /// requirement and selected target.
    CompilerIntrinsic { machine: String },
    /// COM/UEFI slot dispatch: callee address read from the receiver.
    VtableSlot { index: i64 },
    /// Field-model vtable dispatch: the fn-ptr field of a named table
    /// struct; the byte offset resolves from the layout plan downstream.
    VtableField { table: String, field: String },
    /// UEFI service-table function (the boot-services shape).
    TableFunction { table: String, field: String },
    /// An ORDINARY CHECKED MACHINE realizing the requirement (the ruling's
    /// composite form: lowering sequences and argument adaptation are
    /// checked Omega code with an explicit satisfies edge, never authored
    /// rows). Psi conformance checking enforces the adapter as a REFINEMENT:
    /// its transitive effects must fit inside the satisfied requirement's
    /// declared ceiling.
    CheckedAdapter {
        /// Canonical typed machine-overload identity, never a short name,
        /// interned into the owning [`ProviderPlan::identities`].
        machine_identity: ProviderIdentityId,
        /// Exact package owning the checked adapter. `None` is retained only
        /// for toolchain, standalone, and focused source-free programs.
        machine_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    },
}

/// One method's normalized provider binding. Composite argument adaptation is
/// checked Omega code, so plan rows carry only irreducible leaf mechanisms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanRow {
    pub method: String,
    /// Exact requirement overload supplied by this row, as an index into the
    /// owning plan's `identities` table. It is always present; a method
    /// spelling is display/debug data, not dispatch identity.
    pub requirement_identity: ProviderIdentityId,
    /// First-occurrence-normalized equality partition for the target trait's
    /// lifetime application on this exact realization edge. Empty for a
    /// lifetime-free trait, operator, or top-level requirement.
    pub requirement_lifetime_partition: Vec<u32>,
    pub binding: ProviderBinding,
}

/// The PRV1 carrier: one provider type's plan for one service schema on one
/// target. `origin_package_identity` is exact compiler-derived provenance
/// INPUT to admission (a package can never self-grant); the legacy readable
/// `origin_package` is display data only. The admission verdict itself lives
/// in the chapter-10 receipts, never here.
#[derive(Debug, Clone, Default)]
pub struct ProviderPlan {
    /// The plan's own name (`omega::host::standard::console`, the future
    /// slot-selection key).
    pub name: String,
    /// The nominal provider type whose explicit conformance closure produced
    /// this plan. Empty only for a free external leaf; checked adapters belong
    /// to nominal provider types. Slot overrides select this identity, never
    /// individual rows.
    pub provider_type: String,
    /// Exact package owning `provider_type`. Free external leaves and
    /// toolchain/standalone/source-free trees retain `None`; consumers must not
    /// infer this from `provider_type` or the realizing machine's package.
    pub provider_type_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// The target this plan serves (`windows_x86_64`; empty = every target).
    pub target: String,
    /// The schema served.
    pub schema: ServiceSchema,
    /// Interned canonical identities this plan's binding fields point into.
    /// Schema-owned identities intern into `schema.identities` instead.
    pub identities: ProviderIdentityTable,
    /// One row per bound method.
    pub rows: Vec<ProviderPlanRow>,
    /// Exact package that authored the realizing machine(s). `None` means the
    /// source was not package-owned (for example toolchain or standalone
    /// source); consumers must not infer ownership from any readable name.
    pub origin_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    /// Legacy readable origin label. This is diagnostic data, not admission
    /// provenance, and must never repair a missing exact package identity.
    pub origin_package: String,
    /// The plan's [`Self::identity_digest`], computed on first use and kept.
    /// Construct with `ProviderPlanDigestMemo::default()`; a plan's fields
    /// are construction data and are not mutated after its digest is read.
    pub digest_memo: ProviderPlanDigestMemo,
}

/// Memoized [`ProviderPlan::identity_digest`]. It is not part of the plan's
/// value: equality ignores it, debug output omits its state, and a clone
/// starts empty, so a clone whose public fields are changed before its first
/// digest cannot inherit the original's commitment.
#[derive(Default)]
pub struct ProviderPlanDigestMemo(std::sync::OnceLock<ProviderPlanDigest>);

impl Clone for ProviderPlanDigestMemo {
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl PartialEq for ProviderPlanDigestMemo {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for ProviderPlanDigestMemo {}

impl std::fmt::Debug for ProviderPlanDigestMemo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ProviderPlanDigestMemo")
    }
}

fn push_package_identity(
    rendered: &mut String,
    label: &str,
    identity: Option<semantic_vocabulary::PackageKeyIdentity>,
) {
    rendered.push('\n');
    rendered.push_str(label);
    rendered.push(':');
    match identity {
        Some(identity) => {
            for byte in identity.digest() {
                rendered.push_str(&format!("{byte:02x}"));
            }
        }
        None => rendered.push_str("<unbound>"),
    }
}

impl PartialEq for ProviderPlan {
    /// Semantic equality: field values plus the interned text each binding
    /// resolves, never the process-local mint order that produced the ids.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.provider_type == other.provider_type
            && self.provider_type_package_identity == other.provider_type_package_identity
            && self.target == other.target
            && self.schema == other.schema
            && self.rows.len() == other.rows.len()
            && self
                .rows
                .iter()
                .zip(&other.rows)
                .all(|(left, right)| left.eq_in(&self.identities, right, &other.identities))
            && self.origin_package_identity == other.origin_package_identity
            && self.origin_package == other.origin_package
    }
}

impl Eq for ProviderPlan {}

impl ProviderPlanRow {
    /// Semantic equality given the tables each row's binding identities were
    /// interned into; `CheckedAdapter` machine identities compare by
    /// resolved text so mint order never decides equality.
    fn eq_in(
        &self,
        left_table: &ProviderIdentityTable,
        other: &Self,
        right_table: &ProviderIdentityTable,
    ) -> bool {
        self.method == other.method
            && self.requirement_identity == other.requirement_identity
            && self.requirement_lifetime_partition == other.requirement_lifetime_partition
            && match (&self.binding, &other.binding) {
                (
                    ProviderBinding::CheckedAdapter {
                        machine_identity: left_identity,
                        machine_package_identity: left_package,
                    },
                    ProviderBinding::CheckedAdapter {
                        machine_identity: right_identity,
                        machine_package_identity: right_package,
                    },
                ) => {
                    left_table.text(*left_identity) == right_table.text(*right_identity)
                        && left_package == right_package
                }
                (left, right) => left == right,
            }
    }
}

impl ProviderPlan {
    /// The canonical identity text one of this plan's interned fields names.
    /// Projects the empty string for the zero id, matching the empty-field
    /// rendering the encoder wrote before interning.
    pub fn identity_text(&self, identity: ProviderIdentityId) -> &str {
        self.identities.text(identity)
    }

    /// Domain-separated SHA-256 commitment to the complete normalized plan
    /// structure. Unlike [`Self::report_fingerprint`], this includes exact
    /// normalized foreign-locator coordinates and is suitable for retained
    /// evidence identity. Computed once per plan value.
    pub fn identity_digest(&self) -> ProviderPlanDigest {
        *self
            .digest_memo
            .0
            .get_or_init(|| self.encode_identity_digest())
    }

    fn encode_identity_digest(&self) -> ProviderPlanDigest {
        let mut encoder = ProviderPlanDigestEncoder::for_provider_plan();
        encoder.string(&self.name);
        encoder.string(&self.provider_type);
        encoder.package_identity(self.provider_type_package_identity);
        encoder.string(&self.target);
        encoder.string(self.schema.identity_text(self.schema.trait_name));
        encoder.package_identity(self.schema.trait_package_identity);
        encoder.receiver_applications(&self.schema);

        let mut methods = self.schema.methods.iter().collect::<Vec<_>>();
        methods.sort_by(|left, right| {
            left.name.cmp(&right.name).then_with(|| {
                self.schema
                    .identity_text(left.requirement_identity)
                    .cmp(self.schema.identity_text(right.requirement_identity))
            })
        });
        encoder.len(methods.len());
        for method in methods {
            encoder.service_method(method, &self.schema.identities);
        }

        let mut rows = self.rows.iter().collect::<Vec<_>>();
        rows.sort_by_cached_key(|row| {
            (
                row.method.clone(),
                self.identities.text(row.requirement_identity).to_owned(),
                row.requirement_lifetime_partition.clone(),
            )
        });
        encoder.len(rows.len());
        for row in rows {
            encoder.string(&row.method);
            encoder.string(self.identities.text(row.requirement_identity));
            encoder.len(row.requirement_lifetime_partition.len());
            for ordinal in &row.requirement_lifetime_partition {
                encoder.u64(u64::from(*ordinal));
            }
            encoder.provider_binding(&row.binding, &self.identities);
        }
        encoder.package_identity(self.origin_package_identity);
        ProviderPlanDigest::from_digest(encoder.finish())
    }

    /// Historical FNV-1a report fingerprint over the canonical rendering
    /// (name, exact package provenance, target, schema surface, and rows in
    /// method order). Presentation order and whitespace are excluded. This
    /// compact value supports sorting, diagnostics, and compatibility lookup;
    /// admission and execution must retain the exact plan and its
    /// collision-resistant [`Self::identity_digest`].
    pub fn report_fingerprint(&self) -> u64 {
        let mut rendered = format!(
            "{}\n{}\n{}\n{}",
            self.name,
            self.provider_type,
            self.target,
            self.schema.identity_text(self.schema.trait_name)
        );
        push_package_identity(
            &mut rendered,
            "provider-type-package",
            self.provider_type_package_identity,
        );
        push_package_identity(
            &mut rendered,
            "schema-package",
            self.schema.trait_package_identity,
        );
        rendered.push_str("\npackage:");
        match self.origin_package_identity {
            Some(identity) => {
                for byte in identity.digest() {
                    rendered.push_str(&format!("{byte:02x}"));
                }
            }
            None => rendered.push_str("<unbound>"),
        }
        let mut methods: Vec<&ServiceMethod> = self.schema.methods.iter().collect();
        methods.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.requirement_identity.cmp(&right.requirement_identity))
        });
        for method in methods {
            push_package_identity(
                &mut rendered,
                "requirement-owner-package",
                method.requirement_owner_package_identity,
            );
            rendered.push_str(&format!("\ncompletion:{:?}", method.completion));
            rendered.push_str(&format!(
                "\nm:{}/{}/{}/{}/services:{}/invokes:{}/suspend:{}/block:{}/terminates:{}",
                method.name,
                self.schema.identity_text(method.requirement_identity),
                method.parameter_count,
                method.has_result,
                method.service_reach.join("+"),
                method.synchronous_invocations.join("+"),
                method.may_suspend,
                method.may_block,
                method.terminates_guarantee,
            ));
            let mut premises = method.termination_premises.iter().collect::<Vec<_>>();
            premises.sort_by(|left, right| {
                left.profile
                    .cmp(&right.profile)
                    .then_with(|| left.subject.cmp(&right.subject))
                    .then_with(|| left.subject_projections.cmp(&right.subject_projections))
            });
            for premise in premises {
                rendered.push_str(&format!(
                    "\nmt:{}/{}/{}",
                    premise.profile,
                    match premise.subject {
                        ServiceProgressSubject::ProviderReceiver => "self".to_owned(),
                        ServiceProgressSubject::Parameter(index) => format!("parameter:{index}"),
                    },
                    premise.subject_projections.join("::")
                ));
                let mut establishment_routes =
                    premise.establishment_routes.iter().collect::<Vec<_>>();
                establishment_routes.sort_by(|left, right| {
                    left.kind.cmp(&right.kind).then_with(|| {
                        self.schema
                            .identity_text(left.requirement_identity)
                            .cmp(self.schema.identity_text(right.requirement_identity))
                    })
                });
                for route in establishment_routes {
                    rendered.push_str(&format!(
                        "\nmtr:{}/{}",
                        route.kind.as_str(),
                        self.schema.identity_text(route.requirement_identity),
                    ));
                }
            }
            for parameter in &method.parameter_type_identities {
                rendered.push_str("\nmp:");
                rendered.push_str(self.schema.identities.text(*parameter));
            }
            let mut entry_claims = method.entry_claims.iter().collect::<Vec<_>>();
            entry_claims.sort_by(|left, right| {
                left.parameter_index
                    .cmp(&right.parameter_index)
                    .then_with(|| left.carrier_identity.cmp(&right.carrier_identity))
                    .then_with(|| left.domain.cmp(&right.domain))
            });
            for claim in entry_claims {
                rendered.push_str(&format!(
                    "\nmc:{}/{}/{}/{}/{}/{}",
                    claim.parameter_index,
                    self.schema.identities.text(claim.carrier_identity),
                    claim.domain,
                    claim.predicate_body.as_str(),
                    claim.authority_flow.as_str(),
                    claim.effective_carry,
                ));
            }
            if method.result_type_identity.is_valid() {
                rendered.push_str("\nmr:");
                rendered.push_str(self.schema.identities.text(method.result_type_identity));
            }
            let mut result_claims = method.result_claims.iter().collect::<Vec<_>>();
            result_claims.sort_by(|left, right| left.domain.cmp(&right.domain));
            for claim in result_claims {
                rendered.push_str(&format!("\nmrc:{}/{}", claim.domain, claim.effective_carry,));
            }
            if let Some(fingerprint) = method.calling_plan_report_fingerprint {
                rendered.push_str(&format!("/calling:{fingerprint:016x}"));
            }
        }
        let mut rows: Vec<&ProviderPlanRow> = self.rows.iter().collect();
        rows.sort_by_cached_key(|row| {
            (
                row.method.clone(),
                self.identities.text(row.requirement_identity).to_owned(),
            )
        });
        for row in rows {
            let binding_identity = match &row.binding {
                ProviderBinding::Import { evaluated } => {
                    let locator = evaluated.locator();
                    let receipt = evaluated
                        .receipt()
                        .identity_digest()
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect::<String>();
                    format!(
                        "EvaluatedImport:{:016x}/{receipt}",
                        locator.non_authoritative_compatibility_fingerprint(),
                    )
                }
                ProviderBinding::CompilerIntrinsic { machine, .. } => {
                    format!("CompilerIntrinsic {{ machine: {machine:?} }}")
                }
                ProviderBinding::CheckedAdapter {
                    machine_identity,
                    machine_package_identity,
                } => {
                    let mut identity =
                        format!("CheckedAdapter:{}", self.identity_text(*machine_identity));
                    push_package_identity(
                        &mut identity,
                        "checked-adapter-package",
                        *machine_package_identity,
                    );
                    identity
                }
                binding => format!("{binding:?}"),
            };
            rendered.push_str(&format!(
                "\nr:{}/{}/lifetimes:{:?}/{}",
                row.method,
                self.identities.text(row.requirement_identity),
                row.requirement_lifetime_partition,
                binding_identity
            ));
        }
        let mut hash = crate::fnv1a::Fnv1a::new();
        hash.bytes(rendered.as_bytes());
        // Report coordinates distinguish exact applied receiver rosters too.
        // This is still a compact coordinate, never an admission identity.
        hash.bytes(self.schema.identity_digest().as_bytes());
        hash.finish()
    }

    /// The deterministic order selection pipelines apply before replaying or
    /// reporting chosen plans: name, origin package, provider-type package,
    /// schema trait package, then the report fingerprint.
    pub fn selection_order(&self, other: &ProviderPlan) -> std::cmp::Ordering {
        self.name
            .cmp(&other.name)
            .then_with(|| {
                self.origin_package_identity
                    .cmp(&other.origin_package_identity)
            })
            .then_with(|| {
                self.provider_type_package_identity
                    .cmp(&other.provider_type_package_identity)
            })
            .then_with(|| {
                self.schema
                    .trait_package_identity
                    .cmp(&other.schema.trait_package_identity)
            })
            .then_with(|| self.report_fingerprint().cmp(&other.report_fingerprint()))
    }

    /// PRV2: full structural validation against the schema -- every method
    /// bound exactly once and no stray rows. Returns NAMED errors; empty =
    /// structurally valid.
    pub fn validate_against_schema(&self) -> Vec<String> {
        let mut errors = self.validate_candidate_against_schema();
        for method in &self.schema.methods {
            let count = self
                .rows
                .iter()
                .filter(|row| self.schema.row_binds_method(row, method, &self.identities))
                .count();
            if count == 0 {
                errors.push(format!(
                    "plan `{}` does not bind `{}::{}`",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name
                ));
            }
        }
        errors
    }

    /// Validate one candidate before coverage/selection. Partial candidates
    /// are legitimate, but a candidate cannot duplicate a requirement, name a
    /// callable row outside its schema. This is the additive-only conformance check;
    /// selection decides whether the surviving candidate covers the complete
    /// slot.
    pub fn validate_candidate_against_schema(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if let Err(reason) = self.schema.validate_receiver_applications() {
            errors.push(format!(
                "plan `{}` has an invalid applied receiver roster: {reason}",
                self.name
            ));
        }
        if self.name.is_empty() {
            errors.push("provider plan has no exact selection name".to_owned());
        }
        if self.schema.identity_text(self.schema.trait_name).is_empty() {
            errors.push(format!(
                "plan `{}` schema has no exact boundary-slot identity",
                self.name
            ));
        }
        for (method_index, method) in self.schema.methods.iter().enumerate() {
            if method.calling_plan_report_fingerprint.is_some()
                != method.calling_plan_commitment.is_some()
            {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` must retain its calling-plan report coordinate and strong commitment together",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                ));
            }
            if method
                .calling_plan_commitment
                .is_some_and(BoundaryCallingPlanCommitment::is_zero)
            {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` retains an empty calling-plan commitment",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                ));
            }
            if method.name.is_empty() {
                errors.push(format!(
                    "plan `{}` schema `{}` has a method with no readable drift name",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                ));
            }
            if method.requirement_owner.is_empty() {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` has no exact requirement owner",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name
                ));
            }
            if !method.requirement_identity.is_valid() {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` has no exact requirement identity",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name
                ));
            } else if let Some(previous_index) = self.schema.methods[..method_index]
                .iter()
                .position(|previous| previous.requirement_identity == method.requirement_identity)
            {
                errors.push(format!(
                    "plan `{}` schema methods at indexes {previous_index} and {method_index} repeat exact requirement identity `{}`",
                    self.name, self.schema.identity_text(method.requirement_identity),
                ));
            }
            if method.parameter_type_identities.len() != method.parameter_count {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` declares {} parameters but retains {} exact parameter type identities",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                    method.parameter_count,
                    method.parameter_type_identities.len(),
                ));
            }
            for (parameter_index, identity) in method.parameter_type_identities.iter().enumerate() {
                if !identity.is_valid() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` parameter {} has no exact semantic type identity",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        parameter_index,
                    ));
                }
            }
            for premise in &method.termination_premises {
                if !method.terminates_guarantee {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` retains a progress premise without a termination guarantee",
                        self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                    ));
                }
                if premise.profile.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` has a progress premise with no exact profile identity",
                        self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                    ));
                }
                if premise.establishment_routes.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` has no authorized establishment route",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        premise.profile,
                    ));
                }
                if premise
                    .establishment_routes
                    .iter()
                    .any(|route| !route.requirement_identity.is_valid())
                {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` has an establishment route with no exact requirement identity",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        premise.profile,
                    ));
                }
                if premise.establishment_routes.iter().any(|route| {
                    route.kind != ServiceProgressEstablishmentRouteKind::BoundaryRequirement
                }) {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` has a non-boundary establishment route",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        premise.profile,
                    ));
                }
                let mut normalized_routes = premise.establishment_routes.clone();
                normalized_routes.sort_by(|left, right| {
                    left.kind.cmp(&right.kind).then_with(|| {
                        self.schema
                            .identity_text(left.requirement_identity)
                            .cmp(self.schema.identity_text(right.requirement_identity))
                    })
                });
                normalized_routes.dedup();
                if normalized_routes.len() != premise.establishment_routes.len() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise `{}` repeats an establishment route",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        premise.profile,
                    ));
                }
                if let ServiceProgressSubject::Parameter(index) = premise.subject
                    && index >= method.parameter_count
                {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise names out-of-range parameter {index} of {}",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        method.parameter_count,
                    ));
                }
                if premise
                    .subject_projections
                    .iter()
                    .any(|projection| projection.is_empty())
                {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` progress premise has an empty subject projection identity",
                        self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                    ));
                }
            }
            for claim in &method.entry_claims {
                if claim.parameter_index >= method.parameter_count {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim names out-of-range parameter {} of {}",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        claim.parameter_index,
                        method.parameter_count,
                    ));
                }
                if claim.domain.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim for parameter {} has no exact semantic domain identity",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        claim.parameter_index,
                    ));
                }
                if !claim.carrier_identity.is_valid() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim for parameter {} has no exact carrier identity",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        claim.parameter_index,
                    ));
                }
                if claim.effective_carry != language_semantics::CarryPolicy::STRICT {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claim for parameter {} and domain `{}` is not born-strict",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        claim.parameter_index,
                        claim.domain,
                    ));
                }
            }
            for (index, pair) in method.entry_claims.windows(2).enumerate() {
                let left = (
                    pair[0].parameter_index,
                    self.schema.identities.text(pair[0].carrier_identity),
                    pair[0].domain.as_str(),
                );
                let right = (
                    pair[1].parameter_index,
                    self.schema.identities.text(pair[1].carrier_identity),
                    pair[1].domain.as_str(),
                );
                if left >= right {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` entry claims are not strictly increasing at indexes {index} and {}",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        index + 1,
                    ));
                }
            }
            if matches!(
                method.completion,
                semantic_vocabulary::BoundaryCompletion::External(_)
            ) && method.has_result
            {
                errors.push(format!(
                    "plan `{}` external completion requirement `{}` declares a normal result",
                    self.name,
                    self.schema.identity_text(method.requirement_identity)
                ));
            }
            if method.has_result == !method.result_type_identity.is_valid() {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` result presence disagrees with its exact result type identity",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                ));
            }
            if method.has_result
                && self
                    .schema
                    .identities
                    .text(method.result_type_identity)
                    .is_empty()
            {
                errors.push(format!(
                    "plan `{}` schema method `{}::{}` result has no exact semantic type identity",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                ));
            }
            for claim in &method.result_claims {
                if !method.has_result || !method.result_type_identity.is_valid() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` retains a result claim without a real result",
                        self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                    ));
                }
                if claim.domain.is_empty() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` result claim has no exact semantic domain identity",
                        self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                    ));
                }
                if claim.effective_carry != language_semantics::CarryPolicy::STRICT {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` result claim for domain `{}` is not born-strict",
                        self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name, claim.domain,
                    ));
                }
            }
            for (index, pair) in method.result_claims.windows(2).enumerate() {
                if pair[0].domain.as_str() >= pair[1].domain.as_str() {
                    errors.push(format!(
                        "plan `{}` schema method `{}::{}` result claims are not strictly increasing at indexes {index} and {}",
                        self.name,
                        self.schema.identity_text(self.schema.trait_name),
                        method.name,
                        index + 1,
                    ));
                }
            }
            for (axis, identities) in [
                ("service-reach", method.service_reach.as_slice()),
                (
                    "synchronous-invocation",
                    method.synchronous_invocations.as_slice(),
                ),
            ] {
                for (index, identity) in identities.iter().enumerate() {
                    if identity.is_empty() {
                        errors.push(format!(
                            "plan `{}` schema method `{}::{}` {axis} identity at index {index} is empty",
                            self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name,
                        ));
                    }
                }
                for (index, pair) in identities.windows(2).enumerate() {
                    if pair[0] >= pair[1] {
                        errors.push(format!(
                            "plan `{}` schema method `{}::{}` {axis} identities are not strictly increasing at indexes {index} and {}",
                            self.name,
                            self.schema.identity_text(self.schema.trait_name),
                            method.name,
                            index + 1,
                        ));
                    }
                }
            }
        }
        for row in &self.rows {
            if !row.requirement_identity.is_valid() {
                errors.push(format!(
                    "plan `{}` row `{}` has no exact requirement identity",
                    self.name, row.method
                ));
            }
            if language_semantics::type_identity::normalize_requirement_lifetime_partition(
                &row.requirement_lifetime_partition,
            ) != row.requirement_lifetime_partition
            {
                errors.push(format!(
                    "plan `{}` row `{}` has a noncanonical requirement lifetime partition",
                    self.name, row.method,
                ));
            }
            match &row.binding {
                ProviderBinding::Import { evaluated } => {
                    let locator = evaluated.locator();
                    if self.target != locator.target().target_name() {
                        errors.push(format!(
                            "plan `{}` row `{}` normalized import targets `{}`, but the provider plan targets `{}`",
                            self.name,
                            row.method,
                            locator.target().target_name(),
                            self.target,
                        ));
                    }
                }
                ProviderBinding::Syscall { number } => {
                    if u32::try_from(*number).is_err() {
                        errors.push(format!(
                            "provider binding `{}::{}` has syscall number {number}, but the target syscall plan requires a value in 0..={}",
                            self.schema.identity_text(self.schema.trait_name),
                            row.method,
                            u32::MAX,
                        ));
                    }
                }
                ProviderBinding::CompilerIntrinsic { machine } => {
                    if machine.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` compiler intrinsic has no exact realization-machine identity",
                            self.name, row.method,
                        ));
                    }
                }
                ProviderBinding::VtableSlot { index } => {
                    if *index < 0 {
                        errors.push(format!(
                            "plan `{}` row `{}` vtable slot index {index} is negative",
                            self.name, row.method,
                        ));
                    }
                }
                ProviderBinding::VtableField { table, field }
                | ProviderBinding::TableFunction { table, field } => {
                    if table.is_empty() {
                        errors.push(format!(
                            "external leaf for `{}::{}` uses a table field without a table data owner; declare it as `machine TableType::leaf(...) satisfies {}::{} via ForeignBinding::...`",
                            self.schema.identity_text(self.schema.trait_name),
                            row.method,
                            self.schema.identity_text(self.schema.trait_name),
                            row.method,
                        ));
                    } else if self.provider_type.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` table binding has no nominal provider type",
                            self.name, row.method,
                        ));
                    }
                    // A named implementation maps requirements to machines;
                    // it is not the table data type. Layout comes from each
                    // selected leaf's exact declaration provenance, not equality
                    // between the implementation's name and the table's name.
                    if field.is_empty() {
                        errors.push(format!(
                            "plan `{}` row `{}` table binding has no exact field identity",
                            self.name, row.method,
                        ));
                    }
                }
                ProviderBinding::CheckedAdapter {
                    machine_identity,
                    machine_package_identity,
                } => {
                    if !machine_identity.is_valid() {
                        errors.push(format!(
                            "plan `{}` row `{}` checked adapter has no exact machine identity",
                            self.name, row.method,
                        ));
                    }
                    if self.provider_type.is_empty() {
                        errors.push(format!(
                            "checked adapter `{}` for `{}::{}` has no nominal provider type; attach it as an ordinary checked machine satisfying {}::{}` and select that provider for the boundary slot",
                            self.identity_text(*machine_identity),
                            self.schema.identity_text(self.schema.trait_name),
                            row.method,
                            self.schema.identity_text(self.schema.trait_name),
                            row.method,
                        ));
                    }
                    if *machine_package_identity != self.origin_package_identity {
                        errors.push(format!(
                            "plan `{}` row `{}` checked-adapter package identity does not match the realizing package",
                            self.name, row.method,
                        ));
                    }
                }
            }
        }
        for method in &self.schema.methods {
            let count = self
                .rows
                .iter()
                .filter(|row| self.schema.row_binds_method(row, method, &self.identities))
                .count();
            if count > 1 {
                errors.push(format!(
                    "plan `{}` binds `{}::{}` {count} times; one row per method",
                    self.name,
                    self.schema.identity_text(self.schema.trait_name),
                    method.name
                ));
            }
        }
        for row in &self.rows {
            if !self
                .schema
                .methods
                .iter()
                .any(|method| self.schema.row_binds_method(row, method, &self.identities))
            {
                errors.push(format!(
                    "plan `{}` binds `{}`, which is not a `{}` method",
                    self.name,
                    row.method,
                    self.schema.identity_text(self.schema.trait_name)
                ));
            }
        }
        errors
    }

    /// PRV2 preview (the cheapest structural fact, used by tests today):
    /// every schema method has exactly one row and every row names a
    /// schema method.
    pub fn covers_schema(&self) -> bool {
        self.schema.methods.iter().all(|method| {
            self.rows
                .iter()
                .filter(|row| self.schema.row_binds_method(row, method, &self.identities))
                .count()
                == 1
        }) && self.rows.iter().all(|row| {
            self.schema
                .methods
                .iter()
                .any(|method| self.schema.row_binds_method(row, method, &self.identities))
        })
    }
}
