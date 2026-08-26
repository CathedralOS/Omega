use super::correspondence_certificate::{
    DirectLiftPreconditionProof, QuotientCorrespondenceEvidence,
    compose_lift_correspondence_certificate,
};
use super::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeContractFactLocation,
    RepresentativeContractOwner, RepresentativeRuntimeParameter, RepresentativeStaticApplication,
    RepresentativeStaticBinding, RepresentativeStaticBindingKind, RepresentativeTelescope,
    complete_single_state_result_flow, complete_state_forwarding_result_flow,
    derive_define_precondition_correspondence, derive_define_runtime_correspondence,
    derive_direct_lift_precondition_implication, derive_direct_lift_public_precondition_partition,
    derive_direct_lift_runtime_correspondence, derive_direct_terminal_plan,
    derive_exact_representative_static_application, derive_public_precondition_partition,
    derive_representative_precondition_partition, derive_representative_telescope,
    derive_selected_theorem_telescope, fallthrough_result_root, immutable_alias_fallthrough_root,
    pure_representative_effect, substituted_type_matches, unconditional_representative_termination,
};
use psi_arena::HandleSpan;
use psi_numerics::arithmetic::ArithmeticDomain;
use psi_numerics::literals::{
    FloatFormat, FloatLiteral, IntegerLanding, IntegerLiteral, LandedIntegerType,
};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{
    DataDefinition, MachineParameterContract, QuotientDefinition, TypeParameter, TypeParameterKind,
};
use psi_typed_trees::domain::{ProofFact, ProofMembershipFact};
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, QuotientOperationKind,
    QuotientOperationRequest, StaticMachineArgument, StaticSymbolApplication,
    TableBinaryExpression, TableCallExpression, TableNamePath,
};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::name::Identifier;
use psi_typed_trees::proposition::{
    PropositionApplication, PropositionBinder, PropositionBinderKind, PropositionDefinition,
};
use psi_typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableLocalData, TableTransition};
use psi_typed_trees::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use std::sync::Arc;

use super::theorem::SelectedTheoremTelescope;
use super::theorem_schema::{
    TheoremApplicationSide, TheoremContractFactLocation, TheoremContractOwner,
    TheoremParameterRole, derive_expected_theorem_schema,
};
use super::theorem_schema_verification::verify_selected_theorem_schema;

fn exact_public_location(
    proof: &DirectLiftPreconditionProof,
) -> RepresentativeContractFactLocation {
    let DirectLiftPreconditionProof::ExactMatch { public } = proof else {
        panic!("expected exact precondition match")
    };
    *public
}

fn symbol(index: u32) -> SymbolHandle {
    SymbolHandle::from_arena_index(index)
}

fn quotient_type(
    program: &mut TypedTrees,
    quotient_symbol: SymbolHandle,
    quotient_name: &'static str,
    relation_symbol: SymbolHandle,
    relation_name: &'static str,
) -> TypeReferenceHandle {
    let carrier_symbol = symbol(500);
    if !program
        .data_definitions()
        .iter()
        .any(|definition| definition.symbol == carrier_symbol)
    {
        program.push_data_definition(DataDefinition {
            symbol: carrier_symbol,
            name: Identifier::generated_static("Carrier"),
            ..Default::default()
        });
    }
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: carrier_symbol,
            name: Identifier::generated_static("Carrier"),
        });
    if !program
        .propositions()
        .iter()
        .any(|proposition| proposition.symbol == relation_symbol)
    {
        program.push_proposition(PropositionDefinition {
            symbol: relation_symbol,
            name: Identifier::generated_static(relation_name),
            ..Default::default()
        });
    }
    program.push_data_definition(DataDefinition {
        symbol: quotient_symbol,
        name: Identifier::generated_static(quotient_name),
        quotient: Some(QuotientDefinition {
            carrier,
            relation: vec![Identifier::generated_static(relation_name)],
            relation_symbol,
            equivalence: None,
        }),
        ..Default::default()
    });
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: quotient_symbol,
            name: Identifier::generated_static(quotient_name),
        })
}

fn quotient_type_over(
    program: &mut TypedTrees,
    quotient_symbol: SymbolHandle,
    quotient_name: &'static str,
    relation_symbol: SymbolHandle,
    relation_name: &'static str,
    carrier: TypeReferenceHandle,
) -> TypeReferenceHandle {
    program.push_proposition(PropositionDefinition {
        symbol: relation_symbol,
        name: Identifier::generated_static(relation_name),
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        symbol: quotient_symbol,
        name: Identifier::generated_static(quotient_name),
        quotient: Some(QuotientDefinition {
            carrier,
            relation: vec![Identifier::generated_static(relation_name)],
            relation_symbol,
            equivalence: None,
        }),
        ..Default::default()
    });
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: quotient_symbol,
            name: Identifier::generated_static(quotient_name),
        })
}

fn carrier_type(program: &mut TypedTrees) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(500),
            name: Identifier::generated_static("Carrier"),
        })
}

fn primitive_type(program: &mut TypedTrees, name: &'static str) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: SymbolHandle::invalid(),
            name: Identifier::generated_static(name),
        })
}

fn wrap_fixed_array_type(
    program: &mut TypedTrees,
    element_type: TypeReferenceHandle,
    width: usize,
) -> TypeReferenceHandle {
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(width),
        })
}

fn byte_slice_reference_type(
    program: &mut TypedTrees,
    access: psi_language_core::ReferenceAccess,
) -> TypeReferenceHandle {
    let element_type = primitive_type(program, "u8");
    let slice = program
        .type_reference_table
        .insert(TypeReferenceNode::Slice { element_type });
    program
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: slice,
            access,
            lifetime: None,
        })
}

fn bounded_byte_buffer_type(program: &mut TypedTrees, capacity: usize) -> TypeReferenceHandle {
    let element_type = primitive_type(program, "u8");
    let fixed_array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(capacity),
        });
    let constraints =
        program
            .type_reference_table
            .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                name: Identifier::generated_static("Utf8"),
                ..Default::default()
            })]);
    program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: fixed_array,
            constraints,
        })
}

fn fixed_byte_array_type(program: &mut TypedTrees, width: usize) -> TypeReferenceHandle {
    let element_type = primitive_type(program, "u8");
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(width),
        })
}

fn fixed_nested_byte_array_type(
    program: &mut TypedTrees,
    rows: usize,
    columns: usize,
) -> TypeReferenceHandle {
    let row_type = fixed_byte_array_type(program, columns);
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: row_type,
            length: FixedArrayLength::Literal(rows),
        })
}

fn fixed_boolean_array_type(program: &mut TypedTrees, width: usize) -> TypeReferenceHandle {
    let element_type = primitive_type(program, "bool");
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(width),
        })
}

fn fixed_nested_boolean_array_type(
    program: &mut TypedTrees,
    rows: usize,
    columns: usize,
) -> TypeReferenceHandle {
    let row_type = fixed_boolean_array_type(program, columns);
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: row_type,
            length: FixedArrayLength::Literal(rows),
        })
}

fn fixed_boolean_tensor3_type(
    program: &mut TypedTrees,
    planes: usize,
    rows: usize,
    columns: usize,
) -> TypeReferenceHandle {
    let plane_type = fixed_nested_boolean_array_type(program, rows, columns);
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: plane_type,
            length: FixedArrayLength::Literal(planes),
        })
}

fn fixed_integer_array_type(
    program: &mut TypedTrees,
    primitive: &'static str,
    width: usize,
) -> TypeReferenceHandle {
    let element_type = primitive_type(program, primitive);
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(width),
        })
}

fn fixed_nested_integer_array_type(
    program: &mut TypedTrees,
    primitive: &'static str,
    rows: usize,
    columns: usize,
) -> TypeReferenceHandle {
    let row_type = fixed_integer_array_type(program, primitive, columns);
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: row_type,
            length: FixedArrayLength::Literal(rows),
        })
}

fn fixed_float_array_type(
    program: &mut TypedTrees,
    primitive: &'static str,
    width: usize,
) -> TypeReferenceHandle {
    let element_type = primitive_type(program, primitive);
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(width),
        })
}

fn fixed_nested_float_array_type(
    program: &mut TypedTrees,
    primitive: &'static str,
    rows: usize,
    columns: usize,
) -> TypeReferenceHandle {
    let row_type = fixed_float_array_type(program, primitive, columns);
    program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: row_type,
            length: FixedArrayLength::Literal(rows),
        })
}

fn canonical_byte_array_literal(program: &mut TypedTrees, bytes: &[u8]) -> ExpressionHandle {
    let elements =
        bytes
            .iter()
            .map(|byte| {
                program.expression_table.insert(ExpressionNode::Integer(
                    IntegerLiteral::from_value(i64::from(*byte)),
                ))
            })
            .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements))
}

fn wrap_array_literal(
    program: &mut TypedTrees,
    elements: impl IntoIterator<Item = ExpressionHandle>,
) -> ExpressionHandle {
    let elements = program.expression_table.insert_expression_handles(elements);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements))
}

fn nested_canonical_byte_array_literal(
    program: &mut TypedTrees,
    rows: &[&[u8]],
) -> ExpressionHandle {
    let rows = rows
        .iter()
        .map(|row| canonical_byte_array_literal(program, row))
        .collect::<Vec<_>>();
    let rows = program.expression_table.insert_expression_handles(rows);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(rows))
}

fn boolean_array_literal(program: &mut TypedTrees, values: &[bool]) -> ExpressionHandle {
    let elements = values
        .iter()
        .map(|value| {
            program
                .expression_table
                .insert(ExpressionNode::Boolean(*value))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements))
}

fn nested_boolean_array_literal(program: &mut TypedTrees, rows: &[&[bool]]) -> ExpressionHandle {
    let rows = rows
        .iter()
        .map(|row| boolean_array_literal(program, row))
        .collect::<Vec<_>>();
    let rows = program.expression_table.insert_expression_handles(rows);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(rows))
}

fn boolean_tensor3_literal(
    program: &mut TypedTrees,
    planes: Vec<Vec<Vec<bool>>>,
) -> ExpressionHandle {
    let planes = planes
        .into_iter()
        .map(|plane| {
            let rows = plane.iter().map(|row| row.as_slice()).collect::<Vec<_>>();
            nested_boolean_array_literal(program, &rows)
        })
        .collect::<Vec<_>>();
    let planes = program.expression_table.insert_expression_handles(planes);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(planes))
}

fn integer_array_literal(
    program: &mut TypedTrees,
    values: impl IntoIterator<Item = IntegerLiteral>,
) -> ExpressionHandle {
    let elements = values
        .into_iter()
        .map(|value| {
            program
                .expression_table
                .insert(ExpressionNode::Integer(value))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements))
}

fn nested_integer_array_literal(
    program: &mut TypedTrees,
    rows: Vec<Vec<IntegerLiteral>>,
) -> ExpressionHandle {
    let rows = rows
        .into_iter()
        .map(|row| integer_array_literal(program, row))
        .collect::<Vec<_>>();
    let rows = program.expression_table.insert_expression_handles(rows);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(rows))
}

fn float_array_literal(
    program: &mut TypedTrees,
    values: impl IntoIterator<Item = FloatLiteral>,
) -> ExpressionHandle {
    let elements = values
        .into_iter()
        .map(|value| {
            program
                .expression_table
                .insert(ExpressionNode::Float(value))
        })
        .collect::<Vec<_>>();
    let elements = program.expression_table.insert_expression_handles(elements);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(elements))
}

fn nested_float_array_literal(
    program: &mut TypedTrees,
    rows: Vec<Vec<FloatLiteral>>,
) -> ExpressionHandle {
    let rows = rows
        .into_iter()
        .map(|row| float_array_literal(program, row))
        .collect::<Vec<_>>();
    let rows = program.expression_table.insert_expression_handles(rows);
    program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(rows))
}

fn named_argument(
    program: &mut TypedTrees,
    name: &'static str,
    name_symbol: SymbolHandle,
) -> psi_typed_trees::expression::ExpressionHandle {
    let mut members = HandleSpan::empty();
    program
        .expression_table
        .push_name_path_member(&mut members, Identifier::generated_static(name));
    program
        .expression_table
        .insert(ExpressionNode::Name(TableNamePath {
            members,
            head_symbol: name_symbol,
            symbol: name_symbol,
            ..Default::default()
        }))
}

fn binary_expression(
    program: &mut TypedTrees,
    left: ExpressionHandle,
    operator: BinaryOperator,
    right: ExpressionHandle,
) -> ExpressionHandle {
    program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left,
            operator,
            right,
        }))
}

fn call_with_arguments(
    arguments: HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
) -> TableCallExpression {
    TableCallExpression {
        receiver: ExpressionHandle::invalid(),
        target_symbol: SymbolHandle::invalid(),
        target: Identifier::generated_static("lift"),
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
    }
}

fn static_argument(name: &'static str) -> StaticMachineArgument {
    StaticMachineArgument {
        path: vec![Identifier::generated_static(name)].into_boxed_slice(),
        application: None,
        const_literal: None,
        evidence_projection: None,
        symbol: SymbolHandle::invalid(),
    }
}

fn request_with_representative(symbol: SymbolHandle) -> QuotientOperationRequest {
    let mut representative_operation = static_argument("representative");
    representative_operation.symbol = symbol;
    QuotientOperationRequest {
        kind: QuotientOperationKind::Lift,
        representative_operation,
        selected_theorem: static_argument("ExactRespect"),
    }
}

fn push_selected_theorem(program: &mut TypedTrees) -> StaticMachineArgument {
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut theorem = Machine {
        symbol: symbol(700),
        name: Identifier::generated_static("selected_theorem"),
        termination_plan: psi_language_semantics::MachineTerminationPlan {
            checked_summary: psi_language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            },
            ..Default::default()
        },
        ..Default::default()
    };
    program.push_machine_state(
        &mut theorem,
        State {
            symbol: symbol(701),
            name: Identifier::generated_static("prove"),
            return_type: unit,
            ..Default::default()
        },
    );
    program.push_machine(theorem);
    let mut selected = static_argument("selected_theorem");
    selected.symbol = symbol(701);
    selected
}

fn push_custom_selected_theorem(
    program: &mut TypedTrees,
    supply_mode: psi_language_semantics::MachineSupplyMode,
    body_is_present: bool,
    return_type: TypeReferenceHandle,
) -> StaticMachineArgument {
    let mut theorem = Machine {
        symbol: symbol(710),
        name: Identifier::generated_static("custom_theorem"),
        supply_mode,
        body_is_present,
        ..Default::default()
    };
    program.push_machine_state(
        &mut theorem,
        State {
            symbol: symbol(711),
            name: Identifier::generated_static("prove"),
            return_type,
            ..Default::default()
        },
    );
    program.push_machine(theorem);
    let mut selected = static_argument("custom_theorem");
    selected.symbol = symbol(711);
    selected
}

#[test]
fn selected_theorem_requires_a_bodyful_checked_machine() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.selected_theorem = push_custom_selected_theorem(
        &mut program,
        psi_language_semantics::MachineSupplyMode::Boundary,
        false,
        unit,
    );

    assert_eq!(
        derive_selected_theorem_telescope(&program, &request),
        Err(RelationPlanError::TheoremMustBeCheckedBody)
    );
}

#[test]
fn selected_theorem_requires_a_resultless_machine() {
    let mut program = TypedTrees::default();
    let result = carrier_type(&mut program);
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.selected_theorem = push_custom_selected_theorem(
        &mut program,
        psi_language_semantics::MachineSupplyMode::CheckedBody,
        true,
        result,
    );

    assert_eq!(
        derive_selected_theorem_telescope(&program, &request),
        Err(RelationPlanError::TheoremMustBeResultless)
    );
}

#[test]
fn selected_theorem_retains_the_exact_closed_static_application() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let selected_type_symbol = symbol(722);
    program.push_data_definition(DataDefinition {
        symbol: selected_type_symbol,
        name: Identifier::generated_static("SelectedType"),
        ..Default::default()
    });
    let mut theorem = Machine {
        symbol: symbol(720),
        name: Identifier::generated_static("generic_theorem"),
        ..Default::default()
    };
    program.push_machine_type_parameter(
        &mut theorem,
        TypeParameter {
            symbol: symbol(723),
            name: Identifier::generated_static("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        },
    );
    program.push_machine_state(
        &mut theorem,
        State {
            symbol: symbol(721),
            name: Identifier::generated_static("prove"),
            return_type: unit,
            ..Default::default()
        },
    );
    program.push_machine(theorem);

    let mut selected_type = static_argument("SelectedType");
    selected_type.symbol = selected_type_symbol;
    let mut selected_theorem = static_argument("generic_theorem");
    selected_theorem.symbol = symbol(721);
    selected_theorem.application = Some(Box::new(StaticSymbolApplication {
        lifetime_arguments: Box::default(),
        arguments: vec![selected_type].into_boxed_slice(),
    }));
    let mut request = request_with_representative(SymbolHandle::invalid());
    request.selected_theorem = selected_theorem;

    let theorem = derive_selected_theorem_telescope(&program, &request)
        .expect("one closed theorem application should derive");
    assert_eq!(theorem.machine_symbol, symbol(720));
    assert_eq!(theorem.state_symbol, symbol(721));
    assert_eq!(theorem.static_application.bindings.len(), 1);
    assert_eq!(
        theorem.static_application.bindings[0].parameter,
        symbol(723)
    );
    assert_eq!(
        theorem.static_application.bindings[0].argument.symbol,
        selected_type_symbol
    );
}

fn push_representative(
    program: &mut TypedTrees,
    parameters: &[(TypeReferenceHandle, bool, bool)],
    return_type: TypeReferenceHandle,
) -> QuotientOperationRequest {
    let mut machine = Machine {
        symbol: symbol(90),
        name: Identifier::generated_static("representative"),
        ..Default::default()
    };
    let mut state = State {
        symbol: symbol(91),
        name: Identifier::generated_static("entry"),
        return_type,
        ..Default::default()
    };
    for (position, (type_reference, is_self, is_const)) in parameters.iter().enumerate() {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: symbol(100 + u32::try_from(position).expect("test position")),
                name: Identifier::generated(format!("p{position}")),
                type_reference: *type_reference,
                is_self: *is_self,
                is_const: *is_const,
                ..Default::default()
            },
        );
    }
    program.push_machine_contract(&mut machine, Default::default());
    program.push_state_contract(&mut state, Default::default());
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut request = request_with_representative(symbol(91));
    request.selected_theorem = push_selected_theorem(program);
    request
}

fn push_generic_representative_application(program: &mut TypedTrees) -> QuotientOperationRequest {
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let type_symbol = symbol(600);
    program.push_data_definition(DataDefinition {
        symbol: type_symbol,
        name: Identifier::generated_static("StaticType"),
        ..Default::default()
    });

    let mut selected_machine = Machine {
        symbol: symbol(610),
        name: Identifier::generated_static("selected"),
        ..Default::default()
    };
    program.push_machine_state(
        &mut selected_machine,
        State {
            symbol: symbol(611),
            return_type: unit,
            ..Default::default()
        },
    );
    program.push_machine(selected_machine);

    let mut representative = Machine {
        symbol: symbol(620),
        name: Identifier::generated_static("generic_representative"),
        ..Default::default()
    };
    for parameter in [
        TypeParameter {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
            kind: TypeParameterKind::Type,
            ..Default::default()
        },
        TypeParameter {
            symbol: symbol(623),
            name: Identifier::generated_static("N"),
            kind: TypeParameterKind::Const {
                type_reference: unit,
            },
            ..Default::default()
        },
        TypeParameter {
            symbol: symbol(624),
            name: Identifier::generated_static("F"),
            kind: TypeParameterKind::Machine {
                contract: MachineParameterContract::default(),
            },
            ..Default::default()
        },
    ] {
        program.push_machine_type_parameter(&mut representative, parameter);
    }
    let representative_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
        });
    let mut representative_state = State {
        symbol: symbol(621),
        return_type: representative_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut representative_state,
        StateParameter {
            symbol: symbol(625),
            name: Identifier::generated_static("value"),
            type_reference: representative_type,
            ..Default::default()
        },
    );
    program.push_machine_state(&mut representative, representative_state);
    program.push_machine(representative);

    let mut type_argument = static_argument("StaticType");
    type_argument.symbol = type_symbol;
    let const_argument = StaticMachineArgument {
        path: Box::default(),
        application: None,
        const_literal: Some(Default::default()),
        evidence_projection: None,
        symbol: SymbolHandle::invalid(),
    };
    let mut machine_argument = static_argument("selected");
    machine_argument.symbol = symbol(611);
    let mut request = request_with_representative(symbol(621));
    request.representative_operation.application = Some(Box::new(StaticSymbolApplication {
        lifetime_arguments: Box::default(),
        arguments: vec![type_argument, const_argument, machine_argument].into_boxed_slice(),
    }));
    request.selected_theorem = push_selected_theorem(program);
    request
}

#[test]
fn direct_plan_retains_exact_input_and_result_quotient_identities() {
    let mut program = TypedTrees::default();
    let left_type = quotient_type(&mut program, symbol(1), "LeftQ", symbol(2), "LeftR");
    let right_type = quotient_type(&mut program, symbol(3), "RightQ", symbol(4), "RightR");
    let ordinary_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let left_symbol = symbol(5);
    let ordinary_symbol = symbol(6);
    let left = named_argument(&mut program, "left", left_symbol);
    let ordinary = named_argument(&mut program, "ordinary", ordinary_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([left, ordinary]);
    let call = call_with_arguments(arguments);
    let machine = Machine::default();
    let mut state = State {
        return_type: right_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: left_symbol,
            name: Identifier::generated_static("left"),
            type_reference: left_type,
            ..Default::default()
        },
    );
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: ordinary_symbol,
            name: Identifier::generated_static("ordinary"),
            type_reference: ordinary_type,
            ..Default::default()
        },
    );
    let representative_carrier = carrier_type(&mut program);
    let request = push_representative(
        &mut program,
        &[
            (representative_carrier, true, false),
            (ordinary_type, false, false),
            (ordinary_type, false, true),
        ],
        representative_carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &machine, &state, &call, &request)
        .expect("direct named operands and quotient result derive an exact plan");

    assert_eq!(plan.input_relations.len(), 2);
    let InputRelation::Quotient(left_relation) = plan.input_relations[0] else {
        panic!("quotient input must retain its exact relation");
    };
    assert_eq!(left_relation.quotient_type, left_type);
    assert_eq!(left_relation.quotient_symbol, symbol(1));
    assert_eq!(left_relation.relation_symbol, symbol(2));
    assert_eq!(
        plan.input_relations[1],
        InputRelation::ExactEquality(ordinary_type)
    );
    assert_eq!(plan.result_relation.quotient_type, right_type);
    assert_eq!(plan.result_relation.quotient_symbol, symbol(3));
    assert_eq!(plan.result_relation.relation_symbol, symbol(4));
    assert_eq!(plan.representative.machine_symbol, symbol(90));
    assert_eq!(plan.representative.state_symbol, symbol(91));
    assert_eq!(plan.selected_theorem.machine_symbol, symbol(700));
    assert_eq!(plan.selected_theorem.state_symbol, symbol(701));
    assert_eq!(
        plan.selected_theorem.static_application.bindings,
        Vec::new()
    );
    assert_eq!(
        plan.selected_theorem_termination,
        Some(super::SelectedTheoremTermination {
            machine_symbol: symbol(700),
            state_symbol: symbol(701),
        })
    );
    assert_eq!(
        plan.selected_theorem_purity,
        Some(super::SelectedTheoremPurity {
            machine_symbol: symbol(700),
            state_symbol: symbol(701),
        })
    );
    assert!(plan.selected_theorem_crash_free);
    assert_eq!(plan.representative.parameters.len(), 2);
    assert!(plan.representative.parameters[0].is_self);
    assert!(!plan.representative.parameters[1].is_self);
    assert_eq!(plan.representative.return_type, representative_carrier);
    assert_eq!(plan.representative.machine_contracts.count(), 1);
    assert_eq!(plan.representative.state_contracts.count(), 1);
}

