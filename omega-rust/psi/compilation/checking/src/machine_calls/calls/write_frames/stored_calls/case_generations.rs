//! Completed calls retire case epochs separately from pointer contents.
//! A closed declared frontier names possible cases after a valid normal return,
//! not a selected tag and not the old constructor's value. Actual writable
//! argument captures authenticate storage roots; may-write frames only decide
//! which of their case containers lose old precision. Missing frames widen,
//! never prove non-interference. Unknown storage/shape supplies no coverage.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::machine_calls::calls::write_frames) struct CaseStorageEffect {
    pub root: SymbolHandle,
    pub container: Vec<PlaceSegment>,
    /// Complete declared possible case paths relative to this container.
    /// An empty/unknown frontier is never issued for a case container.
    pub possible: Vec<Vec<PlaceSegment>>,
}

impl CaseStorageEffect {
    pub(in crate::machine_calls::calls::write_frames) fn covers(
        &self,
        root: SymbolHandle,
        case: &[PlaceSegment],
    ) -> bool {
        self.root == root && stored_origins::path_covers(&self.container, case)
    }

    pub(in crate::machine_calls::calls::write_frames) fn install(
        &self,
        stored: &mut [StoredLocalOrigins],
    ) {
        let Some(record) = stored
            .iter_mut()
            .find(|record| record.local_symbol == self.root)
        else {
            return;
        };
        let root = record.local_symbol;
        record.cases.retain(|case| !self.covers(root, case));
        for possible in &self.possible {
            let mut case = self.container.clone();
            case.extend_from_slice(possible);
            if !record.cases.contains(&case) {
                record.cases.push(case);
            }
        }
        // A source-value move snapshot overlapping replaced contents cannot
        // describe the new value. This retires origin evidence, not ownership
        // claims or the independent affine restoration ledger.
        record.moves.retain(|moved| {
            !stored_origins::path_covers(&self.container, &moved.local_segments)
                && !stored_origins::path_covers(&moved.local_segments, &self.container)
        });
    }
}

/// One invocation operation, independent of statement/expression/result shape.
/// The actual writable input capture supplies every possible source. Lifetime
/// dependencies and opaque provider roots never become storage addresses here.
pub(super) fn completed_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    caller_state: &State,
    target: SymbolHandle,
    captures: &[SlotArgumentCapture],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Vec<CaseStorageEffect> {
    let Some((callee, state)) = super::super::machine_state_by_symbol(program, target) else {
        return Vec::new();
    };
    let parameters = program.state_parameters(state);
    if parameters.len() != captures.len() {
        return Vec::new();
    }
    // Scalar-only calls do not need another frame query. Lazily obtain the
    // invocation's frame after finding an actual writable case container.
    let mut written_paths: Option<Option<Vec<String>>> = None;
    let mut effects = Vec::new();
    for (parameter, capture) in parameters.iter().zip(captures) {
        let Some(carrier) = super::super::reference_subjects::carrier_storage_declaration(
            program,
            parameter.type_reference,
        ) else {
            continue;
        };
        let Some(inventory) = stored_origins::declared_origins_for_query(
            program,
            parameter.symbol,
            parameter.name.as_str(),
            carrier,
            true,
        ) else {
            continue;
        };
        let mut routes = Vec::new();
        if crate::reference_shell(program, parameter.type_reference)
            .is_some_and(|shell| shell.access.is_exclusive())
        {
            routes.push(Vec::new());
        }
        for reference in &inventory.references {
            if matches!(reference.kind, stored_origins::StoredOriginKind::Reference(access) if access.is_exclusive())
                && !routes.contains(&reference.local_segments)
            {
                routes.push(reference.local_segments.clone());
            }
        }
        for route in routes {
            let relative = FrameSourcePlace {
                segments: route.clone(),
                ..FrameSourcePlace::parameter_root(parameter)
            };
            let Some(sources) = aggregate_argument_sources(
                program,
                machine,
                parameter,
                capture,
                &SlotSource::from_place(program, relative, FramePathPrecision::Exact, None),
                context,
                symbols,
            ) else {
                continue;
            };
            for source in sources {
                let base = &source.origin.source;
                if source.lifetime_access.is_some()
                    || !base.root.is_valid()
                    || base.is_static()
                    || !base.builtin_coordinates
                {
                    continue;
                }
                let Some(record) = context
                    .stored
                    .iter()
                    .find(|record| record.local_symbol == base.root)
                else {
                    continue;
                };
                let reference = program
                    .state_parameter_by_symbol(caller_state, base.root)
                    .map(|parameter| parameter.type_reference)
                    .or_else(|| {
                        program
                            .state_local_declaration(caller_state, base.root)
                            .map(|(_, local)| local.type_reference)
                    });
                let Some(reference) = reference.and_then(|reference| {
                    super::super::reference_subjects::carrier_storage_declaration(
                        program, reference,
                    )
                }) else {
                    continue;
                };
                let Some(declared) = stored_origins::declared_origins_for_query(
                    program,
                    base.root,
                    program.symbols.name(base.root),
                    reference,
                    true,
                ) else {
                    continue;
                };
                for case in record.cases.iter().chain(&declared.cases) {
                    let Some((PlaceSegment::Case { .. }, container)) = case.split_last() else {
                        continue;
                    };
                    // A borrow of a payload cannot replace its ancestor tag.
                    if !stored_origins::path_covers(&base.segments, container) {
                        continue;
                    }
                    let mut relative_container = route.clone();
                    relative_container.extend_from_slice(&container[base.segments.len()..]);
                    let (ceiling, _) = super::super::place_paths::coordinate_storage_path(
                        program,
                        parameter.name.as_str(),
                        &relative_container,
                    );
                    // A missing frame means every reachable writable case
                    // may be replaced; it never authenticates an address.
                    let writes = written_paths.get_or_insert_with(|| {
                        if inference.active_states.contains(&state.symbol) {
                            None
                        } else {
                            inference.active_states.push(state.symbol);
                            let writes = super::super::state_write_walk::summarize_complete_state_written_paths(
                                program, callee, state, symbols, inference,
                            );
                            inference.active_states.pop();
                            writes
                        }
                    });
                    let may_replace = writes.as_ref().is_none_or(|writes| {
                        writes.iter().any(|written| {
                            stored_origins::place_suffix(written, &ceiling).is_some()
                        })
                    });
                    if !may_replace {
                        continue;
                    }
                    let Some(projected) =
                        stored_origins::project_stored_origins(program, &declared, container, true)
                    else {
                        continue;
                    };
                    if projected.cases.is_empty() {
                        continue;
                    }
                    let effect = CaseStorageEffect {
                        root: base.root,
                        container: container.to_vec(),
                        possible: projected.cases,
                    };
                    if !effects.contains(&effect) {
                        effects.push(effect);
                    }
                }
            }
        }
    }
    effects
}
