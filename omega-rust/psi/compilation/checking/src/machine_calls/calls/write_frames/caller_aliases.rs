//! Caller-prefix alias closure for public write-frame demand.
//!
//! The state walker owns alias transfer. This leaf locates a query's statement
//! and closes its writes over the canonical storage paths and live local names
//! used by fact consumers. It never publishes prefix writes as call writes.

pub use super::place_paths::RecastSource;
use super::stored_origins::{StoredLocalOrigins, expand_write_paths, place_suffix};
use super::{
    ExpressionHandle, ExpressionNode, FrameInference, FramePathPrecision, FramePlaceOrigin,
    Machine, StatementNode, SymbolHandle, TableCall, TopLevelSymbols, append_place_suffix,
    memoized, statement_value_expression_roots, type_is_caller_isolated_local,
    type_may_carry_write,
};
use crate::declarations::symbols::PrefixSiteEntry;
use crate::machine_calls::calls::write_frames::state_write_walk::{
    CollectedStatementPrefix, collected_prefix_at,
};
use std::sync::Mutex;
use symbols::SymbolKeyMap as HashMap;
use typed_trees::signature::StateParameter;

pub(super) fn caller_binding_type(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<super::TypeReferenceHandle> {
    let name = program.expression_table.name_path_node(argument)?;
    let [_] = program.expression_table.name_path_members(name.members) else {
        return None;
    };
    caller_name_root_type(program, current_machine, argument)
}

/// Validate the root declaration of a retained name path without treating a
/// projected member as a standalone reference binding.
pub(super) fn caller_name_root_type(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<super::TypeReferenceHandle> {
    let name = program.expression_table.name_path_node(argument)?;
    let members = program.expression_table.name_path_members(name.members);
    let member = members.first()?;
    let root = name.head_symbol;
    if !root.is_valid() || (members.len() == 1 && root != name.symbol) {
        return None;
    }
    let (state, _, index) = caller_statement_owner(
        program,
        current_machine,
        CallerWriteSite::Expression(argument),
    )?;
    let declaration = program.symbols.get(root);
    if declaration.parent != state.symbol || program.symbols.name(root) != member.as_str() {
        return None;
    }
    let reference = match declaration.kind {
        symbols::SymbolKind::Parameter => {
            program
                .state_parameter_by_symbol(state, root)?
                .type_reference
        }
        symbols::SymbolKind::Local => {
            let (_, local) = program
                .state_local_declaration(state, root)
                .filter(|&(ordinal, _)| ordinal < index)?;
            local.type_reference
        }
        _ => return None,
    };
    reference.is_valid().then_some(reference)
}

#[derive(Clone, Copy)]
pub(crate) enum CallerWriteSite<'query> {
    Call(&'query TableCall),
    Statement(&'query StatementNode),
    Expression(ExpressionHandle),
    Transition(typed_trees::statement::TransitionTargetHandle),
}

/// Transient caller-prefix inventory for projecting local references into
/// another place representation. Unknown sources remain explicit rows: the
/// consumer first selects overlapping leaves, then requires their sources.
/// Omitting an unknown row would misclassify its referent as private storage.
#[derive(Clone)]
pub struct LocalWriteOrigin {
    pub local_symbol: SymbolHandle,
    /// Empty for a bare reference; otherwise the exact reference leaf selector.
    /// A type-derived array leaf uses Index with a zero expression handle to
    /// denote any element. This is may-write evidence, never access authority.
    pub local_segments: Vec<facts::PlaceSegment>,
    pub source: Option<LocalWriteSource>,
}

/// Store destinations and live reference bindings from one captured prefix.
/// Consumers compare footprints; these rows grant no equality or access.
pub struct AssignmentStorageOrigins {
    pub destinations: Vec<LocalWriteSource>,
    pub references: Option<Vec<LocalWriteOrigin>>,
}

/// Storage captured by one exact invocation. Each operand keeps all possible
/// origins; `None` is unknown and `Some([])` means no caller storage. These
/// coordinates do not grant permission to access the referents.
#[derive(Clone)]
pub struct CallWriteOrigins {
    pub receiver: Option<Vec<LocalWriteSource>>,
    pub arguments: Vec<Option<Vec<LocalWriteSource>>>,
}

/// Proven referent of a local reference leaf. Coarse observer paths stay
/// coarse; exact structured coordinates are composed and checked independently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalWriteSource {
    /// Literal-produced image backing cannot alias writable declaration storage.
    /// This category survives projection independently of coordinate precision;
    /// an absent declaration root alone never establishes static backing.
    pub is_image_storage: bool,
    pub recasts: Vec<RecastSource>,
    /// Logical recast carrier and selectors, separate from the raw backing.
    pub representation: Option<(super::TypeReferenceHandle, Vec<facts::PlaceSegment>)>,
    pub path: String,
    pub collection_coarse: bool,
    /// Structural selectors retained before string-path coarsening. Consumers
    /// must validate the declaration path; runtime selectors are not snapshots.
    /// A zero root withholds primitive-coordinate precision without removing
    /// the coarse source path.
    pub root: SymbolHandle,
    pub segments: Vec<facts::PlaceSegment>,
}

/// Complete chronological source of one returned reference leaf. This is
/// construction evidence for origin publication, not a new lifetime or loan.
/// Unresolved leaves are unavailable, never an empty result roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceResultLeafOrigin {
    pub result_path: Vec<facts::PlaceSegment>,
    pub source: LocalWriteSource,
}

impl LocalWriteSource {
    /// Substitute relative storage using the frame owner's coordinate algebra.
    /// Text paths are only display/footprint payload; they never select fields.
    pub fn substitute_storage(&self, relative: &Self) -> Option<Self> {
        if !self.root.is_valid() || !relative.root.is_valid() {
            return None;
        }
        let coordinate = |source: &Self| super::place_paths::FrameSourcePlace {
            recasts: source.recasts.clone(),
            root: source.root,
            segments: source.segments.clone(),
            representation: source.representation.clone(),
            builtin_coordinates: true,
            ..Default::default()
        };
        let source = coordinate(self).append_relative(&coordinate(relative));
        Some(Self {
            is_image_storage: self.is_image_storage,
            recasts: source.recasts,
            root: source.root,
            segments: source.segments,
            representation: source.representation,
            // Keep the coarse observer path; exact selectors live above.
            path: self.path.clone(),
            collection_coarse: self.collection_coarse || relative.collection_coarse,
        })
    }
}

impl From<FramePlaceOrigin> for LocalWriteSource {
    fn from(origin: FramePlaceOrigin) -> Self {
        Self {
            is_image_storage: origin.source.is_static(),
            recasts: origin.source.recasts,
            representation: origin.source.representation,
            path: origin.path,
            collection_coarse: origin.precision == FramePathPrecision::CollectionCoarse,
            root: if origin.source.builtin_coordinates {
                origin.source.root
            } else {
                SymbolHandle::invalid()
            },
            segments: origin.source.segments,
        }
    }
}

/// The direct assignment effect, excluding calls evaluated in its operands.
/// A binding replacement changes the local slot, not its previous referent.
#[derive(Clone)]
pub enum AssignmentWriteTarget {
    LocalBindingReplacement { path: String },
    Storage { paths: Vec<String> },
}

struct AssignmentEvidence {
    target: AssignmentWriteTarget,
    aliases: Vec<(String, FramePlaceOrigin)>,
    divergent: Vec<(String, Vec<FramePlaceOrigin>)>,
    stored: Vec<StoredLocalOrigins>,
}

/// A state's recorded assignment results, as `walk_state_assignments`
/// returns them; the resolver walks each state once.
pub(super) type StateAssignments<'lookup> = dyn Fn(
        &typed_trees::state::State,
    ) -> std::sync::Arc<Vec<(usize, super::state_write_walk::AssignmentPrefix)>>
    + 'lookup;

