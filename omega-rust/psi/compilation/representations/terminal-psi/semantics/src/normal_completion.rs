//! The declared carrier of an actual normal return, independent of reachability.
//!
//! Both source checking and artifact reception ask this same typed question.
//! Executable producers and input declarations supply values; result placeholders,
//! erased inputs and provider attachments do not. An owned empty source may fit
//! any result type, but this supplies no availability, ownership or domain proof.

use semantic_vocabulary::{PlaceId, ScalarType, StructuralPlaceKind, StructuralTypeId, ValueId};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine,
    TerminalMachineResult, Terminator, ValueDeclaration,
};

use crate::inhabitation::{Inhabitation, Status};
use crate::plain_affine::{declared_multiplicity, required_result_multiplicity};

/// A loan-producing operation owns an affine occurrence even for a copyable
/// shared-reference type. A checked return transfers that occurrence and its
/// loan into the result. The caller's copy permission comes from the result
/// type. The ordinary return custody checks still prove liveness and backing.
pub fn structural_return_carrier_matches(
    result: &StructuralResultDeclaration,
    source: &StructuralTypeDeclaration,
    multiplicity: StructuralMultiplicity,
) -> bool {
    source.id == result.structural_type
        && (multiplicity == result.multiplicity
            || (multiplicity == StructuralMultiplicity::Affine
                && result.multiplicity == StructuralMultiplicity::Unrestricted
                && source.is_copyable()
                && !source.owns_linear_claim
                && matches!(
                    source.shape,
                    StructuralTypeShape::Reference {
                        access: StructuralAccess::SharedBorrow,
                        ..
                    }
                )))
}

/// Once-per-machine borrowed declaration scratch. Prepare it from the machine
/// whose terminators will be queried; IDs from another machine confer no source
/// identity here. Lookups do not scan the machine again at every return edge.
#[derive(Debug)]
pub struct ReturnSources<'machine> {
    scalars: Vec<&'machine ValueDeclaration>,
    places: Vec<StructuralSource>,
}

#[derive(Debug, Clone, Copy)]
struct StructuralSource {
    place: PlaceId,
    structural_type: StructuralTypeId,
    access: StructuralAccess,
    /// A trivial local derives multiplicity from its structural type.
    explicit_multiplicity: Option<StructuralMultiplicity>,
    /// A fresh reference can own an affine permission while its type is copyable.
    required_result_multiplicity: Option<StructuralMultiplicity>,
}

impl<'machine> ReturnSources<'machine> {
    pub fn new(machine: &'machine TerminalMachine) -> Result<Self, &'static str> {
        let mut scalars: Vec<_> = machine
            .parameters
            .iter()
            .chain(machine.block_parameters())
            .chain(
                machine
                    .operations()
                    .filter_map(|operation| operation.result.scalar_ref()),
            )
            .collect();
        scalars.sort_unstable_by_key(|value| value.id);
        if scalars.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err("normal return sources repeat a scalar identity");
        }
        let mut contracts: Vec<_> = machine
            .structural_parameters
            .iter()
            .chain(machine.block_structural_parameters())
            .map(|parameter| StructuralSource {
                place: parameter.place,
                structural_type: parameter.structural_type,
                access: parameter.access,
                explicit_multiplicity: Some(parameter.multiplicity),
                required_result_multiplicity: None,
            })
            .chain(machine.operations().filter_map(|operation| {
                let result = operation.result.structural()?;
                Some(StructuralSource {
                    place: result.place,
                    structural_type: result.structural_type,
                    access: StructuralAccess::Owned,
                    explicit_multiplicity: Some(result.multiplicity),
                    required_result_multiplicity: required_result_multiplicity(operation),
                })
            }))
            .collect();
        contracts.sort_unstable_by_key(|row| row.place);
        if contracts
            .windows(2)
            .any(|pair| pair[0].place == pair[1].place)
        {
            return Err("normal return sources repeat a structural identity");
        }
        let mut places = Vec::with_capacity(machine.structural_places.len());
        for declaration in &machine.structural_places {
            let contract = match declaration.kind {
                StructuralPlaceKind::Parameter { .. }
                | StructuralPlaceKind::BlockParameter { .. }
                | StructuralPlaceKind::OperationResult { .. } => contracts
                    .binary_search_by_key(&declaration.id, |row| row.place)
                    .ok()
                    .map(|index| contracts[index]),
                StructuralPlaceKind::TrivialAffineLocal {
                    structural_type, ..
                } => Some(StructuralSource {
                    place: declaration.id,
                    structural_type,
                    access: StructuralAccess::Owned,
                    explicit_multiplicity: None,
                    required_result_multiplicity: None,
                }),
                StructuralPlaceKind::ByteSequenceLiteral {
                    structural_type, ..
                } => Some(StructuralSource {
                    place: declaration.id,
                    structural_type,
                    access: StructuralAccess::Owned,
                    explicit_multiplicity: Some(StructuralMultiplicity::Unrestricted),
                    required_result_multiplicity: None,
                }),
                StructuralPlaceKind::Result | StructuralPlaceKind::ProviderAttachment { .. } => {
                    None
                }
            };
            places.extend(contract);
        }
        places.sort_unstable_by_key(|row| row.place);
        if places.windows(2).any(|pair| pair[0].place == pair[1].place) {
            return Err("normal return sources repeat a structural place");
        }
        Ok(Self { scalars, places })
    }

