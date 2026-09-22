//! External leaf native shapes: the carriers an external conformance may
//! expose and the diagnostics for those it may not.

use crate::declarations::standard_declarations::is_core_vector;
use crate::declarations::traits::trait_definition_by_symbol;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::trait_definition::TraitDefinition;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// Default native leaves may only expose types whose public shape determines
/// every ABI fact. A source-selected `Calling<C>` policy is the explicit escape
/// hatch: its checked plan can publish a canonical descriptor representation.
/// Without one, private slice/text/vector carriers must stop at a checked
/// adapter rather than silently inheriting the compiler's storage layout.
pub(crate) fn validate_external_leaf_native_shapes(
    program: &TypedTrees,
    machine: &Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for conformance in program
        .machine_trait_conformances(machine)
        .iter()
        .filter(|conformance| {
            conformance.external_binding.is_some() || conformance.via_expression.is_valid()
        })
    {
        // Compiler intrinsics are not
        // foreign ABI leaves: they select a
        // compiler-owned lowering whose safe carrier semantics are already
        // part of the target plan. In particular, Console::read_line may
        // retain its checked mutable-slice surface while the lowering derives
        // the concrete owned destination's capacity and live-length write.
        if matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::ExternalRealization {
                mechanism: Some(language_semantics::ExternalBindingMechanism::CompilerIntrinsic,),
                ..
            }
        ) {
            continue;
        }
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            continue;
        };
        if boundary_has_explicit_calling_policy(program, trait_definition) {
            continue;
        }
        let Some(requirement) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|requirement| requirement.symbol == conformance.requirement_symbol)
        else {
            continue;
        };

        for parameter in program
            .state_signature_parameters(requirement)
            .iter()
            .filter(|parameter| !parameter.is_self)
        {
            if let Some(carrier) =
                private_native_carrier(program, parameter.type_reference, &mut Vec::new())
            {
                diagnostics.push(native_carrier_diagnostic(
                    program,
                    machine,
                    trait_definition,
                    requirement,
                    parameter.type_reference,
                    carrier,
                    "parameter",
                ));
            }
        }
        if requirement.return_type.is_valid()
            && let Some(carrier) =
                private_native_carrier(program, requirement.return_type, &mut Vec::new())
        {
            diagnostics.push(native_carrier_diagnostic(
                program,
                machine,
                trait_definition,
                requirement,
                requirement.return_type,
                carrier,
                "result",
            ));
        }
    }
}

