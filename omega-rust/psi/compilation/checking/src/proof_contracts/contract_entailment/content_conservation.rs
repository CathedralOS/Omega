//! Content-conservation guarantees proved from a checked body.
//!
//! The content normalizer (`value_custody::content_conservation`) already
//! reduces an authored `Owner::content(old(&input)) == separate(...)` ensures
//! to a closed plan: projection terms over structural places with an entry or
//! current revision, and `separate` lists. A bodied callable that authors such
//! a theorem (a selected partition provider) is judged here over the structural
//! judge's sole-arm normal-return value, the same environment that binds
//! locals to their initializer terms.
//!
//! Each projection evaluates the owner's normalized fields on actual captured
//! values. Parameter places rejoin the normal-return owner's exact typed read
//! and entry/current revision; no freshly projected name can replace a read.
//! Result places use the completed returned value. Both algebras use the
//! ordinary captured-integer judge, retaining selected arithmetic formation
//! and observation identities. Counted separation sums magnitudes; interval
//! separation additionally proves natural, ordered bounds and pairwise
//! disjointness before comparing signed boundary multiplicities. Equal lengths
//! never establish interval equality or confer backing authority.

mod interval_sets;
mod observed_contracts;
mod proof_dependencies;

pub use proof_dependencies::{
    ConditionalContentGuarantee, ContentConservationDependencies, ContentConservationImplication,
};

pub(super) use observed_contracts::{ObservedContentEquation, call_content_guarantees};

use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use language_semantics::content::{
    ContentAlgebraIdentity, ContentArithmeticOperator, ContentConservationPlan,
    ContentConservationTerm, ContentFieldSegment, ContentPlaceRoot, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionExpression, ContentProjectionPlan,
    ContentScalarExpression, ContentStructuralPlace,
};
use numerics::bignum::BigInt;
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;

/// The normalized plan of one authored content-conservation ensures fact on
/// this machine, if the fact is one.
pub(super) fn content_conservation_plan<'plans>(
    machine: &Machine,
    fact: ExpressionHandle,
    plans: &'plans [crate::value_custody::content_conservation::ContentConservationSourcePlan],
) -> Option<&'plans ContentConservationPlan> {
    plans
        .iter()
        .find(|source| source.source_expression == fact && source.plan.owner == machine.symbol)
        .map(|source| &source.plan)
}

/// A statement call to a unit-returning checked machine whose every state
/// body is empty computes nothing and writes nothing: core `drop(value)` is
/// the ordinary case. The captured term environment is unchanged by it, so
/// the sole-arm and unfolding walks step over it like a citation.
pub(super) fn is_empty_body_statement_call(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::statement::TableCall,
) -> bool {
    if !call.receiver.is_empty() || call.discards_result {
        return false;
    }
    let Some(machine) = program.machines().iter().find(|machine| {
        machine.symbol == call.target_symbol
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
    }) else {
        return false;
    };
    machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.body_is_present
        && program.machine_states(machine).iter().all(|state| {
            !state.return_type.is_valid()
                && program
                    .statement_table
                    .statements(state.statement_nodes)
                    .is_empty()
        })
}

pub(super) fn judge_content_conservation_with_dependencies(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'_>,
    result: &StructuralTerm,
    plan: &ContentConservationPlan,
    projections: &[ContentProjectionPlan],
) -> (StructuralJudgment, Option<ContentConservationDependencies>) {
    let parameter_projection = |root, path: &[ContentPlaceSegment], version| {
        judge.captured_parameter_projection(root, path, version)
    };
    let evaluator = ContentValues {
        program,
        judge,
        result,
        projections,
        parameter_projection: &parameter_projection,
    };
    let numeric = judge_numeric_content(&evaluator, plan);
    if matches!(numeric, StructuralJudgment::Proven) {
        return (numeric, None);
    }
    let dependency = observed_contracts::proof_dependencies(&evaluator, plan);
    if dependency
        .as_ref()
        .is_some_and(|proof| proof.independently_proved())
    {
        (StructuralJudgment::Proven, None)
    } else if let Some(dependency) = dependency {
        // The exact frame supports a law path, not yet a rooted theorem.
        (StructuralJudgment::Unknown, Some(dependency))
    } else {
        (numeric, None)
    }
}

