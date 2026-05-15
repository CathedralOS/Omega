use crate::ResolvedTrees;
use crate::data::{DataDefinition, DataMember};
use crate::expression::{BinaryOperator, Expression, NamePath};
use crate::invariant::InvariantDefinition;
use crate::machine::{Machine, OwnedData};
use crate::platform::Platform;
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::types::{TypeConstraint, TypeReference};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedProgramSnapshot {
    pub roots: ResolvedRootsSnapshot,
    pub tables: ResolvedTableSnapshot,
}

impl ResolvedProgramSnapshot {
    pub fn from_program(program: &ResolvedTrees) -> Self {
        Self {
            roots: ResolvedRootsSnapshot {
                data_definitions: program
                    .data_definitions
                    .iter()
                    .map(|data| data_definition_snapshot(program, data))
                    .collect(),
                invariant_definitions: program
                    .invariant_definitions
                    .iter()
                    .map(|invariant| invariant_definition_snapshot(program, invariant))
                    .collect(),
                machines: program
                    .machines
                    .iter()
                    .map(|machine| machine_snapshot(program, machine))
                    .collect(),
                platforms: program
                    .platforms
                    .iter()
                    .map(|platform| platform_snapshot(program, platform))
                    .collect(),
            },
            tables: ResolvedTableSnapshot {
                type_constraint_count: program.tables.types.constraints.len(),
                expression_count: program.tables.bodies.expressions.expression_count(),
                statement_count: program.tables.bodies.statements.statement_count(),
                type_reference_count: program.tables.types.references.type_reference_count(),
            },
        }
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedRootsSnapshot {
    pub data_definitions: Vec<DataDefinitionSnapshot>,
    pub invariant_definitions: Vec<InvariantDefinitionSnapshot>,
    pub machines: Vec<MachineSnapshot>,
    pub platforms: Vec<PlatformSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedTableSnapshot {
    pub type_constraint_count: usize,
    pub expression_count: usize,
    pub statement_count: usize,
    pub type_reference_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataDefinitionSnapshot {
    pub name: String,
    pub type_parameters: Vec<String>,
    pub members: Vec<DataMemberSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataMemberSnapshot {
    Field {
        name: String,
        type_reference: TypeReferenceSnapshot,
    },
    Variant {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InvariantDefinitionSnapshot {
    pub name: String,
    pub constraints: Vec<TypeConstraintSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MachineSnapshot {
    pub name: String,
    pub contains: Vec<ContainedObjectSnapshot>,
    pub owned_data: Vec<OwnedDataSnapshot>,
    pub states: Vec<StateSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContainedObjectSnapshot {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OwnedDataSnapshot {
    pub name: String,
    pub type_reference: TypeReferenceSnapshot,
    pub initial_value: Option<ExpressionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformSnapshot {
    pub name: String,
    pub states: Vec<StateSignatureSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSnapshot {
    pub name: String,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
    pub statements: Vec<StatementSnapshot>,
    pub table_statement_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateSignatureSnapshot {
    pub name: String,
    pub parameters: Vec<StateParameterSnapshot>,
    pub return_type: Option<TypeReferenceSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StateParameterSnapshot {
    pub name: String,
    pub type_reference: TypeReferenceSnapshot,
    pub is_mutable: bool,
    pub is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StatementSnapshot {
    Assignment {
        target: ExpressionSnapshot,
        value: ExpressionSnapshot,
    },
    Call {
        receiver: Option<Vec<String>>,
        target: String,
        arguments: Vec<ExpressionSnapshot>,
    },
    Expression {
        value: ExpressionSnapshot,
    },
    LocalData {
        name: String,
        type_reference: TypeReferenceSnapshot,
        initial_value: Option<ExpressionSnapshot>,
    },
    Transition {
        target: TransitionTargetSnapshot,
        continuation: Option<TransitionTargetSnapshot>,
        guard: TransitionGuardSnapshot,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionGuardSnapshot {
    Always,
    When { value: ExpressionSnapshot },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionTargetSnapshot {
    Named {
        path: Vec<String>,
        arguments: Vec<ExpressionSnapshot>,
    },
    Value {
        value: ExpressionSnapshot,
    },
    SelfTarget,
    Terminal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExpressionSnapshot {
    ArrayLiteral { values: Vec<ExpressionSnapshot> },
    Binary {
        left: Box<ExpressionSnapshot>,
        operator: &'static str,
        right: Box<ExpressionSnapshot>,
    },
    Boolean { value: bool },
    Cast {
        value: Box<ExpressionSnapshot>,
        target_type: Vec<String>,
    },
    Call {
        receiver: Option<Box<ExpressionSnapshot>>,
        target: String,
        arguments: Vec<ExpressionSnapshot>,
    },
    Float { value: String },
    Indexed {
        collection: Box<ExpressionSnapshot>,
        index: Box<ExpressionSnapshot>,
    },
    Integer { value: i64 },
    Member {
        receiver: Box<ExpressionSnapshot>,
        member: String,
    },
    Mutable { value: Box<ExpressionSnapshot> },
    Name { path: Vec<String> },
    StructLiteral {
        type_name: String,
        fields: Vec<StructLiteralFieldSnapshot>,
    },
    String { value: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StructLiteralFieldSnapshot {
    pub name: String,
    pub value: ExpressionSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeReferenceSnapshot {
    Reference {
        referee: Box<TypeReferenceSnapshot>,
        is_mutable: bool,
    },
    Constrained {
        base_type: Box<TypeReferenceSnapshot>,
        constraints: Vec<TypeConstraintSnapshot>,
    },
    FixedArray {
        element_type: Box<TypeReferenceSnapshot>,
        length: usize,
    },
    Slice {
        element_type: Box<TypeReferenceSnapshot>,
    },
    Generic {
        base_name: String,
        arguments: Vec<TypeReferenceSnapshot>,
    },
    Named {
        name: String,
    },
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeConstraintSnapshot {
    Named { name: String },
    Range {
        minimum: ExpressionSnapshot,
        maximum: ExpressionSnapshot,
    },
}

fn data_definition_snapshot(program: &ResolvedTrees, data: &DataDefinition) -> DataDefinitionSnapshot {
    DataDefinitionSnapshot {
        name: data.name.to_string(),
        type_parameters: data
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        members: data
            .members
            .iter()
            .map(|member| data_member_snapshot(program, member))
            .collect(),
    }
}

fn data_member_snapshot(program: &ResolvedTrees, member: &DataMember) -> DataMemberSnapshot {
    match member {
        DataMember::Field(field) => DataMemberSnapshot::Field {
            name: field.name.to_string(),
            type_reference: type_reference_snapshot(program, &field.type_reference),
        },
        DataMember::Variant(variant) => DataMemberSnapshot::Variant {
            name: variant.name.to_string(),
        },
    }
}

fn invariant_definition_snapshot(
    program: &ResolvedTrees,
    invariant: &InvariantDefinition,
) -> InvariantDefinitionSnapshot {
    InvariantDefinitionSnapshot {
        name: invariant.name.to_string(),
        constraints: program
            .tables
            .types
            .constraints
            .span_or_empty(invariant.constraints)
            .iter()
            .map(type_constraint_snapshot)
            .collect(),
    }
}

fn machine_snapshot(program: &ResolvedTrees, machine: &Machine) -> MachineSnapshot {
    MachineSnapshot {
        name: machine.name.to_string(),
        contains: machine
            .contains
            .iter()
            .map(|contained| ContainedObjectSnapshot {
                name: contained.name.to_string(),
                type_name: contained.type_name.to_string(),
            })
            .collect(),
        owned_data: machine
            .owned_data
            .iter()
            .map(|owned| owned_data_snapshot(program, owned))
            .collect(),
        states: machine
            .states
            .iter()
            .map(|state| state_snapshot(program, state))
            .collect(),
    }
}

fn owned_data_snapshot(program: &ResolvedTrees, owned: &OwnedData) -> OwnedDataSnapshot {
    OwnedDataSnapshot {
        name: owned.name.to_string(),
        type_reference: type_reference_snapshot(program, &owned.type_reference),
        initial_value: owned.initial_value.as_ref().map(expression_snapshot),
    }
}

fn platform_snapshot(program: &ResolvedTrees, platform: &Platform) -> PlatformSnapshot {
    PlatformSnapshot {
        name: platform.name.to_string(),
        states: platform
            .states
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

fn state_snapshot(program: &ResolvedTrees, state: &State) -> StateSnapshot {
    StateSnapshot {
        name: state.name.to_string(),
        parameters: state
            .parameters
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
        statements: state
            .statements
            .iter()
            .map(|statement| statement_snapshot(program, statement))
            .collect(),
        table_statement_count: state.statement_nodes.count() as usize,
    }
}

fn state_signature_snapshot(program: &ResolvedTrees, signature: &StateSignature) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: signature.name.to_string(),
        parameters: signature
            .parameters
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
    }
}

fn state_parameter_snapshot(program: &ResolvedTrees, parameter: &StateParameter) -> StateParameterSnapshot {
    StateParameterSnapshot {
        name: parameter.name.to_string(),
        type_reference: type_reference_snapshot(program, &parameter.type_reference),
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    }
}

fn statement_snapshot(program: &ResolvedTrees, statement: &Statement) -> StatementSnapshot {
    match statement {
        Statement::Assignment(assignment) => StatementSnapshot::Assignment {
            target: expression_snapshot(&assignment.target),
            value: expression_snapshot(&assignment.value),
        },
        Statement::Call(call) => StatementSnapshot::Call {
            receiver: call.receiver.as_ref().map(name_path_snapshot),
            target: call.target.to_string(),
            arguments: call.arguments.iter().map(expression_snapshot).collect(),
        },
        Statement::Expression(expression) => StatementSnapshot::Expression {
            value: expression_snapshot(expression),
        },
        Statement::LocalData(local_data) => StatementSnapshot::LocalData {
            name: local_data.name.to_string(),
            type_reference: type_reference_snapshot(program, &local_data.type_reference),
            initial_value: local_data.initial_value.as_ref().map(expression_snapshot),
        },
        Statement::Transition(transition) => transition_snapshot(transition),
    }
}

fn transition_snapshot(transition: &Transition) -> StatementSnapshot {
    StatementSnapshot::Transition {
        target: transition_target_snapshot(&transition.target),
        continuation: transition
            .continuation
            .as_ref()
            .map(transition_target_snapshot),
        guard: match &transition.guard {
            TransitionGuard::Always => TransitionGuardSnapshot::Always,
            TransitionGuard::When(expression) => TransitionGuardSnapshot::When {
                value: expression_snapshot(expression),
            },
        },
    }
}

fn transition_target_snapshot(target: &TransitionTarget) -> TransitionTargetSnapshot {
    match target {
        TransitionTarget::Named(named) => TransitionTargetSnapshot::Named {
            path: name_path_snapshot(&named.path),
            arguments: named.arguments.iter().map(expression_snapshot).collect(),
        },
        TransitionTarget::Value(expression) => TransitionTargetSnapshot::Value {
            value: expression_snapshot(expression),
        },
        TransitionTarget::SelfTarget => TransitionTargetSnapshot::SelfTarget,
        TransitionTarget::Terminal => TransitionTargetSnapshot::Terminal,
    }
}

fn expression_snapshot(expression: &Expression) -> ExpressionSnapshot {
    match expression {
        Expression::ArrayLiteral(values) => ExpressionSnapshot::ArrayLiteral {
            values: values.iter().map(expression_snapshot).collect(),
        },
        Expression::Binary(binary) => ExpressionSnapshot::Binary {
            left: Box::new(expression_snapshot(&binary.left)),
            operator: binary_operator_name(binary.operator),
            right: Box::new(expression_snapshot(&binary.right)),
        },
        Expression::Boolean(value) => ExpressionSnapshot::Boolean { value: *value },
        Expression::Cast(cast) => ExpressionSnapshot::Cast {
            value: Box::new(expression_snapshot(&cast.value)),
            target_type: name_path_snapshot(&cast.target_type),
        },
        Expression::Call(call) => ExpressionSnapshot::Call {
            receiver: call.receiver.as_ref().map(|receiver| Box::new(expression_snapshot(receiver))),
            target: call.target.to_string(),
            arguments: call.arguments.iter().map(expression_snapshot).collect(),
        },
        Expression::Float(value) => ExpressionSnapshot::Float {
            value: value.to_string(),
        },
        Expression::Indexed(indexed) => ExpressionSnapshot::Indexed {
            collection: Box::new(expression_snapshot(&indexed.collection)),
            index: Box::new(expression_snapshot(&indexed.index)),
        },
        Expression::Integer(value) => ExpressionSnapshot::Integer { value: *value },
        Expression::Member(member) => ExpressionSnapshot::Member {
            receiver: Box::new(expression_snapshot(&member.receiver)),
            member: member.member.to_string(),
        },
        Expression::Mutable(value) => ExpressionSnapshot::Mutable {
            value: Box::new(expression_snapshot(value)),
        },
        Expression::Name(path) => ExpressionSnapshot::Name {
            path: name_path_snapshot(path),
        },
        Expression::StructLiteral(struct_literal) => ExpressionSnapshot::StructLiteral {
            type_name: struct_literal.type_name.to_string(),
            fields: struct_literal
                .fields
                .iter()
                .map(|field| StructLiteralFieldSnapshot {
                    name: field.name.to_string(),
                    value: expression_snapshot(&field.value),
                })
                .collect(),
        },
        Expression::String(value) => ExpressionSnapshot::String {
            value: value.as_str().to_owned(),
        },
    }
}

fn type_reference_snapshot(program: &ResolvedTrees, type_reference: &TypeReference) -> TypeReferenceSnapshot {
    type_reference_snapshot_from_constraints(type_reference, |constraints| {
        program
            .tables
            .types
            .constraints
            .span_or_empty(*constraints)
            .iter()
            .map(type_constraint_snapshot)
            .collect()
    })
}

fn type_reference_snapshot_from_constraints(
    type_reference: &TypeReference,
    resolve_constraints: impl Fn(&omega_core::arena::HandleSpan<TypeConstraint>) -> Vec<TypeConstraintSnapshot> + Copy,
) -> TypeReferenceSnapshot {
    match type_reference {
        TypeReference::Reference { referee, is_mutable } => TypeReferenceSnapshot::Reference {
            referee: Box::new(type_reference_snapshot_from_constraints(
                referee,
                resolve_constraints,
            )),
            is_mutable: *is_mutable,
        },
        TypeReference::Constrained(constrained) => TypeReferenceSnapshot::Constrained {
            base_type: Box::new(type_reference_snapshot_from_constraints(
                &constrained.base_type,
                resolve_constraints,
            )),
            constraints: resolve_constraints(&constrained.constraints),
        },
        TypeReference::FixedArray { element_type, length } => TypeReferenceSnapshot::FixedArray {
            element_type: Box::new(type_reference_snapshot_from_constraints(
                element_type,
                resolve_constraints,
            )),
            length: *length,
        },
        TypeReference::Slice { element_type } => TypeReferenceSnapshot::Slice {
            element_type: Box::new(type_reference_snapshot_from_constraints(
                element_type,
                resolve_constraints,
            )),
        },
        TypeReference::Generic(generic) => TypeReferenceSnapshot::Generic {
            base_name: generic.base_name.to_string(),
            arguments: generic
                .arguments
                .iter()
                .map(|argument| {
                    type_reference_snapshot_from_constraints(argument, resolve_constraints)
                })
                .collect(),
        },
        TypeReference::Named { name, .. } => TypeReferenceSnapshot::Named {
            name: name.to_string(),
        },
        TypeReference::Unit => TypeReferenceSnapshot::Unit,
    }
}

fn type_constraint_snapshot(constraint: &TypeConstraint) -> TypeConstraintSnapshot {
    match constraint {
        TypeConstraint::Named(name) => TypeConstraintSnapshot::Named {
            name: name.to_string(),
        },
        TypeConstraint::Range { minimum, maximum } => TypeConstraintSnapshot::Range {
            minimum: expression_snapshot(minimum),
            maximum: expression_snapshot(maximum),
        },
    }
}

fn name_path_snapshot(path: &NamePath) -> Vec<String> {
    path.as_slice().iter().map(ToString::to_string).collect()
}

fn binary_operator_name(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "+",
        BinaryOperator::And => "&&",
        BinaryOperator::Divide => "/",
        BinaryOperator::Equal => "==",
        BinaryOperator::Greater => ">",
        BinaryOperator::GreaterOrEqual => ">=",
        BinaryOperator::Less => "<",
        BinaryOperator::LessOrEqual => "<=",
        BinaryOperator::Modulo => "%",
        BinaryOperator::Multiply => "*",
        BinaryOperator::NotEqual => "!=",
        BinaryOperator::Or => "||",
        BinaryOperator::ShiftLeft => "<<",
        BinaryOperator::ShiftRight => ">>",
        BinaryOperator::Subtract => "-",
    }
}

#[cfg(test)]
mod tests {
    use super::ResolvedProgramSnapshot;
    use crate::ResolvedTrees;
    use crate::expression::Expression;
    use crate::machine::{Machine, MachineStorage};
    use crate::name::ProgramName;
    use crate::state::{State, StateStorage};
    use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
    use crate::types::TypeReference;
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn snapshots_materialize_resolved_roots_and_table_counts() {
        let mut program = ResolvedTrees::default();
        program.machines = vec![Machine {
            symbol: SymbolHandle::invalid(),
            name: ProgramName::generated("main"),
            storage: MachineStorage {
                contains: Vec::new(),
                owned_data: Vec::new(),
                states: vec![State {
                    symbol: SymbolHandle::invalid(),
                    name: ProgramName::generated("entry"),
                    storage: StateStorage {
                        parameters: Vec::new(),
                        return_type: Some(TypeReference::Named {
                            symbol: SymbolHandle::invalid(),
                            name: ProgramName::generated("i32"),
                        }),
                        statements: vec![Statement::Transition(Transition {
                            target: TransitionTarget::Terminal,
                            continuation: None,
                            guard: TransitionGuard::When(Expression::Integer(1)),
                        })],
                        statement_nodes: HandleSpan::empty(),
                    },
                }],
            },
        }];
        program.rebuild_tables();

        let snapshot = ResolvedProgramSnapshot::from_program(&program);
        assert_eq!(snapshot.roots.machines.len(), 1);
        assert_eq!(snapshot.roots.machines[0].states.len(), 1);
        assert_eq!(snapshot.tables.statement_count, 1);
        assert_eq!(snapshot.tables.expression_count, 1);
        assert_eq!(snapshot.tables.type_reference_count, 1);
        assert!(snapshot.to_json_pretty().is_ok());
    }
}
