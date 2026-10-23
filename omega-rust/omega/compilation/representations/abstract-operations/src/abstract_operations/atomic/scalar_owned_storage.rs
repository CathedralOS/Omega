//! Closed storage reach of a fresh owned value, without nominal-name rules.
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape};

pub(super) fn contains_only_scalars<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> bool {
    fn visit<'types>(
        types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
        root: StructuralTypeId,
        active: &mut Vec<StructuralTypeId>,
    ) -> bool {
        if active.contains(&root) {
            return false;
        }
        let mut matching = types.clone().filter(|declaration| declaration.id == root);
        let Some(declaration) = matching.next() else {
            return false;
        };
        if matching.next().is_some()
            || declaration.owns_linear_claim
            || declaration.quotient_carrier.is_some()
            || declaration.scalar_restriction.is_some()
        {
            return false;
        }
        // Descriptor-cleanup is disposal authority for a closed value that
        // may contain descriptors, not evidence that any descriptor exists.
        // Decide reach from every retained field instead.
        active.push(root);
        let mut field = |field: &terminal_psi::StructuralFieldDeclaration| {
            if field.storage.is_some() {
                return false;
            }
            if field.relevance.is_erased() {
                return matches!(field.field_type, StructuralFieldType::Erased { .. });
            }
            match field.field_type {
                StructuralFieldType::Scalar(_) => true,
                StructuralFieldType::Structural(child) => visit(types.clone(), child, active),
                // Dedicated atomic cells, views, routed fields and unknown
                // storage never become inert ordinary scalar snapshots.
                _ => false,
            }
        };
        let result = match &declaration.shape {
            StructuralTypeShape::PrimitiveScalar(_) => true,
            StructuralTypeShape::Record {
                fields,
                storage: terminal_psi::StructuralRecordStorage::Packed,
            } => fields.iter().all(&mut field),
            StructuralTypeShape::Sum { cases } => {
                cases.iter().flat_map(|case| &case.fields).all(&mut field)
            }
            StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .all(&mut field),
            StructuralTypeShape::FixedArray { element, .. } => {
                visit(types.clone(), *element, active)
            }
            _ => false,
        };
        active.pop();
        result
    }
    visit(types, root, &mut Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        OperationId, PlaceId, ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind,
    };
    use terminal_psi::{
        BindingRelevance, StructuralAccess, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
        TerminalIdentityIndex,
    };

    fn declaration() -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::Sum {
                cases: vec![StructuralCaseDeclaration {
                    id: StructuralCaseId::new(1).unwrap(),
                    identity: TerminalIdentityIndex(2),
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: TerminalIdentityIndex(3),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                        storage: None,
                    }],
                }],
            },
        }
    }

    fn may_alias(
        declaration: &StructuralTypeDeclaration,
        fresh: bool,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> bool {
        let query = PlaceId::new(1).unwrap();
        let value = PlaceId::new(2).unwrap();
        let places = vec![StructuralPlaceDeclaration {
            id: value,
            kind: if fresh {
                StructuralPlaceKind::OperationResult {
                    producer: OperationId::new(1).unwrap(),
                    structural_type: declaration.id,
                }
            } else {
                StructuralPlaceKind::Parameter { position: 1 }
            },
        }];
        let arrivals = vec![StructuralParameterDeclaration {
            place: query,
            position: 0,
            structural_type: declaration.id,
            access: StructuralAccess::SharedBorrow,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: vec![],
            projected_qualifications: vec![],
        }];
        super::super::atomic_projections_may_alias(
            &places,
            std::iter::once(arrivals.as_slice()),
            std::iter::once(declaration),
            query,
            &[],
            value,
            path,
        )
    }

    #[test]
    fn fresh_scalar_values_cannot_lend_separate_atomic_backing() {
        let mut declaration = declaration();
        declaration.descriptor_cleanup = true;
        assert!(contains_only_scalars(
            std::iter::once(&declaration),
            declaration.id
        ));
        assert!(!may_alias(&declaration, true, &[]));
        assert!(
            may_alias(&declaration, false, &[]),
            "a mere parameter name proves no fresh storage"
        );
        assert!(
            may_alias(
                &declaration,
                true,
                &[terminal_psi::StructuralPathSegment::Referent]
            ),
            "crossing a descriptor is never the scalar home"
        );
    }

    #[test]
    fn atomic_reference_and_unknown_payloads_retain_alias_uncertainty() {
        for mutation in 0..4 {
            let mut declaration = declaration();
            match mutation {
                0 => {
                    let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
                        unreachable!()
                    };
                    cases[0].fields[0].field_type =
                        StructuralFieldType::Atomic(ScalarType::Boolean);
                }
                1 => {
                    declaration.shape = StructuralTypeShape::Reference {
                        referent: declaration.id,
                        access: StructuralAccess::SharedBorrow,
                    }
                }
                2 => {
                    let StructuralTypeShape::Sum { cases } = &mut declaration.shape else {
                        unreachable!()
                    };
                    cases[0].fields[0].field_type =
                        StructuralFieldType::Structural(StructuralTypeId::new(99).unwrap());
                }
                3 => declaration.owns_linear_claim = true,
                _ => unreachable!(),
            }
            assert!(may_alias(&declaration, true, &[]), "mutation {mutation}");
        }
    }
    #[test]
    fn all_cases_and_closed_children_constrain_storage_reach() {
        let mut outer = declaration();
        let mut child = declaration();
        child.id = StructuralTypeId::new(2).unwrap();
        child.shape = StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean);
        let StructuralTypeShape::Sum { cases } = &mut outer.shape else {
            unreachable!()
        };
        let mut second = cases[0].clone();
        second.id = StructuralCaseId::new(2).unwrap();
        second.fields[0].field_type = StructuralFieldType::Structural(child.id);
        cases.push(second);
        assert!(contains_only_scalars(
            [&outer, &child].into_iter(),
            outer.id
        ));
        assert!(
            !contains_only_scalars([&outer].into_iter(), outer.id),
            "missing child"
        );
        assert!(
            !contains_only_scalars([&outer, &child, &child].into_iter(), outer.id),
            "ambiguous child"
        );
        child.shape = StructuralTypeShape::Reference {
            referent: outer.id,
            access: StructuralAccess::SharedBorrow,
        };
        assert!(
            !contains_only_scalars([&outer, &child].into_iter(), outer.id),
            "an inactive case can carry a reference"
        );
        child.shape = StructuralTypeShape::FixedArray {
            element: outer.id,
            length: 1,
        };
        assert!(
            !contains_only_scalars([&outer, &child].into_iter(), outer.id),
            "recursive graph is unknown"
        );
    }
}
