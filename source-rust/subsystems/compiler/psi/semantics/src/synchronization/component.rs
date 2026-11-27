//! Put every cell observed by a selected export in the importing program's type coordinates.

use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, NominalDeclarationIdentity, StructuralPathSegment,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule, find_structural_type,
};

use super::{SpinWait, selected_spin_wait};

/// The caller has matched the boundary signature, argument order and exact
/// provider selection. Read the exported body from that artifact. Every field
/// along its path must denote the same declaration in the receiver.
pub fn component_spin_wait(
    receiver: &TerminalModule,
    receiving: &BoundaryMachineDeclaration,
    producer: &TerminalModule,
    candidate: MachineId,
) -> Option<SpinWait> {
    let body = producer.machine(candidate)?;
    selected_spin_wait(producer, candidate, |_| None)?.map_subjects(|subject| {
        let source = body.structural_parameters.get(subject.parameter)?;
        let target = receiving.structural_parameters.get(subject.parameter)?;
        if source.access != target.access {
            return None;
        }
        subject.path = translate_cell(
            producer,
            source.structural_type,
            receiver,
            target.structural_type,
            &subject.path,
        )?;
        Some(())
    })
}

fn translate_cell(
    producer: &TerminalModule,
    source_root: StructuralTypeId,
    receiver: &TerminalModule,
    target_root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<Vec<StructuralPathSegment>> {
    let same_declaration = |left: TerminalIdentityIndex, right: TerminalIdentityIndex| {
        NominalDeclarationIdentity::new(left).same_identity_as(
            &producer.identities,
            &NominalDeclarationIdentity::new(right),
            &receiver.identities,
        )
    };
    let mut translated = Vec::with_capacity(path.len());
    let mut source = source_root;
    let mut target = target_root;
    for (position, segment) in path.iter().enumerate() {
        let left = find_structural_type(&producer.types.structural_types, source)?;
        let right = find_structural_type(&receiver.types.structural_types, target)?;
        match segment {
            StructuralPathSegment::Field(identity) => {
                if !same_declaration(left.identity, right.identity)
                    || !same_declaration(left.declaration_identity, right.declaration_identity)
                {
                    return None;
                }
                let field =
                    crate::exactly_one::exactly_one(left.shape.direct_fields()?.iter(), |f| {
                        f.identity == *identity
                    })?;
                let destination = crate::exactly_one::exactly_one(
                    right.shape.direct_fields()?.iter(),
                    |other| {
                        same_declaration(field.identity, other.identity)
                            && same_declaration(
                                field.declaration_identity,
                                other.declaration_identity,
                            )
                    },
                )?;
                if field.relevance.is_erased() || destination.relevance.is_erased() {
                    return None;
                }
                if position + 1 < path.len() {
                    source = field.field_type.as_structural().or(field.inline_carrier)?;
                    target = destination
                        .field_type
                        .as_structural()
                        .or(destination.inline_carrier)?;
                }
                translated.push(StructuralPathSegment::Field(destination.identity));
            }
            StructuralPathSegment::FixedIndex(index) => {
                let (
                    StructuralTypeShape::FixedArray {
                        element: a,
                        length: a_len,
                    },
                    StructuralTypeShape::FixedArray {
                        element: b,
                        length: b_len,
                    },
                ) = (&left.shape, &right.shape)
                else {
                    return None;
                };
                let extent = a_len.literal()?;
                if b_len.literal()? != extent || *index >= extent {
                    return None;
                }
                source = *a;
                target = *b;
                translated.push(StructuralPathSegment::FixedIndex(*index));
            }
            _ => return None,
        }
    }
    let observed =
        crate::atomic_cell_type(producer.types.structural_types.iter(), source_root, path)?;
    let imported = crate::atomic_cell_type(
        receiver.types.structural_types.iter(),
        target_root,
        &translated,
    )?;
    (observed == imported).then_some(translated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{AtomicCellType, StructuralFieldId};
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralMultiplicity,
        StructuralRecordStorage, StructuralTypeDeclaration, TerminalIdentityTableBuilder,
    };

    fn signal(
        prior_identity: Option<&str>,
        declaration: &str,
    ) -> (TerminalModule, StructuralTypeId, Vec<StructuralPathSegment>) {
        let mut names = TerminalIdentityTableBuilder::default();
        if let Some(identity) = prior_identity {
            names.intern(identity);
        }
        let root = StructuralTypeId::new(1).unwrap();
        let word = names.intern("word");
        let field = StructuralFieldDeclaration {
            declaration_identity: names.intern(declaration),
            inline_carrier: None,
            id: StructuralFieldId::new(1).unwrap(),
            identity: word,
            relevance: BindingRelevance::Relevant,
            field_type: AtomicCellType::Boolean.into(),
            storage: None,
        };
        let mut module = TerminalModule::declarations();
        module
            .types
            .structural_types
            .push(StructuralTypeDeclaration {
                declaration_identity: names.intern("signals::Signal"),
                identity: names.intern("Signal"),
                id: root,
                quotient_carrier: None,
                owns_linear_claim: false,
                multiplicity: StructuralMultiplicity::Unrestricted,
                no_code_affine_cleanup: true,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                shape: StructuralTypeShape::Record {
                    fields: vec![field],
                    storage: StructuralRecordStorage::Packed,
                },
                scalar_restriction: None,
                requires: Vec::new(),
            });
        module.identities = names.build();
        (module, root, vec![StructuralPathSegment::Field(word)])
    }

    #[test]
    fn independent_cell_paths_match_declarations_across_different_identity_tables() {
        let (producer, source, path) = signal(None, "signals::Signal.word");
        let (receiver, destination, imported) = signal(Some("local"), "signals::Signal.word");
        assert_ne!(path, imported);
        assert_eq!(
            translate_cell(&producer, source, &receiver, destination, &path),
            Some(imported)
        );

        let (wrong_owner, destination, same_coordinates) = signal(None, "other::Signal.word");
        assert_eq!(same_coordinates, path);
        assert_eq!(
            translate_cell(&producer, source, &wrong_owner, destination, &path),
            None
        );
    }
}
