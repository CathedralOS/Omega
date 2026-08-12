//! Runtime noninterference and fail-closed fences for occurrence-level
//! `[erased]` data fields.
//!
//! The executable slice supports transparent records and sums, plus closed
//! synthesized generic-record instances at explicitly typed local
//! initializers. The full semantic tree remains intact for proofs and
//! ownership; native lowering later strips erased literal fields from its
//! private runtime expression graph.

use psi_diagnostics::Diagnostic;
use psi_language_core::BindingRelevance;
use psi_language_semantics::{DataSupplyMode, MachineSupplyMode};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{DataDefinition, DataField, DataMember};
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableStructLiteral};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Context {
    Runtime,
    ErasedInitializer,
    Proof,
}

pub(crate) fn validate_relevance(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    let proof_only = psi_typed_trees::proof_only::classify(program);
    validate_supported_shapes(program, diagnostics);

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                match statement {
                    StatementNode::AssemblyFact(fact) => validate_expression(
                        program,
                        &proof_only,
                        machine,
                        state,
                        fact.expression,
                        Context::Proof,
                        diagnostics,
                    ),
                    StatementNode::Assignment(assignment) => {
                        validate_expression(
                            program,
                            &proof_only,
                            machine,
                            state,
                            assignment.target,
                            Context::Runtime,
                            diagnostics,
                        );
                        validate_expression(
                            program,
                            &proof_only,
                            machine,
                            state,
                            assignment.value,
                            Context::Runtime,
                            diagnostics,
                        );
                    }
                    StatementNode::Call(call) => {
                        for argument in program.statement_table.expression_handles(call.arguments) {
                            validate_expression(
                                program,
                                &proof_only,
                                machine,
                                state,
                                *argument,
                                Context::Runtime,
                                diagnostics,
                            );
                        }
                    }
                    StatementNode::Expression(expression) => validate_expression(
                        program,
                        &proof_only,
                        machine,
                        state,
                        *expression,
                        Context::Runtime,
                        diagnostics,
                    ),
                    StatementNode::LocalData(local) => validate_expression(
                        program,
                        &proof_only,
                        machine,
                        state,
                        local.initial_value,
                        Context::Runtime,
                        diagnostics,
                    ),
                    StatementNode::Transition(transition) => {
                        if let TransitionGuardNode::When(guard) = transition.guard {
                            validate_expression(
                                program,
                                &proof_only,
                                machine,
                                state,
                                guard,
                                Context::Runtime,
                                diagnostics,
                            );
                        }
                        for target in [transition.target, transition.continuation] {
                            if !target.is_valid() {
                                continue;
                            }
                            match program.statement_table.transition_target(target) {
                                TransitionTargetNode::Named { arguments, .. } => {
                                    for argument in
                                        program.statement_table.expression_handles(*arguments)
                                    {
                                        validate_expression(
                                            program,
                                            &proof_only,
                                            machine,
                                            state,
                                            *argument,
                                            Context::Runtime,
                                            diagnostics,
                                        );
                                    }
                                }
                                TransitionTargetNode::Value(value) => validate_expression(
                                    program,
                                    &proof_only,
                                    machine,
                                    state,
                                    *value,
                                    Context::Runtime,
                                    diagnostics,
                                ),
                                TransitionTargetNode::SelfTarget
                                | TransitionTargetNode::Terminal => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn validate_supported_shapes(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for definition in program.data_definitions() {
        let erased = erased_fields(program, definition);
        if erased.is_empty() {
            continue;
        }
        let field_names = erased
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if definition.supply_mode == DataSupplyMode::BoundaryOpaque {
            diagnostics.push(unsupported(
                definition,
                &field_names,
                "boundary-opaque ABI data",
            ));
        }
        if program
            .plan_laid_layouts
            .iter()
            .any(|plan| plan.data_name == definition.name.as_str())
        {
            diagnostics.push(unsupported(definition, &field_names, "plan-laid data"));
        }
        if program
            .placed_view_plans
            .iter()
            .any(|plan| plan.data_name == definition.name.as_str())
        {
            diagnostics.push(unsupported(definition, &field_names, "placed-view data"));
        }
        if program
            .wire_schemas()
            .iter()
            .any(|schema| schema.name == definition.name)
        {
            diagnostics.push(unsupported(definition, &field_names, "wire data"));
        }
        if program
            .machines()
            .iter()
            .any(|machine| machine.attached_data.as_ref() == Some(&definition.name))
        {
            diagnostics.push(unsupported(
                definition,
                &field_names,
                "data with attached machines",
            ));
        }
    }

    validate_unresolved_erased_generic_uses(program, diagnostics);

    for schema in program.wire_schemas() {
        for member in program.wire_members(schema.members) {
            let psi_typed_trees::wire::WireMember::Field(field) = member else {
                continue;
            };
            if type_mentions_erased_record(program, field.type_reference) {
                diagnostics.push(Diagnostic::error(format!(
                    "wire data `{}` field `{}` mentions data with `[erased]` fields, but erased-stripped wire classification is not implemented yet",
                    schema.name, field.name
                )));
            }
        }
    }

    for machine in program.machines().iter().filter(|machine| {
        matches!(
            machine.supply_mode,
            MachineSupplyMode::Boundary | MachineSupplyMode::ExternalRealization { .. }
        )
    }) {
        for state in program.machine_states(machine) {
            let mentions_erased =
                program.state_parameters(state).iter().any(|parameter| {
                    type_mentions_erased_record(program, parameter.type_reference)
                }) || type_mentions_erased_record(program, state.return_type);
            if mentions_erased {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary ABI state `{}::{}` mentions data with `[erased]` fields, but erased-stripped ABI classification is not implemented yet",
                    machine.name, state.name
                )));
            }
        }
    }
}

fn validate_unresolved_erased_generic_uses(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for definition in program
        .data_definitions()
        .iter()
        .filter(|definition| definition.type_parameters.is_empty())
    {
        for member in program.data_members(definition) {
            let fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => program.data_payload_fields(variant),
            };
            for field in fields {
                if let Some(base) = unresolved_erased_generic_base(program, field.type_reference) {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{}` field `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized record instance",
                        definition.name, field.name
                    )));
                }
            }
        }
    }

    for machine in program
        .machines()
        .iter()
        .filter(|machine| program.machine_type_parameters(machine).is_empty())
    {
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if let Some(base) =
                    unresolved_erased_generic_base(program, parameter.type_reference)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` parameter `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized record instance",
                        machine.name, state.name, parameter.name
                    )));
                }
            }
            if let Some(base) = unresolved_erased_generic_base(program, state.return_type) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}::{}` result uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized record instance",
                    machine.name, state.name
                )));
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                if let Some(base) = unresolved_erased_generic_base(program, local.type_reference) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}::{}` local `{}` uses unresolved erased generic data `{base}`; this slice requires a closed monomorphized record instance",
                        machine.name, state.name, local.name
                    )));
                }
            }
        }
    }
}

