use diagnostics::Diagnostic;
use language_core::inline_assembly::AsmAuthorityRequirement;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;

/// The privileged-service authority classes an evaluated build supplies to the
/// asm authority discharge. Each catalog contract carries its own
/// `AsmAuthorityRequirement` and this admission independently answers for the
/// exact class, per
/// wiki/spec/build/permissions.md#privileged-services: a machine's `reaches`
/// row names the service it may reach; this evidence states which privileged
/// classes the produced image may exercise at all.
///
/// `Build.freestanding` is the machine-owner supply: the freestanding
/// selection is the boot-root machine owner, and the machine owner can
/// self-grant the mediated classes (port permission maps, interrupt-table
/// publication) alongside machine control itself. A hosted build begins with
/// no class but may grant each mediated class independently through
/// `Build.privileged_services` flags — port permission without
/// interrupt-table control, and so on. Machine-owner authority itself has no
/// granular grant and stays `freestanding`-only. Consumer-defined
/// publication authority stays receiver-side per the permissions spec and
/// never enters this input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsmAuthorityAdmission {
    machine_owner: bool,
    port_io: bool,
    idt_control: bool,
}

impl AsmAuthorityAdmission {
    /// A hosted image supplies no privileged-service authority.
    pub const HOSTED: Self = Self {
        machine_owner: false,
        port_io: false,
        idt_control: false,
    };

    /// The freestanding machine-owner selection admits every authority class
    /// the catalog defines: owning the machine includes its mediated
    /// capabilities.
    pub const MACHINE_OWNER: Self = Self {
        machine_owner: true,
        port_io: true,
        idt_control: true,
    };

    /// The admission evidence an evaluated `Build.freestanding` selection
    /// supplies: full machine-owner admission or none.
    pub const fn from_freestanding(freestanding: bool) -> Self {
        if freestanding {
            Self::MACHINE_OWNER
        } else {
            Self::HOSTED
        }
    }

    /// Widen by each authored `Build.privileged_services` flag. Grants are
    /// independent: `port_io` admits port I/O only and `interrupt_table`
    /// admits interrupt-table publication only; neither claims machine-owner
    /// authority, which stays `freestanding`-only and already covers both
    /// mediated classes when set.
    pub const fn with_grants(self, port_io: bool, interrupt_table: bool) -> Self {
        Self {
            machine_owner: self.machine_owner,
            port_io: self.port_io || port_io,
            idt_control: self.idt_control || interrupt_table,
        }
    }

    /// Whether this admission supplies the exact authority class one
    /// instruction contract requires. `None`-authority instructions admit
    /// unconditionally and never reach the gate's rejection path.
    pub const fn admits(&self, requirement: AsmAuthorityRequirement) -> bool {
        match requirement {
            AsmAuthorityRequirement::None => true,
            AsmAuthorityRequirement::MachineOwner => self.machine_owner,
            AsmAuthorityRequirement::PortIo => self.port_io,
            AsmAuthorityRequirement::IdtControl => self.idt_control,
        }
    }

    fn description(requirement: AsmAuthorityRequirement) -> &'static str {
        match requirement {
            AsmAuthorityRequirement::None => "no authority",
            AsmAuthorityRequirement::MachineOwner => "machine-owner authority",
            AsmAuthorityRequirement::PortIo => "port-I/O authority",
            AsmAuthorityRequirement::IdtControl => "interrupt-table publication authority",
        }
    }
}

/// The authored supply the diagnostic names for the missing class. A
/// freestanding boundary root always suffices; each mediated class also
/// accepts its exact `Build.privileged_services` flag, while machine-owner
/// authority has no granular grant.
fn authored_admission_fix(requirement: AsmAuthorityRequirement) -> &'static str {
    match requirement {
        AsmAuthorityRequirement::PortIo => {
            "Set `b.freestanding = true` or grant exactly this class with \
             `b.privileged_services.port_io = true` in build.omg, \
             or remove the asm block"
        }
        AsmAuthorityRequirement::IdtControl => {
            "Set `b.freestanding = true` or grant exactly this class with \
             `b.privileged_services.interrupt_table = true` in build.omg, \
             or remove the asm block"
        }
        AsmAuthorityRequirement::None | AsmAuthorityRequirement::MachineOwner => {
            "Set `b.freestanding = true` in build.omg, or remove the asm block"
        }
    }
}

