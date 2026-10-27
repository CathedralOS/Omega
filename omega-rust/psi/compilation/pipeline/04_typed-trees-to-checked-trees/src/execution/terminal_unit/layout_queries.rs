//! Freeze exact query subjects while their stored shapes are being registered.

use std::collections::{BTreeSet, HashSet};

use checked_trees::{
    CheckFacts, CheckedLayoutQuery, CheckedLayoutQueryCarrier, CheckedScalarComputationKind,
    CheckedUnitPlanGap, CheckedUnitPlanGapReason,
};
use semantic_vocabulary::{IeeeFloatFormat, IntegerSign, IntegerType, ScalarType};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::references::TypedReferences;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

use super::types::ShapeCollector;
use super::{plan_gaps, types};

pub(super) fn retain(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    retained: &mut BTreeSet<TypeIdentityId>,
    gaps: &mut Vec<CheckedUnitPlanGap>,
) -> Vec<CheckedLayoutQuery> {
    let mut subjects = Vec::new();
    for (_, computation) in facts.values.scalar_computations.nodes.iter() {
        let CheckedScalarComputationKind::LayoutObservation {
            expression,
            machine,
            kind,
            operand,
            carrier_identity,
            ..
        } = &computation.kind
        else {
            continue;
        };
        if !program.expression_table.expression_is_valid(*expression) {
            missing_subject(program, gaps, *machine);
            continue;
        }
        let query = program
            .expression_table
            .call_node(*expression)
            .and_then(|call| {
                call.layout_query
                    .as_ref()
                    .filter(|query| query.kind == *kind && query.carrier == *operand)
            });
        let carrier = query.and_then(|_| {
            freeze_carrier(
                program,
                shapes,
                *machine,
                *operand,
                Some(*carrier_identity),
                retained,
            )
        });
        match carrier {
            Some(carrier) => retain_subject(
                program,
                &mut subjects,
                gaps,
                CheckedLayoutQuery {
                    expression: *expression,
                    owner: *machine,
                    kind: *kind,
                    carrier,
                },
            ),
            None => missing_subject(program, gaps, *machine),
        }
    }

    // Closed authored contracts can have no executable query operation and
    // no separate range/predicate recipe. Their retained leaves still own a
    // complete query subject and transitive stored-type catalog entry.
    for plan in &facts.contract_plans.machines {
        if program
            .machine_by_symbol(plan.machine)
            .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
        {
            continue;
        }
        for clause in plan
            .closed_scalar_values
            .requires()
            .iter()
            .chain(plan.closed_scalar_values.ensures())
            .flatten()
        {
            if let checked_trees::ClosedScalarContractValue::Predicate(predicate) = clause {
                predicate.visit_layout_observations(&mut |leaf| {
                    let checked_trees::CheckedScalarExpression::LayoutObservation {
                        expression,
                        owner,
                        kind,
                        carrier_identity,
                    } = leaf
                    else {
                        return;
                    };
                    let query = program
                        .expression_table
                        .expression_is_valid(*expression)
                        .then(|| program.expression_table.call_node(*expression))
                        .flatten()
                        .and_then(|call| call.layout_query.as_ref())
                        .filter(|query| query.kind == *kind);
                    let carrier = query.and_then(|query| {
                        freeze_carrier(
                            program,
                            shapes,
                            *owner,
                            query.carrier,
                            Some(*carrier_identity),
                            retained,
                        )
                    });
                    match carrier {
                        Some(carrier) => retain_subject(
                            program,
                            &mut subjects,
                            gaps,
                            CheckedLayoutQuery {
                                expression: *expression,
                                owner: *owner,
                                kind: *kind,
                                carrier,
                            },
                        ),
                        None => missing_subject(program, gaps, *owner),
                    }
                });
            }
        }
    }

    let dependencies = facts
        .constant_predicate_obligations
        .iter()
        .flat_map(|obligation| obligation.observation_dependencies())
        .chain(
            facts
                .integer_range_obligations
                .iter()
                .flat_map(|obligation| obligation.observation_dependencies()),
        )
        .chain(
            facts
                .values
                .scalar_computations
                .nodes
                .iter()
                .filter_map(|(_, computation)| match &computation.kind {
                    CheckedScalarComputationKind::RetainedConstant {
                        expression,
                        machine,
                    } => Some((*expression, *machine)),
                    _ => None,
                }),
        );
    let declaration_jobs = program
        .const_declarations()
        .iter()
        .filter(|declaration| declaration.evaluated_value.is_none())
        .flat_map(|declaration| {
            [
                declaration.authored_initializer,
                declaration.materialized_initializer,
            ]
            .into_iter()
            .filter(|root| root.is_valid())
            .map(|root| (root, declaration.symbol))
        });
    let mut visited = HashSet::new();
    let mut references = ObservationReferences {
        program,
        owner: SymbolHandle::invalid(),
        visited: &mut visited,
        pending: dependencies.chain(declaration_jobs).collect(),
        queries: Vec::new(),
    };
    // The catalog covers proof declarations as well as callable contracts.
    // A membership's domain can own the query even when no scalar contract
    // projection exists on its caller. Retain lexical owners from the shared
    // proof-fact inventory rather than reconstructing declaration families.
    for (owner, _, fact) in typed_trees::references::program_scoped_proof_facts(program) {
        if program
            .machine_by_symbol(owner)
            .is_some_and(|machine| program.callable_body_has_open_parameters(machine))
        {
            continue;
        }
        references.owner = owner;
        typed_trees::references::visit_proof_fact(program, &mut references, fact);
    }
    while let Some((expression, owner)) = references.pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            missing_subject(program, gaps, owner);
            continue;
        }
        references.owner = owner;
        typed_trees::references::visit_expression(program, &mut references, expression);
    }
    for (expression, owner) in references.queries {
        let Some(query) = program
            .expression_table
            .call_node(expression)
            .and_then(|call| call.layout_query.as_ref())
        else {
            missing_subject(program, gaps, owner);
            continue;
        };
        // An open theory can mention a physical query on a static type binder.
        // It has no closed catalog subject until substitution. Executable
        // computations above still require their subject, and actual deferred
        // consumers fail closed if their queried subject is absent.
        if !validation::stored_structural_type_identity(program, query.carrier, &[], &[]).is_valid()
        {
            continue;
        }
        match freeze_carrier(program, shapes, owner, query.carrier, None, retained) {
            Some(carrier) => retain_subject(
                program,
                &mut subjects,
                gaps,
                CheckedLayoutQuery {
                    expression,
                    owner,
                    kind: query.kind,
                    carrier,
                },
            ),
            None => missing_subject(program, gaps, owner),
        }
    }
    subjects
}