pub(super) fn assignment_write_target(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
    state_assignments: &StateAssignments<'_>,
) -> Option<AssignmentWriteTarget> {
    assignment_evidence(program, machine, symbols, statement, state_assignments)
        .map(|evidence| evidence.target)
}

pub(super) fn assignment_write_paths(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
    state_assignments: &StateAssignments<'_>,
) -> Option<Vec<String>> {
    let evidence = assignment_evidence(program, machine, symbols, statement, state_assignments)?;
    match evidence.target {
        AssignmentWriteTarget::LocalBindingReplacement { path } => Some(vec![path]),
        AssignmentWriteTarget::Storage { paths } => close_over_origins(
            program,
            paths,
            &evidence.aliases,
            &evidence.divergent,
            &evidence.stored,
        ),
    }
}

/// Address provenance of the performed store, not the contents before it.
/// The same recorded assignment prefix distinguishes replacing a reference
/// slot from writing its referent and retains every current alias alternative.
pub(super) fn assignment_storage_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    machine_symbols: &super::MachineSymbols<'_>,
    statement: &StatementNode,
    state_assignments: &StateAssignments<'_>,
) -> Option<AssignmentStorageOrigins> {
    let (state, _, index) =
        caller_statement_owner(program, machine, CallerWriteSite::Statement(statement))?;
    let assignments = state_assignments(state);
    let (_, evidence) = assignments.iter().find(|(ordinal, _)| *ordinal == index)?;
    if matches!(
        evidence.target,
        AssignmentWriteTarget::LocalBindingReplacement { .. }
    ) {
        return Some(AssignmentStorageOrigins {
            destinations: Vec::new(),
            references: Some(Vec::new()),
        });
    }
    let assignment = statement.as_assignment()?;
    let mut inference = FrameInference::for_state(state.symbol);
    for local in &evidence.stored {
        inference.record_local(local);
    }
    // Destination provenance and operand exposure belong to this same write
    // prefix. Reconstructing a shared-reference transport prefix here rejects
    // an earlier established exclusive reborrow, even when it cannot replace
    // the destination binding. Current operand effects must still preserve the
    // captured slots: neither a complete write frame nor a scalar result alone
    // proves that the address remains the one recorded before evaluation.
    if super::stored_origins::expression_exposes_frozen_binding(
        program,
        machine,
        state,
        assignment.value,
        &evidence.stored,
        &evidence.aliases,
        symbols,
        &mut inference,
    ) || super::local_aliases::expression_reborrows_unresolved_reference_binding(
        program,
        machine,
        assignment.value,
        program.state_parameters(state),
        &evidence.isolated_locals,
        &evidence.aliases,
        &evidence.divergent,
    ) {
        return None;
    }
    let origins = super::alias_origins::stable_alias_initializer_origins(
        program,
        machine,
        machine_symbols,
        &mut inference,
        assignment.target,
        program.state_parameters(state),
        &evidence.isolated_locals,
        &evidence.aliases,
        &evidence.divergent,
        symbols,
        true,
        &evidence.stored,
    )?;
    // A stable place can name a stored reference descriptor. Performed writes
    // target its captured referents, not the private carrier slot. Resolve
    // every selected reference leaf; unresolved candidates invalidate the
    // judgment rather than silently dropping a possible destination.
    let mut destinations = Vec::new();
    for origin in origins {
        for destination in super::stored_origins::canonical_reference_origins(
            program,
            &Some(origin),
            &[],
            &evidence.stored,
        ) {
            let destination = LocalWriteSource::from(destination?);
            if !destinations.contains(&destination) {
                destinations.push(destination);
            }
        }
    }
    Some(AssignmentStorageOrigins {
        destinations,
        references: prefix_reference_origins(
            program,
            Some((state, index)),
            &evidence.stored,
            &evidence.aliases,
            &evidence.divergent,
        ),
    })
}

