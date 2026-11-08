// We classify the exact value operands of compiler-owned placement lifecycle
// operations (`Relocate`, `Initialize`, `MoveOut`, `View`). `Initialize` and
// `MoveOut` consume the original owner while `View` retains its shared or
// exclusive loan; each borrows a completed profile, and initialization also
// consumes its value.

use terminal_psi::{
    StructuralAccess, StructuralArgument, TerminalAccessLifecycleOperation as LifecycleOperation,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AccessLifecycleOperands {
    Relocate {
        source: StructuralArgument,
        destination: StructuralArgument,
        source_profile: StructuralArgument,
        destination_profile: StructuralArgument,
    },
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

impl AccessLifecycleOperands {
    pub fn from_arguments(
        operation: LifecycleOperation,
        arguments: &[StructuralArgument],
    ) -> Result<Self, &'static str> {
        match (operation, arguments) {
            (
                LifecycleOperation::Relocate,
                [source, destination, source_profile, destination_profile],
            ) if source.access == StructuralAccess::Owned
                && destination.access == StructuralAccess::Owned
                && source_profile.access == StructuralAccess::SharedBorrow
                && destination_profile.access == StructuralAccess::SharedBorrow =>
            {
                Ok(Self::Relocate {
                    source: source.clone(),
                    destination: destination.clone(),
                    source_profile: source_profile.clone(),
                    destination_profile: destination_profile.clone(),
                })
            }
            (LifecycleOperation::View, [storage, profile])
                if matches!(storage.access, terminal_psi::reading_borrow_pair_pattern!())
                    && profile.access == StructuralAccess::SharedBorrow =>
            {
                Ok(Self::View {
                    storage: storage.clone(),
                    profile: profile.clone(),
                })
            }
            (LifecycleOperation::Initialize, [storage, value, profile])
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
            (LifecycleOperation::MoveOut, [storage, profile])
                if storage.access == StructuralAccess::Owned
                    && profile.access == StructuralAccess::SharedBorrow =>
            {
                Ok(Self::MoveOut {
                    storage: storage.clone(),
                    profile: profile.clone(),
                })
            }
            _ => Err("access lifecycle operands differ from the operation's ownership roles"),
        }
    }

    pub fn arguments(&self) -> impl Iterator<Item = &StructuralArgument> {
        match self {
            Self::Relocate {
                source,
                destination,
                source_profile,
                destination_profile,
            } => [
                Some(source),
                Some(destination),
                Some(source_profile),
                Some(destination_profile),
            ],
            Self::Initialize {
                storage,
                value,
                profile,
            } => [Some(storage), Some(value), Some(profile), None],
            Self::MoveOut { storage, profile } | Self::View { storage, profile } => {
                [Some(storage), Some(profile), None, None]
            }
        }
        .into_iter()
        .flatten()
    }

    pub const fn operation(&self) -> LifecycleOperation {
        match self {
            Self::Relocate { .. } => LifecycleOperation::Relocate,
            Self::Initialize { .. } => LifecycleOperation::Initialize,
            Self::MoveOut { .. } => LifecycleOperation::MoveOut,
            Self::View { .. } => LifecycleOperation::View,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AccessLifecycleOperands, LifecycleOperation, StructuralAccess, StructuralArgument,
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
    fn relocation_retains_two_owned_inputs_and_two_distinct_profile_positions() {
        let inputs = vec![
            argument(1, StructuralAccess::Owned),
            argument(2, StructuralAccess::Owned),
            argument(3, StructuralAccess::SharedBorrow),
            argument(4, StructuralAccess::SharedBorrow),
        ];
        let operands =
            AccessLifecycleOperands::from_arguments(LifecycleOperation::Relocate, &inputs)
                .expect("run and destination with separate profiles");
        assert_eq!(operands.operation(), LifecycleOperation::Relocate);
        assert_eq!(operands.arguments().cloned().collect::<Vec<_>>(), inputs);
        for position in 0..inputs.len() {
            let mut changed = inputs.clone();
            changed[position].access = StructuralAccess::MutableBorrow;
            assert!(
                AccessLifecycleOperands::from_arguments(LifecycleOperation::Relocate, &changed)
                    .is_err()
            );
        }
        assert!(
            AccessLifecycleOperands::from_arguments(LifecycleOperation::Relocate, &inputs[..3])
                .is_err()
        );
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
        let operands =
            AccessLifecycleOperands::from_arguments(LifecycleOperation::Initialize, &expected)
                .expect("owned inputs and borrowed profile");
        assert_eq!(operands.arguments().cloned().collect::<Vec<_>>(), expected);
        assert_eq!(operands.operation(), LifecycleOperation::Initialize);
        let extracted = AccessLifecycleOperands::from_arguments(
            LifecycleOperation::MoveOut,
            &[storage.clone(), expected[2].clone()],
        )
        .expect("original resident");
        assert_eq!(
            extracted.arguments().cloned().collect::<Vec<_>>(),
            vec![storage, expected[2].clone()]
        );
        assert_eq!(extracted.operation(), LifecycleOperation::MoveOut);
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
                AccessLifecycleOperands::from_arguments(LifecycleOperation::Initialize, &changed,)
                    .is_err()
            );
        }
        for count in 0..3 {
            assert!(
                AccessLifecycleOperands::from_arguments(
                    LifecycleOperation::Initialize,
                    &original[..count],
                )
                .is_err()
            );
        }
        let mut extra = original.clone();
        extra.push(argument(4, StructuralAccess::Owned));
        assert!(
            AccessLifecycleOperands::from_arguments(LifecycleOperation::Initialize, &extra,)
                .is_err()
        );
        assert!(
            AccessLifecycleOperands::from_arguments(LifecycleOperation::MoveOut, &original,)
                .is_err()
        );
        assert!(
            AccessLifecycleOperands::from_arguments(LifecycleOperation::MoveOut, &original[2..],)
                .is_err()
        );
    }
}
