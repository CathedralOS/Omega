//! Writes retire live storage observations, not completed value identities.
//!
//! The normal-return path owns effect order. A judge with a storage version
//! reads validity from the program's one storage graph: a stored value stays
//! current while the store or incoming storage it was anchored at still
//! reaches its place at the query version. Earlier copies and equations keep
//! describing their captured values; a later read of replaced storage gets a
//! new unknown observation. A judge no graph reaches treats every write as
//! opaque and keeps only immutable scalars' private copies.

use super::StructuralJudge;
use crate::CanonicalPlace;
use facts::PlaceSegment;
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

mod argument_reads;
mod entry_reads;
mod graph_versions;
mod read_backing;
mod stored_values;
pub(in crate::proof_contracts::contract_entailment) use graph_versions::{
    StorageAnchor, StorageVersion,
};
pub(super) use stored_values::StoredValues;

type StorageScope<'scope> = Option<(&'scope Machine, &'scope State)>;

/// A canonical place with live selectors replaced by completed value identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CapturedStorageCoordinate {
    root: facts::PlaceRoot,
    segments: Vec<CapturedStorageSegment>,
}

impl CapturedStorageCoordinate {
    /// Owned inline storage has a stable backing namespace up to its first
    /// reference boundary. Preserving that value separately erases old pointee
    /// observations. An
    /// incoming exclusive formal likewise names its caller-owned referent for
    /// the invocation. Local reference aliases and nested descriptors require
    /// an independently retained backing join; spelling alone cannot preserve
    /// them against a write through another alias.
    fn has_stable_backing(
        &self,
        program: &dyn crate::CheckingInput,
        state: &State,
        frames: &crate::machine_calls::calls::CallFrameResolver<'_>,
    ) -> bool {
        let Some(root) = self.root.as_symbol() else {
            return false;
        };
        let Some(reference) = crate::declaration_type_reference(program, root) else {
            return false;
        };
        if frames.value_is_reference_free(reference) {
            return true;
        }
        if crate::reference_shell(program, reference).is_none() {
            let mut current = reference;
            for segment in &self.segments {
                if crate::reference_shell(program, current).is_some() {
                    return false;
                }
                let Some(projected) = segment.projected_type(program, current) else {
                    return false;
                };
                current = projected;
            }
            return true;
        }
        program
            .state_parameter_by_symbol(state, root)
            .is_some_and(|parameter| {
                // Incoming exclusive formals lend their referent, not a rebindable
                // descriptor slot (assignment_targets_parameter_referent). The
                // parameter's is_mutable flag also describes referent access and
                // cannot distinguish these from mutable descriptor bindings.
                crate::reference_shell(program, parameter.type_reference).is_some_and(|shell| {
                    shell.access == language_semantics::ReferenceAccess::Mutable
                        && frames.value_is_reference_free(shell.referee)
                })
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CapturedStorageSegment {
    Place(PlaceSegment),
    Index(super::StructuralTerm),
}

impl CapturedStorageSegment {
    fn projected_type(
        &self,
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<TypeReferenceHandle> {
        match self {
            Self::Place(segment) => {
                crate::value_custody::places::project_type_reference_from_segments(
                    program,
                    reference,
                    std::slice::from_ref(segment),
                )
            }
            Self::Index(_) => {
                crate::value_custody::places::collection_element_type_reference(program, reference)
            }
        }
    }
}

/// Project the same completed value through typed fields and frozen selectors.
/// These paths were authenticated when captured; no live selector is reevaluated
/// and no receiving declaration contributes a premise about its own initializer.
fn project_captured_value(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    value: super::StructuralTerm,
    path: &[CapturedStorageSegment],
) -> Option<(TypeReferenceHandle, super::StructuralTerm)> {
    project_captured_value_by(
        program,
        reference,
        value,
        path,
        |reference, value, segment| {
            super::super::observed_projections::project_observed_value(
                program,
                reference,
                value,
                std::slice::from_ref(segment),
            )
        },
    )
}

fn project_captured_value_by(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
    mut value: super::StructuralTerm,
    path: &[CapturedStorageSegment],
    mut project_place: impl FnMut(
        TypeReferenceHandle,
        super::StructuralTerm,
        &facts::PlaceSegment,
    ) -> Option<super::StructuralTerm>,
) -> Option<(TypeReferenceHandle, super::StructuralTerm)> {
    for segment in path {
        let projected = segment.projected_type(program, reference)?;
        value = match segment {
            CapturedStorageSegment::Place(segment) => project_place(reference, value, segment)?,
            CapturedStorageSegment::Index(index) => {
                super::super::observed_projections::project_index(
                    program,
                    reference,
                    value,
                    index.clone(),
                )?
            }
        };
        reference = projected;
    }
    Some((reference, value))
}

fn content_storage_segments(
    path: &[language_semantics::content::ContentPlaceSegment],
) -> Vec<CapturedStorageSegment> {
    use language_semantics::content::ContentPlaceSegment;
    path.iter()
        .map(|segment| match segment {
            ContentPlaceSegment::Field(field) => {
                CapturedStorageSegment::Place(PlaceSegment::Field {
                    symbol: field.symbol,
                })
            }
            ContentPlaceSegment::Case(case) => CapturedStorageSegment::Place(PlaceSegment::Case {
                variant: case.symbol,
            }),
            ContentPlaceSegment::FixedIndex(index) => CapturedStorageSegment::Index(
                super::StructuralTerm::Integer(BigInt::from_u64(*index)),
            ),
        })
        .collect()
}

/// One evaluated place keeps its detached entry snapshot beside its live value.
/// Writes retire only the latter. An entry value is captured by an actual read
/// before that parameter is invalidated, never reconstructed from an initializer.
#[derive(Clone, PartialEq)]
pub(super) struct CapturedStorageValue {
    pub(super) coordinate: CapturedStorageCoordinate,
    pub(super) entry: Option<super::StructuralTerm>,
    pub(super) current: Option<super::StructuralTerm>,
    /// A payload store observes the containing case without asserting any of
    /// its predicates or unobserved fields. Whole/opaque writes retire it.
    pub(super) selected_case: Option<SymbolHandle>,
    pub(super) completed_read: Option<(ExpressionHandle, usize)>,
    pub(super) backing: Option<read_backing::CapturedScalarReadBacking>,
    /// The storage versions `current` was established at, in `anchor_state`'s
    /// history. Several versions are the reaching cuts of joined paths.
    pub(super) current_anchors: Vec<StorageAnchor>,
    pub(super) anchor_state: SymbolHandle,
}

/// One authenticated store destination, frozen before replacement evaluation.
#[derive(Clone)]
pub(super) struct CapturedStoreDestination {
    coordinate: Option<CapturedStorageCoordinate>,
    frame: facts::NormalizedWriteFrame,
    /// The storage version after this store's statement completes.
    version: Option<StorageVersion>,
    /// The stored value, the operand the graph's store event records.
    value_expression: ExpressionHandle,
    stored_type: TypeReferenceHandle,
    representation: Option<(
        typed_trees::types::PrimitiveType,
        typed_trees::types::PrimitiveType,
    )>,
    /// A complete IntegerAt cell written through a same-width view. The written
    /// bits decode into this encoding; they are not a native bitcast of the
    /// wider semantic carrier.
    stored_integer: Option<semantic_vocabulary::IntegerType>,
}

impl StructuralJudge<'_> {
    /// Rebase only live observed cells from authenticated actual referents.
    /// A callee entry snapshot is the current value at this arrival, never the
    /// caller's earlier entry epoch. Frozen selectors were captured before
    /// clearing expression-local caches. Ambiguous origins donate no cells.
    pub(super) fn enter_storage_scope(
        &mut self,
        entry: &State,
        caller_state: &State,
        subjects: &[crate::machine_calls::calls::InvocationEntrySubject],
        bindings: &[(SymbolHandle, Option<super::StructuralTerm>)],
    ) {
        let caller_version = self.storage_version;
        let mut values = self.owned_argument_storage_values(entry, caller_state, bindings);
        self.storage_version = self.incoming_storage_version(entry);
        // Operand writes belong to the caller; this invocation starts at its
        // own entry cut before any of its body writes.
        self.storage_write_sequence = 0;
        for subject in subjects {
            if !crate::reference_shell(self.program, subject.parameter_type)
                .is_some_and(|shell| shell.access.is_readable())
            {
                continue;
            }
            let Some(source) = self.captured_reference_subject(subject, caller_state) else {
                continue;
            };
            let scalar = subject.scalar_storage_coordinate(self.program, caller_state);
            for stored in self.stored_values.iter() {
                if stored.coordinate.root != source.root {
                    continue;
                }
                let Some(suffix) = stored
                    .coordinate
                    .segments
                    .strip_prefix(source.segments.as_slice())
                else {
                    continue;
                };
                if !self.storage_value_transfer_is_current(stored, caller_version) {
                    continue;
                }
                let Some(current) = stored.current.clone() else {
                    continue;
                };
                let current = match &scalar {
                    Some(scalar) if !scalar.preserves_scalar_value => {
                        if !suffix.is_empty() {
                            continue;
                        }
                        super::super::structural_terms::normalize_scalar_representation(
                            scalar.source_primitive,
                            scalar.view_primitive,
                            current,
                        )
                    }
                    _ => current,
                };
                values.push(CapturedStorageValue {
                    coordinate: CapturedStorageCoordinate {
                        root: facts::PlaceRoot::Symbol(subject.parameter),
                        segments: suffix.to_vec(),
                    },
                    entry: Some(current.clone()),
                    current: Some(current),
                    selected_case: None,
                    completed_read: None,
                    backing: None,
                    current_anchors: vec![StorageAnchor::Incoming],
                    anchor_state: entry.symbol,
                });
            }
        }
        self.stored_values = std::sync::Arc::new(StoredValues::from_values(values));
        self.invalidated_storage = std::sync::Arc::new(Vec::new());
        for parameter in self.program.state_parameters(entry) {
            let coordinate = CapturedStorageCoordinate {
                root: facts::PlaceRoot::Symbol(parameter.symbol),
                segments: Vec::new(),
            };
            if crate::reference_shell(self.program, parameter.type_reference).is_some()
                && !self.frame_resolver().is_some_and(|frames| {
                    coordinate.has_stable_backing(self.program, entry, frames)
                })
            {
                // Only an exact incoming exclusive, reference-free referent
                // supplies a new whole snapshot. Nested descriptors and other
                // aliasing roots still enter through the source roster above.
                continue;
            }
            if let Some((_, Some(value))) = bindings
                .iter()
                .find(|(symbol, _)| *symbol == parameter.symbol)
            {
                // Some comes from entry_argument_value's independently checked
                // actual/referent join. Keep that distinction until here; a
                // same-typed unobserved descriptor donates no live contents.
                self.retain_storage_value(coordinate, value.clone(), true);
            }
        }
    }

    /// Return live referent observations through the same authenticated origins
    /// used at entry. Caller entry snapshots stay caller snapshots: a callee's
    /// entry is not evidence about an earlier invocation epoch. Complete writes
    /// first retire old caller contents, including cells the callee cannot
    /// describe on this path; only observed current values are installed again.
    pub(super) fn leave_storage_scope(
        &mut self,
        caller: &Self,
        state: &State,
        subjects: &[crate::machine_calls::calls::InvocationEntrySubject],
        writes: &facts::NormalizedWriteFrame,
    ) -> Option<()> {
        let mut returned = caller.clone();
        returned.observe_storage_effects(state, writes.complete_paths(), None)?;
        if writes.is_complete() {
            for subject in subjects {
                let Some(source) = caller.captured_reference_subject(subject, state) else {
                    continue;
                };
                let scalar = subject.scalar_storage_coordinate(self.program, state);
                for stored in self.stored_values.iter() {
                    if stored.coordinate.root != facts::PlaceRoot::Symbol(subject.parameter) {
                        continue;
                    }
                    if !self.storage_value_transfer_is_current(stored, self.storage_version) {
                        continue;
                    }
                    let Some(current) = stored.current.clone() else {
                        continue;
                    };
                    let current = match &scalar {
                        Some(scalar) if !scalar.preserves_scalar_value => {
                            if !stored.coordinate.segments.is_empty() {
                                continue;
                            }
                            super::super::structural_terms::normalize_scalar_representation(
                                scalar.view_primitive,
                                scalar.source_primitive,
                                current,
                            )
                        }
                        _ => current,
                    };
                    let mut coordinate = source.clone();
                    coordinate
                        .segments
                        .extend_from_slice(&stored.coordinate.segments);
                    returned.retain_anchored_storage_value(
                        coordinate,
                        current,
                        false,
                        StorageAnchor::CallReturn,
                    );
                }
            }
        }
        self.storage_version = returned.storage_version;
        self.stored_values = returned.stored_values;
        self.invalidated_storage = returned.invalidated_storage;
        self.premise_facts = returned.premise_facts;
        self.storage_write_sequence = returned.storage_write_sequence;
        self.capture_sequence = self.capture_sequence.max(returned.capture_sequence);
        Some(())
    }

    /// Transferring a current cell requires its actual source cut whenever
    /// the program has a storage graph. Captured arguments and returned values
    /// carry their own saved fields separately.
    fn storage_value_transfer_is_current(
        &self,
        stored: &CapturedStorageValue,
        version: Option<StorageVersion>,
    ) -> bool {
        match version {
            Some(version) => self.current_value_valid(stored, &stored.coordinate, version),
            None => self.storage_graph().is_none(),
        }
    }

    /// Unknown effects retire reachable live storage observations, not immutable
    /// scalar bindings or detached values captured before the call. Absence of
    /// a complete frame is never an empty frame or evidence of non-interference.
    /// Incoming by-value parameters own their scalar payload or inline cells.
    /// Retain that entry observation before the first effect, just as completed
    /// local bindings do. A disjoint write may erase pointee observations without
    /// erasing this copy's descriptor words. Readable nominal reference formals
    /// retain an authenticated referent observation through their frame backing.
    pub(super) fn retain_entry_values(&mut self, state: &State) {
        if self.storage_version.is_none() {
            self.storage_version = self.incoming_storage_version(state);
        }
        for parameter in self.program.state_parameters(state) {
            let reference = parameter.type_reference;
            let mut value = super::StructuralTerm::Variable(parameter.name.to_string());
            let mut backing = None;
            if crate::reference_shell(self.program, reference).is_some() {
                let Some(observed) =
                    crate::value_custody::expression_types::readable_observation_type_reference(
                        self.program,
                        reference,
                    )
                else {
                    continue;
                };
                // A nominal record's initialized inline cells are a complete
                // entry observation, independently of which scalar leaves a
                // declaration later demands. Views keep their descriptor-only
                // contract. A record that stores references is not complete
                // this way: its contained references carry no pointee facts,
                // and a whole-record entry value would answer reads through
                // them before their own descriptor loads can.
                if self.program.closed_data_definition(observed).is_none()
                    || !self
                        .frame_resolver()
                        .is_some_and(|frames| frames.value_is_reference_free(observed))
                {
                    continue;
                }
                let Some(machine) = self.program.machine(self.machine_symbol) else {
                    continue;
                };
                let Some(read) = self.entry_read_backing(machine, state, parameter.symbol, &[])
                else {
                    continue;
                };
                backing = Some(self.freeze_entry_read_backing(state, read));
                value = super::super::structural_terms::owned_projection_observation(
                    self.program,
                    observed,
                    value,
                );
            }
            // Entry permissions still expose readable pointees. The existing
            // write transfer masks them at the first effect; masking here
            // would lose independently declared entry contracts before use.
            self.retain_storage_value(
                CapturedStorageCoordinate {
                    root: facts::PlaceRoot::Symbol(parameter.symbol),
                    segments: Vec::new(),
                },
                value,
                true,
            );
            if let Some(backing) = backing
                && let Some(stored) = std::sync::Arc::make_mut(&mut self.stored_values)
                    .value_at_mut(&CapturedStorageCoordinate {
                        root: facts::PlaceRoot::Symbol(parameter.symbol),
                        segments: Vec::new(),
                    })
            {
                stored.backing = Some(backing);
            }
        }
    }

    /// Retain the scalar subjects of an admitted invocation contract at its
    /// original entry cut. The hypotheses already establish their predicates;
    /// this captures storage identity only and adds no value or permission.
    pub(super) fn retain_entry_scalar_reads(&mut self, state: &State, fact: ExpressionHandle) {
        let Some(machine) = self.program.machine(self.machine_symbol) else {
            return;
        };
        for (canonical, expression) in self.entry_scalar_read_coordinates(machine, state, fact) {
            let Some(_) = canonical.root.as_symbol() else {
                continue;
            };
            let Some(value) = self.declaration_term(expression) else {
                continue;
            };
            let _ = self.retain_entry_scalar_snapshot(machine, state, canonical, value);
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn observe_storage_effects(
        &mut self,
        state: &State,
        written: Option<&[String]>,
        version: Option<StorageVersion>,
    ) -> Option<()> {
        self.advance_storage_version(version);
        if written.is_some_and(<[String]>::is_empty) || !self.requires_value_preservation() {
            return Some(());
        }
        self.storage_write_sequence += 1;
        // A versioned judge asks the storage graph at each read. Without a
        // version no history reaches this judge, so the write is opaque.
        if !self.graph_storage_mode() {
            self.retire_unversioned_storage(state)?;
        }
        // Source premises name storage too: keep only those whose storage
        // still holds the observed contents.
        let program = self.program;
        let mut facts = std::mem::take(&mut self.premise_facts);
        facts.retain(|fact| {
            let mut expressions = Vec::new();
            crate::queries::expression_traversal::collect_expression_tree(
                program,
                *fact,
                &mut expressions,
            );
            !expressions
                .iter()
                .any(|expression| self.storage_observation_invalidated(*expression))
        });
        self.premise_facts = facts;
        Some(())
    }

    /// An opaque write retires every live observation and binding reachable
    /// from this invocation, except an immutable scalar's private copy.
    /// Detached values captured before the write keep their identities.
    fn retire_unversioned_storage(&mut self, state: &State) -> Option<()> {
        self.frame_resolver()?;
        let program = self.program;
        let preserved = self
            .stored_values
            .iter()
            .map(|stored| {
                stored.coordinate.segments.is_empty()
                    && stored
                        .coordinate
                        .root
                        .as_symbol()
                        .is_some_and(|root| self.immutable_scalar_binding(root))
            })
            .collect::<Vec<_>>();
        let mut preserved = preserved.into_iter();
        std::sync::Arc::make_mut(&mut self.stored_values).retain_mut(|stored| {
            if !preserved.next().unwrap_or(false) {
                stored.current = None;
            }
            stored.selected_case = None;
            stored.completed_read = None;
            stored.entry.is_some() || stored.current.is_some()
        });
        let mut invalidated = Vec::new();
        let mut consider = |symbol: SymbolHandle| {
            if !self.immutable_scalar_binding(symbol) {
                invalidated.push(symbol);
            }
        };
        // A named state may receive an earlier state's reference under a new
        // binder. The exit contract still names the original binder, so every
        // state's bindings and the machine's data fields retire together.
        if let Some(machine) = program.machine(self.machine_symbol) {
            for earlier in program.machine_states(machine) {
                for parameter in program.state_parameters(earlier) {
                    consider(parameter.symbol);
                }
                for statement in program.statement_table.statements(earlier.statement_nodes) {
                    if let Some(local) = statement.as_local_data() {
                        consider(local.symbol);
                    }
                }
            }
            if let Some(data) =
                crate::value_custody::places::machine_attached_data(program, machine)
            {
                for member in program.data_members(data) {
                    if let typed_trees::data::DataMember::Field(field) = member {
                        consider(field.symbol);
                    }
                }
            }
        }
        for parameter in program.state_parameters(state) {
            consider(parameter.symbol);
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data() {
                consider(local.symbol);
            }
        }
        invalidated.push(self.machine_symbol);
        let storage = std::sync::Arc::make_mut(&mut self.invalidated_storage);
        for symbol in invalidated {
            if !storage.contains(&symbol) {
                storage.push(symbol);
            }
        }
        Some(())
    }

    fn ordinary_storage_path(&self, scope: StorageScope<'_>, expression: ExpressionHandle) -> bool {
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => true,
            ExpressionNode::Member(member) => self.ordinary_storage_path(scope, member.receiver),
            ExpressionNode::Indexed(indexed) => {
                let builtin = scope.is_some_and(|(machine, state)| {
                    crate::value_custody::places::place_has_builtin_coordinates(
                        self.program,
                        machine,
                        Some(state),
                        expression,
                    )
                });
                let collection = self.ordinary_storage_path(scope, indexed.collection);
                builtin && collection
            }
            // Canonical places deliberately erase borrow recasts to their
            // backing coordinate. That coordinate does not authorize reusing
            // the captured value under a different representation.
            _ => false,
        }
    }

    fn captured_storage_coordinate(
        &self,
        scope: StorageScope<'_>,
        expression: ExpressionHandle,
    ) -> Option<CapturedStorageCoordinate> {
        if !self.ordinary_storage_path(scope, expression) {
            return None;
        }
        let place = crate::value_custody::places::canonical_place_from_expression(
            self.program,
            expression,
        )?;
        self.capture_storage_place(place)
    }

    pub(super) fn captured_reference_subject(
        &self,
        subject: &crate::machine_calls::calls::InvocationEntrySubject,
        state: &State,
    ) -> Option<CapturedStorageCoordinate> {
        // A view's logical fields cannot be appended to its raw backing path.
        // Scalar views have their own authenticated representation translation.
        if subject.has_aggregate_view(self.program) {
            return None;
        }
        // A finite origin join authorizes an entry observation, not one
        // physical write destination. No selected payload identifies an alias.
        let [source] = subject.sources.as_slice() else {
            return None;
        };
        let place = subject
            .scalar_storage_coordinate(self.program, state)
            .map(|scalar| scalar.place)
            .unwrap_or_else(|| CanonicalPlace {
                root: facts::PlaceRoot::Symbol(source.root),
                segments: source.segments.clone(),
            });
        self.capture_storage_place(place)
    }

    fn capture_storage_place(&self, place: CanonicalPlace) -> Option<CapturedStorageCoordinate> {
        self.capture_storage_place_with_selectors(place, |expression| {
            let (_, value) = self
                .normal_results
                .iter()
                .find(|(source, _)| *source == expression)?;
            Some(self.completed_value_identity(value.clone()))
        })
    }

    fn capture_storage_place_with_selectors(
        &self,
        place: CanonicalPlace,
        mut selector: impl FnMut(ExpressionHandle) -> Option<super::StructuralTerm>,
    ) -> Option<CapturedStorageCoordinate> {
        let segments = place
            .segments
            .into_iter()
            .map(|segment| match segment {
                PlaceSegment::FixedIndex { index } => Some(CapturedStorageSegment::Index(
                    super::StructuralTerm::Integer(BigInt::from_u64(u64::try_from(index).ok()?)),
                )),
                PlaceSegment::Index { expression } => {
                    let value = selector(expression);
                    if super::super::struct_trace() {
                        eprintln!(
                            "STRUCT captured-index owner={} source={} value={value:?}",
                            self.program.symbols.display_path(self.machine_symbol, "::"),
                            self.program.expression_table.display_name(expression)
                        );
                    }
                    let value = value?;
                    value.observation_identity_key()?;
                    Some(CapturedStorageSegment::Index(value))
                }
                PlaceSegment::Field { symbol }
                    if symbol.is_valid()
                        && self.program.symbols.get(symbol).kind == symbols::SymbolKind::Field =>
                {
                    Some(CapturedStorageSegment::Place(segment))
                }
                PlaceSegment::Case { variant } if variant.is_valid() => {
                    Some(CapturedStorageSegment::Place(segment))
                }
                PlaceSegment::Field { .. } | PlaceSegment::Case { .. } => None,
                // A window is not one storage coordinate. Unknown selection cannot
                // borrow the identity of its source expression or an arbitrary slot.
                PlaceSegment::FixedRange { .. } | PlaceSegment::OpaqueRange => None,
            })
            .collect::<Option<Vec<_>>>()?;
        Some(CapturedStorageCoordinate {
            root: place.root,
            segments,
        })
    }

    pub(super) fn stored_value_observation(
        &self,
        scope: StorageScope<'_>,
        expression: ExpressionHandle,
        mut specification_selector: impl FnMut(ExpressionHandle) -> Option<super::StructuralTerm>,
    ) -> Option<super::StructuralTerm> {
        // Case-only judgments intentionally do not track storage writes.
        if self.stored_values.is_empty() || !self.requires_value_preservation() {
            return None;
        }
        let place = if self.specification_terms {
            // A predicate observes selectors at this proof point; it has no
            // executable operand schedule of its own. Its term owner supplies
            // the exact current environment and selected operation meanings.
            // Executed reads still require their already completed selectors.
            self.ordinary_storage_path(scope, expression)
                .then_some(())?;
            let place = crate::canonical_place_from_expression(self.program, expression)?;
            self.capture_storage_place_with_selectors(place, |selector| {
                specification_selector(selector).map(|value| self.completed_value_identity(value))
            })?
        } else {
            self.captured_storage_coordinate(scope, expression)?
        };
        let read_version = match scope {
            Some((_, state)) if !self.specification_terms => self
                .read_storage_version(state, expression)
                .or(self.storage_version),
            _ => self.storage_version,
        };
        let current = self.current_storage_value_at(&place, read_version);
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT current-store owner={} source={} current={current:?}",
                self.program.symbols.display_path(self.machine_symbol, "::"),
                self.program.expression_table.display_name(expression)
            );
        }
        // An owned descriptor cell identifies the pointer, not its scalar
        // contents. A separately authenticated live backing read may observe
        // those contents; otherwise retain the descriptor's existing fence.
        let missing_contents = current.as_ref().is_none_or(|value| {
            super::super::structural_terms::term_is_unobserved_descriptor(self.program, value)
                || scope
                    .and_then(|(machine, state)| {
                        self.scalar_reference_read_type(machine, state, expression)
                    })
                    .is_some_and(|reference| {
                        !super::super::structural_terms::scalar_operand_is_observed(
                            self.program,
                            reference,
                            value,
                        )
                    })
        });
        if missing_contents
            && let Some((machine, state)) = scope
            && let Some(value) =
                self.current_backing_value(machine, state, expression, read_version)
        {
            return Some(value);
        }
        current
    }

    pub(super) fn current_storage_value(
        &self,
        place: &CapturedStorageCoordinate,
    ) -> Option<super::StructuralTerm> {
        self.current_storage_value_at(place, self.storage_version)
    }

    /// The current contents of `place` as observed at `at`.
    pub(super) fn current_storage_value_at(
        &self,
        place: &CapturedStorageCoordinate,
        at: Option<StorageVersion>,
    ) -> Option<super::StructuralTerm> {
        let graph_mode = self.graph_storage_mode();
        let valid = |stored: &CapturedStorageValue| {
            !graph_mode || at.is_some_and(|at| self.current_value_valid(stored, place, at))
        };
        if let Some(value) = self
            .stored_values
            .value_at(place)
            .filter(|stored| valid(stored))
            .and_then(|stored| stored.current.clone())
        {
            return Some(value);
        }
        // A fresh aggregate observation owns its fields too. Projection uses
        // that exact current value, never the retired source initializer.
        self.stored_values.iter().find_map(|stored| {
            if stored.coordinate.root != place.root
                || !place.segments.starts_with(&stored.coordinate.segments)
                || !valid(stored)
            {
                return None;
            }
            let root_reference =
                crate::declaration_type_reference(self.program, place.root.as_symbol()?)?;
            let mut reference = stored
                .coordinate
                .segments
                .iter()
                .try_fold(root_reference, |reference, segment| {
                    segment.projected_type(self.program, reference)
                })?;
            let mut value = stored.current.clone()?;
            for segment in &place.segments[stored.coordinate.segments.len()..] {
                let projected_type = segment.projected_type(self.program, reference)?;
                value = match segment {
                    CapturedStorageSegment::Place(PlaceSegment::Field { symbol }) => {
                        self.observed_field_term(value, self.program.symbols.name(*symbol), 0)?
                    }
                    CapturedStorageSegment::Index(_) => {
                        // Preserve the current-field reader's completed-value
                        // and descriptor fences for indexed observations too.
                        let current = if self.runtime_body_values {
                            self.completed_value_identity(value)
                        } else {
                            self.resolve_at(value, 1)
                        };
                        if self.require_closed_computation && !current.is_closed_value() {
                            return None;
                        }
                        let (_, projected) = project_captured_value(
                            self.program,
                            reference,
                            current,
                            std::slice::from_ref(segment),
                        )?;
                        if super::super::structural_terms::term_is_unobserved_descriptor(
                            self.program,
                            &projected,
                        ) {
                            return None;
                        }
                        projected
                    }
                    CapturedStorageSegment::Place(segment @ PlaceSegment::Case { .. }) => {
                        // A payload read traverses the classifier of this
                        // current owned value. Consume the independent tag
                        // through the shared typed projection owner; the path
                        // itself establishes neither the tag nor its predicate.
                        if self.require_closed_computation {
                            let closed = if self.runtime_body_values {
                                self.completed_value_identity(value.clone())
                            } else {
                                self.resolve_at(value.clone(), 1)
                            };
                            if !closed.is_closed_value() {
                                return None;
                            }
                        }
                        super::super::observed_projections::project_observed_value_in_scope(
                            self.program,
                            self,
                            reference,
                            value,
                            std::slice::from_ref(segment),
                        )?
                    }
                    CapturedStorageSegment::Place(_) => return None,
                };
                reference = projected_type;
            }
            Some(value)
        })
    }

    /// A whole-value read after a field replacement captures the current field
    /// snapshots alongside its otherwise unknown value. It cannot reconstruct
    /// unobserved siblings from the old aggregate or grant reference contents.
    pub(super) fn intake_current_storage_fields(
        &mut self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        value: &super::StructuralTerm,
    ) {
        let Some(coordinate) = self.captured_storage_coordinate(Some((machine, state)), expression)
        else {
            return;
        };
        let Some(reference) =
            crate::expression_result_type_reference(self.program, machine, state, expression)
        else {
            return;
        };
        let version = self
            .read_storage_version(state, expression)
            .or(self.storage_version);
        let selected_case = self
            .stored_values
            .values_at(&coordinate)
            .find(|stored| {
                !self.graph_storage_mode()
                    || version.is_some_and(|at| self.current_value_valid(stored, &coordinate, at))
            })
            .and_then(|stored| stored.selected_case);
        if !super::super::structural_terms::term_is_unobserved_descriptor(self.program, value)
            && let Some(case) = selected_case
            && let Some((owner, _)) = self.program.variant_owner(case)
            && let Some(definition) = self.program.data_constructor_definition(
                self.program
                    .type_reference_table
                    .peeled_type_reference(reference),
                owner.symbol,
            )
            && let Some(selected) = self.program.data_constructor_case(
                self.program
                    .type_reference_table
                    .peeled_type_reference(reference),
                owner.symbol,
                case,
            )
        {
            self.intake_case_guarantee(&super::CaseGuarantee {
                subject: value.clone(),
                data: definition.symbol,
                case: selected.symbol,
            });
        }
        self.intake_storage_field_observations(&coordinate, reference, value, version);
    }

    /// Capture this view's current fields before entering a call. The frame
    /// already authenticated the complete readable supplier list. The descriptor
    /// must still be the evaluated argument, and each field must independently
    /// survive to this storage version. Unobserved fields remain unknown.
    pub(super) fn capture_view_argument_fields(
        &mut self,
        subject: &crate::machine_calls::calls::InvocationEntrySubject,
        actual: &super::StructuralTerm,
        machine: &Machine,
        state: &State,
    ) -> Result<super::StructuralTerm, &'static str> {
        self.storage_graph()
            .ok_or("the current storage graph is unavailable")?;
        let version = self
            .storage_version
            .ok_or("the call-entry storage version is unavailable")?;
        let reference =
            crate::expression_result_type_reference(self.program, machine, state, subject.argument)
                .ok_or("the evaluated argument has no declared carrier")?;
        let shell = crate::reference_shell(self.program, reference)
            .ok_or("the evaluated argument is not a reference")?;
        if !shell.access.is_readable() {
            return Err("the evaluated reference does not permit reading");
        }
        if !crate::declared_carriers_match(self.program, shell.referee, subject.type_reference) {
            return Err("the logical view carrier differs from the arriving referee");
        }
        let coordinate = self
            .captured_storage_coordinate(Some((machine, state)), subject.argument)
            .ok_or("the evaluated descriptor has no captured logical storage path")?;
        let current = self
            .current_storage_value_at(&coordinate, Some(version))
            .ok_or("the descriptor has no current value at call entry")?;
        let current_identity = current
            .into_collection_descriptor()
            .observation_identity_key()
            .ok_or("the current descriptor has no captured identity")?;
        let actual_identity = actual
            .clone()
            .into_collection_descriptor()
            .observation_identity_key()
            .ok_or("the evaluated argument has no captured identity")?;
        if current_identity != actual_identity {
            return Err("the current descriptor differs from the evaluated argument");
        }
        // A new content observation must not reuse the pointer's identity:
        // the same pointer can designate different contents after a write.
        self.capture_sequence += 1;
        let value = super::StructuralTerm::CapturedValue {
            expression: subject.argument,
            statement: arena::Handle::invalid(),
            callable: SymbolHandle::invalid(),
            occurrence: self.capture_sequence,
            result_type: subject.type_reference,
            integer_range: None,
        };
        let observed = super::super::structural_terms::owned_projection_observation(
            self.program,
            subject.type_reference,
            value,
        );
        self.intake_projected_storage_fields(
            &coordinate,
            subject.type_reference,
            &observed,
            Some(version),
            Some((subject, state)),
        );
        Ok(observed)
    }

    /// Attach surviving child observations to a fresh aggregate identity. Both
    /// whole reads and normal-call handback must perform this join: replacing
    /// the whole observation alone would hide unchanged fields behind an opaque
    /// new value. Graph-backed observations retain their history after writes;
    /// only the read cut can establish which children are still current.
    fn intake_storage_field_observations(
        &mut self,
        coordinate: &CapturedStorageCoordinate,
        reference: TypeReferenceHandle,
        value: &super::StructuralTerm,
        version: Option<StorageVersion>,
    ) {
        self.intake_projected_storage_fields(coordinate, reference, value, version, None);
    }

    fn intake_projected_storage_fields(
        &mut self,
        coordinate: &CapturedStorageCoordinate,
        reference: TypeReferenceHandle,
        value: &super::StructuralTerm,
        version: Option<StorageVersion>,
        view: Option<(&crate::machine_calls::calls::InvocationEntrySubject, &State)>,
    ) {
        let mut children = self
            .stored_values
            .iter()
            .filter(|stored| {
                stored.coordinate.root == coordinate.root
                    && stored.coordinate.segments.len() > coordinate.segments.len()
                    && stored.coordinate.segments.starts_with(&coordinate.segments)
            })
            .map(|stored| stored.coordinate.clone())
            .collect::<Vec<_>>();
        // A field need not have been read separately: the old aggregate may
        // already observe it. Ask the graph about each field before replacing
        // that aggregate, descending through changed inline records only.
        // Do not expand arrays into every element or traverse reference
        // descriptors; individually observed indices remain in `children`.
        children.push(coordinate.clone());
        let mut next_child = 0;
        let mut equations = Vec::new();
        while let Some(child) = children.get(next_child).cloned() {
            next_child += 1;
            let suffix = &child.segments[coordinate.segments.len()..];
            let Some(projected_type) = suffix.iter().try_fold(reference, |reference, segment| {
                segment.projected_type(self.program, reference)
            }) else {
                continue;
            };
            // Passing a checked view to a helper keeps the fields already read
            // through that same view. Its scalar may span several backing bytes,
            // so physical leaf translation is not required for that observation.
            // Both the logical read and a translated read must still be current
            // at this exact call-entry version; neither revives a retired value.
            let current = self.current_storage_value_at(&child, version);
            let translated = view.and_then(|(subject, state)| {
                self.current_view_field(subject, state, suffix, version)
            });
            if !suffix.is_empty()
                && (current.is_some() || translated.is_some())
                && let Some((_, projection)) = project_captured_value_by(
                    self.program,
                    reference,
                    value.clone(),
                    suffix,
                    |reference, value, segment| {
                        super::super::observed_projections::project_observed_value_in_scope(
                            self.program,
                            self,
                            reference,
                            value,
                            std::slice::from_ref(segment),
                        )
                    },
                )
            {
                for observation in [current, translated].into_iter().flatten() {
                    equations.push((projected_type, projection.clone(), observation));
                }
                continue;
            }
            if !suffix.is_empty() && crate::reference_shell(self.program, projected_type).is_some()
            {
                continue;
            }
            // A malformed recursive inline carrier may reach proof checking
            // before its layout refusal. Do not unfold the same applied type
            // again on this path; finite nested applications remain distinct.
            let mut ancestor_type = reference;
            if suffix.iter().any(|segment| {
                let repeats = ancestor_type == projected_type;
                if let Some(child_type) = segment.projected_type(self.program, ancestor_type) {
                    ancestor_type = child_type;
                }
                repeats
            }) {
                continue;
            }
            let Some(definition) =
                crate::field_domains::readable_nominal_definition(self.program, projected_type)
            else {
                continue;
            };
            for field in self
                .program
                .data_members(definition)
                .iter()
                .filter_map(|member| member.as_field())
            {
                let mut field_coordinate = child.clone();
                field_coordinate
                    .segments
                    .push(CapturedStorageSegment::Place(PlaceSegment::Field {
                        symbol: field.symbol,
                    }));
                if !children.contains(&field_coordinate) {
                    children.push(field_coordinate);
                }
            }
        }
        for (declared, projection, current) in equations {
            self.assume_completed_value_equation(declared, projection, current);
        }
    }

    /// A view's fields can have different symbols from the original record.
    /// Translate each logical leaf through the authenticated recast chain, then
    /// read its physical contents at call entry. All possible suppliers must
    /// provide the same observed value; unknown alternatives remain unknown.
    fn current_view_field(
        &self,
        subject: &crate::machine_calls::calls::InvocationEntrySubject,
        state: &State,
        suffix: &[CapturedStorageSegment],
        version: Option<StorageVersion>,
    ) -> Option<super::StructuralTerm> {
        let path = suffix
            .iter()
            .map(|segment| match segment {
                CapturedStorageSegment::Place(segment) => Some(*segment),
                CapturedStorageSegment::Index(super::StructuralTerm::Integer(index)) => {
                    Some(PlaceSegment::FixedIndex {
                        index: usize::try_from(index.to_i64()?).ok()?,
                    })
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let suppliers = subject.scalar_projection_coordinates(self.program, state, &path)?;
        let mut common = None;
        for scalar in suppliers {
            let coordinate = self.capture_storage_place(scalar.place)?;
            let current = self.current_storage_value_at(&coordinate, version);
            if super::super::struct_trace() {
                eprintln!(
                    "STRUCT view-field-observation argument={:?} path={path:?} coordinate={coordinate:?} version={version:?} current={current:?}",
                    subject.argument
                );
            }
            let current = current?;
            let value = if scalar.preserves_scalar_value {
                current
            } else {
                super::super::structural_terms::normalize_scalar_representation(
                    scalar.source_primitive,
                    scalar.view_primitive,
                    current,
                )
            };
            let value = self.completed_value_identity(value);
            if !value.has_observation_identity() {
                return None;
            }
            match &common {
                Some(previous) if *previous != value => return None,
                Some(_) => {}
                None => common = Some(value),
            }
        }
        common
    }

    /// Capture a completed scalar read using the same storage roster as exact
    /// stores. Unrepresentable coordinates still have a detached event value,
    /// but cannot authorize a later read of another coordinate.
    pub(super) fn observe_completed_storage_read(
        &mut self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        value: super::StructuralTerm,
    ) {
        let Some(place) = self.captured_storage_coordinate(Some((machine, state)), expression)
        else {
            return;
        };
        // A state-local judgment starts at that state's independently checked
        // arrival contract. An invocation-wide judgment keeps the machine
        // entry anchor while traversing later states; never relabel its reads.
        let entry = self.declaration_state == state.symbol
            && place.root.as_symbol().is_some_and(|root| {
                self.program
                    .state_parameter_by_symbol(state, root)
                    .is_some()
                    && !self.storage_binding_invalidated(root)
            });
        let backing = self.frame_resolver().and_then(|frames| {
            frames.storage_read_backing_at_expression(
                machine,
                state,
                expression,
                place.root.as_symbol()?,
                &crate::canonical_place_from_expression(self.program, expression)?.segments,
            )
        });
        let backing = backing.map(|backing| self.freeze_scalar_read_backing(state, backing));
        match self.read_anchor(state, expression, &place) {
            Some(anchor) if !entry => {
                self.retain_anchored_storage_value(place.clone(), value, entry, anchor)
            }
            _ => self.retain_storage_value(place.clone(), value, entry),
        }
        if let Some(stored) = std::sync::Arc::make_mut(&mut self.stored_values).value_at_mut(&place)
        {
            stored.completed_read = Some((expression, self.storage_write_sequence));
            stored.backing = backing;
        }
    }

    /// Only a real read at the current write epoch can name live scalar storage.
    /// Post-state descriptor/store metadata is deliberately absent here. Every
    /// write retires this roster, including writes to a captured index selector.
    pub(in crate::proof_contracts::contract_entailment) fn current_scalar_reads(
        &self,
    ) -> impl Iterator<Item = (ExpressionHandle, &super::StructuralTerm)> {
        self.stored_values.iter().filter_map(|stored| {
            let (expression, sequence) = stored.completed_read?;
            (sequence == self.storage_write_sequence)
                .then_some((expression, stored.current.as_ref()?))
        })
    }

    pub(super) fn retain_storage_value(
        &mut self,
        coordinate: CapturedStorageCoordinate,
        value: super::StructuralTerm,
        entry: bool,
    ) {
        let anchor = if entry {
            StorageAnchor::Incoming
        } else {
            match self.storage_version {
                Some(version) if version.snapshot.is_valid() => {
                    StorageAnchor::After(version.snapshot)
                }
                _ => StorageAnchor::Incoming,
            }
        };
        self.retain_anchored_storage_value(coordinate, value, entry, anchor);
    }

    /// Record `value` as the current contents of `coordinate`, established at
    /// `anchor` in this judge's current state history.
    pub(super) fn retain_anchored_storage_value(
        &mut self,
        coordinate: CapturedStorageCoordinate,
        value: super::StructuralTerm,
        entry: bool,
        anchor: StorageAnchor,
    ) {
        let anchor_state = self
            .storage_version
            .map_or_else(SymbolHandle::invalid, |version| version.state);
        let values = std::sync::Arc::make_mut(&mut self.stored_values);
        if let Some(stored) = values.value_at_mut(&coordinate) {
            if entry && stored.entry.is_none() {
                stored.entry = Some(value.clone());
            }
            stored.current = Some(value);
            stored.current_anchors = vec![anchor];
            stored.anchor_state = anchor_state;
            stored.completed_read = None;
            stored.backing = None;
        } else {
            values.push(CapturedStorageValue {
                coordinate,
                entry: entry.then(|| value.clone()),
                current: Some(value),
                selected_case: None,
                completed_read: None,
                backing: None,
                current_anchors: vec![anchor],
                anchor_state,
            });
        }
    }

    /// Initialization installs the completed value after its operand effects.
    /// An earlier opaque write may have retired this declaration's entry
    /// observation before the binding existed; it cannot retire a later
    /// completed initializer. Future writes still invalidate this current home.
    pub(in crate::proof_contracts::contract_entailment) fn observe_completed_binding(
        &mut self,
        symbol: SymbolHandle,
        reference: TypeReferenceHandle,
        value: super::StructuralTerm,
        initializer: ExpressionHandle,
        version: Option<StorageVersion>,
    ) {
        self.advance_storage_version(version);
        if !self.requires_value_preservation() || !symbol.is_valid() {
            return;
        }
        // Owned bindings retain inline cells while masking contained pointees.
        // A completed shared borrow already carries a checked inline view of
        // its exact referent. Retain that typed observation at initialization;
        // arbitrary reference values still supply no referent observation.
        let observed =
            super::super::structural_terms::shared_inline_observation_type(self.program, &value)
                .filter(|observed| {
                    crate::declared_carriers_match(self.program, reference, *observed)
                })
                .map(|_| value.clone())
                .or_else(|| {
                    // A local pointer keeps its address when its referent can
                    // change. Retain only the completed descriptor here;
                    // subsequent reads must establish current field contents.
                    crate::reference_shell(self.program, reference).map(|_| {
                        super::super::structural_terms::descriptor_only_observation(
                            reference,
                            value.clone(),
                        )
                    })
                })
                .or_else(|| self.owned_argument_observation(reference, &value));
        let Some(value) = observed else {
            return;
        };
        self.retain_anchored_storage_value(
            CapturedStorageCoordinate {
                root: facts::PlaceRoot::Symbol(symbol),
                segments: Vec::new(),
            },
            value,
            false,
            StorageAnchor::Store(initializer),
        );
    }

    /// Rejoin a normalized content projection to the completed read of that
    /// exact parameter place and revision. This supplies no arithmetic or loan
    /// authority; absent reads and retired current values remain unavailable.
    pub(in crate::proof_contracts::contract_entailment) fn captured_parameter_projection(
        &self,
        root: SymbolHandle,
        path: &[language_semantics::content::ContentPlaceSegment],
        version: language_semantics::content::ContentPlaceVersion,
    ) -> Option<super::StructuralTerm> {
        self.requires_value_preservation().then_some(())?;
        let segments = content_storage_segments(path);
        self.captured_projection(
            &CapturedStorageCoordinate {
                root: facts::PlaceRoot::Symbol(root),
                segments,
            },
            version,
        )
    }

    /// Project an already completed value through the same typed field, case
    /// and fixed-index operation as captured storage. This never rereads an
    /// address and retains the actual leaf type beside its observed value.
    pub(in crate::proof_contracts::contract_entailment) fn completed_value_projection(
        &self,
        reference: TypeReferenceHandle,
        value: super::StructuralTerm,
        path: &[language_semantics::content::ContentPlaceSegment],
    ) -> Option<(TypeReferenceHandle, super::StructuralTerm)> {
        project_captured_value(
            self.program,
            reference,
            value,
            &content_storage_segments(path),
        )
    }

    /// Rejoin a logical subject from the current tag and every current field.
    /// A field store can retire the whole-value snapshot while preserving its
    /// tag and untouched fields. All of those observations belong to this
    /// prefix ledger, rather than a separate literal valuation's active case.
    /// Missing fields or an unknown applied tag leave the subject unknown;
    /// assembling its contents supplies no default-domain admission.
    pub(in crate::proof_contracts::contract_entailment) fn captured_data_subject(
        &self,
        place: &CanonicalPlace,
        definition: &typed_trees::data::DataDefinition,
        mut field_place: impl FnMut(symbols::SymbolHandle) -> Option<CanonicalPlace>,
    ) -> Option<super::StructuralTerm> {
        self.captured_data_subject_at(place, definition, &mut field_place, &mut Vec::new())
    }

    fn captured_data_subject_at(
        &self,
        place: &CanonicalPlace,
        definition: &typed_trees::data::DataDefinition,
        field_place: &mut dyn FnMut(symbols::SymbolHandle) -> Option<CanonicalPlace>,
        active: &mut Vec<symbols::SymbolHandle>,
    ) -> Option<super::StructuralTerm> {
        self.requires_value_preservation().then_some(())?;
        if active.contains(&definition.symbol) {
            return None;
        }
        active.push(definition.symbol);
        let coordinate = self.capture_storage_place(place.clone())?;
        let active_case = self
            .stored_values
            .value_at(&coordinate)
            .and_then(|stored| stored.selected_case);
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT stored-subject data={} place={place:?} case={active_case:?}",
                definition.name,
            );
        }
        let variant = match active_case {
            Some(symbol) => Some(
                self.program
                    .data_members(definition)
                    .iter()
                    .filter_map(typed_trees::data::DataMember::as_variant)
                    .find(|variant| variant.symbol == symbol)?,
            ),
            None => {
                if self
                    .program
                    .data_members(definition)
                    .iter()
                    .any(|member| member.as_variant().is_some())
                {
                    return None;
                }
                None
            }
        };
        let subject = super::super::structural_terms::captured_constructor_term(
            self.program,
            definition,
            variant,
            |field| {
                let selected = field_place(field.symbol)?;
                let value = self.current_place_projection(&selected).or_else(|| {
                    // A retired aggregate has no whole snapshot. Its inline
                    // child may still have a complete current field roster.
                    // Reference descriptors are never rebuilt from contents.
                    crate::reference_shell(self.program, field.type_reference)
                        .is_none()
                        .then_some(())?;
                    let child = crate::value_custody::places::data_definition_for_type(
                        self.program,
                        field.type_reference,
                    )?;
                    self.captured_data_subject_at(
                        &selected,
                        child,
                        &mut |symbol| {
                            let mut field = selected.clone();
                            crate::value_custody::places::push_field_place_segments(
                                self.program,
                                &mut field.segments,
                                symbol,
                            );
                            Some(field)
                        },
                        active,
                    )
                });
                if super::super::struct_trace() {
                    eprintln!("STRUCT stored-subject field={} value={value:?}", field.name,);
                }
                value
            },
        );
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT stored-subject data={} value={subject:?}",
                definition.name
            );
        }
        active.pop();
        subject
    }

    /// The current contents of one place for a stored-predicate judgment.
    /// A captured observation wins. Otherwise an entry formal's cell that no
    /// recorded exposure has touched still holds its entry contents: the same
    /// declaration projection an executed read would observe, under the same
    /// exposure rule `storage_observation_invalidated` applies to such reads.
    pub(in crate::proof_contracts::contract_entailment) fn current_place_projection(
        &self,
        place: &CanonicalPlace,
    ) -> Option<super::StructuralTerm> {
        self.captured_place_projection(
            place,
            language_semantics::content::ContentPlaceVersion::Current,
        )
        .or_else(|| self.untouched_entry_projection(place))
    }

    /// Project an entry formal's declaration value along fields and live tags.
    /// A case segment needs the ledger's surviving tag for its exact prefix;
    /// indexed and reference-bearing cells retain their captured-read fence.
    /// Only an owned formal or an exclusive formal of reference-free storage
    /// qualifies: nothing else can write its cells without an exposure here,
    /// which is what lets this skip the scalar read-backing authentication.
    fn untouched_entry_projection(&self, place: &CanonicalPlace) -> Option<super::StructuralTerm> {
        self.requires_value_preservation().then_some(())?;
        let root = place.root.as_symbol()?;
        let (_, state) = self.runtime_declaration_scope()?;
        let parameter = self.program.state_parameter_by_symbol(state, root)?;
        let frames = self.frame_resolver()?;
        let exclusive = match crate::reference_shell(self.program, parameter.type_reference) {
            None => frames.value_is_reference_free(parameter.type_reference),
            Some(shell) => {
                shell.access == language_semantics::ReferenceAccess::Mutable
                    && frames.value_is_reference_free(shell.referee)
            }
        };
        if !exclusive {
            return None;
        }
        // The storage graph answers per place whether incoming storage still
        // reaches it; without a graph, any recorded write to the root fences.
        if self.graph_storage_mode() {
            if self.graph_place_invalidated(place) {
                return None;
            }
        } else if self.invalidated_storage.contains(&root) {
            return None;
        }
        // The formal's own exclusive shell names its caller-owned referent;
        // any further reference belongs to a separate live epoch.
        let referent = crate::reference_shell(self.program, parameter.type_reference)
            .map_or(parameter.type_reference, |shell| shell.referee);
        let reference = crate::value_custody::places::owned_projection_type_reference(
            self.program,
            referent,
            &place.segments,
        )?;
        let mut value = super::StructuralTerm::Variable(parameter.name.to_string());
        let mut prefix = CapturedStorageCoordinate {
            root: place.root,
            segments: Vec::new(),
        };
        for segment in &place.segments {
            match segment {
                PlaceSegment::Case { variant } => {
                    self.stored_values
                        .iter()
                        .find(|stored| stored.coordinate == prefix)
                        .and_then(|stored| stored.selected_case)
                        .filter(|selected| selected == variant)?;
                }
                PlaceSegment::Field { symbol } => {
                    value = super::super::observed_projections::project_field(
                        value,
                        self.program.symbols.name(*symbol),
                    )?;
                }
                _ => return None,
            }
            prefix
                .segments
                .push(CapturedStorageSegment::Place(*segment));
        }
        frames.value_is_reference_free(reference).then_some(value)
    }

    /// Query a captured place rather than reconstructing its initializer.
    /// Current local/call-result storage and an entry parameter use the same
    /// revision ledger; claim forwarding alone never establishes byte equality.
    pub(in crate::proof_contracts::contract_entailment) fn captured_place_projection(
        &self,
        place: &CanonicalPlace,
        version: language_semantics::content::ContentPlaceVersion,
    ) -> Option<super::StructuralTerm> {
        self.requires_value_preservation().then_some(())?;
        let coordinate = self.capture_storage_place(place.clone())?;
        let value = self.captured_projection(&coordinate, version);
        if super::super::struct_trace() {
            eprintln!(
                "STRUCT current-place-projection coordinate={coordinate:?} requested={version:?} storage={:?} value={value:?}",
                self.storage_version,
            );
        }
        value
    }

    fn captured_projection(
        &self,
        coordinate: &CapturedStorageCoordinate,
        version: language_semantics::content::ContentPlaceVersion,
    ) -> Option<super::StructuralTerm> {
        use language_semantics::content::ContentPlaceVersion;
        if version == ContentPlaceVersion::Current {
            // A write may leave the old payload recorded while invalidating
            // its storage version. Only the current-value reader checks that
            // version; its refusal must not fall back to the old payload.
            return self.current_storage_value(coordinate);
        }
        if let Some(stored) = self.stored_values.value_at(coordinate) {
            let value = stored.entry.clone();
            if value.is_some() {
                return value;
            }
        }
        // An entry snapshot retains its fields after later writes. Project
        // that snapshot when the leaf was not captured separately.
        let reference =
            crate::declaration_type_reference(self.program, coordinate.root.as_symbol()?)?;
        let whole = CapturedStorageCoordinate {
            root: coordinate.root,
            segments: Vec::new(),
        };
        self.stored_values.values_at(&whole).find_map(|stored| {
            let value = stored.entry.clone()?;
            project_captured_value(self.program, reference, value, &coordinate.segments)
                .map(|(_, value)| value)
        })
    }

    pub(super) fn capture_store_destination(
        &self,
        machine: &typed_trees::machine::Machine,
        state: &State,
        statement: &typed_trees::statement::StatementNode,
    ) -> Option<CapturedStoreDestination> {
        let frame = self
            .frame_resolver()?
            .assignment_write_frame(machine, statement);
        let stored_type = crate::assignment_stored_value_type(
            self.program,
            machine,
            state,
            statement.as_assignment()?,
        )?;
        let captured = frame
            .complete_paths()
            .and_then(|written| self.exact_store_coordinate(machine, state, statement, written));
        let (coordinate, representation, stored_integer) = match captured {
            Some((coordinate, representation, stored_integer)) => {
                (Some(coordinate), representation, stored_integer)
            }
            None => (None, None, None),
        };
        if coordinate.is_none() && !self.requires_value_preservation() {
            return None;
        }
        Some(CapturedStoreDestination {
            coordinate,
            frame,
            version: self.statement_node_storage_version(state, statement),
            value_expression: statement.as_assignment()?.value,
            stored_type,
            representation,
            stored_integer,
        })
    }

    /// Exact post-store value equations require a single authenticated backing.
    /// Failure to capture it does not cancel the store: its frame still retires
    /// live observations, conservatively all of them when that frame is opaque.
    fn exact_store_coordinate(
        &self,
        machine: &typed_trees::machine::Machine,
        state: &State,
        statement: &typed_trees::statement::StatementNode,
        written: &[String],
    ) -> Option<(
        CapturedStorageCoordinate,
        Option<(
            typed_trees::types::PrimitiveType,
            typed_trees::types::PrimitiveType,
        )>,
        Option<semantic_vocabulary::IntegerType>,
    )> {
        let assignment = statement.as_assignment()?;
        crate::place_has_builtin_coordinates(self.program, machine, Some(state), assignment.target)
            .then_some(())?;
        let program = self.program;
        let frames = self.frame_resolver()?;
        let origins = frames.assignment_storage_origins(machine, statement).ok()?;
        let mut representation = None;
        let mut stored_integer = None;
        let place = match origins.destinations.as_slice() {
            [origin]
                if origin.root.is_valid()
                    && !origin.is_image_storage
                    && origin.recasts.is_empty()
                    && origin.representation.is_none() =>
            {
                CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(origin.root),
                    segments: origin.segments.clone(),
                }
            }
            [origin] => {
                let scalar = origin.scalar_storage_coordinate(program, state)?;
                if !scalar.preserves_scalar_value && !scalar.preserves_primitive_bits {
                    // A stored-width codec is not a native bitcast of the wide
                    // carrier. A view that covers the whole cell still decodes.
                    stored_integer = Some(complete_stored_integer_cell(
                        program,
                        &scalar.place,
                        scalar.view_primitive,
                        scalar.source_primitive,
                    )?);
                } else {
                    representation = (!scalar.preserves_scalar_value)
                        .then_some((scalar.view_primitive, scalar.source_primitive));
                }
                scalar.place
            }
            [] => {
                // Replacing a plain owned binding names the binding itself;
                // the frame owner intentionally publishes no referent origins.
                let place = crate::value_custody::places::canonical_place_from_expression(
                    program,
                    assignment.target,
                )?;
                let root = place.root.as_symbol()?;
                let declared = crate::state_root_type(program, state, root)?;
                if !place.segments.is_empty() || !frames.value_is_reference_free(declared) {
                    return None;
                }
                place
            }
            // Multiple backing alternatives cannot install one exact value.
            _ => return None,
        };
        // A footprint identifies possible writes, not an unchanged reference
        // slot. Retain the actual captured coordinate only while RHS effects
        // preserve the address inputs by which this semantic place names it.
        frames
            .captured_assignment_destination_remains_current(machine, statement, &place)
            .then_some(())?;
        let root = place.root.as_symbol()?;
        let footprint = match origins.destinations.as_slice() {
            [destination] => destination.write_path(program)?,
            [] => crate::frame_coordinate_path(program, root, &place.segments)?,
            _ => return None,
        };
        if written != [footprint] {
            return None;
        }
        // Frame paths deliberately coarsen collections for invalidation only.
        // Exact storage retains the already completed selector value identities.
        Some((
            self.capture_storage_place(place)?,
            representation,
            stored_integer,
        ))
    }