fn assignment_evidence(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
    state_assignments: &StateAssignments<'_>,
) -> Option<AssignmentEvidence> {
    let assignment = statement.as_assignment()?;
    let site = caller_prefix_site(
        program,
        machine,
        symbols,
        CallerWriteSite::Statement(statement),
    )?;
    if matches!(site, CallerPrefixSite::Untracked)
        && let Some(path) = super::coarse_place_path(program, assignment.target)
    {
        return Some(AssignmentEvidence {
            target: AssignmentWriteTarget::Storage { paths: vec![path] },
            aliases: Vec::new(),
            divergent: Vec::new(),
            stored: Vec::new(),
        });
    }
    let state = match site {
        CallerPrefixSite::Tracked { state, .. } => state,
        CallerPrefixSite::Untracked => program.machine_states(machine).iter().find(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|candidate| std::ptr::eq(statement, candidate))
        })?,
    };
    // One exact prefix supplies both the target and alias closure. Storage
    // writes leave the prefix origins intact; binding replacements return only
    // their slot and must not be closed over the slot's former referent. The
    // state's single recorded walk holds that prefix for every assignment.
    let index = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .position(|candidate| std::ptr::eq(candidate, statement))?;
    let assignments = state_assignments(state);
    let (_, prefix) = assignments
        .iter()
        .find(|(recorded, _)| *recorded == index)?;
    Some(AssignmentEvidence {
        target: prefix.target.clone(),
        aliases: prefix.aliases.clone(),
        divergent: prefix.divergent.clone(),
        stored: prefix.stored.clone(),
    })
}

