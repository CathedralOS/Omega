//! Physical authority observations in the shared canonical synchronous closure.
//!
//! Provider supply, cycle admission and authority review use the same retained
//! call semantics. Physical mechanism classification remains native-owned and
//! reads the corresponding lowered operations; it cannot invent call edges.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{AbstractFunction, AbstractOperation};
use effects::{
    CheckedPhysicalTerminalMechanismIdentity, TerminalAuthorityClosureLeafOrigin,
    TerminalMechanismIdentity,
};
use semantic_vocabulary::{BoundaryMachineId, IdSet, MachineId, OperationId};
use terminal_psi::RegisterTargetApplicability;

use super::operations::{PhysicalAuthorityUse, physical_authority_use};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReachableCheckedOperation {
    pub(crate) origin: TerminalAuthorityClosureLeafOrigin,
    pub(crate) service: semantic_vocabulary::ServiceId,
    pub(crate) mechanism: TerminalMechanismIdentity,
}

pub(crate) struct ReachableAuthorityEdges {
    pub(crate) boundaries: BTreeSet<BoundaryMachineId>,
    pub(crate) checked_operations: Vec<ReachableCheckedOperation>,
}

/// The shared canonical call graph plus this receiver's native observations.
pub(crate) struct AuthorityGraph<'a> {
    input: &'a crate::AdmissionSource<'a>,
    functions: BTreeMap<MachineId, &'a AbstractFunction>,
    calls: terminal_installation::SynchronousCallGraph<'a>,
    target_profile: target::TargetProfile,
    installed_scopes: Option<Vec<&'a crate::AdmittedInstalledInterruptScope>>,
}

impl<'a> AuthorityGraph<'a> {
    pub(crate) fn new(
        input: &'a crate::AdmissionSource<'a>,
        target_profile: target::TargetProfile,
    ) -> Result<Self, String> {
        let plan = input.plan();
        let mut functions = BTreeMap::new();
        for function in &plan.functions {
            if functions.insert(function.machine, function).is_some() {
                return Err(format!(
                    "abstract operation plan repeats machine {:?}",
                    function.machine
                ));
            }
        }
        if !functions.contains_key(&plan.entry) {
            return Err(format!(
                "abstract operation plan entry {:?} has no function",
                plan.entry
            ));
        }
        let bridge = input
            .selected_program_entry_call()
            .map(|entry| (entry.boundary(), entry.entry()));
        let calls = terminal_installation::SynchronousCallGraph::new(
            input.module(),
            input.target_machine_selections(),
            bridge,
        )?;
        Ok(Self {
            input,
            functions,
            calls,
            target_profile,
            installed_scopes: None,
        })
    }

    /// Add only the finite helper calls owned by an admitted installation.
    /// Retaining a handler address or a Scope row does not make it a call.
    pub(crate) fn with_installed_interrupts(
        mut self,
        installed: &'a crate::AdmittedInstalledInterruptCohort,
    ) -> Result<Self, String> {
        if installed.psi() != self.input.plan().psi {
            return Err("authority review installation belongs to another Terminal program".into());
        }
        let mut scopes: Vec<_> = installed.scopes().iter().collect();
        scopes.sort_unstable_by_key(|scope| scope.operation());
        if scopes
            .windows(2)
            .any(|pair| pair[0].operation() == pair[1].operation())
        {
            return Err("authority review repeats an installed Scope operation".into());
        }
        self.installed_scopes = Some(scopes);
        Ok(self)
    }

    fn installed_scope(
        &self,
        operation: OperationId,
    ) -> Option<&crate::AdmittedInstalledInterruptScope> {
        let scopes = self.installed_scopes.as_ref()?;
        let index = scopes
            .binary_search_by_key(&operation, |scope| scope.operation())
            .ok()?;
        Some(scopes[index])
    }

