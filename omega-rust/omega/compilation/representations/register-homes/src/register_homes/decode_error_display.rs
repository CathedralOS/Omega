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