#[test]
fn expected_theorem_schema_pairs_quotient_positions_and_shares_ordinary_positions() {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let ordinary = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(800),
        relation_symbol: symbol(801),
    };
    let result_relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(802),
        relation_symbol: symbol(803),
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(804),
        state_symbol: symbol(805),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(806),
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(807),
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };

    let schema = derive_expected_theorem_schema(
        &program,
        &[
            InputRelation::Quotient(relation),
            InputRelation::ExactEquality(ordinary),
        ],
        result_relation,
        &representative,
    )
    .expect("one quotient position and one ordinary position form an exact schema");

    assert_eq!(schema.parameters.len(), 3);
    assert_eq!(
        schema.parameters[0].role,
        TheoremParameterRole::QuotientLeft
    );
    assert_eq!(
        schema.parameters[1].role,
        TheoremParameterRole::QuotientRight
    );
    assert_eq!(schema.parameters[2].role, TheoremParameterRole::Shared);
    assert_eq!(schema.parameters[0].representative_position, 0);
    assert_eq!(schema.parameters[1].representative_position, 0);
    assert_eq!(schema.parameters[2].representative_position, 1);
    assert_eq!(schema.left_application.machine_symbol, symbol(804));
    assert_eq!(schema.left_application.state_symbol, symbol(805));
    assert_eq!(schema.left_application.arguments, vec![0, 2]);
    assert_eq!(schema.right_application.arguments, vec![1, 2]);
    assert_eq!(schema.relation_premises.len(), 1);
    assert_eq!(schema.relation_premises[0].relation, relation);
    assert_eq!(schema.relation_premises[0].left_parameter, 0);
    assert_eq!(schema.relation_premises[0].right_parameter, 1);
    assert_eq!(schema.result_relation, result_relation);
}

#[test]
fn expected_theorem_schema_identity_rejects_same_count_relation_and_mapping_drift() {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let ordinary = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(810),
        relation_symbol: symbol(811),
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(812),
        state_symbol: symbol(813),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(814),
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(815),
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let schema = derive_expected_theorem_schema(
        &program,
        &[
            InputRelation::Quotient(relation),
            InputRelation::ExactEquality(ordinary),
        ],
        relation,
        &representative,
    )
    .expect("baseline structural schema");

    let mut changed_relation = schema.clone();
    changed_relation.relation_premises[0]
        .relation
        .relation_symbol = symbol(816);
    assert_eq!(
        changed_relation.relation_premises.len(),
        schema.relation_premises.len()
    );
    assert_ne!(changed_relation, schema);

    let mut changed_mapping = schema.clone();
    changed_mapping.right_application.arguments.swap(0, 1);
    assert_eq!(
        changed_mapping.right_application.arguments.len(),
        schema.right_application.arguments.len()
    );
    assert_ne!(changed_mapping, schema);
}

#[test]
fn expected_theorem_schema_retains_each_representative_requires_for_both_calls() {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let first = named_argument(&mut program, "first", symbol(820));
    let second = named_argument(&mut program, "second", symbol(821));
    let machine_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(first)]);
    let state_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(first), ProofFact::Expression(second)]);
    let ignored_ensures = program
        .proof_facts
        .insert_many([ProofFact::Expression(second)]);
    let machine_contracts = program.signature_contracts.insert_many([
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: machine_facts,
            ..Default::default()
        },
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            facts: ignored_ensures,
            ..Default::default()
        },
    ]);
    let state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: state_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(822),
        state_symbol: symbol(823),
        parameters: vec![RepresentativeRuntimeParameter {
            symbol: symbol(820),
            type_reference: carrier,
            is_mutable: false,
            is_self: false,
        }],
        return_type: carrier,
        machine_contracts,
        state_contracts,
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(824),
        relation_symbol: symbol(825),
    };

    let schema = derive_expected_theorem_schema(
        &program,
        &[InputRelation::Quotient(relation)],
        relation,
        &representative,
    )
    .expect("all exact representative requires should enter both legality applications");

    assert_eq!(schema.legality_premises.len(), 6);
    assert_eq!(
        schema.legality_premises[0].fact,
        TheoremContractFactLocation {
            owner: TheoremContractOwner::Machine,
            contract_position: 0,
            fact_position: 0,
        }
    );
    assert_eq!(
        schema.legality_premises[0].application,
        TheoremApplicationSide::Left
    );
    assert_eq!(
        schema.legality_premises[1].application,
        TheoremApplicationSide::Right
    );
    assert_eq!(
        schema.legality_premises[4].fact,
        TheoremContractFactLocation {
            owner: TheoremContractOwner::State,
            contract_position: 0,
            fact_position: 1,
        }
    );
    assert_eq!(
        schema.legality_premises[5].application,
        TheoremApplicationSide::Right
    );
}

#[derive(Clone, Copy)]
enum TheoremSchemaMutation {
    Exact,
    ExtraPremise,
    WrongRelation,
    WrongLegality,
    RedirectedOperation,
    DuplicatedLeftApplication,
    OmittedRightApplication,
    ReboundSharedArgument,
    NamedEvidenceLane,
    ConstParameter,
    AttachedReceiver,
    ParameterTypeMismatch,
    UnexpectedContractKind,
    MissingConclusion,
}

fn selected_theorem_schema_fixture(
    mutation: TheoremSchemaMutation,
) -> (
    TypedTrees,
    RepresentativeTelescope,
    SelectedTheoremTelescope,
    super::theorem_schema::ExpectedTheoremSchema,
) {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let ordinary = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let representative_type_parameter = symbol(849);
    let theorem_type_parameter = symbol(848);
    let representative_carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: representative_type_parameter,
            name: Identifier::generated_static("RepresentativeCarrier"),
        });
    let theorem_carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: theorem_type_parameter,
            name: Identifier::generated_static("TheoremCarrier"),
        });
    let mut selected_carrier = static_argument("Carrier");
    selected_carrier.symbol = symbol(500);
    let relation_symbol = symbol(850);
    program.push_proposition(PropositionDefinition {
        symbol: relation_symbol,
        name: Identifier::generated_static("ExactRelation"),
        ..Default::default()
    });

    let representative_parameter = symbol(851);
    let representative_shared = symbol(852);
    let representative_legality = named_argument(
        &mut program,
        "representative_value",
        representative_parameter,
    );
    let representative_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(representative_legality)]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(853),
        state_symbol: symbol(854),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_parameter,
                type_reference: representative_carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_shared,
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: representative_carrier,
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: vec![RepresentativeStaticBinding {
                parameter: representative_type_parameter,
                kind: RepresentativeStaticBindingKind::Type,
                argument: selected_carrier.clone(),
            }],
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: carrier,
        quotient_symbol: symbol(855),
        relation_symbol,
    };
    let expected = derive_expected_theorem_schema(
        &program,
        &[
            InputRelation::Quotient(relation),
            InputRelation::ExactEquality(ordinary),
        ],
        relation,
        &representative,
    )
    .expect("closed direct theorem schema");

    let left_symbol = symbol(856);
    let right_symbol = symbol(857);
    let shared_symbol = symbol(858);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let shared = named_argument(&mut program, "shared", shared_symbol);
    let relation_arguments = program
        .expression_table
        .insert_expression_handles([left, right]);
    let relation_fact = ProofFact::Proposition(PropositionApplication {
        proposition: if matches!(mutation, TheoremSchemaMutation::WrongRelation) {
            symbol(859)
        } else {
            relation_symbol
        },
        name: Identifier::generated_static("ExactRelation"),
        binder_arguments: Box::default(),
        arguments: relation_arguments,
    });
    let right_legality = if matches!(mutation, TheoremSchemaMutation::WrongLegality) {
        left
    } else {
        right
    };
    let mut requires = vec![
        relation_fact,
        ProofFact::Expression(left),
        ProofFact::Expression(right_legality),
    ];
    if matches!(mutation, TheoremSchemaMutation::ExtraPremise) {
        requires.push(ProofFact::Expression(shared));
    }
    let requires = program.proof_facts.insert_many(requires);

    let representative_call = |program: &mut TypedTrees,
                               first: ExpressionHandle,
                               shared: ExpressionHandle,
                               redirected: bool| {
        let arguments = program
            .expression_table
            .insert_expression_handles([first, shared]);
        program
            .expression_table
            .insert(ExpressionNode::Call(TableCallExpression {
                receiver: ExpressionHandle::invalid(),
                target_symbol: if redirected {
                    symbol(860)
                } else {
                    representative.state_symbol
                },
                target: Identifier::generated_static("representative"),
                machine_arguments: vec![selected_carrier.clone()].into_boxed_slice(),
                quotient_operation: None,
                private_layout_operation: None,
                arguments,
                evidence_arguments: Box::default(),
                operational_acknowledgement: Default::default(),
            }))
    };
    let left_call = representative_call(
        &mut program,
        left,
        shared,
        matches!(mutation, TheoremSchemaMutation::RedirectedOperation),
    );
    let right_first = if matches!(mutation, TheoremSchemaMutation::DuplicatedLeftApplication) {
        left
    } else {
        right
    };
    let right_shared = if matches!(mutation, TheoremSchemaMutation::ReboundSharedArgument) {
        left
    } else {
        shared
    };
    let right_call = representative_call(&mut program, right_first, right_shared, false);
    let conclusion_arguments = program.expression_table.insert_expression_handles([
        left_call,
        if matches!(mutation, TheoremSchemaMutation::OmittedRightApplication) {
            right
        } else {
            right_call
        },
    ]);
    let conclusion_fact = ProofFact::Proposition(PropositionApplication {
        proposition: relation_symbol,
        name: Identifier::generated_static("ExactRelation"),
        binder_arguments: Box::default(),
        arguments: conclusion_arguments,
    });
    let conclusion = if matches!(mutation, TheoremSchemaMutation::MissingConclusion) {
        program.proof_facts.insert_many(std::iter::empty())
    } else {
        program.proof_facts.insert_many([conclusion_fact])
    };
    let theorem_machine_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        binding: matches!(mutation, TheoremSchemaMutation::NamedEvidenceLane)
            .then(|| Identifier::generated_static("dictionary")),
        facts: requires,
        ..Default::default()
    }]);
    let theorem_state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: if matches!(mutation, TheoremSchemaMutation::UnexpectedContractKind) {
            SignatureContractKind::Crashes {
                cause: psi_typed_trees::signature::CrashCause::Trap,
            }
        } else {
            SignatureContractKind::Ensures
        },
        facts: conclusion,
        ..Default::default()
    }]);
    let mut theorem_machine = Machine {
        symbol: symbol(861),
        contracts: theorem_machine_contracts,
        ..Default::default()
    };
    let mut theorem_state = State {
        symbol: symbol(862),
        return_type: ordinary,
        contracts: theorem_state_contracts,
        ..Default::default()
    };
    for (position, (parameter_symbol, mut type_reference)) in [
        (left_symbol, theorem_carrier),
        (right_symbol, theorem_carrier),
        (shared_symbol, ordinary),
    ]
    .into_iter()
    .enumerate()
    {
        if position == 2 && matches!(mutation, TheoremSchemaMutation::ParameterTypeMismatch) {
            type_reference = theorem_carrier;
        }
        program.push_state_parameter(
            &mut theorem_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(match position {
                    0 => "left",
                    1 => "right",
                    _ => "shared",
                }),
                type_reference,
                is_const: position == 0
                    && matches!(mutation, TheoremSchemaMutation::ConstParameter),
                is_self: position == 0
                    && matches!(mutation, TheoremSchemaMutation::AttachedReceiver),
                ..Default::default()
            },
        );
    }
    program.push_machine_state(&mut theorem_machine, theorem_state);
    program.push_machine(theorem_machine);
    let theorem = SelectedTheoremTelescope {
        machine_symbol: symbol(861),
        state_symbol: symbol(862),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: left_symbol,
                type_reference: theorem_carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: right_symbol,
                type_reference: theorem_carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: shared_symbol,
                type_reference: ordinary,
                is_mutable: false,
                is_self: false,
            },
        ],
        machine_contracts: theorem_machine_contracts,
        state_contracts: theorem_state_contracts,
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: vec![RepresentativeStaticBinding {
                parameter: theorem_type_parameter,
                kind: RepresentativeStaticBindingKind::Type,
                argument: selected_carrier,
            }],
        },
    };
    (program, representative, theorem, expected)
}

#[test]
fn selected_theorem_schema_verification_certifies_exact_fact_coordinates() {
    let (program, representative, theorem, expected) =
        selected_theorem_schema_fixture(TheoremSchemaMutation::Exact);
    let certificate =
        verify_selected_theorem_schema(&program, &representative, &theorem, &expected)
            .expect("the exact selected theorem schema must be certified");

    assert_eq!(certificate.theorem_machine_symbol, symbol(861));
    assert_eq!(certificate.theorem_state_symbol, symbol(862));
    assert_eq!(certificate.parameters.len(), 3);
    assert_eq!(certificate.relation_premises.len(), 1);
    assert_eq!(certificate.relation_premises[0].expected_position, 0);
    assert_eq!(certificate.legality_premises.len(), 2);
    assert_ne!(
        certificate.legality_premises[0].actual,
        certificate.legality_premises[1].actual,
    );
    assert_eq!(certificate.conclusion.owner, TheoremContractOwner::State);
}

#[test]
fn selected_theorem_schema_verification_rejects_extra_and_wrong_premises() {
    for (mutation, expected_error) in [
        (
            TheoremSchemaMutation::ExtraPremise,
            RelationPlanError::TheoremSchemaPremiseCountMismatch,
        ),
        (
            TheoremSchemaMutation::WrongRelation,
            RelationPlanError::TheoremSchemaRelationPremiseMismatch(0),
        ),
        (
            TheoremSchemaMutation::WrongLegality,
            RelationPlanError::TheoremSchemaLegalityPremiseMismatch(1),
        ),
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(expected_error),
        );
    }
}

#[test]
fn selected_theorem_schema_verification_rejects_application_mapping_drift() {
    for mutation in [
        TheoremSchemaMutation::RedirectedOperation,
        TheoremSchemaMutation::DuplicatedLeftApplication,
        TheoremSchemaMutation::OmittedRightApplication,
        TheoremSchemaMutation::ReboundSharedArgument,
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(RelationPlanError::TheoremSchemaConclusionMismatch),
        );
    }
}

#[test]
fn selected_theorem_schema_verification_rejects_runtime_evidence_and_const_parameters() {
    for (mutation, expected_error) in [
        (
            TheoremSchemaMutation::NamedEvidenceLane,
            RelationPlanError::TheoremSchemaNamedEvidenceLane,
        ),
        (
            TheoremSchemaMutation::ConstParameter,
            RelationPlanError::TheoremSchemaConstParameter(0),
        ),
        (
            TheoremSchemaMutation::AttachedReceiver,
            RelationPlanError::TheoremSchemaAttachedReceiver(0),
        ),
        (
            TheoremSchemaMutation::ParameterTypeMismatch,
            RelationPlanError::TheoremSchemaParameterTypeMismatch(2),
        ),
        (
            TheoremSchemaMutation::UnexpectedContractKind,
            RelationPlanError::TheoremSchemaUnexpectedContractKind,
        ),
        (
            TheoremSchemaMutation::MissingConclusion,
            RelationPlanError::TheoremSchemaConclusionCountMismatch,
        ),
    ] {
        let (program, representative, theorem, schema) = selected_theorem_schema_fixture(mutation);
        assert_eq!(
            verify_selected_theorem_schema(&program, &representative, &theorem, &schema),
            Err(expected_error),
        );
    }
}

struct ArithmeticImplicationFixture {
    program: TypedTrees,
    public_machine: Machine,
    public_state: State,
    representative: RepresentativeTelescope,
    public_partition: super::RepresentativePreconditionPartition,
    representative_partition: super::RepresentativePreconditionPartition,
    runtime: super::DirectLiftRuntimeCorrespondence,
    expected_theorem: super::theorem_schema::ExpectedTheoremSchema,
    verified_theorem: super::VerifiedTheoremSchema,
}

fn arithmetic_implication_fixture(
    build_facts: impl FnOnce(
        &mut TypedTrees,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
    ) -> (Vec<ProofFact>, ProofFact),
) -> ArithmeticImplicationFixture {
    let mut program = TypedTrees::default();
    let integer_type = primitive_type(&mut program, "i32");
    let quotient_symbol = symbol(1000);
    let relation_symbol = symbol(1001);
    let quotient = quotient_type_over(
        &mut program,
        quotient_symbol,
        "IntegerQ",
        relation_symbol,
        "IntegerR",
        integer_type,
    );
    let public_symbol = symbol(1002);
    let representative_symbol = symbol(1003);
    let literal_parameter_symbol = symbol(1004);
    let static_const_symbol = symbol(1005);
    let (public_facts, representative_fact) = build_facts(
        &mut program,
        public_symbol,
        representative_symbol,
        literal_parameter_symbol,
        static_const_symbol,
    );

    let public_facts = program.proof_facts.insert_many(public_facts);
    let mut public_machine = Machine {
        symbol: symbol(1006),
        ..Default::default()
    };
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: public_facts,
            ..Default::default()
        },
    );
    let mut public_state = State::default();
    program.push_state_parameter(
        &mut public_state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("public"),
            type_reference: quotient,
            ..Default::default()
        },
    );

    let representative_facts = program.proof_facts.insert_many([representative_fact]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(1007),
        state_symbol: symbol(1008),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_symbol,
                type_reference: integer_type,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: literal_parameter_symbol,
                type_reference: integer_type,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: integer_type,
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: vec![RepresentativeStaticBinding {
                parameter: static_const_symbol,
                kind: RepresentativeStaticBindingKind::Const,
                argument: StaticMachineArgument {
                    path: Box::default(),
                    application: None,
                    const_literal: Some(IntegerLiteral::from_value(1)),
                    evidence_projection: None,
                    symbol: SymbolHandle::invalid(),
                },
            }],
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: quotient,
        quotient_symbol,
        relation_symbol,
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::ExactEquality(integer_type),
    ];
    let runtime = super::DirectLiftRuntimeCorrespondence {
        positions: vec![
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_symbol),
                representative_parameter: representative_symbol,
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Integer {
                        spelling: "2".to_owned(),
                        landing: IntegerLanding {
                            landed_type: LandedIntegerType::I32,
                            domain: ArithmeticDomain::Exact,
                        },
                    },
                ),
                representative_parameter: literal_parameter_symbol,
            },
        ],
    };
    let public_partition = derive_direct_lift_public_precondition_partition(
        &program,
        &public_machine,
        &public_state,
        &input_relations,
        &runtime,
    )
    .expect("integer public Q partitions exactly");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("integer representative P partitions exactly");
    let expected_theorem =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("integer implication theorem schema");
    let verified_theorem = super::VerifiedTheoremSchema {
        theorem_machine_symbol: symbol(1009),
        theorem_state_symbol: symbol(1010),
        parameters: expected_theorem
            .parameters
            .iter()
            .enumerate()
            .map(|(expected_position, _)| {
                super::theorem_schema_verification::VerifiedTheoremParameter {
                    expected_position,
                    theorem_symbol: symbol(1020 + u32::try_from(expected_position).unwrap()),
                }
            })
            .collect(),
        relation_premises: expected_theorem
            .relation_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| super::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 10 + expected_position,
                    },
                },
            )
            .collect(),
        legality_premises: expected_theorem
            .legality_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| super::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 20 + expected_position,
                    },
                },
            )
            .collect(),
        conclusion: TheoremContractFactLocation {
            owner: TheoremContractOwner::State,
            contract_position: 0,
            fact_position: 0,
        },
    };
    ArithmeticImplicationFixture {
        program,
        public_machine,
        public_state,
        representative,
        public_partition,
        representative_partition,
        runtime,
        expected_theorem,
        verified_theorem,
    }
}

struct DirectLiftImplicationFixture {
    program: TypedTrees,
    public_machine: Machine,
    public_state: State,
    representative: RepresentativeTelescope,
    public_partition: super::RepresentativePreconditionPartition,
    representative_partition: super::RepresentativePreconditionPartition,
    runtime: super::DirectLiftRuntimeCorrespondence,
    expected_theorem: super::theorem_schema::ExpectedTheoremSchema,
    verified_theorem: super::VerifiedTheoremSchema,
}

fn baseline_arithmetic_implication_fixture() -> ArithmeticImplicationFixture {
    arithmetic_implication_fixture(
        |program, public_symbol, representative_symbol, literal_symbol, static_symbol| {
            let public_value = named_argument(program, "public", public_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let lower = binary_expression(program, public_value, BinaryOperator::Greater, two);
            let public_value = named_argument(program, "public", public_symbol);
            let ten = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(10)));
            let upper = binary_expression(program, public_value, BinaryOperator::LessOrEqual, ten);

            let representative = named_argument(program, "representative", representative_symbol);
            let static_value = named_argument(program, "K", static_symbol);
            let adjusted =
                binary_expression(program, representative, BinaryOperator::Add, static_value);
            let literal = named_argument(program, "literal", literal_symbol);
            let goal = binary_expression(program, adjusted, BinaryOperator::Greater, literal);
            (
                vec![ProofFact::Expression(lower), ProofFact::Expression(upper)],
                ProofFact::Expression(goal),
            )
        },
    )
}

