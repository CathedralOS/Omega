//! The diagnostic every access-plan operation reports.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPlanDiagnostic(pub String);

impl std::fmt::Display for AccessPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AccessPlanDiagnostic {}

/// One carrier-plus-diagnostic error: the rejected inputs return intact
/// beside the diagnostic, `diagnostic` borrows it, and `into_parts`
/// destructures the whole error in field order ending at the diagnostic.
macro_rules! access_plan_rejection {
    ($(#[$meta:meta])* $name:ident $(<$($lt:lifetime),*>)? {
        $($fv:vis $field:ident : $fty:ty),+ $(,)?
    }) => {
        $(#[$meta])*
        #[derive(Debug)]
        pub struct $name $(<$($lt),*>)? {
            $($fv $field : $fty,)+
        }

        impl $(<$($lt),*>)? $name $(<$($lt),*>)? {
            pub const fn diagnostic(&self) -> &$crate::AccessPlanDiagnostic {
                &self.diagnostic
            }

            pub fn into_parts(self) -> ($($fty,)+) {
                ($(self.$field,)+)
            }
        }
    };
}
pub(crate) use access_plan_rejection;

/// Routes one access-contract transition: `validate` inspects the carrier,
/// then `accept` builds the validated carrier from it and its evidence, or
/// `reject` returns the intact carrier with the diagnostic.
pub(crate) fn into_validated_access<Input, Evidence, Accepted, Rejected>(
    input: Input,
    validate: impl FnOnce(&Input) -> Result<Evidence, AccessPlanDiagnostic>,
    accept: impl FnOnce(Input, Evidence) -> Accepted,
    reject: impl FnOnce(Input, AccessPlanDiagnostic) -> Rejected,
) -> Result<Accepted, Rejected> {
    match validate(&input) {
        Ok(evidence) => Ok(accept(input, evidence)),
        Err(diagnostic) => Err(reject(input, diagnostic)),
    }
}
