//! How one root enters and leaves a slot: validation, admission, the
//! required slot closure, provider execution, progress profile installation
//! and opaque callback replacement.

pub(crate) mod opaque_callback_replacement;
pub(crate) mod progress_profile_installation;
pub(crate) mod provider_execution;
pub(crate) mod required_root_slots;
pub(crate) mod root_admission;
pub(crate) mod root_validation;
