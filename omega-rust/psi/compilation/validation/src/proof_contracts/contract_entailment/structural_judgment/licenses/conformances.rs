//! Checked trait applications and selected operation entries for algebra laws.
//! Storage reads exact requirement slots; joining laws preserves the entire
//! application and the actual selected operations, never a family label alone.

use super::{SymbolHandle, TypeReferenceHandle, entry_carrier};
use crate::proof_contracts::contract_entailment::law_conformance::select_slot_candidate;

/// Each requirement symbol owns one carrier group. The source declaration is
/// validated before its dense slot is read, and the retained full handle and
/// trait prevent a reused generation from selecting another declaration.
#[derive(Default)]
pub(super) struct SlotCarriers {
    requirements: Vec<Option<SlotCarrierGroup>>,
}

pub(super) struct SlotCarrierRow {
    pub(super) carrier: TypeReferenceHandle,
    entry: SymbolHandle,
    application: typed_trees::type_identity::NormalizedTypeIdentity,
    alias: Option<String>,
    declaring_applications: Vec<(
        SymbolHandle,
        typed_trees::type_identity::NormalizedTypeIdentity,
    )>,
}

impl SlotCarrierRow {
    fn from_scope<'program>(
        program: &'program dyn crate::CheckingInput,
        scope: &typed_trees::trait_definition::AppliedTraitScope<'program>,
        carrier: TypeReferenceHandle,
        entry: SymbolHandle,
    ) -> Option<Self> {
        let application = scope.application_identity(program.typed_program())?;
        let mut declaring_applications = Vec::new();
        scope.visit(program.typed_program(), &mut |scope| {
            if let Some(identity) = scope.application_identity(program.typed_program()) {
                declaring_applications.push((scope.declaration.symbol, identity));
            }
        });
        Some(Self {
            carrier,
            entry,
            application,
            alias: None,
            declaring_applications,
        })
    }

    fn operation_application(
        &self,
        program: &dyn crate::CheckingInput,
        operation: SymbolHandle,
    ) -> Option<(
        SymbolHandle,
        SymbolHandle,
        typed_trees::type_identity::NormalizedTypeIdentity,
    )> {
        let (owner, template) = program
            .closed_requirement_application(operation)
            .map_or_else(
                || (program.symbols.get(operation).parent, operation),
                |closed| (closed.declaring_trait, closed.template),
            );
        let mut applications = self
            .declaring_applications
            .iter()
            .filter(|(candidate, _)| *candidate == owner);
        let application = applications.next()?.1;
        // The source law already selected its declaring scope. A produced
        // row transports that scope as a whole; an ambiguous inherited tuple
        // cannot be recovered from a bare declaration symbol.
        if applications.any(|(_, other)| *other != application) {
            return None;
        }
        Some((owner, template, application))
    }
}

struct SlotCarrierGroup {
    requirement: SymbolHandle,
    declaring_trait: SymbolHandle,
    rows: Vec<SlotCarrierRow>,
}

impl SlotCarriers {
    fn insert(
        &mut self,
        program: &dyn crate::CheckingInput,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
        row: SlotCarrierRow,
    ) {
        if !requirement.is_valid() {
            return;
        }
        program.symbols.get(requirement);
        let Some(declaration) = program.trait_definition_by_symbol(declaring_trait) else {
            return;
        };
        if program
            .trait_machine_signature_by_symbol(declaration, requirement)
            .is_none()
        {
            return;
        }
        let position = requirement.arena_index() as usize;
        if self.requirements.len() <= position {
            self.requirements.resize_with(position + 1, || None);
        }
        let group = self.requirements[position].get_or_insert_with(|| SlotCarrierGroup {
            requirement,
            declaring_trait,
            rows: Vec::new(),
        });
        if group.requirement == requirement && group.declaring_trait == declaring_trait {
            group.rows.push(row);
        }
    }

