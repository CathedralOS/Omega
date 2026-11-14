//! Checks by-value storage dependencies without following borrowed referents.
//!
//! Artifact and native validators share this graph so records, case payloads and
//! arrays have one containment rule. Sparse semantic identities are mapped onto
//! sorted declaration rows; traversal state is sized to the catalog, not its
//! largest identity. Explicit work stacks keep artifact depth off the Rust stack.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{StructuralTypeDeclaration, StructuralTypeShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageGraphError {
    MissingType(StructuralTypeId),
    DuplicateType(StructuralTypeId),
    Cycle(StructuralTypeId),
}

/// Borrows the declarations used by both cycle and attachment-storage checks.
pub struct StorageTypeGraph<'types> {
    rows: Vec<&'types StructuralTypeDeclaration>,
}

impl<'types> StorageTypeGraph<'types> {
    pub fn new(
        declarations: impl IntoIterator<Item = &'types StructuralTypeDeclaration>,
    ) -> Result<Self, StorageGraphError> {
        let mut rows: Vec<_> = declarations.into_iter().collect();
        rows.sort_unstable_by_key(|declaration| declaration.id);
        for pair in rows.windows(2) {
            if pair[0].id == pair[1].id {
                return Err(StorageGraphError::DuplicateType(pair[0].id));
            }
        }
        Ok(Self { rows })
    }

    fn row(&self, identity: StructuralTypeId) -> Result<usize, StorageGraphError> {
        self.rows
            .binary_search_by_key(&identity, |declaration| declaration.id)
            .map_err(|_| StorageGraphError::MissingType(identity))
    }

    /// Returns sorted identities of the storage owned by the supplied roots.
    /// References and element views do not make their referents owned storage.
    pub fn storage_reachable_from(
        &self,
        roots: impl IntoIterator<Item = StructuralTypeId>,
    ) -> Result<Vec<StructuralTypeId>, StorageGraphError> {
        let mut pending: Vec<_> = roots.into_iter().collect();
        let mut visited = vec![false; self.rows.len()];
        while let Some(identity) = pending.pop() {
            let row = self.row(identity)?;
            if visited[row] {
                continue;
            }
            visited[row] = true;
            for_each_owned_child(&self.rows[row].shape, |child| {
                pending.push(child);
                Ok(())
            })?;
        }
        Ok(self
            .rows
            .iter()
            .zip(visited)
            .filter_map(|(declaration, visited)| visited.then_some(declaration.id))
            .collect())
    }

    /// Rejects missing by-value children and cycles, including zero-length arrays.
    pub fn validate_acyclic(&self) -> Result<(), StorageGraphError> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Visit {
            Unseen,
            Active,
            Complete,
        }
        let mut visits = vec![Visit::Unseen; self.rows.len()];
        let mut pending = Vec::with_capacity(self.rows.len());
        for root_row in 0..self.rows.len() {
            if visits[root_row] == Visit::Complete {
                continue;
            }
            pending.push((self.rows[root_row].id, false));
            while let Some((identity, finishing)) = pending.pop() {
                let row = self.row(identity)?;
                if finishing {
                    visits[row] = Visit::Complete;
                    continue;
                }
                match visits[row] {
                    Visit::Complete => continue,
                    Visit::Active => return Err(StorageGraphError::Cycle(self.rows[row].id)),
                    Visit::Unseen => visits[row] = Visit::Active,
                }
                pending.push((identity, true));
                let children_start = pending.len();
                for_each_owned_child(&self.rows[row].shape, |child| {
                    pending.push((child, false));
                    Ok(())
                })?;
                // Preserve declaration-order depth-first diagnostics with a LIFO work stack.
                pending[children_start..].reverse();
            }
        }
        Ok(())
    }
}

