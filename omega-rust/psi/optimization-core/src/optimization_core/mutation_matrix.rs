//! Shared one-field substitution machinery for custody-mutation matrices.
//!
//! A record family declares its canonical field inventory once through
//! [`custody_field_inventory!`]; the declaration emits the `*FieldForTest`
//! vocabulary and its `INVENTORY`, so the covered field set derives from the
//! recorded receipt instead of a parallel handwritten list. The family also
//! provides an honest-recomputation hook (`corrupt_*_for_test` /
//! `substitute_*_for_test`) that mutates exactly one field and recomputes any
//! containing identity, and a named independent checker. The test then hands
//! those pieces to [`run_one_field_substitution_matrix`] — a new family adds a
//! declaration, not another several-hundred-line matrix.

/// Declare a `*FieldForTest` one-field substitution inventory.
///
/// The invocation writes the enum once; the expansion derives the enum plus
/// `INVENTORY`, the canonical covered-field set consumed by
/// [`run_one_field_substitution_matrix`](crate::run_one_field_substitution_matrix).
/// A new representable field is one more variant: the per-family
/// honest-recomputation hook's exhaustive match then demands its substitution
/// arm, and `INVENTORY` grows a matrix leg automatically.
///
/// ```text
/// optimization_core::custody_field_inventory! {
///     /// One substitutable field of `MyCustodyReceipt`; checked by
///     /// `validate_my_custody`.
///     pub enum MyCustodyFieldForTest {
///         Source,
///         Count,
///     }
/// }
/// ```
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
            /// Every declared field in declaration order — the canonical
            /// substitution inventory a matrix driver iterates. It is derived
            /// from this declaration, so adding a variant extends coverage
            /// rather than a parallel list.
            pub const INVENTORY: &'static [Self] = &[$($inventory::$variant),+];
        }
    };
}

/// How the family's independent checker must expose a one-field substitution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationOutcome<E> {
    /// The checker rejects the mutated record with this exact error — the
    /// usual shape for canonical-encoding families whose checked decode
    /// revalidates every claim.
    ExactError(E),
    /// The checker still replays honestly, and the custody it rebuilds must
    /// differ from the retained (substituted) record — the usual shape for
    /// in-memory staged receipts whose replay recomputes the honest evidence.
    RebuiltCustodyDiffers,
}

/// A record family's declared one-field substitution matrix.
///
/// `fields` is the family's canonical inventory — `FieldForTest::INVENTORY`
/// from [`custody_field_inventory!`]. `honest` builds a genuinely produced
/// record — staged records are not `Clone`, so each leg stages afresh — and
/// `donor` is an authentic foreign record of the same family whose retained
/// custody differs; the family's `substitute` hook draws donor values for
/// nested-custody legs and fixed non-identity alternates for
/// closed-vocabulary legs, then recomputes any containing identity honestly.
/// `check` is the family's named independent checker and `outcome` the
/// expected checker result per field. `joined_replay` optionally appends a
/// second assertion per leg, such as a pipeline wrapper surfacing the same
/// rejection under its own error variant.
pub struct OneFieldSubstitutionMatrix<'a, R, F, C, E> {
    /// Record family name for diagnostics.
    pub family: &'a str,
    /// The canonical field inventory — every leg the matrix must cover.
    pub fields: &'a [F],
    /// Builds a fresh honestly produced record of the family.
    pub honest: &'a dyn Fn() -> R,
    /// An authentic foreign record whose retained custody differs from what
    /// `honest` builds; it supplies donor values and proves they are real
    /// evidence.
    pub donor: R,
    /// Extracts the retained custody view a substitution must move.
    pub custody: &'a dyn Fn(&R) -> C,
    /// The family's honest-recomputation hook.
    pub substitute: &'a dyn Fn(&mut R, F, &R),
    /// The family's independent checker.
    pub check: &'a dyn Fn(&R) -> Result<C, E>,
    /// The expected checker outcome for each declared field.
    pub outcome: &'a dyn Fn(F) -> MutationOutcome<E>,
    /// Optional per-leg assertion appended after the standard rejection
    /// checks, such as joined replay surfacing the same rejection under a
    /// wrapper error variant.
    pub joined_replay: Option<&'a dyn Fn(&R, F)>,
}

/// Run a declared one-field substitution matrix.
///
/// For every field in the family's canonical inventory: substitute exactly
/// that field through the honest-recomputation hook, require the retained
/// record to change, require the independent checker to expose the
/// substitution per the leg's declared outcome, and apply the optional
/// joined-replay assertion.
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
