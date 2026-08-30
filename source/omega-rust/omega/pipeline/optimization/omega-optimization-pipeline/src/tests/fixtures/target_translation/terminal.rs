use super::common::scalar_terminal_artifact;
use super::*;

pub(crate) fn scalar_crash_artifact(
    scalar_type: ScalarType,
    cause: CrashCause,
) -> (Vec<u8>, Vec<u8>) {
    scalar_terminal_artifact(scalar_type, Vec::new(), None, Some(cause), None)
}
