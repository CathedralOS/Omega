//! Read complete equality statements in their declaring parameter telescope.
//!
//! A sealed Id and a selected Boolean comparison remain distinct relations.
//! Only the existing same-carrier builtin scalar bridge can make the latter
//! available as identity. These views describe obligations, never their proof.

use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::signature::StateParameter;
use typed_trees::type_identity::{TypeIdentityRequest, normalized_expression_identity};
use typed_trees::types::TypeReferenceHandle;

use crate::mathematical_applications::{IdentityApplication, identity_application_at};

#[derive(Clone, Copy)]
pub(crate) struct EqualityConjunct<'scope> {
    owner: SymbolHandle,
    parameters: &'scope [StateParameter],
    expression: ExpressionHandle,
    subjects: [ExpressionHandle; 2],
    meaning: EqualityMeaning,
}

#[derive(Clone, Copy)]
enum EqualityMeaning {
    Identity(TypeReferenceHandle),
    TypeEquality,
    Comparison,
}

impl<'scope> EqualityConjunct<'scope> {
    pub(crate) fn expression(self) -> ExpressionHandle {
        self.expression
    }

    fn scope(self) -> (SymbolHandle, &'scope [StateParameter]) {
        (self.owner, self.parameters)
    }

    pub(crate) fn identity(self) -> Option<IdentityApplication> {
        let EqualityMeaning::Identity(carrier) = self.meaning else {
            return None;
        };
        Some(IdentityApplication {
            carrier,
            subjects: self.subjects,
        })
    }
}

pub(crate) fn equality_conjunct<'scope>(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &'scope [StateParameter],
    expression: ExpressionHandle,
) -> Option<EqualityConjunct<'scope>> {
    if super::super::structural_terms::is_case_observation(program, expression) {
        return None;
    }
    if let Ok(identity) = identity_application_at(program, expression) {
        return Some(EqualityConjunct {
            owner,
            parameters,
            expression,
            subjects: identity.subjects,
            meaning: EqualityMeaning::Identity(identity.carrier),
        });
    }
    let (subjects, meaning) = if let Some(binary) = program.expression_table.binary_node(expression)
    {
        if binary.operator != BinaryOperator::Equal {
            return None;
        }
        let meaning = if program.static_type_predicate_operands(expression).is_some() {
            EqualityMeaning::TypeEquality
        } else {
            super::super::structural_terms::builtin_identity_operand_references(
                program, owner, parameters, expression,
            )
            .map_or(EqualityMeaning::Comparison, |references| {
                EqualityMeaning::Identity(references[0])
            })
        };
        ([binary.left, binary.right], meaning)
    } else {
        (
            selected_comparison_subjects(program, owner, expression)?,
            EqualityMeaning::Comparison,
        )
    };
    Some(EqualityConjunct {
        owner,
        parameters,
        expression,
        subjects,
        meaning,
    })
}

/// Preparation can bind an authored comparison to an ordinary call. Its
/// selected public declaration, never a callee's displayed name or body,
/// determines whether it is still a comparison statement.
fn selected_comparison_subjects(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<[ExpressionHandle; 2]> {
    use super::super::specification_calls::{SelectedCallContract, selected_call_contract};
    let call = program.expression_table.call_node(expression)?;
    let contract = selected_call_contract(program, owner, call)?;
    let equal = Some(language_core::OperatorSpelling::Equal);
    let selected = match contract {
        SelectedCallContract::Signature(signature)
        | SelectedCallContract::DynamicSlot { signature, .. } => signature.spelling == equal,
        SelectedCallContract::Operator(operator) => operator.spelling == equal,
        SelectedCallContract::Concrete(machine, _) => {
            machine.spelling == equal
                || program
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| {
                        program
                            .trait_definition_by_symbol(conformance.symbol)
                            .and_then(|definition| {
                                program.trait_machine_signature_by_symbol(
                                    definition,
                                    conformance.requirement_symbol,
                                )
                            })
                            .is_some_and(|signature| signature.spelling == equal)
                    })
        }
    };
    if !selected {
        return None;
    }
    let namespace = !call.receiver.is_valid()
        || super::super::structural_terms::static_call_receiver(program, owner, call);
    match (
        namespace,
        program.expression_table.expression_handles(call.arguments),
    ) {
        (true, [left, right]) => Some([*left, *right]),
        (false, [right]) => Some([call.receiver, *right]),
        _ => None,
    }
}

