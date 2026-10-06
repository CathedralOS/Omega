//! Current-value facts between successive selected cleanup hooks.
//!
//! A death edge starts with its ordinary flow contexts. Each hook then has the
//! same contract lifecycle as a call: prove prerequisites, invalidate its
//! transitive storage writes, and admit its normal guarantees. No source call
//! coordinate or independently replayed ownership trace is manufactured here.
//! Effect discovery compresses array paths; declaration geometry supplies the
//! exact parent-first, reverse-child execution order when premises are checked.

use crate::facts::boolean_predicates::{self, BoundPredicate};
use crate::flow::{CanonicalPlace, CleanupLeaf, StateMutationSummaryCache};
use checked_trees::{CheckFacts, ContractProofFactKind};
use facts::{FactContextHandle, PlaceRoot, PlaceSegment, ScalarValue};
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, data::DataMember, expression::ExpressionHandle};

#[derive(Clone, Default)]
pub(super) struct CurrentCleanupFacts {
    pub contexts: Vec<FactContextHandle>,
    pub guarantees: Vec<BoundPredicate>,
    entry_contexts: Vec<FactContextHandle>,
    writes: Vec<CanonicalPlace>,
    opaque_write: bool,
}

impl CurrentCleanupFacts {
    pub(super) fn new(contexts: Vec<FactContextHandle>, guarantees: Vec<BoundPredicate>) -> Self {
        // A construction context may contain several independently captured
        // fields. Retiring that context after one field write must not erase
        // the completed value of an untouched sibling. Preserve the original
        // contexts and consult their ordinary current-value join only for a
        // place untouched by every intervening hook. This neither replays an
        // initializer nor rereads the source of a captured copy.
        Self {
            entry_contexts: contexts.clone(),
            contexts,
            guarantees,
            ..Self::default()
        }
    }

    pub(super) fn value_at_place(
        &self,
        program: &TypedTrees,
        facts: &CheckFacts,
        place: &CanonicalPlace,
    ) -> Option<ScalarValue> {
        if self.opaque_write
            || self
                .writes
                .iter()
                .any(|write| overlaps(program, place, write))
        {
            return None;
        }
        ordinary_value(program, facts, &self.entry_contexts, place)
    }

    /// Entry transport cannot repair a premise killed by an earlier hook.
    pub(super) fn entry_stable(&self, program: &TypedTrees, predicate: &BoundPredicate) -> bool {
        !self.opaque_write
            && predicate.bindings.iter().all(|binding| {
                !self
                    .writes
                    .iter()
                    .any(|write| overlaps(program, &binding.place, write))
            })
    }

    pub(super) fn advance(
        &mut self,
        program: &TypedTrees,
        facts: &CheckFacts,
        summaries: &StateMutationSummaryCache,
        call_frames: Option<&validation::CallFrameResolver<'_>>,
        hook: SymbolHandle,
        place: &CanonicalPlace,
    ) -> Option<()> {
        let machine = program.machine_by_symbol(hook)?;
        let state = program.machine_states(machine).first()?;
        let receiver = program.state_parameters(state).first()?;
        let writes = crate::flow::state_storage_writes(
            program,
            &facts.borrow,
            summaries,
            state,
            call_frames,
        )
        .and_then(|writes| {
            writes
                .into_iter()
                .map(|write| rebase_place(receiver.symbol, place, write))
                .collect::<Option<Vec<_>>>()
        });
        if let Some(writes) = writes {
            self.contexts.retain(|context| {
                crate::flow::context_survives_place_mutations(
                    program,
                    &facts.semantic,
                    &facts.domains,
                    *context,
                    &writes,
                )
            });
            self.guarantees.retain(|predicate| {
                predicate.bindings.iter().all(|binding| {
                    !writes
                        .iter()
                        .any(|write| overlaps(program, &binding.place, write))
                })
            });
            self.writes.extend(writes);
        } else {
            // Missing completeness is not purity. A normal guarantee may
            // establish a fresh premise even after an opaque storage effect.
            self.contexts.clear();
            self.guarantees.clear();
            self.opaque_write = true;
        }
        let clauses = crate::execution::terminal_unit::cleanup::checked_contract_expressions(
            program,
            facts,
            hook,
            state.symbol,
            ContractProofFactKind::Ensures,
        )?;
        for clause in clauses {
            if let Some(predicate) = bound_clause(
                program,
                facts,
                hook,
                clause,
                ContractProofFactKind::Ensures,
                place,
            ) {
                self.guarantees.push(predicate);
            }
        }
        Some(())
    }
}