    pub(super) fn observe_captured_store(
        &mut self,
        state: &State,
        destination: CapturedStoreDestination,
        value: super::StructuralTerm,
    ) -> Option<()> {
        let destination_value_expression = destination.value_expression;
        // The outward frame deliberately coarsens indexed stores. This actual
        // store additionally owns one authenticated, already evaluated place.
        // Preserve observed cells proved disjoint from that place, using the
        // current roster after RHS effects, never the pre-RHS snapshot. Calls
        // and opaque/multiple destinations still use only their complete frame.
        // A cell the graph no longer holds before this store stays retired.
        let mut preserved = Vec::new();
        if let Some(written) = &destination.coordinate {
            let frames = self.frame_resolver()?;
            for stored in self.stored_values.iter() {
                if !stored
                    .coordinate
                    .has_stable_backing(self.program, state, frames)
                    || !self.exact_storage_coordinates_disjoint(&stored.coordinate, written)
                    || self
                        .storage_version
                        .is_some_and(|at| !self.current_value_valid(stored, &stored.coordinate, at))
                {
                    continue;
                }
                let Some(current) = stored.current.clone() else {
                    continue;
                };
                let Some(reference) = stored.coordinate.root.as_symbol().and_then(|root| {
                    stored.coordinate.segments.iter().try_fold(
                        crate::declaration_type_reference(self.program, root)?,
                        |reference, segment| segment.projected_type(self.program, reference),
                    )
                }) else {
                    continue;
                };
                // A disjoint descriptor slot preserves its words, not an old
                // observation of potentially aliased storage behind the slot.
                preserved.push((
                    stored.coordinate.clone(),
                    super::super::structural_terms::owned_projection_observation(
                        self.program,
                        reference,
                        current,
                    ),
                ));
            }
        }
        self.observe_storage_effects(
            state,
            destination.frame.complete_paths(),
            destination.version,
        )?;
        for (coordinate, current) in preserved {
            self.retain_storage_value(coordinate, current, false);
        }
        if let Some(coordinate) = destination.coordinate {
            let value = if let Some(stored) = destination.stored_integer {
                decode_stored_integer_term(self.program, destination.stored_type, stored, value)
            } else {
                match destination.representation {
                    Some((source, target)) => {
                        super::super::structural_terms::normalize_scalar_representation(
                            source, target, value,
                        )
                    }
                    None => value,
                }
            };
            if super::super::struct_trace() {
                eprintln!(
                    "STRUCT install-store owner={} type={:?} value={value:?} coordinate={coordinate:?} version={:?}",
                    self.program.symbols.display_path(self.machine_symbol, "::"),
                    self.program
                        .type_reference_table
                        .type_reference(destination.stored_type),
                    destination.version,
                );
            }
            let value = if self
                .frame_resolver()
                .is_some_and(|frames| frames.value_is_reference_free(destination.stored_type))
            {
                value
            } else {
                super::super::structural_terms::owned_projection_observation(
                    self.program,
                    destination.stored_type,
                    value,
                )
            };
            // Record what this store wrote to its observed destination, even
            // when the graph tracks the write at an enclosing record. A later
            // read must still prove that no overlapping write changed it.
            let anchor = match destination.version {
                Some(version) => StorageAnchor::After(version.snapshot),
                None => StorageAnchor::Store(destination_value_expression),
            };
            self.retain_anchored_storage_value(coordinate.clone(), value, false, anchor);
            self.retain_written_case_tags(&coordinate);
        }
        Some(())
    }

