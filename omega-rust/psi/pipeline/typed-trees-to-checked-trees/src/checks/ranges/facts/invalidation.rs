use super::RangeFacts;
use crate::flow::CanonicalPlace;
use language_core::{is_self_receiver, receiver_place_field};
use typed_trees::{TypedTrees, machine::Machine, state::State, statement::StatementNode};

impl RangeFacts<'_> {
    /// Shared direct-assignment transfer for the checking and incoming-edge
    /// walks. Replacing an index invalidates its ordering/position premises;
    /// replacing a collection descriptor invalidates its extent/window rows.
    /// Value snapshots are replaced separately after the RHS is evaluated.
    pub(in crate::checks::ranges) fn invalidate_assignment_bounds(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        statement: &StatementNode,
    ) {
        let StatementNode::Assignment(assignment) = statement else {
            return;
        };
        // A builtin byte-element write changes contents, not the descriptor.
        // Capture only extent metadata still available after RHS evaluation;
        // never resurrect facts invalidated by an effectful RHS.
        let extent = self.mutable_byte_view_extent(program, machine, state, assignment.target);
        let target = program.expression_table.display_name(assignment.target);
        let mut owned_frames = None;
        let shared_frames = crate::flow::shared_call_frames_or(
            self.checked_calls.and_then(|context| context.call_frames()),
            program,
            &mut owned_frames,
        );
        let writes = (!self.expression_dependencies.is_empty() || self.has_descriptor_facts())
            .then(|| {
                crate::flow::statement_storage_writes(
                    program,
                    machine.symbol,
                    state.symbol,
                    self.statement_index,
                    statement,
                    shared_frames,
                )
            })
            .flatten();
        let preserved =
            self.preserved_expression_labels(program, machine, state, writes.as_deref());
        let affected = self.affected_expression_labels(program, machine, state, writes.as_deref());
        let statement_index = self.statement_index;
        let string_fallback = |name: &str| {
            !preserved.iter().any(|label| label == name) && write_affects_bound(name, &target)
        };
        let overlaps =
            |name: &str| affected.iter().any(|label| label == name) || string_fallback(name);
        let covers = |name: &str| {
            affected.iter().any(|label| label == name)
                || match writes.as_deref() {
                    Some(writes) => descriptor_covered(
                        program,
                        state,
                        statement_index,
                        writes,
                        symbols::SymbolHandle::invalid(),
                        name,
                    )
                    .unwrap_or_else(|| string_fallback(name)),
                    None => string_fallback(name),
                }
        };
        self.invalidate_relational_bounds(overlaps, covers);
        if let Some((collection, minimum, exact)) = extent {
            if let Some(minimum) = minimum {
                self.prove_minimum_length(collection.clone(), minimum);
            }
            if let Some(exact) = exact {
                self.prove_exact_length(collection, exact);
            }
        }
        // A saved Boolean expression is not a persistent proof of its old
        // operands after a direct assignment any more than after a call.
        self.boolean_locals.clear();
    }

    fn mutable_byte_view_extent(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        target: typed_trees::expression::ExpressionHandle,
    ) -> Option<(String, Option<i64>, Option<i64>)> {
        use typed_trees::{expression::ExpressionNode, types::TypeReferenceNode};
        if !crate::checks::ranges::indexes::is_builtin_scalar_index(
            program, machine, state, self, target,
        ) {
            return None;
        }
        let ExpressionNode::Indexed(indexed) = program.expression_table.expression(target) else {
            return None;
        };
        let ExpressionNode::Name(name) = program.expression_table.expression(indexed.collection)
        else {
            return None;
        };
        let parameter = program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == name.symbol)?;
        let TypeReferenceNode::Reference {
            referee,
            access: language_core::ReferenceAccess::Mutable,
            ..
        } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let TypeReferenceNode::Slice { element_type } =
            program.type_reference_table.type_reference(*referee)
        else {
            return None;
        };
        if program.primitive_type_reference(*element_type)
            != Some(typed_trees::types::PrimitiveType::U8)
        {
            return None;
        }
        let collection = program.expression_table.display_name(indexed.collection);
        let minimum = self.minimum_length(&collection);
        let exact = self.exact_length(&collection);
        Some((collection, minimum, exact))
    }

    /// A complete call frame includes both caller storage and overlapping live
    /// alias spellings. Unknown effects retire all value-dependent premises;
    /// declared fixed-array extents remain true regardless of element writes.
    pub(in crate::checks::ranges) fn invalidate_call_writes(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        paths: Option<&[String]>,
        site: Option<&crate::semantic::calls::CallSite<'_>>,
    ) {
        if paths.is_some_and(|paths| paths.is_empty()) {
            return;
        }
        // A located call's structured write set keeps the element coordinates
        // the flattened frame paths deliberately drop: `clear(&mut rooms[0])`
        // instantiates to `rooms[0].tag`, never to the `rooms` descriptor.
        // Descriptor-keyed rows (view extents, window parentage, the
        // collection keys of index/range proofs) therefore retire only when a
        // write reaches the collection place itself, so they consult the
        // structured set even when no typed read set exists.
        let call_writes = site
            .filter(|_| !self.expression_dependencies.is_empty() || self.has_descriptor_facts())
            .and_then(|site| self.structured_call_writes(program, machine, state, site));
        let structured = paths
            .filter(|_| !self.expression_dependencies.is_empty())
            .and_then(|_| call_writes.clone());
        let mut owned_frames = None;
        let shared_frames = crate::flow::shared_call_frames_or(
            self.checked_calls.and_then(|context| context.call_frames()),
            program,
            &mut owned_frames,
        );
        let writes = structured.or_else(|| {
            paths
                .filter(|_| !self.expression_dependencies.is_empty())
                .and_then(|paths| {
                    crate::flow::frame_storage_writes(
                        program,
                        machine.symbol,
                        state.symbol,
                        self.statement_index,
                        &facts::NormalizedWriteFrame::complete(paths.to_vec()),
                        shared_frames,
                    )
                })
        });
        let preserved =
            self.preserved_expression_labels(program, machine, state, writes.as_deref());
        let affected = self.affected_expression_labels(program, machine, state, writes.as_deref());
        let statement_index = self.statement_index;
        let string_fallback = |name: &str| {
            !preserved.iter().any(|label| label == name)
                && paths
                    .is_none_or(|paths| paths.iter().any(|path| write_affects_bound(name, path)))
        };
        let overlaps =
            |name: &str| affected.iter().any(|label| label == name) || string_fallback(name);
        // A descriptor row survives when every caller-visible write stays
        // below the collection's own place. Labels that resolve to no caller
        // place, or a missing/incomplete write set, keep the conservative
        // string answer.
        let covers = |name: &str| {
            affected.iter().any(|label| label == name)
                || match call_writes.as_deref() {
                    Some(writes) => descriptor_covered(
                        program,
                        state,
                        statement_index,
                        writes,
                        symbols::SymbolHandle::invalid(),
                        name,
                    )
                    .unwrap_or_else(|| string_fallback(name)),
                    None => string_fallback(name),
                }
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
            let covered = affected.iter().any(|label| label == name.as_str())
                || match call_writes.as_deref() {
                    Some(writes) => {
                        descriptor_covered(program, state, statement_index, writes, *symbol, name)
                            .unwrap_or_else(|| string_fallback(name))
                    }
                    None => string_fallback(name),
                };
            !covered || {
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
        self.invalidate_relational_bounds(overlaps, covers);
        // These rows replay their defining expressions. Until they carry all
        // operand dependencies, no mutating call may preserve such a shortcut.
        self.boolean_locals.clear();
    }

    /// Any live row whose key is a collection's own storage — a view extent,
    /// a window parent edge, or the collection key of an index/range proof —
    /// deserves the structured-write coverage answer rather than the coarse
    /// frame paths.
    fn has_descriptor_facts(&self) -> bool {
        !self.locals.is_empty()
            || !self.minimum_lengths.is_empty()
            || !self.exact_lengths.is_empty()
            || !self.window_parents.is_empty()
            || !self.proven_indexes.is_empty()
            || !self.proven_range_bounds.is_empty()
    }

    /// `overlaps` decides value-level rows (indices, bounds, orderings);
    /// `covers` decides descriptor rows whose key names a collection place.
    /// A write below the collection — `rooms[0]` inside `rooms` — retires the
    /// element's own facts but leaves the collection's extent true.
    pub(super) fn invalidate_relational_bounds(
        &mut self,
        overlaps: impl Fn(&str) -> bool,
        covers: impl Fn(&str) -> bool,
    ) {
        self.proven_indexes
            .retain(|(collection, index)| !covers(collection) && !overlaps(index));
        self.proven_index_upper_bounds
            .retain(|(index, _)| !overlaps(index));
        self.proven_non_negatives.retain(|index| !overlaps(index));
        self.proven_orderings
            .retain(|(left, right, _)| !overlaps(left) && !overlaps(right));
        self.proven_range_bounds
            .retain(|(collection, bound)| !covers(collection) && !overlaps(bound));
        self.minimum_lengths
            .retain(|(collection, _)| !covers(collection));
        self.exact_lengths
            .retain(|(collection, _)| !covers(collection));
        self.window_parents
            .retain(|(child, parent, _)| !covers(child) && !covers(parent));
    }
}

/// Resolve a descriptor row's key to the caller place it measures, then ask
/// whether any write reaches that place itself. A declared local symbol is
/// the identity when present; otherwise the dotted label resolves through
/// the same parameter/local namespace the write paths use. `None` reports
/// "unresolvable — keep the caller's conservative answer".
fn descriptor_covered(
    program: &TypedTrees,
    state: &State,
    statement_index: usize,
    writes: &[CanonicalPlace],
    symbol: symbols::SymbolHandle,
    label: &str,
) -> Option<bool> {
    let place = if symbol.is_valid() {
        crate::flow::canonical_place_from_symbol(symbol)?
    } else {
        crate::flow::place_from_origin_path(program, state, statement_index, label)?
    };
    Some(
        writes
            .iter()
            .any(|write| write_covers_place(program, write, &place)),
    )
}

/// A write covers a descriptor place only by reaching it or an ancestor:
/// `rooms` covers `rooms`, but `rooms[0].tag` — any depth below — does not.
fn write_covers_place(
    program: &TypedTrees,
    write: &CanonicalPlace,
    place: &CanonicalPlace,
) -> bool {
    crate::flow::normalized_event_place_root(program, write.root)
        == crate::flow::normalized_event_place_root(program, place.root)
        && write.segments.len() <= place.segments.len()
        && crate::flow::canonical_place_segments_may_overlap(
            program,
            &write.segments,
            &place.segments,
        )
}

fn write_affects_bound(name: &str, path: &str) -> bool {
    // Without a complete, disjoint typed read set, computed labels and dynamic
    // selectors must expire on writes. A separately named value snapshot keeps
    // its own numeric facts; this predicate never parses a computation's reads.
    !name.split('.').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
    }) || validation::frame_paths_overlap(name, path)
        || receiver_place_field(path)
            .is_some_and(|path| validation::frame_paths_overlap(name, path))
        || (is_self_receiver(path) && !name.contains('.'))
}

#[cfg(test)]
mod tests {
    use super::{Machine, State, TypedTrees};
    use crate::checks::ranges::RangeFacts;
    use crate::checks::ranges::facts::invalidation::write_affects_bound;
    use symbols::SymbolHandle;

    #[test]
    fn direct_assignment_retires_only_its_index_bound() {
        let mut facts = RangeFacts::new(&[]);
        facts.prove_index_upper_bound("index".to_owned(), 4);
        facts.prove_index_upper_bound("unrelated".to_owned(), 4);

        facts.invalidate_relational_bounds(
            |name| write_affects_bound(name, "index"),
            |name| write_affects_bound(name, "index"),
        );

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
                    &Machine::default(),
                    &State::default(),
                    Some(&["record".into()]),
                    None,
                );
            } else {
                facts.invalidate_relational_bounds(
                    |name| write_affects_bound(name, "record"),
                    |name| write_affects_bound(name, "record"),
                );
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
            &Machine::default(),
            &State::default(),
            Some(&["index".to_owned()]),
            None,
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
        facts.invalidate_call_writes(
            &TypedTrees::default(),
            &Machine::default(),
            &State::default(),
            None,
            None,
        );
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