/// The first ordinary `via` evaluation rung is deliberately narrow: one
/// receiverless direct invocation of one exact zero-argument producer. Psi
/// validates that invocation shape and exact producer selection; Omega later
/// validates the compiler-owned `Binding` return vocabulary and normalizes its
/// object-format locator.
pub(crate) fn validate_external_via_expression(
    program: &TypedTrees,
    machine: &Machine,
    conformance: &typed_trees::machine::TraitConformance,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expression = conformance.via_expression;
    if !expression.is_valid() {
        return;
    }
    let source_span = program.expression_table.source_span(expression);
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
    else {
        diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` must use one direct zero-argument machine call after `via`",
                machine.name
            ))
            .with_source_span(source_span),
        );
        return;
    };
    let has_value_arguments = !program
        .expression_table
        .expression_handles(call.arguments)
        .is_empty();
    let invalid_call_shape = call.receiver.is_valid()
        || has_value_arguments
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
        || !call.target_symbol.is_valid();
    if invalid_call_shape {
        diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` must use one exact receiverless zero-argument machine call after `via`",
                machine.name
            ))
            .with_source_span(source_span),
        );
        return;
    }

    let producers = program
        .machines()
        .iter()
        .filter_map(|producer| {
            program
                .machine_states(producer)
                .iter()
                .find(|state| state.symbol == call.target_symbol)
                .map(|state| (producer, state))
        })
        .collect::<Vec<_>>();
    let [(producer, state)] = producers.as_slice() else {
        diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` does not select one exact `via` producer machine",
                machine.name
            ))
            .with_source_span(source_span),
        );
        return;
    };
    let producer_leaf = producer
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default();
    let producer_entry = program
        .machine_states(producer)
        .iter()
        .find(|candidate| candidate.name.as_str() == producer_leaf)
        .or_else(|| program.machine_states(producer).first());
    if !producer.body_is_present
        || !producer.lifetime_parameters.is_empty()
        || !program.machine_type_parameters(producer).is_empty()
        || !program.state_parameters(state).is_empty()
        || producer_entry.is_none_or(|entry| entry.symbol != state.symbol)
    {
        diagnostics.push(
            Diagnostic::error(format!(
                "external realization `{}` selects `via` producer `{}`, but the first evaluated-binding rung requires its body-bearing, non-generic, zero-parameter entry state",
                machine.name, producer.name
            ))
            .with_source_span(source_span),
        );
    }
}

fn boundary_has_explicit_calling_policy(
    program: &TypedTrees,
    trait_definition: &TraitDefinition,
) -> bool {
    crate::declarations::standard_declarations::standard_calling_traits(program).is_some_and(
        |(calling, _)| {
            program
                .trait_requirements(trait_definition)
                .iter()
                .any(|requirement| requirement.symbol == calling.symbol)
        },
    )
}

#[derive(Debug, Clone, Copy)]
enum PrivateNativeCarrier {
    Slice,
    Text,
    Vector,
}

impl PrivateNativeCarrier {
    const fn label(self) -> &'static str {
        match self {
            Self::Slice => "safe slice",
            Self::Text => "text view or bounded-text carrier",
            Self::Vector => "vector carrier",
        }
    }
}

fn private_native_carrier(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut Vec<SymbolHandle>,
) -> Option<PrivateNativeCarrier> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            if program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| matches!(constraint, TypeConstraintNode::Domain(_)))
                && carrier_is_slice_or_fixed_array(program, *base_type)
            {
                return Some(PrivateNativeCarrier::Text);
            }
            private_native_carrier(program, *base_type, visiting)
        }
        TypeReferenceNode::Reference { referee, .. } => {
            private_native_carrier(program, *referee, visiting)
        }
        TypeReferenceNode::Slice { .. } => Some(PrivateNativeCarrier::Slice),
        TypeReferenceNode::FixedArray { element_type, .. } => {
            private_native_carrier(program, *element_type, visiting)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } => {
            if is_core_vector(program, *base_symbol) {
                return Some(PrivateNativeCarrier::Vector);
            }
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| private_native_carrier(program, *argument, visiting))
                .or_else(|| private_record_carrier(program, *base_symbol, visiting))
        }
        TypeReferenceNode::Named { symbol, .. } => {
            if is_core_vector(program, *symbol) {
                return Some(PrivateNativeCarrier::Vector);
            }
            private_record_carrier(program, *symbol, visiting)
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn private_record_carrier(
    program: &TypedTrees,
    symbol: SymbolHandle,
    visiting: &mut Vec<SymbolHandle>,
) -> Option<PrivateNativeCarrier> {
    if !symbol.is_valid() || visiting.contains(&symbol) {
        return None;
    }
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol)?;
    visiting.push(symbol);
    let carrier = program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        private_native_carrier(program, field.type_reference, visiting)
    });
    visiting.pop();
    carrier
}

fn carrier_is_slice_or_fixed_array(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. }
        | TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => carrier_is_slice_or_fixed_array(program, *referee),
        TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. } => true,
        _ => false,
    }
}

fn native_carrier_diagnostic(
    program: &TypedTrees,
    machine: &Machine,
    trait_definition: &TraitDefinition,
    requirement: &StateSignature,
    type_reference: TypeReferenceHandle,
    carrier: PrivateNativeCarrier,
    position: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "external leaf `{}` cannot use {} `{}` as a default-native {} for `{}::{}`; declare the foreign pointer/length/terminator or record shape explicitly in a checked adapter, or select a custom `Calling<C>` policy that publishes a canonical representation",
        machine.name,
        carrier.label(),
        program.display_type_reference(type_reference),
        position,
        trait_definition.name,
        requirement.name,
    ))
}
