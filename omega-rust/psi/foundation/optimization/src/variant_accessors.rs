//! Shared result-variant accessor expansions for operation result enums.

/// Expands the `structural` result-variant accessor on an operation result
/// enum: `Self::Structural(result) => Some(result)` while every other variant
/// maps to `None`. The rejected arms are passed verbatim so the match keeps
/// naming the variants it refuses rather than catching all.
#[macro_export]
macro_rules! structural_variant_accessor {
    ($result:ty; $($rest:tt)*) => {
        pub const fn structural(&self) -> Option<&$result> {
            match self {
                Self::Structural(result) => Some(result),
                $($rest)* => None,
            }
        }
    };
}
