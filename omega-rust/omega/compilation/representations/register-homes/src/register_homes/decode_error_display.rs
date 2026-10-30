//! The decode-error display surface artifact codecs declare once per enum.
//!
//! Every artifact decode error renders one description and the full variant
//! list; restating that pair per enum is how one member's surface drifts from
//! the family's. The row below is the surface: a member names its error type
//! and its own description, and nothing else.
#[macro_export]
macro_rules! decode_error_display {
    ($error:ty, $description:literal) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "{}: {self:?}", $description)
            }
        }

        impl std::error::Error for $error {}
    };
}

/// One decode error's whole declaration: the display/error surface above and
/// the two cursor adapters every framed decoder in this crate needs. A member
/// names its own error type and description; the adapter pair is the
/// family's, so it is declared here rather than restated beside each enum.
#[macro_export]
macro_rules! decode_error {
    ($error:ty, $description:literal) => {
        $crate::decode_error_display!($error, $description);

        optimization_core::truncated_from_adapter!($error);

        optimization_core::take_error_from_adapter!($error);
    };
}
