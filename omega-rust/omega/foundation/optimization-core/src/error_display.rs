//! Shared error-display expansions for the crate's leaf error enums: the
//! rendered refusal and the `Error` impl are generated from the enum's own
//! subject or sentence, so the enum stays the only place its variants live.

/// Expands the `Display`/`Error` pair of a validation or decode error enum
/// whose rendering is its own `Debug` under an `invalid <subject>` prefix.
#[macro_export]
macro_rules! invalid_error_display {
    ($error:ty, $subject:literal) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::write!(formatter, concat!("invalid ", $subject, ": {:?}"), self)
            }
        }

        impl std::error::Error for $error {}
    };
}

/// Expands the `Display`/`Error` pair of an error enum whose rendering is one
/// fixed sentence, independent of the variant that carries it.
#[macro_export]
macro_rules! fixed_error_display {
    ($error:ty, $message:literal) => {
        impl std::fmt::Display for $error {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str($message)
            }
        }

        impl std::error::Error for $error {}
    };
}
