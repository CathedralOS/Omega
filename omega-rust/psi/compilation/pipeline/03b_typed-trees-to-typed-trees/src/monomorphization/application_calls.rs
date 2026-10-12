//! Call substitution from authentic retained static application rows.
//!
//! Public requirement facts and executable bodies select the same declaration
//! and evidence rows. Ordinary application selection owns runtime subjects;
//! no candidate or private implementation is required
//! to apply the public header's exact static operands.

use super::application_arguments::{
    forwarded_static_argument_rewrites, substitute_forwarded_machine_arguments,
};
use super::application_evidence::{
    EvidenceArgumentRewrite, EvidenceRequirementRewrite, evidence_argument_rewrites,
    evidence_requirement_rewrites, rewrite_index_algebra,
};
use super::body_rewriting::insert_subject_name;
use super::selection::state_by_symbol;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::machine::GenericConformanceBound;
use typed_trees::name::Identifier;
use typed_trees::static_applications::{StaticMachineBinding, StaticMachineBindingValue};

/// Invocation-owned substitution scratch, borrowed while copied occurrences
/// are rewritten. Exact bindings and original declaration bounds own selection.
pub(crate) struct ApplicationCallRewrites {
    pub(crate) machine_parameters: Vec<SymbolHandle>,
    pub(crate) target_rewrites: Vec<(SymbolHandle, SymbolHandle, Identifier)>,
    pub(crate) selected_carriers: Vec<(SymbolHandle, SymbolHandle, Identifier)>,
    pub(crate) evidence_target_rewrites: Vec<EvidenceRequirementRewrite>,
    pub(crate) argument_rewrites: Vec<(SymbolHandle, SymbolHandle, Identifier)>,
    pub(crate) evidence_rewrites: Vec<EvidenceArgumentRewrite>,
    pub(crate) static_argument_rewrites: Vec<(SymbolHandle, StaticMachineArgument)>,
}