    /// A checked store through a case field has already selected that case.
    /// Record only the tag of each containing storage value. This supplies no
    /// default-domain admission, named membership, sibling contents or loan.
    fn retain_written_case_tags(&mut self, destination: &CapturedStorageCoordinate) {
        let program = self.program;
        let Some(root) = destination.root.as_symbol() else {
            return;
        };
        let Some(mut reference) = crate::declaration_type_reference(program, root) else {
            return;
        };
        let mut prefix = CapturedStorageCoordinate {
            root: destination.root,
            segments: Vec::new(),
        };
        for segment in &destination.segments {
            if let CapturedStorageSegment::Place(facts::PlaceSegment::Case { variant }) = segment
                && crate::value_custody::places::data_definition_for_type(program, reference)
                    .is_some_and(|definition| {
                        program
                            .data_members(definition)
                            .iter()
                            .filter_map(typed_trees::data::DataMember::as_variant)
                            .any(|case| case.symbol == *variant)
                    })
            {
                let values = std::sync::Arc::make_mut(&mut self.stored_values);
                if let Some(stored) = values.value_at_mut(&prefix) {
                    stored.selected_case = Some(*variant);
                } else {
                    values.push(CapturedStorageValue {
                        coordinate: prefix.clone(),
                        entry: None,
                        current: None,
                        selected_case: Some(*variant),
                        completed_read: None,
                        backing: None,
                        current_anchors: Vec::new(),
                        anchor_state: SymbolHandle::invalid(),
                    });
                }
            }
            let Some(projected) = segment.projected_type(program, reference) else {
                break;
            };
            reference = projected;
            prefix.segments.push(segment.clone());
        }
    }

