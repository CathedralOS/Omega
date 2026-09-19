//! Fixtures shared by the relation plan tests: quotient, carrier and
//! primitive types, fixed and canonical byte layouts, selected theorems and
//! representative builders.

mod arithmetic_implications;
mod definitions;
mod direct_lifts;
mod proof_facts;
mod representatives;
mod selected_theorems;
mod transport;

use super::correspondence_certificate::DirectLiftPreconditionProof;
use super::theorem::SelectedTheoremTelescope;
use super::theorem_schema::{
    TheoremContractFactLocation, TheoremContractOwner, derive_expected_theorem_schema,
};
use super::theorem_schema_verification::verify_selected_theorem_schema;
use super::transport_schema::verify_forward_precondition_transport_schema;
use crate::proof_contracts::quotients::relation_plan::{
    ExactQuotientRelation, InputRelation, RelationPlanError, RepresentativeContractFactLocation,
    RepresentativeRuntimeParameter, RepresentativeStaticApplication, RepresentativeStaticBinding,
    RepresentativeStaticBindingKind, RepresentativeTelescope,
    derive_direct_lift_public_precondition_partition, derive_representative_precondition_partition,
};
use arena::HandleSpan;
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{FloatLiteral, IntegerLanding, IntegerLiteral, LandedIntegerType};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{
    DataDefinition, MachineParameterContract, QuotientDefinition, TypeParameter, TypeParameterKind,
};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, QuotientOperationKind,
    QuotientOperationRequest, QuotientTheoremRole, QuotientTheoremSelection, StaticMachineArgument,
    StaticSymbolApplication, TableBinaryExpression, TableCallExpression, TableNamePath,
};
use typed_trees::machine::Machine;
use typed_trees::name::Identifier;
use typed_trees::proposition::{PropositionApplication, PropositionDefinition};
use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
use typed_trees::state::State;
use typed_trees::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

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
    access: language_core::ReferenceAccess,
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
) -> typed_trees::expression::ExpressionHandle {
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
    arguments: HandleSpan<typed_trees::expression::ExpressionHandle>,
) -> TableCallExpression {
    TableCallExpression {
        receiver: ExpressionHandle::invalid(),
        target_symbol: SymbolHandle::invalid(),
        target: Identifier::generated_static("lift"),
        static_machine_parameter: symbols::SymbolHandle::invalid(),
        static_requirement_dispatch: None,
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
        type_reference: Default::default(),
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
        theorem_evidence: vec![QuotientTheoremSelection {
            role: QuotientTheoremRole::Congruence,
            application: static_argument("ExactRespect"),
        }]
        .into_boxed_slice(),
    }
}