pub(super) fn local_write_origins_before_statement(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    statement: &StatementNode,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<Vec<LocalWriteOrigin>> {
    let site = caller_prefix_site(
        program,
        machine,
        symbols,
        CallerWriteSite::Statement(statement),
    )?;
    let evidence = match site {
        CallerPrefixSite::Tracked { state, index, .. } => {
            let statements = program.statement_table.statements(state.statement_nodes);
            let statement = statements.get(index)?;
            tracked_prefix_evidence(
                collections,
                program,
                machine,
                state,
                statement,
                index,
                symbols,
            )?
        }
        CallerPrefixSite::Untracked => CallerPrefixEvidence {
            state: None,
            isolated_locals: Vec::new(),
            aliases: Vec::new(),
            divergent: Vec::new(),
            stored: Vec::new(),
        },
    };
    let position = match site {
        CallerPrefixSite::Tracked { state, index, .. } => Some((state, index)),
        CallerPrefixSite::Untracked => None,
    };
    prefix_reference_origins(
        program,
        position,
        &evidence.stored,
        &evidence.aliases,
        &evidence.divergent,
    )
}

pub(super) fn prefix_reference_origins(
    program: &dyn crate::CheckingInput,
    position: Option<(&typed_trees::state::State, usize)>,
    stored: &[StoredLocalOrigins],
    aliases: &[(String, FramePlaceOrigin)],
    divergent: &[(String, Vec<FramePlaceOrigin>)],
) -> Option<Vec<LocalWriteOrigin>> {
    let mut origins = stored
        .iter()
        .flat_map(|local| &local.references)
        .map(|leaf| LocalWriteOrigin {
            local_symbol: leaf.local_symbol,
            local_segments: leaf.local_segments.clone(),
            source: leaf.origin.clone().map(LocalWriteSource::from),
        })
        .collect::<Vec<_>>();
    if aliases.is_empty() && divergent.is_empty() {
        return Some(origins);
    }
    let (state, index) = position?;
    let preceding = &program.statement_table.statements(state.statement_nodes)[..index];
    let local_symbol = |name: &str| {
        preceding
            .iter()
            .find_map(|candidate| match candidate {
                StatementNode::LocalData(local) if local.name.as_str() == name => {
                    Some(local.symbol)
                }
                _ => None,
            })
            .filter(|symbol| {
                symbol.is_valid()
                    && program.symbols.get(*symbol).parent == state.symbol
                    && matches!(
                        program.symbols.get(*symbol).kind,
                        symbols::SymbolKind::Local
                    )
            })
    };
    let alias_origin = |local_symbol, origin: FramePlaceOrigin| LocalWriteOrigin {
        local_symbol,
        local_segments: Vec::new(),
        source: Some(LocalWriteSource::from(origin)),
    };
    for (name, origin) in aliases {
        origins.push(alias_origin(local_symbol(name)?, origin.clone()));
    }
    // A divergent binding names one of several proven referents. A write
    // through it may land on any of them, so each candidate is its own origin
    // of the same local; omitting the binding would let consumers treat the
    // write as private to the local.
    for (name, candidates) in divergent {
        let local_symbol = local_symbol(name)?;
        origins.extend(
            candidates
                .iter()
                .map(|origin| alias_origin(local_symbol, origin.clone())),
        );
    }
    Some(origins)
}

/// Alias maps a boundary resolve consults when it spells call-argument
/// origins: the enclosing state's parameters, the prefix's established
/// single-origin bindings and divergent referent sets, and its stored
/// carriers. Every slice is empty for an untracked site.
pub(super) struct CallOriginContext<'prefix> {
    pub parameters: &'prefix [StateParameter],
    pub isolated_locals: &'prefix [String],
    pub aliases: &'prefix [(String, FramePlaceOrigin)],
    pub divergent: &'prefix [(String, Vec<FramePlaceOrigin>)],
    pub stored: &'prefix [StoredLocalOrigins],
}

