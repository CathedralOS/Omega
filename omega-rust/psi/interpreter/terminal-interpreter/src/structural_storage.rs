//! Storage keys order by opaque carrier identity, then path and field. A
//! subtree operation visits that identity's cells, never historical unrelated
//! carriers. Semantic path selection and stage-before-prune ordering are unchanged.
use crate::values::{
    StructuralByteSequenceRuntimeField, StructuralRuntimePlace, StructuralScalarRuntimeField,
};
use semantic_vocabulary::StructuralFieldId;
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included, Unbounded};

pub(crate) trait StorageKey: Ord + Clone {
    fn identity_start(identity: u64) -> Self;
    fn parent(&self) -> &StructuralRuntimePlace;
}
impl StorageKey for StructuralRuntimePlace {
    fn identity_start(opaque_identity: u64) -> Self {
        Self {
            opaque_identity,
            path: Vec::new(),
        }
    }
    fn parent(&self) -> &StructuralRuntimePlace {
        self
    }
}
impl StorageKey for StructuralScalarRuntimeField {
    fn identity_start(identity: u64) -> Self {
        Self::member(
            StructuralRuntimePlace::identity_start(identity),
            StructuralFieldId::new(1).unwrap(),
        )
    }
    fn parent(&self) -> &StructuralRuntimePlace {
        &self.parent
    }
}
impl StorageKey for StructuralByteSequenceRuntimeField {
    fn identity_start(identity: u64) -> Self {
        Self {
            parent: StructuralRuntimePlace::identity_start(identity),
            namespace: crate::values::StructuralFieldNamespace::Member,
            field: StructuralFieldId::new(1).unwrap(),
        }
    }
    fn parent(&self) -> &StructuralRuntimePlace {
        &self.parent
    }
}
pub(crate) fn cells<'a, K: StorageKey, V>(
    map: &'a BTreeMap<K, V>,
    root: &'a StructuralRuntimePlace,
) -> impl Iterator<Item = (&'a K, &'a V)> {
    let lower = Included(K::identity_start(root.opaque_identity));
    let upper = root
        .opaque_identity
        .checked_add(1)
        .map_or(Unbounded, |identity| Excluded(K::identity_start(identity)));
    map.range((lower, upper))
        .filter(move |(key, _)| key.parent().path.starts_with(&root.path))
}
pub(crate) fn prune<K: StorageKey, V>(map: &mut BTreeMap<K, V>, root: &StructuralRuntimePlace) {
    let keys: Vec<_> = cells(map, root).map(|(key, _)| key.clone()).collect();
    for key in keys {
        map.remove(&key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::StructuralFieldNamespace;
    use terminal_psi::StructuralPathSegment;
    #[test]
    fn byte_cells_keep_both_namespaces_under_empty_root() {
        let root = StructuralRuntimePlace::identity_start(u64::MAX);
        let mut map = BTreeMap::new();
        for namespace in [
            StructuralFieldNamespace::Member,
            StructuralFieldNamespace::CasePayload,
        ] {
            map.insert(
                StructuralByteSequenceRuntimeField {
                    parent: root.clone(),
                    namespace,
                    field: StructuralFieldId::new(1).unwrap(),
                },
                namespace,
            );
        }
        assert_eq!(cells(&map, &root).count(), 2);
        prune(&mut map, &root);
        assert!(map.is_empty());
    }
    #[test]
    fn selects_and_prunes_only_the_exact_subtree_at_identity_extremes() {
        let field = StructuralFieldId::new(1).unwrap();
        for identity in [0, u64::MAX] {
            let root = StructuralRuntimePlace {
                opaque_identity: identity,
                path: vec![StructuralPathSegment::FixedIndex(0)],
            };
            let sibling = StructuralRuntimePlace {
                opaque_identity: identity,
                path: vec![],
            };
            let unrelated = StructuralRuntimePlace {
                opaque_identity: if identity == 0 { 1 } else { u64::MAX - 1 },
                path: root.path.clone(),
            };
            let mut map = BTreeMap::new();
            map.insert(StructuralScalarRuntimeField::member(root.clone(), field), 1);
            map.insert(
                StructuralScalarRuntimeField {
                    parent: root.clone(),
                    namespace: StructuralFieldNamespace::CasePayload,
                    field,
                },
                2,
            );
            map.insert(StructuralScalarRuntimeField::member(sibling, field), 3);
            map.insert(StructuralScalarRuntimeField::member(unrelated, field), 4);
            assert_eq!(
                cells(&map, &root)
                    .map(|(_, value)| *value)
                    .collect::<Vec<_>>(),
                vec![1, 2]
            );
            prune(&mut map, &root);
            assert_eq!(
                map.values()
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>(),
                [3, 4].into_iter().collect()
            );
        }
    }
}

impl crate::execution::TerminalExecution<'_> {
    /// Reclaim only scalar-field roots with no remaining placed handle or
    /// non-scalar storage. This is conservative storage liveness, never a
    /// custody judgment: claims, loans, descriptors and non-scalar roots keep
    /// their existing semantics and cleanup routes. Complex view/cleanup
    /// states defer collection rather than guessing their hidden backing.
    pub(crate) fn reclaim_orphan_scalar_records(&mut self) {
        use crate::execution::SuspendedCallResult;
        use std::collections::BTreeSet;
        let Some(cursor) = self.local_structural_identities.cursor() else {
            return;
        };
        if cursor < self.next_scalar_collection_identity {
            return;
        }
        self.next_scalar_collection_identity = cursor.saturating_add(1024);
        if self.structural_scalar_fields.len() < 4096
            || !self.primitive_loan_values.is_empty()
            || !self.byte_sequence_values.is_empty()
            || !self.element_view_values.is_empty()
            || !self.structural_element_view_fields.is_empty()
            || self
                .reference_referents
                .values()
                .any(|referent| referent.primitive_loan.is_some() || referent.byte_region.is_some())
            || self.call_stack.iter().any(|frame| {
                !frame.primitive_loan_values.is_empty()
                    || !frame.byte_sequence_values.is_empty()
                    || !frame.element_view_values.is_empty()
                    || matches!(frame.result, SuspendedCallResult::NominalCleanups { .. })
            })
        {
            return;
        }
        let mut roots = BTreeSet::new();
        // A placed view lends an explicit qualified backing. Preserve its
        // whole containing identity, including paths outside the lent subtree;
        // the occurrence's custody and retirement remain unchanged.
        for occurrence in self.placed_view_occurrences.values() {
            roots.insert(occurrence.referent.opaque_identity);
        }
        for value in self.structural_values.values().chain(
            self.aggregate_argument_backings
                .values()
                .map(|backing| &backing.value),
        ) {
            roots.insert(value.opaque_identity);
        }
        for frame in &self.call_stack {
            for occurrence in frame.placed_view_occurrences.values() {
                roots.insert(occurrence.referent.opaque_identity);
            }
            for value in frame.structural_values.values().chain(
                frame
                    .aggregate_argument_backings
                    .values()
                    .map(|backing| &backing.value),
            ) {
                roots.insert(value.opaque_identity);
            }
            if let SuspendedCallResult::Structural {
                expected_reference_backings,
                ..
            } = &frame.result
            {
                for value in expected_reference_backings.iter().flatten() {
                    roots.insert(value.value.opaque_identity);
                }
            }
        }
        if let Some(crate::results::TerminalExecutionResult::Structural(result)) = &self.result {
            roots.insert(result.value.opaque_identity);
        }
        for referent in self.reference_referents.values() {
            roots.insert(referent.view.opaque_identity);
        }
        for key in self
            .reference_referents
            .keys()
            .chain(self.structural_primitive_storage.keys())
            .chain(self.structural_primitive_entry_places.values())
            .chain(self.structural_byte_storage.keys())
            .chain(self.structural_cases.keys())
        {
            roots.insert(key.opaque_identity);
        }
        for key in self.structural_byte_sequence_fields.keys() {
            roots.insert(key.parent.opaque_identity);
        }
        roots.extend(
            self.private_entry_ram
                .iter()
                .map(|original| original.allocation.opaque_identity),
        );
        self.structural_scalar_fields
            .retain(|key, _| roots.contains(&key.parent.opaque_identity));
    }
}
