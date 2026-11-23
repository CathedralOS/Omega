//! Read each type where its arguments were supplied before comparing them.
//! When a named type still contains const parameters, the checker must prove
//! that their values agree.

use super::{Program, contracts::ClauseScope};
use crate::lowering::body::{Qualification, Regions, ValueType};
use crate::lowering::canonical;
use crate::source_state::SourceTypeApplication;
use crate::source_state::data::TypeParameterKind;
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use semantic_vocabulary::{MachineId, Proposition, ScalarTerm, ScalarType};
use std::collections::BTreeSet;
use terminal_check_tables::Checked;

impl Program<'_> {
    fn equation_operand(&self, source: &SourceTypeApplication) -> Option<SourceTypeApplication> {
        let supplied = source
            .scope
            .supplied_type_reference(self.typed, source.reference)
            .ok()?;
        let TypeReferenceNode::Named { symbol, .. } = self
            .typed
            .type_reference_table
            .type_reference(supplied.reference)
        else {
            return Some(supplied);
        };
        let actual = self.scope.iter().find_map(|(binder, value)| {
            if binder != symbol {
                return None;
            }
            let ValueType::Record(record, Qualification::NONE, Regions::NONE) = value else {
                return None;
            };
            self.record_sources.get(*record)?.application.clone()
        });
        Some(actual.unwrap_or(supplied))
    }

    fn equation_const_argument(
        &self,
        machine: MachineId,
        clause: &ClauseScope<'_>,
        source: SourceTypeApplication,
        scalar: ScalarType,
    ) -> Checked<ScalarTerm> {
        let source = source
            .scope
            .supplied_type_reference(self.typed, source.reference)
            .map_err(|detail| super::services::unavailable(machine, detail))?;
        if let TypeReferenceNode::Named { symbol, .. } = self
            .typed
            .type_reference_table
            .type_reference(source.reference)
            && let Some((_, value)) = clause
                .bound
                .iter()
                .find(|(binder, value)| binder == symbol && value.scalar_type() == scalar)
        {
            return Ok(value.clone());
        }
        self.bound_scalar_value(machine, scalar, source.reference, true)
    }

    pub(super) fn structural_type_equation(
        &mut self,
        machine: MachineId,
        clause: &ClauseScope<'_>,
        left: TypeReferenceHandle,
        right: TypeReferenceHandle,
    ) -> Checked<Option<Proposition>> {
        let operand = |reference| SourceTypeApplication {
            reference,
            scope: self.source_scope.clone(),
            substitutions: Vec::new(),
        };
        let mut pending = vec![(operand(left), operand(right))];
        let mut visited = BTreeSet::new();
        let mut obligations = Vec::new();
        while let Some((left, right)) = pending.pop() {
            let (Some(left), Some(right)) =
                (self.equation_operand(&left), self.equation_operand(&right))
            else {
                return Ok(None);
            };
            let typed = self.typed;
            let identity = |source: &SourceTypeApplication| {
                if typed
                    .data_application_definition(source.reference)
                    .is_some_and(|data| !data.lifetime_parameters.is_empty())
                {
                    return None;
                }
                match typed.type_reference_table.type_reference(source.reference) {
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                        Some(source.type_identity(typed))
                    }
                    // The type identity includes the elements, extents and
                    // qualifications. Borrowed types also need their lifetimes.
                    _ if source.scope.admits_argument(typed, source.reference) => source
                        .scope
                        .region_free_type_identity(typed, source.reference),
                    _ => None,
                }
            };
            let (Some(left_id), Some(right_id)) = (identity(&left), identity(&right)) else {
                return Ok(None);
            };
            // Normalization does not establish formation. Read both types
            // under their actual supplying frames before using their identity.
            for source in [&left, &right] {
                let previous = std::mem::replace(&mut self.source_scope, source.scope.clone());
                let formed = self.value_type(machine, source.reference);
                self.source_scope = previous;
                formed?;
            }
            if !left_id.is_valid() || !right_id.is_valid() {
                return Ok(None);
            }
            if left_id == right_id || !visited.insert((left_id, right_id)) {
                continue;
            }
            let table = &self.typed.type_reference_table;
            let (
                TypeReferenceNode::Generic {
                    base_symbol: left_base,
                    ..
                },
                TypeReferenceNode::Generic {
                    base_symbol: right_base,
                    ..
                },
            ) = (
                table.type_reference(left.reference),
                table.type_reference(right.reference),
            )
            else {
                // Distinct normalized types are unequal once both sides are
                // closed. Open binders could still receive the same type.
                let closed = |source: &SourceTypeApplication| {
                    crate::source_state::domain::index_argument_is_closed(typed, source.reference)
                };
                return Ok((closed(&left) && closed(&right)).then_some(Proposition::Falsehood));
            };
            let (Some(left_data), Some(right_data)) = (
                self.typed.data_application_definition(left.reference),
                self.typed.data_application_definition(right.reference),
            ) else {
                return Ok(None);
            };
            let application = |source: SourceTypeApplication, data| {
                super::source_records::source_application_in_scope(
                    self.typed,
                    data,
                    source.reference,
                    machine,
                    source.scope,
                )
            };
            let left = application(left, left_data)?;
            let right = application(right, right_data)?;
            if left_base != right_base {
                return Ok(Some(Proposition::Falsehood));
            }
            if !left_data.lifetime_parameters.is_empty() {
                return Ok(None);
            }
            let Some(template) = self.typed.unique_data_definition_by_symbol(*left_base) else {
                return Ok(None);
            };
            for (parameter, ((_, left_arg), (_, right_arg))) in self
                .typed
                .data_type_parameters(template)
                .iter()
                .zip(left.substitutions.iter().zip(&right.substitutions))
            {
                let argument = |reference, source: &SourceTypeApplication| SourceTypeApplication {
                    reference,
                    scope: source.scope.clone(),
                    substitutions: Vec::new(),
                };
                let left = argument(*left_arg, &left);
                let right = argument(*right_arg, &right);
                match parameter.kind {
                    TypeParameterKind::Type => pending.push((left, right)),
                    TypeParameterKind::Const { type_reference }
                    | TypeParameterKind::Value { type_reference } => {
                        let scalar = self.scalar_type(machine, type_reference)?;
                        let left = self.equation_const_argument(machine, clause, left, scalar)?;
                        let right = self.equation_const_argument(machine, clause, right, scalar)?;
                        obligations.push(canonical::equal(left, right, machine)?);
                    }
                    _ => return Ok(None),
                }
            }
        }
        canonical::conjunction(obligations, machine).map(Some)
    }
}
