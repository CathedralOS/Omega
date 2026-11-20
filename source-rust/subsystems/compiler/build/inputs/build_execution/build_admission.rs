// Admit the selected source-owned Build entry and its original Build value
// against the actual accepted invocation module. Host effects remain bounded
// by structurally emitted compiler-owned Build protocol companions.

use super::{BuildMachineExecutionMode, BuildMachineFilesystemScope};
use super::build_value::BuildValueAdmission;
use diagnostics::Diagnostic;
use filesystem_authority::FilesystemMetadataLayout;
use build_time_values::BuildTimeValue;
use symbols::SymbolHandle;

pub struct SelectedBuildMachine {
    receiver: super::build_receiver::BuildReceiver,
    returned_scalars: Box<[source_compiler::ReturnedScalarFact]>,
    pub symbol: SymbolHandle,
    pub name: String,
    pub normalized_callable_identity: String,
    pub value_admission: BuildValueAdmission,
    pub filesystem_reachable: bool,
    pub execution_mode: BuildMachineExecutionMode,
    pub initial_build: BuildTimeValue,
}

impl SelectedBuildMachine {
    pub fn receiver_for<'module>(
        &self,
        module: &'module terminal_psi::TerminalModule,
    ) -> Result<&'module terminal_psi::StructuralParameterDeclaration, String> {
        self.entry_storage_for(module).map(|parameters| &parameters[0])
    }

    pub fn entry_storage_for<'module>(
        &self,
        module: &'module terminal_psi::TerminalModule,
    ) -> Result<&'module [terminal_psi::StructuralParameterDeclaration], String> {
        self.receiver.parameters_for(module)
    }

    /// Only these admitted frame homes can carry a toolchain dependency-registration marker.
    pub fn registration_frame_positions(&self) -> impl Iterator<Item = u32> + '_ {
        self.receiver.registration_frame_positions()
    }

    pub fn returned_scalars(&self) -> &[source_compiler::ReturnedScalarFact] {
        &self.returned_scalars
    }

    pub fn has_same_frame(&self, previous: &Self) -> bool {
        self.receiver.has_same_frame(&previous.receiver)
    }
}

pub fn select_build_machine(
    typed: &source_compiler::source_state::TypedTrees,
    entry: crate::source_assembly::BuildSourceSelection,
) -> Result<&source_compiler::source_state::machine::Machine, Vec<Diagnostic>> {
    let mut machines = typed.machines().iter().filter(|machine| {
        typed.symbols.symbol_source_is_selected(machine.symbol)
            && typed.symbols.symbol_source_span(machine.symbol) == Some(entry.entry())
    });
    match (machines.next(), machines.next()) {
        (Some(machine), None) => Ok(machine),
        _ => Err(vec![
            Diagnostic::error("the selected Build declaration has no unique typed machine")
                .with_source_span(entry.entry()),
        ]),
    }
}

pub fn admit_build_machine(
    prepared: &source_compiler::AcceptedTerminalSource,
    machine_symbol: SymbolHandle,
    stage: &source_compiler::BuildStage,
    filesystem_scope: &mut BuildMachineFilesystemScope,
) -> Result<SelectedBuildMachine, Vec<Diagnostic>> {
    let typed = prepared.source();
    let elaboration = &prepared.closure().elaboration();
    if !prepared.is_invocation()
        || prepared
            .entry()
            .and_then(|entry| elaboration.machines.get(entry))
            != Some(&machine_symbol)
    {
        return Err(vec![Diagnostic::error(
            "Build entry differs from its accepted invocation root",
        )]);
    }
    let machine = typed.machine(machine_symbol).ok_or_else(|| {
        vec![Diagnostic::error(
            "prepared Build program lost its selected entry",
        )]
    })?;
    let machine_name = machine.name.as_str().to_owned();
    let receiver = super::build_receiver::BuildReceiver::admit(prepared, machine, stage)
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
    let normalized_callable_identity =
        typed
            .machine_callable_identity_text(machine)
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "selected build machine has no canonical callable identity",
                )]
            })?;
    let value_admission = BuildValueAdmission::admit(typed)?;

    // Host authority is bounded by the boundary calls of the accepted
    // module: each must be an operation of a compiler-owned Build protocol,
    // which is exactly the calls publication gives a host row.
    let protocols = prepared.build_protocol_use()?;
    if protocols.reaches_outside {
        return Err(vec![Diagnostic::error(format!(
            "`{machine_name}` reaches a boundary outside the compiler-owned Build protocols"
        ))]);
    }
    let filesystem_reachable = protocols.filesystem;

    let zero_build = value_admission.initial_value(typed, machine, filesystem_scope);

    let execution_mode = if !filesystem_reachable {
        BuildMachineExecutionMode::Pure
    } else {
        let filesystem = filesystem_scope.establish_filesystem_access()?;
        BuildMachineExecutionMode::Granted {
            filesystem,
            filesystem_metadata_layout: FilesystemMetadataLayout::default(),
        }
    };
    Ok(SelectedBuildMachine {
        receiver,
        returned_scalars: prepared.closure().returned_scalars().into(),
        symbol: machine_symbol,
        name: machine_name,
        normalized_callable_identity,
        value_admission,
        filesystem_reachable,
        execution_mode,
        initial_build: zero_build,
    })
}