fn judge_numeric_content(
    evaluator: &ContentValues<'_, '_>,
    plan: &ContentConservationPlan,
) -> StructuralJudgment {
    match plan.algebra {
        ContentAlgebraIdentity::CountedQuantity { .. } => {
            let (Some(left), Some(right)) = (
                evaluator.magnitude(plan.equation.left()),
                evaluator.magnitude(plan.equation.right()),
            ) else {
                return StructuralJudgment::Unknown;
            };
            evaluator.compare(BinaryOperator::Equal, &left, &right)
        }
        ContentAlgebraIdentity::IntervalSet { .. } => interval_sets::judge(&evaluator, plan),
    }
}

/// Retain one conditional proof for every observed completed-value alternative.
/// An empty vector means all observed alternatives were proved independently;
/// `None` means the observation was incomplete or an alternative had no proof.
/// Neither case grants custody: the consuming owner separately checks claims,
/// backing, access and the exact completion occurrence.
pub fn completed_value_content_conservation_dependencies(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    plan: &ContentConservationPlan,
    projections: &[ContentProjectionPlan],
) -> Option<Vec<ContentConservationDependencies>> {
    if plan.owner != machine.symbol || plan.callable != state.symbol {
        return None;
    }
    let mut complete = true;
    let mut dependencies = Vec::new();
    let observed = super::completed_observations::observe_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        |judge, value| {
            let (judgment, conditional) = judge_content_conservation_with_dependencies(
                program,
                judge,
                &value,
                plan,
                projections,
            );
            match (judgment, conditional) {
                (StructuralJudgment::Proven, _) => {}
                (_, Some(proof)) => dependencies.push(proof),
                _ => complete = false,
            }
        },
    );
    (observed == Some(true) && complete).then_some(dependencies)
}

/// Provisional numeric bindings carry possible law dependencies; they do not
/// publish independently rooted guarantees or authorize claim establishment.
pub fn completed_value_has_content_conservation_candidate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    plan: &ContentConservationPlan,
    projections: &[ContentProjectionPlan],
) -> bool {
    completed_value_content_conservation_dependencies(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        plan,
        projections,
    )
    .is_some()
}

/// Final consumers accept conditional equations only through the one rooted
/// source frontier. Every actual completed-value alternative must be proved.
pub fn completed_value_proves_content_conservation(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    plan: &ContentConservationPlan,
    projections: &[ContentProjectionPlan],
    available: &[ContentConservationPlan],
) -> bool {
    completed_value_content_conservation_dependencies(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        plan,
        projections,
    )
    .is_some_and(|proofs| {
        proofs.iter().all(|proof| {
            proof
                .derivation_depth(|source| available.contains(source).then_some(0))
                .is_some()
        })
    })
}

/// The consumed owner's current projection must still equal the input revision
/// used by the conservation theorem. Evaluate both in the exact completion
/// frame, including completed call guarantees and intervening writes. Nominal
/// claim correspondence supplies neither this equality nor permission to read.
pub fn completed_value_preserves_entry_projection(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    current: &crate::CanonicalPlace,
    entry: &crate::CanonicalPlace,
) -> bool {
    let Some(reference) =
        crate::expression_result_type_reference(program, machine, state, expression)
    else {
        return false;
    };
    let (Some(current_reference), Some(entry_reference)) = (
        crate::canonical_place_type_reference(program, state.symbol, statement_index, current),
        crate::canonical_place_type_reference(program, state.symbol, statement_index, entry),
    ) else {
        return false;
    };
    let mut proven = true;
    let coverage = super::completed_observations::observe_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        |judge, _| {
            let (Some(current), Some(entry)) = (
                judge.captured_place_projection(current, ContentPlaceVersion::Current),
                judge.captured_place_projection(entry, ContentPlaceVersion::Entry),
            ) else {
                proven = false;
                return;
            };
            let embed = |value, reference| StructuralTerm::IntegerEmbedding {
                reference,
                value: Box::new(judge.resolve(value)),
            };
            proven &= matches!(
                judge.judge_observed_predicate(
                    mathematical(
                        BinaryOperator::Equal,
                        embed(current, current_reference),
                        embed(entry, entry_reference)
                    ),
                    0,
                ),
                StructuralJudgment::Proven
            );
        },
    );
    coverage == Some(true) && proven
}