impl CallOriginContext<'_> {
    /// Expand finite local referents before a state summary filters private
    /// roots. Do not back-map canonical writes to coarse aggregate aliases;
    /// that observer-only closure belongs to the public demand boundary.
    pub fn close_paths(
        &self,
        program: &dyn crate::CheckingInput,
        paths: Vec<String>,
    ) -> Vec<String> {
        let mut expanded = Vec::new();
        for path in paths {
            let (root, suffix) = super::split_place_root(&path);
            if let Some((_, candidates)) = self.divergent.iter().find(|(name, _)| name == root) {
                for candidate in candidates {
                    let path = super::place_paths::project_write_path(program, candidate, suffix);
                    if !expanded.contains(&path) {
                        expanded.push(path);
                    }
                }
            } else if !expanded.contains(&path) {
                expanded.push(path);
            }
        }
        expanded
    }

    pub fn mentions_divergent(
        &self,
        program: &dyn crate::CheckingInput,
        expression: ExpressionHandle,
    ) -> bool {
        self.divergent.iter().any(|(name, _)| {
            super::local_aliases::expression_mentions_place_roots(
                program,
                expression,
                std::slice::from_ref(name),
            )
        })
    }

    /// Resolve actual referents before substituting a callee's relative writes.
    /// Scalar actuals need their operand effects, not a fictitious place origin.
    #[allow(clippy::too_many_arguments)]
    pub fn argument_origins(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        machine_symbols: &super::MachineSymbols<'_>,
        symbols: &TopLevelSymbols<'_>,
        inference: &mut FrameInference,
        arguments: &[ExpressionHandle],
        argument_types: &[super::TypeReferenceHandle],
    ) -> Option<Vec<Option<Vec<FramePlaceOrigin>>>> {
        arguments
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                super::local_aliases::validate_divergent_projections(
                    program,
                    machine,
                    *argument,
                    self.divergent,
                )?;
                let origins = super::alias_origins::stable_alias_initializer_origins(
                    program,
                    machine,
                    machine_symbols,
                    inference,
                    *argument,
                    self.parameters,
                    self.isolated_locals,
                    self.aliases,
                    self.divergent,
                    symbols,
                    true,
                    self.stored,
                );
                // A reference operand transports its captured referent, not
                // the carrier slot selected by the expression. Resolve that
                // boundary against this prefix before another invocation
                // substitutes its writes; unknown leaves remain unknown.
                // Owned operands retain their storage coordinate and separate
                // per-leaf inventory, not an unlabelled union of referents.
                let origins = if argument_types.get(index).is_some_and(|reference| {
                    super::type_reference_is_reference(program, *reference)
                }) {
                    origins.and_then(|origins| {
                        let mut captured = Vec::new();
                        for origin in origins {
                            for origin in super::reference_subjects::captured_value_origins(
                                program,
                                *argument,
                                &origin,
                                self.aliases,
                                self.stored,
                            ) {
                                super::place_paths::push_unique_origin(&mut captured, origin?);
                            }
                        }
                        (!captured.is_empty()).then_some(captured)
                    })
                } else {
                    origins
                };
                if origins.is_none()
                    && self.mentions_divergent(program, *argument)
                    && !argument_types.get(index).is_some_and(|reference| {
                        type_is_caller_isolated_local(program, *reference)
                            && !super::type_reference_is_reference(program, *reference)
                    })
                {
                    return None;
                }
                Some(origins)
            })
            .collect()
    }
}

/// Freeze the caller prefix once before resolving a demand, then use exactly
/// that evidence for both contextual case selection and storage closure.
pub(super) fn with_caller_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
    resolve: impl FnOnce(&mut FrameInference, &CallOriginContext<'_>) -> Option<Vec<String>>,
) -> Option<Vec<String>> {
    with_caller_origin_context(
        program,
        machine,
        symbols,
        site,
        collections,
        |inference, prefix| {
            let written = resolve(inference, prefix)?;
            close_over_origins(
                program,
                written,
                prefix.aliases,
                prefix.divergent,
                prefix.stored,
            )
        },
    )
}

/// Typed and footprint consumers share the same retained caller prefix. Only
/// the footprint wrapper performs rendered-path closure afterward.
pub(super) fn with_caller_origin_context<T>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerWriteSite<'_>,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
    resolve: impl FnOnce(&mut FrameInference, &CallOriginContext<'_>) -> Option<T>,
) -> Option<T> {
    let mut evidence = caller_aliases_at_site(program, machine, symbols, site, collections)?;
    let owner = caller_statement_owner(program, machine, site);
    if evidence.state.is_none()
        && let Some((state, _, index)) = owner
    {
        // An untracked prefix declares no local that can carry caller
        // origins, so the walk was skipped; the caller-isolated locals it
        // would have recorded are exactly the earlier owned declarations.
        evidence.isolated_locals = preceding_isolated_locals(program, state, index);
    }
    let mut inference = FrameInference::default();
    for local in &evidence.stored {
        inference.record_local(local);
    }
    let prefix = CallOriginContext {
        // The no-local-origins shortcut skips alias replay, not the current
        // state's formal storage. Typed actuals still denote those parameters.
        parameters: evidence
            .state
            .or_else(|| owner.map(|(state, _, _)| state))
            .map(|state| program.state_parameters(state))
            .unwrap_or(&[]),
        isolated_locals: &evidence.isolated_locals,
        aliases: &evidence.aliases,
        divergent: &evidence.divergent,
        stored: &evidence.stored,
    };
    resolve(&mut inference, &prefix)
}

