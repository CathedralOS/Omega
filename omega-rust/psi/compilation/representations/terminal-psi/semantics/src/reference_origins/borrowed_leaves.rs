//! Returned borrow contracts describe owned descriptor leaves, never the
//! storage behind them. Root canonical views obtain access from the result
//! occurrence; nested views must retain their own declared access. An affine
//! aggregate can contain both shared and exclusive leaves.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralFieldDeclaration, StructuralMultiplicity,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

pub enum BorrowedResultLeafKind {
    Reference(StructuralTypeId),
    View,
}

pub struct BorrowedResultLeaf {
    pub path: Vec<StructuralPathSegment>,
    pub access: StructuralAccess,
    pub kind: BorrowedResultLeafKind,
}

/// Complete owned borrow roster, bounded by the public contract's size. A
/// reference and a view each terminate the walk: their backing is borrowed,
/// regardless of whether it contains further descriptors.
pub fn borrowed_result_leaves<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    multiplicity: StructuralMultiplicity,
    maximum_leaves: usize,
) -> Result<Vec<BorrowedResultLeaf>, &'static str> {
    let mut roster = BorrowedResultRoster {
        types,
        multiplicity,
        maximum_leaves,
        active: Vec::new(),
        leaves: Vec::new(),
    };
    roster.visit(root, &mut Vec::new())?;
    roster
        .leaves
        .sort_by(|left, right| left.path.cmp(&right.path));
    Ok(roster.leaves)
}

struct BorrowedResultRoster<I> {
    types: I,
    multiplicity: StructuralMultiplicity,
    maximum_leaves: usize,
    active: Vec<StructuralTypeId>,
    leaves: Vec<BorrowedResultLeaf>,
}

impl<'types, I: Iterator<Item = &'types StructuralTypeDeclaration> + Clone>
    BorrowedResultRoster<I>
{
    fn push(
        &mut self,
        path: &[StructuralPathSegment],
        access: StructuralAccess,
        kind: BorrowedResultLeafKind,
    ) -> Result<(), &'static str> {
        if self.leaves.len() == self.maximum_leaves {
            return Err("dynamic result has more borrowed leaves than public regions");
        }
        if access == StructuralAccess::Owned {
            return Err("dynamic borrowed result leaf has owned access");
        }
        self.leaves.push(BorrowedResultLeaf {
            path: path.to_vec(),
            access,
            kind,
        });
        Ok(())
    }

    fn visit(
        &mut self,
        root: StructuralTypeId,
        path: &mut Vec<StructuralPathSegment>,
    ) -> Result<(), &'static str> {
        if self.active.contains(&root) {
            return Err("dynamic result contains a cyclic owned type");
        }
        let mut declarations = self.types.clone().filter(|row| row.id == root);
        let declaration = declarations
            .next()
            .ok_or("dynamic result has an unknown structural type")?;
        if declarations.next().is_some() {
            return Err("dynamic result has an ambiguous structural type");
        }
        match &declaration.shape {
            StructuralTypeShape::Reference { access, .. } => {
                return self.push(path, *access, BorrowedResultLeafKind::Reference(root));
            }
            StructuralTypeShape::ElementView { access, .. }
            | StructuralTypeShape::ByteSequence(
                ByteSequenceCarrier::BorrowedView { access }
                | ByteSequenceCarrier::BoundedView { access, .. },
            ) => {
                let access = if path.is_empty() {
                    crate::descriptor_results::descriptor_value_access(
                        self.types.clone(),
                        root,
                        self.multiplicity,
                    )
                    .ok_or("dynamic result descriptor has incompatible access and multiplicity")?
                } else {
                    access.ok_or("stored result descriptor has no declared access")?
                };
                return self.push(path, access, BorrowedResultLeafKind::View);
            }
            _ => {}
        }
        self.active.push(root);
        match &declaration.shape {
            StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
                // Visit one element before multiplying its roster. Plain arrays
                // cost one type walk; borrowed arrays cannot expand beyond the
                // declared contract even with adversarial nested extents.
                let first = self.leaves.len();
                path.push(StructuralPathSegment::FixedIndex(0));
                self.visit(*element, path)?;
                path.pop();
                let per_element = self.leaves.len() - first;
                if per_element != 0 {
                    let count = usize::try_from(*length)
                        .ok()
                        .and_then(|length| length.checked_mul(per_element))
                        .and_then(|count| first.checked_add(count))
                        .ok_or("dynamic result borrowed leaf count overflows")?;
                    if count > self.maximum_leaves {
                        return Err("dynamic result has more borrowed leaves than public regions");
                    }
                    for index in 1..*length {
                        path.push(StructuralPathSegment::FixedIndex(index));
                        self.visit(*element, path)?;
                        path.pop();
                    }
                }
            }
            StructuralTypeShape::Record { fields, .. }
            | StructuralTypeShape::Mixed { fields, .. } => self.fields(fields, path)?,
            _ => {}
        }
        if let StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } =
            &declaration.shape
        {
            for case in cases {
                path.push(StructuralPathSegment::Case(case.id));
                self.fields(&case.fields, path)?;
                path.pop();
            }
        }
        self.active.pop();
        Ok(())
    }

    fn fields(
        &mut self,
        fields: &[StructuralFieldDeclaration],
        path: &mut Vec<StructuralPathSegment>,
    ) -> Result<(), &'static str> {
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            path.push(StructuralPathSegment::Field(field.identity));
            if let Some(child) = field.field_type.as_structural() {
                self.visit(child, path)?;
            } else if let Some(
                ByteSequenceCarrier::BorrowedView { access }
                | ByteSequenceCarrier::BoundedView { access, .. },
            ) = field.field_type.byte_sequence_carrier()
            {
                self.push(
                    path,
                    access.ok_or("stored result byte view has no declared access")?,
                    BorrowedResultLeafKind::View,
                )?;
            }
            path.pop();
        }
        Ok(())
    }
}