#[test]
fn direct_lift_arithmetic_q_implies_p_retains_full_ordered_q_and_both_sides() {
    let fixture = baseline_arithmetic_implication_fixture();
    let implication = derive_direct_lift_precondition_implication(
        &fixture.program,
        &fixture.public_machine,
        &fixture.public_state,
        &fixture.representative,
        &fixture.public_partition,
        &fixture.representative_partition,
        &fixture.runtime,
        &fixture.expected_theorem,
        &fixture.verified_theorem,
    )
    .expect("x > 2 entails x + 1 > 2 after exact literal/static substitution");
    assert_eq!(implication.rows.len(), 2);
    for (row, side) in implication
        .rows
        .iter()
        .zip([TheoremApplicationSide::Left, TheoremApplicationSide::Right])
    {
        assert_eq!(row.application, side);
        let DirectLiftPreconditionProof::ArithmeticEntailment { premises } = &row.proof else {
            panic!("non-identical integer facts require strict arithmetic evidence")
        };
        assert_eq!(premises, &fixture.public_partition.dependent);
        assert_eq!(premises.len(), 2);
    }
    assert_ne!(implication.rows[0].theorem, implication.rows[1].theorem);

    let exact = direct_lift_implication_fixture(true);
    let exact_implication = derive_direct_lift_precondition_implication(
        &exact.program,
        &exact.public_machine,
        &exact.public_state,
        &exact.representative,
        &exact.public_partition,
        &exact.representative_partition,
        &exact.runtime,
        &exact.expected_theorem,
        &exact.verified_theorem,
    )
    .expect("exact matching remains the priority owner");
    assert!(matches!(
        exact_implication.rows[0].proof,
        DirectLiftPreconditionProof::ExactMatch { .. }
    ));
}

#[test]
fn direct_lift_arithmetic_implication_rejects_unknown_stronger_and_mixed_facts() {
    let stronger = arithmetic_implication_fixture(
        |program, public_symbol, representative_symbol, literal_symbol, static_symbol| {
            let public = named_argument(program, "public", public_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let q = binary_expression(program, public, BinaryOperator::Greater, two);
            let representative = named_argument(program, "representative", representative_symbol);
            let static_value = named_argument(program, "K", static_symbol);
            let left =
                binary_expression(program, representative, BinaryOperator::Add, static_value);
            let literal = named_argument(program, "literal", literal_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let right = binary_expression(program, literal, BinaryOperator::Add, two);
            let p = binary_expression(program, left, BinaryOperator::Greater, right);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &stronger.program,
            &stronger.public_machine,
            &stronger.public_state,
            &stronger.representative,
            &stronger.public_partition,
            &stronger.representative_partition,
            &stronger.runtime,
            &stronger.expected_theorem,
            &stronger.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let refuted = arithmetic_implication_fixture(
        |program, public_symbol, representative_symbol, literal_symbol, _| {
            let public = named_argument(program, "public", public_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let q = binary_expression(program, public, BinaryOperator::Greater, two);
            let representative = named_argument(program, "representative", representative_symbol);
            let literal = named_argument(program, "literal", literal_symbol);
            let p = binary_expression(
                program,
                representative,
                BinaryOperator::LessOrEqual,
                literal,
            );
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &refuted.program,
            &refuted.public_machine,
            &refuted.public_state,
            &refuted.representative,
            &refuted.public_partition,
            &refuted.representative_partition,
            &refuted.runtime,
            &refuted.expected_theorem,
            &refuted.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let mixed = arithmetic_implication_fixture(
        |program, public_symbol, representative_symbol, literal_symbol, _| {
            let public = named_argument(program, "public", public_symbol);
            let two = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
            let q = binary_expression(program, public, BinaryOperator::Greater, two);
            let member = named_argument(program, "public", public_symbol);
            let representative = named_argument(program, "representative", representative_symbol);
            let literal = named_argument(program, "literal", literal_symbol);
            let p = binary_expression(program, representative, BinaryOperator::Greater, literal);
            (
                vec![
                    ProofFact::Expression(q),
                    ProofFact::Membership(ProofMembershipFact {
                        value: member,
                        domain: HandleSpan::empty(),
                        domain_symbol: symbol(1040),
                    }),
                ],
                ProofFact::Expression(p),
            )
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &mixed.program,
            &mixed.public_machine,
            &mixed.public_state,
            &mixed.representative,
            &mixed.public_partition,
            &mixed.representative_partition,
            &mixed.runtime,
            &mixed.expected_theorem,
            &mixed.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn strict_arithmetic_implication_rejects_member_views_and_conflicting_bindings() {
    use crate::contract_entailment::{
        StrictArithmeticBindingValue, StrictArithmeticImplicationJudgment,
        StrictArithmeticSymbolBinding, strict_arithmetic_expression_implication,
    };

    let member_path =
        arithmetic_implication_fixture(|program, public_symbol, representative_symbol, _, _| {
            let public = named_argument(program, "public", public_symbol);
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let q = binary_expression(program, public, BinaryOperator::Greater, zero);
            let mut members = HandleSpan::empty();
            for name in ["representative", "member"] {
                program
                    .expression_table
                    .push_name_path_member(&mut members, Identifier::generated_static(name));
            }
            let member = program
                .expression_table
                .insert(ExpressionNode::Name(TableNamePath {
                    members,
                    head_symbol: representative_symbol,
                    symbol: representative_symbol,
                    ..Default::default()
                }));
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let p = binary_expression(program, member, BinaryOperator::Greater, zero);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        });
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &member_path.program,
            &member_path.public_machine,
            &member_path.public_state,
            &member_path.representative,
            &member_path.public_partition,
            &member_path.representative_partition,
            &member_path.runtime,
            &member_path.expected_theorem,
            &member_path.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let mut program = TypedTrees::default();
    let one_left = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let one_right = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
    let constant_goal = binary_expression(&mut program, one_left, BinaryOperator::Equal, one_right);
    let conflicting = [
        StrictArithmeticSymbolBinding {
            symbol: symbol(1050),
            value: StrictArithmeticBindingValue::Atom {
                identity: "$a".to_owned(),
                unsigned: false,
            },
        },
        StrictArithmeticSymbolBinding {
            symbol: symbol(1050),
            value: StrictArithmeticBindingValue::Atom {
                identity: "$b".to_owned(),
                unsigned: false,
            },
        },
    ];
    assert_eq!(
        strict_arithmetic_expression_implication(
            &program,
            &Machine::default(),
            &[],
            constant_goal,
            &conflicting,
        ),
        StrictArithmeticImplicationJudgment::Unknown,
    );
}

#[test]
fn direct_lift_arithmetic_implication_rejects_proof_views_float_and_domain_drift() {
    let proof_view =
        arithmetic_implication_fixture(|program, public_symbol, representative_symbol, _, _| {
            let public = named_argument(program, "public", public_symbol);
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let q = binary_expression(program, public, BinaryOperator::Greater, zero);
            let argument = named_argument(program, "representative", representative_symbol);
            let arguments = program
                .expression_table
                .insert_expression_handles([argument]);
            let mut call = call_with_arguments(arguments);
            call.target = Identifier::generated_static("Bag");
            let view = program.expression_table.insert(ExpressionNode::Call(call));
            let p = binary_expression(program, view, BinaryOperator::Equal, view);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        });
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &proof_view.program,
            &proof_view.public_machine,
            &proof_view.public_state,
            &proof_view.representative,
            &proof_view.public_partition,
            &proof_view.representative_partition,
            &proof_view.runtime,
            &proof_view.expected_theorem,
            &proof_view.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let float =
        arithmetic_implication_fixture(|program, public_symbol, representative_symbol, _, _| {
            let public = named_argument(program, "public", public_symbol);
            let zero = program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::zero()));
            let q = binary_expression(program, public, BinaryOperator::Greater, zero);
            let representative = named_argument(program, "representative", representative_symbol);
            let value = program
                .expression_table
                .insert(ExpressionNode::Float(FloatLiteral::from_f64(0.0)));
            let p = binary_expression(program, representative, BinaryOperator::Greater, value);
            (vec![ProofFact::Expression(q)], ProofFact::Expression(p))
        });
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &float.program,
            &float.public_machine,
            &float.public_state,
            &float.representative,
            &float.public_partition,
            &float.representative_partition,
            &float.runtime,
            &float.expected_theorem,
            &float.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let mut domain_drift = baseline_arithmetic_implication_fixture();
    let super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::Integer { landing, .. },
    ) = &mut domain_drift.runtime.positions[1].source
    else {
        panic!("integer literal fixture")
    };
    landing.domain = ArithmeticDomain::Wrapping;
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &domain_drift.program,
            &domain_drift.public_machine,
            &domain_drift.public_state,
            &domain_drift.representative,
            &domain_drift.public_partition,
            &domain_drift.representative_partition,
            &domain_drift.runtime,
            &domain_drift.expected_theorem,
            &domain_drift.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

fn direct_lift_implication_fixture(
    public_fact_uses_exact_symbol: bool,
) -> DirectLiftImplicationFixture {
    let (mut program, representative, theorem, expected_theorem) =
        selected_theorem_schema_fixture(TheoremSchemaMutation::Exact);
    let verified_theorem =
        verify_selected_theorem_schema(&program, &representative, &theorem, &expected_theorem)
            .expect("baseline theorem schema");
    let omitted_quotient_type = quotient_type(
        &mut program,
        symbol(874),
        "OmittedQ",
        symbol(875),
        "OmittedR",
    );
    let public_value_symbol = symbol(870);
    let public_shared_symbol = symbol(871);
    let omitted_quotient_symbol = symbol(873);
    let fact_symbol = if public_fact_uses_exact_symbol {
        public_value_symbol
    } else {
        symbol(872)
    };
    let public_value = named_argument(&mut program, "value", fact_symbol);
    let public_extra = {
        let left = named_argument(&mut program, "value", public_value_symbol);
        let right = named_argument(&mut program, "value", public_value_symbol);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            }))
    };
    let omitted_quotient = named_argument(&mut program, "omitted", omitted_quotient_symbol);
    let public_facts = program.proof_facts.insert_many([
        ProofFact::Expression(public_value),
        ProofFact::Expression(public_extra),
        ProofFact::Expression(omitted_quotient),
    ]);
    let mut public_machine = Machine::default();
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: public_facts,
            ..Default::default()
        },
    );
    let mut public_state = State::default();
    for (symbol, name, type_reference) in [
        (
            public_value_symbol,
            "value",
            representative.parameters[0].type_reference,
        ),
        (
            public_shared_symbol,
            "shared",
            representative.parameters[1].type_reference,
        ),
        (omitted_quotient_symbol, "omitted", omitted_quotient_type),
    ] {
        program.push_state_parameter(
            &mut public_state,
            StateParameter {
                symbol,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    let input_relations = [
        InputRelation::Quotient(expected_theorem.relation_premises[0].relation),
        InputRelation::ExactEquality(representative.parameters[1].type_reference),
    ];
    let runtime = super::DirectLiftRuntimeCorrespondence {
        positions: vec![
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_value_symbol),
                representative_parameter: representative.parameters[0].symbol,
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_shared_symbol),
                representative_parameter: representative.parameters[1].symbol,
            },
        ],
    };
    let public_partition = derive_direct_lift_public_precondition_partition(
        &program,
        &public_machine,
        &public_state,
        &input_relations,
        &runtime,
    )
    .expect("exact public fact identities");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("exact representative fact identities");
    DirectLiftImplicationFixture {
        program,
        public_machine,
        public_state,
        representative,
        public_partition,
        representative_partition,
        runtime,
        expected_theorem,
        verified_theorem,
    }
}

#[test]
fn direct_lift_q_implies_p_retains_both_exact_theorem_coordinates_and_allows_extra_q() {
    let fixture = direct_lift_implication_fixture(true);
    assert_eq!(fixture.public_partition.dependent.len(), 3);
    assert_eq!(fixture.representative_partition.dependent.len(), 1);
    let implication = derive_direct_lift_precondition_implication(
        &fixture.program,
        &fixture.public_machine,
        &fixture.public_state,
        &fixture.representative,
        &fixture.public_partition,
        &fixture.representative_partition,
        &fixture.runtime,
        &fixture.expected_theorem,
        &fixture.verified_theorem,
    )
    .expect("one matching Q fact may imply P while another Q fact remains extra");

    assert_eq!(implication.rows.len(), 2);
    assert_eq!(
        implication.rows[0].application,
        TheoremApplicationSide::Left
    );
    assert_eq!(
        implication.rows[1].application,
        TheoremApplicationSide::Right
    );
    assert_eq!(
        exact_public_location(&implication.rows[0].proof).fact_position,
        0
    );
    assert_eq!(implication.rows[0].representative.fact_position, 0);
    assert_eq!(implication.rows[0].theorem.fact_position, 1);
    assert_eq!(implication.rows[1].theorem.fact_position, 2);

    let certificate = compose_lift_correspondence_certificate(
        &Ok(fixture.verified_theorem.clone()),
        &fixture.runtime,
        &implication,
    )
    .expect("verified theorem plus exact implication composes");
    let QuotientCorrespondenceEvidence::DirectLift {
        runtime,
        precondition,
    } = certificate.evidence
    else {
        panic!("direct lift evidence")
    };
    assert_eq!(runtime, fixture.runtime);
    assert_eq!(precondition, implication);
}

#[test]
fn direct_lift_q_implies_p_rejects_missing_identity_and_theorem_coordinate_tamper() {
    let missing = direct_lift_implication_fixture(false);
    assert_eq!(missing.public_partition.dependent.len(), 2);
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &missing.program,
            &missing.public_machine,
            &missing.public_state,
            &missing.representative,
            &missing.public_partition,
            &missing.representative_partition,
            &missing.runtime,
            &missing.expected_theorem,
            &missing.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );

    let exact = direct_lift_implication_fixture(true);
    let mut tampered_theorem = exact.verified_theorem.clone();
    tampered_theorem.legality_premises.pop();
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &exact.program,
            &exact.public_machine,
            &exact.public_state,
            &exact.representative,
            &exact.public_partition,
            &exact.representative_partition,
            &exact.runtime,
            &exact.expected_theorem,
            &tampered_theorem,
        ),
        Err(RelationPlanError::DirectLiftTheoremLegalityMismatch),
    );
    assert!(
        compose_lift_correspondence_certificate(
            &Err(RelationPlanError::TheoremSchemaConclusionMismatch),
            &exact.runtime,
            &super::DirectLiftPreconditionImplication { rows: Vec::new() },
        )
        .is_none()
    );
}