    /// A common backing and equal prefix identify the collection whose frozen
    /// selectors are compared. Different roots, unknown selectors and prefix
    /// writes supply no non-interference proof. Case tags are not disjoint
    /// storage: changing a variant may replace its entire payload.
    fn exact_storage_coordinates_disjoint(
        &self,
        observed: &CapturedStorageCoordinate,
        written: &CapturedStorageCoordinate,
    ) -> bool {
        if observed.root != written.root {
            return false;
        }
        for (observed, written) in observed.segments.iter().zip(&written.segments) {
            if observed == written {
                continue;
            }
            return match (observed, written) {
                (
                    CapturedStorageSegment::Place(facts::PlaceSegment::Field { symbol: left }),
                    CapturedStorageSegment::Place(facts::PlaceSegment::Field { symbol: right }),
                ) => left != right,
                (CapturedStorageSegment::Index(left), CapturedStorageSegment::Index(right)) => {
                    matches!(
                        self.judge_equation(
                            self.resolve_at(left.clone(), 0),
                            self.resolve_at(right.clone(), 0),
                            0,
                        ),
                        super::StructuralJudgment::Refuted
                    )
                }
                _ => false,
            };
        }
        false
    }

    /// A slice descriptor in a private owned home is a completed value,
    /// independent of backing contents. Project only authenticated fixed fields
    /// of that retained value; never reconstruct an initializer or cross a
    /// reference to a replaceable descriptor slot.
    pub(super) fn completed_descriptor_place(
        &self,
        scope: Option<(
            &typed_trees::machine::Machine,
            &State,
            typed_trees::types::TypeReferenceHandle,
        )>,
        expression: ExpressionHandle,
        environment: &[(String, super::StructuralTerm)],
    ) -> Option<super::StructuralTerm> {
        if !self.requires_value_preservation() {
            return None;
        }
        let (machine, state, _) = scope?;
        if !self.ordinary_storage_path(Some((machine, state)), expression) {
            return None;
        }
        let place = crate::value_custody::places::canonical_place_from_expression(
            self.program,
            expression,
        )?;
        let root = place.root.as_symbol()?;
        if !self.storage_binding_invalidated(root) {
            return None;
        }
        // A coarse outward frame can describe writes through a view without
        // distinguishing its backing from its carrier slot. The binding owner
        // separately establishes that this private descriptor home is neither
        // replaced nor exposed; referent snapshots retain the stronger fence.
        let reference =
            crate::stable_slice_descriptor_place(self.program, state, root, &place.segments)?;
        let root_reference = self
            .program
            .state_parameter_by_symbol(state, root)
            .map(|parameter| parameter.type_reference)
            .or_else(|| {
                self.program
                    .state_local_declaration(state, root)
                    .map(|(_, local)| local.type_reference)
            })?;
        let binding = self.program.symbols.name(root);
        let mut values = environment.iter().filter(|(name, _)| name == binding);
        let value = values.next()?.1.clone();
        if values.next().is_some() {
            return None;
        }
        let value = super::super::observed_projections::project_observed_value(
            self.program,
            root_reference,
            value,
            &place.segments,
        )?;
        Some(super::super::structural_terms::descriptor_only_observation(
            reference, value,
        ))
    }