    /// Executable input/producer types only. A signature result placeholder,
    /// erased input or foreign ID supplies no runtime scalar in this machine.
    pub fn scalar_type(&self, value: ValueId) -> Option<ScalarType> {
        self.scalars
            .binary_search_by_key(&value, |value| value.id)
            .ok()
            .map(|index| self.scalars[index].scalar_type)
    }

    /// Check only the actual return's declared carrier. Existing storage and
    /// proof judgments still establish availability, custody and qualifications.
    pub fn validate_return(
        &self,
        expected: &TerminalMachineResult,
        terminator: &Terminator,
        inhabitation: &Inhabitation,
    ) -> Result<(), &'static str> {
        match terminator {
            Terminator::ReturnUnit { .. } => {
                if !matches!(expected, TerminalMachineResult::Unit) {
                    return Err("unit return differs from the declared result");
                }
            }
            Terminator::Return { value, .. } => {
                let actual = self.scalar_type(*value);
                if !expected
                    .scalar_ref()
                    .is_some_and(|expected| actual == Some(expected.scalar_type))
                {
                    return Err("scalar return differs from the declared result");
                }
            }
            Terminator::ReturnStructural { source, .. } => {
                let actual = self.structural(*source)?.structural_type;
                if expected
                    .structural()
                    .is_some_and(|expected| expected.structural_type == actual)
                {
                    return Ok(());
                }
                if !self.is_uninhabited_return(terminator, inhabitation)? {
                    return Err("structural return differs from the declared result");
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Only this returned owned root can establish the empty-value exception.
    /// An unrelated empty type, borrowed referent, or result placeholder cannot.
    /// This does not change physical normal-completion reachability: an identity
    /// machine `Empty -> Empty` still physically contains a normal return edge.
    pub fn is_uninhabited_return(
        &self,
        terminator: &Terminator,
        inhabitation: &Inhabitation,
    ) -> Result<bool, &'static str> {
        let Terminator::ReturnStructural { source, .. } = terminator else {
            return Ok(false);
        };
        let actual = self.structural(*source)?;
        Ok(actual.access == StructuralAccess::Owned
            && inhabitation.status(actual.structural_type) == Status::ProvedEmpty)
    }

    /// A whole discard consumes an owned affine value on the normal edge.
    /// Check its actual input or producer independently of other homes.
    pub fn validate_whole_discards<'types>(
        &self,
        terminator: &Terminator,
        lookup_type: impl Fn(StructuralTypeId) -> Option<&'types StructuralTypeDeclaration>,
    ) -> Result<(), &'static str> {
        const WRONG_CUSTODY: &str = "whole-root discard has no owned affine source";
        for (_, actions) in terminator.cleanup_rosters() {
            for action in actions {
                let TerminalAffineCleanupAction::DiscardRoot(place) = action else {
                    continue;
                };
                let source = self.structural(*place).map_err(|_| WRONG_CUSTODY)?;
                let declaration = lookup_type(source.structural_type).ok_or(WRONG_CUSTODY)?;
                let declared = declared_multiplicity(declaration, source.access);
                if source.access != StructuralAccess::Owned
                    || source.explicit_multiplicity.unwrap_or(declared)
                        != StructuralMultiplicity::Affine
                    || source.required_result_multiplicity.unwrap_or(declared)
                        != StructuralMultiplicity::Affine
                    || declaration.owns_linear_claim
                {
                    return Err(WRONG_CUSTODY);
                }
            }
        }
        Ok(())
    }

