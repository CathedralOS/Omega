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
mod production;
pub(crate) mod replay;
pub(crate) mod reporting;
mod source;
mod structural;
pub(crate) mod validation;

pub use production::{
    FunctionFragmentObjectArtifactRequest, build_function_fragment_object_artifact,
};
pub(crate) use validation::validate_function_fragment_object_artifact;

#[derive(Debug)]
pub enum FunctionFragmentObjectArtifactError {
    LayoutObservation {
        machine: semantic_vocabulary::MachineId,
        operation: semantic_vocabulary::OperationId,
        reason: terminal_semantics::LayoutObservationError,
    },
    Source(object_file::RelocationFreeObjectContainerError),
    Unsupported(&'static str),
    UnaccountedOperation {
        machine: semantic_vocabulary::MachineId,
        operation_ordinal: usize,
        operation_description: String,
    },
    Mismatch(&'static str),
    PlacedBackingGeometry {
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
            Self::LayoutObservation {
                machine,
                operation,
                reason,
            } => write!(
                formatter,
                "layout observation at {machine:?}/{operation:?}: {reason}"
            ),
            Self::Source(error) => write!(formatter, "shared object replay: {error}"),
            Self::NativeEntryBridge(error) => write!(formatter, "native entry transport: {error}"),
            Self::PhysicalEntry(error) => error.fmt(formatter),
            Self::Unsupported(reason) | Self::Mismatch(reason) => formatter.write_str(reason),
            Self::PlacedBackingGeometry {
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
