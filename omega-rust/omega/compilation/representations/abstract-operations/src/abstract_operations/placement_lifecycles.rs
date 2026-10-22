//! Exact value operands of compiler-owned placement operations.
//!
//! Initialize/MoveOut consume the original owner; View retains its shared or
//! exclusive loan. Each borrows a completed profile, and initialization also
//! consumes its value. Keep those roles explicit
//! through native lowering. This is operand transport, not backing admission or
//! a claim that the profile is authoritative.

use terminal_psi::{StructuralAccess, StructuralArgument, TerminalPlacementLifecycleOperation};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlacementLifecycleOperands {
    Initialize {
        storage: StructuralArgument,
        value: StructuralArgument,
        profile: StructuralArgument,
    },
    MoveOut {
        storage: StructuralArgument,
        profile: StructuralArgument,
    },
    View {
        storage: StructuralArgument,
        profile: StructuralArgument,
    },
}

#[cfg(test)]
mod tests {
    use super::{
        PlacementLifecycleOperands, StructuralAccess, StructuralArgument,
        TerminalPlacementLifecycleOperation,
    };
    use semantic_vocabulary::{ObligationId, PlaceId, ValueId};
    use terminal_psi::{StructuralPathSegment, TerminalIdentityIndex};

    fn argument(place: u64, access: StructuralAccess) -> StructuralArgument {
        StructuralArgument {
            place: PlaceId::new(place).expect("place"),
            access,
            path: Vec::new(),
        }
    }

    #[test]
    fn lifecycle_operands_retain_each_original_projection_in_role_order() {
        let mut storage = argument(1, StructuralAccess::Owned);
        storage
            .path
            .push(StructuralPathSegment::Field(TerminalIdentityIndex(7)));
        let mut value = argument(2, StructuralAccess::Owned);
        value.path.push(StructuralPathSegment::RuntimeIndex {
            index: ValueId::new(3).expect("index"),
            obligation: ObligationId::new(4).expect("bound"),
        });
        let mut profile = argument(3, StructuralAccess::SharedBorrow);
        profile.path.push(StructuralPathSegment::Referent);
        let expected = vec![storage.clone(), value, profile];
        let operands = PlacementLifecycleOperands::from_arguments(
            TerminalPlacementLifecycleOperation::Initialize,
            &expected,
        )
        .expect("owned inputs and borrowed profile");
        assert_eq!(operands.arguments().cloned().collect::<Vec<_>>(), expected);
        assert_eq!(
            operands.operation(),
            TerminalPlacementLifecycleOperation::Initialize
        );
        let extracted = PlacementLifecycleOperands::from_arguments(
            TerminalPlacementLifecycleOperation::MoveOut,
            &[storage.clone(), expected[2].clone()],
        )
        .expect("original resident");
        assert_eq!(
            extracted.arguments().cloned().collect::<Vec<_>>(),
            vec![storage, expected[2].clone()]
        );
        assert_eq!(
            extracted.operation(),
            TerminalPlacementLifecycleOperation::MoveOut
        );
    }

    #[test]
    fn lifecycle_operands_reject_changed_access_missing_or_extra_roles() {
        let original = vec![
            argument(1, StructuralAccess::Owned),
            argument(2, StructuralAccess::Owned),
            argument(3, StructuralAccess::SharedBorrow),
        ];
        for position in 0..original.len() {
            let mut changed = original.clone();
            changed[position].access = StructuralAccess::MutableBorrow;
            assert!(
                PlacementLifecycleOperands::from_arguments(
                    TerminalPlacementLifecycleOperation::Initialize,
                    &changed,
                )
                .is_err()
            );
        }
        for count in 0..3 {
            assert!(
                PlacementLifecycleOperands::from_arguments(
                    TerminalPlacementLifecycleOperation::Initialize,
                    &original[..count],
                )
                .is_err()
            );
        }
        let mut extra = original.clone();
        extra.push(argument(4, StructuralAccess::Owned));
        assert!(
            PlacementLifecycleOperands::from_arguments(
                TerminalPlacementLifecycleOperation::Initialize,
                &extra,
            )
            .is_err()
        );
        assert!(
            PlacementLifecycleOperands::from_arguments(
                TerminalPlacementLifecycleOperation::MoveOut,
                &original,
            )
            .is_err()
        );
        assert!(
            PlacementLifecycleOperands::from_arguments(
                TerminalPlacementLifecycleOperation::MoveOut,
                &original[2..],
            )
            .is_err()
        );
    }
}

impl PlacementLifecycleOperands {
    pub fn from_arguments(
        operation: TerminalPlacementLifecycleOperation,
        arguments: &[StructuralArgument],
    ) -> Result<Self, &'static str> {
        match (operation, arguments) {
            (TerminalPlacementLifecycleOperation::View, [storage, profile])
                if matches!(storage.access, terminal_psi::reading_borrow_pair_pattern!())
                    && profile.access == StructuralAccess::SharedBorrow =>
            {
                Ok(Self::View {
                    storage: storage.clone(),
                    profile: profile.clone(),
                })
            }
            (TerminalPlacementLifecycleOperation::Initialize, [storage, value, profile])
                if storage.access == StructuralAccess::Owned
                    && value.access == StructuralAccess::Owned
                    && profile.access == StructuralAccess::SharedBorrow =>
            {
                Ok(Self::Initialize {
                    storage: storage.clone(),
                    value: value.clone(),
                    profile: profile.clone(),
                })
            }
            (TerminalPlacementLifecycleOperation::MoveOut, [storage, profile])
                if storage.access == StructuralAccess::Owned
                    && profile.access == StructuralAccess::SharedBorrow =>
            {
                Ok(Self::MoveOut {
                    storage: storage.clone(),
                    profile: profile.clone(),
                })
            }
            _ => Err("placement lifecycle operands differ from the operation's ownership roles"),
        }
    }

    pub fn arguments(&self) -> impl Iterator<Item = &StructuralArgument> {
        match self {
            Self::Initialize {
                storage,
                value,
                profile,
            } => [Some(storage), Some(value), Some(profile)],
            Self::MoveOut { storage, profile } | Self::View { storage, profile } => {
                [Some(storage), Some(profile), None]
            }
        }
        .into_iter()
        .flatten()
    }

    pub const fn operation(&self) -> TerminalPlacementLifecycleOperation {
        match self {
            Self::Initialize { .. } => TerminalPlacementLifecycleOperation::Initialize,
            Self::MoveOut { .. } => TerminalPlacementLifecycleOperation::MoveOut,
            Self::View { .. } => TerminalPlacementLifecycleOperation::View,
        }
    }
}
