//! Bind selected contract terms under the observer's existing case scope.
//!
//! Formal/result substitution preserves exact declarations and completed values.
//! A payload projection additionally consumes this scope's independent case
//! judgment through the ordinary typed observation walker. Source formation and
//! call-entry admission remain prerequisites, not conclusions imported here.

use super::super::{StructuralJudge, StructuralTerm};
use super::metadata_paths;
use super::{owner_symbol, selected_call_contract_for_parameters};
use crate::proof_contracts::contract_entailment::{
    observed_projections, proof_integer, structural_terms,
};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

impl StructuralJudge<'_> {
    /// Formal substitution uses selected symbols. Names are only the existing
    /// structural judge's diagnostic payload; no caller spelling chooses a formal.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::proof_contracts::contract_entailment) fn bound_contract_term(
        &self,
        expression: ExpressionHandle,
        scope: SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        callable: SymbolHandle,
        bindings: &[(SymbolHandle, StructuralTerm)],
        type_bindings: &[(SymbolHandle, TypeReferenceHandle)],
        result: Option<&StructuralTerm>,
        depth: usize,
    ) -> Option<StructuralTerm> {
        let program = self.program;
        if depth >= 64 {
            return None;
        }
        if program.static_type_predicate_operands(expression).is_some() {
            return structural_terms::static_type_predicate_term(
                program,
                expression,
                type_bindings,
            );
        }
        if let Some(value) = proof_integer::anonymous_integer_value(program, expression) {
            return Some(StructuralTerm::Integer(value));
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(path) => {
                if let Some((_, value)) = bindings
                    .iter()
                    .find(|(symbol, _)| symbol.is_valid() && *symbol == path.symbol)
                {
                    return Some(value.clone());
                }
                // An applied contract may already name the supplying static
                // binder rather than its receiving formal. That exact value
                // remains authenticated in the selected substitution. Other
                // unbound parameters still supply no contract term.
                if path.symbol.is_valid()
                    && let Some(value) = bindings.iter().map(|(_, value)| value).find(|value| {
                        matches!(value, StructuralTerm::BoundValue(symbol) if *symbol == path.symbol)
                    })
                {
                    return Some(value.clone());
                }
                if crate::reserved_result_owner(program, expression)
                    .is_some_and(|owner| owner.0 == callable)
                {
                    return result.cloned();
                }
                if path.symbol.is_valid()
                    && !matches!(
                        program.symbols.get(path.symbol).kind,
                        symbols::SymbolKind::Parameter | symbols::SymbolKind::TypeParameter
                    )
                {
                    return Some(StructuralTerm::BoundValue(path.symbol));
                }
                None
            }
            ExpressionNode::Member(member) => {
                // A formal/result substitutes an exact completed value. Only
                // its live authenticated scalar cell may observe a returned
                // reference; the ordinary projection walker stays descriptor-only.
                let completed = if let Some(place) =
                    crate::reserved_result_place(program, expression)
                {
                    (place.machine_symbol == callable)
                        .then(|| {
                            result.and_then(|result| {
                                self.completed_result_read(result, &place.segments)
                            })
                        })
                        .flatten()
                } else {
                    crate::canonical_place_from_expression(program, expression).and_then(|place| {
                        let symbol = place.root.as_symbol()?;
                        parameters
                            .iter()
                            .find(|parameter| parameter.symbol == symbol)?;
                        let (_, value) = bindings.iter().find(|(bound, _)| *bound == symbol)?;
                        self.completed_result_read(value, &place.segments)
                    })
                };
                if completed.is_some() {
                    return completed;
                }
                let reference = crate::parameter_expression_result_type_reference(
                    program,
                    scope,
                    parameters,
                    member.receiver,
                )?;
                if let Some(measure) =
                    structural_terms::collection_measure_term(program, member, reference, || {
                        self.bound_contract_term(
                            member.receiver,
                            scope,
                            parameters,
                            callable,
                            bindings,
                            type_bindings,
                            result,
                            depth + 1,
                        )
                    })
                {
                    return Some(measure);
                }
                if let Some(measure) =
                    structural_terms::collection_measure_term(program, member, reference, || {
                        metadata_paths::bound_descriptor(
                            self,
                            member.receiver,
                            scope,
                            parameters,
                            callable,
                            bindings,
                            type_bindings,
                            result,
                            depth + 1,
                        )
                    })
                {
                    return Some(measure);
                }
                let carrier =
                    crate::value_custody::places::unwrapped_type_reference(program, reference)?;
                // The applied contract's fields belong to its selected data
                // application, not a same-named field on the generic base.
                let data =
                    crate::value_custody::places::data_definition_for_type(program, carrier)?;
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|case| case.as_str()),
                )?;
                let value = self.bound_contract_term(
                    member.receiver,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    type_bindings,
                    result,
                    depth + 1,
                )?;
                let mut path = Vec::new();
                crate::value_custody::places::push_field_place_segments(
                    program,
                    &mut path,
                    field.symbol,
                );
                observed_projections::project_observed_value_in_scope(
                    program, self, reference, value, &path,
                )
            }
            ExpressionNode::Indexed(indexed) => structural_terms::indexed_term_in_scope(
                program,
                None,
                Some((scope, parameters)),
                expression,
                self.bound_contract_term(
                    indexed.collection,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    type_bindings,
                    result,
                    depth + 1,
                )?,
                self.bound_contract_term(
                    indexed.index,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    type_bindings,
                    result,
                    depth + 1,
                )?,
            ),
            ExpressionNode::Call(call) => {
                if let Some((_, argument)) = crate::integer_embedding_argument(program, expression)
                {
                    let reference = crate::parameter_expression_result_type_reference(
                        program, scope, parameters, argument,
                    )?;
                    let value = self.bound_contract_term(
                        argument,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        type_bindings,
                        result,
                        depth + 1,
                    )?;
                    return structural_terms::integer_embedding_term(
                        program, expression, reference, value,
                    );
                }
                let selected =
                    selected_call_contract_for_parameters(program, call, scope, parameters)?;
                let realization = structural_terms::realized_requirement_state(call);
                let arguments = program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|argument| {
                        self.bound_contract_term(
                            *argument,
                            scope,
                            parameters,
                            callable,
                            bindings,
                            type_bindings,
                            result,
                            depth + 1,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?;
                let result_type = crate::parameter_expression_result_type_reference(
                    program, scope, parameters, expression,
                )
                .unwrap_or_default();
                Some(StructuralTerm::Application {
                    result_type,
                    target: realization.unwrap_or_else(|| owner_symbol(&selected)),
                    selections: if realization.is_some() {
                        Vec::new()
                    } else {
                        call.machine_arguments.to_vec()
                    },
                    machine: structural_terms::structural_call_machine_name(
                        call.target.as_str(),
                        &call.machine_arguments,
                        &[],
                    ),
                    arguments,
                })
            }
            ExpressionNode::StructLiteral(literal) => structural_terms::constructor_literal_term(
                program,
                expression,
                literal,
                |_, field| {
                    let field = field?;
                    self.bound_contract_term(
                        field.value,
                        scope,
                        parameters,
                        callable,
                        bindings,
                        type_bindings,
                        result,
                        depth + 1,
                    )
                },
            ),
            ExpressionNode::Unary(unary) => structural_terms::logical_not_term_in_scope(
                program,
                None,
                Some((scope, parameters)),
                expression,
                self.bound_contract_term(
                    unary.operand,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    type_bindings,
                    result,
                    depth + 1,
                )?,
            ),
            // Public guarantees observe the admitted conversion's value through
            // the same denotation rule as ordinary contract expressions. Conversion
            // formation remains a separate obligation at its authored occurrence.
            ExpressionNode::Cast(cast) => {
                let source = crate::parameter_expression_result_type_reference(
                    program, scope, parameters, cast.value,
                )?;
                structural_terms::denotation_preserving_conversion(program, cast, source)
                    .then(|| {
                        self.bound_contract_term(
                            cast.value,
                            scope,
                            parameters,
                            callable,
                            bindings,
                            type_bindings,
                            result,
                            depth + 1,
                        )
                    })
                    .flatten()
            }
            ExpressionNode::Integer(_) | ExpressionNode::Float(_) | ExpressionNode::Boolean(_) => {
                structural_terms::structural_term(program, None, None, expression, None)
            }
            ExpressionNode::Binary(binary) => {
                // A sealed primitive float comparison is an IEEE Boolean, not
                // identity and not a reason to drop the substituted operands.
                // Other non-builtin operators stay unbound here.
                if !crate::has_builtin_parameter_bound_expression_meaning(
                    program, scope, parameters, expression,
                ) {
                    let types = [binary.left, binary.right].map(|operand| {
                        crate::parameter_expression_result_type_reference(
                            program, scope, parameters, operand,
                        )
                    });
                    structural_terms::sealed_primitive_float_operator(
                        program, scope, expression, &types,
                    )?;
                }
                let left = self.bound_contract_term(
                    binary.left,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    type_bindings,
                    result,
                    depth + 1,
                )?;
                let right = self.bound_contract_term(
                    binary.right,
                    scope,
                    parameters,
                    callable,
                    bindings,
                    type_bindings,
                    result,
                    depth + 1,
                )?;
                structural_terms::binary_term_in_scope(
                    program,
                    None,
                    Some((scope, parameters)),
                    expression,
                    left,
                    right,
                )
            }
            _ => None,
        }
    }
}