impl ApplicationCallRewrites {
    pub(crate) fn prepare(
        declarations: &dyn validation::CheckingInput,
        bindings: &[StaticMachineBinding],
        bounds: &[GenericConformanceBound],
    ) -> Result<Self, Diagnostic> {
        let machine_rewrites = bindings
            .iter()
            .filter_map(|row| {
                let StaticMachineBindingValue::Machine(binding) = &row.value else {
                    return None;
                };
                Some((row.parameter, binding))
            })
            .map(|(parameter, binding)| {
                let name = binding
                    .path
                    .last()
                    .cloned()
                    .or_else(|| {
                        state_by_symbol(declarations, binding.symbol)
                            .map(|state| state.name.clone())
                    })
                    .ok_or_else(|| {
                        Diagnostic::error("static machine binding has no original callable entry")
                    })?;
                Ok((parameter, binding.symbol, name))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        // An attached declaration keeps its explicit Data namespace. Selecting
        // it does not convert the first authored operand into a receiver.
        let selected_carriers = machine_rewrites
            .iter()
            .filter_map(|(parameter, target, _)| {
                let machine = declarations.machines().iter().find(|machine| {
                    typed_trees::state::state_by_symbol(
                        declarations.machine_states(machine),
                        *target,
                    )
                    .is_some()
                })?;
                if !machine.attached_data_symbol.is_valid() {
                    return None;
                }
                Some((
                    *parameter,
                    machine.attached_data_symbol,
                    machine.attached_data.as_ref()?.clone(),
                ))
            })
            .collect();
        let machine_parameters = machine_rewrites
            .iter()
            .map(|(parameter, _, _)| *parameter)
            .collect();
        let evidence_target_rewrites =
            evidence_requirement_rewrites(declarations, bounds, bindings);
        let mut target_rewrites = machine_rewrites.clone();
        target_rewrites.extend(
            evidence_target_rewrites
                .iter()
                .map(|rewrite| (rewrite.placeholder, rewrite.target, rewrite.name.clone())),
        );
        let mut argument_rewrites = machine_rewrites;
        let evidence_rewrites = evidence_argument_rewrites(declarations, bindings);
        argument_rewrites.extend(evidence_rewrites.iter().map(|rewrite| {
            (
                rewrite.binder,
                rewrite.argument.symbol,
                rewrite.alias.clone(),
            )
        }));
        let static_argument_rewrites = forwarded_static_argument_rewrites(declarations, bindings);
        Ok(Self {
            machine_parameters,
            target_rewrites,
            selected_carriers,
            evidence_target_rewrites,
            argument_rewrites,
            evidence_rewrites,
            static_argument_rewrites,
        })
    }

    pub(crate) fn rewrite_expression(
        &self,
        program: &mut TypedTrees,
        handle: ExpressionHandle,
        state_symbols: &[(SymbolHandle, SymbolHandle)],
    ) {
        let Self {
            machine_parameters,
            target_rewrites,
            selected_carriers,
            evidence_target_rewrites,
            argument_rewrites,
            evidence_rewrites,
            static_argument_rewrites,
        } = self;
        rewrite_index_algebra(program, handle, evidence_rewrites, static_argument_rewrites);
        let evidence_dispatch = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) => evidence_target_rewrites
                .iter()
                .find(|rewrite| rewrite.placeholder == call.target_symbol),
            _ => None,
        };
        let selected_carrier = match program.expression_table.expression(handle) {
            ExpressionNode::Call(call) => selected_carriers
                .iter()
                .find(|(parameter, _, _)| *parameter == call.target_symbol)
                .map(|(_, symbol, name)| (*symbol, name.clone())),
            _ => None,
        }
        .map(|(symbol, name)| insert_subject_name(&mut program.expression_table, name, symbol));
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(handle) else {
            return;
        };
        if machine_parameters.contains(&call.target_symbol) {
            call.static_machine_parameter = call.target_symbol;
        }
        if let Some((_, target, name)) = target_rewrites
            .iter()
            .find(|(parameter, _, _)| *parameter == call.target_symbol)
        {
            call.target_symbol = *target;
            call.target = name.clone();
        }
        if let Some(receiver) = selected_carrier {
            call.receiver = receiver;
        }
        rewrite_application_target_and_arguments(
            &mut call.target_symbol,
            &mut call.static_requirement_dispatch,
            &mut call.machine_arguments,
            evidence_dispatch,
            static_argument_rewrites,
            argument_rewrites,
            state_symbols,
            || call.receiver = ExpressionHandle::invalid(),
        );
        let application = call.authored_machine_application;
        if application.is_valid() {
            substitute_forwarded_machine_arguments(
                program
                    .expression_table
                    .static_machine_application_arguments_mut(application),
                static_argument_rewrites,
                argument_rewrites,
            );
            substitute_forwarded_machine_arguments(
                program
                    .expression_table
                    .static_machine_application_selected_machines_mut(application),
                static_argument_rewrites,
                argument_rewrites,
            );
        }
    }
}

/// Substitute call operands while keeping application selection separate from
/// cloning local control flow. A copied call to a sibling state must select its
/// own complete tuple through the ordinary selection loop. Cloning it into the
/// current instance and erasing its statics would silently equate, for example,
/// a call of F<7> inside F<5> with a self call. Named state transitions retain
/// their cloned destinations; they do not introduce another application.
#[allow(clippy::too_many_arguments)]
pub(crate) fn rewrite_application_target_and_arguments(
    target_symbol: &mut SymbolHandle,
    static_requirement_dispatch: &mut Option<typed_trees::typed_trees::StaticRequirementDispatch>,
    machine_arguments: &mut Box<[typed_trees::expression::StaticMachineArgument]>,
    evidence_dispatch: Option<
        &crate::monomorphization::application_evidence::EvidenceRequirementRewrite,
    >,
    static_argument_rewrites: &[(SymbolHandle, typed_trees::expression::StaticMachineArgument)],
    argument_rewrites: &[(SymbolHandle, SymbolHandle, typed_trees::name::Identifier)],
    state_symbols: &[(SymbolHandle, SymbolHandle)],
    mut invalidate_receiver: impl FnMut(),
) {
    if let Some(rewrite) = evidence_dispatch {
        invalidate_receiver();
        *static_requirement_dispatch = Some(rewrite.dispatch.clone());
        *machine_arguments = rewrite.application_arguments.clone();
    }
    substitute_forwarded_machine_arguments(
        machine_arguments,
        static_argument_rewrites,
        argument_rewrites,
    );
    if let Some((authored, _)) = state_symbols
        .iter()
        .find(|(_, concrete)| *concrete == *target_symbol)
    {
        *target_symbol = *authored;
    }
}
