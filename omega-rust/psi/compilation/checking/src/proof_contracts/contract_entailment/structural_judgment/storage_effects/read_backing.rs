//! Join scalar observations through authenticated, frozen backing coordinates.
//!
//! Descriptor homes are not pointee identities. Frame resolution supplies the
//! exact source at the read cut; the scalar-coordinate owner translates recasts
//! and verifies value preservation. Freeze its selectors before retaining it.
//! Only one exact source may identify an alias, and only a still-live cell may
//! donate a value. An expired entry snapshot never becomes current again.

use super::{CapturedStorageCoordinate, StructuralJudge};
use crate::machine_calls::calls::CapturedStorageReadBacking;
use typed_trees::types::PrimitiveType;
use typed_trees::{expression::ExpressionHandle, machine::Machine, state::State};

#[derive(Clone, PartialEq)]
pub(in crate::proof_contracts::contract_entailment::structural_judgment) struct CapturedScalarReadBacking
{
    dependencies: CapturedStorageReadBacking,
    identity: Option<(CapturedStorageCoordinate, PrimitiveType)>,
}

impl CapturedScalarReadBacking {
    pub(super) fn survives(&self, written: &[String]) -> bool {
        self.dependencies.survives(written)
    }
}

impl StructuralJudge<'_> {
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
            });
            self.retain_storage_value(coordinate.clone(), value, false);
            if let Some(stored) = std::sync::Arc::make_mut(&mut self.stored_values)
                .iter_mut()
                .find(|stored| stored.coordinate == coordinate)
            {
                stored.backing = Some(CapturedScalarReadBacking {
                    dependencies: backing,
                    identity: None,
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
        self.retain_storage_value(storage.coordinate.clone(), storage.value, false);
        if let Some(backing) = backing
            && let Some(stored) = std::sync::Arc::make_mut(&mut self.stored_values)
                .iter_mut()
                .find(|stored| stored.coordinate == storage.coordinate)
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
        self.stored_values.iter().find_map(|stored| {
            if stored.coordinate != coordinate || stored.backing.is_none() {
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
        let identity = (|| {
            let [source] = dependencies.sources.as_slice() else {
                return None;
            };
            let scalar = source.scalar_storage_coordinate(self.program, state)?;
            if !scalar.preserves_scalar_value || scalar.source_primitive != scalar.view_primitive {
                return None;
            }
            Some((
                self.capture_storage_place(scalar.place)?,
                scalar.view_primitive,
            ))
        })();
        CapturedScalarReadBacking {
            dependencies,
            identity,
        }
    }

    pub(super) fn current_backing_value(
        &self,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<super::super::super::StructuralTerm> {
        // Whole references/aggregates retain their ordinary conservative fence.
        let declared =
            crate::expression_result_type_reference(self.program, machine, state, expression)?;
        // Ordinary scalar operands may read an explicit readable scalar borrow.
        // This selects meaning only: the exact captured-load chain below still
        // supplies its independent content observation and authority.
        let (primitive, _) = crate::value_custody::expression_types::scalar_observation_meaning(
            self.program,
            declared,
        )?;
        let place = crate::canonical_place_from_expression(self.program, expression)?;
        let dependencies = self.frame_resolver()?.storage_read_backing_at_expression(
            machine,
            state,
            expression,
            place.root.as_symbol()?,
            &place.segments,
        );
        if super::super::super::struct_trace() {
            eprintln!(
                "STRUCT scalar-backing read={} dependencies={dependencies:?}",
                self.program.expression_table.display_name(expression)
            );
        }
        let dependencies = dependencies?;
        let identity = self
            .freeze_scalar_read_backing(state, dependencies)
            .identity?;
        if identity.1 != primitive {
            return None;
        }
        if super::super::super::struct_trace() {
            for stored in self.stored_values.iter() {
                eprintln!(
                    "STRUCT scalar-backing donor read={} current={:?} identity={} same={}",
                    self.program.expression_table.display_name(expression),
                    stored.current,
                    stored
                        .backing
                        .as_ref()
                        .is_some_and(|b| b.identity.is_some()),
                    stored
                        .backing
                        .as_ref()
                        .is_some_and(|b| b.identity.as_ref() == Some(&identity))
                );
            }
        }
        self.stored_values.iter().find_map(|stored| {
            (stored.backing.as_ref()?.identity.as_ref()? == &identity)
                .then(|| stored.current.clone())
                .flatten()
        })
    }
}
