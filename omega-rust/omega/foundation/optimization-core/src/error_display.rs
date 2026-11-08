// Declarative `Display` and `std::error::Error` macros (`invalid_error_display!`,
// `fixed_error_display!`) for leaf validation and decode error types.

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