/// One typed address transport demanded by a boundary implementation's custody
/// correspondence. Entry and current places are distinct observations: a
/// constant content projection cannot hide a changed backing address.
pub struct ContentBackingAddressGoal {
    pub entry: crate::CanonicalPlace,
    pub current: crate::CanonicalPlace,
    pub result: ContentStructuralPlace,
}

/// Judge all candidate backing transports in the same completed-value frame.
/// These are ordinary address equalities, not domain membership or fresh
/// issuance evidence. The custody owner must still authenticate the complete
/// source/result roster, authorized requirement and current live claims.
pub fn completed_value_proves_backing_addresses(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    reference: typed_trees::types::TypeReferenceHandle,
    goals: &[ContentBackingAddressGoal],
) -> Result<Vec<bool>, &'static str> {
    use typed_trees::types::PrimitiveType;

    if goals.is_empty() {
        return Err("backing correspondence has no address observations");
    }
    let references = goals
        .iter()
        .map(|goal| {
            let entry = crate::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &goal.entry,
            )
            .ok_or("backing entry address has no typed observation")?;
            let current = crate::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &goal.current,
            )
            .ok_or("backing current address has no typed observation")?;
            if [entry, current].into_iter().any(|reference| {
                program.primitive_type_reference(reference) != Some(PrimitiveType::Addr)
            }) {
                return Err("backing correspondence requires exact address carriers");
            }
            if goal.result.root != ContentPlaceRoot::Result
                || goal.result.version != ContentPlaceVersion::Current
            {
                return Err("backing correspondence does not name the completed result");
            }
            Ok((entry, current))
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let mut proven = vec![true; goals.len()];
    let coverage = super::completed_observations::observe_completed_values(
        program,
        machine,
        state,
        statement_index,
        expression,
        reference,
        |judge, value| {
            for ((goal, (entry_reference, current_reference)), proven) in
                goals.iter().zip(&references).zip(&mut proven)
            {
                let (Some(entry), Some(current), Some((result_reference, result))) = (
                    judge.captured_place_projection(&goal.entry, ContentPlaceVersion::Entry),
                    judge.captured_place_projection(&goal.current, ContentPlaceVersion::Current),
                    judge.completed_value_projection(
                        reference,
                        value.clone(),
                        &goal.result.segments,
                    ),
                ) else {
                    *proven = false;
                    continue;
                };
                if program.primitive_type_reference(result_reference) != Some(PrimitiveType::Addr) {
                    *proven = false;
                    continue;
                }
                let embed = |term, reference| StructuralTerm::IntegerEmbedding {
                    reference,
                    value: Box::new(judge.resolve(term)),
                };
                let entry = embed(entry, *entry_reference);
                let current = embed(current, *current_reference);
                let result = embed(result, result_reference);
                *proven &=
                    [(&entry, &current), (&current, &result)]
                        .into_iter()
                        .all(|(left, right)| {
                            matches!(
                                judge.judge_observed_predicate(
                                    mathematical(
                                        BinaryOperator::Equal,
                                        left.clone(),
                                        right.clone()
                                    ),
                                    0,
                                ),
                                StructuralJudgment::Proven
                            )
                        });
            }
        },
    );
    if coverage != Some(true) {
        return Err("backing correspondence lacks complete value observation coverage");
    }
    Ok(proven)
}

struct ContentValues<'judge, 'program> {
    program: &'judge dyn crate::CheckingInput,
    judge: &'judge StructuralJudge<'program>,
    result: &'judge StructuralTerm,
    projections: &'judge [ContentProjectionPlan],
    parameter_projection: &'judge dyn Fn(
        symbols::SymbolHandle,
        &[ContentPlaceSegment],
        ContentPlaceVersion,
    ) -> Option<StructuralTerm>,
}

impl ContentValues<'_, '_> {
    fn magnitude(&self, term: &ContentConservationTerm) -> Option<StructuralTerm> {
        match term {
            ContentConservationTerm::Projection {
                projection_machine,
                subject,
                ..
            } => {
                let projection = self
                    .projections
                    .iter()
                    .find(|projection| projection.machine == *projection_machine)?;
                let ContentProjectionExpression::CountedQuantity { magnitude } =
                    &projection.expression
                else {
                    return None;
                };
                self.scalar(subject, magnitude)
            }
            ContentConservationTerm::Separate(terms) => {
                terms.iter().try_fold(integer(0), |sum, term| {
                    Some(mathematical(
                        BinaryOperator::Add,
                        sum,
                        self.magnitude(term)?,
                    ))
                })
            }
        }
    }