fn for_each_owned_child(
    shape: &StructuralTypeShape,
    mut visit: impl FnMut(StructuralTypeId) -> Result<(), StorageGraphError>,
) -> Result<(), StorageGraphError> {
    let mut fields =
        |fields: &[terminal_psi::StructuralFieldDeclaration]| -> Result<(), StorageGraphError> {
            for field in fields {
                if let Some(child) = field.field_type.as_structural() {
                    visit(child)?;
                }
            }
            Ok(())
        };
    match shape {
        StructuralTypeShape::Record {
            fields: members, ..
        } => fields(members)?,
        StructuralTypeShape::Sum { cases } => {
            for case in cases {
                fields(&case.fields)?;
            }
        }
        StructuralTypeShape::Mixed {
            fields: members,
            cases,
        } => {
            fields(members)?;
            for case in cases {
                fields(&case.fields)?;
            }
        }
        StructuralTypeShape::FixedArray { element, .. } => visit(*element)?,
        StructuralTypeShape::PrimitiveScalar(_)
        | StructuralTypeShape::AtomicCell(_)
        | StructuralTypeShape::ByteSequence(_)
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ElementView { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{ArrayExtent, TerminalIdentityIndex};

    fn id(raw: u64) -> StructuralTypeId {
        StructuralTypeId::new(raw).unwrap()
    }

    fn declaration(raw: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: id(raw),
            identity: TerminalIdentityIndex(1),
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        }
    }

    fn array(raw: u64, element: u64) -> StructuralTypeDeclaration {
        declaration(
            raw,
            StructuralTypeShape::FixedArray {
                element: id(element),
                length: ArrayExtent::Literal(1),
            },
        )
    }

    #[test]
    fn sparse_shared_children_are_not_cycles_and_roots_limit_storage() {
        let rows = [
            array(900, 20),
            declaration(
                20,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            array(10, 20),
        ];
        let graph = StorageTypeGraph::new(&rows).unwrap();
        assert_eq!(graph.validate_acyclic(), Ok(()));
        assert_eq!(
            graph.storage_reachable_from([id(10)]),
            Ok(vec![id(10), id(20)])
        );
        assert_eq!(
            graph.storage_reachable_from([id(999)]),
            Err(StorageGraphError::MissingType(id(999)))
        );
    }

    #[test]
    fn borrowed_element_view_does_not_own_its_element() {
        let rows = [declaration(
            1,
            StructuralTypeShape::ElementView {
                element: id(1),
                access: None,
            },
        )];
        let graph = StorageTypeGraph::new(&rows).unwrap();
        assert_eq!(graph.validate_acyclic(), Ok(()));
        assert_eq!(graph.storage_reachable_from([id(1)]), Ok(vec![id(1)]));
    }

    #[test]
    fn records_and_case_payloads_share_the_owned_storage_rule() {
        use semantic_vocabulary::{StructuralCaseId, StructuralFieldId};
        use terminal_psi::{
            BindingRelevance, StructuralAccess, StructuralCaseDeclaration,
            StructuralFieldDeclaration, StructuralFieldType, StructuralRecordStorage,
        };
        let field = |child| StructuralFieldDeclaration {
            inline_carrier: None,
            id: StructuralFieldId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(id(child)),
            storage: None,
        };
        let case = |child| StructuralCaseDeclaration {
            id: StructuralCaseId::new(1).unwrap(),
            identity: TerminalIdentityIndex(1),
            fields: vec![field(child)],
        };
        let mut rows = [
            declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![field(2)],
                    storage: StructuralRecordStorage::Packed,
                },
            ),
            declaration(
                2,
                StructuralTypeShape::Sum {
                    cases: vec![case(3)],
                },
            ),
            declaration(
                3,
                StructuralTypeShape::Mixed {
                    fields: vec![field(4)],
                    cases: vec![case(5)],
                },
            ),
            declaration(4, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
            declaration(
                5,
                StructuralTypeShape::Reference {
                    referent: id(1),
                    access: StructuralAccess::SharedBorrow,
                },
            ),
        ];
        let graph = StorageTypeGraph::new(&rows).unwrap();
        assert_eq!(graph.validate_acyclic(), Ok(()));
        assert_eq!(
            graph.storage_reachable_from([id(1)]),
            Ok((1..=5).map(id).collect())
        );
        rows[4] = array(5, 1);
        assert_eq!(
            StorageTypeGraph::new(&rows).unwrap().validate_acyclic(),
            Err(StorageGraphError::Cycle(id(1)))
        );
    }

    #[test]
    fn missing_children_and_duplicate_identities_reject() {
        let rows = [array(1, 2)];
        assert_eq!(
            StorageTypeGraph::new(&rows).unwrap().validate_acyclic(),
            Err(StorageGraphError::MissingType(id(2)))
        );
        let rows = [array(1, 2), array(1, 3)];
        assert!(
            matches!(StorageTypeGraph::new(&rows), Err(StorageGraphError::DuplicateType(identity)) if identity == id(1))
        );
    }

    #[test]
    fn deep_graphs_use_an_explicit_stack_and_cycles_still_reject() {
        let mut rows: Vec<_> = (1..4096).map(|raw| array(raw, raw + 1)).collect();
        rows.push(declaration(
            4096,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        ));
        assert_eq!(
            StorageTypeGraph::new(&rows).unwrap().validate_acyclic(),
            Ok(())
        );
        rows[4095].shape = StructuralTypeShape::FixedArray {
            element: id(1),
            length: ArrayExtent::Literal(0),
        };
        assert_eq!(
            StorageTypeGraph::new(&rows).unwrap().validate_acyclic(),
            Err(StorageGraphError::Cycle(id(1)))
        );
    }
}