fn freeze_carrier(
    program: &dyn validation::CheckingInput,
    shapes: &mut ShapeCollector<'_>,
    owner: SymbolHandle,
    operand: TypeReferenceHandle,
    expected: Option<TypeIdentityId>,
    retained: &mut BTreeSet<TypeIdentityId>,
) -> Option<CheckedLayoutQueryCarrier> {
    if !program
        .type_reference_table
        .contains_type_reference(operand)
    {
        return None;
    }
    let binders = program
        .machine_by_symbol(owner)
        .or_else(|| program.machine_holding_state(owner))
        .map(|machine| types::machine_binders(program, machine))
        .unwrap_or_default();
    let identity = validation::stored_structural_type_identity(program, operand, &binders, &[]);
    if !identity.is_valid() || expected.is_some_and(|expected| expected != identity) {
        return None;
    }
    if matches!(
        program.type_reference_table.type_reference(operand),
        TypeReferenceNode::Unit
    ) {
        return Some(CheckedLayoutQueryCarrier::Unit);
    }
    if let Some(primitive) = program.primitive_type_reference(operand) {
        let scalar = match primitive {
            PrimitiveType::Bool => ScalarType::Boolean,
            PrimitiveType::F32 => ScalarType::IeeeFloat(IeeeFloatFormat::Binary32),
            PrimitiveType::F64 => ScalarType::IeeeFloat(IeeeFloatFormat::Binary64),
            PrimitiveType::Addr => ScalarType::Integer(IntegerType::address(64).ok()?),
            PrimitiveType::Selector => {
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).ok()?)
            }
            primitive => {
                let (sign, bits) = primitive.integer_sign_bits()?;
                ScalarType::Integer(IntegerType::new(sign, bits).ok()?)
            }
        };
        return Some(CheckedLayoutQueryCarrier::Scalar(scalar));
    }
    if shapes.add_stored_type(operand, &binders, &[])? != identity {
        return None;
    }
    retained.insert(identity);
    Some(CheckedLayoutQueryCarrier::Structural(identity))
}

fn retain_subject(
    program: &dyn validation::CheckingInput,
    subjects: &mut Vec<CheckedLayoutQuery>,
    gaps: &mut Vec<CheckedUnitPlanGap>,
    subject: CheckedLayoutQuery,
) {
    if let Some(previous) = subjects.iter().find(|previous| {
        previous.expression == subject.expression && previous.owner == subject.owner
    }) {
        if *previous != subject {
            missing_subject(program, gaps, subject.owner);
        }
    } else {
        subjects.push(subject);
    }
}

fn missing_subject(
    program: &dyn validation::CheckingInput,
    gaps: &mut Vec<CheckedUnitPlanGap>,
    owner: SymbolHandle,
) {
    gaps.push(plan_gaps::plan_gap(
        program,
        owner,
        plan_gaps::GapSite::Machine,
        CheckedUnitPlanGapReason::Unsupported(
            "layout observation has no matching checked stored carrier",
        ),
    ));
}

struct ObservationReferences<'program, 'visited> {
    program: &'program dyn validation::CheckingInput,
    owner: SymbolHandle,
    visited: &'visited mut HashSet<(ExpressionHandle, SymbolHandle)>,
    pending: Vec<(ExpressionHandle, SymbolHandle)>,
    queries: Vec<(ExpressionHandle, SymbolHandle)>,
}

impl Extend<SymbolHandle> for ObservationReferences<'_, '_> {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: T) {
        for symbol in symbols {
            let Some(declaration) = self.program.const_declaration_by_symbol(symbol) else {
                continue;
            };
            // Const declarations own their detached authored evidence as well
            // as the materialized root. Neither root inherits a caller's
            // operator/binder namespace merely because that caller names it.
            for expression in [
                declaration.authored_initializer,
                declaration.materialized_initializer,
            ] {
                if expression.is_valid() {
                    self.pending.push((expression, declaration.symbol));
                }
            }
        }
    }
}

impl TypedReferences for ObservationReferences<'_, '_> {
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        if !self.visited.insert((expression, self.owner)) {
            return false;
        }
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Call(call) if call.layout_query.is_some() => {
                self.queries.push((expression, self.owner));
                typed_trees::references::visit_pending_constant_inputs(
                    self.program,
                    self,
                    expression,
                );
                false
            }
            _ => true,
        }
    }

    fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
        false
    }

    fn call_arguments(
        &mut self,
        _: &typed_trees::TypedTrees,
        _: typed_trees::references::StaticArgumentCall<'_>,
    ) -> bool {
        false
    }
}