/// The owned locals declared before statement `index` whose storage no
/// caller can see, by the same rule the state walk records them.
fn preceding_isolated_locals(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    index: usize,
) -> Vec<String> {
    program.statement_table.statements(state.statement_nodes)[..index]
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local)
                if type_is_caller_isolated_local(program, local.type_reference) =>
            {
                Some(local.name.as_str().to_owned())
            }
            _ => None,
        })
        .collect()
}

fn close_over_origins(
    program: &dyn crate::CheckingInput,
    written: Vec<String>,
    aliases: &[(String, FramePlaceOrigin)],
    divergent: &[(String, Vec<FramePlaceOrigin>)],
    stored: &[StoredLocalOrigins],
) -> Option<Vec<String>> {
    let canonical = expand_write_paths(program, &written, aliases, stored)?
        .into_iter()
        // A divergent binding's raw spelling names every referent it may
        // carry: keep the local name for fact invalidation and add each
        // candidate route, composing a projected suffix onto exact
        // candidates while a coarse candidate already covers its whole
        // storage root.
        .chain(written.iter().flat_map(|path| {
            divergent
                .iter()
                .filter(|(name, _)| place_suffix(name, path).is_some())
                .flat_map(|(name, candidates)| {
                    let suffix = place_suffix(name, path).unwrap_or_default().to_owned();
                    candidates
                        .iter()
                        .map(|candidate| {
                            super::place_paths::project_write_path(program, candidate, &suffix)
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>()
        }))
        .collect::<Vec<_>>();
    let mut closed = written;
    for path in canonical {
        if !closed.contains(&path) {
            closed.push(path.clone());
        }
        for (alias, origin, local_coarse) in aliases
            .iter()
            .map(|(alias, origin)| (alias, Some(origin), false))
            .chain(
                stored
                    .iter()
                    .flat_map(|local| &local.references)
                    .map(|leaf| {
                        (
                            &leaf.local_path,
                            leaf.origin.as_ref(),
                            leaf.local_segments.iter().any(|segment| {
                                matches!(
                                    segment,
                                    facts::PlaceSegment::FixedIndex { .. }
                                        | facts::PlaceSegment::Index { .. }
                                )
                            }),
                        )
                    }),
            )
        {
            let Some(origin) = origin else {
                // No disjointness evidence exists for this captured reference.
                // Invalidate its facts on every potentially aliasing write.
                if !closed.contains(alias) {
                    closed.push(alias.clone());
                }
                continue;
            };
            let spelling = if let Some(suffix) = place_suffix(&origin.path, &path) {
                match (origin.precision, local_coarse) {
                    (FramePathPrecision::Exact, false) if origin.source.recasts.is_empty() => {
                        append_place_suffix(alias, suffix)
                    }
                    // The canonical write already names precise backing.
                    // Observer aliases use their logical view namespace; a
                    // physical field suffix cannot be spliced into it.
                    _ => alias.clone(),
                }
            } else if place_suffix(&path, &origin.path).is_some() {
                alias.clone()
            } else {
                continue;
            };
            if !closed.contains(&spelling) {
                closed.push(spelling);
            }
        }
    }
    Some(closed)
}

/// The prefix evidence a boundary site established: the enclosing state plus
/// the alias, divergent-referent, and stored-carrier maps the write walk
/// produced before the boundary statement.
struct CallerPrefixEvidence<'program> {
    state: Option<&'program typed_trees::state::State>,
    isolated_locals: Vec<String>,
    aliases: Vec<(String, FramePlaceOrigin)>,
    divergent: Vec<(String, Vec<FramePlaceOrigin>)>,
    stored: Vec<StoredLocalOrigins>,
}

fn caller_aliases_at_site<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'program>,
    site: CallerWriteSite<'_>,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<CallerPrefixEvidence<'program>> {
    let site = caller_prefix_site(program, machine, symbols, site)?;
    caller_aliases_at_prefix(program, machine, symbols, site, collections)
}

#[derive(Clone, Copy)]
enum CallerPrefixSite<'program> {
    Untracked,
    Tracked {
        state: &'program typed_trees::state::State,
        statement: &'program StatementNode,
        index: usize,
    },
}

fn caller_site_key(site: CallerWriteSite<'_>) -> (u8, usize) {
    match site {
        CallerWriteSite::Call(call) => (0, std::ptr::from_ref(call).addr()),
        CallerWriteSite::Statement(statement) => (1, std::ptr::from_ref(statement).addr()),
        CallerWriteSite::Expression(expression) => (2, expression.arena_index() as usize),
        CallerWriteSite::Transition(target) => (3, target.arena_index() as usize),
    }
}

/// Whether any of the machine's states can carry caller origins: an incoming
/// non-reference carrier parameter, or a local declaration able to hold one.
/// The scan covers every statement of every state; it is a pure property of
/// the machine, so it memoizes on the shared symbol index rather than
/// repeating for every demand site in the machine.
fn machine_has_tracked_origins(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
) -> bool {
    memoized(
        &symbols.caller_sites.tracked_origin_machines,
        machine.symbol,
        || {
            let may_declare_origins = |statement: &StatementNode| {
                matches!(statement, StatementNode::LocalData(local)
                    if super::stored_origins::has_aggregate_case_shape(program, local.type_reference)
                        || (type_may_carry_write(program, local.type_reference)
                            && !type_is_caller_isolated_local(program, local.type_reference)))
            };
            program.machine_states(machine).iter().any(|state| {
                program.state_parameters(state).iter().any(|parameter| {
                    !super::type_reference_is_reference(program, parameter.type_reference)
                        && type_may_carry_write(program, parameter.type_reference)
                        && !type_is_caller_isolated_local(program, parameter.type_reference)
                }) || program
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .any(may_declare_origins)
            })
        },
    )
}

fn caller_prefix_site<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'program>,
    site: CallerWriteSite<'_>,
) -> Option<CallerPrefixSite<'program>> {
    let (kind, id) = caller_site_key(site);
    let entry = memoized(
        &symbols.caller_sites.prefix_site_entries,
        (machine.symbol, kind, id),
        || caller_prefix_entry(program, machine, symbols, site),
    )?;
    Some(match entry {
        PrefixSiteEntry::Untracked => CallerPrefixSite::Untracked,
        PrefixSiteEntry::Tracked {
            state: state_symbol,
            index,
        } => {
            let state = program.state_in_machine(machine, state_symbol)?;
            let statement = program.statement_table.statement_at(state, index)?;
            CallerPrefixSite::Tracked {
                state,
                statement,
                index,
            }
        }
    })
}