    fn compare(
        &self,
        operator: BinaryOperator,
        left: &StructuralTerm,
        right: &StructuralTerm,
    ) -> StructuralJudgment {
        self.judge
            .judge_observed_predicate(mathematical(operator, left.clone(), right.clone()), 0)
    }

    fn subject_term(&self, place: &ContentStructuralPlace) -> Option<StructuralTerm> {
        let root = match &place.root {
            ContentPlaceRoot::Parameter { name, .. } => StructuralTerm::Variable(name.clone()),
            ContentPlaceRoot::Result => self.result.clone(),
        };
        place
            .segments
            .iter()
            .try_fold(root, |subject, segment| match segment {
                ContentPlaceSegment::Field(field) => Some(self.field(subject, field)),
                ContentPlaceSegment::Case(_) | ContentPlaceSegment::FixedIndex(_) => None,
            })
    }

    fn field(&self, subject: StructuralTerm, field: &ContentFieldSegment) -> StructuralTerm {
        self.judge.resolve(StructuralTerm::Projection {
            subject: Box::new(subject),
            path: field.name.clone(),
        })
    }

    fn scalar(
        &self,
        subject: &ContentStructuralPlace,
        expression: &ContentScalarExpression,
    ) -> Option<StructuralTerm> {
        match expression {
            ContentScalarExpression::SubjectField(path)
            | ContentScalarExpression::RuntimeScalarEmbedding(path) => {
                let term = match &subject.root {
                    ContentPlaceRoot::Parameter { symbol, .. } => {
                        let mut segments = subject.segments.clone();
                        segments.extend(path.iter().cloned().map(ContentPlaceSegment::Field));
                        (self.parameter_projection)(*symbol, &segments, subject.version)?
                    }
                    ContentPlaceRoot::Result => {
                        let value = self.subject_term(subject)?;
                        path.iter()
                            .fold(value, |value, field| self.field(value, field))
                    }
                };
                let value = self.judge.resolve(term);
                // Proof-natural fields already denote mathematical values. Only
                // runtime embeddings need an authenticated scalar carrier; using
                // that carrier path for both would discard natural-value facts.
                if matches!(
                    expression,
                    ContentScalarExpression::RuntimeScalarEmbedding(_)
                ) {
                    let field = path.last()?;
                    let (_, declaration) = self.program.any_field_owner(field.symbol)?;
                    super::structural_terms::scalar_operand_meaning(
                        self.program,
                        declaration.type_reference,
                        &value,
                    )?;
                    Some(StructuralTerm::IntegerEmbedding {
                        reference: declaration.type_reference,
                        value: Box::new(value),
                    })
                } else {
                    Some(value)
                }
            }
            ContentScalarExpression::Natural(text) => {
                BigInt::from_decimal_str(text).map(StructuralTerm::Integer)
            }
            ContentScalarExpression::Successor(inner) => Some(StructuralTerm::MathematicalBinary {
                operator: BinaryOperator::Add,
                left: Box::new(self.scalar(subject, inner)?),
                right: Box::new(StructuralTerm::Integer(BigInt::from_u64(1))),
            }),
            ContentScalarExpression::Arithmetic {
                operator,
                left,
                right,
            } => {
                let left = self.scalar(subject, left)?;
                let right = self.scalar(subject, right)?;
                Some(StructuralTerm::MathematicalBinary {
                    operator: match operator {
                        ContentArithmeticOperator::Add => BinaryOperator::Add,
                        ContentArithmeticOperator::Subtract => BinaryOperator::Subtract,
                        ContentArithmeticOperator::Multiply => BinaryOperator::Multiply,
                    },
                    left: Box::new(left),
                    right: Box::new(right),
                })
            }
        }
    }
}

fn integer(value: u64) -> StructuralTerm {
    StructuralTerm::Integer(BigInt::from_u64(value))
}

fn mathematical(
    operator: BinaryOperator,
    left: StructuralTerm,
    right: StructuralTerm,
) -> StructuralTerm {
    StructuralTerm::MathematicalBinary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}
