//! Compare nominal applications without replacing their const inputs by one
//! caller's values. The normalized type and argument owners remain exact;
//! unresolved const indices become ordinary equality obligations.

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

    pub(super) fn nominal_type_equation(
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
            // Reference regions and outer qualifications need their own
            // equality account. This reader never strips them to reach a
            // nominal constructor or equates lifetime spellings.
            for source in [&left, &right] {
                if !matches!(
                    self.typed
                        .type_reference_table
                        .type_reference(source.reference),
                    TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
                ) || self
                    .typed
                    .data_application_definition(source.reference)
                    .is_some_and(|data| !data.lifetime_parameters.is_empty())
                {
                    return Ok(None);
                }
            }
            // Normalization does not establish formation. Read both types
            // under their actual supplying frames before using their identity.
            for source in [&left, &right] {
                let previous = std::mem::replace(&mut self.source_scope, source.scope.clone());
                let formed = self.value_type(machine, source.reference);
                self.source_scope = previous;
                formed?;
            }
            let identities = (
                left.type_identity(self.typed),
                right.type_identity(self.typed),
            );
            if !identities.0.is_valid() || !identities.1.is_valid() {
                return Ok(None);
            }
            if identities.0 == identities.1 || !visited.insert(identities) {
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
                // Distinct rigid binders may still be supplied equal types.
                // Only already closed scalar/nominal alternatives can decide
                // this case; do not declare an unknown pair unequal.
                let closed =
                    |source: &SourceTypeApplication| match table.type_reference(source.reference) {
                        TypeReferenceNode::Named { symbol, .. } => {
                            self.typed.symbols.builtin_type_atom(*symbol).is_some()
                                || self
                                    .typed
                                    .closed_data_definition(source.reference)
                                    .is_some()
                        }
                        _ => false,
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
                    TypeParameterKind::Const { type_reference } => {
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
