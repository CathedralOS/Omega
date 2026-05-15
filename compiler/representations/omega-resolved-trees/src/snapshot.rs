use crate::data::{DataDefinition, DataMember};
use crate::expression::{BinaryOperator, Expression, NamePath};
use crate::invariant::InvariantDefinition;
use crate::machine::{Machine, OwnedData};
use crate::platform::Platform;
use crate::signature::{StateParameter, StateSignature};
use crate::state::State;
use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
use crate::types::{TypeConstraint, TypeReference};
use crate::SymbolResolvedTrees;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolResolvedTreesSnapshot {
    pub roots: SymbolResolvedRootsSnapshot,
    pub tables: ResolvedTableSnapshot,
}

impl SymbolResolvedTreesSnapshot {
    pub fn from_symbol_resolved_trees(symbol_resolved_trees: &SymbolResolvedTrees) -> Self {
        Self {
            roots: SymbolResolvedRootsSnapshot {
                data_definitions: symbol_resolved_trees
                    .data_definitions
                    .iter()
                    .map(|data| data_definition_snapshot(symbol_resolved_trees, data))
                    .collect(),
                invariant_definitions: symbol_resolved_trees
                    .invariant_definitions
                    .iter()
                    .map(|invariant| {
                        invariant_definition_snapshot(symbol_resolved_trees, invariant)
                    })
                    .collect(),
                machines: symbol_resolved_trees
                    .machines
                    .iter()
                    .map(|machine| machine_snapshot(symbol_resolved_trees, machine))
                    .collect(),
                platforms: symbol_resolved_trees
                    .platforms
                    .iter()
                    .map(|platform| platform_snapshot(symbol_resolved_trees, platform))
                    .collect(),
            },
            tables: ResolvedTableSnapshot {
                type_constraint_count: symbol_resolved_trees.tables.types.constraints.len(),
                expression_count: symbol_resolved_trees
                    .tables
                    .bodies
                    .expressions
                    .expression_count(),
                statement_count: symbol_resolved_trees
                    .tables
                    .bodies
                    .statements
                    .statement_count(),
                type_reference_count: symbol_resolved_trees
                    .tables
                    .types
                    .references
                    .type_reference_count(),
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
pub struct SymbolResolvedRootsSnapshot {
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
    ArrayLiteral {
        values: Vec<ExpressionSnapshot>,
    },
    Binary {
        left: Box<ExpressionSnapshot>,
        operator: &'static str,
        right: Box<ExpressionSnapshot>,
    },
    Boolean {
        value: bool,
    },
    Cast {
        value: Box<ExpressionSnapshot>,
        target_type: Vec<String>,
    },
    Call {
        receiver: Option<Box<ExpressionSnapshot>>,
        target: String,
        arguments: Vec<ExpressionSnapshot>,
    },
    Float {
        value: String,
    },
    Indexed {
        collection: Box<ExpressionSnapshot>,
        index: Box<ExpressionSnapshot>,
    },
    Integer {
        value: i64,
    },
    Member {
        receiver: Box<ExpressionSnapshot>,
        member: String,
    },
    Mutable {
        value: Box<ExpressionSnapshot>,
    },
    Name {
        path: Vec<String>,
    },
    StructLiteral {
        type_name: String,
        fields: Vec<StructLiteralFieldSnapshot>,
    },
    String {
        value: String,
    },
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
    SelfType,
    Unit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TypeConstraintSnapshot {
    Named {
        name: String,
    },
    Range {
        minimum: ExpressionSnapshot,
        maximum: ExpressionSnapshot,
    },
}

fn data_definition_snapshot(
    program: &SymbolResolvedTrees,
    data: &DataDefinition,
) -> DataDefinitionSnapshot {
    DataDefinitionSnapshot {
        name: data.name.to_string(),
        type_parameters: program
            .data_type_parameters(data.type_parameters)
            .iter()
            .map(|parameter| parameter.name.to_string())
            .collect(),
        members: program
            .data_members(data.members)
            .iter()
            .map(|member| data_member_snapshot(program, member))
            .collect(),
    }
}

fn data_member_snapshot(program: &SymbolResolvedTrees, member: &DataMember) -> DataMemberSnapshot {
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
    program: &SymbolResolvedTrees,
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

fn machine_snapshot(program: &SymbolResolvedTrees, machine: &Machine) -> MachineSnapshot {
    MachineSnapshot {
        name: machine.name.to_string(),
        contains: program
            .machine_contained_objects(machine.contains)
            .iter()
            .map(|contained| ContainedObjectSnapshot {
                name: contained.name.to_string(),
                type_name: contained.type_name.to_string(),
            })
            .collect(),
        owned_data: program
            .machine_owned_data(machine.owned_data)
            .iter()
            .map(|owned| owned_data_snapshot(program, owned))
            .collect(),
        states: program
            .machine_state_handles(machine.states)
            .iter()
            .map(|state| program.machine_state(*state))
            .map(|state| state_snapshot(program, state))
            .collect(),
    }
}

fn owned_data_snapshot(program: &SymbolResolvedTrees, owned: &OwnedData) -> OwnedDataSnapshot {
    OwnedDataSnapshot {
        name: owned.name.to_string(),
        type_reference: type_reference_snapshot(program, &owned.type_reference),
        initial_value: owned.initial_value.as_ref().map(expression_snapshot),
    }
}

fn platform_snapshot(program: &SymbolResolvedTrees, platform: &Platform) -> PlatformSnapshot {
    PlatformSnapshot {
        name: platform.name.to_string(),
        states: program
            .platform_state_signatures(platform.states)
            .iter()
            .map(|signature| state_signature_snapshot(program, signature))
            .collect(),
    }
}

fn state_snapshot(program: &SymbolResolvedTrees, state: &State) -> StateSnapshot {
    StateSnapshot {
        name: state.name.to_string(),
        parameters: program
            .state_parameters(state.parameters)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
        statements: program
            .state_statements(state.statements)
            .iter()
            .map(|statement| statement_snapshot(program, statement))
            .collect(),
        table_statement_count: state.statement_nodes.count() as usize,
    }
}

fn state_signature_snapshot(
    program: &SymbolResolvedTrees,
    signature: &StateSignature,
) -> StateSignatureSnapshot {
    StateSignatureSnapshot {
        name: signature.name.to_string(),
        parameters: program
            .state_parameters(signature.parameters)
            .iter()
            .map(|parameter| state_parameter_snapshot(program, parameter))
            .collect(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| type_reference_snapshot(program, type_reference)),
    }
}

fn state_parameter_snapshot(
    program: &SymbolResolvedTrees,
    parameter: &StateParameter,
) -> StateParameterSnapshot {
    StateParameterSnapshot {
        name: parameter.name.to_string(),
        type_reference: type_reference_snapshot(program, &parameter.type_reference),
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    }
}

fn statement_snapshot(program: &SymbolResolvedTrees, statement: &Statement) -> StatementSnapshot {
    match statement {
        Statement::Assignment(assignment) => StatementSnapshot::Assignment {
            target: statement_expression_snapshot(program, assignment.target),
            value: statement_expression_snapshot(program, assignment.value),
        },
        Statement::Call(call) => StatementSnapshot::Call {
            receiver: call.receiver.as_ref().map(name_path_snapshot),
            target: call.target.to_string(),
            arguments: program
                .state_statement_expressions(call.arguments)
                .iter()
                .map(expression_snapshot)
                .collect(),
        },
        Statement::Expression(expression) => StatementSnapshot::Expression {
            value: statement_expression_snapshot(program, *expression),
        },
        Statement::LocalData(local_data) => StatementSnapshot::LocalData {
            name: local_data.name.to_string(),
            type_reference: type_reference_snapshot(program, &local_data.type_reference),
            initial_value: local_data
                .initial_value
                .map(|expression| statement_expression_snapshot(program, expression)),
        },
        Statement::Transition(transition) => transition_snapshot(program, transition),
    }
}

fn transition_snapshot(
    program: &SymbolResolvedTrees,
    transition: &Transition,
) -> StatementSnapshot {
    StatementSnapshot::Transition {
        target: transition_target_snapshot(program, &transition.target),
        continuation: transition
            .continuation
            .as_ref()
            .map(|target| transition_target_snapshot(program, target)),
        guard: match &transition.guard {
            TransitionGuard::Always => TransitionGuardSnapshot::Always,
            TransitionGuard::When(expression) => TransitionGuardSnapshot::When {
                value: statement_expression_snapshot(program, *expression),
            },
        },
    }
}

fn transition_target_snapshot(
    program: &SymbolResolvedTrees,
    target: &TransitionTarget,
) -> TransitionTargetSnapshot {
    match target {
        TransitionTarget::Named(named) => TransitionTargetSnapshot::Named {
            path: name_path_snapshot(&named.path),
            arguments: program
                .state_statement_expressions(named.arguments)
                .iter()
                .map(expression_snapshot)
                .collect(),
        },
        TransitionTarget::Value(expression) => TransitionTargetSnapshot::Value {
            value: statement_expression_snapshot(program, *expression),
        },
        TransitionTarget::SelfTarget => TransitionTargetSnapshot::SelfTarget,
        TransitionTarget::Terminal => TransitionTargetSnapshot::Terminal,
    }
}

fn statement_expression_snapshot(
    program: &SymbolResolvedTrees,
    expression: omega_core::arena::Handle<Expression>,
) -> ExpressionSnapshot {
    expression_snapshot(program.state_statement_expression(expression))
}

fn expression_snapshot(expression: &Expression) -> ExpressionSnapshot {
    match expression {
        Expression::ArrayLiteral(array_literal) => ExpressionSnapshot::ArrayLiteral {
            values: array_literal
                .values
                .iter()
                .map(expression_snapshot)
                .collect(),
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
            receiver: call
                .receiver
                .as_ref()
                .map(|receiver| Box::new(expression_snapshot(receiver))),
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

fn type_reference_snapshot(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> TypeReferenceSnapshot {
    type_reference_snapshot_from_program(program, type_reference)
}

fn type_reference_snapshot_from_program(
    program: &SymbolResolvedTrees,
    type_reference: &TypeReference,
) -> TypeReferenceSnapshot {
    match type_reference {
        TypeReference::Reference(reference) => TypeReferenceSnapshot::Reference {
            referee: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(reference.referee),
            )),
            is_mutable: reference.is_mutable,
        },
        TypeReference::Constrained(constrained) => TypeReferenceSnapshot::Constrained {
            base_type: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(constrained.base_type),
            )),
            constraints: program
                .tables
                .types
                .constraints
                .span_or_empty(constrained.constraints)
                .iter()
                .map(type_constraint_snapshot)
                .collect(),
        },
        TypeReference::FixedArray(fixed_array) => TypeReferenceSnapshot::FixedArray {
            element_type: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(fixed_array.element_type),
            )),
            length: fixed_array.length,
        },
        TypeReference::Slice(slice) => TypeReferenceSnapshot::Slice {
            element_type: Box::new(type_reference_snapshot_from_program(
                program,
                program.child_type_reference(slice.element_type),
            )),
        },
        TypeReference::Generic(generic) => TypeReferenceSnapshot::Generic {
            base_name: generic.base_name.to_string(),
            arguments: program
                .child_type_references(generic.arguments)
                .iter()
                .map(|argument| type_reference_snapshot_from_program(program, argument))
                .collect(),
        },
        TypeReference::Named { name, .. } => TypeReferenceSnapshot::Named {
            name: name.to_string(),
        },
        TypeReference::SelfType { .. } => TypeReferenceSnapshot::SelfType,
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
    path.members().iter().map(ToString::to_string).collect()
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
    use super::SymbolResolvedTreesSnapshot;
    use crate::expression::Expression;
    use crate::machine::{Machine, MachineStorage};
    use crate::name::DiagnosticName;
    use crate::state::{State, StateStorage};
    use crate::statement::{Statement, Transition, TransitionGuard, TransitionTarget};
    use crate::types::TypeReference;
    use crate::SymbolResolvedTrees;
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;

    #[test]
    fn snapshots_materialize_resolved_roots_and_table_counts() {
        let mut program = SymbolResolvedTrees::default();
        let guard = program
            .tables
            .declarations
            .state_statement_expressions
            .append(Expression::Integer(1));
        let statements =
            program
                .tables
                .declarations
                .state_statements
                .insert_many([Statement::Transition(Transition {
                    target: TransitionTarget::Terminal,
                    continuation: None,
                    guard: TransitionGuard::When(guard),
                })]);
        let state = program.tables.declarations.machine_states.append(State {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("entry"),
            storage: StateStorage {
                parameters: HandleSpan::empty(),
                return_type: Some(TypeReference::Named {
                    symbol: SymbolHandle::invalid(),
                    name: DiagnosticName::generated("i32"),
                }),
                statements,
                statement_nodes: HandleSpan::empty(),
            },
        });
        let states = program
            .tables
            .declarations
            .machine_state_handles
            .insert_many([state]);
        program.machines.push(Machine {
            symbol: SymbolHandle::invalid(),
            name: DiagnosticName::generated("main"),
            storage: MachineStorage {
                contains: HandleSpan::empty(),
                owned_data: HandleSpan::empty(),
                states,
            },
        });
        program.rebuild_tables();

        let snapshot = SymbolResolvedTreesSnapshot::from_symbol_resolved_trees(&program);
        assert_eq!(snapshot.roots.machines.len(), 1);
        assert_eq!(snapshot.roots.machines[0].states.len(), 1);
        assert_eq!(snapshot.tables.statement_count, 1);
        assert_eq!(snapshot.tables.expression_count, 1);
        assert_eq!(snapshot.tables.type_reference_count, 1);
        assert!(snapshot.to_json_pretty().is_ok());
    }
}