#[test]
fn direct_lift_literal_stays_fixed_and_dependent_use_requires_an_exact_public_fact() {
    let fixture = direct_lift_implication_fixture(true);
    let literal = super::runtime_correspondence::ClosedLiftLiteral::Boolean(true);

    let mut fixed_literal = fixture.runtime.clone();
    fixed_literal.positions[1].source = super::DirectLiftArgumentSource::Literal(literal.clone());
    assert!(
        derive_direct_lift_precondition_implication(
            &fixture.program,
            &fixture.public_machine,
            &fixture.public_state,
            &fixture.representative,
            &fixture.public_partition,
            &fixture.representative_partition,
            &fixed_literal,
            &fixture.expected_theorem,
            &fixture.verified_theorem,
        )
        .is_ok(),
        "a literal-fed ordinary position remains a fixed call obligation"
    );

    let mut dependent_literal = fixture.runtime.clone();
    dependent_literal.positions[0].source = super::DirectLiftArgumentSource::Literal(literal);
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &fixture.program,
            &fixture.public_machine,
            &fixture.public_state,
            &fixture.representative,
            &fixture.public_partition,
            &fixture.representative_partition,
            &dependent_literal,
            &fixture.expected_theorem,
            &fixture.verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn direct_lift_literal_substitutes_exactly_inside_dependent_representative_p() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(930),
        "LiteralQ",
        symbol(931),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let i32_type = primitive_type(&mut program, "i32");
    let public_symbol = symbol(932);
    let representative_quotient = symbol(933);
    let representative_literal = symbol(934);
    let public_value = named_argument(&mut program, "public", public_symbol);
    let public_literal = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I32,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let public_fact =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: public_value,
                operator: BinaryOperator::Equal,
                right: public_literal,
            }));
    let public_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(public_fact)]);
    let mut public_machine = Machine::default();
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: public_facts,
            ..Default::default()
        },
    );
    let mut public_state = State::default();
    program.push_state_parameter(
        &mut public_state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("public"),
            type_reference: quotient,
            ..Default::default()
        },
    );

    let representative_value =
        named_argument(&mut program, "representative", representative_quotient);
    let representative_constant = named_argument(&mut program, "constant", representative_literal);
    let representative_fact =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: representative_value,
                operator: BinaryOperator::Equal,
                right: representative_constant,
            }));
    let representative_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(representative_fact)]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(935),
        state_symbol: symbol(936),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_quotient,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_literal,
                type_reference: i32_type,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: quotient,
        quotient_symbol: symbol(930),
        relation_symbol: symbol(931),
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::ExactEquality(i32_type),
    ];
    let runtime = super::DirectLiftRuntimeCorrespondence {
        positions: vec![
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_symbol),
                representative_parameter: representative_quotient,
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Integer {
                        spelling: "7".to_owned(),
                        landing: IntegerLanding {
                            landed_type: LandedIntegerType::I32,
                            domain: ArithmeticDomain::Exact,
                        },
                    },
                ),
                representative_parameter: representative_literal,
            },
        ],
    };
    let public_partition = derive_direct_lift_public_precondition_partition(
        &program,
        &public_machine,
        &public_state,
        &input_relations,
        &runtime,
    )
    .expect("mixed public Q partitions on its quotient value");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("mixed representative P partitions on its quotient value");
    assert_eq!(public_partition.dependent.len(), 1);
    assert_eq!(representative_partition.dependent.len(), 1);
    let expected =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("literal positions remain ordinary universal theorem positions");
    assert_eq!(expected.parameters.len(), 3);
    assert_eq!(expected.left_application.arguments, [0, 2]);
    assert_eq!(expected.right_application.arguments, [1, 2]);
    let verified = super::VerifiedTheoremSchema {
        theorem_machine_symbol: symbol(937),
        theorem_state_symbol: symbol(938),
        parameters: expected
            .parameters
            .iter()
            .enumerate()
            .map(|(expected_position, _)| {
                super::theorem_schema_verification::VerifiedTheoremParameter {
                    expected_position,
                    theorem_symbol: symbol(940 + u32::try_from(expected_position).unwrap()),
                }
            })
            .collect(),
        relation_premises: expected
            .relation_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| super::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 10 + expected_position,
                    },
                },
            )
            .collect(),
        legality_premises: expected
            .legality_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| super::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 20 + expected_position,
                    },
                },
            )
            .collect(),
        conclusion: TheoremContractFactLocation {
            owner: TheoremContractOwner::State,
            contract_position: 0,
            fact_position: 0,
        },
    };
    let implication = derive_direct_lift_precondition_implication(
        &program,
        &public_machine,
        &public_state,
        &representative,
        &public_partition,
        &representative_partition,
        &runtime,
        &expected,
        &verified,
    )
    .expect("Q(public, 7i32) exactly contains P(representative, literal-fed parameter)");
    assert_eq!(implication.rows.len(), 2);
    assert_eq!(
        implication.rows[0].application,
        TheoremApplicationSide::Left
    );
    assert_eq!(
        implication.rows[1].application,
        TheoremApplicationSide::Right
    );
    assert_ne!(implication.rows[0].theorem, implication.rows[1].theorem);

    let mut drifted = runtime.clone();
    drifted.positions[1].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::Integer {
            spelling: "8".to_owned(),
            landing: IntegerLanding {
                landed_type: LandedIntegerType::I32,
                domain: ArithmeticDomain::Exact,
            },
        },
    );
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &program,
            &public_machine,
            &public_state,
            &representative,
            &public_partition,
            &representative_partition,
            &drifted,
            &expected,
            &verified,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn proof_fact_literal_substitution_retains_value_landing_and_recursive_fact_shape() {
    use super::proof_fact_identity::{
        ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
    };

    let mut program = TypedTrees::default();
    let parameter = symbol(950);
    let parameter_value = named_argument(&mut program, "constant", parameter);
    let exact_landing = IntegerLanding {
        landed_type: LandedIntegerType::I32,
        domain: ArithmeticDomain::Exact,
    };
    let exact_integer = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(exact_landing),
    ));
    let wrapping_integer = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I32,
            domain: ArithmeticDomain::Wrapping,
        }),
    ));
    let no_values = Vec::new();
    let exact_value = vec![ProofValueSubstitution::integer(
        parameter,
        "7",
        exact_landing,
    )];
    let context = |values| ProofFactIdentityContext {
        values,
        static_bindings: &[],
    };
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(parameter_value),
        &ProofFact::Expression(exact_integer),
        context(&exact_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(exact_integer),
        &ProofFact::Expression(wrapping_integer),
        context(&no_values),
        context(&no_values),
    ));

    let proposition = symbol(951);
    let parameter_arguments = program
        .expression_table
        .insert_expression_handles([parameter_value]);
    let literal_arguments = program
        .expression_table
        .insert_expression_handles([exact_integer]);
    let proposition_fact = |arguments| {
        ProofFact::Proposition(PropositionApplication {
            proposition,
            name: Identifier::generated_static("ExactLiteral"),
            binder_arguments: Box::default(),
            arguments,
        })
    };
    assert!(proof_facts_match(
        &program,
        &proposition_fact(parameter_arguments),
        &proposition_fact(literal_arguments),
        context(&exact_value),
        context(&no_values),
    ));

    let domain_symbol = symbol(952);
    assert!(proof_facts_match(
        &program,
        &ProofFact::Membership(ProofMembershipFact {
            value: parameter_value,
            domain: HandleSpan::empty(),
            domain_symbol,
        }),
        &ProofFact::Membership(ProofMembershipFact {
            value: exact_integer,
            domain: HandleSpan::empty(),
            domain_symbol,
        }),
        context(&exact_value),
        context(&no_values),
    ));

    let boolean_parameter = symbol(953);
    let boolean_name = named_argument(&mut program, "flag", boolean_parameter);
    let true_literal = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let false_literal = program
        .expression_table
        .insert(ExpressionNode::Boolean(false));
    let true_value = vec![ProofValueSubstitution::boolean(boolean_parameter, true)];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_name),
        &ProofFact::Expression(true_literal),
        context(&true_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_name),
        &ProofFact::Expression(false_literal),
        context(&true_value),
        context(&no_values),
    ));

    let float_parameter = symbol(954);
    let float_name = named_argument(&mut program, "scale", float_parameter);
    let f32_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
    ));
    let f64_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
    ));
    let other_f32_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.5f32").expect("format-landed f32 literal"),
    ));
    let float_value = vec![ProofValueSubstitution::float(
        float_parameter,
        "1.25",
        FloatFormat::F32,
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(f32_literal),
        context(&float_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(f64_literal),
        context(&float_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(other_f32_literal),
        context(&float_value),
        context(&no_values),
    ));

    let string_parameter = symbol(955);
    let string_name = named_argument(&mut program, "label", string_parameter);
    let string_literal = program
        .expression_table
        .insert(ExpressionNode::String(Arc::from(&b"value"[..])));
    let other_string_literal = program
        .expression_table
        .insert(ExpressionNode::String(Arc::from(&b"other"[..])));
    let string_value = vec![ProofValueSubstitution::byte_string(
        string_parameter,
        b"value",
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(string_name),
        &ProofFact::Expression(string_literal),
        context(&string_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(string_name),
        &ProofFact::Expression(other_string_literal),
        context(&string_value),
        context(&no_values),
    ));

    let byte_array_parameter = symbol(956);
    let byte_array_name = named_argument(&mut program, "bytes", byte_array_parameter);
    let byte_array_literal = canonical_byte_array_literal(&mut program, b"value");
    let other_byte_array_literal = canonical_byte_array_literal(&mut program, b"other");
    let byte_array_value = vec![ProofValueSubstitution::fixed_byte_array(
        byte_array_parameter,
        b"value",
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(byte_array_name),
        &ProofFact::Expression(byte_array_literal),
        context(&byte_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(byte_array_name),
        &ProofFact::Expression(other_byte_array_literal),
        context(&byte_array_value),
        context(&no_values),
    ));

    let nested_byte_array_parameter = symbol(960);
    let nested_byte_array_name =
        named_argument(&mut program, "byte_rows", nested_byte_array_parameter);
    let exact_nested_byte_array_literal =
        nested_canonical_byte_array_literal(&mut program, &[&[1, 2], &[3, 4]]);
    let row_boundary_drifted_byte_array_literal =
        nested_canonical_byte_array_literal(&mut program, &[&[1], &[2, 3, 4]]);
    let flattened_byte_array_literal = canonical_byte_array_literal(&mut program, &[1, 2, 3, 4]);
    let nested_byte_array_value = vec![ProofValueSubstitution::nested_fixed_byte_array(
        nested_byte_array_parameter,
        &[Arc::from(&[1, 2][..]), Arc::from(&[3, 4][..])],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_byte_array_name),
        &ProofFact::Expression(exact_nested_byte_array_literal),
        context(&nested_byte_array_value),
        context(&no_values),
    ));
    assert!(
        !proof_facts_match(
            &program,
            &ProofFact::Expression(nested_byte_array_name),
            &ProofFact::Expression(row_boundary_drifted_byte_array_literal),
            context(&nested_byte_array_value),
            context(&no_values),
        ),
        "row-delimited traces retain byte-matrix boundaries"
    );
    assert!(
        !proof_facts_match(
            &program,
            &ProofFact::Expression(nested_byte_array_name),
            &ProofFact::Expression(flattened_byte_array_literal),
            context(&nested_byte_array_value),
            context(&no_values),
        ),
        "container-delimited traces distinguish nested and flat byte arrays"
    );

    let boolean_array_parameter = symbol(957);
    let boolean_array_name = named_argument(&mut program, "flags", boolean_array_parameter);
    let exact_boolean_array_literal = boolean_array_literal(&mut program, &[true, false]);
    let other_boolean_array_literal = boolean_array_literal(&mut program, &[false, true]);
    let boolean_array_value = vec![ProofValueSubstitution::boolean_array(
        boolean_array_parameter,
        &[true, false],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_array_name),
        &ProofFact::Expression(exact_boolean_array_literal),
        context(&boolean_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_array_name),
        &ProofFact::Expression(other_boolean_array_literal),
        context(&boolean_array_value),
        context(&no_values),
    ));

    let integer_array_parameter = symbol(958);
    let integer_array_name = named_argument(&mut program, "offsets", integer_array_parameter);
    let i16_landing = IntegerLanding {
        landed_type: LandedIntegerType::I16,
        domain: ArithmeticDomain::Exact,
    };
    let exact_integer_array_literal = integer_array_literal(
        &mut program,
        [
            IntegerLiteral::from_value(-1).with_landing(i16_landing),
            IntegerLiteral::from_value(7).with_landing(i16_landing),
        ],
    );
    let wrapping_integer_array_literal = integer_array_literal(
        &mut program,
        [
            IntegerLiteral::from_value(-1).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::I16,
                domain: ArithmeticDomain::Wrapping,
            }),
            IntegerLiteral::from_value(7).with_landing(i16_landing),
        ],
    );
    let integer_array_value = vec![ProofValueSubstitution::integer_array(
        integer_array_parameter,
        [
            ("-1".to_owned(), i16_landing),
            ("7".to_owned(), i16_landing),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(integer_array_name),
        &ProofFact::Expression(exact_integer_array_literal),
        context(&integer_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(integer_array_name),
        &ProofFact::Expression(wrapping_integer_array_literal),
        context(&integer_array_value),
        context(&no_values),
    ));

    let nested_integer_array_parameter = symbol(962);
    let nested_integer_array_name =
        named_argument(&mut program, "offset_rows", nested_integer_array_parameter);
    let landed_integer = |value| IntegerLiteral::from_value(value).with_landing(i16_landing);
    let exact_nested_integer_array_literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![landed_integer(-1), landed_integer(7)],
            vec![landed_integer(8), landed_integer(9)],
        ],
    );
    let row_boundary_drifted_integer_array_literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![landed_integer(-1)],
            vec![landed_integer(7), landed_integer(8), landed_integer(9)],
        ],
    );
    let flattened_integer_array_literal = integer_array_literal(
        &mut program,
        [
            landed_integer(-1),
            landed_integer(7),
            landed_integer(8),
            landed_integer(9),
        ],
    );
    let wrapping_nested_integer_array_literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![
                IntegerLiteral::from_value(-1).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::I16,
                    domain: ArithmeticDomain::Wrapping,
                }),
                landed_integer(7),
            ],
            vec![landed_integer(8), landed_integer(9)],
        ],
    );
    let nested_integer_array_value = vec![ProofValueSubstitution::nested_integer_array(
        nested_integer_array_parameter,
        &[
            Arc::from(vec![
                super::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "-1".to_owned(),
                    landing: i16_landing,
                },
                super::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "7".to_owned(),
                    landing: i16_landing,
                },
            ]),
            Arc::from(vec![
                super::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "8".to_owned(),
                    landing: i16_landing,
                },
                super::runtime_correspondence::ClosedIntegerArrayElement {
                    spelling: "9".to_owned(),
                    landing: i16_landing,
                },
            ]),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_integer_array_name),
        &ProofFact::Expression(exact_nested_integer_array_literal),
        context(&nested_integer_array_value),
        context(&no_values),
    ));
    for drifted in [
        row_boundary_drifted_integer_array_literal,
        flattened_integer_array_literal,
        wrapping_nested_integer_array_literal,
    ] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(nested_integer_array_name),
            &ProofFact::Expression(drifted),
            context(&nested_integer_array_value),
            context(&no_values),
        ));
    }

    let float_array_parameter = symbol(959);
    let float_array_name = named_argument(&mut program, "scales", float_array_parameter);
    let exact_float_array_literal = float_array_literal(
        &mut program,
        [
            FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
            FloatLiteral::parse("2.5f32").expect("format-landed f32 literal"),
        ],
    );
    let drifted_float_array_literal = float_array_literal(
        &mut program,
        [
            FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
            FloatLiteral::parse("2.5f32").expect("format-landed f32 literal"),
        ],
    );
    let float_array_value = vec![ProofValueSubstitution::float_array(
        float_array_parameter,
        [
            ("1.25".to_owned(), FloatFormat::F32),
            ("2.5".to_owned(), FloatFormat::F32),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(float_array_name),
        &ProofFact::Expression(exact_float_array_literal),
        context(&float_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_array_name),
        &ProofFact::Expression(drifted_float_array_literal),
        context(&float_array_value),
        context(&no_values),
    ));

    let nested_float_array_parameter = symbol(963);
    let nested_float_array_name =
        named_argument(&mut program, "scale_rows", nested_float_array_parameter);
    let f32_literal = |text| FloatLiteral::parse(text).expect("format-landed f32 literal");
    let exact_nested_float_array_literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![f32_literal("1.25f32"), f32_literal("2.5f32")],
            vec![f32_literal("3.75f32"), f32_literal("4.5f32")],
        ],
    );
    let row_drifted_float_array_literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![f32_literal("1.25f32")],
            vec![
                f32_literal("2.5f32"),
                f32_literal("3.75f32"),
                f32_literal("4.5f32"),
            ],
        ],
    );
    let flattened_float_array_literal = float_array_literal(
        &mut program,
        [
            f32_literal("1.25f32"),
            f32_literal("2.5f32"),
            f32_literal("3.75f32"),
            f32_literal("4.5f32"),
        ],
    );
    let format_drifted_nested_float_array_literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![
                FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
                f32_literal("2.5f32"),
            ],
            vec![f32_literal("3.75f32"), f32_literal("4.5f32")],
        ],
    );
    let float_element = |spelling: &str| super::runtime_correspondence::ClosedFloatArrayElement {
        spelling: spelling.to_owned(),
        landing: FloatFormat::F32,
    };
    let nested_float_array_value = vec![ProofValueSubstitution::nested_float_array(
        nested_float_array_parameter,
        &[
            Arc::from(vec![float_element("1.25"), float_element("2.5")]),
            Arc::from(vec![float_element("3.75"), float_element("4.5")]),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_float_array_name),
        &ProofFact::Expression(exact_nested_float_array_literal),
        context(&nested_float_array_value),
        context(&no_values),
    ));
    for drifted in [
        row_drifted_float_array_literal,
        flattened_float_array_literal,
        format_drifted_nested_float_array_literal,
    ] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(nested_float_array_name),
            &ProofFact::Expression(drifted),
            context(&nested_float_array_value),
            context(&no_values),
        ));
    }

    let nested_boolean_array_parameter = symbol(960);
    let nested_boolean_array_name =
        named_argument(&mut program, "flag_rows", nested_boolean_array_parameter);
    let exact_nested_boolean_array_literal =
        nested_boolean_array_literal(&mut program, &[&[true, false], &[false, true]]);
    let row_drifted_boolean_array_literal =
        nested_boolean_array_literal(&mut program, &[&[true, false], &[true, false]]);
    let flattened_boolean_array_literal =
        boolean_array_literal(&mut program, &[true, false, false, true]);
    let nested_boolean_array_value = vec![ProofValueSubstitution::nested_boolean_array(
        nested_boolean_array_parameter,
        &[Arc::from(&[true, false][..]), Arc::from(&[false, true][..])],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(nested_boolean_array_name),
        &ProofFact::Expression(exact_nested_boolean_array_literal),
        context(&nested_boolean_array_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(nested_boolean_array_name),
        &ProofFact::Expression(row_drifted_boolean_array_literal),
        context(&nested_boolean_array_value),
        context(&no_values),
    ));
    assert!(
        !proof_facts_match(
            &program,
            &ProofFact::Expression(nested_boolean_array_name),
            &ProofFact::Expression(flattened_boolean_array_literal),
            context(&nested_boolean_array_value),
            context(&no_values),
        ),
        "container-delimited traces distinguish nested and flat arrays with identical leaves"
    );

    let tensor_parameter = symbol(964);
    let tensor_name = named_argument(&mut program, "flag_planes", tensor_parameter);
    let tensor_planes = vec![
        vec![vec![true, false], vec![false, true]],
        vec![vec![true, true], vec![false, false]],
    ];
    let exact_tensor_literal = boolean_tensor3_literal(&mut program, tensor_planes.clone());
    let regrouped_tensor_literal = boolean_tensor3_literal(
        &mut program,
        vec![
            vec![tensor_planes[0][0].clone()],
            vec![
                tensor_planes[0][1].clone(),
                tensor_planes[1][0].clone(),
                tensor_planes[1][1].clone(),
            ],
        ],
    );
    let tensor_as_matrix_literal = nested_boolean_array_literal(
        &mut program,
        &[
            &tensor_planes[0][0],
            &tensor_planes[0][1],
            &tensor_planes[1][0],
            &tensor_planes[1][1],
        ],
    );
    let tensor_as_flat_literal = boolean_array_literal(
        &mut program,
        &[true, false, false, true, true, true, false, false],
    );
    let tensor_value = vec![ProofValueSubstitution::boolean_tensor3(
        tensor_parameter,
        &[
            Arc::from(vec![
                Arc::from(&[true, false][..]),
                Arc::from(&[false, true][..]),
            ]),
            Arc::from(vec![
                Arc::from(&[true, true][..]),
                Arc::from(&[false, false][..]),
            ]),
        ],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(tensor_name),
        &ProofFact::Expression(exact_tensor_literal),
        context(&tensor_value),
        context(&no_values),
    ));
    for drifted in [
        regrouped_tensor_literal,
        tensor_as_matrix_literal,
        tensor_as_flat_literal,
    ] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(tensor_name),
            &ProofFact::Expression(drifted),
            context(&tensor_value),
            context(&no_values),
        ));
    }
}

#[test]
fn proof_fact_recursive_primitive_arrays_retain_every_container_and_leaf_landing() {
    use super::proof_fact_identity::{
        ProofFactIdentityContext, ProofValueSubstitution, proof_facts_match,
    };
    use super::runtime_correspondence::{
        ClosedFloatArrayElement, ClosedIntegerArrayElement, ClosedRecursiveArrayElement as Value,
    };

    let mut program = TypedTrees::default();
    let no_values = Vec::new();
    let context = |values| ProofFactIdentityContext {
        values,
        static_bindings: &[],
    };

    let boolean_parameter = symbol(1005);
    let boolean_name = named_argument(&mut program, "deep_flags", boolean_parameter);
    let tensor = boolean_tensor3_literal(
        &mut program,
        vec![vec![vec![true, false], vec![false, true]]],
    );
    let exact_boolean = wrap_array_literal(&mut program, [tensor]);
    let matrix = nested_boolean_array_literal(&mut program, &[&[true, false], &[false, true]]);
    let flat = boolean_array_literal(&mut program, &[true, false, false, true]);
    let boolean_value = vec![ProofValueSubstitution::recursive_primitive_array(
        boolean_parameter,
        &[Value::Array(Arc::from(vec![Value::Array(Arc::from(
            vec![
                Value::Array(Arc::from(vec![Value::Boolean(true), Value::Boolean(false)])),
                Value::Array(Arc::from(vec![Value::Boolean(false), Value::Boolean(true)])),
            ],
        ))]))],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(boolean_name),
        &ProofFact::Expression(exact_boolean),
        context(&boolean_value),
        context(&no_values),
    ));
    for collapsed in [tensor, matrix, flat] {
        assert!(!proof_facts_match(
            &program,
            &ProofFact::Expression(boolean_name),
            &ProofFact::Expression(collapsed),
            context(&boolean_value),
            context(&no_values),
        ));
    }

    let integer_parameter = symbol(1006);
    let integer_name = named_argument(&mut program, "deep_offsets", integer_parameter);
    let exact_integer_landing = IntegerLanding {
        landed_type: LandedIntegerType::I16,
        domain: ArithmeticDomain::Exact,
    };
    let exact_integer_matrix = nested_integer_array_literal(
        &mut program,
        vec![vec![
            IntegerLiteral::from_value(7).with_landing(exact_integer_landing),
        ]],
    );
    let exact_integer_tensor = wrap_array_literal(&mut program, [exact_integer_matrix]);
    let wrapping_integer_matrix = nested_integer_array_literal(
        &mut program,
        vec![vec![IntegerLiteral::from_value(7).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::I16,
                domain: ArithmeticDomain::Wrapping,
            },
        )]],
    );
    let wrapping_integer_tensor = wrap_array_literal(&mut program, [wrapping_integer_matrix]);
    let integer_value = vec![ProofValueSubstitution::recursive_primitive_array(
        integer_parameter,
        &[Value::Array(Arc::from(vec![Value::Array(Arc::from(
            vec![Value::Integer(ClosedIntegerArrayElement {
                spelling: "7".to_owned(),
                landing: exact_integer_landing,
            })],
        ))]))],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(integer_name),
        &ProofFact::Expression(exact_integer_tensor),
        context(&integer_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(integer_name),
        &ProofFact::Expression(wrapping_integer_tensor),
        context(&integer_value),
        context(&no_values),
    ));

    let float_parameter = symbol(1007);
    let float_name = named_argument(&mut program, "deep_scales", float_parameter);
    let exact_float_matrix = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
        ]],
    );
    let exact_float_tensor = wrap_array_literal(&mut program, [exact_float_matrix]);
    let drifted_float_matrix = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
        ]],
    );
    let drifted_float_tensor = wrap_array_literal(&mut program, [drifted_float_matrix]);
    let float_value = vec![ProofValueSubstitution::recursive_primitive_array(
        float_parameter,
        &[Value::Array(Arc::from(vec![Value::Array(Arc::from(
            vec![Value::Float(ClosedFloatArrayElement {
                spelling: "1.25".to_owned(),
                landing: FloatFormat::F32,
            })],
        ))]))],
    )];
    assert!(proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(exact_float_tensor),
        context(&float_value),
        context(&no_values),
    ));
    assert!(!proof_facts_match(
        &program,
        &ProofFact::Expression(float_name),
        &ProofFact::Expression(drifted_float_tensor),
        context(&float_value),
        context(&no_values),
    ));
}

#[test]
fn direct_lift_duplication_shares_each_side_value_without_collapsing_legality_coordinates() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(
        &mut program,
        symbol(904),
        "DiagonalQ",
        symbol(905),
        "DiagonalR",
    );
    let carrier = carrier_type(&mut program);
    let public_symbol = symbol(906);
    let unused_public_symbol = symbol(907);
    let public_value = named_argument(&mut program, "value", public_symbol);
    let public_diagonal = {
        let left = named_argument(&mut program, "value", public_symbol);
        let right = named_argument(&mut program, "value", public_symbol);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            }))
    };
    let public_facts = program.proof_facts.insert_many([
        ProofFact::Expression(public_diagonal),
        ProofFact::Expression(public_value),
    ]);
    let mut public_machine = Machine::default();
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: public_facts,
            ..Default::default()
        },
    );
    let mut public_state = State::default();
    for (symbol, name) in [(public_symbol, "value"), (unused_public_symbol, "unused")] {
        program.push_state_parameter(
            &mut public_state,
            StateParameter {
                symbol,
                name: Identifier::generated_static(name),
                type_reference: quotient_type,
                ..Default::default()
            },
        );
    }

    let representative_left = symbol(908);
    let representative_right = symbol(909);
    let representative_diagonal = {
        let left = named_argument(&mut program, "left", representative_left);
        let right = named_argument(&mut program, "right", representative_right);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Equal,
                right,
            }))
    };
    let representative_left_value = named_argument(&mut program, "left", representative_left);
    let representative_right_value = named_argument(&mut program, "right", representative_right);
    let representative_facts = program.proof_facts.insert_many([
        ProofFact::Expression(representative_diagonal),
        ProofFact::Expression(representative_left_value),
        ProofFact::Expression(representative_right_value),
    ]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(910),
        state_symbol: symbol(911),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_left,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_right,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type,
        quotient_symbol: symbol(904),
        relation_symbol: symbol(905),
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::Quotient(relation),
    ];
    let runtime = super::DirectLiftRuntimeCorrespondence {
        positions: vec![
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_symbol),
                representative_parameter: representative_left,
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_symbol),
                representative_parameter: representative_right,
            },
        ],
    };
    let public_partition = derive_direct_lift_public_precondition_partition(
        &program,
        &public_machine,
        &public_state,
        &input_relations,
        &runtime,
    )
    .expect("duplicated and omitted quotient parameters remain dependent Q");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("both representative occurrences remain dependent P");
    let expected_theorem =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("duplication does not alter the universal positional theorem");
    assert_eq!(expected_theorem.parameters.len(), 4);
    assert_eq!(expected_theorem.relation_premises.len(), 2);
    assert_eq!(expected_theorem.left_application.arguments, [0, 2]);
    assert_eq!(expected_theorem.right_application.arguments, [1, 3]);

    let verified_theorem = super::VerifiedTheoremSchema {
        theorem_machine_symbol: symbol(912),
        theorem_state_symbol: symbol(913),
        parameters: expected_theorem
            .parameters
            .iter()
            .enumerate()
            .map(|(expected_position, _)| {
                super::theorem_schema_verification::VerifiedTheoremParameter {
                    expected_position,
                    theorem_symbol: symbol(920 + expected_position as u32),
                }
            })
            .collect(),
        relation_premises: expected_theorem
            .relation_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| super::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 20 + expected_position,
                    },
                },
            )
            .collect(),
        legality_premises: expected_theorem
            .legality_premises
            .iter()
            .enumerate()
            .map(
                |(expected_position, _)| super::theorem_schema_verification::VerifiedTheoremFact {
                    expected_position,
                    actual: TheoremContractFactLocation {
                        owner: TheoremContractOwner::Machine,
                        contract_position: 0,
                        fact_position: 30 + expected_position,
                    },
                },
            )
            .collect(),
        conclusion: TheoremContractFactLocation {
            owner: TheoremContractOwner::State,
            contract_position: 0,
            fact_position: 0,
        },
    };

    let implication = derive_direct_lift_precondition_implication(
        &program,
        &public_machine,
        &public_state,
        &representative,
        &public_partition,
        &representative_partition,
        &runtime,
        &expected_theorem,
        &verified_theorem,
    )
    .expect("Q(x, x) and Q(x) instantiate both representative occurrences per side");
    assert_eq!(implication.rows.len(), 6);
    assert_eq!(
        exact_public_location(&implication.rows[1].proof).fact_position,
        1
    );
    assert_eq!(
        exact_public_location(&implication.rows[2].proof).fact_position,
        1
    );
    assert_eq!(implication.rows[1].representative.fact_position, 1);
    assert_eq!(implication.rows[2].representative.fact_position, 2);
    assert_ne!(implication.rows[1].theorem, implication.rows[2].theorem);
    assert_eq!(
        exact_public_location(&implication.rows[4].proof).fact_position,
        1
    );
    assert_eq!(
        exact_public_location(&implication.rows[5].proof).fact_position,
        1
    );
    assert_ne!(implication.rows[4].theorem, implication.rows[5].theorem);

    let mut tampered_runtime = runtime.clone();
    tampered_runtime.positions[1].source =
        super::DirectLiftArgumentSource::PublicParameter(unused_public_symbol);
    assert_eq!(
        derive_direct_lift_precondition_implication(
            &program,
            &public_machine,
            &public_state,
            &representative,
            &public_partition,
            &representative_partition,
            &tampered_runtime,
            &expected_theorem,
            &verified_theorem,
        ),
        Err(RelationPlanError::DirectLiftLeftPreconditionNotImplied(0)),
    );
}

