// `structural_variant_accessor!`: generates `pub const fn structural(&self)`
// on an operation result enum, returning `Some(result)` on `Self::Structural`
// and `None` on explicitly named non-structural arms.

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
