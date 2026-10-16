use super::RangeFacts;
use validation::CanonicalPlace;

use language_core::place_field;
use typed_trees::{machine::Machine, state::State, statement::StatementNode};

impl RangeFacts<'_> {
    /// Shared direct-assignment transfer for the checking and incoming-edge
    /// walks. Replacing an index invalidates its ordering/position premises;
    /// replacing a collection descriptor invalidates its extent/window rows.
    /// Value snapshots are replaced separately after the RHS is evaluated.
    pub(in crate::checks::ranges) fn invalidate_assignment_bounds(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        statement: &StatementNode,
    ) {
        let Some(assignment) = statement.as_assignment() else {
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
        // A published bound may name a backing field with no retained
        // expression dependency. Use the same assignment frame as proof
        // witnesses: writing an alias cannot preserve the backing's bound.
        let bound_frame = (!self.proven_index_upper_bounds.is_empty()).then(|| {
            shared_frames.map_or_else(facts::NormalizedWriteFrame::opaque, |frames| {
                frames.assignment_write_frame(machine, statement)
            })
        });
        let writes = (!self.expression_dependencies.is_empty()
            || !self.scalar_binding_dependencies.is_empty()
            || !self.measured_results.is_empty()
            || self.has_descriptor_facts())
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
        self.retire_captured_observations(program, writes.as_deref());
        let preserved =
            self.preserved_expression_labels(program, machine, state, writes.as_deref());
        let affected = self.affected_expression_labels(machine, state, &preserved);
        let statement_index = self.statement_index;
        let data_binding = program
            .state_data_binding(machine, state)
            .map(|name| name.as_str());
        let string_fallback = |name: &str| {
            !preserved.iter().any(|label| label == name)
                && bound_frame.as_ref().map_or_else(
                    || write_affects_bound(data_binding, name, &target),
                    |frame| {
                        frame.complete_paths().is_none_or(|paths| {
                            paths
                                .iter()
                                .any(|path| write_affects_bound(data_binding, name, path))
                        })
                    },
                )
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
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        target: typed_trees::expression::ExpressionHandle,
    ) -> Option<(String, Option<i64>, Option<i64>)> {
        if !crate::checks::ranges::indexes::is_builtin_scalar_index(
            program, machine, state, self, target,
        ) {
            return None;
        }
        let indexed = program.expression_table.indexed_node(target)?;
        let Some(name) = program.expression_table.name_path_node(indexed.collection) else {
            return None;
        };
        ::validation::mutable_byte_slice_parameter_element(program, state, name.symbol)?;
        let collection = program.expression_table.display_name(indexed.collection);
        let minimum = self.minimum_length(&collection);
        let exact = self.exact_length(&collection);
        Some((collection, minimum, exact))
    }

    /// A complete call frame includes both caller storage and overlapping live
    /// alias spellings. Opaque frames retain the checked signature's write
    /// ceiling; an unrepresentable ceiling retires value-dependent premises.
    /// Declared fixed-array extents remain true regardless of element writes.
    pub(in crate::checks::ranges) fn invalidate_call_writes(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        paths: Option<&[String]>,
        site: Option<&validation::CallSite<'_>>,
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
            .filter(|_| {
                !self.expression_dependencies.is_empty()
                    || !self.scalar_binding_dependencies.is_empty()
                    || !self.measured_results.is_empty()
                    || self.has_descriptor_facts()
            })
            .and_then(|site| self.structured_call_writes(program, machine, state, site));
        let scalar_preserved = self.preserved_scalar_binding_labels(
            program,
            machine,
            state,
            call_writes.as_ref().map(|writes| writes.storage.as_slice()),
        );
        // Reference reads and collection descriptors still require complete
        // alias closure; the raw ceiling never becomes their write frame.
        let call_writes = call_writes.and_then(|writes| writes.aliases);
        self.retire_captured_observations(program, call_writes.as_deref());
        let mut owned_frames = None;
        let shared_frames = crate::flow::shared_call_frames_or(
            self.checked_calls.and_then(|context| context.call_frames()),
            program,
            &mut owned_frames,
        );
        // The mutation owner can bound an opaque body by its checked
        // signature and borrow ledger. That complete storage ceiling is
        // sufficient for disjoint typed reads even without textual paths.
        // Missing call identity or unrepresentable origins still supply None.
        let writes = call_writes.clone().or_else(|| {
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
        let affected = self.affected_expression_labels(machine, state, &preserved);
        let statement_index = self.statement_index;
        let data_binding = program
            .state_data_binding(machine, state)
            .map(|name| name.as_str());
        let string_fallback = |name: &str| {
            !preserved.iter().any(|label| label == name)
                && paths.is_none_or(|paths| {
                    paths
                        .iter()
                        .any(|path| write_affects_bound(data_binding, name, path))
                })
        };
        let overlaps = |name: &str| {
            !scalar_preserved.iter().any(|label| label == name)
                && (affected.iter().any(|label| label == name) || string_fallback(name))
        };
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
                    .state_parameter_by_symbol(state, *symbol)
                    .map(|parameter| parameter.type_reference)
                    .or_else(|| {
                        program
                            .state_local_declaration(state, *symbol)
                            .map(|(_, local)| local.type_reference)
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
        self.below_const_binders
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
    program: &dyn ::validation::CheckingInput,
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
pub(super) fn write_covers_place(
    program: &dyn ::validation::CheckingInput,
    write: &CanonicalPlace,
    place: &CanonicalPlace,
) -> bool {
    (write.root) == (place.root)
        && write.segments.len() <= place.segments.len()
        && crate::flow::canonical_place_segments_may_overlap(
            program,
            &write.segments,
            &place.segments,
        )
}

/// Whether a write to `path` expires the bound labelled `name`.
///
/// `data_binding` is the name of the writing state's data record, so a write
/// spelled through it (`main.i`) expires a bound spelled plainly (`i`), and a
/// write of the whole record expires every unqualified bound. A state with no
/// data binding writes no such place.
fn write_affects_bound(data_binding: Option<&str>, name: &str, path: &str) -> bool {
    // Without a complete, disjoint typed read set, computed labels and dynamic
    // selectors must expire on writes. A separately named value snapshot keeps
    // its own numeric facts; this predicate never parses a computation's reads.
    !name.split('.').all(|part| {
        !part.is_empty()
            && part
                .chars()
                .all(|character| character.is_alphanumeric() || character == '_')
    }) || validation::frame_paths_overlap(name, path)
        || data_binding
            .and_then(|root| place_field(root, path))
            .is_some_and(|path| validation::frame_paths_overlap(name, path))
        || (data_binding == Some(path) && !name.contains('.'))
}
