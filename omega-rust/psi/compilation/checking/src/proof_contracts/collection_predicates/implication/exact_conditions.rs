//! Whole conjunctions of typed Boolean and integer observations. Candidate atoms grant
//! nothing: every atom owes necessity, the complete conjunction owes its own
//! converse, and partial reads must form from preceding guards without P.
use super::super::{
    PredicateApplication, PredicateBooleanSubject, PredicateCondition, PredicateIntegerOperand,
    PredicateOrderingConsequence,
};
use super::*;

pub(in crate::proof_contracts::collection_predicates) fn exact_conjunction(
    program: &dyn crate::CheckingInput,
    application: &PredicateApplication<'_>,
) -> Option<Vec<PredicateCondition>> {
    let machine = program.machines().get(application.machine)?;
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);
    if !admitted_activation_parameters(program, parameters, application.arguments.len()) {
        return None;
    }
    let mut proof = Implication {
        program,
        context: machine,
        remaining: WORK_BUDGET,
        next_identity: 0,
        lemmas: Vec::new(),
        updates: Vec::new(),
        records: Vec::new(),
        antecedents: Vec::new(),
    };
    let mut initial = Path::default();
    let arguments = parameters
        .iter()
        .map(|parameter| proof.parameter(parameter.type_reference, &mut initial))
        .collect::<Option<Vec<_>>>()?;
    let frame = Frame {
        machine: application.machine,
        state: 0,
        bindings: parameters
            .iter()
            .zip(&arguments)
            .map(|(parameter, value)| (parameter.symbol, value.clone()))
            .collect(),
    };
    let mut expressions = Vec::new();
    for statement in program.statement_table.statements(entry.statement_nodes) {
        proof.spend()?;
        crate::expression_traversal::collect_statement_expression_trees(
            program,
            statement,
            &mut expressions,
        );
    }
    let mut candidates = Vec::new();
    let mut seen = Vec::new();
    let mut seen_booleans = Vec::new();
    for expression in &expressions {
        proof.spend()?;
        if let Some(subject) = super::super::predicate_boolean_subject(
            program,
            machine.symbol,
            application.arguments,
            *expression,
        ) {
            let identity =
                crate::immutable_entry_boolean_projection(program, machine.symbol, *expression)?;
            if !seen_booleans.contains(&identity) {
                seen_booleans.push(identity);
                for holds in [true, false] {
                    candidates.push(Candidate::Boolean {
                        expression: *expression,
                        subject: subject.clone(),
                        holds,
                    });
                }
            }
        }
        let Some(binary) = program.expression_table.binary_node(*expression) else {
            continue;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal
                | BinaryOperator::NotEqual
                | BinaryOperator::Less
                | BinaryOperator::LessOrEqual
                | BinaryOperator::Greater
                | BinaryOperator::GreaterOrEqual
        ) || !has_builtin_binary_expression_meaning(program, machine, Some(entry), *expression)
            || seen.contains(expression)
        {
            continue;
        }
        let Some(left) = super::super::predicate_integer_operand(
            program,
            machine.symbol,
            application.arguments,
            binary.left,
        ) else {
            continue;
        };
        let Some(right) = super::super::predicate_integer_operand(
            program,
            machine.symbol,
            application.arguments,
            binary.right,
        ) else {
            continue;
        };
        seen.push(*expression);
        candidates.push(Candidate::Ordering {
            left: binary.left,
            right: binary.right,
            condition: PredicateOrderingConsequence {
                left,
                right,
                operator: binary.operator,
            },
        });
    }
    // Retain authored guards first. Every later partial read must form from
    // preceding conditions, independently of the predicate being replaced.
    candidates.sort_by_key(Candidate::has_reads);
    let mut selected = Vec::new();
    let mut paths = vec![initial.clone()];
    for candidate in candidates {
        if necessary(
            &mut proof,
            application,
            &frame,
            &arguments,
            &initial,
            &candidate,
        )? {
            paths = proof.form_condition(&frame, &paths, &candidate)?;
            selected.push(candidate.into_condition());
        }
    }
    if sufficient(&mut proof, application, &arguments, &paths)? {
        return (!selected.is_empty()).then_some(selected);
    }

    // A helper may establish a relation without spelling that comparison in
    // this body. Questions retain original callee expressions, never atoms
    // imported from a separate proof or a reverse lookup of repeated actuals.
    let mut subjects = Vec::new();
    for expression in expressions {
        proof.spend()?;
        let Some(reference) =
            crate::expression_result_type_reference(program, machine, entry, expression)
        else {
            continue;
        };
        let Some(carrier) = program
            .primitive_type_reference(reference)
            .filter(|carrier| carrier.is_fixed_integer())
        else {
            continue;
        };
        if program.arithmetic_domain_for_type_reference(reference)
            != numerics::arithmetic::ArithmeticDomain::Exact
        {
            continue;
        }
        let Some(operand) = super::super::predicate_integer_operand(
            program,
            machine.symbol,
            application.arguments,
            expression,
        ) else {
            continue;
        };
        let identity = match &operand {
            PredicateIntegerOperand::Argument(_) => {
                let (position, projection) =
                    crate::immutable_entry_integer_projection(program, machine.symbol, expression)?;
                SubjectIdentity::Parameter(position, projection)
            }
            PredicateIntegerOperand::Read(_) => SubjectIdentity::Read(
                crate::immutable_entry_indexed_read_identity(program, expression)?,
            ),
            PredicateIntegerOperand::Integer(_) => continue,
        };
        if subjects
            .iter()
            .any(|previous: &Subject| previous.identity == identity)
        {
            continue;
        }
        subjects.push(Subject {
            identity,
            expression,
            operand,
            carrier,
        });
    }
    // Authored read operands are the relevant questions before unrelated
    // scalar-field pairs. All scans and judgments share the original budget.
    subjects.sort_by_key(|subject| !matches!(subject.identity, SubjectIdentity::Read(_)));
    for left in 0..subjects.len() {
        for right in (left + 1)..subjects.len() {
            if subjects[left].carrier != subjects[right].carrier {
                continue;
            }
            for operator in [
                BinaryOperator::NotEqual,
                BinaryOperator::Equal,
                BinaryOperator::Less,
                BinaryOperator::LessOrEqual,
                BinaryOperator::Greater,
                BinaryOperator::GreaterOrEqual,
            ] {
                proof.spend()?;
                let candidate = Candidate::Ordering {
                    left: subjects[left].expression,
                    right: subjects[right].expression,
                    condition: PredicateOrderingConsequence {
                        left: subjects[left].operand.clone(),
                        right: subjects[right].operand.clone(),
                        operator,
                    },
                };
                if !necessary(
                    &mut proof,
                    application,
                    &frame,
                    &arguments,
                    &initial,
                    &candidate,
                )? {
                    continue;
                }
                paths = proof.form_condition(&frame, &paths, &candidate)?;
                selected.push(candidate.into_condition());
                if sufficient(&mut proof, application, &arguments, &paths)? {
                    return Some(selected);
                }
            }
        }
    }
    None
}