fn caller_prefix_entry<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'program>,
    site: CallerWriteSite<'_>,
) -> Option<PrefixSiteEntry> {
    let may_declare_origins = |statement: &StatementNode| {
        matches!(statement, StatementNode::LocalData(local)
            if super::stored_origins::has_aggregate_case_shape(program, local.type_reference)
                || (type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)))
    };
    let has_incoming_carrier = |state: &typed_trees::state::State| {
        program.state_parameters(state).iter().any(|parameter| {
            !super::type_reference_is_reference(program, parameter.type_reference)
                && type_may_carry_write(program, parameter.type_reference)
                && !type_is_caller_isolated_local(program, parameter.type_reference)
        })
    };
    if !machine_has_tracked_origins(program, machine, symbols) {
        return Some(PrefixSiteEntry::Untracked);
    }
    let (state, _statement, index) = caller_statement_at_site(program, machine, symbols, site)?;
    if !has_incoming_carrier(state)
        && !program.statement_table.statements(state.statement_nodes)[..index]
            .iter()
            .any(may_declare_origins)
    {
        return Some(PrefixSiteEntry::Untracked);
    }
    Some(PrefixSiteEntry::Tracked {
        state: state.symbol,
        index,
    })
}

/// Collect the caller-origin evidence one tracked site contributes: the
/// state's recorded prefix, unless the statement it names exposes a frozen
/// binding at that slot.
fn tracked_prefix_evidence<'program>(
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    state: &'program typed_trees::state::State,
    statement: &'program StatementNode,
    index: usize,
    symbols: &TopLevelSymbols<'_>,
) -> Option<CallerPrefixEvidence<'program>> {
    let prefix = collected_prefix_at(collections, program, machine, state, symbols, index, false)?;
    if super::stored_origins::statement_exposes_frozen_binding(
        program,
        machine,
        state,
        statement,
        &prefix.stored,
        &prefix.aliases,
        symbols,
        &mut FrameInference::for_state(state.symbol),
        false,
    ) {
        return None;
    }
    Some(CallerPrefixEvidence {
        state: Some(state),
        isolated_locals: prefix.isolated_locals,
        aliases: prefix.aliases,
        divergent: prefix.divergent,
        stored: prefix.stored,
    })
}