/// One declaration reuses graph scratch across all its clause candidates.
/// Epochs make a tiny law visit cost proportional to its reached expressions,
/// rather than clearing the unrelated expression-arena prefix per comparison.
#[derive(Default)]
pub(crate) struct SourceCorrespondence {
    visits: Vec<SourceVisit>,
    epoch: u32,
}

#[derive(Clone, Copy, Default)]
struct SourceVisit {
    epoch: u32,
    state: u8,
}

impl SourceCorrespondence {
    /// Match statements under complete positional binders. Identity may reverse
    /// endpoints; a selected comparison remains that exact Boolean expression.
    /// The canonical declaration identity retains calls, carriers and static
    /// operands, and supplies no proof that distinct expressions are equivalent.
    pub(crate) fn matching_conjuncts(
        &mut self,
        program: &dyn crate::CheckingInput,
        required: EqualityConjunct<'_>,
        required_binders: &[(SymbolHandle, String)],
        provided: EqualityConjunct<'_>,
        provided_binders: &[(SymbolHandle, String)],
    ) -> bool {
        match (required.meaning, provided.meaning) {
            (
                EqualityMeaning::Identity(required_carrier),
                EqualityMeaning::Identity(provided_carrier),
            ) => {
                let carrier = |reference, binders| {
                    program.type_identity(TypeIdentityRequest {
                        binders,
                        ..TypeIdentityRequest::ordinary(reference)
                    })
                };
                if carrier(required_carrier, required_binders)
                    != carrier(provided_carrier, provided_binders)
                {
                    return false;
                }
                (self.matching_values(
                    program,
                    required.scope(),
                    required.subjects[0],
                    required_binders,
                    provided.scope(),
                    provided.subjects[0],
                    provided_binders,
                ) && self.matching_values(
                    program,
                    required.scope(),
                    required.subjects[1],
                    required_binders,
                    provided.scope(),
                    provided.subjects[1],
                    provided_binders,
                )) || (self.matching_values(
                    program,
                    required.scope(),
                    required.subjects[0],
                    required_binders,
                    provided.scope(),
                    provided.subjects[1],
                    provided_binders,
                ) && self.matching_values(
                    program,
                    required.scope(),
                    required.subjects[1],
                    required_binders,
                    provided.scope(),
                    provided.subjects[0],
                    provided_binders,
                ))
            }
            (EqualityMeaning::TypeEquality, EqualityMeaning::TypeEquality)
            | (EqualityMeaning::Comparison, EqualityMeaning::Comparison) => self.matching_values(
                program,
                required.scope(),
                required.expression,
                required_binders,
                provided.scope(),
                provided.expression,
                provided_binders,
            ),
            _ => false,
        }
    }