enum Candidate {
    Ordering {
        left: ExpressionHandle,
        right: ExpressionHandle,
        condition: PredicateOrderingConsequence,
    },
    Boolean {
        expression: ExpressionHandle,
        subject: PredicateBooleanSubject,
        holds: bool,
    },
}
impl Candidate {
    fn has_reads(&self) -> bool {
        match self {
            Self::Ordering { condition, .. } => {
                matches!(condition.left, PredicateIntegerOperand::Read(_))
                    || matches!(condition.right, PredicateIntegerOperand::Read(_))
            }
            Self::Boolean { .. } => false,
        }
    }
    fn into_condition(self) -> PredicateCondition {
        match self {
            Self::Ordering { condition, .. } => PredicateCondition::Ordering(condition),
            Self::Boolean { subject, holds, .. } => PredicateCondition::Boolean { subject, holds },
        }
    }
}
#[derive(PartialEq, Eq)]
enum SubjectIdentity {
    Parameter(usize, Vec<facts::PlaceSegment>),
    Read(crate::ImmutableEntryReadIdentity),
}
struct Subject {
    identity: SubjectIdentity,
    expression: ExpressionHandle,
    operand: PredicateIntegerOperand,
    carrier: typed_trees::types::PrimitiveType,
}

fn necessary(
    proof: &mut Implication<'_>,
    application: &PredicateApplication<'_>,
    frame: &Frame,
    arguments: &[Value],
    initial: &Path,
    candidate: &Candidate,
) -> Option<bool> {
    for (path, goal) in proof.condition(frame, initial.clone(), candidate, true)? {
        if proof.generalized_activation(
            application.machine,
            0,
            arguments.to_vec(),
            path,
            &goal,
            Judgment::Consequence,
            0,
        ) != Some(true)
        {
            return Some(false);
        }
    }
    Some(true)
}