fn caller_aliases_at_prefix<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    site: CallerPrefixSite<'program>,
    collections: &Mutex<
        HashMap<(SymbolHandle, SymbolHandle), Option<Vec<Option<CollectedStatementPrefix>>>>,
    >,
) -> Option<CallerPrefixEvidence<'program>> {
    let CallerPrefixSite::Tracked {
        state,
        statement,
        index,
    } = site
    else {
        return Some(CallerPrefixEvidence {
            state: None,
            isolated_locals: Vec::new(),
            aliases: Vec::new(),
            divergent: Vec::new(),
            stored: Vec::new(),
        });
    };
    tracked_prefix_evidence(
        collections,
        program,
        machine,
        state,
        statement,
        index,
        symbols,
    )
}

/// Locate a unique retained occurrence without resolving declarations by name.
/// The whole-machine statement scan is a pure identity of (machine, site);
/// it memoizes on the shared symbol index, and a hit rebuilds the borrowed
/// state/statement references instead of storing addresses.
pub(super) fn caller_statement_at_site<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    symbols: &TopLevelSymbols<'program>,
    site: CallerWriteSite<'_>,
) -> Option<(
    &'program typed_trees::state::State,
    &'program StatementNode,
    usize,
)> {
    let (kind, id) = caller_site_key(site);
    let (state_symbol, index) = memoized(
        &symbols.caller_sites.site_statements,
        (machine.symbol, kind, id),
        || {
            caller_statement_owner(program, machine, site)
                .map(|(state, _statement, index)| (state.symbol, index))
        },
    )?;
    let state = program.state_in_machine(machine, state_symbol)?;
    let statement = program.statement_table.statement_at(state, index)?;
    Some((state, statement, index))
}

pub(super) fn caller_statement_owner<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    site: CallerWriteSite<'_>,
) -> Option<(
    &'program typed_trees::state::State,
    &'program StatementNode,
    usize,
)> {
    let mut owner = None;
    for state in program.machine_states(machine) {
        let statements = program.statement_table.statements(state.statement_nodes);
        for (index, statement) in statements.iter().enumerate() {
            let matches = match site {
                CallerWriteSite::Call(call) => {
                    matches!(statement, StatementNode::Call(candidate) if std::ptr::eq(call, candidate))
                }
                CallerWriteSite::Statement(candidate) => std::ptr::eq(statement, candidate),
                CallerWriteSite::Transition(target) => {
                    target.is_valid()
                        && matches!(statement, StatementNode::Transition(transition)
                        if transition.target == target || transition.continuation == target)
                }
                CallerWriteSite::Expression(expression) => {
                    statement_value_expression_roots(program, statement)
                        .into_iter()
                        .any(|root| contains_expression(program, root, expression))
                }
            };
            if matches {
                if owner.is_some() {
                    return None;
                }
                owner = Some((state, statement, index));
            }
        }
    }
    owner
}

fn contains_expression(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    target: ExpressionHandle,
) -> bool {
    expression_any(program, root, |expression| expression == target)
}

pub(crate) fn expression_has_calls(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
) -> bool {
    expression_any(program, root, |expression| {
        matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Call(_)
        )
    })
}

pub(super) fn expression_any(
    program: &dyn crate::CheckingInput,
    root: ExpressionHandle,
    mut predicate: impl FnMut(ExpressionHandle) -> bool,
) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if predicate(expression) {
            return true;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                pending.extend(program.expression_table.match_pending(dispatch));
            }
            ExpressionNode::Atomic(atomic) => pending.push(atomic.value),
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Call(call) => {
                pending.push(call.receiver);
                pending.extend(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(program.expression_table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Range(range) => pending.extend([range.start, range.end]),
            typed_trees::literal_expression_pattern!() => {}
        }
    }
    false
}