    /// Source structure has a canonical key only after every infix operation
    /// retains builtin meaning in this declaration. Selected calls retain their
    /// resolved target and full static application in the shared key. A raw
    /// selected infix instead uses its checked structural application below.
    fn canonical_source_value(
        &mut self,
        program: &dyn crate::CheckingInput,
        statement: (SymbolHandle, &[StateParameter]),
        expression: ExpressionHandle,
        binders: &[(SymbolHandle, String)],
    ) -> Option<String> {
        if self.epoch == u32::MAX {
            self.visits.fill(SourceVisit::default());
            self.epoch = 0;
        }
        self.epoch += 1;
        let mut pending = vec![(expression, false)];
        while let Some((expression, completed)) = pending.pop() {
            if !expression.is_valid() {
                continue; // An absent receiver is a structural call edge.
            }
            if !program.expression_table.expression_is_valid(expression) {
                return None;
            }
            let position = expression.arena_index() as usize;
            if self.visits.len() <= position {
                self.visits.resize(position + 1, SourceVisit::default());
            }
            let visit = &mut self.visits[position];
            if completed {
                *visit = SourceVisit {
                    epoch: self.epoch,
                    state: 2,
                };
                continue;
            }
            match (visit.epoch == self.epoch, visit.state) {
                (true, 1) => return None,
                (true, 2) => continue,
                _ => {
                    *visit = SourceVisit {
                        epoch: self.epoch,
                        state: 1,
                    }
                }
            }
            pending.push((expression, true));
            let node = program.expression_table.expression(expression);
            if let Some(binary) = program.expression_table.binary_node(expression)
                && program.static_type_predicate_operands(expression).is_none()
                && let Some(spelling) =
                    crate::proof_contracts::parameter_expression_meaning::spelled_binary_operator(
                        binary.operator,
                    )
            {
                let operands = [binary.left, binary.right].map(|operand| {
                    crate::parameter_expression_result_type_reference(
                        program,
                        statement.0,
                        statement.1,
                        operand,
                    )
                });
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    statement.0,
                    expression,
                    spelling,
                    &operands,
                ) {
                    return None;
                }
            }
            crate::queries::expression_traversal::children(program, node, |child| {
                pending.push((child, false))
            });
        }
        if !crate::proof_contracts::contract_results::expression_has_source_builtin_index_meaning(
            program, expression,
        ) {
            return None;
        }
        normalized_expression_identity(program.typed_program(), expression, binders)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn matching_values(
        &mut self,
        program: &dyn crate::CheckingInput,
        required: (SymbolHandle, &[StateParameter]),
        required_expression: ExpressionHandle,
        required_binders: &[(SymbolHandle, String)],
        provided: (SymbolHandle, &[StateParameter]),
        provided_expression: ExpressionHandle,
        provided_binders: &[(SymbolHandle, String)],
    ) -> bool {
        if let (Some(required), Some(provided)) = (
            self.canonical_source_value(program, required, required_expression, required_binders),
            self.canonical_source_value(program, provided, provided_expression, provided_binders),
        ) {
            return required == provided;
        }
        let term = |statement: (SymbolHandle, &[StateParameter]), expression| {
            super::super::structural_term(
                program,
                None,
                Some((statement.0, statement.1)),
                expression,
                None,
            )
        };
        let (Some(required_term), Some(provided_term)) = (
            term(required, required_expression),
            term(provided, provided_expression),
        ) else {
            return false;
        };
        let (Some(required_roles), Some(provided_roles)) = (
            parameter_roles(required.1, required_binders),
            parameter_roles(provided.1, provided_binders),
        ) else {
            return false;
        };
        if required_roles.len() != provided_roles.len() {
            return false;
        }
        let mut bindings = Vec::with_capacity(required_roles.len());
        for ((required_key, required_parameter), (provided_key, provided_parameter)) in
            required_roles.into_iter().zip(provided_roles)
        {
            if required_key != provided_key {
                return false;
            }
            bindings.push((
                required_parameter.name.as_str().to_owned(),
                super::super::StructuralTerm::Variable(provided_parameter.name.as_str().to_owned()),
            ));
        }
        let variables = required
            .1
            .iter()
            .map(|parameter| parameter.name.as_str().to_owned())
            .collect::<Vec<_>>();
        super::slot_bindings::diagnostic_shape_match(
            program,
            &required_term,
            &provided_term,
            &variables,
            &mut bindings,
        )
    }
}

/// Join the retained symbol roster to positional roles once. Sorting is
/// bounded by this callable's telescope; repeated or missing symbols/roles
/// cannot turn an incomplete scope into a correspondence.
fn parameter_roles<'parameters, 'binders>(
    parameters: &'parameters [StateParameter],
    binders: &'binders [(SymbolHandle, String)],
) -> Option<Vec<(&'binders str, &'parameters StateParameter)>> {
    let mut by_symbol = binders
        .iter()
        .map(|(symbol, key)| (*symbol, key.as_str()))
        .collect::<Vec<_>>();
    by_symbol.sort_unstable_by_key(|(symbol, _)| *symbol);
    if by_symbol.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return None;
    }
    let mut roles = parameters
        .iter()
        .map(|parameter| {
            if !parameter.symbol.is_valid() {
                return None;
            }
            let position = by_symbol
                .binary_search_by_key(&parameter.symbol, |(symbol, _)| *symbol)
                .ok()?;
            Some((by_symbol[position].1, parameter))
        })
        .collect::<Option<Vec<_>>>()?;
    roles.sort_unstable_by_key(|(key, _)| *key);
    (!roles.windows(2).any(|pair| pair[0].0 == pair[1].0)).then_some(roles)
}

/// Collect supported leaves in source order. The result says whether the
/// entire conjunction was supported; unsupported siblings never acquire a
/// matching-law receipt merely because another leaf was an identity.
pub(crate) fn collect_equality_conjuncts<'scope>(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &'scope [StateParameter],
    expression: ExpressionHandle,
    out: &mut Vec<EqualityConjunct<'scope>>,
) -> bool {
    let mut pending = vec![expression];
    let mut complete = true;
    while let Some(expression) = pending.pop() {
        if let Some(binary) = program.expression_table.binary_node(expression)
            && binary.operator == BinaryOperator::And
        {
            pending.extend([binary.right, binary.left]);
        } else if let Some(conjunct) = equality_conjunct(program, owner, parameters, expression) {
            out.push(conjunct);
        } else {
            complete = false;
        }
    }
    complete
}

#[cfg(test)]
mod tests;
