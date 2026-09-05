use super::RangeFacts;
use typed_trees::{TypedTrees, state::State, statement::StatementNode};

impl RangeFacts<'_> {
    /// Shared direct-assignment transfer for the checking and incoming-edge
    /// walks. Replacing an index invalidates its ordering/position premises;
    /// replacing a collection descriptor invalidates its extent/window rows.
    /// Value snapshots are replaced separately after the RHS is evaluated.
    pub(in crate::checks::ranges) fn invalidate_assignment_bounds(&mut self, target: &str) {
        self.forget_index_upper_bound(target);
        self.forget_index_position_facts(target);
        self.forget_non_negative(target);
        self.forget_orderings(target);
        self.forget_collection_facts(target);
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
            paths.is_none_or(|paths| {
                // Computed labels and dynamic selectors have dependencies not
                // represented by a single frame path. Retire them rather than
                // recover semantic identity by parsing display text.
                !name.split('.').all(|part| {
                    !part.is_empty()
                        && part
                            .chars()
                            .all(|character| character.is_alphanumeric() || character == '_')
                }) || paths.iter().any(|path| {
                    validation::frame_paths_overlap(name, path)
                        || path
                            .strip_prefix("self.")
                            .is_some_and(|path| validation::frame_paths_overlap(name, path))
                        || (path == "self" && !name.contains('.'))
                })
            })
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
        // These rows replay their defining expressions. Until they carry all
        // operand dependencies, no mutating call may preserve such a shortcut.
        self.boolean_locals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbols::SymbolHandle;

    #[test]
    fn unknown_call_retires_dynamic_values_and_relations_not_declared_extents() {
        let fields = [(SymbolHandle::invalid(), "fixed".to_owned(), 2)];
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
        assert_eq!(
            facts.field_length(SymbolHandle::invalid(), Some("fixed")),
            Some(2)
        );
    }
}