    /// Source-only engines do not consume the current storage roster. They
    /// may use a declaration observation only while its entry epoch survives.
    pub(super) fn entry_storage_observations_preserved(
        &self,
        expression: ExpressionHandle,
    ) -> bool {
        if !self.requires_value_preservation() {
            return true;
        }
        let mut expressions = Vec::new();
        crate::queries::expression_traversal::collect_expression_tree(
            self.program,
            expression,
            &mut expressions,
        );
        !expressions
            .into_iter()
            .any(|expression| self.storage_observation_invalidated(expression))
    }

    pub(super) fn storage_observation_invalidated(&self, expression: ExpressionHandle) -> bool {
        if self.graph_storage_mode() {
            return self.graph_observation_invalidated(expression);
        }
        crate::value_custody::places::expression_root_symbol(
            expression,
            &self.program.expression_table,
            self.machine_symbol,
        )
        .is_some_and(|symbol| self.invalidated_storage.contains(&symbol))
    }

    pub(super) fn storage_binding_invalidated(&self, symbol: SymbolHandle) -> bool {
        if self.graph_storage_mode() {
            return self.graph_binding_invalidated(symbol);
        }
        self.invalidated_storage.contains(&symbol)
    }

    /// An immutable ordinary scalar has no write authority or reference-bearing
    /// payload through which an opaque callee could replace its private copy.
    /// Atomic cells are storage, not such values; classify their sealed atoms
    /// independently of the primitive width they use.
    pub(super) fn immutable_scalar_binding(&self, symbol: SymbolHandle) -> bool {
        self.immutable_scalar_place(&CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments: Vec::new(),
        })
    }

    /// A call can change storage reached through an owned value without
    /// replacing its immutable inline scalar fields. Follow only owned
    /// projections: a stored reference, atomic cell or opaque projection
    /// cannot acquire this exemption from observation invalidation.
    pub(super) fn immutable_scalar_place(&self, place: &CanonicalPlace) -> bool {
        let Some(symbol) = place.root.as_symbol() else {
            return false;
        };
        if !symbol.is_valid() {
            return false;
        }
        let program = self.program;
        let owner = program.symbols.get(symbol).parent;
        let declaration = crate::call_target_parameters(program, owner)
            .and_then(|parameters| {
                parameters
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)
            })
            .map(|parameter| (parameter.is_mutable, parameter.type_reference))
            .or_else(|| {
                let state = program.state_by_symbol(owner).or_else(|| {
                    program
                        .machine_by_symbol(owner)
                        .and_then(|machine| program.machine_states(machine).first())
                })?;
                let (_, local) = program.state_local_declaration(state, symbol)?;
                Some((local.is_mutable, local.type_reference))
            });
        let Some((false, reference)) = declaration else {
            return false;
        };
        let Some(reference) = crate::value_custody::places::owned_projection_type_reference(
            program,
            reference,
            &place.segments,
        ) else {
            return false;
        };
        let reference = program
            .type_reference_table
            .unconstrained_type_reference(reference);
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => {
                program
                    .symbols
                    .builtin_type_atom(*symbol)
                    .is_some_and(|atom| !atom.is_atomic())
                    && program.primitive_type_reference(reference).is_some()
            }
            typed_trees::types::TypeReferenceNode::DynamicTrait { .. } => true,
            _ => false,
        }
    }
}

