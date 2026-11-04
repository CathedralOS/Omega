//! Join scalar observations through authenticated, frozen backing coordinates.
//!
//! Descriptor homes are not pointee identities. Frame resolution supplies the
//! exact source at the read cut; the scalar-coordinate owner translates recasts
//! and verifies value preservation. Freeze its selectors before retaining it.
//! Only one exact source may identify an alias, and only a still-live cell may
//! donate a value. An expired entry snapshot never becomes current again.

use super::{CapturedStorageCoordinate, StructuralJudge};
use crate::machine_calls::calls::CapturedStorageReadBacking;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
use typed_trees::{expression::ExpressionHandle, machine::Machine, state::State};

#[derive(Clone, PartialEq)]
pub(in crate::proof_contracts::contract_entailment::structural_judgment) struct CapturedScalarReadBacking
{
    dependencies: CapturedStorageReadBacking,
    identity: Option<(CapturedStorageCoordinate, PrimitiveType)>,
    representation: Option<(CapturedStorageCoordinate, PrimitiveType, PrimitiveType)>,
}

impl StructuralJudge<'_> {
    /// A readable reference place can observe scalar contents without making
    /// its descriptor home a scalar slot. This selects the read's address cut;
    /// backing authentication and access admission remain separate.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn scalar_reference_read_type(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        self.ordinary_storage_path(Some((machine, state)), expression)
            .then_some(())?;
        let reference =
            crate::expression_result_type_reference(self.program, machine, state, expression)?;
        crate::reference_shell(self.program, reference)?;
        crate::value_custody::expression_types::scalar_observation_meaning(
            self.program,
            reference,
        )?;
        Some(reference)
    }

    /// A completed call's public coordinate observes current contents through
    /// its independently retained result suppliers. It is a logical post-state
    /// cell, not an invented executable scalar-read occurrence.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn stage_completed_result_reads(
        &mut self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        captured: &super::super::StructuralTerm,
        pending: Vec<super::super::super::scoped_arithmetic::PendingScopedCallResultObservation>,
    ) -> Vec<super::super::normal_returns::PostCallStorage> {
        if !self.requires_value_preservation() {
            return Vec::new();
        }
        let Some(reference) =
            crate::machine_calls::calls::call_result_type(self.program, expression)
        else {
            return Vec::new();
        };
        let mut staged = Vec::new();
        for candidate in pending {
            if !candidate.requires_backing
                || !self.completed_result_coordinate_is_formed(reference, captured, &candidate)
            {
                continue;
            }
            let Some(observation) = (|| {
                super::super::super::scoped_arithmetic::resolve_scoped_call_result_observation(
                    self.program,
                    machine,
                    state,
                    expression,
                    candidate,
                    Some(self.frame_resolver()?),
                )
            })() else {
                continue;
            };
            let Some(backing) = observation.backing else {
                continue;
            };
            let Some(coordinate) = self.capture_storage_place(crate::CanonicalPlace {
                root: facts::PlaceRoot::Expression(expression),
                segments: observation.path.clone(),
            }) else {
                continue;
            };
            let Some(geometry) = crate::structural_scalar_coordinate_in_type(
                self.program,
                facts::PlaceRoot::Expression(expression),
                reference,
                &observation.path,
            ) else {
                continue;
            };
            let mut value = Some(captured.clone());
            let mut current_type = reference;
            for segment in &observation.path {
                let Some(projected) = crate::project_type_reference_from_segments(
                    self.program,
                    current_type,
                    std::slice::from_ref(segment),
                ) else {
                    value = None;
                    break;
                };
                value = value.and_then(|value| match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        super::super::super::observed_projections::project_field(
                            value,
                            self.program.symbols.name(*symbol),
                        )
                    }
                    facts::PlaceSegment::Case { .. } => Some(value),
                    facts::PlaceSegment::FixedIndex { index } => {
                        Some(super::super::StructuralTerm::IndexedValue {
                            element_type: projected,
                            element_identity: self.program.normalized_type_identity(projected),
                            collection: Box::new(value),
                            index: Box::new(super::super::StructuralTerm::Integer(
                                numerics::bignum::BigInt::from_u64(u64::try_from(*index).ok()?),
                            )),
                        })
                    }
                    _ => None,
                });
                current_type = projected;
            }
            let Some(value) = value else {
                continue;
            };
            let value = super::super::super::structural_terms::owned_projection_observation(
                self.program,
                geometry.result_type_reference,
                value,
            );
            // This temporary cell only binds the checked public guarantee.
            // It supplies no pre-write alias identity: the owning completion
            // freezes the raw backing after applying this call's own effects.
            staged.push(super::super::normal_returns::PostCallStorage {
                coordinate: coordinate.clone(),
                value: value.clone(),
                backing: Some(backing.clone()),
                representation: None,
            });
            self.retain_storage_value(coordinate.clone(), value, false);
            if let Some(stored) =
                std::sync::Arc::make_mut(&mut self.stored_values).value_at_mut(&coordinate)
            {
                stored.backing = Some(CapturedScalarReadBacking {
                    dependencies: backing,
                    identity: None,
                    representation: None,
                });
            }
        }
        staged
    }

    /// A coordinate becomes available only after the completed public promises
    /// establish its case and extent. Project descriptor words for the extent;
    /// the bytes themselves still require the separate backing authentication.
    fn completed_result_coordinate_is_formed(
        &self,
        reference: TypeReferenceHandle,
        captured: &super::super::StructuralTerm,
        pending: &super::super::super::scoped_arithmetic::PendingScopedCallResultObservation,
    ) -> bool {
        let formed = self.scalar_coordinate_is_formed(
            reference,
            captured,
            &pending.observation.path,
            &pending.cases,
            &pending.indices,
        );
        formed
    }

    pub(super) fn scalar_coordinate_is_formed(
        &self,
        reference: TypeReferenceHandle,
        captured: &super::super::StructuralTerm,
        path: &[facts::PlaceSegment],
        cases: &[super::super::super::scoped_arithmetic::ScalarProjectionCaseRequirement],
        indices: &[super::super::super::scoped_arithmetic::ScalarProjectionIndexRequirement],
    ) -> bool {
        (|| {
            for required in cases {
                let prefix = path.get(..=required.prefix_length)?;
                if !matches!(prefix.last(), Some(facts::PlaceSegment::Case { variant }) if *variant == required.variant)
                {
                    return None;
                }
                super::super::super::observed_projections::project_observed_value_in_scope(
                    self.program,
                    self,
                    reference,
                    captured.clone(),
                    prefix,
                )?;
            }
            for required in indices {
                let prefix = path.get(..required.prefix_length)?;
                if !matches!(path.get(required.prefix_length), Some(facts::PlaceSegment::FixedIndex { index }) if *index == required.index)
                {
                    return None;
                }
                let collection_type =
                    crate::project_type_reference_from_segments(self.program, reference, prefix)?;
                let descriptor =
                    super::super::super::observed_projections::project_observed_value_in_scope(
                        self.program,
                        self,
                        reference,
                        captured.clone(),
                        prefix,
                    )?;
                let length = super::super::super::structural_terms::observed_collection_measure(
                    self.program,
                    collection_type,
                    language_semantics::declaration_selection::CollectionMeasure::Length,
                    || Some(descriptor),
                )?;
                let index = u64::try_from(required.index).ok()?;
                let question = super::super::StructuralTerm::MathematicalBinary {
                    operator: typed_trees::expression::BinaryOperator::Less,
                    left: Box::new(super::super::StructuralTerm::Integer(
                        numerics::bignum::BigInt::from_u64(index),
                    )),
                    right: Box::new(length),
                };
                if !matches!(
                    self.judge_observed_predicate(question, 0),
                    super::super::StructuralJudgment::Proven
                ) {
                    return None;
                }
            }
            Some(())
        })()
        .is_some()
    }

    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn install_post_call_storage(
        &mut self,
        state: &State,
        storage: super::super::normal_returns::PostCallStorage,
    ) {
        let backing = storage
            .backing
            .map(|backing| self.freeze_scalar_read_backing(state, backing));
        let value = match storage.representation {
            Some((source, target)) => {
                super::super::super::structural_terms::normalize_scalar_representation(
                    source,
                    target,
                    storage.value,
                )
            }
            None => storage.value,
        };
        if let Some(reference) = storage.coordinate.root.as_symbol().and_then(|root| {
            storage.coordinate.segments.iter().try_fold(
                crate::value_custody::places::assignment_value_type(
                    self.program,
                    crate::declaration_type_reference(self.program, root)?,
                ),
                |reference, segment| segment.projected_type(self.program, reference),
            )
        }) && let super::super::StructuralTerm::CapturedValue { result_type, .. } =
            value.semantic_value()
            && crate::declared_carriers_match(self.program, reference, *result_type)
        {
            // A differently represented recast cannot inherit the source's
            // field namespace merely because both views have the same size.
            self.intake_storage_field_observations(
                &storage.coordinate,
                reference,
                &value,
                self.storage_version,
            );
        }
        self.retain_storage_value(storage.coordinate.clone(), value, false);
        if let Some(backing) = backing
            && let Some(stored) =
                std::sync::Arc::make_mut(&mut self.stored_values).value_at_mut(&storage.coordinate)
        {
            stored.backing = Some(backing);
        }
    }

    /// Read through one exact completed value. A retained result cell keeps
    /// its checked public equation; a saved input instead needs its original
    /// descriptor and independently available current backing values.
    pub(in crate::proof_contracts::contract_entailment) fn captured_scalar_read(
        &self,
        captured: &super::super::StructuralTerm,
        path: &[facts::PlaceSegment],
    ) -> Option<super::super::StructuralTerm> {
        let (captured, mut prefix) = match self.completed_result_read_source(captured, 0) {
            Ok(source) => source,
            Err(_) => {
                return None;
            }
        };
        let super::super::StructuralTerm::CapturedValue {
            expression,
            result_type,
            ..
        } = &captured
        else {
            return None;
        };
        prefix.extend_from_slice(path);
        let Some(geometry) = crate::structural_scalar_coordinate_in_type(
            self.program,
            facts::PlaceRoot::Expression(*expression),
            *result_type,
            &prefix,
        ) else {
            return None;
        };
        let Some(coordinate) = self.capture_storage_place(crate::CanonicalPlace {
            root: facts::PlaceRoot::Expression(*expression),
            segments: prefix.clone(),
        }) else {
            return None;
        };
        let value = self
            .stored_values
            .values_at(&coordinate)
            .find_map(|stored| {
                if stored.backing.is_none() {
                    return None;
                }
                if !self.storage_value_transfer_is_current(stored, self.storage_version) {
                    return None;
                }
                let Some(current) = stored.current.as_ref() else {
                    return None;
                };
                let mut root = current.semantic_value();
                loop {
                    root = match root {
                        super::super::StructuralTerm::Projection { subject, .. } => {
                            subject.semantic_value()
                        }
                        super::super::StructuralTerm::IndexedValue { collection, .. } => {
                            collection.semantic_value()
                        }
                        _ => break,
                    };
                }
                root.same_resolution_payload(&captured)
                    .then(|| current.clone())
            });
        let Some(value) = value else {
            return self.captured_operand_scalar_read(
                &captured,
                &prefix,
                geometry.primitive_type,
                geometry.result_type_reference,
            );
        };
        // Keep the checked equation for this current scalar before resolving
        // its enclosing value. A case skeleton can replace that value without
        // retaining the original indexed cell's identity. Only the cell checks
        // above authorize this read; a descriptor or a whole-data Id cannot.
        if self.requires_value_preservation()
            && let Some(resolved) = self.captured_result_equations.iter().find_map(|equation| {
                let replacement = equation.public_scalar_replacement(&value)?;
                if super::super::super::structural_terms::term_contains(replacement, &value) {
                    return None;
                }
                let resolved = self.resolve(replacement.clone());
                super::super::super::structural_terms::scalar_operand_is_observed(
                    self.program,
                    geometry.result_type_reference,
                    &resolved,
                )
                .then_some(resolved)
            })
        {
            return Some(resolved);
        }
        Some(value)
    }

    /// A saved input retains its descriptor, not a copy of borrowed contents.
    /// Rejoin its actual completed expression and ask the ordinary backing
    /// owner for current scalar values. Missing or disagreeing suppliers add
    /// no observation, and the requested contract supplies no value itself.
    fn captured_operand_scalar_read(
        &self,
        captured: &super::super::StructuralTerm,
        path: &[facts::PlaceSegment],
        primitive: PrimitiveType,
        observed_type: TypeReferenceHandle,
    ) -> Option<super::super::StructuralTerm> {
        if !self.requires_value_preservation() {
            return None;
        }
        let super::super::StructuralTerm::CapturedValue {
            expression,
            result_type,
            ..
        } = captured
        else {
            return None;
        };
        let result = (|| {
            if !self.completed_nominal_values.iter().any(|receipt| {
                receipt.anchor == *expression
                    && receipt.reference == *result_type
                    && receipt.value.same_resolution_payload(captured)
            }) {
                return Err("completed-value");
            }
            let (machine, state) = self.runtime_declaration_scope().ok_or("caller-scope")?;
            if self.storage_graph().is_some()
                && !self
                    .storage_version
                    .is_some_and(|version| version.state == state.symbol)
            {
                return Err("current-source-cut");
            }
            if !self
                .program
                .expression_table
                .expression_is_valid(*expression)
                || crate::expression_result_type_reference(
                    self.program,
                    machine,
                    state,
                    *expression,
                ) != Some(*result_type)
            {
                return Err("source-type");
            }
            let (cases, indices) =
                super::super::super::scoped_arithmetic::scalar_projection_requirements(
                    self.program,
                    *result_type,
                    path,
                )
                .ok_or("projection-requirements")?;
            if !self.scalar_coordinate_is_formed(*result_type, captured, path, &cases, &indices) {
                return Err("case-or-extent");
            }
            let backing = self
                .frame_resolver()
                .ok_or("read-owner")?
                .completed_value_read_backing(machine, state, *expression, *result_type, path)
                .ok_or("completed-backing")?;
            // The descriptor was captured during operand evaluation. Any
            // later pointer slot still has to designate that same supplier;
            // a different current pointee is not the saved read's backing.
            if !backing.bindings.iter().all(|binding| {
                binding.root.is_valid()
                    && binding.invocation_output.is_none()
                    && binding.invocation_result.is_none()
                    && !binding.is_image_storage
                    && binding.recasts.is_empty()
                    && binding.representation.is_none()
                    && !binding.collection_coarse
                    && !self
                        .program
                        .state_parameter_by_symbol(state, binding.root)
                        .is_some_and(|parameter| {
                            crate::reference_shell(self.program, parameter.type_reference).is_some()
                        })
                    && crate::value_custody::places::binding_observations::stable_reference_descriptor_place(
                        self.program,
                        state,
                        binding.root,
                        &binding.segments,
                    )
                    .is_some()
            }) {
                return Err("descriptor-bindings");
            }
            let values = self
                .backing_values_at(state, *expression, primitive, backing, self.storage_version)
                .ok_or("current-scalar-values")?
                .into_iter()
                .map(|value| self.resolve(value))
                .collect::<Vec<_>>();
            let first = values.first().ok_or("empty-suppliers")?;
            if !values.iter().all(|value| {
                first.same_resolution_payload(value)
                    && super::super::super::structural_terms::scalar_operand_is_observed(
                        self.program,
                        observed_type,
                        value,
                    )
            }) {
                return Err("supplier-values");
            }
            Ok(first.clone())
        })();
        result.ok()
    }

    /// A payload passed to another call still names its original result cell.
    /// Follow its already formed owned fields to the saved call identity. The
    /// existing cell's current storage checks decide whether its contents
    /// remain available to this caller.
    pub(super) fn completed_result_read_source(
        &self,
        value: &super::super::StructuralTerm,
        depth: usize,
    ) -> Result<(super::super::StructuralTerm, Vec<facts::PlaceSegment>), &'static str> {
        if depth >= 64 {
            return Err("prefix-depth");
        }
        match value.semantic_value() {
            captured @ super::super::StructuralTerm::CapturedValue { .. } => {
                Ok((captured.clone(), Vec::new()))
            }
            super::super::StructuralTerm::Projection { subject, path } => {
                let (captured, mut prefix) =
                    self.completed_result_read_source(subject, depth + 1)?;
                let super::super::StructuralTerm::CapturedValue { result_type, .. } = &captured
                else {
                    return Err("prefix-capture");
                };
                let reference = crate::project_type_reference_from_segments(
                    self.program,
                    *result_type,
                    &prefix,
                )
                .ok_or("prefix-type")?;
                let observed =
                    super::super::super::observed_projections::project_observed_value_in_scope(
                        self.program,
                        self,
                        *result_type,
                        super::super::super::structural_terms::owned_projection_observation(
                            self.program,
                            *result_type,
                            captured.clone(),
                        ),
                        &prefix,
                    )
                    .ok_or("prefix-observation")?;
                let names = path.split('.').collect::<Vec<_>>();
                let paths = super::super::super::completed_values::owned_term_field_paths_in_scope(
                    self.program,
                    self,
                    reference,
                    &observed,
                    &names,
                );
                let [selected] = paths.as_slice() else {
                    return Err("prefix-case-or-field");
                };
                prefix.extend_from_slice(selected);
                Ok((captured, prefix))
            }
            _ => Err("source-value"),
        }
    }

    pub(super) fn freeze_scalar_read_backing(
        &self,
        state: &State,
        dependencies: CapturedStorageReadBacking,
    ) -> CapturedScalarReadBacking {
        self.freeze_scalar_read_backing_with(state, dependencies, |place| {
            self.capture_storage_place(place)
        })
    }

    pub(super) fn freeze_entry_read_backing(
        &self,
        state: &State,
        dependencies: CapturedStorageReadBacking,
    ) -> CapturedScalarReadBacking {
        self.freeze_scalar_read_backing_with(state, dependencies, |place| {
            self.capture_entry_storage_place(place)
        })
    }

    fn freeze_scalar_read_backing_with(
        &self,
        state: &State,
        dependencies: CapturedStorageReadBacking,
        capture: impl Fn(crate::CanonicalPlace) -> Option<CapturedStorageCoordinate>,
    ) -> CapturedScalarReadBacking {
        let scalar = (|| {
            let [source] = dependencies.sources.as_slice() else {
                return None;
            };
            let scalar = source.scalar_storage_coordinate(self.program, state)?;
            Some((capture(scalar.place.clone())?, scalar))
        })();
        let identity = scalar.as_ref().and_then(|(coordinate, scalar)| {
            (scalar.preserves_scalar_value && scalar.source_primitive == scalar.view_primitive)
                .then(|| (coordinate.clone(), scalar.view_primitive))
        });
        let representation = scalar.and_then(|(coordinate, scalar)| {
            (identity.is_none() && scalar.preserves_primitive_bits).then_some((
                coordinate,
                scalar.source_primitive,
                scalar.view_primitive,
            ))
        });
        CapturedScalarReadBacking {
            dependencies,
            identity,
            representation,
        }
    }

    pub(super) fn current_backing_value(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        at: Option<super::StorageVersion>,
    ) -> Option<super::super::super::StructuralTerm> {
        let values = self.current_backing_values(machine, state, expression, at)?;
        let first = values.first()?;
        // A single-value consumer needs agreement, not a convenient supplier.
        values
            .iter()
            .all(|value| value == first)
            .then(|| first.clone())
    }

    /// Prove returned contents from the storage after the function's final
    /// writes. A descriptor saved earlier identifies the buffer, not its final
    /// contents; the return promise cannot supply those contents itself.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_return_read(
        &self,
        source: &super::super::normal_returns::CompletedValueSource<'_>,
        path: &[facts::PlaceSegment],
    ) -> Option<super::super::super::StructuralTerm> {
        if !self.requires_value_preservation()
            || !std::ptr::eq(self.program, source.context.program)
            || source.storage_version != source.context.storage_version
        {
            return None;
        }
        let machine = self.program.machine_by_symbol(source.machine)?;
        let state = self.program.state_in_machine(machine, source.state)?;
        let Some(geometry) = crate::structural_scalar_coordinate_in_type(
            self.program,
            facts::PlaceRoot::Expression(source.expression),
            source.type_reference,
            path,
        ) else {
            return None;
        };
        let dependencies = source
            .context
            .frame_resolver()?
            .completed_value_read_backing(
                machine,
                state,
                source.expression,
                source.type_reference,
                path,
            );
        let Some(dependencies) = dependencies else {
            return None;
        };
        let values = source
            .context
            .backing_values_at(
                state,
                source.expression,
                geometry.primitive_type,
                dependencies,
                source.storage_version,
            )?
            .into_iter()
            // A stored scalar can still name a local value or field. Resolve
            // it before leaving its producing environment; the caller cannot
            // reinterpret those names while comparing the returned contents.
            .map(|value| source.context.resolve(value))
            .collect::<Vec<_>>();
        let first = values.first()?;
        values
            .iter()
            .all(|value| value == first)
            .then(|| first.clone())
    }

    /// Observe every possible scalar cell at the performed read's cut. This
    /// frontier supplies proof alternatives, never one physical alias identity.
    /// Missing or expired suppliers leave the whole observation unavailable.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn current_backing_values(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        at: Option<super::StorageVersion>,
    ) -> Option<Vec<super::super::super::StructuralTerm>> {
        self.ordinary_storage_path(Some((machine, state)), expression)
            .then_some(())?;
        let declared =
            crate::expression_result_type_reference(self.program, machine, state, expression)?;
        let (primitive, _) = crate::value_custody::expression_types::scalar_observation_meaning(
            self.program,
            declared,
        )?;
        let frames = self.frame_resolver()?;
        let dependencies = if crate::reference_shell(self.program, declared).is_some() {
            let supplied =
                frames.completed_value_read_backing(machine, state, expression, declared, &[]);
            if supplied.is_none() && super::super::super::struct_trace() {
                eprintln!(
                    "STRUCT scalar-read-refused expression={expression:?} phase=performed-descriptor-load-frontier"
                );
            }
            supplied?
        } else {
            let place = crate::canonical_place_from_expression(self.program, expression)?;
            frames.storage_read_backing_at_expression(
                machine,
                state,
                expression,
                place.root.as_symbol()?,
                &place.segments,
            )?
        };
        self.backing_values_at(state, expression, primitive, dependencies, at)
    }

    fn backing_values_at(
        &self,
        state: &State,
        expression: ExpressionHandle,
        primitive: PrimitiveType,
        dependencies: CapturedStorageReadBacking,
        at: Option<super::StorageVersion>,
    ) -> Option<Vec<super::super::super::StructuralTerm>> {
        if dependencies.sources.is_empty() {
            return None;
        }
        dependencies
            .sources
            .iter()
            .map(|source| {
                // Coarse mutation labels do not erase a fully resolved supplier
                // coordinate. Dynamic/unknown coordinates still fail the ordinary
                // scalar geometry owner; all source interpretations are retained.
                let scalar = match source.scalar_storage_coordinate_result(self.program, state) {
                    Ok(scalar) => scalar,
                    Err(reason) => {
                        if super::super::super::struct_trace() {
                            eprintln!("STRUCT scalar-read-refused expression={expression:?} source={source:?} phase=scalar-geometry reason={reason}");
                        }
                        return None;
                    }
                };
                if scalar.view_primitive != primitive
                    || (!scalar.preserves_scalar_value && !scalar.preserves_primitive_bits)
                {
                    if super::super::super::struct_trace() {
                        eprintln!("STRUCT scalar-read-refused expression={expression:?} source={source:?} phase=primitive-interpretation source_primitive={:?} view_primitive={:?}", scalar.source_primitive, scalar.view_primitive);
                    }
                    return None;
                }
                let coordinate = self.capture_storage_place(scalar.place)?;
                let identity = (coordinate.clone(), scalar.source_primitive);
                let value = self.current_storage_value_at(&coordinate, at).or_else(|| {
                    self.stored_values.iter().find_map(|stored| {
                        if stored.backing.as_ref()?.identity.as_ref()? != &identity
                            || (self.graph_storage_mode()
                                && !at.is_some_and(|at| {
                                    self.current_value_valid(stored, &coordinate, at)
                                }))
                        {
                            return None;
                        }
                        stored.current.clone()
                    })
                }).or_else(|| {
                    // Entry predicates can name a field before its pointer is
                    // loaded. They supply contents only to this performed-read
                    // owner, after the whole load frontier and current cut are
                    // independently authenticated. A retired Entry snapshot is
                    // never a substitute for changed current storage.
                    self.stored_values.values_at(&coordinate).find_map(|stored| {
                        (stored.current.is_none()
                            && if self.storage_graph().is_some() {
                                at.is_some_and(|at| {
                                    self.current_value_valid(stored, &coordinate, at)
                                })
                            } else {
                                self.storage_write_sequence == 0
                                    && stored.anchor_state == state.symbol
                            })
                        .then(|| stored.entry.clone())
                        .flatten()
                    })
                });
                let Some(value) = value else {
                    if super::super::super::struct_trace() {
                        eprintln!("STRUCT scalar-read-refused expression={expression:?} source={source:?} coordinate={coordinate:?} at={at:?} phase=current-or-preserved-entry-value");
                    }
                    return None;
                };
                Some(if scalar.preserves_scalar_value {
                    value
                } else {
                    super::super::super::structural_terms::normalize_scalar_representation(
                        scalar.source_primitive,
                        scalar.view_primitive,
                        value,
                    )
                })
            })
            .collect()
    }
}
