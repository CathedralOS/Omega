//! Constructor values retain the selected nominal application and case.
//! Local handles rejoin declarations; semantic equality uses the complete
//! applied carrier so equivalent declaration copies have the same meaning.
//! These are ephemeral proof terms: each judge and its clones borrow one
//! CheckingInput. Neither local handles nor its interned identities cross a
//! program boundary or enter a saved artifact.

use std::hash::{Hash, Hasher};
use symbols::SymbolHandle;
use typed_trees::data::{DataDefinition, DataMember, DataVariant};
use typed_trees::type_identity::NormalizedTypeIdentity;

#[derive(Clone, Copy)]
pub(crate) enum ConstructorIdentity {
    Boolean(bool),
    Nominal {
        carrier: NormalizedTypeIdentity,
        reference: typed_trees::types::TypeReferenceHandle,
        declaration: SymbolHandle,
        case: Option<SymbolHandle>,
        member: Option<usize>,
    },
}

impl ConstructorIdentity {
    pub(in crate::proof_contracts::contract_entailment) fn nominal(
        program: &dyn crate::CheckingInput,
        definition: &DataDefinition,
        variant: Option<&DataVariant>,
    ) -> Option<Self> {
        let member = match variant {
            Some(variant) => Some(program.data_members(definition).iter().position(|member| {
                matches!(member, DataMember::Variant(selected) if selected.symbol == variant.symbol)
            })?),
            None => None,
        };
        Some(Self::Nominal {
            carrier: match definition.generic_instance {
                Some(reference) => program.normalized_type_identity(
                    program
                        .type_reference_table
                        .unconstrained_type_reference(reference),
                ),
                None => program.normalized_nominal_type_identity(definition.symbol)?,
            },
            reference: definition.generic_instance.unwrap_or_default(),
            declaration: definition.symbol,
            case: variant.map(|variant| variant.symbol),
            member,
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn at_reference(
        program: &dyn crate::CheckingInput,
        reference: typed_trees::types::TypeReferenceHandle,
        definition: &DataDefinition,
        variant: Option<&DataVariant>,
    ) -> Option<Self> {
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        let selected = program.data_constructor_definition(reference, definition.symbol)?;
        let variant = match variant {
            Some(variant) => {
                Some(program.data_constructor_case(reference, definition.symbol, variant.symbol)?)
            }
            None => None,
        };
        let Self::Nominal {
            declaration,
            case,
            member,
            ..
        } = Self::nominal(program, selected, variant)?
        else {
            return None;
        };
        Some(Self::Nominal {
            carrier: program.normalized_type_identity(constructor_reference(program, reference)),
            reference,
            declaration,
            case,
            member,
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn boolean(self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(value),
            Self::Nominal { .. } => None,
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn declaration(
        self,
    ) -> Option<SymbolHandle> {
        match self {
            Self::Nominal { declaration, .. } => Some(declaration),
            Self::Boolean(_) => None,
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn same_carrier(self, other: Self) -> bool {
        match (self, other) {
            (Self::Boolean(_), Self::Boolean(_)) => true,
            (Self::Nominal { carrier: left, .. }, Self::Nominal { carrier: right, .. }) => {
                left == right
            }
            _ => false,
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn same_resolution_payload(
        self,
        other: Self,
    ) -> bool {
        self == other
            && match (self, other) {
                (
                    Self::Nominal {
                        declaration: left,
                        case: left_case,
                        reference: left_reference,
                        ..
                    },
                    Self::Nominal {
                        declaration: right,
                        case: right_case,
                        reference: right_reference,
                        ..
                    },
                ) => left == right && left_case == right_case && left_reference == right_reference,
                (Self::Boolean(_), Self::Boolean(_)) => true,
                _ => false,
            }
    }

    /// A constructor can be formed through distinct checked type references.
    /// Its value still names the same complete carrier and exact declared case;
    /// the receiver must interpret each reference's field schema separately.
    pub(super) fn same_constructor(self, other: Self) -> bool {
        self == other
            && match (self, other) {
                (
                    Self::Nominal {
                        declaration: left,
                        case: left_case,
                        ..
                    },
                    Self::Nominal {
                        declaration: right,
                        case: right_case,
                        ..
                    },
                ) => left == right && left_case == right_case,
                (Self::Boolean(_), Self::Boolean(_)) => true,
                _ => false,
            }
    }

    /// Classifiers can name an authored template case. Rejoin that exact case
    /// through the receiving application; spelling never selects its owner.
    pub(in crate::proof_contracts::contract_entailment) fn matches_case(
        self,
        program: &dyn crate::CheckingInput,
        declaration: SymbolHandle,
        case: SymbolHandle,
    ) -> Option<bool> {
        let Self::Nominal {
            reference,
            declaration: observed,
            case: Some(observed_case),
            ..
        } = self
        else {
            return None;
        };
        let expected = program.data_definition_by_symbol(declaration)?;
        let expected_case =
            program
                .data_members(expected)
                .iter()
                .find_map(|member| match member {
                    DataMember::Variant(variant) if variant.symbol == case => Some(variant),
                    _ => None,
                })?;
        let expected_identity = Self::nominal(program, expected, Some(expected_case))?;
        if self.same_carrier(expected_identity) {
            return Some(self == expected_identity);
        }
        let receiving = if reference.is_valid() {
            reference
        } else {
            program
                .data_definition_by_symbol(observed)?
                .generic_instance?
        };
        let selected = program.data_constructor_case(receiving, declaration, case)?;
        Some(observed_case == selected.symbol)
    }
}

impl PartialEq for ConstructorIdentity {
    fn eq(&self, other: &Self) -> bool {
        match (*self, *other) {
            (Self::Boolean(left), Self::Boolean(right)) => left == right,
            (
                Self::Nominal {
                    carrier: left,
                    member: left_member,
                    ..
                },
                Self::Nominal {
                    carrier: right,
                    member: right_member,
                    ..
                },
            ) => left == right && left_member == right_member,
            _ => false,
        }
    }
}
impl Eq for ConstructorIdentity {}

impl Hash for ConstructorIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Boolean(value) => value.hash(state),
            Self::Nominal {
                carrier, member, ..
            } => (carrier, member).hash(state),
        }
    }
}

impl std::fmt::Debug for ConstructorIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Boolean(value) => formatter.debug_tuple("Boolean").field(value).finish(),
            Self::Nominal {
                carrier, member, ..
            } => formatter
                .debug_tuple("Nominal")
                .field(carrier)
                .field(member)
                .finish(),
        }
    }
}

// LayoutStorage presents its semantic value's constructor. The surrounding
// ObservedValue/capture retains the physical carrier and access obligations;
// normalizing this value identity grants no right to read or copy the storage.
fn constructor_reference(
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> typed_trees::types::TypeReferenceHandle {
    loop {
        reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        let Some(storage) = program.layout_storage_arguments(reference) else {
            return reference;
        };
        reference = storage.semantic_type;
    }
}