#[test]
fn direct_lift_runtime_rung_accepts_subsets_permutations_and_duplicates_but_rejects_adaptation() {
    let mut program = TypedTrees::default();
    let left_quotient = quotient_type(&mut program, symbol(880), "LeftQ", symbol(881), "LeftR");
    let right_quotient = quotient_type(&mut program, symbol(888), "RightQ", symbol(889), "RightR");
    let carrier = carrier_type(&mut program);
    let left_symbol = symbol(882);
    let right_symbol = symbol(883);
    let omitted_symbol = symbol(890);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let adapted = program
        .expression_table
        .insert(ExpressionNode::Integer(Default::default()));
    let mut state = State {
        return_type: left_quotient,
        ..Default::default()
    };
    for (symbol, name, type_reference) in [
        (left_symbol, "left", left_quotient),
        (right_symbol, "right", right_quotient),
        (omitted_symbol, "omitted", left_quotient),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(884),
        state_symbol: symbol(885),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(886),
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(887),
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let left_relation = ExactQuotientRelation {
        quotient_type: left_quotient,
        quotient_symbol: symbol(880),
        relation_symbol: symbol(881),
    };
    let right_relation = ExactQuotientRelation {
        quotient_type: right_quotient,
        quotient_symbol: symbol(888),
        relation_symbol: symbol(889),
    };
    let derive = |program: &mut TypedTrees,
                  arguments: [ExpressionHandle; 2],
                  input_relations: [InputRelation; 2]| {
        let arguments = program
            .expression_table
            .insert_expression_handles(arguments);
        derive_direct_lift_runtime_correspondence(
            program,
            &Machine::default(),
            &state,
            &call_with_arguments(arguments),
            &input_relations,
            left_relation,
            &representative,
        )
    };

    assert!(
        derive(
            &mut program,
            [left, right],
            [
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
        )
        .is_ok(),
        "lift may select a direct subset of a larger public telescope"
    );
    let reordered = derive(
        &mut program,
        [right, left],
        [
            InputRelation::Quotient(right_relation),
            InputRelation::Quotient(left_relation),
        ],
    )
    .expect("lift may explicitly permute unique direct public parameters");
    assert_eq!(
        reordered.positions,
        [
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(right_symbol),
                representative_parameter: representative.parameters[0].symbol,
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(left_symbol),
                representative_parameter: representative.parameters[1].symbol,
            },
        ]
    );
    let duplicated = derive(
        &mut program,
        [left, left],
        [
            InputRelation::Quotient(left_relation),
            InputRelation::Quotient(left_relation),
        ],
    )
    .expect("lift may repeat one exact direct public parameter");
    assert_eq!(
        duplicated.positions,
        [
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(left_symbol),
                representative_parameter: representative.parameters[0].symbol,
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(left_symbol),
                representative_parameter: representative.parameters[1].symbol,
            },
        ]
    );
    assert_eq!(
        derive(
            &mut program,
            [left, adapted],
            [
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
        ),
        Err(RelationPlanError::DirectLiftLiteralTargetMismatch(1)),
    );
    assert_eq!(
        derive(
            &mut program,
            [right, left],
            [
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
        ),
        Err(RelationPlanError::DirectLiftParameterTypeMismatch(0)),
        "a permutation cannot retain the stale declaration-order relation vector"
    );

    let arguments = program
        .expression_table
        .insert_expression_handles([left, right]);
    assert_eq!(
        derive_define_runtime_correspondence(
            &program,
            &Machine::default(),
            &state,
            &call_with_arguments(arguments),
            &[
                InputRelation::Quotient(left_relation),
                InputRelation::Quotient(right_relation),
            ],
            left_relation,
            &representative,
        ),
        Err(RelationPlanError::DefineRuntimeArityMismatch),
        "define may not omit a public parameter"
    );
}

#[test]
fn direct_lift_runtime_accepts_only_closed_exact_scalar_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(920),
        "LiteralQ",
        symbol(921),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let bool_type = primitive_type(&mut program, "bool");
    let i8_type = primitive_type(&mut program, "i8");
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let integer = program
        .expression_table
        .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
    let arguments = program
        .expression_table
        .insert_expression_handles([boolean, integer]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(bool_type, false, false), (i8_type, false, false)],
        carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("the exact representative type lands an anonymous integer once");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(bool_type),
            InputRelation::ExactEquality(i8_type),
        ]
    );
    assert_eq!(plan.expected_theorem_schema.parameters.len(), 2);
    assert_eq!(
        plan.expected_theorem_schema.left_application.arguments,
        [0, 1]
    );
    assert_eq!(
        plan.expected_theorem_schema.right_application.arguments,
        [0, 1]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Boolean(true),
                ),
                representative_parameter: symbol(100),
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Integer {
                        spelling: "7".to_owned(),
                        landing: IntegerLanding {
                            landed_type: LandedIntegerType::I8,
                            domain: ArithmeticDomain::Exact,
                        },
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    let mut spelling_drift = runtime.clone();
    spelling_drift.positions[1].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::Integer {
            spelling: "0x7".to_owned(),
            landing: IntegerLanding {
                landed_type: LandedIntegerType::I8,
                domain: ArithmeticDomain::Exact,
            },
        },
    );
    assert_ne!(
        runtime, spelling_drift,
        "literal spelling is retained identity"
    );
    let mut landing_drift = runtime.clone();
    landing_drift.positions[1].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::Integer {
            spelling: "7".to_owned(),
            landing: IntegerLanding {
                landed_type: LandedIntegerType::I8,
                domain: ArithmeticDomain::Wrapping,
            },
        },
    );
    assert_ne!(
        runtime, landing_drift,
        "literal landing is retained identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_explicit_and_target_landed_float_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(970),
        "FloatLiteralQ",
        symbol(971),
        "FloatLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let f32_type = primitive_type(&mut program, "f32");
    let f64_type = primitive_type(&mut program, "f64");
    let f32_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
    ));
    let f64_literal = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
    ));
    let arguments = program
        .expression_table
        .insert_expression_handles([f32_literal, f64_literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(f32_type, false, false), (f64_type, false, false)],
        carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("the exact f32 target lands an anonymous decimal once");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(f32_type),
            InputRelation::ExactEquality(f64_type),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("float literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Float {
                        spelling: "1.25".to_owned(),
                        landing: FloatFormat::F32,
                    },
                ),
                representative_parameter: symbol(100),
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::Float {
                        spelling: "1.25".to_owned(),
                        landing: FloatFormat::F64,
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    assert_ne!(
        runtime.positions[0].source, runtime.positions[1].source,
        "float format landing remains runtime-evidence identity"
    );
    let mut spelling_drift = runtime.clone();
    spelling_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::Float {
            spelling: "1.5".to_owned(),
            landing: FloatFormat::F32,
        },
    );
    assert_ne!(
        runtime, spelling_drift,
        "float spelling remains runtime-evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_shared_and_bounded_byte_string_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(972),
        "ByteStringLiteralQ",
        symbol(973),
        "ByteStringLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let byte_view =
        byte_slice_reference_type(&mut program, psi_language_core::ReferenceAccess::Shared);
    let bounded_bytes = bounded_byte_buffer_type(&mut program, 8);
    let literal_bytes: Arc<[u8]> = Arc::from(&b"value"[..]);
    let literal = program
        .expression_table
        .insert(ExpressionNode::String(literal_bytes.clone()));
    let arguments = program
        .expression_table
        .insert_expression_handles([literal, literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(byte_view, false, false), (bounded_bytes, false, false)],
        carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("an immutable-image byte string has the exact shared byte-view type");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(byte_view),
            InputRelation::ExactEquality(bounded_bytes),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("byte-string literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::ByteString {
                        bytes: literal_bytes.clone(),
                        target_type: program.normalized_type_identity(byte_view),
                    },
                ),
                representative_parameter: symbol(100),
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::ByteString {
                        bytes: literal_bytes,
                        target_type: program.normalized_type_identity(bounded_bytes),
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    assert_ne!(
        runtime.positions[0].source, runtime.positions[1].source,
        "shared and owned bounded byte targets remain distinct identity"
    );
    let mut bytes_drift = runtime.clone();
    bytes_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::ByteString {
            bytes: Arc::from(&b"other"[..]),
            target_type: program.normalized_type_identity(byte_view),
        },
    );
    assert_ne!(
        runtime, bytes_drift,
        "exact bytes remain occurrence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_fixed_byte_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(974),
        "FixedByteArrayLiteralQ",
        symbol(975),
        "FixedByteArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_bytes = fixed_byte_array_type(&mut program, 5);
    let literal = canonical_byte_array_literal(&mut program, b"value");
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_bytes, false, false)], carrier);

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("ordinary contextual typing already landed the exact fixed byte array");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_bytes)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("fixed byte-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [super::DirectLiftRuntimePosition {
            source: super::DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::FixedByteArray {
                    bytes: Arc::from(&b"value"[..]),
                    target_type: program.normalized_type_identity(fixed_bytes),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut bytes_drift = runtime.clone();
    bytes_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::FixedByteArray {
            bytes: Arc::from(&b"other"[..]),
            target_type: program.normalized_type_identity(fixed_bytes),
        },
    );
    assert_ne!(
        runtime, bytes_drift,
        "element order and values remain identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_nested_fixed_byte_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(986),
        "NestedFixedByteArrayLiteralQ",
        symbol(987),
        "NestedFixedByteArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_bytes = fixed_nested_byte_array_type(&mut program, 2, 3);
    let literal = nested_canonical_byte_array_literal(&mut program, &[&[0, 127, 255], &[3, 2, 1]]);
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_bytes, false, false)], carrier);

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("each exact byte row is already canonically context-landed");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_bytes)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested fixed-byte-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [super::DirectLiftRuntimePosition {
            source: super::DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::NestedFixedByteArray {
                    rows: Arc::from(vec![
                        Arc::from(&[0, 127, 255][..]),
                        Arc::from(&[3, 2, 1][..]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_bytes),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut row_boundary_drift = runtime.clone();
    row_boundary_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::NestedFixedByteArray {
            rows: Arc::from(vec![
                Arc::from(&[0, 127][..]),
                Arc::from(&[255, 3, 2, 1][..]),
            ]),
            target_type: program.normalized_type_identity(fixed_bytes),
        },
    );
    assert_ne!(
        runtime, row_boundary_drift,
        "byte row boundaries remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_boolean_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(976),
        "BooleanArrayLiteralQ",
        symbol(977),
        "BooleanArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_booleans = fixed_boolean_array_type(&mut program, 3);
    let literal = boolean_array_literal(&mut program, &[true, false, true]);
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_booleans, false, false)], carrier);

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("an exact fixed Boolean array needs no element adaptation");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_booleans)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("Boolean-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [super::DirectLiftRuntimePosition {
            source: super::DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::BooleanArray {
                    values: Arc::from(&[true, false, true][..]),
                    target_type: program.normalized_type_identity(fixed_booleans),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut order_drift = runtime.clone();
    order_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::BooleanArray {
            values: Arc::from(&[true, true, false][..]),
            target_type: program.normalized_type_identity(fixed_booleans),
        },
    );
    assert_ne!(
        runtime, order_drift,
        "Boolean element order remains identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_nested_boolean_array_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(982),
        "NestedBooleanArrayLiteralQ",
        symbol(983),
        "NestedBooleanArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_booleans = fixed_nested_boolean_array_type(&mut program, 2, 3);
    let literal =
        nested_boolean_array_literal(&mut program, &[&[true, false, true], &[false, true, false]]);
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_booleans, false, false)], carrier);

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("an exact depth-two Boolean array needs no adaptation");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_booleans)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested Boolean-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [super::DirectLiftRuntimePosition {
            source: super::DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::NestedBooleanArray {
                    rows: Arc::from(vec![
                        Arc::from(&[true, false, true][..]),
                        Arc::from(&[false, true, false][..]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_booleans),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut row_boundary_drift = runtime.clone();
    row_boundary_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::NestedBooleanArray {
            rows: Arc::from(vec![
                Arc::from(&[true, false][..]),
                Arc::from(&[true, false, true, false][..]),
            ]),
            target_type: program.normalized_type_identity(fixed_booleans),
        },
    );
    assert_ne!(
        runtime, row_boundary_drift,
        "row boundaries remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_boolean_tensor3_literals() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(998),
        "BooleanTensor3LiteralQ",
        symbol(999),
        "BooleanTensor3LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let tensor_type = fixed_boolean_tensor3_type(&mut program, 2, 2, 2);
    let literal = boolean_tensor3_literal(
        &mut program,
        vec![
            vec![vec![true, false], vec![false, true]],
            vec![vec![true, true], vec![false, false]],
        ],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(tensor_type, false, false)], carrier);

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("an exact depth-three Boolean tensor needs no adaptation");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(tensor_type)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("Boolean tensor runtime correspondence");
    assert_eq!(
        runtime.positions,
        [super::DirectLiftRuntimePosition {
            source: super::DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::BooleanTensor3 {
                    planes: Arc::from(vec![
                        Arc::from(vec![
                            Arc::from(&[true, false][..]),
                            Arc::from(&[false, true][..]),
                        ]),
                        Arc::from(vec![
                            Arc::from(&[true, true][..]),
                            Arc::from(&[false, false][..]),
                        ]),
                    ]),
                    target_type: program.normalized_type_identity(tensor_type),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut grouping_drift = runtime.clone();
    grouping_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::BooleanTensor3 {
            planes: Arc::from(vec![
                Arc::from(vec![Arc::from(&[true, false][..])]),
                Arc::from(vec![
                    Arc::from(&[false, true][..]),
                    Arc::from(&[true, true][..]),
                    Arc::from(&[false, false][..]),
                ]),
            ]),
            target_type: program.normalized_type_identity(tensor_type),
        },
    );
    assert_ne!(
        runtime, grouping_drift,
        "plane and row boundaries remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_remaining_recursive_primitive_arrays() {
    use super::runtime_correspondence::{
        ClosedFloatArrayElement, ClosedIntegerArrayElement, ClosedRecursiveArrayElement as Value,
    };

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(1003),
        "RecursivePrimitiveArrayQ",
        symbol(1004),
        "RecursivePrimitiveArrayR",
    );
    let carrier = carrier_type(&mut program);

    let byte_matrix_type = fixed_nested_byte_array_type(&mut program, 1, 2);
    let byte_tensor_type = wrap_fixed_array_type(&mut program, byte_matrix_type, 2);
    let first_bytes = nested_canonical_byte_array_literal(&mut program, &[&[1, 2]]);
    let second_bytes = nested_canonical_byte_array_literal(&mut program, &[&[3, 4]]);
    let byte_tensor = wrap_array_literal(&mut program, [first_bytes, second_bytes]);

    let integer_matrix_type = fixed_nested_integer_array_type(&mut program, "i8", 1, 1);
    let integer_tensor_type = wrap_fixed_array_type(&mut program, integer_matrix_type, 1);
    let integer_matrix =
        nested_integer_array_literal(&mut program, vec![vec![IntegerLiteral::from_value(-1)]]);
    let integer_tensor = wrap_array_literal(&mut program, [integer_matrix]);

    let float_matrix_type = fixed_nested_float_array_type(&mut program, "f32", 1, 1);
    let float_tensor_type = wrap_fixed_array_type(&mut program, float_matrix_type, 1);
    let float_matrix = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
        ]],
    );
    let float_tensor = wrap_array_literal(&mut program, [float_matrix]);

    let boolean_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 1, 1);
    let boolean_depth_four_type = wrap_fixed_array_type(&mut program, boolean_tensor_type, 1);
    let boolean_tensor = boolean_tensor3_literal(&mut program, vec![vec![vec![true]]]);
    let boolean_depth_four = wrap_array_literal(&mut program, [boolean_tensor]);

    let arguments = program.expression_table.insert_expression_handles([
        byte_tensor,
        integer_tensor,
        float_tensor,
        boolean_depth_four,
    ]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[
            (byte_tensor_type, false, false),
            (integer_tensor_type, false, false),
            (float_tensor_type, false, false),
            (boolean_depth_four_type, false, false),
        ],
        carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("remaining exact primitive arrays use recursive evidence");
    let runtime = plan
        .direct_lift_correspondence
        .expect("recursive primitive-array runtime correspondence");
    let i8_landing = IntegerLanding {
        landed_type: LandedIntegerType::I8,
        domain: ArithmeticDomain::Exact,
    };
    let expected = [
        (
            vec![
                Value::Array(Arc::from(vec![Value::Array(Arc::from(vec![
                    Value::Byte(1),
                    Value::Byte(2),
                ]))])),
                Value::Array(Arc::from(vec![Value::Array(Arc::from(vec![
                    Value::Byte(3),
                    Value::Byte(4),
                ]))])),
            ],
            program.normalized_type_identity(byte_tensor_type),
        ),
        (
            vec![Value::Array(Arc::from(vec![Value::Array(Arc::from(
                vec![Value::Integer(ClosedIntegerArrayElement {
                    spelling: "-1".to_owned(),
                    landing: i8_landing,
                })],
            ))]))],
            program.normalized_type_identity(integer_tensor_type),
        ),
        (
            vec![Value::Array(Arc::from(vec![Value::Array(Arc::from(
                vec![Value::Float(ClosedFloatArrayElement {
                    spelling: "1.25".to_owned(),
                    landing: FloatFormat::F32,
                })],
            ))]))],
            program.normalized_type_identity(float_tensor_type),
        ),
        (
            vec![Value::Array(Arc::from(vec![Value::Array(Arc::from(
                vec![Value::Array(Arc::from(vec![Value::Boolean(true)]))],
            ))]))],
            program.normalized_type_identity(boolean_depth_four_type),
        ),
    ];
    for (position, ((expected_elements, expected_type), actual)) in
        expected.into_iter().zip(&runtime.positions).enumerate()
    {
        let super::DirectLiftArgumentSource::Literal(
            super::runtime_correspondence::ClosedLiftLiteral::RecursivePrimitiveArray {
                elements,
                target_type,
            },
        ) = &actual.source
        else {
            panic!("recursive fallback owns newly admitted position {position}");
        };
        assert_eq!(elements.as_ref(), expected_elements);
        assert_eq!(*target_type, expected_type);
    }

    assert!(matches!(
        super::closed_lift_literal_for_representative(
            &program,
            boolean_tensor,
            boolean_tensor_type,
            4,
        ),
        Ok(Some(
            super::runtime_correspondence::ClosedLiftLiteral::BooleanTensor3 { .. }
        ))
    ));
    assert!(matches!(
        super::closed_lift_literal_for_representative(&program, float_matrix, float_matrix_type, 5,),
        Ok(Some(
            super::runtime_correspondence::ClosedLiftLiteral::NestedFloatArray { .. }
        ))
    ));
    let boolean_depth_five_type = wrap_fixed_array_type(&mut program, boolean_depth_four_type, 1);
    let boolean_depth_five = wrap_array_literal(&mut program, [boolean_depth_four]);
    assert!(matches!(
        super::closed_lift_literal_for_representative(
            &program,
            boolean_depth_five,
            boolean_depth_five_type,
            6,
        ),
        Ok(Some(
            super::runtime_correspondence::ClosedLiftLiteral::RecursivePrimitiveArray { .. }
        ))
    ));
}

#[test]
fn direct_lift_runtime_accepts_exact_integer_array_literals() {
    use super::runtime_correspondence::ClosedIntegerArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(978),
        "IntegerArrayLiteralQ",
        symbol(979),
        "IntegerArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let i8_array = fixed_integer_array_type(&mut program, "i8", 2);
    let u16_array = fixed_integer_array_type(&mut program, "u16", 1);
    let i8_literal = integer_array_literal(
        &mut program,
        [
            IntegerLiteral::from_value(-1),
            IntegerLiteral::from_value(127),
        ],
    );
    let u16_landing = IntegerLanding {
        landed_type: LandedIntegerType::U16,
        domain: ArithmeticDomain::Exact,
    };
    let u16_literal = integer_array_literal(
        &mut program,
        [IntegerLiteral::from_value(7).with_landing(u16_landing)],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([i8_literal, u16_literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(i8_array, false, false), (u16_array, false, false)],
        carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("each fixed integer-array element lands by the exact scalar rule");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(i8_array),
            InputRelation::ExactEquality(u16_array),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("integer-array literal runtime correspondence");
    let i8_landing = IntegerLanding {
        landed_type: LandedIntegerType::I8,
        domain: ArithmeticDomain::Exact,
    };
    assert_eq!(
        runtime.positions,
        [
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::IntegerArray {
                        elements: Arc::from(vec![
                            ClosedIntegerArrayElement {
                                spelling: "-1".to_owned(),
                                landing: i8_landing,
                            },
                            ClosedIntegerArrayElement {
                                spelling: "127".to_owned(),
                                landing: i8_landing,
                            },
                        ]),
                        target_type: program.normalized_type_identity(i8_array),
                    },
                ),
                representative_parameter: symbol(100),
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::IntegerArray {
                        elements: Arc::from(vec![ClosedIntegerArrayElement {
                            spelling: "7".to_owned(),
                            landing: u16_landing,
                        }]),
                        target_type: program.normalized_type_identity(u16_array),
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    assert_ne!(
        runtime.positions[0].source, runtime.positions[1].source,
        "element landing and exact array target remain identity"
    );
    let mut landing_drift = runtime.clone();
    landing_drift.positions[1].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::IntegerArray {
            elements: Arc::from(vec![ClosedIntegerArrayElement {
                spelling: "7".to_owned(),
                landing: IntegerLanding {
                    landed_type: LandedIntegerType::U16,
                    domain: ArithmeticDomain::Wrapping,
                },
            }]),
            target_type: program.normalized_type_identity(u16_array),
        },
    );
    assert_ne!(runtime, landing_drift, "element domain remains identity");
}

#[test]
fn direct_lift_runtime_accepts_exact_nested_integer_array_literals() {
    use super::runtime_correspondence::ClosedIntegerArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(990),
        "NestedIntegerArrayLiteralQ",
        symbol(991),
        "NestedIntegerArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_integers = fixed_nested_integer_array_type(&mut program, "i8", 2, 2);
    let i8_landing = IntegerLanding {
        landed_type: LandedIntegerType::I8,
        domain: ArithmeticDomain::Exact,
    };
    let literal = nested_integer_array_literal(
        &mut program,
        vec![
            vec![
                IntegerLiteral::from_value(-1),
                IntegerLiteral::from_value(127).with_landing(i8_landing),
            ],
            vec![IntegerLiteral::from_value(3), IntegerLiteral::from_value(2)],
        ],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_integers, false, false)], carrier);

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("every integer-matrix leaf follows the exact scalar landing rule");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_integers)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested integer-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [super::DirectLiftRuntimePosition {
            source: super::DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::NestedIntegerArray {
                    rows: Arc::from(vec![
                        Arc::from(vec![
                            ClosedIntegerArrayElement {
                                spelling: "-1".to_owned(),
                                landing: i8_landing,
                            },
                            ClosedIntegerArrayElement {
                                spelling: "127".to_owned(),
                                landing: i8_landing,
                            },
                        ]),
                        Arc::from(vec![
                            ClosedIntegerArrayElement {
                                spelling: "3".to_owned(),
                                landing: i8_landing,
                            },
                            ClosedIntegerArrayElement {
                                spelling: "2".to_owned(),
                                landing: i8_landing,
                            },
                        ]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_integers),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut evidence_drift = runtime.clone();
    evidence_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::NestedIntegerArray {
            rows: Arc::from(vec![
                Arc::from(vec![ClosedIntegerArrayElement {
                    spelling: "-1".to_owned(),
                    landing: i8_landing,
                }]),
                Arc::from(vec![
                    ClosedIntegerArrayElement {
                        spelling: "127".to_owned(),
                        landing: i8_landing,
                    },
                    ClosedIntegerArrayElement {
                        spelling: "3".to_owned(),
                        landing: i8_landing,
                    },
                    ClosedIntegerArrayElement {
                        spelling: "2".to_owned(),
                        landing: IntegerLanding {
                            landed_type: LandedIntegerType::I8,
                            domain: ArithmeticDomain::Wrapping,
                        },
                    },
                ]),
            ]),
            target_type: program.normalized_type_identity(fixed_integers),
        },
    );
    assert_ne!(
        runtime, evidence_drift,
        "row boundaries and integer landing domains remain evidence identity"
    );
}

#[test]
fn direct_lift_runtime_accepts_exact_float_array_literals() {
    use super::runtime_correspondence::ClosedFloatArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(980),
        "FloatArrayLiteralQ",
        symbol(981),
        "FloatArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let f32_array = fixed_float_array_type(&mut program, "f32", 2);
    let f64_array = fixed_float_array_type(&mut program, "f64", 1);
    let f32_literal = float_array_literal(
        &mut program,
        [
            FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
            FloatLiteral::parse("2.5").expect("anonymous exact decimal literal"),
        ],
    );
    let f64_literal = float_array_literal(
        &mut program,
        [FloatLiteral::parse("3.75f64").expect("format-landed f64 literal")],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([f32_literal, f64_literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(
        &mut program,
        &[(f32_array, false, false), (f64_array, false, false)],
        carrier,
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("each fixed float-array element follows the exact scalar format rule");
    assert_eq!(
        plan.input_relations,
        [
            InputRelation::ExactEquality(f32_array),
            InputRelation::ExactEquality(f64_array),
        ]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("float-array literal runtime correspondence");
    assert_eq!(
        runtime.positions,
        [
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::FloatArray {
                        elements: Arc::from(vec![
                            ClosedFloatArrayElement {
                                spelling: "1.25".to_owned(),
                                landing: FloatFormat::F32,
                            },
                            ClosedFloatArrayElement {
                                spelling: "2.5".to_owned(),
                                landing: FloatFormat::F32,
                            },
                        ]),
                        target_type: program.normalized_type_identity(f32_array),
                    },
                ),
                representative_parameter: symbol(100),
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::Literal(
                    super::runtime_correspondence::ClosedLiftLiteral::FloatArray {
                        elements: Arc::from(vec![ClosedFloatArrayElement {
                            spelling: "3.75".to_owned(),
                            landing: FloatFormat::F64,
                        }]),
                        target_type: program.normalized_type_identity(f64_array),
                    },
                ),
                representative_parameter: symbol(101),
            },
        ]
    );
    let mut format_drift = runtime.clone();
    format_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::FloatArray {
            elements: Arc::from(vec![
                ClosedFloatArrayElement {
                    spelling: "1.25".to_owned(),
                    landing: FloatFormat::F64,
                },
                ClosedFloatArrayElement {
                    spelling: "2.5".to_owned(),
                    landing: FloatFormat::F32,
                },
            ]),
            target_type: program.normalized_type_identity(f32_array),
        },
    );
    assert_ne!(runtime, format_drift, "element format remains identity");
}

#[test]
fn direct_lift_runtime_accepts_exact_nested_float_array_literals() {
    use super::runtime_correspondence::ClosedFloatArrayElement;

    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(994),
        "NestedFloatArrayLiteralQ",
        symbol(995),
        "NestedFloatArrayLiteralR",
    );
    let carrier = carrier_type(&mut program);
    let fixed_floats = fixed_nested_float_array_type(&mut program, "f32", 2, 2);
    let literal = nested_float_array_literal(
        &mut program,
        vec![
            vec![
                FloatLiteral::parse("1.25").expect("anonymous exact decimal literal"),
                FloatLiteral::parse("2.5f32").expect("format-landed f32 literal"),
            ],
            vec![
                FloatLiteral::parse("3.75").expect("anonymous exact decimal literal"),
                FloatLiteral::parse("4.5f32").expect("format-landed f32 literal"),
            ],
        ],
    );
    let arguments = program
        .expression_table
        .insert_expression_handles([literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let request = push_representative(&mut program, &[(fixed_floats, false, false)], carrier);

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("every float-matrix leaf follows the exact scalar format rule");
    assert_eq!(
        plan.input_relations,
        [InputRelation::ExactEquality(fixed_floats)]
    );
    let runtime = plan
        .direct_lift_correspondence
        .expect("nested float-array literal runtime correspondence");
    let element = |spelling: &str, landing| ClosedFloatArrayElement {
        spelling: spelling.to_owned(),
        landing,
    };
    assert_eq!(
        runtime.positions,
        [super::DirectLiftRuntimePosition {
            source: super::DirectLiftArgumentSource::Literal(
                super::runtime_correspondence::ClosedLiftLiteral::NestedFloatArray {
                    rows: Arc::from(vec![
                        Arc::from(vec![
                            element("1.25", FloatFormat::F32),
                            element("2.5", FloatFormat::F32),
                        ]),
                        Arc::from(vec![
                            element("3.75", FloatFormat::F32),
                            element("4.5", FloatFormat::F32),
                        ]),
                    ]),
                    target_type: program.normalized_type_identity(fixed_floats),
                },
            ),
            representative_parameter: symbol(100),
        }]
    );
    let mut evidence_drift = runtime.clone();
    evidence_drift.positions[0].source = super::DirectLiftArgumentSource::Literal(
        super::runtime_correspondence::ClosedLiftLiteral::NestedFloatArray {
            rows: Arc::from(vec![
                Arc::from(vec![element("1.25", FloatFormat::F32)]),
                Arc::from(vec![
                    element("2.5", FloatFormat::F32),
                    element("3.75", FloatFormat::F64),
                    element("4.5", FloatFormat::F32),
                ]),
            ]),
            target_type: program.normalized_type_identity(fixed_floats),
        },
    );
    assert_ne!(
        runtime, evidence_drift,
        "row boundaries and float formats remain evidence identity"
    );
}

#[test]
fn repeated_equal_direct_lift_literals_keep_distinct_runtime_and_theorem_positions() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(960),
        "LiteralQ",
        symbol(961),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let i32_type = primitive_type(&mut program, "i32");
    let literal = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(7).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I32,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let arguments = program
        .expression_table
        .insert_expression_handles([literal, literal]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(962),
        state_symbol: symbol(963),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(964),
                type_reference: i32_type,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(965),
                type_reference: i32_type,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type: quotient,
        quotient_symbol: symbol(960),
        relation_symbol: symbol(961),
    };
    let input_relations = [
        InputRelation::ExactEquality(i32_type),
        InputRelation::ExactEquality(i32_type),
    ];
    let runtime = derive_direct_lift_runtime_correspondence(
        &program,
        &Machine::default(),
        &state,
        &call,
        &input_relations,
        relation,
        &representative,
    )
    .expect("equal literals may repeat without coalescing their positions");
    assert_eq!(runtime.positions.len(), 2);
    assert_eq!(runtime.positions[0].source, runtime.positions[1].source);
    assert_ne!(
        runtime.positions[0].representative_parameter,
        runtime.positions[1].representative_parameter,
    );
    let expected =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("equal literals remain distinct universal theorem positions");
    assert_eq!(expected.parameters.len(), 2);
    assert_eq!(expected.left_application.arguments, [0, 1]);
    assert_eq!(expected.right_application.arguments, [0, 1]);
    assert!(
        expected.relation_premises.is_empty(),
        "ordinary exact-equality positions share their binders instead of adding quotient premises"
    );
}

#[test]
fn direct_lift_literal_fences_mismatched_and_unadmitted_values() {
    let mut program = TypedTrees::default();
    let i8_type = primitive_type(&mut program, "i8");
    let u8_type = primitive_type(&mut program, "u8");
    let bool_type = primitive_type(&mut program, "bool");
    let f32_type = primitive_type(&mut program, "f32");
    let f64_type = primitive_type(&mut program, "f64");
    let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let cases = [
        (
            program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_value(7).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::I16,
                    domain: ArithmeticDomain::Exact,
                }),
            )),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(0),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(128))),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(1),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Integer(IntegerLiteral::from_value(-1))),
            u8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(2),
        ),
        (
            program.expression_table.insert(ExpressionNode::Integer(
                IntegerLiteral::from_value(7).with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::I8,
                    domain: ArithmeticDomain::Wrapping,
                }),
            )),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(3),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Boolean(true)),
            i8_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(4),
        ),
        (
            program
                .expression_table
                .insert(ExpressionNode::Boolean(true)),
            unit_type,
            RelationPlanError::DirectLiftLiteralTargetMismatch(5),
        ),
    ];
    for (position, (expression, target, expected)) in cases.iter().copied().enumerate() {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position,),
            Err(expected),
        );
    }
    let landed_float = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
    ));
    for (position, (expression, target)) in [(landed_float, f64_type), (landed_float, i8_type)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            super::closed_lift_literal_for_representative(
                &program,
                expression,
                target,
                position + 6,
            ),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(
                position + 6
            )),
        );
    }
    let zero = program
        .expression_table
        .insert(ExpressionNode::ZeroValue(bool_type));
    let string = program
        .expression_table
        .insert(ExpressionNode::String(Arc::from(&b"value"[..])));
    let mutable_byte_view =
        byte_slice_reference_type(&mut program, psi_language_core::ReferenceAccess::Mutable);
    let undersized_buffer = bounded_byte_buffer_type(&mut program, 4);
    let bare_byte_array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: u8_type,
            length: FixedArrayLength::Literal(5),
        });
    for (position, target) in [
        (8, bool_type),
        (9, mutable_byte_view),
        (10, undersized_buffer),
        (11, bare_byte_array),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, string, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let array_values = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let array = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(array_values));
    assert_eq!(
        super::closed_lift_literal_for_representative(&program, array, bool_type, 12),
        Err(RelationPlanError::DirectLiftLiteralTargetMismatch(12)),
    );
    let computed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: cases[4].0,
            operator: BinaryOperator::Equal,
            right: cases[4].0,
        }));
    let call_arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let call = program
        .expression_table
        .insert(ExpressionNode::Call(call_with_arguments(call_arguments)));
    for (position, (expression, target)) in
        [(zero, bool_type), (computed, bool_type), (call, bool_type)]
            .into_iter()
            .enumerate()
    {
        assert_eq!(
            super::closed_lift_literal_for_representative(
                &program,
                expression,
                target,
                position + 13,
            ),
            Ok(None),
        );
    }

    let constrained = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: bool_type,
            constraints: HandleSpan::empty(),
        });
    let generic = program
        .type_reference_table
        .insert(TypeReferenceNode::Generic {
            base_symbol: SymbolHandle::invalid(),
            base_name: Identifier::generated_static("Box"),
            lifetime_arguments: Vec::new(),
            arguments: HandleSpan::empty(),
        });
    for (position, target) in [(16, constrained), (17, generic)] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, cases[4].0, target, position,),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let short_array = canonical_byte_array_literal(&mut program, b"four");
    let mismatched_landing_element = program.expression_table.insert(ExpressionNode::Integer(
        IntegerLiteral::from_value(1).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::U16,
            domain: ArithmeticDomain::Exact,
        }),
    ));
    let mismatched_landing_elements = program
        .expression_table
        .insert_expression_handles([mismatched_landing_element]);
    let mismatched_landing_array = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(mismatched_landing_elements));
    let one_byte_array = fixed_byte_array_type(&mut program, 1);
    for (position, expression, target) in [
        (18, short_array, bare_byte_array),
        (19, mismatched_landing_array, one_byte_array),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let integer_array = canonical_byte_array_literal(&mut program, &[1]);
    let boolean_array = boolean_array_literal(&mut program, &[true]);
    let one_boolean_array = fixed_boolean_array_type(&mut program, 1);
    let two_boolean_array = fixed_boolean_array_type(&mut program, 2);
    for (position, expression, target) in [
        (20, integer_array, one_boolean_array),
        (21, boolean_array, one_byte_array),
        (22, boolean_array, two_boolean_array),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let out_of_range_array = integer_array_literal(&mut program, [IntegerLiteral::from_value(128)]);
    let wrapping_array = integer_array_literal(
        &mut program,
        [IntegerLiteral::from_value(1).with_landing(IntegerLanding {
            landed_type: LandedIntegerType::I8,
            domain: ArithmeticDomain::Wrapping,
        })],
    );
    let one_i8_array = fixed_integer_array_type(&mut program, "i8", 1);
    let constrained_boolean_array =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_i8_array = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: i8_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(980),
                name: Identifier::generated_static("N"),
            },
        });
    for (position, expression, target) in [
        (23, out_of_range_array, one_i8_array),
        (24, wrapping_array, one_i8_array),
        (25, boolean_array, constrained_boolean_array),
        (26, out_of_range_array, unresolved_i8_array),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
    let mismatched_float_array = float_array_literal(
        &mut program,
        [FloatLiteral::parse("1.25f64").expect("format-landed f64 literal")],
    );
    let short_float_array = float_array_literal(
        &mut program,
        [FloatLiteral::parse("1.25f32").expect("format-landed f32 literal")],
    );
    let computed_left = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("1.0f32").expect("format-landed f32 literal"),
    ));
    let computed_right = program.expression_table.insert(ExpressionNode::Float(
        FloatLiteral::parse("2.0f32").expect("format-landed f32 literal"),
    ));
    let computed_float =
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: computed_left,
                operator: BinaryOperator::Add,
                right: computed_right,
            }));
    let computed_elements = program
        .expression_table
        .insert_expression_handles([computed_float]);
    let computed_float_array = program
        .expression_table
        .insert(ExpressionNode::ArrayLiteral(computed_elements));
    let one_f32_array = fixed_float_array_type(&mut program, "f32", 1);
    let two_f32_array = fixed_float_array_type(&mut program, "f32", 2);
    let constrained_f32 = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: f32_type,
            constraints: HandleSpan::empty(),
        });
    let constrained_float_array =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_f32,
                length: FixedArrayLength::Literal(1),
            });
    for (position, expression, target) in [
        (27, mismatched_float_array, one_f32_array),
        (28, integer_array, one_f32_array),
        (29, short_float_array, two_f32_array),
        (30, computed_float_array, one_f32_array),
        (31, short_float_array, constrained_float_array),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let exact_nested_boolean_array =
        nested_boolean_array_literal(&mut program, &[&[true], &[false]]);
    let ragged_nested_boolean_array =
        nested_boolean_array_literal(&mut program, &[&[true], &[false, true]]);
    let exact_nested_boolean_type = fixed_nested_boolean_array_type(&mut program, 2, 1);
    let wrong_outer_width_nested_boolean_type = fixed_nested_boolean_array_type(&mut program, 1, 1);
    let constrained_boolean_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_nested_boolean_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_boolean_row,
                length: FixedArrayLength::Literal(2),
            });
    let unresolved_boolean_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: bool_type,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(984),
                    name: Identifier::generated_static("M"),
                },
            });
    let unresolved_inner_width_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_boolean_row,
                length: FixedArrayLength::Literal(2),
            });
    let exact_boolean_row = fixed_boolean_array_type(&mut program, 1);
    let unresolved_outer_width_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_boolean_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(985),
                    name: Identifier::generated_static("N"),
                },
            });
    let nested_integer_array = {
        let row = integer_array_literal(&mut program, [IntegerLiteral::from_value(1)]);
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let computed_nested_boolean_array = {
        let row = program
            .expression_table
            .insert_expression_handles([computed]);
        let row = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(row));
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let one_by_one_nested_boolean_type = fixed_nested_boolean_array_type(&mut program, 1, 1);
    for (position, expression, target) in [
        (
            32,
            exact_nested_boolean_array,
            wrong_outer_width_nested_boolean_type,
        ),
        (33, ragged_nested_boolean_array, exact_nested_boolean_type),
        (
            34,
            exact_nested_boolean_array,
            constrained_nested_boolean_type,
        ),
        (35, exact_nested_boolean_array, unresolved_inner_width_type),
        (36, exact_nested_boolean_array, unresolved_outer_width_type),
        (
            38,
            computed_nested_boolean_array,
            one_by_one_nested_boolean_type,
        ),
        (40, nested_integer_array, one_by_one_nested_boolean_type),
        (41, boolean_array, one_by_one_nested_boolean_type),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let exact_nested_byte_array = nested_canonical_byte_array_literal(&mut program, &[&[1], &[2]]);
    let ragged_nested_byte_array =
        nested_canonical_byte_array_literal(&mut program, &[&[1], &[2, 3]]);
    let exact_nested_byte_type = fixed_nested_byte_array_type(&mut program, 2, 1);
    let wrong_outer_width_nested_byte_type = fixed_nested_byte_array_type(&mut program, 1, 1);
    let landed_byte_array = {
        let row = integer_array_literal(
            &mut program,
            [IntegerLiteral::from_value(1).with_landing(IntegerLanding {
                landed_type: LandedIntegerType::U8,
                domain: ArithmeticDomain::Exact,
            })],
        );
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let out_of_range_nested_byte_array = {
        let row = integer_array_literal(&mut program, [IntegerLiteral::from_value(256)]);
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let computed_nested_byte_array = {
        let left = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(1)));
        let right = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(2)));
        let computed =
            program
                .expression_table
                .insert(ExpressionNode::Binary(TableBinaryExpression {
                    left,
                    operator: BinaryOperator::Add,
                    right,
                }));
        let row = program
            .expression_table
            .insert_expression_handles([computed]);
        let row = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(row));
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let nested_boolean_array = nested_boolean_array_literal(&mut program, &[&[true]]);
    let constrained_u8 = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: u8_type,
            constraints: HandleSpan::empty(),
        });
    let constrained_byte_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: constrained_u8,
            length: FixedArrayLength::Literal(1),
        });
    let constrained_nested_byte_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_byte_row,
                length: FixedArrayLength::Literal(2),
            });
    let unresolved_byte_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: u8_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(988),
                name: Identifier::generated_static("M"),
            },
        });
    let unresolved_inner_byte_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_byte_row,
                length: FixedArrayLength::Literal(2),
            });
    let exact_byte_row = fixed_byte_array_type(&mut program, 1);
    let unresolved_outer_byte_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_byte_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(989),
                    name: Identifier::generated_static("N"),
                },
            });
    let one_by_one_nested_byte_type = fixed_nested_byte_array_type(&mut program, 1, 1);
    for (position, expression, target) in [
        (
            42,
            exact_nested_byte_array,
            wrong_outer_width_nested_byte_type,
        ),
        (43, ragged_nested_byte_array, exact_nested_byte_type),
        (44, landed_byte_array, one_by_one_nested_byte_type),
        (
            45,
            out_of_range_nested_byte_array,
            one_by_one_nested_byte_type,
        ),
        (46, computed_nested_byte_array, one_by_one_nested_byte_type),
        (47, nested_boolean_array, one_by_one_nested_byte_type),
        (48, exact_nested_byte_array, constrained_nested_byte_type),
        (49, exact_nested_byte_array, unresolved_inner_byte_type),
        (50, exact_nested_byte_array, unresolved_outer_byte_type),
        (52, integer_array, one_by_one_nested_byte_type),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let one_by_one_nested_i8_type = fixed_nested_integer_array_type(&mut program, "i8", 1, 1);
    let two_by_one_nested_i8_type = fixed_nested_integer_array_type(&mut program, "i8", 2, 1);
    let out_of_range_nested_integer_array =
        nested_integer_array_literal(&mut program, vec![vec![IntegerLiteral::from_value(128)]]);
    let mismatched_nested_integer_array = nested_integer_array_literal(
        &mut program,
        vec![vec![IntegerLiteral::from_value(1).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::U16,
                domain: ArithmeticDomain::Exact,
            },
        )]],
    );
    let wrapping_nested_integer_array = nested_integer_array_literal(
        &mut program,
        vec![vec![IntegerLiteral::from_value(1).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::I8,
                domain: ArithmeticDomain::Wrapping,
            },
        )]],
    );
    let ragged_nested_integer_array = nested_integer_array_literal(
        &mut program,
        vec![
            vec![IntegerLiteral::from_value(1)],
            vec![IntegerLiteral::from_value(2), IntegerLiteral::from_value(3)],
        ],
    );
    let nested_float_array = {
        let row = float_array_literal(
            &mut program,
            [FloatLiteral::parse("1.0f32").expect("format-landed f32 literal")],
        );
        let rows = program.expression_table.insert_expression_handles([row]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let constrained_i8 = program
        .type_reference_table
        .insert(TypeReferenceNode::Constrained {
            base_type: i8_type,
            constraints: HandleSpan::empty(),
        });
    let constrained_i8_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: constrained_i8,
            length: FixedArrayLength::Literal(1),
        });
    let constrained_nested_i8_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_i8_row,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_i8_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: i8_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(992),
                name: Identifier::generated_static("M"),
            },
        });
    let unresolved_inner_i8_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_i8_row,
                length: FixedArrayLength::Literal(1),
            });
    let exact_i8_row = fixed_integer_array_type(&mut program, "i8", 1);
    let unresolved_outer_i8_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_i8_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(993),
                    name: Identifier::generated_static("N"),
                },
            });
    for (position, expression, target) in [
        (
            53,
            out_of_range_nested_integer_array,
            one_by_one_nested_i8_type,
        ),
        (
            54,
            mismatched_nested_integer_array,
            one_by_one_nested_i8_type,
        ),
        (55, wrapping_nested_integer_array, one_by_one_nested_i8_type),
        (56, ragged_nested_integer_array, two_by_one_nested_i8_type),
        (57, computed_nested_byte_array, one_by_one_nested_i8_type),
        (58, nested_float_array, one_by_one_nested_i8_type),
        (59, nested_boolean_array, one_by_one_nested_i8_type),
        (60, nested_integer_array, constrained_nested_i8_type),
        (61, nested_integer_array, unresolved_inner_i8_type),
        (62, nested_integer_array, unresolved_outer_i8_type),
        (64, integer_array, one_by_one_nested_i8_type),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let one_by_one_nested_f32_type = fixed_nested_float_array_type(&mut program, "f32", 1, 1);
    let two_by_one_nested_f32_type = fixed_nested_float_array_type(&mut program, "f32", 2, 1);
    let exact_nested_float_array = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f32").expect("format-landed f32 literal"),
        ]],
    );
    let mismatched_nested_float_array = nested_float_array_literal(
        &mut program,
        vec![vec![
            FloatLiteral::parse("1.25f64").expect("format-landed f64 literal"),
        ]],
    );
    let ragged_nested_float_array = nested_float_array_literal(
        &mut program,
        vec![
            vec![FloatLiteral::parse("1.0").expect("anonymous float literal")],
            vec![
                FloatLiteral::parse("2.0").expect("anonymous float literal"),
                FloatLiteral::parse("3.0").expect("anonymous float literal"),
            ],
        ],
    );
    let computed_nested_float_array = {
        let rows = program
            .expression_table
            .insert_expression_handles([computed_float_array]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(rows))
    };
    let constrained_float_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_f32,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_nested_float_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_float_row,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_float_row = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: f32_type,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(996),
                name: Identifier::generated_static("M"),
            },
        });
    let unresolved_inner_float_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_float_row,
                length: FixedArrayLength::Literal(1),
            });
    let exact_float_row = fixed_float_array_type(&mut program, "f32", 1);
    let unresolved_outer_float_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_float_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(997),
                    name: Identifier::generated_static("N"),
                },
            });
    let wrong_outer_width_nested_f32_type =
        fixed_nested_float_array_type(&mut program, "f32", 2, 1);
    for (position, expression, target) in [
        (
            65,
            exact_nested_float_array,
            wrong_outer_width_nested_f32_type,
        ),
        (
            66,
            mismatched_nested_float_array,
            one_by_one_nested_f32_type,
        ),
        (67, ragged_nested_float_array, two_by_one_nested_f32_type),
        (68, computed_nested_float_array, one_by_one_nested_f32_type),
        (69, nested_integer_array, one_by_one_nested_f32_type),
        (70, nested_boolean_array, one_by_one_nested_f32_type),
        (71, exact_nested_float_array, constrained_nested_float_type),
        (72, exact_nested_float_array, unresolved_inner_float_type),
        (73, exact_nested_float_array, unresolved_outer_float_type),
        (75, short_float_array, one_by_one_nested_f32_type),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let exact_tensor = boolean_tensor3_literal(&mut program, vec![vec![vec![true]]]);
    let two_plane_tensor_type = fixed_boolean_tensor3_type(&mut program, 2, 1, 1);
    let tall_plane_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 2, 1);
    let wide_row_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 1, 2);
    let exact_tensor_type = fixed_boolean_tensor3_type(&mut program, 1, 1, 1);
    let ragged_planes_tensor = boolean_tensor3_literal(
        &mut program,
        vec![vec![vec![true]], vec![vec![false], vec![true]]],
    );
    let ragged_rows_tensor =
        boolean_tensor3_literal(&mut program, vec![vec![vec![true], vec![false, true]]]);
    let numeric_tensor = {
        let planes = program
            .expression_table
            .insert_expression_handles([nested_integer_array]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(planes))
    };
    let computed_tensor = {
        let row = program
            .expression_table
            .insert_expression_handles([computed]);
        let row = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(row));
        let plane = program.expression_table.insert_expression_handles([row]);
        let plane = program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(plane));
        let planes = program.expression_table.insert_expression_handles([plane]);
        program
            .expression_table
            .insert(ExpressionNode::ArrayLiteral(planes))
    };
    let constrained_tensor_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_tensor_plane =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_tensor_row,
                length: FixedArrayLength::Literal(1),
            });
    let constrained_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: constrained_tensor_plane,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_tensor_row =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: bool_type,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(1000),
                    name: Identifier::generated_static("K"),
                },
            });
    let unresolved_row_tensor_plane =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_tensor_row,
                length: FixedArrayLength::Literal(1),
            });
    let unresolved_row_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_row_tensor_plane,
                length: FixedArrayLength::Literal(1),
            });
    let exact_tensor_row = fixed_boolean_array_type(&mut program, 1);
    let unresolved_plane_height =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_tensor_row,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(1001),
                    name: Identifier::generated_static("M"),
                },
            });
    let unresolved_plane_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: unresolved_plane_height,
                length: FixedArrayLength::Literal(1),
            });
    let exact_tensor_plane = fixed_nested_boolean_array_type(&mut program, 1, 1);
    let unresolved_outer_tensor_type =
        program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: exact_tensor_plane,
                length: FixedArrayLength::ConstParameter {
                    symbol: symbol(1002),
                    name: Identifier::generated_static("N"),
                },
            });
    for (position, expression, target) in [
        (76, exact_tensor, two_plane_tensor_type),
        (77, exact_tensor, tall_plane_tensor_type),
        (78, exact_tensor, wide_row_tensor_type),
        (79, ragged_planes_tensor, two_plane_tensor_type),
        (80, ragged_rows_tensor, tall_plane_tensor_type),
        (81, nested_boolean_array, exact_tensor_type),
        (82, numeric_tensor, exact_tensor_type),
        (83, computed_tensor, exact_tensor_type),
        (84, exact_tensor, constrained_tensor_type),
        (85, exact_tensor, unresolved_row_tensor_type),
        (86, exact_tensor, unresolved_plane_tensor_type),
        (87, exact_tensor, unresolved_outer_tensor_type),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }

    let byte_tensor_target = {
        let matrix = fixed_nested_byte_array_type(&mut program, 1, 1);
        wrap_fixed_array_type(&mut program, matrix, 1)
    };
    let noncanonical_byte_tensor = wrap_array_literal(&mut program, [landed_byte_array]);
    let integer_tensor_target = {
        let matrix = fixed_nested_integer_array_type(&mut program, "i8", 1, 1);
        wrap_fixed_array_type(&mut program, matrix, 1)
    };
    let out_of_range_integer_tensor =
        wrap_array_literal(&mut program, [out_of_range_nested_integer_array]);
    let float_tensor_target = {
        let matrix = fixed_nested_float_array_type(&mut program, "f32", 1, 1);
        wrap_fixed_array_type(&mut program, matrix, 1)
    };
    let mismatched_float_tensor = wrap_array_literal(&mut program, [mismatched_nested_float_array]);
    let ragged_depth_four_target = wrap_fixed_array_type(&mut program, two_plane_tensor_type, 1);
    let ragged_depth_four = wrap_array_literal(&mut program, [ragged_planes_tensor]);
    let computed_depth_four_target = wrap_fixed_array_type(&mut program, exact_tensor_type, 1);
    let computed_depth_four = wrap_array_literal(&mut program, [computed_tensor]);
    for (position, expression, target) in [
        (89, noncanonical_byte_tensor, byte_tensor_target),
        (90, out_of_range_integer_tensor, integer_tensor_target),
        (91, mismatched_float_tensor, float_tensor_target),
        (92, ragged_depth_four, ragged_depth_four_target),
        (93, computed_depth_four, computed_depth_four_target),
    ] {
        assert_eq!(
            super::closed_lift_literal_for_representative(&program, expression, target, position),
            Err(RelationPlanError::DirectLiftLiteralTargetMismatch(position)),
        );
    }
}