fn unsupported(definition: &DataDefinition, fields: &str, shape: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "data `{}` has erased field(s) `{fields}`, but erased-stripped runtime support for {shape} is not implemented yet",
        definition.name
    ))
}

fn validate_expression(
    program: &TypedTrees,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    context: Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if context == Context::Runtime {
                let mut reported = false;
                for symbol in program
                    .expression_table
                    .name_path_member_symbols(path.member_symbols)
                {
                    reported |= report_runtime_erased_field(program, *symbol, diagnostics);
                }
                let members = program.expression_table.name_path_members(path.members);
                if !reported && members.len() == 2 && members[0].as_str() == "self" {
                    report_runtime_erased_attached_field(
                        program,
                        machine,
                        members[1].as_str(),
                        diagnostics,
                    );
                }
            }
        }
        ExpressionNode::Member(member) => {
            if context == Context::Runtime {
                if !report_runtime_erased_field(program, member.member_symbol, diagnostics) {
                    report_runtime_erased_member_by_receiver(
                        program,
                        machine,
                        state,
                        member.receiver,
                        member.member.as_str(),
                        member.case_variant.as_ref().map(|variant| variant.as_str()),
                        diagnostics,
                    );
                }
            }
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                member.receiver,
                context,
                diagnostics,
            );
        }
        ExpressionNode::StructLiteral(literal) => validate_struct_literal(
            program,
            proof_only,
            machine,
            state,
            literal,
            context,
            diagnostics,
        ),
        ExpressionNode::Call(call) => {
            if context == Context::ErasedInitializer
                && !call_targets_proof_machine(program, proof_only, call.target_symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "erased field initializer calls runtime machine `{}`; erased initialization cannot perform runtime effects or computation",
                    call.target
                )));
            }
            if call.receiver.is_valid() {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    call.receiver,
                    context,
                    diagnostics,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    *argument,
                    context,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Atomic(atomic) => {
            if context == Context::ErasedInitializer {
                diagnostics.push(Diagnostic::error(
                    "erased field initializer cannot perform an atomic runtime operation",
                ));
            }
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                atomic.value,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                atomic.result,
                context,
                diagnostics,
            );
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                validate_expression(
                    program,
                    proof_only,
                    machine,
                    state,
                    *value,
                    context,
                    diagnostics,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                binary.left,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                binary.right,
                context,
                diagnostics,
            );
        }
        ExpressionNode::Cast(cast) => validate_expression(
            program,
            proof_only,
            machine,
            state,
            cast.value,
            context,
            diagnostics,
        ),
        ExpressionNode::Indexed(indexed) => {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                indexed.collection,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                indexed.index,
                context,
                diagnostics,
            );
        }
        ExpressionNode::Mutable(inner) => validate_expression(
            program,
            proof_only,
            machine,
            state,
            *inner,
            context,
            diagnostics,
        ),
        ExpressionNode::Range(range) => {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                range.start,
                context,
                diagnostics,
            );
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                range.end,
                context,
                diagnostics,
            );
        }
        ExpressionNode::Unary(unary) => validate_expression(
            program,
            proof_only,
            machine,
            state,
            unary.operand,
            context,
            diagnostics,
        ),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn validate_struct_literal(
    program: &TypedTrees,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
    machine: &Machine,
    state: &State,
    literal: &TableStructLiteral,
    context: Context,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == literal.type_name)
    else {
        for field in program.expression_table.struct_fields(literal.fields) {
            validate_expression(
                program,
                proof_only,
                machine,
                state,
                field.value,
                context,
                diagnostics,
            );
        }
        return;
    };
    let unsupported_bare_generic =
        !definition.type_parameters.is_empty() && !erased_fields(program, definition).is_empty();
    if unsupported_bare_generic {
        diagnostics.push(Diagnostic::error(format!(
            "construction of erased generic data `{}` is unsupported in this context; use a closed generic record in an explicitly typed local initializer",
            definition.name
        )));
    }
    let declared_erased = literal_fields(program, definition, literal.case_name.as_ref())
        .filter(|field| field.relevance.is_erased())
        .collect::<Vec<_>>();
    let authored = program.expression_table.struct_fields(literal.fields);
    for erased in &declared_erased {
        if !unsupported_bare_generic && !authored.iter().any(|field| field.name == erased.name) {
            diagnostics.push(Diagnostic::error(format!(
                "construction of `{}` omits erased field `{}`; supply an explicit proof term because no unique accessible nullary constructor determines this binding",
                definition.name, erased.name
            )));
        }
    }
    for field in authored {
        let field_context = if declared_erased
            .iter()
            .any(|declared| declared.name == field.name)
        {
            Context::ErasedInitializer
        } else {
            context
        };
        validate_expression(
            program,
            proof_only,
            machine,
            state,
            field.value,
            field_context,
            diagnostics,
        );
    }
}

