//! Transfer a constructor operand into the concrete field coordinates of its public package.
use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_location_type;

impl DescriptorFlowStep {
    /// A nominal member has a stable declaration, but its field ID belongs to one
    /// storage row. Copy each subtree to the coordinates used by the receiving row.
    /// Whole identical subtrees retain their ordinary value and membership facts.
    pub(super) fn copy_package_field(
        &mut self,
        types: &[StructuralTypeDeclaration],
        source: (DescriptorLocation, StructuralTypeId),
        destination: (DescriptorLocation, StructuralTypeId),
    ) -> Option<()> {
        let (source, source_root) = source;
        let (destination, destination_root) = destination;
        let from = descriptor_location_type(types, source_root, &source.path);
        let to = descriptor_location_type(types, destination_root, &destination.path);
        if from == to {
            self.copy_value(source, destination);
            return Some(());
        }
        let mut paths = Vec::new();
        package_field_paths(types, from?, to?, &mut Vec::new(), &mut paths)?;
        for (input, output) in paths {
            let mut from = source.clone();
            from.path.extend(input);
            let mut to = destination.clone();
            to.path.extend(output);
            self.copy_value(from, to);
        }
        Some(())
    }
}

type FieldPathPair = (Vec<DescriptorPathSegment>, Vec<DescriptorPathSegment>);

/// Return corresponding subtrees in the concrete and public storage rows. Package
/// substitution checks their types and contracts. This walk only locates the copies.
fn package_field_paths(
    types: &[StructuralTypeDeclaration],
    actual: StructuralTypeId,
    public: StructuralTypeId,
    ancestors: &mut Vec<StructuralTypeId>,
    paths: &mut Vec<FieldPathPair>,
) -> Option<()> {
    use crate::StructuralDeclarations;
    if actual == public {
        paths.push((Vec::new(), Vec::new()));
        return Some(());
    }
    if ancestors.contains(&public) {
        return None;
    }
    ancestors.push(public);
    let from = types.iter().unique(actual)?;
    let to = types.iter().unique(public)?;
    match (&from.shape, &to.shape) {
        (
            StructuralTypeShape::Record { fields: inputs, .. },
            StructuralTypeShape::Record {
                fields: outputs, ..
            },
        ) => {
            if from.declaration_identity.is_absent()
                || from.declaration_identity != to.declaration_identity
                || inputs.len() != outputs.len()
            {
                return None;
            }
            for (input, output) in inputs.iter().zip(outputs) {
                if input.declaration_identity.is_absent()
                    || input.declaration_identity != output.declaration_identity
                    || input.relevance != output.relevance
                {
                    return None;
                }
                if output.relevance.is_erased() {
                    continue;
                }
                let start = paths.len();
                match (
                    input.field_type.as_structural(),
                    output.field_type.as_structural(),
                ) {
                    (Some(a), Some(b)) => package_field_paths(types, a, b, ancestors, paths)?,
                    (None, None) => paths.push((Vec::new(), Vec::new())),
                    _ => return None,
                }
                for (a, b) in &mut paths[start..] {
                    a.insert(0, DescriptorPathSegment::Field(input.id));
                    b.insert(0, DescriptorPathSegment::Field(output.id));
                }
            }
        }
        (
            StructuralTypeShape::FixedArray { element: a, length },
            StructuralTypeShape::FixedArray {
                element: b,
                length: size,
            },
        ) if length == size => {
            let mut elements = Vec::new();
            package_field_paths(types, *a, *b, ancestors, &mut elements)?;
            if elements.iter().all(|(a, b)| a == b) {
                paths.push((Vec::new(), Vec::new()));
            } else {
                // Use the same finite expansion admitted by descriptor replay.
                let count = usize::try_from(length.literal()?).ok()?;
                if count
                    .checked_mul(elements.len())?
                    .checked_add(paths.len())?
                    > 4096
                {
                    return None;
                }
                for index in 0..count {
                    for (input, output) in &elements {
                        let prefix = DescriptorPathSegment::FixedIndex(index as u64);
                        paths.push((
                            std::iter::once(prefix)
                                .chain(input.iter().copied())
                                .collect(),
                            std::iter::once(prefix)
                                .chain(output.iter().copied())
                                .collect(),
                        ));
                    }
                }
            }
        }
        // A reference stores its descriptor. Its pointee is separate storage.
        (StructuralTypeShape::Reference { .. }, StructuralTypeShape::Reference { .. }) => {
            paths.push((Vec::new(), Vec::new()))
        }
        _ => return None,
    }
    ancestors.pop();
    Some(())
}