#[test]
fn direct_lift_literal_rejects_mutable_or_attached_representative_destinations() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(922),
        "LiteralQ",
        symbol(923),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let bool_type = primitive_type(&mut program, "bool");
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let arguments = program
        .expression_table
        .insert_expression_handles([boolean]);
    let call = call_with_arguments(arguments);
    let relation = ExactQuotientRelation {
        quotient_type: quotient,
        quotient_symbol: symbol(922),
        relation_symbol: symbol(923),
    };
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    for (is_mutable, is_self) in [(true, false), (false, true)] {
        let representative = RepresentativeTelescope {
            machine_symbol: symbol(924),
            state_symbol: symbol(925),
            parameters: vec![RepresentativeRuntimeParameter {
                symbol: symbol(926),
                type_reference: bool_type,
                is_mutable,
                is_self,
            }],
            return_type: carrier,
            machine_contracts: HandleSpan::empty(),
            state_contracts: HandleSpan::empty(),
            static_application: RepresentativeStaticApplication {
                lifetime_arguments: Vec::new(),
                bindings: Vec::new(),
            },
        };
        assert_eq!(
            derive_direct_lift_runtime_correspondence(
                &program,
                &Machine::default(),
                &state,
                &call,
                &[InputRelation::ExactEquality(bool_type)],
                relation,
                &representative,
            ),
            Err(RelationPlanError::DirectLiftParameterModeMismatch(0)),
        );
    }
}