fn push_selected_theorem(program: &mut TypedTrees) -> StaticMachineArgument {
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let mut theorem = Machine {
        symbol: symbol(700),
        name: Identifier::generated_static("selected_theorem"),
        termination_plan: language_semantics::MachineTerminationPlan {
            checked_summary: language_semantics::TerminationGuarantee::Terminates {
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
    supply_mode: language_semantics::MachineSupplyMode,
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
    request.theorem_evidence[0].application = push_selected_theorem(program);
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
        type_reference: Default::default(),
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
    request.theorem_evidence[0].application = push_selected_theorem(program);
    request
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
                static_machine_parameter: symbols::SymbolHandle::invalid(),
                static_requirement_dispatch: None,
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
                cause: typed_trees::signature::CrashCause::Trap,
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

#[derive(Clone, Copy)]
enum TransportSchemaMutation {
    Exact,
    MissingPremise,
    ExtraPremise,
    ReorderedPremise,
    SideMajorPremiseOrder,
    MissingConclusion,
    WrongConclusion,
    ReboundSharedConclusion,
    NamedEvidenceLane,
    UnexpectedContractKind,
    WrongParameterType,
}

struct TransportSchemaFixture {
    program: TypedTrees,
    public_machine: Machine,
    public_state: State,
    representative: RepresentativeTelescope,
    theorem: SelectedTheoremTelescope,
    runtime: super::DirectLiftRuntimeCorrespondence,
    expected_congruence: super::theorem_schema::ExpectedTheoremSchema,
    public_partition: super::RepresentativePreconditionPartition,
    representative_partition: super::RepresentativePreconditionPartition,
}

fn transport_schema_fixture(mutation: TransportSchemaMutation) -> TransportSchemaFixture {
    let mut program = TypedTrees::default();
    let carrier = carrier_type(&mut program);
    let unit = program.type_reference_table.insert(TypeReferenceNode::Unit);
    let quotient = quotient_type_over(
        &mut program,
        symbol(900),
        "TransportQ",
        symbol(901),
        "TransportR",
        carrier,
    );
    let relation = ExactQuotientRelation {
        quotient_type: quotient,
        quotient_symbol: symbol(900),
        relation_symbol: symbol(901),
    };

    let representative_value_symbol = symbol(902);
    let representative_shared_symbol = symbol(903);
    let representative_value = named_argument(
        &mut program,
        "representative_value",
        representative_value_symbol,
    );
    let representative_shared = named_argument(
        &mut program,
        "representative_shared",
        representative_shared_symbol,
    );
    let representative_facts = program.proof_facts.insert_many([
        ProofFact::Expression(representative_value),
        ProofFact::Expression(representative_shared),
    ]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(904),
        state_symbol: symbol(905),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_value_symbol,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_shared_symbol,
                type_reference: unit,
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
    let expected_congruence = derive_expected_theorem_schema(
        &program,
        &[
            InputRelation::Quotient(relation),
            InputRelation::ExactEquality(unit),
        ],
        relation,
        &representative,
    )
    .expect("exact transport parameter roster");

    let public_value_symbol = symbol(906);
    let public_shared_symbol = symbol(907);
    let public_value = named_argument(&mut program, "public_value", public_value_symbol);
    let public_shared = named_argument(&mut program, "public_shared", public_shared_symbol);
    let public_facts = program.proof_facts.insert_many([
        ProofFact::Expression(public_value),
        ProofFact::Expression(public_shared),
    ]);
    let public_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: public_facts,
        ..Default::default()
    }]);
    let public_machine = Machine {
        contracts: public_contracts,
        ..Default::default()
    };
    let mut public_state = State::default();
    for (parameter_symbol, name, type_reference) in [
        (public_value_symbol, "public_value", quotient),
        (public_shared_symbol, "public_shared", unit),
    ] {
        program.push_state_parameter(
            &mut public_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    let runtime = super::DirectLiftRuntimeCorrespondence {
        positions: vec![
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_value_symbol),
                representative_parameter: representative_value_symbol,
            },
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_shared_symbol),
                representative_parameter: representative_shared_symbol,
            },
        ],
    };
    let input_relations = [
        InputRelation::Quotient(relation),
        InputRelation::ExactEquality(unit),
    ];
    let public_partition = derive_direct_lift_public_precondition_partition(
        &program,
        &public_machine,
        &public_state,
        &input_relations,
        &runtime,
    )
    .expect("exact public transport partition");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("exact representative transport partition");

    let left_symbol = symbol(908);
    let right_symbol = symbol(909);
    let shared_symbol = symbol(910);
    let left = named_argument(&mut program, "left", left_symbol);
    let right = named_argument(&mut program, "right", right_symbol);
    let shared = named_argument(&mut program, "shared", shared_symbol);
    let mut requires = vec![
        ProofFact::Expression(left),
        ProofFact::Expression(right),
        ProofFact::Expression(shared),
        ProofFact::Expression(shared),
    ];
    match mutation {
        TransportSchemaMutation::MissingPremise => {
            requires.pop();
        }
        TransportSchemaMutation::ExtraPremise => {
            requires.push(ProofFact::Expression(shared));
        }
        TransportSchemaMutation::ReorderedPremise => requires.swap(0, 1),
        TransportSchemaMutation::SideMajorPremiseOrder => {
            requires = vec![
                ProofFact::Expression(left),
                ProofFact::Expression(shared),
                ProofFact::Expression(right),
                ProofFact::Expression(shared),
            ];
        }
        _ => {}
    }
    let requires = program.proof_facts.insert_many(requires);
    let mut conclusions = vec![
        ProofFact::Expression(left),
        ProofFact::Expression(right),
        ProofFact::Expression(shared),
        ProofFact::Expression(shared),
    ];
    match mutation {
        TransportSchemaMutation::MissingConclusion => {
            conclusions.pop();
        }
        TransportSchemaMutation::WrongConclusion => {
            conclusions[2] = ProofFact::Expression(left);
        }
        TransportSchemaMutation::ReboundSharedConclusion => {
            conclusions[3] = ProofFact::Expression(left);
        }
        _ => {}
    }
    let conclusions = program.proof_facts.insert_many(conclusions);
    let theorem_machine_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        binding: matches!(mutation, TransportSchemaMutation::NamedEvidenceLane)
            .then(|| Identifier::generated_static("transport_evidence")),
        facts: requires,
        ..Default::default()
    }]);
    let theorem_state_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: if matches!(mutation, TransportSchemaMutation::UnexpectedContractKind) {
            SignatureContractKind::Crashes {
                cause: typed_trees::signature::CrashCause::Trap,
            }
        } else {
            SignatureContractKind::Ensures
        },
        facts: conclusions,
        ..Default::default()
    }]);
    let mut theorem_machine = Machine {
        symbol: symbol(911),
        contracts: theorem_machine_contracts,
        ..Default::default()
    };
    let mut theorem_state = State {
        symbol: symbol(912),
        return_type: unit,
        contracts: theorem_state_contracts,
        ..Default::default()
    };
    for (position, (parameter_symbol, name, mut type_reference)) in [
        (left_symbol, "left", carrier),
        (right_symbol, "right", carrier),
        (shared_symbol, "shared", unit),
    ]
    .into_iter()
    .enumerate()
    {
        if position == 2 && matches!(mutation, TransportSchemaMutation::WrongParameterType) {
            type_reference = carrier;
        }
        program.push_state_parameter(
            &mut theorem_state,
            StateParameter {
                symbol: parameter_symbol,
                name: Identifier::generated_static(name),
                type_reference,
                ..Default::default()
            },
        );
    }
    program.push_machine_state(&mut theorem_machine, theorem_state);
    program.push_machine(theorem_machine);
    let theorem = SelectedTheoremTelescope {
        machine_symbol: symbol(911),
        state_symbol: symbol(912),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: left_symbol,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: right_symbol,
                type_reference: carrier,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: shared_symbol,
                type_reference: unit,
                is_mutable: false,
                is_self: false,
            },
        ],
        machine_contracts: theorem_machine_contracts,
        state_contracts: theorem_state_contracts,
        static_application: RepresentativeStaticApplication {
            lifetime_arguments: Vec::new(),
            bindings: Vec::new(),
        },
    };
    TransportSchemaFixture {
        program,
        public_machine,
        public_state,
        representative,
        theorem,
        runtime,
        expected_congruence,
        public_partition,
        representative_partition,
    }
}

