//! Non-authoritative `RA`/`RR` derivation for direct terminal quotient requests.
//!
//! The plan retains exact quotient TYPE identity as well as relation symbol so
//! two quotients over one carrier cannot collapse. It grants no execution
//! authority and deliberately refuses nested/adapted result flow.

use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{QuotientOperationRequest, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::SignatureContract;
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
    pub(super) representative: RepresentativeTelescope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepresentativeRuntimeParameter {
    pub(super) symbol: SymbolHandle,
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) is_mutable: bool,
    pub(super) is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RepresentativeTelescope {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
    pub(super) parameters: Vec<RepresentativeRuntimeParameter>,
    pub(super) return_type: TypeReferenceHandle,
    pub(super) machine_contracts: HandleSpan<SignatureContract>,
    pub(super) state_contracts: HandleSpan<SignatureContract>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RelationPlanError {
    UnresolvedArgumentType(usize),
    UnresolvedInputRelationApplication(usize),
    ResultIsNotQuotient,
    UnresolvedResultRelationApplication,
    RepresentativeEntryDoesNotResolveExactly,
    RepresentativeApplicationRequiresSubstitution,
    RepresentativeResultTypeIsUnresolved,
}

impl fmt::Display for RelationPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnresolvedArgumentType(position) => write!(
                formatter,
                "argument {position} has no exact declared type; adapted lift arguments require later expression typing"
            ),
            Self::UnresolvedInputRelationApplication(position) => write!(
                formatter,
                "argument {position}'s quotient relation has an open binder application that requires the representative-operation telescope"
            ),
            Self::ResultIsNotQuotient => formatter
                .write_str("the enclosing state's exact result type is not a formed quotient"),
            Self::UnresolvedResultRelationApplication => formatter.write_str(
                "the result quotient relation has an open binder application that requires the representative-operation result telescope",
            ),
            Self::RepresentativeEntryDoesNotResolveExactly => formatter.write_str(
                "the retained representative entry symbol does not resolve to exactly one machine state",
            ),
            Self::RepresentativeApplicationRequiresSubstitution => formatter.write_str(
                "the representative operation has a generic/static application that requires exact telescope substitution",
            ),
            Self::RepresentativeResultTypeIsUnresolved => formatter.write_str(
                "the representative operation has no exact result type",
            ),
        }
    }
}

pub(super) fn derive_direct_terminal_plan(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    request: &QuotientOperationRequest,
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
        input_relations.push(match exact_quotient_relation(program, argument_type) {
            ExactRelationLookup::NotQuotient => InputRelation::ExactEquality(argument_type),
            ExactRelationLookup::Exact(relation) => InputRelation::Quotient(relation),
            ExactRelationLookup::OpenApplication => {
                return Err(RelationPlanError::UnresolvedInputRelationApplication(
                    position,
                ));
            }
        });
    }
    let result_relation = match exact_quotient_relation(program, state.return_type) {
        ExactRelationLookup::NotQuotient => return Err(RelationPlanError::ResultIsNotQuotient),
        ExactRelationLookup::Exact(relation) => relation,
        ExactRelationLookup::OpenApplication => {
            return Err(RelationPlanError::UnresolvedResultRelationApplication);
        }
    };
    let representative = derive_representative_telescope(program, request)?;
    Ok(DirectTerminalRelationPlan {
        input_relations,
        result_relation,
        representative,
    })
}

fn derive_representative_telescope(
    program: &TypedTrees,
    request: &QuotientOperationRequest,
) -> Result<RepresentativeTelescope, RelationPlanError> {
    if request.representative_operation.application.is_some() {
        return Err(RelationPlanError::RepresentativeApplicationRequiresSubstitution);
    }
    let mut matches = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == request.representative_operation.symbol)
            .map(move |state| (machine, state))
    });
    let Some((machine, state)) = matches.next() else {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    };
    if matches.next().is_some() {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    }
    if !program.machine_type_parameters(machine).is_empty() {
        return Err(RelationPlanError::RepresentativeApplicationRequiresSubstitution);
    }
    if !state.return_type.is_valid() {
        return Err(RelationPlanError::RepresentativeResultTypeIsUnresolved);
    }
    let parameters = program
        .state_parameters(state)
        .iter()
        // This is only the RUNTIME telescope. Exact static/const argument
        // correspondence remains a later obligation over the retained static
        // application; filtering here does not discharge it.
        .filter(|parameter| !parameter.is_const)
        .map(|parameter| RepresentativeRuntimeParameter {
            symbol: parameter.symbol,
            type_reference: parameter.type_reference,
            is_mutable: parameter.is_mutable,
            is_self: parameter.is_self,
        })
        .collect();
    Ok(RepresentativeTelescope {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        parameters,
        return_type: state.return_type,
        machine_contracts: machine.contracts,
        state_contracts: state.contracts,
    })
}

