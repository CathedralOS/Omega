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
        for observation in
            crate::scoped_call_result_observations(self.program, machine, state, expression)
        {
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

    /// Only an already installed, still-current completed-result cell can
    /// cross the descriptor fence. Re-querying the old call cannot revive it.
    pub(in crate::proof_contracts::contract_entailment) fn completed_result_read(
        &self,
        captured: &super::super::StructuralTerm,
        path: &[facts::PlaceSegment],
    ) -> Option<super::super::StructuralTerm> {
        let captured = captured.semantic_value();
        let super::super::StructuralTerm::CapturedValue { expression, .. } = captured else {
            return None;
        };
        let coordinate = self.capture_storage_place(crate::CanonicalPlace {
            root: facts::PlaceRoot::Expression(*expression),
            segments: path.to_vec(),
        })?;
        self.stored_values
            .values_at(&coordinate)
            .find_map(|stored| {
                if stored.backing.is_none() {
                    return None;
                }
                let current = stored.current.as_ref()?;
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
                (root == captured).then(|| current.clone())
            })
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
            let supplied = frames.completed_value_read_backing(machine, state, expression, &[]);
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
                    if self.storage_graph().is_none() {
                        return None;
                    }
                    let at = at?;
                    self.stored_values.values_at(&coordinate).find_map(|stored| {
                        (stored.current.is_none()
                            && self.current_value_valid(stored, &coordinate, at))
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