fn verify_transport_fixture(
    fixture: &TransportSchemaFixture,
) -> Result<super::transport_schema::VerifiedForwardPreconditionTransportSchema, RelationPlanError>
{
    verify_forward_precondition_transport_schema(
        &fixture.program,
        &fixture.public_machine,
        &fixture.public_state,
        &fixture.representative,
        &fixture.theorem,
        &fixture.runtime,
        &fixture.expected_congruence,
        &fixture.public_partition,
        &fixture.representative_partition,
    )
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
                    type_reference: Default::default(),
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

struct FixedCallFixture {
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

fn fixed_call_fixture(
    literal_fixed_argument: bool,
    build_facts: impl FnOnce(
        &mut TypedTrees,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
    ) -> (Vec<ProofFact>, ProofFact),
) -> FixedCallFixture {
    let mut program = TypedTrees::default();
    let integer_type = primitive_type(&mut program, "i32");
    let quotient_symbol = symbol(1100);
    let relation_symbol = symbol(1101);
    let quotient = quotient_type_over(
        &mut program,
        quotient_symbol,
        "FixedCallQ",
        relation_symbol,
        "FixedCallR",
        integer_type,
    );
    let public_quotient_symbol = symbol(1102);
    let public_fixed_symbol = symbol(1103);
    let representative_quotient_symbol = symbol(1104);
    let representative_fixed_symbol = symbol(1105);
    let static_const_symbol = symbol(1106);
    let (public_facts, representative_fact) = build_facts(
        &mut program,
        public_fixed_symbol,
        representative_fixed_symbol,
        static_const_symbol,
    );
    let public_facts = program.proof_facts.insert_many(public_facts);
    let mut public_machine = Machine {
        symbol: symbol(1107),
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
    for (symbol, name, type_reference) in [
        (public_quotient_symbol, "quotient", quotient),
        (public_fixed_symbol, "fixed", integer_type),
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
    let representative_facts = program.proof_facts.insert_many([representative_fact]);
    let representative_contracts = program.signature_contracts.insert_many([SignatureContract {
        kind: SignatureContractKind::Requires,
        facts: representative_facts,
        ..Default::default()
    }]);
    let representative = RepresentativeTelescope {
        machine_symbol: symbol(1108),
        state_symbol: symbol(1109),
        parameters: vec![
            RepresentativeRuntimeParameter {
                symbol: representative_quotient_symbol,
                type_reference: integer_type,
                is_mutable: false,
                is_self: false,
            },
            RepresentativeRuntimeParameter {
                symbol: representative_fixed_symbol,
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
                    type_reference: Default::default(),
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
    let fixed_source = if literal_fixed_argument {
        super::DirectLiftArgumentSource::Literal(
            super::runtime_correspondence::ClosedLiftLiteral::Integer {
                spelling: "2".to_owned(),
                landing: IntegerLanding {
                    landed_type: LandedIntegerType::I32,
                    domain: ArithmeticDomain::Exact,
                },
            },
        )
    } else {
        super::DirectLiftArgumentSource::PublicParameter(public_fixed_symbol)
    };
    let runtime = super::DirectLiftRuntimeCorrespondence {
        positions: vec![
            super::DirectLiftRuntimePosition {
                source: super::DirectLiftArgumentSource::PublicParameter(public_quotient_symbol),
                representative_parameter: representative_quotient_symbol,
            },
            super::DirectLiftRuntimePosition {
                source: fixed_source,
                representative_parameter: representative_fixed_symbol,
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
    .expect("fixed public Q partitions exactly");
    let representative_partition =
        derive_representative_precondition_partition(&program, &input_relations, &representative)
            .expect("fixed representative P partitions exactly");
    let expected_theorem =
        derive_expected_theorem_schema(&program, &input_relations, relation, &representative)
            .expect("fixed-call theorem schema");
    let verified_theorem = super::VerifiedTheoremSchema {
        theorem_machine_symbol: symbol(1110),
        theorem_state_symbol: symbol(1111),
        parameters: expected_theorem
            .parameters
            .iter()
            .enumerate()
            .map(|(expected_position, _)| {
                super::theorem_schema_verification::VerifiedTheoremParameter {
                    expected_position,
                    theorem_symbol: symbol(1120 + u32::try_from(expected_position).unwrap()),
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
    FixedCallFixture {
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