    pub(super) fn get(
        &self,
        program: &dyn crate::CheckingInput,
        declaring_trait: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Option<&[SlotCarrierRow]> {
        if !requirement.is_valid() {
            return None;
        }
        program.symbols.get(requirement);
        let group = self
            .requirements
            .get(requirement.arena_index() as usize)?
            .as_ref()?;
        (group.requirement == requirement && group.declaring_trait == declaring_trait)
            .then_some(group.rows.as_slice())
    }
}

/// The (trait symbol, requirement symbol) -> conforming (carrier, entry
/// symbol) index: conformances are fixed for the build, so both the per-law
/// slot membership test and each candidate scan read this instead of
/// re-walking every machine and conformance per license.
pub(super) fn conformance_slot_carriers_uncached(
    program: &dyn crate::CheckingInput,
) -> SlotCarriers {
    let mut slot_carriers = SlotCarriers::default();
    for candidate in program.machines() {
        for conformance in program.machine_trait_conformances(candidate) {
            if !conformance.requirement_symbol.is_valid() {
                continue;
            }
            let Some((candidate_entry, carrier)) = entry_carrier(program, candidate) else {
                continue;
            };
            let Some(scope) =
                typed_trees::trait_definition::AppliedTraitScope::from_machine_conformance(
                    program.typed_program(),
                    candidate,
                    conformance,
                )
            else {
                continue;
            };
            let Some(mut row) =
                SlotCarrierRow::from_scope(program, &scope, carrier, candidate_entry.symbol)
            else {
                continue;
            };
            row.alias = conformance
                .alias
                .as_ref()
                .map(|alias| alias.as_str().to_owned());
            slot_carriers.insert(
                program,
                conformance.symbol,
                conformance.requirement_symbol,
                row,
            );
        }
    }
    slot_carriers
}

pub(super) fn selected_operation(
    slot_carriers: &SlotCarriers,
    program: &dyn crate::CheckingInput,
    law: &SlotCarrierRow,
    operation: SymbolHandle,
) -> Option<SymbolHandle> {
    let (owner, template, application) = law.operation_application(program, operation)?;
    slot_satisfier(
        slot_carriers,
        program,
        owner,
        template,
        law.carrier,
        application,
        law.alias.as_deref(),
    )
}

/// Both the application and the operation selected by each law must agree.
/// Equal family labels cannot substitute for equality of selected entries.
pub(super) fn law_has_operations(
    slot_carriers: &SlotCarriers,
    program: &dyn crate::CheckingInput,
    trait_symbol: SymbolHandle,
    requirement: SymbolHandle,
    anchor: &SlotCarrierRow,
    operations: &[(SymbolHandle, SymbolHandle)],
) -> bool {
    slot_carriers
        .get(program, trait_symbol, requirement)
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row.application == anchor.application
                    && crate::value_custody::type_references::type_references_match(
                        program,
                        row.carrier,
                        anchor.carrier,
                    )
                    && operations.iter().all(|(operation, entry)| {
                        selected_operation(slot_carriers, program, row, *operation) == Some(*entry)
                    })
            })
        })
}

/// A law or operation row belongs to one complete conformance application.
/// Sharing only its first carrier cannot import another tuple's evidence.
fn slot_satisfier(
    slot_carriers: &SlotCarriers,
    program: &dyn crate::CheckingInput,
    trait_symbol: SymbolHandle,
    requirement: SymbolHandle,
    carrier: TypeReferenceHandle,
    application: typed_trees::type_identity::NormalizedTypeIdentity,
    preferred_alias: Option<&str>,
) -> Option<SymbolHandle> {
    let rows = slot_carriers
        .get(program, trait_symbol, requirement)?
        .iter()
        .filter(|row| {
            row.application == application
                && crate::value_custody::type_references::type_references_match(
                    program,
                    row.carrier,
                    carrier,
                )
        })
        .collect::<Vec<_>>();
    select_slot_candidate(&rows, preferred_alias, |row| row.alias.as_deref()).map(|row| row.entry)
}