enum ExactRelationLookup {
    NotQuotient,
    Exact(ExactQuotientRelation),
    OpenApplication,
}

fn exact_quotient_relation(
    program: &TypedTrees,
    quotient_type: TypeReferenceHandle,
) -> ExactRelationLookup {
    let Some(quotient) = super::quotient_for_type(program, quotient_type) else {
        return ExactRelationLookup::NotQuotient;
    };
    let Some(metadata) = quotient.quotient.as_ref() else {
        return ExactRelationLookup::NotQuotient;
    };
    let Some(relation) = program
        .propositions()
        .iter()
        .find(|relation| relation.symbol == metadata.relation_symbol)
    else {
        return ExactRelationLookup::OpenApplication;
    };
    if !program.proposition_binders(relation).is_empty() {
        // The quotient declaration retains the relation declaration identity,
        // but not the closed application needed for heterogeneous families.
        // That application must come from the fully instantiated
        // representative operation telescope; guessing it from the quotient
        // type would collapse independently quantified I/J/K binders.
        return ExactRelationLookup::OpenApplication;
    }
    ExactRelationLookup::Exact(ExactQuotientRelation {
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

    pub(super) fn render_representative_telescope(&self, program: &TypedTrees) -> String {
        let parameters = self
            .representative
            .parameters
            .iter()
            .map(|parameter| {
                let receiver = if parameter.is_self { "self:" } else { "" };
                format!(
                    "{receiver}{}",
                    program.display_type_reference_with_constraints(parameter.type_reference)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "F#{}({parameters})->{}",
            self.representative.state_symbol.arena_index(),
            program.display_type_reference_with_constraints(self.representative.return_type),
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
    use super::{
        InputRelation, RelationPlanError, derive_direct_terminal_plan,
        derive_representative_telescope,
    };
    use psi_arena::HandleSpan;
    use psi_symbols::SymbolHandle;
    use psi_typed_trees::TypedTrees;
    use psi_typed_trees::data::{DataDefinition, QuotientDefinition};
    use psi_typed_trees::expression::{
        ExpressionHandle, ExpressionNode, QuotientOperationKind, QuotientOperationRequest,
        StaticMachineArgument, StaticSymbolApplication, TableCallExpression, TableNamePath,
    };
    use psi_typed_trees::machine::Machine;
    use psi_typed_trees::name::Identifier;
    use psi_typed_trees::proposition::{
        PropositionBinder, PropositionBinderKind, PropositionDefinition,
    };
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

    fn request_with_representative(symbol: SymbolHandle) -> QuotientOperationRequest {
        let mut representative_operation = static_argument("representative");
        representative_operation.symbol = symbol;
        QuotientOperationRequest {
            kind: QuotientOperationKind::Lift,
            representative_operation,
            respect_conformance: static_argument("ExactRespect"),
        }
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
        request_with_representative(symbol(91))
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
        let request = push_representative(
            &mut program,
            &[
                (ordinary_type, true, false),
                (ordinary_type, false, false),
                (ordinary_type, false, true),
            ],
            ordinary_type,
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
        assert_eq!(plan.representative.parameters.len(), 2);
        assert!(plan.representative.parameters[0].is_self);
        assert!(!plan.representative.parameters[1].is_self);
        assert_eq!(plan.representative.return_type, ordinary_type);
        assert_eq!(plan.representative.machine_contracts.count(), 1);
        assert_eq!(plan.representative.state_contracts.count(), 1);
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
        let quotient_type =
            quotient_type(&mut program, symbol(1), "IndexedQ", symbol(2), "IndexedR");
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
    fn representative_telescope_rejects_unsubstituted_static_application() {
        let program = TypedTrees::default();
        let mut request = request_with_representative(symbol(91));
        request.representative_operation.application = Some(Box::new(StaticSymbolApplication {
            lifetime_arguments: Box::default(),
            arguments: Box::default(),
        }));

        assert_eq!(
            derive_representative_telescope(&program, &request),
            Err(RelationPlanError::RepresentativeApplicationRequiresSubstitution)
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
        let carrier_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let mut request =
            push_representative(&mut program, &[(carrier_type, false, false)], carrier_type);
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
        assert!(
            diagnostics[0]
                .message
                .contains("executable quotient operations are not admitted")
        );
    }
}
