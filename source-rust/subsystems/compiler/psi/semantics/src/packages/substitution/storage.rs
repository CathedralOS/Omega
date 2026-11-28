//! Check stored fields against the concrete representation of their package witness.
use super::*;

impl Substitution<'_> {
    pub(super) fn storage(
        &self,
        module: &TerminalModule,
        from: StructuralTypeId,
        to: StructuralTypeId,
        nested: &mut Vec<Comparison>,
    ) -> bool {
        use application_identity::type_identity::package_applications;
        let (Some(public), Some(actual)) =
            (module.structural_type(from), module.structural_type(to))
        else {
            return false;
        };
        // An abstract type may be named behind a reference, never stored inline.
        if self.witness(from).is_some() {
            return false;
        }
        let mut expected = public.clone();
        match (&public.shape, &actual.shape) {
            (
                StructuralTypeShape::Reference { referent, access },
                StructuralTypeShape::Reference {
                    referent: supplied,
                    access: mode,
                },
            ) => {
                if access != mode {
                    return false;
                }
                if let Some(witness) = self.witness(*referent) {
                    if witness != *supplied {
                        return false;
                    }
                } else if referent == supplied {
                    return from == to;
                } else {
                    nested.push(Comparison::Storage(*referent, *supplied));
                }
                expected.declaration_identity = actual.declaration_identity;
            }
            (
                StructuralTypeShape::FixedArray { element, length },
                StructuralTypeShape::FixedArray {
                    element: supplied,
                    length: extent,
                },
            ) => {
                if length != extent || (from != to && element == supplied) {
                    return false;
                }
                nested.push(Comparison::Storage(*element, *supplied));
            }
            (
                StructuralTypeShape::Record { fields, storage },
                StructuralTypeShape::Record {
                    fields: supplied,
                    storage: representation,
                },
            ) => {
                if !self
                    .public
                    .iter()
                    .any(|&witness| fields::nominal_uses(module, from, witness))
                {
                    return from == to;
                }
                let (Some(name), Some(application), Some(witnesses)) = (
                    module.identities.text(public.identity),
                    module.identities.text(actual.identity),
                    self.identities(module),
                ) else {
                    return false;
                };
                if public.declaration_identity.is_absent()
                    || public.declaration_identity != actual.declaration_identity
                    || !package_applications::substituted(name, application, &witnesses)
                    || storage != representation
                    || fields.len() != supplied.len()
                    || !crate::packages::requirements::same(
                        &public.requires,
                        &actual.requires,
                        fields,
                        supplied,
                    )
                {
                    return false;
                }
                expected.requires.clone_from(&actual.requires);
                for (field, supplied) in fields.iter().zip(supplied) {
                    if field.declaration_identity.is_absent()
                        || !Self::field(&field.field_type, &supplied.field_type, nested)
                    {
                        return false;
                    }
                    let mut expected = field.clone();
                    expected.id = supplied.id;
                    expected.field_type = supplied.field_type.clone();
                    if expected != *supplied {
                        return false;
                    }
                }
            }
            _ => return from == to,
        }
        expected.id = actual.id;
        expected.identity = actual.identity;
        expected.shape = actual.shape.clone();
        expected == *actual
    }
}