#[cfg(test)]
mod tests {
    #[test]
    fn same_carrier_different_trait_application_cannot_supply_a_law() {
        use super::{SlotCarrierRow, SlotCarriers, law_has_operations, slot_satisfier};
        use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
        use typed_trees::{
            TypedTrees,
            data::TypeParameter,
            name::Identifier,
            signature::StateSignature,
            trait_definition::{AppliedTraitScope, TraitDefinition},
            types::TypeReferenceNode,
        };
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let top = symbols.insert_children(
            root,
            [
                (SymbolKind::Trait, SymbolNameRef::Static("Algebra")),
                (SymbolKind::Machine, SymbolNameRef::Static("first")),
                (SymbolKind::Machine, SymbolNameRef::Static("second")),
            ],
        );
        let owner = top.handle_at(0);
        let members = symbols.insert_children(
            owner,
            [
                (SymbolKind::TypeParameter, SymbolNameRef::Static("Carrier")),
                (SymbolKind::TypeParameter, SymbolNameRef::Static("Other")),
                (SymbolKind::State, SymbolNameRef::Static("law")),
                (SymbolKind::State, SymbolNameRef::Static("operation")),
            ],
        );
        let requirement = members.handle_at(2);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let carrier = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let other = program
            .type_reference_table
            .insert(TypeReferenceNode::Slice {
                element_type: carrier,
            });
        let mut declaration = TraitDefinition {
            symbol: owner,
            name: Identifier::from("Algebra"),
            ..Default::default()
        };
        declaration.type_parameters = program.data_type_parameters.insert_many([
            TypeParameter {
                symbol: members.handle_at(0),
                name: Identifier::from("Carrier"),
                ..Default::default()
            },
            TypeParameter {
                symbol: members.handle_at(1),
                name: Identifier::from("Other"),
                ..Default::default()
            },
        ]);
        program.push_trait_machine_signature(
            &mut declaration,
            StateSignature {
                symbol: requirement,
                name: Identifier::from("law"),
                return_type: carrier,
                ..Default::default()
            },
        );
        let operation = members.handle_at(3);
        program.push_trait_machine_signature(
            &mut declaration,
            StateSignature {
                symbol: operation,
                name: Identifier::from("operation"),
                return_type: carrier,
                ..Default::default()
            },
        );
        program.push_trait_definition(declaration);
        let declaration = program.trait_definition_by_symbol(owner).unwrap();
        let mut first = AppliedTraitScope::abstract_scope(declaration);
        first.substitutions = vec![
            (members.handle_at(0), carrier),
            (members.handle_at(1), carrier),
        ];
        let mut second = AppliedTraitScope::abstract_scope(declaration);
        second.substitutions = vec![
            (members.handle_at(0), carrier),
            (members.handle_at(1), other),
        ];
        let first_row =
            SlotCarrierRow::from_scope(&program, &first, carrier, top.handle_at(1)).unwrap();
        let second_row =
            SlotCarrierRow::from_scope(&program, &second, carrier, top.handle_at(2)).unwrap();
        let first_application = first_row.application;
        let second_application = second_row.application;
        assert_ne!(first_application, second_application);
        let mut rows = SlotCarriers::default();
        rows.insert(&program, owner, requirement, second_row);
        assert!(
            slot_satisfier(
                &rows,
                &program,
                owner,
                requirement,
                carrier,
                first_application,
                None,
            )
            .is_none()
        );
        assert_eq!(
            slot_satisfier(
                &rows,
                &program,
                owner,
                requirement,
                carrier,
                second_application,
                None,
            ),
            Some(top.handle_at(2))
        );
        rows.insert(&program, owner, requirement, first_row);
        assert_eq!(
            slot_satisfier(
                &rows,
                &program,
                owner,
                requirement,
                carrier,
                first_application,
                None,
            ),
            Some(top.handle_at(1))
        );

        // Even equal type applications may select different operation entries.
        let mut first_operation =
            SlotCarrierRow::from_scope(&program, &first, carrier, top.handle_at(1)).unwrap();
        first_operation.alias = Some("First".to_owned());
        let mut second_operation =
            SlotCarrierRow::from_scope(&program, &first, carrier, top.handle_at(2)).unwrap();
        second_operation.alias = Some("Second".to_owned());
        rows.insert(&program, owner, operation, first_operation);
        rows.insert(&program, owner, operation, second_operation);
        let mut anchor =
            SlotCarrierRow::from_scope(&program, &first, carrier, top.handle_at(1)).unwrap();
        anchor.alias = Some("First".to_owned());
        let mut other_law =
            SlotCarrierRow::from_scope(&program, &first, carrier, top.handle_at(2)).unwrap();
        other_law.alias = Some("Second".to_owned());
        rows.insert(&program, owner, requirement, other_law);
        assert!(!law_has_operations(
            &rows,
            &program,
            owner,
            requirement,
            &anchor,
            &[(operation, top.handle_at(1))]
        ));
        rows.insert(
            &program,
            owner,
            requirement,
            SlotCarrierRow::from_scope(&program, &first, carrier, top.handle_at(1))
                .map(|mut row| {
                    row.alias = Some("First".to_owned());
                    row
                })
                .unwrap(),
        );
        assert!(law_has_operations(
            &rows,
            &program,
            owner,
            requirement,
            &anchor,
            &[(operation, top.handle_at(1))]
        ));
    }
}
