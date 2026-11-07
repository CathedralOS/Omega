//! An established Id permits substitution of its exact values and constructor
//! fields. This operation consumes formed propositions; it does not establish
//! a call's preconditions, termination, or permission to observe its arguments.
//! Reference fields retain descriptor identity and never become content reads.

use super::{ConstructorIdentity, StructuralJudge, StructuralTerm, term_contains};
use std::sync::Arc;

impl StructuralJudge<'_> {
    /// A specification application denotes a stable value only under the same
    /// exact selected public contract/body gate that admitted its source term.
    /// This is distinct from observing the result of an executed call.
    pub(super) fn identity_value_is_available(&self, value: &StructuralTerm) -> bool {
        match value {
            StructuralTerm::Variable(_)
            | StructuralTerm::BoundValue(_)
            | StructuralTerm::BoundProjection { .. }
            | StructuralTerm::CapturedValue { .. }
            | StructuralTerm::Integer(_)
            | StructuralTerm::Float(_) => true,
            StructuralTerm::Application {
                result_type,
                target,
                selections,
                arguments,
                ..
            } => {
                result_type.is_valid()
                    && self
                        .program
                        .operator_definition_by_symbol(*target)
                        .is_none_or(|operator| {
                            self.program.operator_parameters(operator).len() == arguments.len()
                                && crate::value_custody::type_references::type_references_match(
                                    self.program,
                                    *result_type,
                                    operator.return_type,
                                )
                        })
                    && arguments
                        .iter()
                        .all(|value| self.identity_value_is_available(value))
                    && (crate::machine_calls::denotational_calls::abstract_value_call_signature(
                        self.program,
                        *target,
                    )
                    .is_some()
                        || super::specification_application_candidate(
                            self.program,
                            self.machine_symbol,
                            *target,
                            selections,
                            self.frame_resolver(),
                        )
                        .is_ok())
            }
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => {
                if crate::reference_shell(self.program, *observed_type).is_some() {
                    descriptor
                        .as_ref()
                        .is_some_and(|value| self.identity_value_is_available(value))
                } else {
                    self.identity_value_is_available(value)
                }
            }
            StructuralTerm::IntegerEmbedding { value, .. }
            | StructuralTerm::ContentValue { value, .. }
            | StructuralTerm::Projection { subject: value, .. } => {
                self.identity_value_is_available(value)
            }
            StructuralTerm::Constructor { fields, .. } => fields
                .iter()
                .all(|(_, value)| self.identity_value_is_available(value)),
            StructuralTerm::RepeatedArray { length, element } => {
                *length == 0
                    || element
                        .as_ref()
                        .is_some_and(|value| self.identity_value_is_available(value))
            }
            _ => false,
        }
    }

    pub(super) fn intake_identity(&mut self, left: StructuralTerm, right: StructuralTerm) {
        self.trace_le_case(
            "intake-original",
            &[("left", &left), ("right", &right)],
            format_args!(""),
        );
        if self.case_facts_only
            || !self.identity_value_is_available(&left)
            || !self.identity_value_is_available(&right)
        {
            return;
        }
        // Normalize before installing this premise. Its desired consequence
        // cannot supply an operand's missing formation or call admission.
        let normalized = [left.clone(), right.clone()].map(|value| self.resolve(value));
        self.trace_le_case(
            "intake-normalized",
            &[("left", &normalized[0]), ("right", &normalized[1])],
            format_args!(""),
        );
        if !left.same_resolution_payload(&normalized[0])
            || !right.same_resolution_payload(&normalized[1])
        {
            self.retain_identity_pair(left, right);
        }
        self.eliminate_identity(normalized[0].clone(), normalized[1].clone(), 0);
        self.activate_conditional_guarantees();
    }

    fn retain_identity_pair(&mut self, left: StructuralTerm, right: StructuralTerm) {
        Arc::make_mut(&mut self.captured_result_equations)
            .push(super::normal_returns::CapturedResultEquation::data_identity((left, right)));
    }

    fn eliminate_identity(&mut self, left: StructuralTerm, right: StructuralTerm, depth: usize) {
        if depth >= 32 {
            return;
        }
        self.retain_identity_pair(left.clone(), right.clone());
        let owned_subject = |mut value: StructuralTerm| {
            let mut owned = matches!(
                value,
                StructuralTerm::Constructor { .. } | StructuralTerm::IntegerEmbedding { .. }
            );
            loop {
                match value {
                    StructuralTerm::ObservedValue {
                        observed_type,
                        value: inner,
                        ..
                    } => {
                        if !observed_type.is_valid()
                            || crate::reference_shell(self.program, observed_type).is_some()
                        {
                            return None;
                        }
                        owned = true;
                        value = *inner;
                    }
                    StructuralTerm::IntegerEmbedding {
                        reference,
                        value: inner,
                    } if crate::reference_shell(self.program, reference).is_none() => {
                        owned = true;
                        value = *inner;
                    }
                    _ => return Some((value, owned)),
                }
            }
        };
        let (Some((left, left_owned)), Some((right, right_owned))) =
            (owned_subject(left), owned_subject(right))
        else {
            return;
        };
        self.trace_le_case(
            "intake-owned",
            &[("left", &left), ("right", &right)],
            format_args!("depth={depth} left_owned={left_owned} right_owned={right_owned}"),
        );
        if left.same_resolution_payload(&right) {
            return;
        }
        if let (
            StructuralTerm::Constructor {
                identity: left_identity,
                fields: left_fields,
                ..
            },
            StructuralTerm::Constructor {
                identity: right_identity,
                fields: right_fields,
                ..
            },
        ) = (&left, &right)
        {
            if !left_identity.same_carrier(*right_identity) {
                return;
            }
            if left_identity != right_identity {
                self.hypotheses_contradictory = true;
                self.trace_le_case(
                    "constructor-clash",
                    &[("left", &left), ("right", &right)],
                    format_args!("depth={depth}"),
                );
                return;
            }
            let Some(fields) = self.identity_constructor_field_pairs(
                *left_identity,
                left_fields,
                *right_identity,
                right_fields,
            ) else {
                return;
            };
            for (left, right) in fields {
                self.eliminate_identity(left, right, depth + 1);
            }
            return;
        }
        // Only the owned value substitution crosses into the ordinary value
        // resolver. No Boolean comparison is interpreted or asserted here;
        // IEEE fields keep their bits and reference fields stop above.
        if !left_owned && !right_owned {
            return;
        }
        match (&left, &right) {
            (StructuralTerm::Variable(name), _) if !term_contains(&right, &left) => {
                Arc::make_mut(&mut self.substitutions).push((name.clone(), right));
            }
            (_, StructuralTerm::Variable(name)) if !term_contains(&left, &right) => {
                Arc::make_mut(&mut self.substitutions).push((name.clone(), left));
            }
            (
                StructuralTerm::Application { .. }
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::BoundValue(_)
                | StructuralTerm::Projection { .. }
                | StructuralTerm::BoundProjection { .. },
                _,
            ) if !term_contains(&right, &left) => {
                Arc::make_mut(&mut self.rewrites).push_value_identity((left, right));
            }
            (
                _,
                StructuralTerm::Application { .. }
                | StructuralTerm::CapturedValue { .. }
                | StructuralTerm::BoundValue(_)
                | StructuralTerm::Projection { .. }
                | StructuralTerm::BoundProjection { .. },
            ) if !term_contains(&left, &right) => {
                Arc::make_mut(&mut self.rewrites).push_value_identity((right, left));
            }
            _ => {}
        }
    }

    /// Rejoin both checked construction contexts before comparing fields. A
    /// reference handle identifies the producing context, not a new value, but
    /// neither side may borrow the other's substituted field types or policy.
    pub(super) fn identity_constructor_field_pairs(
        &self,
        left: ConstructorIdentity,
        left_fields: &[(String, StructuralTerm)],
        right: ConstructorIdentity,
        right_fields: &[(String, StructuralTerm)],
    ) -> Option<Vec<(StructuralTerm, StructuralTerm)>> {
        if !left.same_constructor(right) || left_fields.len() != right_fields.len() {
            return None;
        }
        let left_declared = self.identity_constructor_fields(left)?;
        let right_declared = self.identity_constructor_fields(right)?;
        if left_declared.len() != left_fields.len() || right_declared.len() != right_fields.len() {
            return None;
        }
        let mut right_by_name = right_fields
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect::<std::collections::HashMap<_, _>>();
        if right_by_name.len() != right_fields.len() {
            return None;
        }
        let mut pairs = Vec::with_capacity(left_fields.len());
        for (name, left) in left_fields {
            let right = right_by_name.remove(name.as_str())?;
            let left_reference = *left_declared.get(name.as_str())?;
            let right_reference = *right_declared.get(name.as_str())?;
            if !left_reference.is_valid()
                || !right_reference.is_valid()
                || !crate::value_custody::type_references::type_references_match(
                    self.program,
                    left_reference,
                    right_reference,
                )
            {
                return None;
            }
            let observe = |reference, value| {
                super::super::structural_terms::owned_projection_observation(
                    self.program,
                    reference,
                    value,
                )
            };
            pairs.push((
                observe(left_reference, left.clone()),
                observe(right_reference, right.clone()),
            ));
        }
        Some(pairs)
    }

    fn identity_constructor_fields(
        &self,
        identity: ConstructorIdentity,
    ) -> Option<std::collections::HashMap<String, typed_trees::types::TypeReferenceHandle>> {
        let ConstructorIdentity::Nominal {
            reference,
            declaration,
            case,
            ..
        } = identity
        else {
            return Some(std::collections::HashMap::new());
        };
        let declaration = if reference.is_valid() {
            self.program
                .data_constructor_definition(reference, declaration)?
        } else {
            self.program.data_definition_by_symbol(declaration)?
        };
        let members = self.program.data_members(declaration);
        let variant = match case {
            Some(symbol) => Some(members.iter().find_map(|member| match member {
                typed_trees::data::DataMember::Variant(variant) if variant.symbol == symbol => {
                    Some(variant)
                }
                _ => None,
            })?),
            None => None,
        };
        let fields = self
            .program
            .data_construction_fields(members, variant.map(|variant| variant.symbol));
        Some(
            fields
                .into_iter()
                .map(|field| (field.name.as_str().to_owned(), field.type_reference))
                .collect(),
        )
    }
}
