//! Put every cell observed by a selected export in the importing program's type coordinates.

use semantic_vocabulary::{MachineId, StructuralTypeId};
use terminal_psi::{
    BoundaryMachineDeclaration, NominalDeclarationIdentity, StructuralTypeShape,
    TerminalIdentityIndex, TerminalModule, find_structural_type,
};

use super::private_cells::normal_effects;
use super::spin::{Operand, Subject, type_path};
use super::{NormalEffects, SpinWait, selected_spin_wait};
use terminal_psi::StructuralPathSegment as Segment;

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
    selected_spin_wait(producer, candidate, |_| None)?
        .map_subjects(|subject| translate_subject(producer, body, receiver, receiving, subject))
}

/// Translate only events established by the exact selected exported body.
pub fn component_normal_effects(
    receiver: &TerminalModule,
    receiving: &BoundaryMachineDeclaration,
    producer: &TerminalModule,
    candidate: MachineId,
) -> Option<NormalEffects> {
    let body = producer.machine(candidate)?;
    let mut effects = if let Some(result) = body.result.structural() {
        let terminal_psi::BoundaryMachineResult::Structural(target) = &receiving.result else {
            return None;
        };
        let mut effects = NormalEffects::returned_guard(producer, body)?;
        for reference in &mut effects.references {
            let source = reference.source.fixed_path()?;
            let formal = body.structural_parameters.get(reference.source.parameter)?;
            let actual = receiving
                .structural_parameters
                .get(reference.source.parameter)?;
            if formal.access != actual.access
                || formal.access == terminal_psi::StructuralAccess::Owned
            {
                return None;
            }
            reference_matches(
                producer,
                result.structural_type,
                &reference.field,
                formal.structural_type,
                &source,
            )?;
            reference.field = translate_path(
                producer,
                result.structural_type,
                receiver,
                target.structural_type,
                &reference.field,
            )?;
            reference.source.path = translate_path(
                producer,
                formal.structural_type,
                receiver,
                actual.structural_type,
                &source,
            )?;
            reference_matches(
                receiver,
                target.structural_type,
                &reference.field,
                actual.structural_type,
                &reference.source.path,
            )?;
        }
        effects
    } else {
        normal_effects(producer, candidate, |_| None)?
    };
    effects.map_operands(|operand| {
        if let Operand::Parameter {
            position,
            scalar_type,
        } = operand
        {
            (body.parameters.get(position)?.scalar_type == scalar_type
                && *receiving.scalar_parameters.get(position)? == scalar_type)
                .then_some(())?;
        }
        Some(operand)
    })?;
    effects
        .map_subjects(|subject| translate_subject(producer, body, receiver, receiving, subject))?;
    Some(effects)
}

fn reference_matches(
    module: &TerminalModule,
    result: StructuralTypeId,
    field: &[Segment],
    parameter: StructuralTypeId,
    path: &[Segment],
) -> Option<()> {
    let types = &module.types.structural_types;
    let descriptor = crate::runtime_structural_path_tip(types.iter(), result, field)?;
    let StructuralTypeShape::Reference {
        referent,
        access: terminal_psi::StructuralAccess::SharedBorrow,
    } = descriptor.shape
    else {
        return None;
    };
    (crate::runtime_structural_path_tip(types.iter(), parameter, path)?.id == referent)
        .then_some(())
}

fn translate_subject(
    producer: &TerminalModule,
    body: &terminal_psi::TerminalMachine,
    receiver: &TerminalModule,
    receiving: &BoundaryMachineDeclaration,
    subject: &mut Subject,
) -> Option<()> {
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
}

fn translate_cell(
    producer: &TerminalModule,
    source_root: StructuralTypeId,
    receiver: &TerminalModule,
    target_root: StructuralTypeId,
    path: &[Segment],
) -> Option<Vec<Segment>> {
    let translated = translate_path(producer, source_root, receiver, target_root, path)?;
    let observed = crate::atomic_cell_type(
        producer.types.structural_types.iter(),
        source_root,
        &type_path(path),
    )?;
    let imported = crate::atomic_cell_type(
        receiver.types.structural_types.iter(),
        target_root,
        &type_path(&translated),
    )?;
    (observed == imported).then_some(translated)
}

fn translate_path(
    producer: &TerminalModule,
    source_root: StructuralTypeId,
    receiver: &TerminalModule,
    target_root: StructuralTypeId,
    path: &[Segment],
) -> Option<Vec<Segment>> {
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
            Segment::Field(identity) => {
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
                translated.push(Segment::Field(destination.identity));
            }
            Segment::FixedIndex(_) | Segment::FixedRange { start: 0, .. } => {
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
                if b_len.literal()? != extent
                    || match segment {
                        Segment::FixedIndex(index) => *index >= extent,
                        Segment::FixedRange {
                            start: 0,
                            end: expected,
                        } => *expected != extent || extent == 0,
                        _ => return None,
                    }
                {
                    return None;
                }
                source = *a;
                target = *b;
                translated.push(segment.clone());
            }
            _ => return None,
        }
    }
    Some(translated)
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
    ) -> (TerminalModule, StructuralTypeId, Vec<Segment>) {
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
        (module, root, vec![Segment::Field(word)])
    }

    #[test]
    fn imported_array_subjects_require_the_same_extent_and_field_declaration() {
        let array = |module: &mut TerminalModule, element, length| {
            let mut declaration = module.structural_type(element).unwrap().clone();
            declaration.id = StructuralTypeId::new(2).unwrap();
            declaration.shape = StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            };
            let id = declaration.id;
            module.types.structural_types.push(declaration);
            id
        };
        let (mut producer, element, fields) = signal(None, "signals::Signal.word");
        let root = array(&mut producer, element, 4);
        let selector = Segment::FixedRange { start: 0, end: 4 };
        let at = |mut fields: Vec<Segment>| {
            fields.insert(0, selector.clone());
            fields
        };
        let source = at(fields);
        for (extent, matches) in [(4, true), (3, false)] {
            let (mut destination, element, fields) = signal(Some("local"), "signals::Signal.word");
            let target = array(&mut destination, element, extent);
            assert_eq!(
                translate_cell(&producer, root, &destination, target, &source),
                matches.then(|| at(fields))
            );
        }
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
