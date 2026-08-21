//! Non-authoritative `RA`/`RR` derivation for direct terminal quotient requests.
//!
//! The plan retains exact quotient TYPE identity as well as relation symbol so
//! two quotients over one carrier cannot collapse. It grants no execution
//! authority and deliberately refuses nested/adapted result flow.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::TableCallExpression;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::TypeReferenceHandle;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ExactQuotientRelation {
    pub(super) quotient_type: TypeReferenceHandle,
    pub(super) quotient_symbol: SymbolHandle,
    pub(super) relation_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputRelation {
    Quotient(ExactQuotientRelation),
    /// Non-quotient operands remain part of the pointwise relation through
    /// exact equality. They must never disappear into an implicit `true`.
    ExactEquality(TypeReferenceHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DirectTerminalRelationPlan {
    /// One entry per authored runtime argument. Quotient positions use their
    /// exact selected relation; ordinary positions use exact typed equality.
    pub(super) input_relations: Vec<InputRelation>,
    pub(super) result_relation: ExactQuotientRelation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RelationPlanError {
    UnresolvedArgumentType(usize),
    ResultIsNotQuotient,
}

impl fmt::Display for RelationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedArgumentType(position) => write!(
                formatter,
                "argument {position} has no exact declared type; adapted lift arguments require later expression typing"
            ),
            Self::ResultIsNotQuotient => formatter
                .write_str("the enclosing state's exact result type is not a formed quotient"),
        }
    }
}

pub(super) fn derive_direct_terminal_plan(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
) -> Result<DirectTerminalRelationPlan, RelationPlanError> {
    let mut input_relations = Vec::new();
    for (position, argument) in program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .enumerate()
    {
        let argument_type =
            crate::places::declared_place_type_raw(program, machine, Some(state), *argument)
                .ok_or(RelationPlanError::UnresolvedArgumentType(position))?;
        input_relations.push(
            exact_quotient_relation(program, argument_type)
                .map(InputRelation::Quotient)
                .unwrap_or(InputRelation::ExactEquality(argument_type)),
        );
    }
    let result_relation = exact_quotient_relation(program, state.return_type)
        .ok_or(RelationPlanError::ResultIsNotQuotient)?;
    Ok(DirectTerminalRelationPlan {
        input_relations,
        result_relation,
    })
}

fn exact_quotient_relation(
    program: &TypedTrees,
    quotient_type: TypeReferenceHandle,
) -> Option<ExactQuotientRelation> {
    let quotient = super::quotient_for_type(program, quotient_type)?;
    let metadata = quotient.quotient.as_ref()?;
    Some(ExactQuotientRelation {
        quotient_type,
        quotient_symbol: quotient.symbol,
        relation_symbol: metadata.relation_symbol,
    })
}

impl DirectTerminalRelationPlan {
    pub(super) fn render_ra(&self, program: &TypedTrees) -> String {
        let positions = self
            .input_relations
            .iter()
            .enumerate()
            .map(|(position, relation)| {
                let relation = match relation {
                    InputRelation::Quotient(relation) => {
                        relation_name(program, relation.relation_symbol)
                    }
                    InputRelation::ExactEquality(type_reference) => format!(
                        "==<{}>",
                        program.display_type_reference_with_constraints(*type_reference)
                    ),
                };
                format!("{position}:{relation}")
            })
            .collect::<Vec<_>>();
        format!("RA=[{}]", positions.join(", "))
    }

    pub(super) fn render_rr(&self, program: &TypedTrees) -> String {
        format!(
            "RR={}",
            relation_name(program, self.result_relation.relation_symbol)
        )
    }
}

fn relation_name(program: &TypedTrees, symbol: SymbolHandle) -> String {
    program
        .propositions()
        .iter()
        .find(|proposition| proposition.symbol == symbol)
        .map(|proposition| proposition.name.as_str().to_owned())
        .unwrap_or_else(|| format!("relation#{symbol:?}"))
}

#[cfg(test)]
mod tests {
    use super::{InputRelation, RelationPlanError, derive_direct_terminal_plan};
    use psi_arena::HandleSpan;
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::data::{DataDefinition, QuotientDefinition};
    use psi_typed_trees::expression::{
        ExpressionHandle, ExpressionNode, QuotientOperationKind, QuotientOperationRequest,
        StaticMachineArgument, TableCallExpression, TableNamePath,
    };
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::signature::StateParameter;
    use psi_typed_trees::state::State;
    use psi_typed_trees::statement::StatementNode;
    use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
        let carrier = program.type_reference_table.insert(TypeReferenceNode::Unit);
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

    fn call_with_arguments(
        arguments: HandleSpan<psi_typed_trees::expression::ExpressionHandle>,
    ) -> TableCallExpression {
        TableCallExpression {
            receiver: ExpressionHandle::invalid(),
            target_symbol: SymbolHandle::invalid(),
            target: Identifier::generated_static("lift"),
            machine_arguments: Box::default(),
            quotient_operation: None,
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

        let plan = derive_direct_terminal_plan(&program, &machine, &state, &call)
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
    }

    #[test]
    fn direct_plan_rejects_untyped_adapted_argument() {
        let mut program = TypedTrees::default();
        let result_type = quotient_type(&mut program, symbol(1), "ResultQ", symbol(2), "ResultR");
        let literal = program
            .expression_table
            .insert(ExpressionNode::Integer(Default::default()));
        let arguments = program
            .expression_table
            .insert_expression_handles([literal]);
        let call = call_with_arguments(arguments);
        let state = State {
            return_type: result_type,
            ..Default::default()
        };

        assert_eq!(
            derive_direct_terminal_plan(&program, &Machine::default(), &state, &call),
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
            derive_direct_terminal_plan(&program, &Machine::default(), &state, &call),
            Err(RelationPlanError::ResultIsNotQuotient)
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
        call.quotient_operation = Some(QuotientOperationRequest {
            kind: QuotientOperationKind::Define,
            representative_operation: static_argument("representative"),
            respect_conformance: static_argument("ExactRespect"),
        });
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
        assert!(
            diagnostics[0]
                .message
                .contains("executable quotient operations are not admitted")
        );
    }
}
