use crate::provider_plan::{ProviderBinding, ProviderPlan};

/// Why an executable entry belongs to the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableEntryOrigin {
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
pub enum ProviderIdentity {
    NominalType(String),
    FreeExternalPlan(String),
}

/// Executable identities the selected-plan carrier can establish today.
///
/// These are identities within the artifact being compiled. Opaque loader
/// names deliberately do not appear here: a path, module, or symbol is not a
/// pinned executable-content identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutableIdentity {
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
pub enum ImplementationEvidence {
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
pub enum ContainmentGuarantee {
    MemoryIsolation,
    ForcibleTermination,
    FaultContainment,
    BoundedResources,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainmentEvidence {
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
pub struct OpaqueExecutableAdmissionCandidate {
    pub provider_plan_identity: u64,
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
pub struct ValidatedOpaqueExecutableAdmission(OpaqueExecutableAdmissionCandidate);

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
pub struct SelectedProviderRequirement {
    pub method: String,
    pub requirement_identity: String,
}

impl PartialEq for SelectedProviderRequirement {
    fn eq(&self, other: &Self) -> bool {
        self.requirement_identity == other.requirement_identity
    }
}

impl Eq for SelectedProviderRequirement {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTcbEntry {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_identity: u64,
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
pub enum IncompleteCause {
    SelectedOpaqueProvider {
        provider_identity: ProviderIdentity,
        provider_plan_identity: u64,
        method: String,
        requirement_identity: String,
        binding: OpaqueInProcessBinding,
    },
    OmegaRuntimeAdmission {
        provider_identity: ProviderIdentity,
        provider_plan_identity: u64,
        executable_identity: String,
        admission_receipt_identity: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpaqueInProcessBinding {
    Import {
        locator: crate::NormalizedForeignLocator,
    },
    StringBackedImportBootstrap {
        library: String,
        symbol: String,
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
pub enum ScopeCompleteness {
    Complete {
        scope: ExecutionScope,
        selected_provider_closure_identity: u64,
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
pub struct OpaqueClosureEvidence {
    pub provider_plan_identity: u64,
    pub method: String,
    pub requirement_identity: String,
    pub evidence_identity: String,
}

/// Evidence that one Omega-mediated runtime admission cannot introduce
/// executable bytes beyond the pinned entry reported by the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecutableClosureEvidence {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_identity: u64,
    pub executable_identity: String,
    pub admission_receipt_identity: String,
    pub evidence_identity: String,
}

/// Input accepted only through the Omega runtime-ledger admission boundary.
/// A loader path is intentionally absent: runtime code identity must already
/// be pinned by content, signer, or an admitted platform profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OmegaRuntimeExecutableAdmissionCandidate {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_identity: u64,
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
pub struct OmegaRuntimeExecutableLedger {
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
pub struct ExecutableTcbManifest {
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
                && admission.0.provider_plan_identity == candidate.provider_plan_identity
                && admission.0.executable_identity == candidate.executable_identity
        }) {
            return Err(format!(
                "Omega runtime executable admission duplicates artifact `{}` in provider plan {:#018x}",
                candidate.executable_identity, candidate.provider_plan_identity
            ));
        }
        self.admissions
            .push(ValidatedOmegaRuntimeExecutableAdmission(candidate));
        self.admissions.sort_by(|left, right| {
            left.0
                .provider_plan_identity
                .cmp(&right.0.provider_plan_identity)
                .then_with(|| left.0.executable_identity.cmp(&right.0.executable_identity))
                .then_with(|| {
                    left.0
                        .admission_receipt_identity
                        .cmp(&right.0.admission_receipt_identity)
                })
        });
        Ok(())
    }

    /// Union this Omega-mediated ledger snapshot with the static selected-plan
    /// manifest. A runtime admission without executable-closure evidence adds
    /// a known entry but also contributes an attributed incomplete cause.
    pub fn union_with_static_manifest(
        &self,
        static_manifest: &ExecutableTcbManifest,
    ) -> Result<ExecutableTcbManifest, String> {
        let static_scope = match &static_manifest.completeness {
            ScopeCompleteness::Complete { scope, .. }
            | ScopeCompleteness::Incomplete { scope, .. } => *scope,
        };
        if static_scope != self.scope {
            return Err(format!(
                "Omega runtime executable ledger scope {:?} does not match static manifest scope {:?}",
                self.scope, static_scope
            ));
        }

        let mut manifest = static_manifest.clone();
        let mut runtime_causes = Vec::new();
        let mut runtime_closure_evidence = Vec::new();
        for admission in &self.admissions {
            let candidate = &admission.0;
            let entry = ExecutableTcbEntry {
                provider_identity: candidate.provider_identity.clone(),
                provider_plan_identity: candidate.provider_plan_identity,
                selected_requirement: None,
                executable_identity: ExecutableIdentity::PinnedOpaqueArtifact(
                    candidate.executable_identity.clone(),
                ),
                implementation_evidence: ImplementationEvidence::AdmittedOpaque {
                    receipt_identity: candidate.implementation_evidence_identity.clone(),
                },
                origin: ExecutableEntryOrigin::OmegaRuntimeAdmission,
                execution_scope: candidate.execution_scope,
                containment: candidate.containment.clone(),
            };
            if !manifest.known_entries.contains(&entry) {
                manifest.known_entries.push(entry);
            }
            if let Some(evidence_identity) = &candidate.executable_closure_evidence_identity {
                runtime_closure_evidence.push(RuntimeExecutableClosureEvidence {
                    provider_identity: candidate.provider_identity.clone(),
                    provider_plan_identity: candidate.provider_plan_identity,
                    executable_identity: candidate.executable_identity.clone(),
                    admission_receipt_identity: candidate.admission_receipt_identity.clone(),
                    evidence_identity: evidence_identity.clone(),
                });
            } else {
                runtime_causes.push(IncompleteCause::OmegaRuntimeAdmission {
                    provider_identity: candidate.provider_identity.clone(),
                    provider_plan_identity: candidate.provider_plan_identity,
                    executable_identity: candidate.executable_identity.clone(),
                    admission_receipt_identity: candidate.admission_receipt_identity.clone(),
                });
            }
        }

        manifest.completeness = match manifest.completeness {
            ScopeCompleteness::Complete {
                scope,
                selected_provider_closure_identity,
                opaque_closure_evidence,
                runtime_closure_evidence: mut retained_runtime_evidence,
            } if runtime_causes.is_empty() => {
                extend_unique(&mut retained_runtime_evidence, runtime_closure_evidence);
                ScopeCompleteness::Complete {
                    scope,
                    selected_provider_closure_identity,
                    opaque_closure_evidence,
                    runtime_closure_evidence: retained_runtime_evidence,
                }
            }
            ScopeCompleteness::Complete {
                scope,
                opaque_closure_evidence,
                runtime_closure_evidence: mut retained_runtime_evidence,
                ..
            } => {
                extend_unique(&mut retained_runtime_evidence, runtime_closure_evidence);
                ScopeCompleteness::Incomplete {
                    scope,
                    causes: runtime_causes,
                    opaque_closure_evidence,
                    runtime_closure_evidence: retained_runtime_evidence,
                }
            }
            ScopeCompleteness::Incomplete {
                scope,
                mut causes,
                opaque_closure_evidence,
                runtime_closure_evidence: mut retained_runtime_evidence,
            } => {
                extend_unique(&mut causes, runtime_causes);
                extend_unique(&mut retained_runtime_evidence, runtime_closure_evidence);
                ScopeCompleteness::Incomplete {
                    scope,
                    causes,
                    opaque_closure_evidence,
                    runtime_closure_evidence: retained_runtime_evidence,
                }
            }
        };
        Ok(manifest)
    }
}

fn extend_unique<T: PartialEq>(target: &mut Vec<T>, additions: impl IntoIterator<Item = T>) {
    for addition in additions {
        if !target.contains(&addition) {
            target.push(addition);
        }
    }
}

fn validate_runtime_admission(
    scope: ExecutionScope,
    candidate: &mut OmegaRuntimeExecutableAdmissionCandidate,
) -> Result<(), String> {
    if candidate.provider_plan_identity == 0 {
        return Err(
            "Omega runtime executable admission has the reserved zero provider-plan identity"
                .into(),
        );
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

pub(crate) fn validate_opaque_executable_admission(
    plans: &[ProviderPlan],
    mut candidate: OpaqueExecutableAdmissionCandidate,
) -> Result<ValidatedOpaqueExecutableAdmission, String> {
    if candidate.provider_plan_identity == 0 {
        return Err(
            "opaque executable admission has the reserved zero provider-plan identity".into(),
        );
    }
    let matching_plans = plans
        .iter()
        .filter(|plan| plan.identity_fingerprint() == candidate.provider_plan_identity)
        .collect::<Vec<_>>();
    let [plan] = matching_plans.as_slice() else {
        return Err(match matching_plans.len() {
            0 => format!(
                "opaque executable admission names unselected provider plan {:#018x}",
                candidate.provider_plan_identity
            ),
            count => format!(
                "opaque executable admission provider plan {:#018x} matches {count} selected plans",
                candidate.provider_plan_identity
            ),
        });
    };
    let matching_rows = plan
        .rows
        .iter()
        .filter(|row| {
            row.method == candidate.method
                && row.requirement_identity == candidate.requirement_identity
        })
        .collect::<Vec<_>>();
    let [row] = matching_rows.as_slice() else {
        return Err(match matching_rows.len() {
            0 => format!(
                "opaque executable admission does not match an exact selected row `{}` / `{}`",
                candidate.method, candidate.requirement_identity
            ),
            count => format!(
                "opaque executable admission matches {count} selected rows `{}` / `{}`",
                candidate.method, candidate.requirement_identity
            ),
        });
    };
    let selected_binding = opaque_binding(&row.binding).ok_or_else(|| {
        format!(
            "opaque executable admission targets non-opaque selected row `{}` / `{}`",
            candidate.method, candidate.requirement_identity
        )
    })?;
    if selected_binding != candidate.binding {
        return Err(format!(
            "opaque executable admission binding drift for selected row `{}` / `{}`",
            candidate.method, candidate.requirement_identity
        ));
    }
    if candidate.executable_identity.trim().is_empty() {
        return Err("opaque executable admission has no pinned executable identity".into());
    }
    if candidate.implementation_evidence_identity.trim().is_empty() {
        return Err("opaque executable admission has no implementation-evidence identity".into());
    }
    if candidate
        .executable_closure_evidence_identity
        .as_ref()
        .is_some_and(|identity| identity.trim().is_empty())
    {
        return Err(
            "opaque executable admission has an empty executable-closure evidence identity".into(),
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
                "opaque executable admission has no evidence identity for {:?}",
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
            "opaque executable admission repeats one containment guarantee; each axis needs one exact result"
                .into(),
        );
    }
    Ok(ValidatedOpaqueExecutableAdmission(candidate))
}

pub(crate) fn derive_static_manifest(
    plans: &[ProviderPlan],
    selected_provider_closure_identity: u64,
    scope: ExecutionScope,
    admissions: &[ValidatedOpaqueExecutableAdmission],
) -> ExecutableTcbManifest {
    let mut known_entries = Vec::new();
    let mut causes = Vec::new();
    let mut opaque_closure_evidence = Vec::new();

    for plan in plans {
        let provider_identity = provider_identity(plan);
        let provider_plan_identity = plan.identity_fingerprint();
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
                    provider_plan_identity,
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
                    provider_plan_identity,
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
                | ProviderBinding::StringBackedImportBootstrap { .. }
                | ProviderBinding::VtableSlot { .. }
                | ProviderBinding::VtableField { .. }
                | ProviderBinding::TableFunction { .. } => {
                    let binding = opaque_binding(&row.binding)
                        .expect("opaque binding match arms have an opaque binding");
                    let requirement_identity = row.requirement_identity.clone();
                    let admission = admissions.iter().find(|admission| {
                        let admission = admission.candidate();
                        admission.provider_plan_identity == provider_plan_identity
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
                                provider_plan_identity,
                                method: row.method.clone(),
                                requirement_identity: requirement_identity.clone(),
                                evidence_identity: evidence_identity.clone(),
                            });
                        } else {
                            causes.push(incomplete_cause(plan, row, binding.clone()));
                        }
                        Some(ExecutableTcbEntry {
                            provider_identity: provider_identity.clone(),
                            provider_plan_identity,
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
            selected_provider_closure_identity,
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
        ProviderBinding::Import { locator } => Some(OpaqueInProcessBinding::Import {
            locator: locator.clone(),
        }),
        ProviderBinding::StringBackedImportBootstrap { library, symbol } => {
            Some(OpaqueInProcessBinding::StringBackedImportBootstrap {
                library: library.clone(),
                symbol: symbol.clone(),
            })
        }
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
        provider_plan_identity: plan.identity_fingerprint(),
        method: row.method.clone(),
        requirement_identity,
        binding,
    }
}

#[cfg(test)]
mod runtime_ledger_tests {
    use super::*;

    fn candidate(
        executable_identity: &str,
        closure: Option<&str>,
    ) -> OmegaRuntimeExecutableAdmissionCandidate {
        OmegaRuntimeExecutableAdmissionCandidate {
            provider_identity: ProviderIdentity::NominalType("RuntimePlugin".into()),
            provider_plan_identity: 41,
            executable_identity: executable_identity.into(),
            implementation_evidence_identity: "receipt:implementation-v1".into(),
            admission_receipt_identity: format!("receipt:admission:{executable_identity}"),
            execution_scope: ExecutionScope::CallerAddressSpace,
            containment: vec![ContainmentEvidence {
                guarantee: ContainmentGuarantee::BoundedResources,
                evidence_identity: "receipt:runtime-quota-v1".into(),
            }],
            executable_closure_evidence_identity: closure.map(str::to_owned),
        }
    }

    fn static_manifest() -> ExecutableTcbManifest {
        ExecutableTcbManifest {
            known_entries: Vec::new(),
            completeness: ScopeCompleteness::Complete {
                scope: ExecutionScope::CallerAddressSpace,
                selected_provider_closure_identity: 17,
                opaque_closure_evidence: Vec::new(),
                runtime_closure_evidence: Vec::new(),
            },
        }
    }

    #[test]
    fn mediated_runtime_admission_adds_an_origin_marked_known_entry() {
        let mut ledger = OmegaRuntimeExecutableLedger::new(ExecutionScope::CallerAddressSpace)
            .expect("valid caller scope");
        ledger
            .admit(candidate(
                "sha256:plugin-a",
                Some("receipt:closed-loader-v1"),
            ))
            .expect("exact mediated admission");

        let manifest = ledger
            .union_with_static_manifest(&static_manifest())
            .expect("matching scope");
        assert_eq!(manifest.known_entries.len(), 1);
        assert_eq!(
            manifest.known_entries[0].origin,
            ExecutableEntryOrigin::OmegaRuntimeAdmission
        );
        assert!(matches!(
            manifest.known_entries[0].executable_identity,
            ExecutableIdentity::PinnedOpaqueArtifact(ref identity)
                if identity == "sha256:plugin-a"
        ));
        assert!(matches!(
            manifest.completeness,
            ScopeCompleteness::Complete {
                ref runtime_closure_evidence,
                ..
            } if runtime_closure_evidence.len() == 1
                && runtime_closure_evidence[0].admission_receipt_identity
                        == "receipt:admission:sha256:plugin-a"
        ));

        let repeated = ledger
            .union_with_static_manifest(&manifest)
            .expect("set union is reusable");
        assert_eq!(repeated, manifest);
    }

    #[test]
    fn runtime_admission_without_closure_is_known_but_attributed_incomplete() {
        let mut ledger = OmegaRuntimeExecutableLedger::new(ExecutionScope::CallerAddressSpace)
            .expect("valid caller scope");
        ledger
            .admit(candidate("sha256:plugin-open", None))
            .expect("pinned executable can be reported without closure evidence");

        let manifest = ledger
            .union_with_static_manifest(&static_manifest())
            .expect("matching scope");
        assert_eq!(manifest.known_entries.len(), 1);
        assert!(matches!(
            manifest.completeness,
            ScopeCompleteness::Incomplete { ref causes, .. }
                if matches!(
                    causes.as_slice(),
                    [IncompleteCause::OmegaRuntimeAdmission {
                        executable_identity,
                        admission_receipt_identity,
                        ..
                    }] if executable_identity == "sha256:plugin-open"
                        && admission_receipt_identity
                            == "receipt:admission:sha256:plugin-open"
                )
        ));
    }

    #[test]
    fn ledger_rejects_unmediated_entries_and_receipt_replay() {
        let mut ledger = OmegaRuntimeExecutableLedger::new(ExecutionScope::CallerAddressSpace)
            .expect("valid caller scope");
        let mut unmediated = candidate("sha256:no-receipt", None);
        unmediated.admission_receipt_identity.clear();
        assert!(
            ledger
                .admit(unmediated)
                .expect_err("missing mediation receipt")
                .contains("no mediation receipt")
        );

        let admitted = candidate("sha256:plugin-a", Some("receipt:closed-loader-v1"));
        ledger
            .admit(admitted.clone())
            .expect("first receipt use is exact");
        let mut replay = candidate("sha256:plugin-b", Some("receipt:closed-loader-v2"));
        replay.admission_receipt_identity = admitted.admission_receipt_identity;
        assert!(
            ledger
                .admit(replay)
                .expect_err("receipt replay")
                .contains("reuses receipt")
        );
    }
}