fn sufficient(
    proof: &mut Implication<'_>,
    application: &PredicateApplication<'_>,
    arguments: &[Value],
    paths: &[Path],
) -> Option<bool> {
    for path in paths {
        proof.spend()?;
        if proof.generalized_activation(
            application.machine,
            0,
            arguments.to_vec(),
            path.clone(),
            &Boolean::Constant(true),
            Judgment::RequiredResult,
            0,
        ) != Some(true)
        {
            return Some(false);
        }
    }
    Some(true)
}

impl Implication<'_> {
    fn inert_integer(
        &mut self,
        frame: &Frame,
        path: Path,
        expression: ExpressionHandle,
    ) -> Option<Vec<(Path, Integer)>> {
        if self
            .program
            .expression_table
            .indexed_node(expression)
            .is_some()
        {
            return self.inert_read(frame, path, expression);
        }
        self.expression(frame, path, expression, 0)?
            .into_iter()
            .map(|(path, value)| {
                let Value::Integer { value, .. } = value else {
                    return None;
                };
                Some((path, value))
            })
            .collect()
    }
    fn condition(
        &mut self,
        frame: &Frame,
        path: Path,
        candidate: &Candidate,
        inert: bool,
    ) -> Option<Vec<(Path, Boolean)>> {
        if let Candidate::Boolean {
            expression, holds, ..
        } = candidate
        {
            return self
                .expression(frame, path, *expression, 0)?
                .into_iter()
                .map(|(path, value)| {
                    let Value::Boolean(value) = value else {
                        return None;
                    };
                    Some((
                        path,
                        if *holds {
                            value
                        } else {
                            Boolean::Not(Box::new(value))
                        },
                    ))
                })
                .collect();
        }
        let Candidate::Ordering {
            left,
            right,
            condition,
        } = candidate
        else {
            return None;
        };
        let mut rows = Vec::new();
        for (path, left) in self.condition_integer(frame, path, *left, inert)? {
            for (path, right) in self.condition_integer(frame, path, *right, inert)? {
                rows.push((
                    path,
                    Boolean::Comparison(compare(condition.operator, left.clone(), right)),
                ));
            }
        }
        (!rows.is_empty()).then_some(rows)
    }

    fn condition_integer(
        &mut self,
        frame: &Frame,
        path: Path,
        expression: ExpressionHandle,
        inert: bool,
    ) -> Option<Vec<(Path, Integer)>> {
        if inert {
            return self.inert_integer(frame, path, expression);
        }
        self.expression(frame, path, expression, 0)?
            .into_iter()
            .map(|(path, value)| {
                let Value::Integer { value, .. } = value else {
                    return None;
                };
                Some((path, value))
            })
            .collect()
    }

    fn form_condition(
        &mut self,
        frame: &Frame,
        paths: &[Path],
        candidate: &Candidate,
    ) -> Option<Vec<Path>> {
        let mut next = Vec::new();
        for path in paths {
            // Ordinary source readers own each bound and exact occurrence;
            // no inert question can supply executable read formation.
            for (path, value) in self.condition(frame, path.clone(), candidate, false)? {
                if let Some(path) = self.assume(path, &value, true)? {
                    next.push(path);
                }
            }
        }
        Some(next)
    }
}
