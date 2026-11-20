//! Move the returned Build receiver and continuation homes to their admitted successors.
//! Both admitted declarations are present at the handoff. Translate storage
//! coordinates by their source declarations while observing the returned instance.

use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use terminal_interpreter::{
    TerminalBoundaryStorageValue, TerminalInstance, TerminalStorageContents,
    TerminalStorageObservationValue,
};
use terminal_psi::{
    NominalDeclarationIdentity, StructuralFieldDeclaration, StructuralFieldType,
    StructuralTypeShape, TerminalIdentityIndex, TerminalModule,
};

pub(super) fn transfer(
    instance: &TerminalInstance<'_, '_>,
    destination: &TerminalModule,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> Result<Vec<terminal_interpreter::TerminalStructuralContents>, String> {
    let entry = instance.program().entry().map_err(super::describe)?;
    if entry.structural_parameters.len() != parameters.len() {
        return Err("Build continuation changed the number of storage homes".into());
    }
    entry
        .structural_parameters
        .iter()
        .zip(parameters)
        .map(|(old, new)| {
            if old.position != new.position {
                return Err("Build continuation changed the order of its storage homes".into());
            }
            let argument_index = old.position;
            let mut transfer = Transfer {
                instance,
                argument_index,
                source: instance.program().module(),
                destination,
                from: Vec::new(),
                to: Vec::new(),
                values: Vec::new(),
            };
            transfer.structural(old.structural_type, new.structural_type)?;
            transfer
                .values
                .sort_by(|left, right| left.path.cmp(&right.path));
            Ok(terminal_interpreter::TerminalStructuralContents {
                argument_index,
                values: transfer.values,
            })
        })
        .collect()
}

/// Read every live copy against the old issuer table before translating any
/// marker to the new program's already matched declaration.
pub(super) fn registration_bindings(
    instance: &TerminalInstance<'_, '_>,
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    positions: impl Iterator<Item = u32>,
    requests: &mut crate::host::requests::CarriedRequests,
) -> Result<Vec<terminal_interpreter::TerminalProjectedHostAtom>, String> {
    let mut bindings = Vec::new();
    for position in positions {
        let Some(mut value) = instance
            .borrowed_entry_host_atom(position, &[])
            .map_err(super::describe)?
        else {
            continue;
        };
        let index = requests.registration_position(&value)?;
        let parameter = parameters
            .get(position as usize)
            .filter(|parameter| parameter.position == position)
            .ok_or("Build continuation lost a registration home")?;
        value.structural_type = parameter.structural_type;
        let binding = terminal_interpreter::TerminalProjectedHostAtom {
            argument_index: position,
            path: Vec::new(),
            value,
        };
        bindings.push((index, binding));
    }
    requests.retain_markers(
        bindings
            .iter()
            .map(|(index, binding)| (*index, &binding.value)),
    );
    Ok(bindings.into_iter().map(|(_, binding)| binding).collect())
}

struct Transfer<'a, 'host, 'program> {
    instance: &'a TerminalInstance<'host, 'program>,
    argument_index: u32,
    source: &'a TerminalModule,
    destination: &'a TerminalModule,
    from: Vec<Path>,
    to: Vec<Path>,
    values: Vec<TerminalBoundaryStorageValue>,
}

fn same_declaration(
    source: &TerminalModule,
    old: TerminalIdentityIndex,
    destination: &TerminalModule,
    new: TerminalIdentityIndex,
) -> bool {
    NominalDeclarationIdentity::new(old).same_identity_as(
        &source.identities,
        &NominalDeclarationIdentity::new(new),
        &destination.identities,
    )
}

fn stored(field: &&StructuralFieldDeclaration) -> bool {
    !field.relevance.is_erased() && !matches!(field.field_type, StructuralFieldType::Erased { .. })
}

fn receiving_field<'a>(
    source: &TerminalModule,
    old: &StructuralFieldDeclaration,
    destination: &TerminalModule,
    fields: &'a [StructuralFieldDeclaration],
) -> Result<&'a StructuralFieldDeclaration, String> {
    fields
        .iter()
        .filter(stored)
        .find(|field| {
            same_declaration(
                source,
                old.declaration_identity,
                destination,
                field.declaration_identity,
            )
        })
        .ok_or_else(|| "Build storage lost a field declaration across preparation".into())
}