/// A same-width view of one IntegerAt cell. Every bit pattern the view can
/// write is a value of this encoding, so the semantic carrier's post-write
/// domain is the encoding itself. A narrower or partial view does not qualify:
/// those bits do not determine the stored integer.
pub(crate) fn complete_stored_integer_cell(
    program: &dyn crate::CheckingInput,
    place: &CanonicalPlace,
    view: typed_trees::types::PrimitiveType,
    source: typed_trees::types::PrimitiveType,
) -> Option<semantic_vocabulary::IntegerType> {
    if !view.is_fixed_integer() || !source.is_fixed_integer() {
        return None;
    }
    let view_bits = view.integer_sign_bits()?.1;
    let source_bits = source.integer_sign_bits()?.1;
    let (facts::PlaceSegment::Field { symbol }, parent_segments) = place.segments.split_last()?
    else {
        return None;
    };
    let root = place.root.as_symbol()?;
    let mut root_type = crate::value_custody::places::declaration_type_reference(program, root)?;
    // A state parameter is a reference to the record. Field projection starts
    // at the referent, not at the loan descriptor.
    for _ in 0..4 {
        let Some(shell) = crate::reference_shell(program, root_type) else {
            break;
        };
        root_type = shell.referee;
    }
    let parent = crate::project_type_reference_from_segments(program, root_type, parent_segments)?;
    let semantic = semantic_layout_carrier(program, parent);
    let stored = integer_at_field(program, *symbol).or_else(|| {
        crate::proof_contracts::arithmetic_domains::stored_integer_field_type(
            program, semantic, *symbol,
        )
        .ok()
        .flatten()
    })?;
    let view_width = u16::try_from(view.scalar_byte_size()?.checked_mul(8)?).ok()?;
    if stored.bits() == 0
        || stored.bits() >= 64
        || stored.bits() != view_bits
        || stored.bits() != view_width
        || stored.bits() > source_bits
    {
        return None;
    }
    let field_type = program.any_field_owner(*symbol)?.1.type_reference;
    (program.primitive_type_reference(field_type) == Some(source)).then_some(stored)
}

