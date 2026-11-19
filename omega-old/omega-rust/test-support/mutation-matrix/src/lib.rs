/*
We provide the shared one-field substitution test machinery for custody-mutation
matrices (`custody_field_inventory!`, `MutationOutcome`,
`OneFieldSubstitutionMatrix`, and `run_one_field_substitution_matrix`). A record
family declares its canonical field inventory once through
`custody_field_inventory!`, which generates the `*FieldForTest` enum and its
exhaustive `INVENTORY` slice.
*/

/*
Every custody-bearing record family must prove that substituting any single
field either triggers a named checker rejection (`MutationOutcome::ExactError`)
or causes honest replay to rebuild a distinct custody record
(`MutationOutcome::RebuiltCustodyDiffers`). Driving that matrix from a single
inventory macro and runner prevents handwritten field lists from drifting when a
record gains a new field.
*/

/*
Each matrix test supplies a fresh honest builder (`honest`), an authentic
foreign record (`donor`) whose custody differs from the honest record, a
single-field substitution hook (`substitute`), and the family's independent
checker (`check`), optionally followed by a `joined_replay` assertion.
*/

#[cfg(any(test, feature = "test-support"))]
#[macro_export]
macro_rules! custody_field_inventory {
    (
        $(#[$enum_meta:meta])*
        $vis:vis enum $inventory:ident {
            $($variant:ident),+ $(,)?
        }
    ) => {
        $(#[$enum_meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $inventory {
            $($variant),+
        }

        impl $inventory {
            pub const INVENTORY: &'static [Self] = &[$($inventory::$variant),+];
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOutcome<E> {
    ExactError(E),         // The checker rejects the mutated record with this exact error.
    RebuiltCustodyDiffers, // The checker replays honestly and rebuilds custody that differs from the substituted record.
}

pub struct OneFieldSubstitutionMatrix<'a, R, F, C, E> {
    pub family: &'a str,                       // Record family name for diagnostics.
    pub fields: &'a [F], // Canonical field inventory covering every leg of the matrix.
    pub honest: &'a dyn Fn() -> R, // Builds a fresh honestly produced record of the family.
    pub donor: R,        // Authentic foreign record whose retained custody differs from `honest`.
    pub custody: &'a dyn Fn(&R) -> C, // Extracts the retained custody view a substitution must move.
    pub substitute: &'a dyn Fn(&mut R, F, &R), // Family honest-recomputation hook.
    pub check: &'a dyn Fn(&R) -> Result<C, E>, // Family independent checker.
    pub outcome: &'a dyn Fn(F) -> MutationOutcome<E>, // Expected checker outcome for each declared field.
    pub joined_replay: Option<&'a dyn Fn(&R, F)>, // Optional per-leg assertion appended after standard rejection checks.
}

pub fn run_one_field_substitution_matrix<R, F, C, E>(
    matrix: &OneFieldSubstitutionMatrix<'_, R, F, C, E>,
) where
    F: Copy + core::fmt::Debug,
    C: PartialEq + core::fmt::Debug,
    E: PartialEq + core::fmt::Debug,
{
    let honest_custody = (matrix.custody)(&(matrix.honest)());
    assert_ne!(
        (matrix.custody)(&matrix.donor),
        honest_custody,
        "{}: the foreign donor must carry authentic custody distinct from the honest record",
        matrix.family,
    );
    for &field in matrix.fields {
        let mut substituted = (matrix.honest)();
        (matrix.substitute)(&mut substituted, field, &matrix.donor);
        let substituted_custody = (matrix.custody)(&substituted);
        assert_ne!(
            substituted_custody, honest_custody,
            "{}: substituting {field:?} must change the retained record",
            matrix.family,
        );
        match (matrix.outcome)(field) {
            MutationOutcome::ExactError(expected) => assert_eq!(
                (matrix.check)(&substituted),
                Err(expected),
                "{}: the independent checker must reject {field:?} with its named error",
                matrix.family,
            ),
            MutationOutcome::RebuiltCustodyDiffers => {
                let rebuilt = (matrix.check)(&substituted).unwrap_or_else(|error| {
                    panic!(
                        "{}: honest replay must still succeed after substituting {field:?}: {error:?}",
                        matrix.family,
                    )
                });
                assert_ne!(
                    rebuilt, substituted_custody,
                    "{}: independent replay must expose the substituted {field:?}",
                    matrix.family,
                );
            }
        }
        if let Some(joined_replay) = matrix.joined_replay {
            joined_replay(&substituted, field);
        }
    }
}