impl Transfer<'_, '_, '_> {
    fn structural(&mut self, from: StructuralTypeId, to: StructuralTypeId) -> Result<(), String> {
        use StructuralTypeShape as Shape;
        let old = self
            .source
            .structural_type(from)
            .ok_or("Build storage lost its original type")?;
        let new = self
            .destination
            .structural_type(to)
            .ok_or("Build storage lost its receiving type")?;
        if !(old.declaration_identity.is_absent() && new.declaration_identity.is_absent()
            || same_declaration(
                self.source,
                old.declaration_identity,
                self.destination,
                new.declaration_identity,
            ))
            || !same_declaration(self.source, old.identity, self.destination, new.identity)
        {
            return Err("Build storage changed its declared type across preparation".into());
        }
        match (&old.shape, &new.shape) {
            (Shape::Record { fields: left, .. }, Shape::Record { fields: right, .. }) => {
                self.fields(left, right)
            }
            (Shape::Sum { cases: left }, Shape::Sum { cases: right }) => {
                let TerminalStorageObservationValue::Storage(TerminalStorageContents::Case(
                    selected,
                )) = self
                    .instance
                    .observe_borrowed_entry(self.argument_index, &self.from)
                    .map_err(super::describe)?
                else {
                    return Err("Build storage lost the selected case".into());
                };
                let old = left
                    .iter()
                    .find(|case| case.id == selected)
                    .ok_or("Build storage selected an undeclared case")?;
                let new = right
                    .iter()
                    .find(|case| {
                        same_declaration(
                            self.source,
                            old.declaration_identity,
                            self.destination,
                            case.declaration_identity,
                        )
                    })
                    .ok_or("Build storage lost the selected case declaration across preparation")?;
                self.push(TerminalStorageContents::Case(new.id));
                self.from.push(Path::Case(old.id));
                self.to.push(Path::Case(new.id));
                self.fields(&old.fields, &new.fields)?;
                self.from.pop();
                self.to.pop();
                Ok(())
            }
            (
                Shape::FixedArray {
                    element: left,
                    length,
                },
                Shape::FixedArray {
                    element: right,
                    length: next,
                },
            ) if length.literal() == next.literal() => {
                for index in 0..length
                    .literal()
                    .ok_or("Build storage requires a checked literal array extent")?
                {
                    self.from.push(Path::FixedIndex(index));
                    self.to.push(Path::FixedIndex(index));
                    self.structural(*left, *right)?;
                    self.from.pop();
                    self.to.pop();
                }
                Ok(())
            }
            (Shape::PrimitiveScalar(left), Shape::PrimitiveScalar(right)) if left == right => {
                self.leaf()
            }
            (Shape::ByteSequence(left), Shape::ByteSequence(right)) if left == right => self.leaf(),
            _ => Err("unsupported or changed Build storage shape across preparation".into()),
        }
    }

    fn fields(
        &mut self,
        left: &[StructuralFieldDeclaration],
        right: &[StructuralFieldDeclaration],
    ) -> Result<(), String> {
        if left.iter().filter(stored).count() != right.iter().filter(stored).count() {
            return Err(
                "Build storage changed the number of stored fields across preparation".into(),
            );
        }
        for old in left.iter().filter(stored) {
            let new = receiving_field(self.source, old, self.destination, right)?;
            self.from.push(Path::Field(old.id));
            self.to.push(Path::Field(new.id));
            match (&old.field_type, &new.field_type) {
                (StructuralFieldType::Structural(left), StructuralFieldType::Structural(right)) => {
                    self.structural(*left, *right)?
                }
                (left, right) if left == right => self.leaf()?,
                _ => return Err("Build storage changed a field's type across preparation".into()),
            }
            self.from.pop();
            self.to.pop();
        }
        Ok(())
    }

    fn leaf(&mut self) -> Result<(), String> {
        let contents = match self
            .instance
            .observe_borrowed_entry(self.argument_index, &self.from)
            .map_err(super::describe)?
        {
            TerminalStorageObservationValue::Bytes(bytes) => {
                let mut copied = vec![0; bytes.len()];
                bytes.copy_to(&mut copied).map_err(super::describe)?;
                TerminalStorageContents::Bytes(copied.into())
            }
            TerminalStorageObservationValue::Storage(
                contents @ (TerminalStorageContents::Bytes(_)
                | TerminalStorageContents::Scalar { .. }),
            ) => contents,
            _ => {
                return Err(
                    "Build storage cannot transfer this value as a scalar or byte sequence".into(),
                );
            }
        };
        self.push(contents);
        Ok(())
    }

    fn push(&mut self, contents: TerminalStorageContents) {
        self.values.push(TerminalBoundaryStorageValue {
            path: self.to.clone(),
            contents,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{ScalarType, StructuralFieldId};

    fn field(module: &mut TerminalModule, owner: &str, id: u64) -> StructuralFieldDeclaration {
        let mut names = std::mem::take(&mut module.identities).into_builder();
        let field = StructuralFieldDeclaration {
            identity: names.intern("disabled"),
            declaration_identity: names.intern(owner),
            id: StructuralFieldId::new(id).unwrap(),
            relevance: Default::default(),
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            storage: None,
            inline_carrier: None,
        };
        module.identities = names.build();
        field
    }

    #[test]
    fn a_resumed_field_uses_its_declaration_even_when_the_old_number_and_name_are_reused() {
        let mut old = TerminalModule::declarations();
        let previous = field(&mut old, "toolchain::BuildTests::disabled", 1);
        let mut new = TerminalModule::declarations();
        let other = field(&mut new, "foreign::BuildTests::disabled", 1);
        assert!(receiving_field(&old, &previous, &new, std::slice::from_ref(&other)).is_err());
        let expected = field(&mut new, "toolchain::BuildTests::disabled", 3);
        let fields = [other, expected];
        assert_eq!(
            receiving_field(&old, &previous, &new, &fields).unwrap(),
            &fields[1]
        );
    }
}
