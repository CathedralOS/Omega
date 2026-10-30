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
        )?;
        let identity = self
            .freeze_scalar_read_backing(state, dependencies)
            .identity?;
        if identity.1 != primitive {
            return None;
        }
        self.stored_values.iter().find_map(|stored| {
            (stored.backing.as_ref()?.identity.as_ref()? == &identity)
                .then(|| stored.current.clone())
                .flatten()
        })
    }
}
