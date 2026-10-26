//! Membership retention can publish only domains in the closed catalog.
//! Compute their conservative type-graph reachability once, rather than replay
//! whole-machine payload histories for each borrow of an unrelated plain type.
//! This is producer query planning, never evidence: declared rows still travel
//! through the ordinary path and independent Terminal validation is unchanged.
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};

pub(super) struct MembershipReachability {
    rows: Vec<(StructuralTypeId, bool)>,
}
impl MembershipReachability {
    pub(super) fn new(
        types: &[StructuralTypeDeclaration],
        carriers: impl IntoIterator<Item = StructuralTypeId>,
    ) -> Self {
        let carriers: Vec<_> = carriers.into_iter().collect();
        // Inline scalar fields need not name a structural declaration. Any
        // primitive domain keeps their query on the full path conservatively.
        let scalar_domains = carriers.iter().any(|carrier| {
            types.iter().any(|declaration| {
                declaration.id == *carrier
                    && matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_))
            })
        });
        let mut result = Self {
            rows: types
                .iter()
                .map(|declaration| {
                    (
                        declaration.id,
                        carriers.contains(&declaration.id)
                            || declaration.quotient_carrier.is_some()
                            || declaration.scalar_restriction.is_some(),
                    )
                })
                .collect(),
        };
        loop {
            let mut changed = false;
            for (index, declaration) in types.iter().enumerate() {
                if result.rows[index].1 {
                    continue;
                }
                let fields = |fields: &[StructuralFieldDeclaration]| {
                    fields
                        .iter()
                        .filter(|field| !field.relevance.is_erased())
                        .any(|field| match field.field_type {
                            StructuralFieldType::Structural(child) => result.may_contain(child),
                            StructuralFieldType::Scalar(_) | StructuralFieldType::IeeeFloat(_) => {
                                scalar_domains
                            }
                            StructuralFieldType::Erased { .. } => false,
                            // Restricted or inline byte leaves can supply memberships
                            // without a child declaration. Preserve their full query.
                            StructuralFieldType::Atomic(_)
                            | StructuralFieldType::AtomicBoundedInteger(_)
                            | StructuralFieldType::BoundedInteger(_)
                            | StructuralFieldType::RefinedScalar(_)
                            | StructuralFieldType::ByteSequence(_)
                            | StructuralFieldType::PredicateByteSequence { .. } => true,
                        })
                };
                let reachable = match &declaration.shape {
                    StructuralTypeShape::Reference { referent, .. } => {
                        result.may_contain(*referent)
                    }
                    StructuralTypeShape::FixedArray { element, .. } => result.may_contain(*element),
                    // Views can select backing whose outer carrier is absent
                    // from their element type. Keep their storage-history query.
                    StructuralTypeShape::ElementView { .. } => true,
                    StructuralTypeShape::Record { fields: rows, .. } => fields(rows),
                    StructuralTypeShape::Sum { cases } => {
                        cases.iter().any(|case| fields(&case.fields))
                    }
                    StructuralTypeShape::Mixed {
                        fields: rows,
                        cases,
                    } => fields(rows) || cases.iter().any(|case| fields(&case.fields)),
                    StructuralTypeShape::PrimitiveScalar(_) => scalar_domains,
                    StructuralTypeShape::ByteSequence(_) => true,
                    StructuralTypeShape::BindingCarrier => false,
                };
                if reachable {
                    result.rows[index].1 = true;
                    changed = true;
                }
            }
            if !changed {
                return result;
            }
        }
    }
    pub(super) fn may_contain(&self, identity: StructuralTypeId) -> bool {
        let mut rows = self.rows.iter().filter(|row| row.0 == identity);
        // Missing or ambiguous declarations cannot justify omitting a query.
        let Some(row) = rows.next() else {
            return true;
        };
        rows.next().is_some() || row.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(value: u64) -> StructuralTypeId {
        StructuralTypeId::new(value).unwrap()
    }
    fn declaration(value: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: id(value),
            identity: terminal_psi::TerminalIdentityIndex(0),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        }
    }
    #[test]
    fn domain_reachability_follows_children_and_conservatively_keeps_unknowns() {
        let types = vec![
            declaration(1, StructuralTypeShape::BindingCarrier),
            declaration(
                2,
                StructuralTypeShape::FixedArray {
                    element: id(1),
                    length: 4,
                },
            ),
            declaration(3, StructuralTypeShape::BindingCarrier),
            declaration(
                4,
                StructuralTypeShape::Reference {
                    referent: id(2),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
            ),
            declaration(
                5,
                StructuralTypeShape::FixedArray {
                    element: id(5),
                    length: 1,
                },
            ),
            declaration(
                6,
                StructuralTypeShape::Reference {
                    referent: id(9),
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
            ),
        ];
        let unrelated = MembershipReachability::new(&types, [id(3)]);
        assert!(!unrelated.may_contain(id(4)));
        assert!(!unrelated.may_contain(id(5)));
        assert!(unrelated.may_contain(id(6)));
        let nested = MembershipReachability::new(&types, [id(1)]);
        assert!(nested.may_contain(id(2)));
        assert!(nested.may_contain(id(4)));
        assert!(!nested.may_contain(id(3)));
        assert!(nested.may_contain(id(99)));
    }
}