    fn structural(&self, source: PlaceId) -> Result<&StructuralSource, &'static str> {
        let index = self
            .places
            .binary_search_by_key(&source, |row| row.place)
            .map_err(|_| "structural return has no actual source declaration")?;
        Ok(&self.places[index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inhabitation::TerminalContracts;
    use semantic_vocabulary::{
        BlockId, BoundaryMachineId, ContractId, EdgeId, MachineId, Proposition, ScalarType,
        StructuralFieldId, ValueId,
    };
    use terminal_psi::{
        Block, BoundaryParameterKind, MachineContract, RegisterTargetApplicability,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        StructuralRecordStorage, StructuralResultDeclaration, StructuralTypeDeclaration,
        StructuralTypeShape, TerminalIdentityIndex, TerminalModule,
    };

    fn id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn scalar(raw: u64) -> ValueDeclaration {
        ValueDeclaration {
            id: ValueId::new(raw).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }
    }

    fn types() -> [StructuralTypeDeclaration; 2] {
        [true, false].map(|empty| StructuralTypeDeclaration {
            declaration_identity: Default::default(),
            id: id(if empty { 1 } else { 2 }),
            identity: TerminalIdentityIndex::ABSENT,
            quotient_carrier: None,
            owns_linear_claim: false,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            no_code_affine_cleanup: true,
            descriptor_cleanup: false,
            nominal_cleanup: None,
            shape: StructuralTypeShape::Record {
                fields: Vec::new(),
                storage: StructuralRecordStorage::Packed,
            },
            scalar_restriction: None,
            requires: if empty {
                vec![Proposition::Falsehood]
            } else {
                Vec::new()
            },
        })
    }

    fn machine() -> TerminalMachine {
        let place = PlaceId::new(1).unwrap();
        let mut contract = MachineContract::empty(ContractId::new(1).unwrap());
        contract
            .structural_parameters
            .push(StructuralParameterDeclaration {
                place,
                position: 0,
                structural_type: id(1),
                access: StructuralAccess::Owned,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                additional_input_qualifications: Vec::new(),
            });
        contract
            .parameter_order
            .push(BoundaryParameterKind::Structural);
        contract.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: PlaceId::new(2).unwrap(),
            structural_type: id(1),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        });
        let entry = BlockId::new(1).unwrap();
        TerminalMachine {
            assembly_target_requirement: RegisterTargetApplicability::Any,
            id: MachineId::new(1).unwrap(),
            attachment: None,
            ranked_scc: None,
            structural_places: vec![
                StructuralPlaceDeclaration {
                    id: place,
                    kind: StructuralPlaceKind::Parameter { position: 0 },
                },
                StructuralPlaceDeclaration {
                    id: PlaceId::new(2).unwrap(),
                    kind: StructuralPlaceKind::Result,
                },
            ],
            call_formations: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry,
            blocks: vec![Block::plain(
                entry,
                Vec::new(),
                Terminator::ReturnStructural {
                    edge: EdgeId::new(1).unwrap(),
                    source: place,
                    returned_claims: Vec::new(),
                    cleanup_actions: Vec::new(),
                },
            )],
            contract,
        }
    }

    #[test]
    fn whole_discard_requires_an_owned_affine_source() {
        let mut machine = machine();
        let mut types = types();
        types[0].multiplicity = terminal_psi::StructuralMultiplicity::Affine;
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
        let place = machine.structural_parameters[0].place;
        let terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
        };
        let check = |machine: &TerminalMachine, types: &[StructuralTypeDeclaration]| {
            ReturnSources::new(machine)
                .unwrap()
                .validate_whole_discards(&terminator, |id| {
                    terminal_psi::find_structural_type(types, id)
                })
        };
        assert_eq!(check(&machine, &types), Ok(()));
        let refused = Err("whole-root discard has no owned affine source");

        machine.structural_parameters[0].access = StructuralAccess::SharedBorrow;
        assert_eq!(check(&machine, &types), refused);