    pub(crate) fn reachable_authority_edges(
        &self,
        entry: MachineId,
    ) -> Result<ReachableAuthorityEdges, String> {
        let mut pending = vec![entry];
        let mut visited = IdSet::default();
        let mut boundaries = BTreeSet::new();
        let mut checked_operations = BTreeMap::new();
        while let Some(root) = pending.pop() {
            let reached = self.calls.from_root(root)?;
            boundaries.extend(reached.boundaries);
            for terminal_machine in reached.machines {
                let machine = terminal_machine.id;
                if !visited.insert(machine) {
                    continue;
                }
                let function = self.functions.get(&machine).ok_or_else(|| {
                    format!("reachable internal call names absent machine {machine:?}")
                })?;
                // Current rewrites preserve reachable calls and their machine IDs.
                // Any future inlining/call-erasing transform must preserve catalog
                // applicability before this selected executable closure is walked.
                let requirement = terminal_machine.assembly_target_requirement;
                if !requirement.accepts_target_name(self.target_profile.target_name()) {
                    // `MachineId` is a table coordinate. The retained source
                    // declaration is what a reader can act on, so name it and say
                    // which catalog architecture the realized target would have to
                    // provide; a bare coordinate sent the next reader back to the
                    // compiler to find out which body demanded what.
                    let declared = terminal_machine
                        .declaration
                        .as_ref()
                        .and_then(|declaration| {
                            self.input
                                .module()
                                .identities
                                .text(declaration.machine.identity())
                        })
                        .map_or_else(|| format!("{machine:?}"), |name| format!("`{name}`"));
                    let architecture = assembly_catalog_architecture(requirement);
                    return Err(format!(
                        "reachable machine {declared} carries checked assembly whose catalog requires {architecture}, which the realized target `{}` does not provide; scope that assembly to a target the catalog admits for {architecture}, or build a target that provides it",
                        self.target_profile.target_name(),
                    ));
                }
                for operation in &function.operations {
                    if self.installed_scopes.is_some()
                        && let AbstractOperation::BoundaryCall {
                            boundary,
                            psi_operation,
                            ..
                        } = operation
                        && self
                            .input
                            .module()
                            .boundary_machine(*boundary)
                            .and_then(|row| row.declaration.as_ref())
                            .and_then(|row| row.execution.static_interrupt_operation())
                            == Some(
                                language_core::static_interrupts::StaticInterruptOperation::Install,
                            )
                    {
                        let scope = self
                            .installed_scope(*psi_operation)
                            .filter(|scope| scope.boundary() == *boundary)
                            .ok_or_else(|| {
                                format!(
                                    "installation {psi_operation:?} has no exact admitted authority inputs"
                                )
                            })?;
                        let origin = TerminalAuthorityClosureLeafOrigin::CheckedIntrinsic {
                            machine,
                            operation: *psi_operation,
                        };
                        let mechanism =
                            effects::CompilerIntrinsicExecutionIdentity::StaticInterrupt(
                                language_core::static_interrupts::StaticInterruptOperation::Install,
                            )
                            .into();
                        record_checked_operation(
                            &mut checked_operations,
                            ReachableCheckedOperation {
                                origin,
                                service: scope.service(),
                                mechanism,
                            },
                        )?;
                        // These ordinary selected bodies execute only when this
                        // Scope executes. Arrival handlers are reviewed separately.
                        pending.extend([scope.validator(), scope.work()]);
                    }
                    match physical_authority_use(operation) {
                        PhysicalAuthorityUse::None => {}
                        PhysicalAuthorityUse::CheckedPortWrite {
                            service,
                            port,
                            operation,
                        } => {
                            // Psi's `validate_port_write` already proved the write's
                            // service sits inside the machine's published ceiling;
                            // re-asking it here would replay the same fact.
                            if self.target_profile.native_target().architecture
                                != target::Architecture::X86_64
                            {
                                return Err(format!(
                                    "reachable machine {machine:?} uses x86 PortWrite on non-x86 target profile `{}`",
                                    self.target_profile.target_name(),
                                ));
                            }
                            let mechanism: TerminalMechanismIdentity =
                                CheckedPhysicalTerminalMechanismIdentity::port_write(
                                    self.target_profile,
                                    port,
                                )
                                .into();
                            let origin = TerminalAuthorityClosureLeafOrigin::CheckedPhysical {
                                machine,
                                operation,
                            };
                            record_checked_operation(
                                &mut checked_operations,
                                ReachableCheckedOperation {
                                    origin,
                                    service,
                                    mechanism,
                                },
                            )?;
                        }
                        PhysicalAuthorityUse::CheckedRegister {
                            service,
                            register_operation,
                            operation,
                        } => {
                            if !matches!(
                                (
                                    register_operation.target_applicability(),
                                    self.target_profile.native_target().architecture
                                ),
                                (
                                    RegisterTargetApplicability::Aarch64,
                                    target::Architecture::Aarch64
                                ) | (
                                    RegisterTargetApplicability::X86_64,
                                    target::Architecture::X86_64
                                )
                            ) {
                                let architecture = assembly_catalog_architecture(
                                    register_operation.target_applicability(),
                                );
                                return Err(format!(
                                    "reachable machine {machine:?} uses {architecture} register access outside its target profile `{}`",
                                    self.target_profile.target_name(),
                                ));
                            }
                            let intrinsic = effects::CompilerIntrinsicExecutionIdentity::checked_register(register_operation)
                            .ok_or_else(|| format!("register access {operation:?} has no admitted compiler catalog identity"))?;
                            if service.is_some() != register_operation.requires_service_authority()
                            {
                                return Err(format!(
                                    "register access {operation:?} disagrees with catalog service authority"
                                ));
                            }
                            let Some(service) = service else {
                                // Exact no-authority catalog observation: target and identity
                                // were checked above; no service-authority leaf is invented.
                                continue;
                            };
                            let origin = TerminalAuthorityClosureLeafOrigin::CheckedIntrinsic {
                                machine,
                                operation,
                            };
                            let mechanism = intrinsic.into();
                            record_checked_operation(
                                &mut checked_operations,
                                ReachableCheckedOperation {
                                    origin,
                                    service,
                                    mechanism,
                                },
                            )?;
                        }
                    }
                }
            }
        }
        Ok(ReachableAuthorityEdges {
            boundaries,
            checked_operations: checked_operations.into_values().collect(),
        })
    }
}

/// The catalog architecture a retained assembly demand names.
fn assembly_catalog_architecture(requirement: RegisterTargetApplicability) -> &'static str {
    match requirement {
        RegisterTargetApplicability::X86_64 => "x86_64",
        RegisterTargetApplicability::Aarch64 => "aarch64",
        RegisterTargetApplicability::Any => "any",
    }
}

fn record_checked_operation(
    operations: &mut BTreeMap<
        (MachineId, semantic_vocabulary::OperationId),
        ReachableCheckedOperation,
    >,
    instruction: ReachableCheckedOperation,
) -> Result<(), String> {
    let site = instruction
        .origin
        .checked_site()
        .ok_or_else(|| "checked operation has no machine/operation custody".to_owned())?;
    if let Some(previous) = operations.get(&site) {
        if previous != &instruction {
            return Err(format!(
                "checked operation {site:?} claims distinct mechanisms or services"
            ));
        }
    } else {
        operations.insert(site, instruction);
    }
    Ok(())
}
