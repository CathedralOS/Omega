//! What is inside one execution scope's trusted computing base, and which entries
//! the compiler could not describe.

use crate::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanDigest};

/// Why an executable entry belongs to the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExecutableEntryOrigin {
    StaticSelection,
    OmegaRuntimeAdmission,
}

/// The execution scope whose known executable entries are being described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionScope {
    CallerAddressSpace,
    IsolatedProvider(u64),
}

/// Exact identity of the provider selected for one boundary slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProviderIdentity {
    NominalType(String),
    FreeExternalPlan(String),
}

/// Executable identities the selected-plan carrier can establish today.
///
/// These are identities within the artifact being compiled. Opaque loader
/// names deliberately do not appear here: a path, module, or symbol is not a
/// pinned executable-content identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ExecutableIdentity {
    CurrentArtifactMachine(String),
    CurrentArtifactIntrinsic {
        target: String,
        machine: String,
    },
    PinnedOpaqueArtifact(String),
    IsolatedProviderEndpoint {
        scope_identity: u64,
        endpoint_identity: String,
    },
}

/// Evidence for how the executable implementation is supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ImplementationEvidence {
    CheckedBody {
        machine: String,
    },
    CompilerKnown {
        target: String,
        machine: String,
    },
    AdmittedOpaque {
        receipt_identity: String,
    },
    AdmittedIsolatedEndpoint {
        endpoint_receipt_identity: String,
        isolated_manifest_receipt_identity: String,
    },
}

/// Containment guarantees remain independent; one receipt cannot imply the
/// others merely because it names the same mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ContainmentGuarantee {
    MemoryIsolation,
    ForcibleTermination,
    FaultContainment,
    BoundedResources,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContainmentEvidence {
    pub guarantee: ContainmentGuarantee,
    pub evidence_identity: String,
}

/// Policy/admission input for one exact opaque selected-provider row.
///
/// Loader spellings remain only a binding match. `executable_identity` must
/// pin content, a signer/profile-owned platform identity, or another stable
/// artifact identity supplied by the admitting policy. Scope completeness is
/// independent: only `executable_closure_evidence_identity` claims that this
/// opaque executable cannot introduce unreported code in the named scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpaqueExecutableAdmissionCandidate {
    /// Compact report coordinate; never sufficient to select a plan.
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub method: String,
    pub requirement_identity: String,
    pub binding: OpaqueInProcessBinding,
    pub executable_identity: String,
    pub implementation_evidence_identity: String,
    pub execution_scope: ExecutionScope,
    pub containment: Vec<ContainmentEvidence>,
    pub executable_closure_evidence_identity: Option<String>,
}

/// Sealed admission whose provider, requirement, binding, and evidence have
/// been matched against the selected-provider closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedOpaqueExecutableAdmission(OpaqueExecutableAdmissionCandidate);

impl ValidatedOpaqueExecutableAdmission {
    pub const fn candidate(&self) -> &OpaqueExecutableAdmissionCandidate {
        &self.0
    }
}

/// Exact selected provider row whose implementation contributes this entry.
///
/// `method` is readable drift/debug data. `requirement_identity` is the
/// canonical overload and blast-radius identity and is never empty for a
/// static selected-plan entry.
#[derive(Debug, Clone)]
pub(crate) struct SelectedProviderRequirement {
    pub method: String,
    pub requirement_identity: String,
}

// Readable method labels may drift without changing the selected requirement.
// Equality also deduplicates repeated entries for that exact requirement.
impl PartialEq for SelectedProviderRequirement {
    fn eq(&self, other: &Self) -> bool {
        self.requirement_identity == other.requirement_identity
    }
}

