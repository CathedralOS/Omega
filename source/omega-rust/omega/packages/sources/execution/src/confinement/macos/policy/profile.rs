use super::MACOS_NULL_DEVICE;
use crate::model::ResolverExecutionPhase;

pub(super) fn encode(phase: ResolverExecutionPhase) -> String {
    debug_assert!(!phase.permits_network());

    let mut encoded = "(version 1) (deny default) ".to_owned();
    encoded.push_str("(allow signal) (allow file-read*) ");
    encoded.push_str(&format!(
        "(allow file-test-existence file-write-data (literal \"{MACOS_NULL_DEVICE}\")) \
         (allow process-exec (literal (param \"EXECUTABLE_0\")))"
    ));
    if phase.requires_mutable_root() {
        encoded.push_str(" (allow file-write* (subpath (param \"MUTABLE_ROOT\")))");
    }
    encoded
}
