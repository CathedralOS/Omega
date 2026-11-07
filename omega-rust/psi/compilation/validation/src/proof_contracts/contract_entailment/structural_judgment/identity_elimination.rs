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
        if self.case_facts_only
            || !self.identity_value_is_available(&left)
            || !self.identity_value_is_available(&right)
        {
            return;
        }
        // Normalize before installing this premise. Its desired consequence
        // cannot supply an operand's missing formation or call admission.
        let normalized = [left.clone(), right.clone()].map(|value| self.resolve(value));
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
                return;
            }
            if !left_identity.same_resolution_payload(*right_identity)
                || left_fields.len() != right_fields.len()
            {
                return;
            }
            let Some(fields) = self.identity_constructor_fields(*left_identity) else {
                return;
            };
            if fields.len() != left_fields.len() {
                return;
            }
            for ((left_name, left), (right_name, right)) in left_fields.iter().zip(right_fields) {
                let Some(reference) = fields.get(left_name.as_str()) else {
                    return;
                };
                if left_name != right_name {
                    return;
                }
                let observe = |value| {
                    super::super::structural_terms::owned_projection_observation(
                        self.program,
                        *reference,
                        value,
                    )
                };
                self.eliminate_identity(observe(left.clone()), observe(right.clone()), depth + 1);
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

    pub(super) fn identity_constructor_fields(
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
        let fields = self.program.data_member_fields(members).chain(
            variant
                .into_iter()
                .flat_map(|variant| self.program.data_payload_fields(variant)),
        );
        Some(
            fields
                .map(|field| (field.name.as_str().to_owned(), field.type_reference))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::bignum::BigInt;
    use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::TypedTrees;
    use typed_trees::data::{DataDefinition, DataField, DataMember};
    use typed_trees::name::Identifier;
    use typed_trees::types::TypeReferenceNode;

    #[test]
    fn canonical_content_identity_does_not_identify_literal_former_fields() {
        // Start after formation and custody admission, as the other judge
        // controls do. These two notations both denote the empty interval set.
        let mut symbols = SymbolTableBuilder::default();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = symbols.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Static("IntervalSet")),
                (SymbolKind::Data, SymbolNameRef::Static("Coordinate")),
                (SymbolKind::Data, SymbolNameRef::Static("Holder")),
            ],
        );
        let interval = declarations.handle_at(0);
        let coordinate = declarations.handle_at(1);
        let holder = declarations.handle_at(2);
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        let integer = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::from("u64"),
            });
        let coordinate = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: coordinate,
                name: Identifier::from("Coordinate"),
            });
        let arguments = program
            .type_reference_table
            .insert_type_reference_handles([coordinate]);
        let interval_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Generic {
                base_symbol: interval,
                base_name: Identifier::from("IntervalSet"),
                lifetime_arguments: Vec::new(),
                arguments,
            });
        for (symbol, name, fields) in [
            (
                interval,
                "IntervalSet",
                [("start", integer), ("end", integer)],
            ),
            (
                holder,
                "Holder",
                [("content", interval_type), ("marker", integer)],
            ),
        ] {
            let members = program
                .data_members
                .insert_many(fields.map(|(name, type_reference)| {
                    DataMember::Field(DataField {
                        name: Identifier::from(name),
                        type_reference,
                        ..Default::default()
                    })
                }));
            program.push_data_definition(DataDefinition {
                symbol,
                name: Identifier::from(name),
                members,
                ..Default::default()
            });
        }
        let constructor = |symbol, name: &str, fields| StructuralTerm::Constructor {
            identity: ConstructorIdentity::nominal(
                &program,
                program.data_definition_by_symbol(symbol).unwrap(),
                None,
            )
            .unwrap(),
            data: name.to_owned(),
            case: String::new(),
            fields,
        };
        let integer = |value| StructuralTerm::Integer(BigInt::from_i64(value));
        let empty = |endpoint| {
            constructor(
                interval,
                "IntervalSet",
                vec![
                    ("start".to_owned(), integer(endpoint)),
                    ("end".to_owned(), integer(endpoint)),
                ],
            )
        };
        let observe = |value| {
            super::super::super::structural_terms::owned_projection_observation(
                &program,
                interval_type,
                value,
            )
        };
        let left = observe(empty(5));
        let right = observe(empty(9));
        assert!(matches!(&left, StructuralTerm::ContentValue { .. }));
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            SymbolHandle::invalid(),
            &[],
            &[],
            false,
        );
        judge.intake_identity(left.clone(), right.clone());
        assert!(matches!(
            judge.judge_data_identity(left.clone(), right, 0),
            super::super::StructuralJudgment::Proven
        ));
        assert!(
            super::super::super::observed_projections::project_declared_field(
                &program, &judge, left, "start",
            )
            .is_none()
        );
        assert!(!matches!(
            judge.judge_explicit_identity(integer(5), integer(9), 0),
            super::super::StructuralJudgment::Proven
        ));

        // Ordinary record elimination must reach the content field and keep
        // its canonical barrier, while still substituting the scalar sibling.
        let mut judge = StructuralJudge::from_declaration_requires(
            &program,
            SymbolHandle::invalid(),
            &[],
            &[],
            false,
        );
        let marker = StructuralTerm::Variable("marker".to_owned());
        judge.intake_identity(
            constructor(
                holder,
                "Holder",
                vec![
                    ("content".to_owned(), empty(5)),
                    ("marker".to_owned(), marker.clone()),
                ],
            ),
            constructor(
                holder,
                "Holder",
                vec![
                    ("content".to_owned(), empty(9)),
                    ("marker".to_owned(), integer(7)),
                ],
            ),
        );
        assert!(judge.resolve(marker).same_resolution_payload(&integer(7)));
        assert!(matches!(
            judge.judge_data_identity(observe(empty(5)), observe(empty(9)), 0),
            super::super::StructuralJudgment::Proven
        ));
        assert!(!judge.hypotheses_contradictory);
        assert!(!matches!(
            judge.judge_explicit_identity(integer(5), integer(9), 0),
            super::super::StructuralJudgment::Proven
        ));
    }
}