impl Eq for SelectedProviderRequirement {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableTcbEntry {
    pub provider_identity: ProviderIdentity,
    /// Compact report coordinate beside the authoritative plan digest.
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    /// `None` only for an admission that does not originate from one selected
    /// static ProviderPlan row, such as a runtime-loaded artifact or isolated
    /// endpoint.
    pub selected_requirement: Option<SelectedProviderRequirement>,
    pub executable_identity: ExecutableIdentity,
    pub implementation_evidence: ImplementationEvidence,
    pub origin: ExecutableEntryOrigin,
    pub execution_scope: ExecutionScope,
    pub containment: Vec<ContainmentEvidence>,
}

/// Attributed reason a scope-relative executable inventory is not exhaustive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum IncompleteCause {
    SelectedOpaqueProvider {
        provider_identity: ProviderIdentity,
        provider_plan_report_identity: u64,
        provider_plan_digest: ProviderPlanDigest,
        method: String,
        requirement_identity: String,
        binding: OpaqueInProcessBinding,
    },
    OmegaRuntimeAdmission {
        provider_identity: ProviderIdentity,
        provider_plan_report_identity: u64,
        provider_plan_digest: ProviderPlanDigest,
        executable_identity: String,
        admission_receipt_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpaqueInProcessBinding {
    Import {
        evaluated: crate::provider_plan::EvaluatedForeignImport,
    },
    VtableSlot {
        index: i64,
    },
    VtableField {
        table: String,
        field: String,
    },
    TableFunction {
        table: String,
        field: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScopeCompleteness {
    Complete {
        scope: ExecutionScope,
        selected_provider_closure_report_identity: u64,
        opaque_closure_evidence: Vec<OpaqueClosureEvidence>,
        runtime_closure_evidence: Vec<RuntimeExecutableClosureEvidence>,
    },
    Incomplete {
        scope: ExecutionScope,
        causes: Vec<IncompleteCause>,
        opaque_closure_evidence: Vec<OpaqueClosureEvidence>,
        runtime_closure_evidence: Vec<RuntimeExecutableClosureEvidence>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpaqueClosureEvidence {
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub method: String,
    pub requirement_identity: String,
    pub evidence_identity: String,
}

/// Evidence that one Omega-mediated runtime admission cannot introduce
/// executable bytes beyond the pinned entry reported by the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeExecutableClosureEvidence {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub executable_identity: String,
    pub admission_receipt_identity: String,
    pub evidence_identity: String,
}

/// Input accepted only through the Omega runtime-ledger admission boundary.
/// A loader path is intentionally absent: runtime code identity must already
/// be pinned by content, signer, or an admitted platform profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OmegaRuntimeExecutableAdmissionCandidate {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: ProviderPlanDigest,
    pub executable_identity: String,
    pub implementation_evidence_identity: String,
    pub admission_receipt_identity: String,
    pub execution_scope: ExecutionScope,
    pub containment: Vec<ContainmentEvidence>,
    pub executable_closure_evidence_identity: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedOmegaRuntimeExecutableAdmission(OmegaRuntimeExecutableAdmissionCandidate);

/// Append-only snapshot of executable admissions performed through Omega's
/// runtime mediation boundary. It makes no claim about code introduced by an
/// opaque provider outside that boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OmegaRuntimeExecutableLedger {
    scope: ExecutionScope,
    admissions: Vec<ValidatedOmegaRuntimeExecutableAdmission>,
}

/// TCB information derivable from the exact selected-provider closure.
///
/// This is intentionally not derived from source service reach. It also does
/// not pretend that an opaque library or function-pointer spelling is an exact
/// executable identity. Such rows remain attributed incompleteness causes
/// until provider admission supplies pinned identity and containment evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutableTcbManifest {
    pub known_entries: Vec<ExecutableTcbEntry>,
    pub completeness: ScopeCompleteness,
}

impl OmegaRuntimeExecutableLedger {
    pub fn new(scope: ExecutionScope) -> Result<Self, String> {
        if matches!(scope, ExecutionScope::IsolatedProvider(0)) {
            return Err(
                "Omega runtime executable ledger has the reserved zero scope identity".into(),
            );
        }
        Ok(Self {
            scope,
            admissions: Vec::new(),
        })
    }

    pub const fn scope(&self) -> ExecutionScope {
        self.scope
    }

    pub fn len(&self) -> usize {
        self.admissions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.admissions.is_empty()
    }

    /// Admit one exact executable through Omega mediation. The sealed ledger
    /// entry is the only route by which runtime-origin entries reach a unioned
    /// manifest.
    pub fn admit(
        &mut self,
        mut candidate: OmegaRuntimeExecutableAdmissionCandidate,
    ) -> Result<(), String> {
        validate_runtime_admission(self.scope, &mut candidate)?;
        if self.admissions.iter().any(|admission| {
            admission.0.admission_receipt_identity == candidate.admission_receipt_identity
        }) {
            return Err(format!(
                "Omega runtime executable admission reuses receipt `{}`",
                candidate.admission_receipt_identity
            ));
        }
        if self.admissions.iter().any(|admission| {
            admission.0.provider_identity == candidate.provider_identity
                && admission.0.provider_plan_report_identity
                    == candidate.provider_plan_report_identity
                && admission.0.provider_plan_digest == candidate.provider_plan_digest
                && admission.0.executable_identity == candidate.executable_identity
        }) {
            return Err(format!(
                "Omega runtime executable admission duplicates artifact `{}` in provider plan {:#018x}",
                candidate.executable_identity, candidate.provider_plan_report_identity
            ));
        }
        self.admissions
            .push(ValidatedOmegaRuntimeExecutableAdmission(candidate));
        self.admissions.sort_by(|left, right| {
            left.0
                .provider_plan_report_identity
                .cmp(&right.0.provider_plan_report_identity)
                .then_with(|| {
                    left.0
                        .provider_plan_digest
                        .cmp(&right.0.provider_plan_digest)
                })
                .then_with(|| left.0.executable_identity.cmp(&right.0.executable_identity))
                .then_with(|| {
                    left.0
                        .admission_receipt_identity
                        .cmp(&right.0.admission_receipt_identity)
                })
        });
        Ok(())
    }

}


fn validate_runtime_admission(
    scope: ExecutionScope,
    candidate: &mut OmegaRuntimeExecutableAdmissionCandidate,
) -> Result<(), String> {
    if candidate.provider_plan_report_identity == 0 {
        return Err(
            "Omega runtime executable admission has the reserved zero provider-plan identity"
                .into(),
        );
    }
    if candidate
        .provider_plan_digest
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err("Omega runtime executable admission has a zero provider-plan digest".into());
    }
    let provider_name = match &candidate.provider_identity {
        ProviderIdentity::NominalType(name) | ProviderIdentity::FreeExternalPlan(name) => name,
    };
    if provider_name.trim().is_empty() {
        return Err("Omega runtime executable admission has no provider identity".into());
    }
    if candidate.executable_identity.trim().is_empty() {
        return Err("Omega runtime executable admission has no pinned executable identity".into());
    }
    if candidate.implementation_evidence_identity.trim().is_empty() {
        return Err(
            "Omega runtime executable admission has no implementation-evidence identity".into(),
        );
    }
    if candidate.admission_receipt_identity.trim().is_empty() {
        return Err("Omega runtime executable admission has no mediation receipt".into());
    }
    if candidate.execution_scope != scope {
        return Err(format!(
            "Omega runtime executable admission scope {:?} does not match ledger scope {:?}",
            candidate.execution_scope, scope
        ));
    }
    if candidate
        .executable_closure_evidence_identity
        .as_ref()
        .is_some_and(|identity| identity.trim().is_empty())
    {
        return Err(
            "Omega runtime executable admission has empty executable-closure evidence".into(),
        );
    }
    candidate.containment.sort_by(|left, right| {
        left.guarantee
            .cmp(&right.guarantee)
            .then_with(|| left.evidence_identity.cmp(&right.evidence_identity))
    });
    for evidence in &candidate.containment {
        if evidence.evidence_identity.trim().is_empty() {
            return Err(format!(
                "Omega runtime executable admission has no evidence identity for {:?}",
                evidence.guarantee
            ));
        }
    }
    if candidate
        .containment
        .windows(2)
        .any(|pair| pair[0].guarantee == pair[1].guarantee)
    {
        return Err(
            "Omega runtime executable admission repeats one containment guarantee; each axis needs one exact result"
                .into(),
        );
    }
    Ok(())
}


pub(crate) fn derive_static_manifest(
    plans: &[ProviderPlan],
    selected_provider_closure_report_identity: u64,
    scope: ExecutionScope,
    admissions: &[ValidatedOpaqueExecutableAdmission],
) -> ExecutableTcbManifest {
    let mut known_entries = Vec::new();
    let mut causes = Vec::new();
    let mut opaque_closure_evidence = Vec::new();

    for plan in plans {
        let provider_identity = provider_identity(plan);
        let provider_plan_report_identity = plan.report_fingerprint();
        let provider_plan_digest = plan.identity_digest();
        for row in &plan.rows {
            assert!(
                !row.requirement_identity.is_empty(),
                "selected ProviderPlan rows have exact requirement identities"
            );
            let selected_requirement = Some(SelectedProviderRequirement {
                method: row.method.clone(),
                requirement_identity: row.requirement_identity.clone(),
            });
            let known = match &row.binding {
                ProviderBinding::CheckedAdapter {
                    machine_identity, ..
                } => Some(ExecutableTcbEntry {
                    provider_identity: provider_identity.clone(),
                    provider_plan_report_identity,
                    provider_plan_digest,
                    selected_requirement: selected_requirement.clone(),
                    executable_identity: ExecutableIdentity::CurrentArtifactMachine(
                        machine_identity.clone(),
                    ),
                    implementation_evidence: ImplementationEvidence::CheckedBody {
                        machine: machine_identity.clone(),
                    },
                    origin: ExecutableEntryOrigin::StaticSelection,
                    execution_scope: scope,
                    containment: Vec::new(),
                }),
                ProviderBinding::CompilerIntrinsic { machine, .. } => Some(ExecutableTcbEntry {
                    provider_identity: provider_identity.clone(),
                    provider_plan_report_identity,
                    provider_plan_digest,
                    selected_requirement: selected_requirement.clone(),
                    executable_identity: ExecutableIdentity::CurrentArtifactIntrinsic {
                        target: plan.target.clone(),
                        machine: machine.clone(),
                    },
                    implementation_evidence: ImplementationEvidence::CompilerKnown {
                        target: plan.target.clone(),
                        machine: machine.clone(),
                    },
                    origin: ExecutableEntryOrigin::StaticSelection,
                    execution_scope: scope,
                    containment: Vec::new(),
                }),
                ProviderBinding::Import { .. }
                | ProviderBinding::VtableSlot { .. }
                | ProviderBinding::VtableField { .. }
                | ProviderBinding::TableFunction { .. } => {
                    let binding = opaque_binding(&row.binding)
                        .expect("opaque binding match arms have an opaque binding");
                    let requirement_identity = row.requirement_identity.clone();
                    let admission = admissions.iter().find(|admission| {
                        let admission = admission.candidate();
                        admission.provider_plan_report_identity == provider_plan_report_identity
                            && admission.provider_plan_digest == provider_plan_digest
                            && admission.method == row.method
                            && admission.requirement_identity == requirement_identity
                            && admission.binding == binding
                    });
                    if let Some(admission) = admission {
                        let admission = admission.candidate();
                        if let Some(evidence_identity) =
                            &admission.executable_closure_evidence_identity
                        {
                            opaque_closure_evidence.push(OpaqueClosureEvidence {
                                provider_plan_report_identity,
                                provider_plan_digest,
                                method: row.method.clone(),
                                requirement_identity: requirement_identity.clone(),
                                evidence_identity: evidence_identity.clone(),
                            });
                        } else {
                            causes.push(incomplete_cause(plan, row, binding.clone()));
                        }
                        Some(ExecutableTcbEntry {
                            provider_identity: provider_identity.clone(),
                            provider_plan_report_identity,
                            provider_plan_digest,
                            selected_requirement: selected_requirement.clone(),
                            executable_identity: ExecutableIdentity::PinnedOpaqueArtifact(
                                admission.executable_identity.clone(),
                            ),
                            implementation_evidence: ImplementationEvidence::AdmittedOpaque {
                                receipt_identity: admission
                                    .implementation_evidence_identity
                                    .clone(),
                            },
                            origin: ExecutableEntryOrigin::StaticSelection,
                            execution_scope: admission.execution_scope,
                            containment: admission.containment.clone(),
                        })
                    } else {
                        causes.push(incomplete_cause(plan, row, binding));
                        None
                    }
                }
                // A syscall transfers execution to another scope; it does not
                // introduce opaque executable bytes into the caller address
                // space. Platform identity belongs to that scope's manifest.
                ProviderBinding::Syscall { .. } => None,
            };
            if let Some(entry) = known
                && !known_entries.contains(&entry)
            {
                known_entries.push(entry);
            }
        }
    }

    let completeness = if causes.is_empty() {
        ScopeCompleteness::Complete {
            scope,
            selected_provider_closure_report_identity,
            opaque_closure_evidence,
            runtime_closure_evidence: Vec::new(),
        }
    } else {
        ScopeCompleteness::Incomplete {
            scope,
            causes,
            opaque_closure_evidence,
            runtime_closure_evidence: Vec::new(),
        }
    };
    ExecutableTcbManifest {
        known_entries,
        completeness,
    }
}

fn provider_identity(plan: &ProviderPlan) -> ProviderIdentity {
    if plan.provider_type.is_empty() {
        ProviderIdentity::FreeExternalPlan(plan.name.clone())
    } else {
        ProviderIdentity::NominalType(plan.provider_type.clone())
    }
}

fn opaque_binding(binding: &ProviderBinding) -> Option<OpaqueInProcessBinding> {
    match binding {
        ProviderBinding::Import { evaluated } => Some(OpaqueInProcessBinding::Import {
            evaluated: evaluated.clone(),
        }),
        ProviderBinding::VtableSlot { index } => {
            Some(OpaqueInProcessBinding::VtableSlot { index: *index })
        }
        ProviderBinding::VtableField { table, field } => {
            Some(OpaqueInProcessBinding::VtableField {
                table: table.clone(),
                field: field.clone(),
            })
        }
        ProviderBinding::TableFunction { table, field } => {
            Some(OpaqueInProcessBinding::TableFunction {
                table: table.clone(),
                field: field.clone(),
            })
        }
        ProviderBinding::Syscall { .. }
        | ProviderBinding::CompilerIntrinsic { .. }
        | ProviderBinding::CheckedAdapter { .. } => None,
    }
}

fn incomplete_cause(
    plan: &ProviderPlan,
    row: &crate::provider_plan::ProviderPlanRow,
    binding: OpaqueInProcessBinding,
) -> IncompleteCause {
    let requirement_identity = row.requirement_identity.clone();
    IncompleteCause::SelectedOpaqueProvider {
        provider_identity: provider_identity(plan),
        provider_plan_report_identity: plan.report_fingerprint(),
        provider_plan_digest: plan.identity_digest(),
        method: row.method.clone(),
        requirement_identity,
        binding,
    }
}
