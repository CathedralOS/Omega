use crate::provider_plan::{ProviderBinding, ProviderPlan, ProviderPlanRow};

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
    CurrentArtifactIntrinsic { target: String, name: String },
    PinnedOpaqueArtifact(String),
}

/// Evidence for how the executable implementation is supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplementationEvidence {
    CheckedBody { machine: String },
    CompilerKnown { target: String, intrinsic: String },
    AdmittedOpaque { receipt_identity: String },
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableTcbEntry {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_identity: u64,
    pub executable_identity: ExecutableIdentity,
    pub implementation_evidence: ImplementationEvidence,
    pub origin: ExecutableEntryOrigin,
    pub execution_scope: ExecutionScope,
    pub containment: Vec<ContainmentEvidence>,
}

/// Exact selected row that prevents an exhaustive inventory for a scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteCause {
    pub provider_identity: ProviderIdentity,
    pub provider_plan_identity: u64,
    pub method: String,
    pub requirement_identity: String,
    pub binding: OpaqueInProcessBinding,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpaqueInProcessBinding {
    Import { library: String, symbol: String },
    VtableSlot { index: i64 },
    VtableField { table: String, field: String },
    TableFunction { table: String, field: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeCompleteness {
    Complete {
        scope: ExecutionScope,
        selected_provider_closure_identity: u64,
        opaque_closure_evidence: Vec<OpaqueClosureEvidence>,
    },
    Incomplete {
        scope: ExecutionScope,
        causes: Vec<IncompleteCause>,
        opaque_closure_evidence: Vec<OpaqueClosureEvidence>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueClosureEvidence {
    pub provider_plan_identity: u64,
    pub method: String,
    pub requirement_identity: String,
    pub evidence_identity: String,
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
                && row_requirement_identity(plan, row) == candidate.requirement_identity
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
    admissions: &[ValidatedOpaqueExecutableAdmission],
) -> ExecutableTcbManifest {
    let scope = ExecutionScope::CallerAddressSpace;
    let mut known_entries = Vec::new();
    let mut causes = Vec::new();
    let mut opaque_closure_evidence = Vec::new();

    for plan in plans {
        let provider_identity = provider_identity(plan);
        let provider_plan_identity = plan.identity_fingerprint();
        for row in &plan.rows {
            let known = match &row.binding {
                ProviderBinding::CheckedAdapter { machine } => Some(ExecutableTcbEntry {
                    provider_identity: provider_identity.clone(),
                    provider_plan_identity,
                    executable_identity: ExecutableIdentity::CurrentArtifactMachine(
                        machine.clone(),
                    ),
                    implementation_evidence: ImplementationEvidence::CheckedBody {
                        machine: machine.clone(),
                    },
                    origin: ExecutableEntryOrigin::StaticSelection,
                    execution_scope: scope,
                    containment: Vec::new(),
                }),
                ProviderBinding::CompilerIntrinsic { name } => Some(ExecutableTcbEntry {
                    provider_identity: provider_identity.clone(),
                    provider_plan_identity,
                    executable_identity: ExecutableIdentity::CurrentArtifactIntrinsic {
                        target: plan.target.clone(),
                        name: name.clone(),
                    },
                    implementation_evidence: ImplementationEvidence::CompilerKnown {
                        target: plan.target.clone(),
                        intrinsic: name.clone(),
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
                    let requirement_identity = row_requirement_identity(plan, row);
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
        }
    } else {
        ScopeCompleteness::Incomplete {
            scope,
            causes,
            opaque_closure_evidence,
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

fn row_requirement_identity(plan: &ProviderPlan, row: &ProviderPlanRow) -> String {
    if row.requirement_identity.is_empty() {
        plan.schema
            .methods
            .iter()
            .find(|method| method.name == row.method)
            .map(|method| method.requirement_identity.clone())
            .unwrap_or_default()
    } else {
        row.requirement_identity.clone()
    }
}

fn opaque_binding(binding: &ProviderBinding) -> Option<OpaqueInProcessBinding> {
    match binding {
        ProviderBinding::Import { library, symbol } => Some(OpaqueInProcessBinding::Import {
            library: library.clone(),
            symbol: symbol.clone(),
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
    let requirement_identity = row_requirement_identity(plan, row);
    IncompleteCause {
        provider_identity: provider_identity(plan),
        provider_plan_identity: plan.identity_fingerprint(),
        method: row.method.clone(),
        requirement_identity,
        binding,
    }
}
