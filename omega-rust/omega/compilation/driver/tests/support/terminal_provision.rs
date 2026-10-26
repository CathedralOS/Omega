//! Shared portable-provision frame: a failed assertion inside one fixture
//! provision becomes a red fixture result carrying its message, never an
//! aborted corpus binary before its other authored controls run.

/// Run one provision body, converting a caught panic into the provision's
/// error text. `fallback` names the provision when the payload carries no
/// message.
pub(super) fn result(body: impl FnOnce(), fallback: &str) -> Result<(), String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(body)).map_err(|failure| {
        failure
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| {
                failure
                    .downcast_ref::<&str>()
                    .map(|message| (*message).to_owned())
            })
            .unwrap_or_else(|| fallback.to_owned())
    })
}