        machine.structural_parameters[0].access = StructuralAccess::Owned;
        types[0].multiplicity = terminal_psi::StructuralMultiplicity::Unrestricted;
        assert_eq!(check(&machine, &types), refused);
        machine.structural_parameters[0].multiplicity = StructuralMultiplicity::Unrestricted;
        assert_eq!(check(&machine, &types), refused);
    }

    #[test]
    fn an_owned_empty_return_is_typed_but_still_physically_returns() {
        let types = types();
        let query = Inhabitation::prepare(&TerminalContracts::new(&types)).unwrap();
        let machine = machine();
        let sources = ReturnSources::new(&machine).unwrap();
        let terminator = &machine.blocks[0].terminator;
        assert_eq!(sources.is_uninhabited_return(terminator, &query), Ok(true));
        sources
            .validate_return(&machine.result, terminator, &query)
            .unwrap();
        for expected in [
            TerminalMachineResult::Unit,
            TerminalMachineResult::Scalar(scalar(1)),
        ] {
            sources
                .validate_return(&expected, terminator, &query)
                .unwrap();
        }
        let mut module = TerminalModule::declarations();
        module.structural_types = types.to_vec();
        module.machines.push(machine);
        assert_eq!(
            crate::control_graph::normal_completion_possible(&module, module.machines[0].id),
            Ok(true)
        );
    }

    #[test]
    fn borrowed_unrelated_and_placeholder_roots_cannot_supply_an_empty_return() {
        let types = types();
        let query = Inhabitation::prepare(&TerminalContracts::new(&types)).unwrap();
        let mut machine = machine();
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
            StructuralAccess::WriteOnlyBorrow,
        ] {
            machine.structural_parameters[0].access = access;
            let sources = ReturnSources::new(&machine).unwrap();
            assert_eq!(
                sources.is_uninhabited_return(&machine.blocks[0].terminator, &query),
                Ok(false)
            );
            assert!(
                sources
                    .validate_return(
                        &TerminalMachineResult::Scalar(scalar(1)),
                        &machine.blocks[0].terminator,
                        &query
                    )
                    .is_err()
            );
        }
        machine.structural_parameters[0].access = StructuralAccess::Owned;
        machine.structural_parameters[0].structural_type = id(2);
        let sources = ReturnSources::new(&machine).unwrap();
        assert_eq!(
            sources.is_uninhabited_return(&machine.blocks[0].terminator, &query),
            Ok(false)
        );
        assert!(
            sources
                .validate_return(&machine.result, &machine.blocks[0].terminator, &query)
                .is_err()
        );
        for kind in [
            StructuralPlaceKind::Result,
            StructuralPlaceKind::ProviderAttachment {
                attachment: id(1),
                field: StructuralFieldId::new(1).unwrap(),
                boundary: BoundaryMachineId::new(1).unwrap(),
            },
        ] {
            machine.structural_places[0].kind = kind;
            let sources = ReturnSources::new(&machine).unwrap();
            assert!(
                sources
                    .is_uninhabited_return(&machine.blocks[0].terminator, &query)
                    .is_err()
            );
        }
    }

    #[test]
    fn scalar_return_sources_exclude_result_and_erased_placeholders() {
        let query = Inhabitation::prepare(&TerminalContracts::new([])).unwrap();
        let mut machine = machine();
        machine.parameters.push(scalar(1));
        machine.result = TerminalMachineResult::Scalar(scalar(2));
        machine.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(1).unwrap(),
            value: ValueId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        };
        let sources = ReturnSources::new(&machine).unwrap();
        assert_eq!(
            sources.scalar_type(ValueId::new(1).unwrap()),
            Some(ScalarType::Boolean)
        );
        assert_eq!(sources.scalar_type(ValueId::new(2).unwrap()), None);
        assert_eq!(sources.scalar_type(ValueId::new(99).unwrap()), None);
        sources
            .validate_return(&machine.result, &machine.blocks[0].terminator, &query)
            .unwrap();
        assert_eq!(
            sources.is_uninhabited_return(&machine.blocks[0].terminator, &query),
            Ok(false)
        );
        assert!(
            sources
                .validate_return(
                    &TerminalMachineResult::Unit,
                    &machine.blocks[0].terminator,
                    &query
                )
                .is_err()
        );
        machine.blocks[0].terminator = Terminator::Return {
            edge: EdgeId::new(1).unwrap(),
            value: ValueId::new(2).unwrap(),
            cleanup_actions: Vec::new(),
        };
        machine.erased_scalar_formals.push(scalar(2));
        let sources = ReturnSources::new(&machine).unwrap();
        assert_eq!(sources.scalar_type(ValueId::new(2).unwrap()), None);
        assert!(
            sources
                .validate_return(&machine.result, &machine.blocks[0].terminator, &query)
                .is_err()
        );
        machine.blocks[0].terminator = Terminator::ReturnUnit {
            edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        };
        assert!(
            ReturnSources::new(&machine)
                .unwrap()
                .validate_return(&machine.result, &machine.blocks[0].terminator, &query)
                .is_err()
        );
    }

    #[test]
    fn repeated_sources_and_missing_large_ids_refuse_before_return_queries() {
        let types = types();
        let query = Inhabitation::prepare(&TerminalContracts::new(&types)).unwrap();
        let mut machine = machine();
        machine.parameters.extend([scalar(1), scalar(1)]);
        assert!(ReturnSources::new(&machine).is_err());
        machine.parameters.clear();
        let repeated = machine.structural_parameters[0].clone();
        machine.structural_parameters.push(repeated);
        assert!(ReturnSources::new(&machine).is_err());
        machine.structural_parameters.pop();
        machine.structural_places.push(machine.structural_places[0]);
        assert!(ReturnSources::new(&machine).is_err());
        machine.structural_places.pop();
        machine.blocks[0].terminator = Terminator::ReturnStructural {
            edge: EdgeId::new(1).unwrap(),
            source: PlaceId::new(u64::MAX).unwrap(),
            returned_claims: Vec::new(),
            cleanup_actions: Vec::new(),
        };
        assert!(
            ReturnSources::new(&machine)
                .unwrap()
                .is_uninhabited_return(&machine.blocks[0].terminator, &query)
                .is_err()
        );
    }
}
