//! Freeze one call's public application and operand/storage bindings.
//!
//! The result ordinal is reserved after entry and staged post-storage captures.
//! Publication waits for normal completion and its actual result evidence.

use super::super::selected_contracts::PreparedSelectedGuarantees;
use super::*;

pub(super) struct PreparedNormalCall<'program, 'actuals> {
    pub(super) expression: ExpressionHandle,
    pub(super) call: &'program typed_trees::expression::TableCallExpression,
    pub(super) owner: SelectedCallContract<'program>,
    pub(super) application: AppliedProofBindings,
    pub(super) parameters: std::borrow::Cow<'program, [typed_trees::signature::StateParameter]>,
    pub(super) bindings: call_bindings::CapturedCallBindings,
    pub(super) actuals: &'actuals [StructuralTerm],
    pub(super) callable: SymbolHandle,
    pub(super) result: Option<ReservedNormalResult>,
}

pub(super) struct ReservedNormalResult {
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) receiving_type: TypeReferenceHandle,
    pub(super) occurrence: usize,
}

impl<'program> StructuralJudge<'program> {
    pub(super) fn prepare_normal_call<'actuals>(
        &mut self,
        expression: ExpressionHandle,
        actuals: Option<&'actuals [StructuralTerm]>,
        environment: &[(String, StructuralTerm)],
        scope: ValueScope<'_>,
        writes: &NormalizedWriteFrame,
    ) -> Option<PreparedNormalCall<'program, 'actuals>> {
        let program = self.program;
        let node = expression_table(program).expression(expression);
        let ExpressionNode::Call(call) = node else {
            return None;
        };
        let Some(owner) = selected_call_contract(program, scope.0.symbol, call) else {
            if super::super::super::struct_trace() {
                eprintln!(
                    "STRUCT normal-capture refused expression={expression:?} target={} stage=selected-contract",
                    call.target
                );
            }
            return None;
        };
        // Computed proof machines are erased mathematical applications. Their
        // definitions and induction remain with the existing term owner;
        // replacing them with opaque runtime occurrences loses that denotation.
        if call_has_mathematical_denotation(program, scope.0.symbol, call) {
            return None;
        }
        if !call.evidence_arguments.is_empty() || call.quotient_operation.is_some() {
            return None;
        }
        let site = crate::CallSite::Expression { expression, call };
        let application =
            AppliedProofBindings::for_call_site(program, scope.0.symbol, &owner, site).or_else(|| {
                if super::super::super::struct_trace() {
                    eprintln!("STRUCT normal-capture refused expression={expression:?} target={} owner={:?} application={:?} stage=static-bindings", call.target, owner_symbol(&owner), call.authored_machine_application);
                }
                None
            })?;
        // The shared application intake authenticates the complete static
        // telescope. This observer imports only its selected public contract;
        // it never interprets a callback body. The exact call's callback
        // dependencies still pass independent_call_application below before
        // any normal guarantee is imported.
        let parameter_scope = super::super::super::specification_calls::call_parameter_scope(
            program,
            scope.0.symbol,
            &owner,
            crate::CallSite::Expression { expression, call },
        ).or_else(|| {
            if super::super::super::struct_trace() {
                eprintln!("STRUCT normal-capture refused expression={expression:?} target={} owner={:?} stage=parameter-scope", call.target, owner_symbol(&owner));
            }
            None
        })?;
        let parameters = parameter_scope.as_ref();
        if super::super::super::struct_trace() {
            eprintln!(
                "STRUCT normal-capture entry expression={expression:?} owner={:?} arguments={actuals:?}",
                owner_symbol(&owner)
            );
        }
        let call_bindings::CapturedCallBindings {
            entry_bindings,
            bindings,
            post_storage,
        } = self.capture_call_bindings(
            site,
            expression,
            arena::Handle::invalid(),
            &owner,
            &application,
            parameters,
            actuals,
            environment,
            scope,
            writes,
        )?;
        let actuals = actuals?.get(owner_leading_operands(&owner)..)?;
        let callable = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(&owner),
        };
        // Carrier equality is checked under the exact public application.
        // A policy adapter still cannot donate a raw declaration's equation.
        // Share constructed_results' exact public/applied result telescope.
        // This immutable type correspondence never re-observes actual values.
        let applied_result_parameters = site.parameters(program, scope.0.symbol);
        let result_presentation = if matches!(
            program
                .type_reference_table
                .type_reference(owner_return_type(&owner)),
            typed_trees::types::TypeReferenceNode::Unit
        ) {
            None
        } else {
            let observed_type =
                crate::expression_result_type_reference(program, scope.0, scope.1, expression)?;
            Some(application.runtime_result_presentation(
                program,
                &owner,
                call,
                expression,
                observed_type,
                applied_result_parameters,
                callable,
                parameters,
                &bindings,
            )?)
        };
        let result = if let Some(presentation) = result_presentation.as_ref() {
            let receiving_type =
                crate::expression_result_type_reference(program, scope.0, scope.1, expression)?;
            self.capture_sequence += 1;
            Some(ReservedNormalResult {
                type_reference: presentation.type_reference,
                receiving_type,
                occurrence: self.capture_sequence,
            })
        } else {
            None
        };
        drop(result_presentation);
        Some(PreparedNormalCall {
            expression,
            call,
            owner,
            application,
            parameters: parameter_scope,
            bindings: call_bindings::CapturedCallBindings {
                entry_bindings,
                bindings,
                post_storage,
            },
            actuals,
            callable,
            result,
        })
    }

    pub(super) fn prepare_normal_call_guarantees<'call>(
        &'call self,
        prepared: &'call PreparedNormalCall<'_, '_>,
    ) -> Option<PreparedSelectedGuarantees<'call>> {
        self.prepare_selected_guarantees(
            &prepared.owner,
            &prepared.bindings.entry_bindings,
            &prepared.bindings.bindings,
            &prepared.application.types,
            0,
        )
    }
}