#[test]
fn define_does_not_admit_closed_literal_arguments() {
    let mut program = TypedTrees::default();
    let quotient = quotient_type(
        &mut program,
        symbol(927),
        "LiteralQ",
        symbol(928),
        "LiteralR",
    );
    let carrier = carrier_type(&mut program);
    let bool_type = primitive_type(&mut program, "bool");
    let boolean = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let arguments = program
        .expression_table
        .insert_expression_handles([boolean]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: quotient,
        ..Default::default()
    };
    let mut request = push_representative(&mut program, &[(bool_type, false, false)], carrier);
    request.kind = QuotientOperationKind::Define;

    assert_eq!(
        derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request,),
        Err(RelationPlanError::UnresolvedArgumentType(0)),
    );
}

#[test]
fn direct_lift_runtime_duplication_can_exceed_public_arity_without_granting_mutable_execution() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(
        &mut program,
        symbol(897),
        "DuplicatedQ",
        symbol(898),
        "DuplicatedR",
    );
    let carrier = carrier_type(&mut program);
    let public_symbol = symbol(899);
    let value = named_argument(&mut program, "value", public_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([value, value]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            is_mutable: true,
            ..Default::default()
        },
    );
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(900),
        state_symbol: symbol(901),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: symbol(902),
                type_reference: carrier,
                is_mutable: true,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: symbol(903),
                type_reference: carrier,
                is_mutable: true,
                is_self: false,
            },
        ],
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relation = ExactQuotientRelation {
        quotient_type,
        quotient_symbol: symbol(897),
        relation_symbol: symbol(898),
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::Quotient(relation),
    ];

    // This judgment retains correspondence only. Ordinary multiplicity,
    // custody, and call admission must still reject an executable duplicate
    // mutable occurrence at their owning layers.
    let runtime = derive_direct_lift_runtime_correspondence(
        &program,
        &Machine::default(),
        &state,
        &call,
        &input_relations,
        relation,
        &representative,
    )
    .expect("two representative positions may consume one public parameter");
    assert_eq!(runtime.positions.len(), 2);
    assert!(runtime.positions.iter().all(|position| position.source
        == super::DirectLiftArgumentSource::PublicParameter(public_symbol)));
    assert_eq!(
        derive_define_runtime_correspondence(
            &program,
            &Machine::default(),
            &state,
            &call,
            &input_relations,
            relation,
            &representative,
        ),
        Err(RelationPlanError::DefineRuntimeArityMismatch),
    );
}

#[test]
fn direct_lift_runtime_rung_allows_a_zero_argument_representative_to_omit_every_public_parameter() {
    let mut program = TypedTrees::default();
    let result_quotient =
        quotient_type(&mut program, symbol(891), "ResultQ", symbol(892), "ResultR");
    let carrier = carrier_type(&mut program);
    let mut state = State {
        return_type: result_quotient,
        ..Default::default()
    };
    for (parameter, name, type_reference) in [
        (symbol(893), "unused_quotient", result_quotient),
        (symbol(894), "unused_ordinary", carrier),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(895),
        state_symbol: symbol(896),
        parameters: Vec::new(),
        return_type: carrier,
        machine_contracts: HandleSpan::empty(),
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let result_relation = ExactQuotientRelation {
        quotient_type: result_quotient,
        quotient_symbol: symbol(891),
        relation_symbol: symbol(892),
    };
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());

    let runtime = derive_direct_lift_runtime_correspondence(
        &program,
        &Machine::default(),
        &state,
        &call_with_arguments(arguments),
        &[],
        result_relation,
        &representative,
    )
    .expect("zero-argument lift may omit the complete public telescope");
    assert!(runtime.positions.is_empty());
}

#[test]
fn direct_plan_rejects_untyped_adapted_argument() {
    let mut program = TypedTrees::default();
    let result_type = quotient_type(&mut program, symbol(1), "ResultQ", symbol(2), "ResultR");
    let literal = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let adapted = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: literal,
            operator: BinaryOperator::Equal,
            right: literal,
        }));
    let arguments = program
        .expression_table
        .insert_expression_handles([adapted]);
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: result_type,
        ..Default::default()
    };

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &Machine::default(),
            &state,
            &call,
            &request_with_representative(SymbolHandle::invalid()),
        ),
        Err(RelationPlanError::UnresolvedArgumentType(0))
    );
}

#[test]
fn direct_plan_rejects_nonquotient_result() {
    let mut program = TypedTrees::default();
    let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let call = call_with_arguments(arguments);
    let state = State {
        return_type: result_type,
        ..Default::default()
    };

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &Machine::default(),
            &state,
            &call,
            &request_with_representative(SymbolHandle::invalid()),
        ),
        Err(RelationPlanError::ResultIsNotQuotient)
    );
}

#[test]
fn direct_plan_rejects_open_relation_application_without_operation_telescope() {
    let mut program = TypedTrees::default();
    let mut relation = PropositionDefinition {
        symbol: symbol(2),
        name: Identifier::generated_static("IndexedR"),
        ..Default::default()
    };
    program.push_proposition_binder(
        &mut relation,
        PropositionBinder {
            symbol: symbol(3),
            name: Identifier::generated_static("I"),
            kind: PropositionBinderKind::Machine,
            ..Default::default()
        },
    );
    program.push_proposition(relation);
    let quotient_type = quotient_type(&mut program, symbol(1), "IndexedQ", symbol(2), "IndexedR");
    let value_symbol = symbol(4);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: value_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            ..Default::default()
        },
    );

    assert_eq!(
        derive_direct_terminal_plan(
            &program,
            &Machine::default(),
            &state,
            &call,
            &request_with_representative(SymbolHandle::invalid()),
        ),
        Err(RelationPlanError::UnresolvedInputRelationApplication(0))
    );
}

#[test]
fn representative_telescope_rejects_duplicate_state_identity_within_one_machine() {
    let mut program = TypedTrees::default();
    let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut machine = Machine {
        symbol: symbol(90),
        ..Default::default()
    };
    for _ in 0..2 {
        program.push_machine_state(
            &mut machine,
            State {
                symbol: symbol(91),
                return_type: result_type,
                ..Default::default()
            },
        );
    }
    program.push_machine(machine);
    let request = request_with_representative(symbol(91));

    assert_eq!(
        derive_representative_telescope(&program, &request),
        Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly)
    );
}

#[test]
fn representative_termination_retains_only_unconditional_checked_summary() {
    fn telescope_with_summary(
        summary: psi_language_semantics::TerminationGuarantee,
    ) -> (TypedTrees, RepresentativeTelescope) {
        let mut program = TypedTrees::default();
        let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let mut machine = Machine {
            symbol: symbol(94),
            termination_plan: psi_language_semantics::MachineTerminationPlan {
                checked_summary: summary,
                ..Default::default()
            },
            ..Default::default()
        };
        program.push_machine_state(
            &mut machine,
            State {
                symbol: symbol(95),
                return_type: result_type,
                ..Default::default()
            },
        );
        program.push_machine(machine);
        let telescope =
            derive_representative_telescope(&program, &request_with_representative(symbol(95)))
                .expect("exact representative telescope");
        (program, telescope)
    }

    let (program, telescope) =
        telescope_with_summary(psi_language_semantics::TerminationGuarantee::Terminates {
            premises: Vec::new(),
        });
    assert_eq!(
        unconditional_representative_termination(&program, &telescope),
        Some(super::RepresentativeTermination {
            machine_symbol: symbol(94),
            state_symbol: symbol(95),
        })
    );

    let (program, telescope) =
        telescope_with_summary(psi_language_semantics::TerminationGuarantee::Terminates {
            premises: vec![psi_language_semantics::ProgressPremise {
                profile: psi_language_semantics::SemanticDomainId(1),
                subject: psi_language_semantics::ProgressSubject {
                    root: symbol(96),
                    projections: Vec::new(),
                },
            }],
        });
    assert_eq!(
        unconditional_representative_termination(&program, &telescope),
        None,
    );

    let (program, telescope) =
        telescope_with_summary(psi_language_semantics::TerminationGuarantee::NoGuarantee);
    assert_eq!(
        unconditional_representative_termination(&program, &telescope),
        None,
    );
}

#[test]
fn representative_purity_consumes_shared_recursive_effect_summaries() {
    let mut program = TypedTrees::default();
    let result_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut machine = Machine {
        symbol: symbol(96),
        ..Default::default()
    };
    program.push_machine_state(
        &mut machine,
        State {
            symbol: symbol(97),
            return_type: result_type,
            ..Default::default()
        },
    );
    program.push_machine(machine);
    let telescope =
        derive_representative_telescope(&program, &request_with_representative(symbol(97)))
            .expect("exact representative telescope");
    let operational = psi_effects::infer_operational_may(&program);
    let service_reaches = psi_effects::infer_service_reaches(&program, &operational);
    assert_eq!(
        pure_representative_effect(&telescope, &operational, &service_reaches,),
        Some(super::RepresentativePurity {
            machine_symbol: symbol(96),
            state_symbol: symbol(97),
        })
    );

    let mut suspending = operational.clone();
    let machine_handle = suspending
        .machines
        .iter()
        .find_map(|(handle, summary)| (summary.symbol == symbol(96)).then_some(handle))
        .expect("machine effect summary");
    suspending
        .machines
        .get_mut(machine_handle)
        .transitive_may_suspend = true;
    assert_eq!(
        pure_representative_effect(&telescope, &suspending, &service_reaches,),
        None,
    );

    let mut mutable_telescope = telescope.clone();
    mutable_telescope
        .parameters
        .push(RepresentativeRuntimeParameter {
            symbol: symbol(98),
            type_reference: result_type,
            is_mutable: true,
            is_self: false,
        });
    assert_eq!(
        pure_representative_effect(&mutable_telescope, &operational, &service_reaches,),
        None,
    );

    let mut unresolved_program = program.clone();
    let arguments = unresolved_program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let unresolved_call = unresolved_program
        .expression_table
        .insert(ExpressionNode::Call(call_with_arguments(arguments)));
    let machine = unresolved_program.machines()[0].clone();
    let mut state = unresolved_program.machine_states(&machine)[0].clone();
    unresolved_program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(unresolved_call),
    );
    unresolved_program.machine_states_mut(&machine)[0] = state;
    let unresolved_operational = psi_effects::infer_operational_may(&unresolved_program);
    let unresolved_reaches =
        psi_effects::infer_service_reaches(&unresolved_program, &unresolved_operational);
    assert_eq!(
        pure_representative_effect(&telescope, &unresolved_operational, &unresolved_reaches,),
        None,
    );
}

#[test]
fn representative_telescope_retains_closed_static_application_for_substitution() {
    let mut program = TypedTrees::default();
    let request = push_generic_representative_application(&mut program);

    let application = derive_exact_representative_static_application(&program, &request)
        .expect("closed type/const/machine application must retain exact bindings");
    assert_eq!(application.bindings.len(), 3);
    assert_eq!(application.bindings[0].parameter, symbol(622));
    assert_eq!(
        application.bindings[0].kind,
        RepresentativeStaticBindingKind::Type
    );
    assert_eq!(application.bindings[1].parameter, symbol(623));
    assert_eq!(
        application.bindings[1].kind,
        RepresentativeStaticBindingKind::Const
    );
    assert_eq!(application.bindings[2].parameter, symbol(624));
    assert_eq!(
        application.bindings[2].kind,
        RepresentativeStaticBindingKind::Machine
    );

    let telescope = derive_representative_telescope(&program, &request)
        .expect("a closed static application is retained on the telescope");
    assert_eq!(telescope.static_application, application);
}

#[test]
fn immutable_telescope_substitution_covers_type_const_and_machine_binders() {
    let mut program = TypedTrees::default();
    let request = push_generic_representative_application(&mut program);
    let bindings = derive_representative_telescope(&program, &request)
        .expect("closed application")
        .static_application
        .bindings;

    let type_template = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
        });
    let type_concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(600),
            name: Identifier::generated_static("StaticType"),
        });
    assert!(substituted_type_matches(
        &program,
        type_template,
        type_concrete,
        &bindings,
    ));

    let machine_template = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(624),
            name: Identifier::generated_static("F"),
        });
    let machine_concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(611),
            name: Identifier::generated_static("selected"),
        });
    assert!(substituted_type_matches(
        &program,
        machine_template,
        machine_concrete,
        &bindings,
    ));

    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let array_template = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(623),
                name: Identifier::generated_static("N"),
            },
        });
    let array_concrete = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::Literal(0),
        });
    assert!(substituted_type_matches(
        &program,
        array_template,
        array_concrete,
        &bindings,
    ));
}

#[test]
fn immutable_telescope_substitution_rejects_type_and_const_near_misses() {
    let mut program = TypedTrees::default();
    let request = push_generic_representative_application(&mut program);
    let bindings = derive_representative_telescope(&program, &request)
        .expect("closed application")
        .static_application
        .bindings;
    let type_template = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(622),
            name: Identifier::generated_static("T"),
        });
    program.push_data_definition(DataDefinition {
        symbol: symbol(601),
        name: Identifier::generated_static("OtherType"),
        ..Default::default()
    });
    let other_type = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(601),
            name: Identifier::generated_static("OtherType"),
        });
    assert!(!substituted_type_matches(
        &program,
        type_template,
        other_type,
        &bindings,
    ));

    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let array_template = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(623),
                name: Identifier::generated_static("N"),
            },
        });
    let wrong_length = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::Literal(1),
        });
    assert!(!substituted_type_matches(
        &program,
        array_template,
        wrong_length,
        &bindings,
    ));
    let stale_length = program
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: unit,
            length: FixedArrayLength::ConstParameter {
                symbol: symbol(623),
                name: Identifier::generated_static("N"),
            },
        });
    assert!(!substituted_type_matches(
        &program,
        array_template,
        stale_length,
        &bindings,
    ));
}

