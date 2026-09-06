use super::RangeFacts;
use typed_trees::{TypedTrees, state::State, statement::StatementNode};

impl RangeFacts<'_> {
    /// Shared direct-assignment transfer for the checking and incoming-edge
    /// walks. Replacing an index invalidates its ordering/position premises;
    /// replacing a collection descriptor invalidates its extent/window rows.
    /// Value snapshots are replaced separately after the RHS is evaluated.
    pub(in crate::checks::ranges) fn invalidate_assignment_bounds(&mut self, target: &str) {
        self.invalidate_relational_bounds(|name| write_affects_bound(name, target));
        // A saved Boolean expression is not a persistent proof of its old
        // operands after a direct assignment any more than after a call.
        self.boolean_locals.clear();
    }

    /// A complete call frame includes both caller storage and overlapping live
    /// alias spellings. Unknown effects retire all value-dependent premises;
    /// declared fixed-array extents remain true regardless of element writes.
    pub(in crate::checks::ranges) fn invalidate_call_writes(
        &mut self,
        program: &TypedTrees,
        state: &State,
        paths: Option<&[String]>,
    ) {
        if paths.is_some_and(|paths| paths.is_empty()) {
            return;
        }
        let overlaps = |name: &str| {
            paths.is_none_or(|paths| paths.iter().any(|path| write_affects_bound(name, path)))
        };
        // Field constants currently retain a leaf name and declaration symbol,
        // not the instance's full storage path. A write to `self.cell.value`
        // must not leave a row named only `value`; preserve the previous
        // conservative field-constant policy until that representation changes.
        self.integer_fields.clear();
        self.integer_locals.retain(|(symbol, name, _)| {
            let declaration = program.symbols.get(*symbol);
            matches!(
                declaration.kind,
                symbols::SymbolKind::Local | symbols::SymbolKind::Parameter
            ) && declaration.parent == state.symbol
                && !overlaps(name)
        });
        self.locals.retain(|(symbol, name, _)| {
            !overlaps(name) || {
                let reference = program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == *symbol)
                    .map(|parameter| parameter.type_reference)
                    .or_else(|| {
                        program
                            .statement_table
                            .statements(state.statement_nodes)
                            .iter()
                            .find_map(|statement| match statement {
                                StatementNode::LocalData(local) if local.symbol == *symbol => {
                                    Some(local.type_reference)
                                }
                                _ => None,
                            })
                    });
                reference.is_some_and(|reference| {
                    super::super::arrays::fixed_array_type_length(program, reference).is_some()
                })
            }
        });
        self.invalidate_relational_bounds(overlaps);
        // These rows replay their defining expressions. Until they carry all
        // operand dependencies, no mutating call may preserve such a shortcut.
        self.boolean_locals.clear();
    }

    fn invalidate_relational_bounds(&mut self, overlaps: impl Fn(&str) -> bool) {
        self.proven_indexes
            .retain(|(collection, index)| !overlaps(collection) && !overlaps(index));
        self.proven_index_upper_bounds
            .retain(|(index, _)| !overlaps(index));
        self.proven_non_negatives.retain(|index| !overlaps(index));
        self.proven_orderings
            .retain(|(left, right)| !overlaps(left) && !overlaps(right));
        self.proven_range_bounds
            .retain(|(collection, bound)| !overlaps(collection) && !overlaps(bound));
        self.minimum_lengths
            .retain(|(collection, _)| !overlaps(collection));
        self.exact_lengths
            .retain(|(collection, _)| !overlaps(collection));
        self.window_parents
            .retain(|(child, parent, _)| !overlaps(child) && !overlaps(parent));
    }
}

fn write_affects_bound(name: &str, path: &str) -> bool {
    // Computed labels and dynamic selectors do not retain their full operand
    // dependencies. Both direct assignments and calls must retire these
    // premises; a separately named value snapshot keeps its own numeric facts.
    !name.split('.').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
    }) || validation::frame_paths_overlap(name, path)
        || path
            .strip_prefix("self.")
            .is_some_and(|path| validation::frame_paths_overlap(name, path))
        || (path == "self" && !name.contains('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::SymbolHandle;

    #[test]
    fn direct_assignment_retires_only_its_index_bound() {
        let mut facts = RangeFacts::new(&[]);
        facts.prove_index_upper_bound("index".to_owned(), 4);
        facts.prove_index_upper_bound("unrelated".to_owned(), 4);

        facts.invalidate_assignment_bounds("index");

        assert!(!facts.index_upper_bound_is_proven("index", 4));
        assert!(facts.index_upper_bound_is_proven("unrelated", 4));
    }

    #[test]
    fn assignments_and_calls_retire_computed_and_overlapping_bound_labels() {
        for call in [false, true] {
            let mut facts = RangeFacts::new(&[]);
            for name in ["record.value", "record.value - 1", "captured", "unrelated"] {
                facts.prove_non_negative(name.into());
                facts.prove_index_upper_bound(name.into(), 5);
                facts.prove_at_most("floor".into(), name.into());
                facts.prove_range_bound("items".into(), name.into());
            }
            if call {
                facts.invalidate_call_writes(
                    &TypedTrees::default(),
                    &State::default(),
                    Some(&["record".into()]),
                );
            } else {
                facts.invalidate_assignment_bounds("record");
            }
            for name in ["record.value", "record.value - 1", "captured", "unrelated"] {
                let survives = matches!(name, "captured" | "unrelated");
                assert_eq!(facts.non_negative_is_proven(name), survives);
                assert_eq!(facts.index_upper_bound_is_proven(name, 5), survives);
                assert_eq!(facts.at_most_is_proven("floor", name), survives);
                assert_eq!(facts.range_bound_is_proven("items", name), survives);
            }
        }
    }

    #[test]
    fn precise_call_write_retires_only_its_index_bound() {
        let mut facts = RangeFacts::new(&[]);
        facts.prove_index_upper_bound("index".to_owned(), 4);
        facts.prove_index_upper_bound("unrelated".to_owned(), 4);

        facts.invalidate_call_writes(
            &TypedTrees::default(),
            &State::default(),
            Some(&["index".to_owned()]),
        );

        assert!(!facts.index_upper_bound_is_proven("index", 4));
        assert!(facts.index_upper_bound_is_proven("unrelated", 4));
    }

    #[test]
    fn unknown_call_retires_dynamic_values_and_relations_not_declared_extents() {
        let fixed_field = SymbolHandle::from_arena_index(1);
        let fields = [(fixed_field, "fixed".to_owned(), 2)];
        let mut facts = RangeFacts::new(&fields);
        facts.define_local(SymbolHandle::invalid(), "index", None, Some(0));
        facts.define_local(SymbolHandle::invalid(), "view", Some(2), None);
        facts.prove_index("view".to_owned(), "index".to_owned());
        facts.prove_index_upper_bound("index".to_owned(), 2);
        facts.invalidate_call_writes(&TypedTrees::default(), &State::default(), None);
        assert_eq!(
            facts.local_integer(SymbolHandle::invalid(), Some("index")),
            None
        );
        assert_eq!(
            facts.local_length(SymbolHandle::invalid(), Some("view")),
            None
        );
        assert!(!facts.index_is_proven("view", "index"));
        assert!(!facts.index_upper_bound_is_proven("index", 2));
        assert_eq!(facts.field_length(fixed_field), Some(2));
    }
}