fn report_runtime_erased_field(
    program: &TypedTrees,
    symbol: SymbolHandle,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if !symbol.is_valid() {
        return false;
    }
    if let Some(field) = field_by_symbol(program, symbol)
        && field.relevance == BindingRelevance::Erased
    {
        diagnostics.push(Diagnostic::error(format!(
            "erased field `{}` has no runtime value, address, read, write, or cleanup; it may be used only by proofs or another erased binding",
            field.name
        )));
        return true;
    }
    false
}

fn report_runtime_erased_attached_field(
    program: &TypedTrees,
    machine: &Machine,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(attached) = machine.attached_data.as_ref() else {
        return;
    };
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name == *attached)
    else {
        return;
    };
    if let Some(field) = program.data_members(definition).iter().find_map(|member| {
        let DataMember::Field(field) = member else {
            return None;
        };
        (field.name.as_str() == name).then_some(field)
    }) {
        report_runtime_erased_field(program, field.symbol, diagnostics);
    }
}

fn report_runtime_erased_member_by_receiver(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: ExpressionHandle,
    name: &str,
    case_variant: Option<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let receiver_type = crate::places::declared_place_type(program, machine, Some(state), receiver)
        .or_else(|| {
            crate::places::declared_indexed_projection_type(program, machine, Some(state), receiver)
        });
    let Some(definition) = receiver_type.and_then(|type_reference| {
        crate::places::data_definition_for_type(program, type_reference)
    }) else {
        return;
    };
    let field = if let Some(case_variant) = case_variant {
        program.data_members(definition).iter().find_map(|member| {
            let DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.name.as_str() == case_variant).then(|| {
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name.as_str() == name)
            })?
        })
    } else {
        let common = program.data_members(definition).iter().find_map(|member| {
            let DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == name).then_some(field)
        });
        common.or_else(|| {
            program.data_members(definition).iter().find_map(|member| {
                let DataMember::Variant(variant) = member else {
                    return None;
                };
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name.as_str() == name && field.relevance.is_erased())
            })
        })
    };
    if let Some(field) = field {
        report_runtime_erased_field(program, field.symbol, diagnostics);
    }
}