/// The encoding domain satisfies every interval fact on `field`. A declaration
/// whose facts are not all field intervals stays unproved: a cross-field
/// conjunct is not discharged by the stored width alone.
pub(crate) fn stored_encoding_preserves_field(
    program: &dyn crate::CheckingInput,
    place: &CanonicalPlace,
    view: typed_trees::types::PrimitiveType,
    source: typed_trees::types::PrimitiveType,
    definition: &typed_trees::data::DataDefinition,
    field: SymbolHandle,
) -> bool {
    let Some(stored) = complete_stored_integer_cell(program, place, view, source) else {
        return false;
    };
    let Some((low, high)) = stored_integer_domain(stored) else {
        return false;
    };
    let Some(bounds) = crate::data_where_field_intervals(program, definition) else {
        return false;
    };
    let mut named = false;
    for (symbol, bound_low, bound_high) in bounds {
        if symbol != field {
            continue;
        }
        named = true;
        if bound_low.is_some_and(|minimum| low < i128::from(minimum))
            || bound_high.is_some_and(|maximum| high > i128::from(maximum))
        {
            return false;
        }
    }
    named
}

/// The IntegerAt row for one semantic field. The plan is owned by the
/// layout-storage application, not by the semantic record's own instance.
fn integer_at_field(
    program: &dyn crate::CheckingInput,
    field: SymbolHandle,
) -> Option<semantic_vocabulary::IntegerType> {
    let typed = program.typed_program();
    let mut found = None;
    for plan in typed.plan_laid_layouts.iter() {
        let Some(index) = plan
            .field_symbols
            .iter()
            .position(|candidate| *candidate == field)
        else {
            continue;
        };
        // Proof runs before a single layout target is selected. Every
        // alternative must name the same IntegerAt row; a disagreement or a
        // missing row leaves the field's encoding unknown.
        let mut agreed = None;
        let mut saw_geometry = false;
        for geometry in plan.geometries(typed) {
            saw_geometry = true;
            let Some(integer) = geometry
                .integer_fields
                .iter()
                .find(|integer| integer.field_index == index)
            else {
                agreed = None;
                break;
            };
            let sign = match integer.interpretation {
                layout_plans::IntegerInterpretation::Signed => {
                    semantic_vocabulary::IntegerSign::Signed
                }
                layout_plans::IntegerInterpretation::Unsigned => {
                    semantic_vocabulary::IntegerSign::Unsigned
                }
            };
            let Some(decoded) =
                semantic_vocabulary::IntegerType::new(sign, integer.stored_width_bits).ok()
            else {
                agreed = None;
                break;
            };
            if agreed.is_some_and(|prior| prior != decoded) {
                agreed = None;
                break;
            }
            agreed = Some(decoded);
        }
        let Some(decoded) = saw_geometry.then_some(agreed).flatten() else {
            continue;
        };
        if found.is_some_and(|prior| prior != decoded) {
            return None;
        }
        found = Some(decoded);
    }
    found
}