/// Per-capability admission gate for authority-bearing assembly intrinsics.
/// Each instruction's contract authority class is checked against the build's
/// supplied admission evidence; instructions with no authority requirement
/// pass unconditionally. A build supplying no class rejects each
/// authority-bearing emission with its exact missing class.
pub fn validate_asm_discharge(
    program: &TypedTrees,
    admission: AsmAuthorityAdmission,
) -> Result<(), Vec<Diagnostic>> {
    if admission == AsmAuthorityAdmission::MACHINE_OWNER {
        return Ok(());
    }

    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some((instruction, _, required_authority)) =
                    statement_asm_intrinsic(program, statement)
                else {
                    continue;
                };
                if admission.admits(required_authority) {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` uses asm instruction `{}`, which requires a FREESTANDING \
                     boundary root (the contract requires {}, and this build's \
                     privileged-service admission does not supply it). {}",
                    machine.name,
                    instruction,
                    AsmAuthorityAdmission::description(required_authority),
                    authored_admission_fix(required_authority)
                )));
            }
        }
    }

    crate::program_validation::finish_diagnostics(diagnostics)
}

#[cfg(test)]
mod tests;

/// Direct assembly emission declares its canonical service at the instruction
/// owner. Ordinary checked callers propagate that reach through the shared
/// fixed point; propagation does not discharge instruction authority.
pub(super) fn validate_asm_intrinsic_declarations(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let declared_services = program
            .service_reach_rows
            .services(machine.service_reach_row);
        let mut direct_asm_services = Vec::new();

        // Direct emission sites retain their authored service obligation.
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some((instruction, service_name, _)) =
                    statement_asm_intrinsic(program, statement)
                else {
                    continue;
                };
                let Some(service_name) = service_name else {
                    continue;
                };
                if !direct_asm_services.contains(&service_name) {
                    direct_asm_services.push(service_name);
                }
                let Some(service) = program.service_reaches.id_for_name(service_name) else {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` uses asm instruction `{}`, which reaches canonical \
                         service `{service_name}`, but that service identity is unavailable: \
                         add `use omega::language::core::assembly;`",
                        machine.name, instruction
                    )));
                    continue;
                };
                if !declared_services.contains(&service) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` uses asm instruction `{}` but does not declare its \
                         service contract: add `reaches {service_name}` (every asm \
                         instruction's service reach must be declared where it is emitted)",
                        machine.name, instruction
                    )));
                }
            }
        }

        // The normalized summary is also an invariant check: a resolved asm
        // service must appear in the ordinary canonical reach fixed point.
        let Some(machine_services) = service_reaches.for_machine(machine.symbol) else {
            continue;
        };
        for service_name in direct_asm_services {
            let Some(service) = program.service_reaches.id_for_name(service_name) else {
                continue;
            };
            debug_assert!(
                service_reaches
                    .services(machine_services.inferred_transitive)
                    .contains(&service)
            );
        }
    }
}

/// The asm intrinsic a statement carries, as (instruction label, contract
/// service identity, required authority): a statement call on an `asm#...` target
/// (`asm { hlt }`, `asm { out .. }`) or an assignment whose value is the
/// `asm#port_in` call (`asm { in dest, port }`). The parser's desugar emits
/// exactly these two shapes and the names are unnameable from source.
fn statement_asm_intrinsic(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Option<(
    &'static str,
    Option<&'static str>,
    language_core::inline_assembly::AsmAuthorityRequirement,
)> {
    let target = match statement {
        StatementNode::Call(call) => call.target.as_str().to_owned(),
        StatementNode::Assignment(assignment) => {
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(assignment.value)
            else {
                return None;
            };
            call.target.as_str().to_owned()
        }
        _ => return None,
    };

    let function = symbols::BuiltinFunction::asm_intrinsics()
        .into_iter()
        .find(|function| function.name() == target)?;
    let service_name = function.asm_intrinsic_service_name();
    // Label the diagnostic with the SOURCE mnemonic, not the internal name.
    let instruction = match function {
        symbols::BuiltinFunction::AsmHlt => "hlt",
        symbols::BuiltinFunction::AsmPortOut => "out",
        symbols::BuiltinFunction::AsmPortIn => "in",
        symbols::BuiltinFunction::AsmLoadFence => "lfence",
        symbols::BuiltinFunction::AsmStoreFence => "sfence",
        symbols::BuiltinFunction::AsmFullFence => "mfence",
        symbols::BuiltinFunction::AsmDisableInterrupts => "cli",
        symbols::BuiltinFunction::AsmEnableInterrupts => "sti",
        symbols::BuiltinFunction::AsmSnapshotFlags => "pushfq",
        symbols::BuiltinFunction::AsmRestoreFlags => "popfq",
        symbols::BuiltinFunction::AsmReadMsr => "rdmsr",
        symbols::BuiltinFunction::AsmWriteMsr => "wrmsr",
        symbols::BuiltinFunction::AsmReadCr0 => "read_cr0",
        symbols::BuiltinFunction::AsmReadCr2 => "read_cr2",
        symbols::BuiltinFunction::AsmReadCr3 => "read_cr3",
        symbols::BuiltinFunction::AsmReadCr4 => "read_cr4",
        symbols::BuiltinFunction::AsmWriteCr0 => "write_cr0",
        symbols::BuiltinFunction::AsmWriteCr3 => "write_cr3",
        symbols::BuiltinFunction::AsmWriteCr4 => "write_cr4",
        symbols::BuiltinFunction::AsmWriteBackInvalidate => "wbinvd",
        symbols::BuiltinFunction::AsmInvalidate => "invd",
        symbols::BuiltinFunction::AsmWriteBackNoInvalidate => "wbnoinvd",
        _ => return None,
    };
    let language_core::inline_assembly::AsmCatalogEntry::Contract(contract) =
        language_core::inline_assembly::asm_catalog_entry(instruction)?
    else {
        return None;
    };
    Some((instruction, service_name, contract.required_authority))
}