#[test]
fn representative_precondition_partition_tracks_exact_dependent_fact_locations() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient_parameter = symbol(700);
    let fixed_parameter = symbol(701);
    let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
    let fixed_name = named_argument(&mut program, "fixed", fixed_parameter);
    let mixed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: quotient_name,
            operator: BinaryOperator::Equal,
            right: fixed_name,
        }));
    let machine_facts = program.proof_facts.insert_many([
        ProofFact::Expression(quotient_name),
        ProofFact::Expression(mixed),
    ]);
    let state_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(fixed_name)]);
    let machine_contracts = program.signature_contracts.insert_many([
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: machine_facts,
            ..Default::default()
        },
        SignatureContract {
            kind: SignatureContractKind::Ensures,
            facts: state_facts,
            ..Default::default()
        },
    ]);
    let state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: state_facts,
        ..Default::default()
    }]);
    let telescope = RepresentativeTelescope {
        machine_symbol: symbol(710),
        state_symbol: symbol(711),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: quotient_parameter,
                type_reference: unit,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: fixed_parameter,
                type_reference: unit,
                is_mutable: false,
                is_self: false,
            },
        ],
        return_type: unit,
        machine_contracts,
        state_contracts,
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let relations = [
        InputRelation::Quotient(ExactQuotientRelation {
            quotient_type: unit,
            quotient_symbol: symbol(720),
            relation_symbol: symbol(721),
        }),
        InputRelation::ExactEquality(unit),
    ];

    let partition = derive_representative_precondition_partition(&program, &relations, &telescope)
        .expect("all value identities are exact");
    assert_eq!(
        partition.dependent,
        vec![
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 0,
            },
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 1,
            },
        ]
    );
    assert_eq!(
        partition.fixed,
        vec![RepresentativeContractFactLocation {
            owner: RepresentativeContractOwner::State,
            contract_position: 0,
            fact_position: 0,
        }]
    );
}

#[test]
fn representative_precondition_partition_rejects_unresolved_value_identity() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient_parameter = symbol(700);
    let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
    let unresolved = named_argument(&mut program, "unknown", SymbolHandle::invalid());
    let mixed = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: quotient_name,
            operator: BinaryOperator::Equal,
            right: unresolved,
        }));
    let facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(mixed)]);
    let machine_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts,
        ..Default::default()
    }]);
    let telescope = RepresentativeTelescope {
        machine_symbol: symbol(710),
        state_symbol: symbol(711),
        parameters: vec![RepresentativeRuntimeParameter {
            symbol: quotient_parameter,
            type_reference: unit,
            is_mutable: false,
            is_self: false,
        }],
        return_type: unit,
        machine_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };

    assert_eq!(
        derive_representative_precondition_partition(
            &program,
            &[InputRelation::Quotient(ExactQuotientRelation {
                quotient_type: unit,
                quotient_symbol: symbol(720),
                relation_symbol: symbol(721),
            })],
            &telescope,
        ),
        Err(RelationPlanError::PreconditionDependencyUnresolved)
    );
}

#[test]
fn public_precondition_partition_follows_the_runtime_permutation() {
    let mut program = TypedTrees::default();
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient_type = quotient_type(
        &mut program,
        symbol(732),
        "PartitionQ",
        symbol(733),
        "PartitionR",
    );
    let quotient_parameter = symbol(730);
    let ordinary_parameter = symbol(731);
    let omitted_quotient_parameter = symbol(736);
    let omitted_ordinary_parameter = symbol(737);
    let quotient_name = named_argument(&mut program, "quotient", quotient_parameter);
    let ordinary_name = named_argument(&mut program, "ordinary", ordinary_parameter);
    let omitted_quotient_name =
        named_argument(&mut program, "omitted_quotient", omitted_quotient_parameter);
    let omitted_ordinary_name =
        named_argument(&mut program, "omitted_ordinary", omitted_ordinary_parameter);
    let machine_facts = program.proof_facts.insert_many([
        ProofFact::Expression(quotient_name),
        ProofFact::Expression(omitted_quotient_name),
    ]);
    let state_facts = program.proof_facts.insert_many([
        ProofFact::Expression(ordinary_name),
        ProofFact::Expression(omitted_ordinary_name),
    ]);
    let mut machine = Machine::default();
    program.push_machine_contract(
        &mut machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: machine_facts,
            ..Default::default()
        },
    );
    let mut state = State::default();
    for (parameter, name, type_reference) in [
        (quotient_parameter, "quotient", quotient_type),
        (ordinary_parameter, "ordinary", unit),
        (
            omitted_quotient_parameter,
            "omitted_quotient",
            quotient_type,
        ),
        (omitted_ordinary_parameter, "omitted_ordinary", unit),
    ] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    program.push_state_contract(
        &mut state,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: state_facts,
            ..Default::default()
        },
    );
    let relations = [
        InputRelation::ExactEquality(unit),
        InputRelation::Quotient(ExactQuotientRelation {
            quotient_type,
            quotient_symbol: symbol(732),
            relation_symbol: symbol(733),
        }),
    ];

    let runtime_positions = [
        super::DefineRuntimePosition {
            public_parameter: ordinary_parameter,
            representative_parameter: symbol(734),
        },
        super::DefineRuntimePosition {
            public_parameter: quotient_parameter,
            representative_parameter: symbol(735),
        },
    ];
    let partition = derive_public_precondition_partition(
        &program,
        &machine,
        &state,
        &relations,
        &runtime_positions,
    )
    .expect("public parameter identities are exact");
    assert_eq!(
        partition.dependent,
        vec![
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 0,
            },
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::Machine,
                contract_position: 0,
                fact_position: 1,
            },
        ]
    );
    assert_eq!(
        partition.fixed,
        vec![
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::State,
                contract_position: 0,
                fact_position: 0,
            },
            RepresentativeContractFactLocation {
                owner: RepresentativeContractOwner::State,
                contract_position: 0,
                fact_position: 1,
            },
        ]
    );
}

#[test]
fn define_preconditions_require_one_exact_alpha_renamed_bijection() {
    let mut program = TypedTrees::default();
    let public_left = symbol(740);
    let public_right = symbol(741);
    let representative_left = symbol(742);
    let representative_right = symbol(743);
    let public_fact = {
        let left = named_argument(&mut program, "public_left", public_left);
        let right = named_argument(&mut program, "public_right", public_right);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Less,
                right,
            }))
    };
    let representative_fact = {
        let left = named_argument(&mut program, "representative_left", representative_left);
        let right = named_argument(&mut program, "representative_right", representative_right);
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator: BinaryOperator::Less,
                right,
            }))
    };
    let public_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(public_fact)]);
    let representative_facts = program
        .proof_facts
        .insert_many([ProofFact::Expression(representative_fact)]);
    let mut public_machine = Machine::default();
    program.push_machine_contract(
        &mut public_machine,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: public_facts,
            ..Default::default()
        },
    );
    let public_state = State::default();
    let mut representative_contracts = HandleSpan::empty();
    program.signature_contracts.append_to_span(
        &mut representative_contracts,
        SignatureContract {
            kind: SignatureContractKind::Requires,
            facts: representative_facts,
            ..Default::default()
        },
    );
    let location = RepresentativeContractFactLocation {
        owner: RepresentativeContractOwner::Machine,
        contract_position: 0,
        fact_position: 0,
    };
    let partition = super::RepresentativePreconditionPartition {
        dependent: vec![location],
        fixed: Vec::new(),
    };
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(744),
        state_symbol: symbol(745),
        parameters: Vec::new(),
        return_type: TypeReferenceHandle::invalid(),
        machine_contracts: representative_contracts,
        state_contracts: HandleSpan::empty(),
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    let correspondence = super::DefineRuntimeCorrespondence {
        positions: vec![
            super::DefineRuntimePosition {
                public_parameter: public_left,
                representative_parameter: representative_left,
            },
            super::DefineRuntimePosition {
                public_parameter: public_right,
                representative_parameter: representative_right,
            },
        ],
    };

    let exact = derive_define_precondition_correspondence(
        &program,
        &public_machine,
        &public_state,
        &representative,
        &partition,
        &partition,
        &correspondence,
    )
    .expect("parameter names may differ while exact positions agree");
    assert_eq!(exact.dependent.len(), 1);

    let redirected = super::DefineRuntimeCorrespondence {
        positions: vec![
            super::DefineRuntimePosition {
                public_parameter: public_left,
                representative_parameter: representative_right,
            },
            super::DefineRuntimePosition {
                public_parameter: public_right,
                representative_parameter: representative_left,
            },
        ],
    };
    assert_eq!(
        derive_define_precondition_correspondence(
            &program,
            &public_machine,
            &public_state,
            &representative,
            &partition,
            &partition,
            &redirected,
        ),
        Err(RelationPlanError::DefinePreconditionMismatch)
    );
}

#[test]
fn define_correspondence_applies_closed_representative_type_substitution() {
    let mut program = TypedTrees::default();
    let mut request = push_generic_representative_application(&mut program);
    request.kind = QuotientOperationKind::Define;
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(600),
            name: Identifier::generated_static("StaticType"),
        });
    program.push_proposition(PropositionDefinition {
        symbol: symbol(2),
        name: Identifier::generated_static("ExactR"),
        ..Default::default()
    });
    program.push_data_definition(DataDefinition {
        symbol: symbol(1),
        name: Identifier::generated_static("ExactQ"),
        quotient: Some(QuotientDefinition {
            carrier,
            relation: vec![Identifier::generated_static("ExactR")],
            relation_symbol: symbol(2),
            equivalence: None,
        }),
        ..Default::default()
    });
    let quotient = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbol(1),
            name: Identifier::generated_static("ExactQ"),
        });
    let public_symbol = symbol(3);
    let argument = named_argument(&mut program, "value", public_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([argument]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: public_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient,
            ..Default::default()
        },
    );

    let plan = derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request)
        .expect("closed T := StaticType must instantiate the runtime telescope");
    assert_eq!(
        plan.representative_precondition,
        Some(super::RepresentativePreconditionPartition {
            dependent: Vec::new(),
            fixed: Vec::new(),
        })
    );
    assert_eq!(
        plan.public_precondition,
        Some(super::RepresentativePreconditionPartition {
            dependent: Vec::new(),
            fixed: Vec::new(),
        })
    );
    assert_eq!(
        plan.define_precondition_correspondence,
        Some(super::DefinePreconditionCorrespondence {
            dependent: Vec::new(),
        })
    );
    assert_eq!(
        plan.define_correspondence
            .expect("define correspondence")
            .positions,
        vec![super::DefineRuntimePosition {
            public_parameter: public_symbol,
            representative_parameter: symbol(625),
        }]
    );
}

#[test]
fn representative_static_application_rejects_const_category_near_miss() {
    let mut program = TypedTrees::default();
    let mut request = push_generic_representative_application(&mut program);
    let application = request
        .representative_operation
        .application
        .as_mut()
        .expect("generic application");
    application.arguments[1] = application.arguments[0].clone();

    assert_eq!(
        derive_exact_representative_static_application(&program, &request),
        Err(RelationPlanError::RepresentativeStaticArgumentCategoryMismatch(1))
    );
}

#[test]
fn define_runtime_correspondence_rejects_reordered_public_parameters() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let carrier_type = carrier_type(&mut program);
    let left_symbol = symbol(3);
    let right_symbol = symbol(4);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let arguments = program
        .expression_table
        .insert_expression_handles([right, left]);
    let call = call_with_arguments(arguments);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    for (parameter_symbol, name) in [(left_symbol, "left"), (right_symbol, "right")] {
        program.push_state_parameter(
            &mut state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference: quotient_type,
                ..Default::default()
            },
        );
    }
    let mut request = push_representative(
        &mut program,
        &[(carrier_type, false, false), (carrier_type, false, false)],
        carrier_type,
    );
    request.kind = QuotientOperationKind::Define;

    assert_eq!(
        derive_direct_terminal_plan(&program, &Machine::default(), &state, &call, &request,),
        Err(RelationPlanError::DefineArgumentOrderMismatch(0))
    );
}

#[test]
fn derived_direct_terminal_plan_remains_non_executable() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let value_symbol = symbol(3);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let mut call = call_with_arguments(arguments);
    let carrier_type = carrier_type(&mut program);
    // Attached and free operations share the normalized positional form:
    // the representative receiver occupies position zero without forcing
    // the public wrapper parameter to be spelled `self`.
    let mut request =
        push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
    request.kind = QuotientOperationKind::Define;
    call.quotient_operation = Some(request);
    let call = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: value_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            ..Default::default()
        },
    );
    program
        .statement_table
        .push_statement(&mut state.statement_nodes, StatementNode::Expression(call));
    let mut machine = Machine::default();
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("compiler-derived direct-terminal relations RA=[0:")
    );
    assert!(diagnostics[0].message.contains("RR="));
    assert!(diagnostics[0].message.contains("define-runtime=[0]"));
    assert!(diagnostics[0].message.contains("Q=[dependent:0, fixed:0]"));
    assert!(diagnostics[0].message.contains("P=[dependent:0, fixed:0]"));
    assert!(diagnostics[0].message.contains("Q<->P=[dependent:0]"));
    assert!(diagnostics[0].message.contains(
        "theorem-schema=[parameters:2, relations:1, legality:0, applications:2, conclusion:1]"
    ));
    assert!(
        diagnostics[0]
            .message
            .contains("exact selected theorem schema verification")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("checked pure representative effect summary")
    );
    assert!(!diagnostics[0].message.contains("the effect fence"));
    assert!(
        diagnostics[0]
            .message
            .contains("one unchanged state-fallthrough result edge through the exact result root")
    );
    assert!(
        diagnostics[0]
            .message
            .contains("executable quotient operations are not admitted")
    );
}

#[test]
fn immutable_alias_fallthrough_requires_an_exact_immutable_chain() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let first_symbol = symbol(10);
    let second_symbol = symbol(11);
    let first_name = named_argument(&mut program, "first", first_symbol);
    let second_name = named_argument(&mut program, "second", second_symbol);
    let mut state = State {
        return_type: quotient_type,
        ..Default::default()
    };
    for local in [
        TableLocalData {
            symbol: first_symbol,
            name: Identifier::generated_static("first"),
            type_reference: quotient_type,
            initial_value: request,
            is_mutable: false,
        },
        TableLocalData {
            symbol: second_symbol,
            name: Identifier::generated_static("second"),
            type_reference: quotient_type,
            initial_value: first_name,
            is_mutable: false,
        },
    ] {
        program
            .statement_table
            .push_statement(&mut state.statement_nodes, StatementNode::LocalData(local));
    }
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(second_name),
    );

    assert_eq!(
        immutable_alias_fallthrough_root(&program, &state),
        Some(super::ImmutableAliasFallthroughRoot {
            request_expression: request,
            alias_count: 2,
        })
    );

    let drifted_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    if let StatementNode::LocalData(first) = &mut program
        .statement_table
        .statements_mut(state.statement_nodes)[0]
    {
        first.type_reference = drifted_type;
    }
    assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);

    if let StatementNode::LocalData(first) = &mut program
        .statement_table
        .statements_mut(state.statement_nodes)[0]
    {
        first.type_reference = quotient_type;
        first.is_mutable = true;
    }
    assert_eq!(immutable_alias_fallthrough_root(&program, &state), None);
}

#[test]
fn complete_result_flow_requires_one_exact_machine_state() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        symbol: symbol(31),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    let mut machine = Machine {
        symbol: symbol(30),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);

    let root = {
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let root = fallthrough_result_root(&program, state).expect("exact result root");
        assert_eq!(
            complete_single_state_result_flow(&program, machine, state, root),
            Some(super::CompleteSingleStateResultFlow {
                machine_symbol: symbol(30),
                state_symbol: symbol(31),
                root,
            })
        );
        root
    };

    let mut machine = program.machines()[0].clone();
    program.push_machine_state(
        &mut machine,
        State {
            symbol: symbol(32),
            return_type,
            ..Default::default()
        },
    );
    program.machines_mut()[0] = machine;
    let machine = &program.machines()[0];
    let state = &program.machine_states(&machine)[0];
    assert_eq!(
        complete_single_state_result_flow(&program, &machine, state, root),
        None
    );
}

#[test]
fn complete_result_flow_rejects_a_transition_before_fallthrough() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let mut state = State {
        symbol: symbol(41),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition::default()),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    let mut machine = Machine {
        symbol: symbol(40),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let machine = &program.machines()[0];
    let state = &program.machine_states(&machine)[0];
    let root = fallthrough_result_root(&program, state).expect("fallthrough edge still exists");

    assert_eq!(
        complete_single_state_result_flow(&program, &machine, state, root),
        None
    );
}

#[test]
fn complete_result_flow_accepts_exact_finite_state_forwarding() {
    let mut program = TypedTrees::default();
    let return_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));

    let mut result_state = State {
        symbol: symbol(52),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut result_state.statement_nodes,
        StatementNode::Expression(request),
    );

    let target = program.statement_table.insert_transition_target(
        psi_typed_trees::statement::TransitionTargetNode::Named {
            path: psi_typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(52),
                symbol: symbol(52),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
        },
    );
    let mut forwarding_state = State {
        symbol: symbol(51),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut forwarding_state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            ..Default::default()
        }),
    );
    let mut machine = Machine {
        symbol: symbol(50),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, forwarding_state);
    program.push_machine_state(&mut machine, result_state);
    program.push_machine(machine);

    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    let root = fallthrough_result_root(&program, result_state).expect("exact result root");
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        Some(super::CompleteStateForwardingResultFlow {
            machine_symbol: symbol(50),
            forwarding_edges: vec![super::StateForwardingEdge {
                source_state_symbol: symbol(51),
                target_state_symbol: symbol(52),
            }],
            result_state_symbol: symbol(52),
            root,
        })
    );

    let intermediate_target = program.statement_table.insert_transition_target(
        psi_typed_trees::statement::TransitionTargetNode::Named {
            path: psi_typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(53),
                symbol: symbol(53),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
        },
    );
    let forwarding_span = program.machine_states(&program.machines()[0])[0].statement_nodes;
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.target = intermediate_target;
    let mut intermediate_state = State {
        symbol: symbol(53),
        return_type,
        ..Default::default()
    };
    program.statement_table.push_statement(
        &mut intermediate_state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            ..Default::default()
        }),
    );
    let mut expanded_machine = program.machines()[0].clone();
    program.push_machine_state(&mut expanded_machine, intermediate_state);
    program.machines_mut()[0] = expanded_machine;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        Some(super::CompleteStateForwardingResultFlow {
            machine_symbol: symbol(50),
            forwarding_edges: vec![
                super::StateForwardingEdge {
                    source_state_symbol: symbol(51),
                    target_state_symbol: symbol(53),
                },
                super::StateForwardingEdge {
                    source_state_symbol: symbol(53),
                    target_state_symbol: symbol(52),
                },
            ],
            result_state_symbol: symbol(52),
            root,
        })
    );

    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.continuation = target;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );

    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(forwarding_span)
    else {
        panic!("one forwarding transition")
    };
    transition.continuation = psi_typed_trees::statement::TransitionTargetHandle::invalid();

    let cycle_target = program.statement_table.insert_transition_target(
        psi_typed_trees::statement::TransitionTargetNode::Named {
            path: psi_typed_trees::statement::TableNamePath {
                members: HandleSpan::empty(),
                head_symbol: symbol(51),
                symbol: symbol(51),
            },
            arguments: HandleSpan::empty(),
            evidence_arguments: Box::default(),
        },
    );
    let intermediate_span = program.machine_states(&program.machines()[0])[2].statement_nodes;
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(intermediate_span)
    else {
        panic!("one intermediate transition")
    };
    transition.target = cycle_target;
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );
    let [StatementNode::Transition(transition)] =
        program.statement_table.statements_mut(intermediate_span)
    else {
        panic!("one intermediate transition")
    };
    transition.target = target;

    let mut duplicate_owner = Machine {
        symbol: symbol(60),
        ..Default::default()
    };
    program.push_machine_state(
        &mut duplicate_owner,
        State {
            symbol: symbol(52),
            return_type,
            ..Default::default()
        },
    );
    program.push_machine(duplicate_owner);
    let machine = &program.machines()[0];
    let result_state = &program.machine_states(machine)[1];
    assert_eq!(
        complete_state_forwarding_result_flow(&program, machine, result_state, root,),
        None,
    );
}

#[test]
fn derived_immutable_alias_fallthrough_remains_non_executable() {
    let mut program = TypedTrees::default();
    let quotient_type = quotient_type(&mut program, symbol(1), "ExactQ", symbol(2), "ExactR");
    let value_symbol = symbol(3);
    let value = named_argument(&mut program, "value", value_symbol);
    let arguments = program.expression_table.insert_expression_handles([value]);
    let mut call = call_with_arguments(arguments);
    let carrier_type = carrier_type(&mut program);
    let mut request =
        push_representative(&mut program, &[(carrier_type, true, false)], carrier_type);
    request.kind = QuotientOperationKind::Define;
    call.quotient_operation = Some(request);
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let result_symbol = symbol(4);
    let result = named_argument(&mut program, "result", result_symbol);
    let mut state = State {
        symbol: symbol(5),
        return_type: quotient_type,
        ..Default::default()
    };
    program.push_state_parameter(
        &mut state,
        StateParameter {
            symbol: value_symbol,
            name: Identifier::generated_static("value"),
            type_reference: quotient_type,
            ..Default::default()
        },
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::LocalData(TableLocalData {
            symbol: result_symbol,
            name: Identifier::generated_static("result"),
            type_reference: quotient_type,
            initial_value: request,
            is_mutable: false,
        }),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(result),
    );
    let mut machine = Machine {
        symbol: symbol(6),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("compiler-derived immutable-alias fallthrough relations")
    );
    assert!(
            diagnostics[0]
                .message
                .contains("complete transition-free single-state normal-result coverage through 1 exact immutable result alias")
        );
    assert!(
        diagnostics[0]
            .message
            .contains("executable quotient operations are not admitted")
    );
}

#[test]
fn nonterminal_expression_request_cannot_claim_direct_result_flow() {
    let mut program = TypedTrees::default();
    let arguments = program
        .expression_table
        .insert_expression_handles(std::iter::empty());
    let mut call = call_with_arguments(arguments);
    call.quotient_operation = Some(request_with_representative(SymbolHandle::invalid()));
    let request = program.expression_table.insert(ExpressionNode::Call(call));
    let terminal = program
        .expression_table
        .insert(ExpressionNode::Boolean(true));
    let mut state = State::default();
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(request),
    );
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Expression(terminal),
    );
    let mut machine = Machine::default();
    program.push_machine_state(&mut machine, state);
    program.push_machine(machine);
    let mut diagnostics = Vec::new();

    super::super::reject_quotient_operation_requests(&program, &mut diagnostics);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("retains its exact representative operation and selected theorem")
    );
    assert!(
        !diagnostics[0]
            .message
            .contains("compiler-derived direct-terminal relations")
    );
    assert!(
        !diagnostics[0]
            .message
            .contains("unchanged state-fallthrough result root")
    );
}
