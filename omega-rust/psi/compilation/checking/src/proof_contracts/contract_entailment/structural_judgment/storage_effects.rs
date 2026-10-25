//! Writes retire live storage observations, not completed value identities.
//!
//! The normal-return path owns effect order. Its complete caller-relative frame
//! may replace a binding's current value while earlier copies and equations
//! still describe their captured values. A later read of retired storage gets
//! a new unknown observation; unrelated owned bindings retain their facts.
//! A reference-free aggregate keeps a sibling field when the exposure names a
//! different field. The whole root still dies when the write covers it.
//! Owned reference-bearing roots can retain inline cells across disjoint
//! writes, with typed projections masking the old pointee contents. References
//! and opaque reachable storage retain their conservative alias fence.

/// Exclusive exposures recorded for one judge. Opaque means the frame did not
/// name places, so every invalidated binding stays retired. A path list lets a
/// reference-free field read survive a loan of a different field.
#[derive(Clone, Default)]
pub(super) struct StorageExposureRecord {
    pub(super) opaque: bool,
    pub(super) paths: Vec<String>,
}

use super::StructuralJudge;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

type StorageScope<'scope> = Option<(&'scope Machine, &'scope State)>;

/// A canonical place with live selectors replaced by completed value identities.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct CapturedStorageCoordinate {
    root: facts::PlaceRoot,
    segments: Vec<CapturedStorageSegment>,
}