fn semantic_layout_carrier(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> TypeReferenceHandle {
    let reference = crate::value_custody::places::unwrapped_type_reference(program, reference)
        .unwrap_or(reference);
    program
        .layout_storage_arguments(reference)
        .map(|storage| storage.semantic_type)
        .unwrap_or(reference)
}

/// Inclusive semantic domain of a narrowed stored integer. Full-width encodings
/// are the carrier itself and are not rewritten here.
pub(in crate::proof_contracts::contract_entailment) fn stored_integer_domain(
    stored: semantic_vocabulary::IntegerType,
) -> Option<(i128, i128)> {
    let width = stored.bits();
    if width == 0 || width >= 64 {
        return None;
    }
    match stored.sign() {
        semantic_vocabulary::IntegerSign::Signed => {
            let half = 1i128 << (width - 1);
            Some((-half, half - 1))
        }
        semantic_vocabulary::IntegerSign::Unsigned => Some((0, (1i128 << width) - 1)),
    }
}

fn decode_stored_integer_literal(
    raw: &BigInt,
    view: typed_trees::types::PrimitiveType,
    stored: semantic_vocabulary::IntegerType,
) -> Option<i128> {
    let raw = raw.to_i64()?;
    let (view_low, view_high) = view.integer_bounds()?;
    let raw_wide = i128::from(raw);
    if raw_wide < view_low || raw_wide > view_high {
        return None;
    }
    let width = u32::from(stored.bits());
    if width == 0 || width >= 64 {
        return None;
    }
    let mask = (1u64 << width) - 1;
    let bits = (raw as u64) & mask;
    let decoded = match stored.sign() {
        semantic_vocabulary::IntegerSign::Signed => {
            let sign = 1u64 << (width - 1);
            if bits & sign == 0 {
                i128::from(bits)
            } else {
                i128::from(bits) - (1i128 << width)
            }
        }
        semantic_vocabulary::IntegerSign::Unsigned => i128::from(bits),
    };
    let (low, high) = stored_integer_domain(stored)?;
    (low..=high).contains(&decoded).then_some(decoded)
}

/// The semantic integer a complete stored-width view write installs. A known
/// literal decodes its bits. Any other value of that view still lands in the
/// encoding's domain, which is the fact a later `requires` check can use.
fn decode_stored_integer_term(
    program: &dyn crate::CheckingInput,
    view_type: TypeReferenceHandle,
    stored: semantic_vocabulary::IntegerType,
    value: super::StructuralTerm,
) -> super::StructuralTerm {
    let Some(view) = program.primitive_type_reference(view_type) else {
        return value;
    };
    if let super::StructuralTerm::Integer(raw) = value.semantic_value()
        && let Some(decoded) = decode_stored_integer_literal(raw, view, stored)
    {
        return super::StructuralTerm::Integer(BigInt::from_i128(decoded));
    }
    let Some((low, high)) = stored_integer_domain(stored) else {
        return value;
    };
    super::StructuralTerm::CapturedValue {
        statement: arena::Handle::invalid(),
        expression: ExpressionHandle::invalid(),
        callable: SymbolHandle::invalid(),
        occurrence: 0,
        result_type: view_type,
        integer_range: Some(std::sync::Arc::new(
            checked_trees::IntegerRangeRecipe::bounds(
                BigInt::from_i128(low),
                BigInt::from_i128(high),
            ),
        )),
    }
}
