use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;

/// Current coarse target-class gate for authority-bearing assembly intrinsics.
/// Hosted compilation rejects them; freestanding selection passes this gate.
/// That implementation restriction is not proof of a concrete machine-control
/// capability. The required authority contract is in
/// wiki/spec/build/permissions.md#privileged-services. Instructions with no
/// authority requirement do not need this gate's freestanding condition.
pub fn validate_asm_discharge(
    program: &TypedTrees,
    freestanding: bool,
) -> Result<(), Vec<Diagnostic>> {
    if freestanding {
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
                if required_authority
                    == language_core::inline_assembly::AsmAuthorityRequirement::None
                {
                    continue;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` uses asm instruction `{}`, which requires a FREESTANDING \
                     boundary root (v0 discharge: only code that owns the machine may emit \
                     privileged instructions; a hosted build would fault at ring 3). Set \
                     `b.freestanding = true` in build.omg, or remove the asm block",
                    machine.name, instruction
                )));
            }
        }
    }

    crate::finish_diagnostics(diagnostics)
}

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
        _ => return None,
    };
    let language_core::inline_assembly::AsmCatalogEntry::Contract(contract) =
        language_core::inline_assembly::asm_catalog_entry(instruction)?
    else {
        return None;
    };
    Some((instruction, service_name, contract.required_authority))
}