impl CapturedStorageCoordinate {
    /// Frame coordinates coarsen every collection selection to its container.
    /// Captured indices remain exact for reading, but cannot claim disjointness
    /// that the effect summary did not retain.
    fn frame_path(&self, program: &dyn crate::CheckingInput) -> Option<String> {
        let root = self.root.as_symbol()?;
        let mut segments = Vec::new();
        for segment in &self.segments {
            match segment {
                CapturedStorageSegment::Place(segment) => segments.push(*segment),
                CapturedStorageSegment::Index(_) => break,
            }
        }
        crate::frame_coordinate_path(program, root, &segments)
    }

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

#[derive(Clone, PartialEq, Eq)]
enum CapturedStorageSegment {
    Place(facts::PlaceSegment),
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
    mut reference: TypeReferenceHandle,
    mut value: super::StructuralTerm,
    path: &[CapturedStorageSegment],
) -> Option<super::StructuralTerm> {
    for segment in path {
        let projected = segment.projected_type(program, reference)?;
        value = match segment {
            CapturedStorageSegment::Place(segment) => {
                super::super::observed_projections::project_observed_value(
                    program,
                    reference,
                    value,
                    std::slice::from_ref(segment),
                )?
            }
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
    Some(value)
}

/// One evaluated place keeps its detached entry snapshot beside its live value.
/// Writes retire only the latter. An entry value is captured by an actual read
/// before that parameter is invalidated, never reconstructed from an initializer.
#[derive(Clone)]
pub(super) struct CapturedStorageValue {
    coordinate: CapturedStorageCoordinate,
    entry: Option<super::StructuralTerm>,
    current: Option<super::StructuralTerm>,
}

/// One authenticated store destination, frozen before replacement evaluation.
#[derive(Clone)]
pub(super) struct CapturedStoreDestination {
    coordinate: Option<CapturedStorageCoordinate>,
    frame: facts::NormalizedWriteFrame,
    stored_type: TypeReferenceHandle,
}

fn expression_exposure_path(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<String> {
    let place = crate::value_custody::places::canonical_place_from_expression(program, expression)?;
    let root = place.root.as_symbol()?;
    crate::frame_coordinate_path(program, root, &place.segments)
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn observe_storage_writes(
        &mut self,
        state: &State,
        written: &[String],
    ) -> Option<()> {
        self.observe_storage_effects(state, Some(written))
    }

    /// Unknown effects retire every live storage observation, not immutable
    /// model premises or detached values captured before the call. Absence of
    /// a complete frame is never an empty frame or evidence of non-interference.
    pub(in crate::proof_contracts::contract_entailment) fn observe_storage_effects(
        &mut self,
        state: &State,
        written: Option<&[String]>,
    ) -> Option<()> {
        if written.is_some_and(<[String]>::is_empty) || !self.requires_value_preservation() {
            return Some(());
        }
        {
            let exposures = std::sync::Arc::make_mut(&mut self.storage_exposures);
            match written {
                None => exposures.opaque = true,
                Some(paths) => exposures.paths.extend(paths.iter().cloned()),
            }
        }
        self.storage_write_sequence += 1;
        let program = self.program;
        let preservation = {
            let frames = self.frame_resolver()?;
            self.stored_values
                .iter()
                .map(|stored| {
                    let stable = written.is_some_and(|written| {
                        stored.coordinate.has_stable_backing(program, state, frames)
                            && stored.coordinate.frame_path(program).is_some_and(|path| {
                                written
                                    .iter()
                                    .all(|written| !crate::frame_paths_overlap(written, &path))
                            })
                    });
                    let inline = stored
                        .coordinate
                        .root
                        .as_symbol()
                        .and_then(|root| crate::declaration_type_reference(program, root))
                        .filter(|reference| crate::reference_shell(program, *reference).is_none())
                        .and_then(|reference| {
                            stored
                                .coordinate
                                .segments
                                .iter()
                                .try_fold(reference, |reference, segment| {
                                    segment.projected_type(program, reference)
                                })
                        })
                        .filter(|reference| !frames.value_is_reference_free(*reference));
                    (stable, inline)
                })
                .collect::<Vec<_>>()
        };
        // Preserve only independently named, disjoint live storage. Completed
        // values keep their identities regardless; this decision concerns the
        // current contents at their former addresses, not detached copies.
        let mut preservation = preservation.into_iter();
        std::sync::Arc::make_mut(&mut self.stored_values).retain_mut(|stored| {
            let (stable, inline) = preservation.next().unwrap_or((false, None));
            if !stable {
                stored.current = None;
            } else if let Some(reference) = inline
                && let Some(value) = stored.current.take()
            {
                // Retain inline cells only. Typed reference projection keeps
                // descriptor words while erasing the old pointee observation.
                stored.current = Some(
                    super::super::structural_terms::owned_projection_observation(
                        program, reference, value,
                    ),
                );
            }
            stored.entry.is_some() || stored.current.is_some()
        });
        let frames = self.frame_resolver()?;
        let mut invalidated = Vec::new();
        let mut consider = |symbol: SymbolHandle, reference| {
            let overlaps = written.is_none_or(|written| {
                crate::frame_coordinate_path(program, symbol, &[]).is_none_or(|path| {
                    written
                        .iter()
                        .any(|written| crate::frame_paths_overlap(written, &path))
                })
            });
            if overlaps || !frames.value_is_reference_free(reference) {
                invalidated.push(symbol);
            }
        };
        for parameter in program.state_parameters(state) {
            consider(parameter.symbol, parameter.type_reference);
        }
        // A named state may receive an earlier state's reference under a new
        // binder. The exit contract still names the original binder; retiring
        // only this state's spelling would revive its entry observation. As
        // with current reference-bearing roots, a write conservatively retires
        // reference-bearing parameters in the enclosing invocation as well.
        if let Some(machine) = program.machine(self.machine_symbol) {
            for earlier in program.machine_states(machine) {
                if earlier.symbol == state.symbol {
                    continue;
                }
                for parameter in program.state_parameters(earlier) {
                    if written.is_none()
                        || !frames.value_is_reference_free(parameter.type_reference)
                    {
                        consider(parameter.symbol, parameter.type_reference);
                    }
                }
                if written.is_none() {
                    for statement in program.statement_table.statements(earlier.statement_nodes) {
                        if let Some(local) = statement.as_local_data() {
                            consider(local.symbol, local.type_reference);
                        }
                    }
                }
            }
            if written.is_none() {
                if let Some(data) =
                    crate::value_custody::places::machine_attached_data(program, machine)
                {
                    for member in program.data_members(data) {
                        if let typed_trees::data::DataMember::Field(field) = member {
                            consider(field.symbol, field.type_reference);
                        }
                    }
                }
            }
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            if let Some(local) = statement.as_local_data() {
                consider(local.symbol, local.type_reference);
            }
        }
        if written.is_none() {
            invalidated.push(self.machine_symbol);
        }
        let storage = std::sync::Arc::make_mut(&mut self.invalidated_storage);
        for symbol in invalidated {
            if !storage.contains(&symbol) {
                storage.push(symbol);
            }
        }
        // These rows are source predicates, unlike the captured term equations.
        // A later branch must not reinterpret an old entry fact over new storage.
        // A reference-free field fact stays when the exposure names a different
        // field; a fact about the lent field or the whole aggregate does not.
        let mut facts = std::mem::take(&mut self.premise_facts);
        facts.retain(|fact| {
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(program, *fact, &mut expressions);
            !expressions
                .iter()
                .any(|expression| self.storage_observation_invalidated(*expression))
        });
        self.premise_facts = facts;
        Some(())
    }

    fn ordinary_storage_path(&self, scope: StorageScope<'_>, expression: ExpressionHandle) -> bool {
        match self.program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Name(_) => true,
            typed_trees::expression::ExpressionNode::Member(member) => {
                self.ordinary_storage_path(scope, member.receiver)
            }
            typed_trees::expression::ExpressionNode::Indexed(indexed) => {
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
        subject: &crate::machine_calls::calls::InvocationReferenceSubject,
    ) -> Option<CapturedStorageCoordinate> {
        // A finite origin join authorizes an entry observation, not one
        // physical write destination. No selected payload identifies an alias.
        let [source] = subject.sources.as_slice() else {
            return None;
        };
        self.capture_storage_place(crate::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(source.root),
            segments: source.segments.clone(),
        })
    }

    fn capture_storage_place(
        &self,
        place: crate::CanonicalPlace,
    ) -> Option<CapturedStorageCoordinate> {
        let segments = place
            .segments
            .into_iter()
            .map(|segment| match segment {
                facts::PlaceSegment::FixedIndex { index } => Some(CapturedStorageSegment::Index(
                    super::StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(
                        u64::try_from(index).ok()?,
                    )),
                )),
                facts::PlaceSegment::Index { expression } => {
                    let completed = self
                        .normal_results
                        .iter()
                        .find(|(source, _)| *source == expression);
                    let (_, value) = completed?;
                    let value = self.completed_value_identity(value.clone());
                    value.observation_identity_key()?;
                    Some(CapturedStorageSegment::Index(value))
                }
                facts::PlaceSegment::Field { symbol }
                    if symbol.is_valid()
                        && self.program.symbols.get(symbol).kind == symbols::SymbolKind::Field =>
                {
                    Some(CapturedStorageSegment::Place(segment))
                }
                facts::PlaceSegment::Case { variant } if variant.is_valid() => {
                    Some(CapturedStorageSegment::Place(segment))
                }
                facts::PlaceSegment::Field { .. } | facts::PlaceSegment::Case { .. } => None,
                // A window is not one storage coordinate. Unknown selection cannot
                // borrow the identity of its source expression or an arbitrary slot.
                facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => None,
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
    ) -> Option<super::StructuralTerm> {
        // Case-only judgments intentionally do not track storage writes.
        if self.stored_values.is_empty() || !self.requires_value_preservation() {
            return None;
        }
        let place = self.captured_storage_coordinate(scope, expression)?;
        self.current_storage_value(&place)
    }

    pub(super) fn current_storage_value(
        &self,
        place: &CapturedStorageCoordinate,
    ) -> Option<super::StructuralTerm> {
        if let Some(value) = self
            .stored_values
            .iter()
            .find(|stored| &stored.coordinate == place)
            .and_then(|stored| stored.current.clone())
        {
            return Some(value);
        }
        // A fresh aggregate observation owns its fields too. Projection uses
        // that exact current value, never the retired source initializer.
        self.stored_values.iter().find_map(|stored| {
            if stored.coordinate.root != place.root
                || !place.segments.starts_with(&stored.coordinate.segments)
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
                    CapturedStorageSegment::Place(facts::PlaceSegment::Field { symbol }) => {
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
                        let projected = project_captured_value(
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
        let equations = self
            .stored_values
            .iter()
            .filter_map(|stored| {
                if stored.coordinate.root != coordinate.root
                    || stored.coordinate.segments.len() <= coordinate.segments.len()
                    || !stored.coordinate.segments.starts_with(&coordinate.segments)
                {
                    return None;
                }
                let current = stored.current.clone()?;
                let projection = project_captured_value(
                    self.program,
                    reference,
                    value.clone(),
                    &stored.coordinate.segments[coordinate.segments.len()..],
                )?;
                let projected_type = stored.coordinate.segments[coordinate.segments.len()..]
                    .iter()
                    .try_fold(reference, |reference, segment| {
                        segment.projected_type(self.program, reference)
                    })?;
                Some((projected_type, projection, current))
            })
            .collect::<Vec<_>>();
        for (declared, projection, current) in equations {
            self.assume_completed_value_equation(declared, projection, current);
        }
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
        self.retain_storage_value(place, value, entry);
    }

    pub(super) fn retain_storage_value(
        &mut self,
        coordinate: CapturedStorageCoordinate,
        value: super::StructuralTerm,
        entry: bool,
    ) {
        let values = std::sync::Arc::make_mut(&mut self.stored_values);
        if let Some(stored) = values
            .iter_mut()
            .find(|stored| stored.coordinate == coordinate)
        {
            if entry && stored.entry.is_none() {
                stored.entry = Some(value.clone());
            }
            stored.current = Some(value);
        } else {
            values.push(CapturedStorageValue {
                coordinate,
                entry: entry.then(|| value.clone()),
                current: Some(value),
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
    ) {
        if !self.requires_value_preservation() || !symbol.is_valid() {
            return;
        }
        // A completed owned binding retains its inline cells. The same typed
        // observation as an owned argument masks contained pointee contents;
        // a local reference alias is not an owned storage namespace.
        let Some(value) = self.owned_argument_observation(reference, &value) else {
            return;
        };
        self.retain_storage_value(
            CapturedStorageCoordinate {
                root: facts::PlaceRoot::Symbol(symbol),
                segments: Vec::new(),
            },
            value,
            false,
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
        use language_semantics::content::ContentPlaceSegment;
        self.requires_value_preservation().then_some(())?;
        let segments = path
            .iter()
            .map(|segment| match segment {
                ContentPlaceSegment::Field(field) => {
                    CapturedStorageSegment::Place(facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    })
                }
                ContentPlaceSegment::Case(case) => {
                    CapturedStorageSegment::Place(facts::PlaceSegment::Case {
                        variant: case.symbol,
                    })
                }
                ContentPlaceSegment::FixedIndex(index) => CapturedStorageSegment::Index(
                    super::StructuralTerm::Integer(numerics::bignum::BigInt::from_u64(*index)),
                ),
            })
            .collect::<Vec<_>>();
        self.captured_projection(
            &CapturedStorageCoordinate {
                root: facts::PlaceRoot::Symbol(root),
                segments,
            },
            version,
        )
    }

    /// Query a captured place rather than reconstructing its initializer.
    /// Current local/call-result storage and an entry parameter use the same
    /// revision ledger; claim forwarding alone never establishes byte equality.
    pub(in crate::proof_contracts::contract_entailment) fn captured_place_projection(
        &self,
        place: &crate::CanonicalPlace,
        version: language_semantics::content::ContentPlaceVersion,
    ) -> Option<super::StructuralTerm> {
        self.requires_value_preservation().then_some(())?;
        let coordinate = self.capture_storage_place(place.clone())?;
        self.captured_projection(&coordinate, version)
    }

    fn captured_projection(
        &self,
        coordinate: &CapturedStorageCoordinate,
        version: language_semantics::content::ContentPlaceVersion,
    ) -> Option<super::StructuralTerm> {
        use language_semantics::content::ContentPlaceVersion;
        if version == ContentPlaceVersion::Current
            && let Some(value) = self.current_storage_value(coordinate)
        {
            return Some(value);
        }
        if let Some(stored) = self
            .stored_values
            .iter()
            .find(|stored| stored.coordinate == *coordinate)
        {
            let value = match version {
                ContentPlaceVersion::Entry => stored.entry.clone(),
                ContentPlaceVersion::Current => stored.current.clone(),
            };
            if value.is_some() {
                return value;
            }
        }
        // Whole-value observations retain typed case/field structure even
        // without an individually captured leaf. Project the requested
        // captured revision; a retired current snapshot cannot borrow Entry.
        let reference =
            crate::declaration_type_reference(self.program, coordinate.root.as_symbol()?)?;
        self.stored_values.iter().find_map(|stored| {
            if stored.coordinate.root != coordinate.root || !stored.coordinate.segments.is_empty() {
                return None;
            }
            let value = match version {
                ContentPlaceVersion::Entry => stored.entry.clone(),
                ContentPlaceVersion::Current => stored.current.clone(),
            }?;
            project_captured_value(self.program, reference, value, &coordinate.segments)
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
        let coordinate = frame
            .complete_paths()
            .and_then(|written| self.exact_store_coordinate(machine, state, statement, written));
        if coordinate.is_none() && !self.requires_value_preservation() {
            return None;
        }
        Some(CapturedStoreDestination {
            coordinate,
            frame,
            stored_type,
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
    ) -> Option<CapturedStorageCoordinate> {
        let assignment = statement.as_assignment()?;
        self.ordinary_storage_path(Some((machine, state)), assignment.target)
            .then_some(())?;
        let program = self.program;
        let frames = self.frame_resolver()?;
        let origins = frames.assignment_storage_origins(machine, statement)?;
        let place = match origins.destinations.as_slice() {
            [origin]
                if origin.root.is_valid()
                    && !origin.is_image_storage
                    && origin.recasts.is_empty()
                    && origin.representation.is_none() =>
            {
                crate::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(origin.root),
                    segments: origin.segments.clone(),
                }
            }
            [] => {
                // Replacing a plain owned binding names the binding itself;
                // the frame owner intentionally publishes no referent origins.
                let place = crate::value_custody::places::canonical_place_from_expression(
                    program,
                    assignment.target,
                )?;
                let root = place.root.as_symbol()?;
                let declared = program
                    .state_parameter_by_symbol(state, root)
                    .map(|parameter| parameter.type_reference)
                    .or_else(|| {
                        program
                            .state_local_declaration(state, root)
                            .map(|(_, local)| local.type_reference)
                    })?;
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
            [destination] => destination.write_path(program),
            [] => crate::frame_coordinate_path(program, root, &place.segments)?,
            _ => return None,
        };
        if written != [footprint] {
            return None;
        }
        // Frame paths deliberately coarsen collections for invalidation only.
        // Exact storage retains the already completed selector value identities.
        self.capture_storage_place(place)
    }

    pub(super) fn observe_captured_store(
        &mut self,
        state: &State,
        destination: CapturedStoreDestination,
        value: super::StructuralTerm,
    ) -> Option<()> {
        self.observe_storage_effects(state, destination.frame.complete_paths())?;
        if let Some(coordinate) = destination.coordinate {
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
            self.retain_storage_value(coordinate, value, false);
        }
        Some(())
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
        if self.invalidated_storage.is_empty() || !self.requires_value_preservation() {
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
        crate::expression_traversal::collect_expression_tree(
            self.program,
            expression,
            &mut expressions,
        );
        !expressions
            .into_iter()
            .any(|expression| self.storage_observation_invalidated(expression))
    }

    pub(super) fn storage_observation_invalidated(&self, expression: ExpressionHandle) -> bool {
        let Some(symbol) = crate::value_custody::places::expression_root_symbol(
            expression,
            &self.program.expression_table,
            self.machine_symbol,
        ) else {
            return false;
        };
        if !self.invalidated_storage.contains(&symbol) {
            return false;
        }
        // Reference-bearing and opaque exposures still retire the binding.
        // A reference-free field read consults the recorded place and survives
        // a loan of a different field of the same aggregate.
        if self.storage_exposures.opaque || !self.root_exposure_is_precise(symbol) {
            return true;
        }
        let Some(path) = expression_exposure_path(self.program, expression) else {
            return true;
        };
        self.storage_exposures
            .paths
            .iter()
            .any(|written| crate::frame_paths_overlap(written, &path))
    }

    /// Owned storage and an exclusive formal of owned storage name exact
    /// fields. Any other root can alias, so a disjoint spelling is not enough.
    fn root_exposure_is_precise(&self, symbol: SymbolHandle) -> bool {
        let Some(frames) = self.frame_resolver() else {
            return false;
        };
        let Some(reference) = crate::declaration_type_reference(self.program, symbol) else {
            return false;
        };
        if frames.value_is_reference_free(reference) {
            return true;
        }
        let Some(machine) = self.program.machine(self.machine_symbol) else {
            return false;
        };
        let state = if self.declaration_state.is_valid() {
            self.program
                .state_in_machine(machine, self.declaration_state)
        } else {
            self.program.machine_states(machine).first()
        };
        let Some(state) = state else {
            return false;
        };
        self.program
            .state_parameter_by_symbol(state, symbol)
            .is_some_and(|parameter| {
                crate::reference_shell(self.program, parameter.type_reference).is_some_and(
                    |shell| {
                        shell.access == language_semantics::ReferenceAccess::Mutable
                            && frames.value_is_reference_free(shell.referee)
                    },
                )
            })
    }

    pub(super) fn storage_binding_invalidated(&self, symbol: SymbolHandle) -> bool {
        self.invalidated_storage.contains(&symbol)
    }
}
