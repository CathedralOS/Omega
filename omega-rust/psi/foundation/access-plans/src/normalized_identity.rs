//! The zero-rejecting u64 identity shape shared by the plan's named
//! identity types. Each invocation declares the type, its admission
//! constructor, and the value accessor.

macro_rules! normalized_identity {
    ($name:ident, $label:literal $(, $field_vis:vis)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($($field_vis)? u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, AccessPlanDiagnostic> {
                if identity == 0 {
                    return Err(AccessPlanDiagnostic(
                        concat!($label, " cannot be zero").into(),
                    ));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

pub(crate) use normalized_identity;
