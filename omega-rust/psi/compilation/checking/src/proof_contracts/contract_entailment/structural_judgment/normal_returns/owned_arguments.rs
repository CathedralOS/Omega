//! Ordinary calls retain owned inline cells independently of referent isolation.
//! Both entry preconditions and normal-return bindings use this admission;
//! post-call preservation remains the separate call-frame decision.

use super::{StructuralJudge, StructuralTerm, TypeReferenceHandle};

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn owned_argument_observation(
        &self,
        declared: TypeReferenceHandle,
        value: &StructuralTerm,
    ) -> Option<StructuralTerm> {
        if !declared.is_valid() || crate::reference_shell(self.program, declared).is_some() {
            return None;
        }
        if self
            .frame_resolver()
            .is_some_and(|frames| frames.proof_value_is_caller_isolated(declared))
        {
            return Some(value.clone());
        }
        Some(
            super::super::super::structural_terms::owned_projection_observation(
                self.program,
                declared,
                value.clone(),
            ),
        )
    }
}

impl StructuralJudge<'_> {
    pub(in crate::proof_contracts::contract_entailment) fn completed_scalar_subject(
        &self,
        subject: super::super::super::specification_calls::RuntimeScalarSubject,
        site: crate::CallSite<'_>,
        arguments: &[StructuralTerm],
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Option<StructuralTerm> {
        use super::super::super::specification_calls::RuntimeScalarSubject;
        match subject {
            RuntimeScalarSubject::Expression(expression) => {
                let mut root = expression;
                let mut path = Vec::new();
                while let typed_trees::expression::ExpressionNode::Member(member) =
                    self.program.expression_table.expression(root)
                {
                    if !member.member_symbol.is_valid() || member.case_variant.is_some() {
                        return None;
                    }
                    path.push(facts::PlaceSegment::Field {
                        symbol: member.member_symbol,
                    });
                    root = member.receiver;
                }
                if let typed_trees::expression::ExpressionNode::SavedSubject(saved) =
                    self.program.expression_table.expression(root)
                    && let Some(position) = capture_position_at_site(self.program, saved, site)
                {
                    let source = self.program.saved_value_source(saved)?;
                    let reference = crate::expression_result_type_reference(
                        self.program,
                        machine,
                        state,
                        source,
                    )?;
                    path.reverse();
                    return super::super::super::observed_projections::project_observed_value(
                        self.program,
                        reference,
                        arguments.get(position)?.clone(),
                        &path,
                    )
                    .map(|value| self.completed_value_identity(value));
                }
                self.normal_results
                    .iter()
                    .rev()
                    .find(|(source, _)| *source == expression)
                    .map(|(_, value)| self.completed_value_identity(value.clone()))
            }
            RuntimeScalarSubject::Symbol(symbol) => {
                if let Some(value) = self.captured_parameter_projection(
                    symbol,
                    &[],
                    language_semantics::content::ContentPlaceVersion::Current,
                ) {
                    return Some(value);
                }
                if self
                    .program
                    .machine_type_parameters(machine)
                    .iter()
                    .any(|parameter| {
                        parameter.symbol == symbol
                            && matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Value { .. }
                                    | typed_trees::data::TypeParameterKind::Const { .. }
                            )
                    })
                {
                    return Some(StructuralTerm::BoundValue(symbol));
                }
                let parameter = self
                    .program
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == symbol)?;
                (self
                    .program
                    .primitive_type_reference(parameter.type_reference)
                    .is_some()
                    && !self.storage_binding_invalidated(symbol))
                .then(|| StructuralTerm::Variable(parameter.name.to_string()))
            }
        }
    }
}

pub(super) fn capture_position_at_site(
    program: &dyn crate::CheckingInput,
    saved: &typed_trees::expression::TableSavedSubject,
    site: crate::CallSite<'_>,
) -> Option<usize> {
    use typed_trees::expression::ValueCaptureOwner;
    program.saved_value_source(saved)?;
    match (
        program.expression_table.value_capture(saved.capture).owner,
        site,
    ) {
        (
            ValueCaptureOwner::ExpressionArgument { call, position },
            crate::CallSite::Expression { expression, .. },
        ) if call == expression => Some(position as usize),
        (
            ValueCaptureOwner::StatementArgument { call, position },
            crate::CallSite::Statement(statement),
        ) if program
            .statement_table
            .statement(call)
            .as_call()
            .is_some_and(|stored| std::ptr::eq(stored, statement)) =>
        {
            Some(position as usize)
        }
        _ => None,
    }
}
