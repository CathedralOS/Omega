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
}

/// Evidence for how the executable implementation is supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImplementationEvidence {
    CheckedBody { machine: String },
    CompilerKnown { target: String, intrinsic: String },
}

/// Containment guarantees remain independent; one receipt cannot imply the
/// others merely because it names the same mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    },
    Incomplete {
        scope: ExecutionScope,
        causes: Vec<IncompleteCause>,
    },
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

pub(crate) fn derive_static_manifest(
    plans: &[ProviderPlan],
    selected_provider_closure_identity: u64,
) -> ExecutableTcbManifest {
    let scope = ExecutionScope::CallerAddressSpace;
    let mut known_entries = Vec::new();
    let mut causes = Vec::new();

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
                ProviderBinding::Import { library, symbol } => {
                    causes.push(incomplete_cause(
                        plan,
                        row,
                        OpaqueInProcessBinding::Import {
                            library: library.clone(),
                            symbol: symbol.clone(),
                        },
                    ));
                    None
                }
                ProviderBinding::VtableSlot { index } => {
                    causes.push(incomplete_cause(
                        plan,
                        row,
                        OpaqueInProcessBinding::VtableSlot { index: *index },
                    ));
                    None
                }
                ProviderBinding::VtableField { table, field } => {
                    causes.push(incomplete_cause(
                        plan,
                        row,
                        OpaqueInProcessBinding::VtableField {
                            table: table.clone(),
                            field: field.clone(),
                        },
                    ));
                    None
                }
                ProviderBinding::TableFunction { table, field } => {
                    causes.push(incomplete_cause(
                        plan,
                        row,
                        OpaqueInProcessBinding::TableFunction {
                            table: table.clone(),
                            field: field.clone(),
                        },
                    ));
                    None
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
        }
    } else {
        ScopeCompleteness::Incomplete { scope, causes }
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

fn incomplete_cause(
    plan: &ProviderPlan,
    row: &crate::provider_plan::ProviderPlanRow,
    binding: OpaqueInProcessBinding,
) -> IncompleteCause {
    let requirement_identity = if row.requirement_identity.is_empty() {
        plan.schema
            .methods
            .iter()
            .find(|method| method.name == row.method)
            .map(|method| method.requirement_identity.clone())
            .unwrap_or_default()
    } else {
        row.requirement_identity.clone()
    };
    IncompleteCause {
        provider_identity: provider_identity(plan),
        provider_plan_identity: plan.identity_fingerprint(),
        method: row.method.clone(),
        requirement_identity,
        binding,
    }
}
