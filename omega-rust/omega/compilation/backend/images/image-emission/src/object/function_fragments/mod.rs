//! Project admitted current fragments into the ordinary image-publisher object.
//! No assigned body, legacy machine plan, or fabricated scalar result home is used.

mod attribution;
mod byte_literals;
mod image_addresses;
pub(crate) mod image_residence;
pub(crate) mod implementation_tables;
mod imports;
mod mixed_scalar_abi;
pub(crate) mod native_code_layouts;
mod port_effects;
mod private_addresses;
mod production;
pub(crate) mod replay;
pub(crate) mod reporting;
mod source;
mod structural;
pub(crate) mod validation;

pub(crate) use production::ProjectedObject;
pub use production::{
    FunctionFragmentObjectArtifactRequest, build_function_fragment_object_artifact,
};
pub(crate) use validation::validate_function_fragment_object_artifact;

#[derive(Debug)]
pub enum FunctionFragmentObjectArtifactError {
    Source(object_file::RelocationFreeObjectContainerError),
    Unsupported(&'static str),
    UnaccountedOperation {
        machine: semantic_vocabulary::MachineId,
        operation_ordinal: usize,
        operation_description: String,
    },
    Mismatch(&'static str),
    InstalledRelativeBranches {
        machine: semantic_vocabulary::MachineId,
        source: machine_emission::frame_application::FunctionFragmentFrameApplicationError,
    },
    InstalledFaultSurface {
        machine: semantic_vocabulary::MachineId,
        instruction: selected_instructions::SelectedInstructionId,
        detail: String,
    },
    InstalledMemorySource {
        machine: semantic_vocabulary::MachineId,
        operation: Option<semantic_vocabulary::OperationId>,
        reason: &'static str,
    },
    InstallationHelperReference {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        detail: String,
    },
    InstallationResultDefinition {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        detail: String,
    },
    InstallationInterval {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        detail: String,
    },
    InstallationGuard {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        detail: String,
    },
    StructuralSourceHome {
        machine: semantic_vocabulary::MachineId,
        place: semantic_vocabulary::PlaceId,
        reason: &'static str,
    },
    InstallationBacking {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        argument: Option<usize>,
        reason: &'static str,
    },
    AccessBackingGeometry {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        reason: &'static str,
    },
    PrivateFunctions(crate::ObjectError),
    ForeignCalls(crate::ObjectError),
    NativeEntryBridge(machine_emission::native_entry_bridge::NativeEntryBridgeError),
    PhysicalEntry(diagnostics::Diagnostic),
    NativeCodeLayout(machine_emission::code_layout::NativeCodeLayoutError),
    Overflow,
}

impl std::fmt::Display for FunctionFragmentObjectArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "shared object replay: {error}"),
            Self::NativeEntryBridge(error) => write!(formatter, "native entry transport: {error}"),
            Self::PhysicalEntry(error) => error.fmt(formatter),
            Self::Unsupported(reason) | Self::Mismatch(reason) => formatter.write_str(reason),
            Self::InstalledRelativeBranches { machine, source } => write!(
                formatter,
                "installed handler {machine:?} relative control: {source}",
            ),
            Self::InstalledFaultSurface {
                machine,
                instruction,
                detail,
            } => write!(
                formatter,
                "installed handler {machine:?} instruction {instruction:?} has no software-fault exclusion: {detail}"
            ),
            Self::InstalledMemorySource {
                machine,
                operation,
                reason,
            } => write!(
                formatter,
                "installed handler {machine:?} operation {operation:?} has no closed RAM source: {reason}"
            ),
            Self::InstallationHelperReference {
                machine,
                operation,
                detail,
            } => write!(
                formatter,
                "installation checked helper receives a different runtime reference at {machine:?}/{operation:?}: {detail}",
            ),
            Self::InstallationResultDefinition {
                machine,
                operation,
                detail,
            } => write!(
                formatter,
                "installation refusal result is not preserved at {machine:?}/{operation:?}: {detail}",
            ),
            Self::InstallationInterval {
                machine,
                operation,
                detail,
            } => write!(
                formatter,
                "another source operation interrupts installation validation or publication at {machine:?}/{operation:?}: {detail}",
            ),
            Self::InstallationGuard {
                machine,
                operation,
                detail,
            } => write!(
                formatter,
                "installation is missing a dominating refusal guard at {machine:?}/{operation:?}: {detail}",
            ),
            Self::StructuralSourceHome {
                machine,
                place,
                reason,
            } => write!(
                formatter,
                "structural source home at {machine:?}/{place:?}: {reason}",
            ),
            Self::InstallationBacking {
                machine,
                operation,
                argument,
                reason,
            } => write!(
                formatter,
                "installation activation backing at {machine:?}/{operation:?}, operand {argument:?}: {reason}",
            ),
            Self::AccessBackingGeometry {
                machine,
                operation,
                reason,
            } => write!(
                formatter,
                "placed backing geometry at {machine:?}/{operation:?}: {reason}",
            ),
            Self::UnaccountedOperation {
                machine,
                operation_ordinal,
                operation_description,
            } => write!(
                formatter,
                "shared function {machine:?} contains an unaccounted operation at ordinal {operation_ordinal}: {operation_description}",
            ),
            Self::PrivateFunctions(error) => {
                write!(formatter, "compiler-private function custody: {error}")
            }
            Self::ForeignCalls(error) => {
                write!(formatter, "foreign-call custody: {error}")
            }
            Self::NativeCodeLayout(error) => {
                write!(formatter, "selected native instruction geometry: {error:?}")
            }
            Self::Overflow => {
                formatter.write_str("shared object coordinate exceeds supported size")
            }
        }
    }
}
impl std::error::Error for FunctionFragmentObjectArtifactError {}

use FunctionFragmentObjectArtifactError as Error;

fn host(value: u64) -> Result<usize, Error> {
    usize::try_from(value).map_err(|_| Error::Overflow)
}

/// Refuse a symbol name another row of this object already claims. The caller
/// names the roster it is about to publish, so the refusal stays specific.
pub(crate) fn refuse_symbol_collision(
    object: &object_file::ObjectPlan,
    name: &str,
    reason: &'static str,
) -> Result<(), Error> {
    if object.layout.symbols.iter().any(|(_, s)| s.name == name) {
        return Err(Error::Mismatch(reason));
    }
    Ok(())
}