fn literal_fields<'program>(
    program: &'program TypedTrees,
    definition: &'program DataDefinition,
    case_name: Option<&psi_typed_trees::name::Identifier>,
) -> impl Iterator<Item = &'program DataField> {
    let mut fields = Vec::new();
    for member in program.data_members(definition) {
        match member {
            DataMember::Field(field) => fields.push(field),
            DataMember::Variant(variant)
                if case_name.is_some_and(|case_name| *case_name == variant.name) =>
            {
                fields.extend(program.data_payload_fields(variant));
            }
            DataMember::Variant(_) => {}
        }
    }
    fields.into_iter()
}

fn call_targets_proof_machine(
    program: &TypedTrees,
    proof_only: &psi_typed_trees::proof_only::ProofOnlyClassification,
    target: SymbolHandle,
) -> bool {
    program.machines().iter().any(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target)
            && proof_only.is_proof_machine(program, machine)
    })
}

fn erased_fields<'program>(
    program: &'program TypedTrees,
    definition: &'program DataDefinition,
) -> Vec<&'program DataField> {
    let mut fields = Vec::new();
    for member in program.data_members(definition) {
        match member {
            DataMember::Field(field) if field.relevance == BindingRelevance::Erased => {
                fields.push(field)
            }
            DataMember::Variant(variant) => fields.extend(
                program
                    .data_payload_fields(variant)
                    .iter()
                    .filter(|field| field.relevance == BindingRelevance::Erased),
            ),
            DataMember::Field(_) => {}
        }
    }
    fields
}

fn field_by_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<&DataField> {
    program.data_definitions().iter().find_map(|definition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                DataMember::Field(field) if field.symbol == symbol => Some(field),
                DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.symbol == symbol),
                _ => None,
            })
    })
}

fn type_mentions_erased_record(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    type_mentions_erased_record_inner(program, handle, &mut Vec::new())
}

fn unresolved_erased_generic_base(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> Option<String> {
    if !handle.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            unresolved_erased_generic_base(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            unresolved_erased_generic_base(program, *base_type)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            unresolved_erased_generic_base(program, *element_type)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            let base = program.data_definitions().iter().find(|definition| {
                if base_symbol.is_valid() {
                    definition.symbol == *base_symbol
                } else {
                    definition.name == *base_name
                }
            });
            if base.is_some_and(|definition| {
                !definition.type_parameters.is_empty()
                    && !erased_fields(program, definition).is_empty()
            }) {
                return Some(base_name.as_str().to_owned());
            }
            program
                .type_reference_table
                .type_reference_handles(*arguments)
                .iter()
                .find_map(|argument| unresolved_erased_generic_base(program, *argument))
        }
        TypeReferenceNode::Named { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => None,
    }
}

fn type_mentions_erased_record_inner(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
    visiting: &mut Vec<u32>,
) -> bool {
    if !handle.is_valid() {
        return false;
    }
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Named { symbol, name } => program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *symbol || definition.name == *name)
            .is_some_and(|definition| data_contains_erased_record(program, definition, visiting)),
        TypeReferenceNode::Reference { referee, .. } => {
            type_mentions_erased_record_inner(program, *referee, visiting)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_mentions_erased_record_inner(program, *base_type, visiting)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            type_mentions_erased_record_inner(program, *element_type, visiting)
        }
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            arguments,
            ..
        } => {
            program
                .data_definitions()
                .iter()
                .find(|definition| {
                    definition.symbol == *base_symbol || definition.name == *base_name
                })
                .is_some_and(|definition| {
                    data_contains_erased_record(program, definition, visiting)
                })
                || program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| type_mentions_erased_record_inner(program, *argument, visiting))
        }
        TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Unit => false,
    }
}

fn data_contains_erased_record(
    program: &TypedTrees,
    definition: &DataDefinition,
    visiting: &mut Vec<u32>,
) -> bool {
    if !erased_fields(program, definition).is_empty() {
        return true;
    }
    let identity = definition.symbol.arena_index();
    if visiting.contains(&identity) {
        return false;
    }
    visiting.push(identity);
    let contains = program.data_members(definition).iter().any(|member| {
        let fields = match member {
            DataMember::Field(field) => std::slice::from_ref(field),
            DataMember::Variant(variant) => program.data_payload_fields(variant),
        };
        fields
            .iter()
            .any(|field| type_mentions_erased_record_inner(program, field.type_reference, visiting))
    });
    visiting.pop();
    contains
}