fn ordinary_value(
    program: &TypedTrees,
    facts: &CheckFacts,
    contexts: &[FactContextHandle],
    place: &CanonicalPlace,
) -> Option<ScalarValue> {
    crate::checks::contracts::boolean_value_at_place(program, &facts.semantic, contexts, place)
        .map(ScalarValue::Boolean)
        .or_else(|| {
            crate::values::scalar_value_at_place(
                program,
                &facts.semantic,
                contexts
                    .iter()
                    .map(|context| facts.semantic.contexts.get(*context)),
                place,
            )
        })
}

pub(super) fn bound_clause(
    program: &TypedTrees,
    facts: &CheckFacts,
    hook: SymbolHandle,
    clause: ExpressionHandle,
    kind: ContractProofFactKind,
    place: &CanonicalPlace,
) -> Option<BoundPredicate> {
    let machine = program.machine_by_symbol(hook)?;
    let state = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(state);
    let receiver = parameters.first()?;
    let expression = match kind {
        ContractProofFactKind::Requires => crate::values::lower_state_scalar_contract_predicate(
            program,
            &facts.operators,
            machine,
            state,
            clause,
            &mut 4096,
        ),
        ContractProofFactKind::Ensures => crate::values::lower_scalar_contract_predicate(
            program,
            &facts.operators,
            machine,
            clause,
            true,
            &mut 4096,
        ),
    }?;
    let mut predicate =
        boolean_predicates::bind_parameters(program, hook, state.symbol, parameters, expression)?;
    for binding in &mut predicate.bindings {
        binding.place = rebase_place(receiver.symbol, place, binding.place.clone())?;
    }
    Some(predicate)
}

fn rebase_place(
    receiver: SymbolHandle,
    place: &CanonicalPlace,
    mut formal: CanonicalPlace,
) -> Option<CanonicalPlace> {
    if formal.root != PlaceRoot::Symbol(receiver) {
        return None;
    }
    let mut segments = place.segments.clone();
    segments.append(&mut formal.segments);
    Some(CanonicalPlace {
        root: place.root,
        segments,
    })
}

fn overlaps(program: &TypedTrees, left: &CanonicalPlace, right: &CanonicalPlace) -> bool {
    left.root == right.root
        && crate::flow::canonical_place_segments_may_overlap(
            program,
            &left.segments,
            &right.segments,
        )
}

pub(super) struct SelectedHook {
    pub machine: SymbolHandle,
    pub path: Vec<PlaceSegment>,
    order: Vec<usize>,
}

/// Expand only the already selected live receivers, counting before allocating.
/// Ancestors sort before their descendants; field/index ordinals come from
/// declaration geometry, never symbol allocation or rendered names.
pub(super) fn ordered_hooks(
    program: &TypedTrees,
    leaves: &[CleanupLeaf],
) -> Option<Vec<SelectedHook>> {
    let mut hooks = Vec::new();
    for leaf in leaves {
        let CleanupLeaf::Nominal { machine, path } = leaf else {
            continue;
        };
        let count = path.iter().try_fold(1_usize, |count, segment| {
            if let PlaceSegment::FixedRange { start, end } = *segment {
                count.checked_mul(end.checked_sub(start)?)
            } else {
                Some(count)
            }
        })?;
        hooks.try_reserve_exact(count).ok()?;
        let paths = crate::flow::concrete_cleanup_paths(path)?;
        for path in paths {
            let order = path
                .iter()
                .map(|segment| segment_order(program, *segment))
                .collect::<Option<Vec<_>>>()?;
            hooks.push(SelectedHook {
                machine: *machine,
                path,
                order,
            });
        }
    }
    hooks.sort_by(|left, right| left.order.cmp(&right.order));
    Some(hooks)
}

fn segment_order(program: &TypedTrees, segment: PlaceSegment) -> Option<usize> {
    let ordinal = match segment {
        PlaceSegment::FixedIndex { index } => index,
        PlaceSegment::Field { symbol } | PlaceSegment::Case { variant: symbol } => {
            program.data_definitions().iter().find_map(|data| {
                for (ordinal, member) in program.data_members(data).iter().enumerate() {
                    match member {
                        DataMember::Field(field) if field.symbol == symbol => return Some(ordinal),
                        DataMember::Variant(variant) => {
                            if variant.symbol == symbol {
                                return Some(ordinal);
                            }
                            if let Some(ordinal) = program
                                .data_payload_fields(variant)
                                .iter()
                                .position(|field| field.symbol == symbol)
                            {
                                return Some(ordinal);
                            }
                        }
                        _ => {}
                    }
                }
                None
            })?
        }
        PlaceSegment::FixedRange { .. } | PlaceSegment::Index { .. } => return None,
    };
    Some(usize::MAX - ordinal)
}
