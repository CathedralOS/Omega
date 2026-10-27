//! Whole conjunctions of authored integer observations. Candidate atoms grant
//! nothing: every atom owes necessity, the complete conjunction owes its own
//! converse, and partial reads must form from preceding guards without P.
use super::super::{PredicateApplication, PredicateOrderingConsequence};
use super::*;

pub(in crate::proof_contracts::collection_predicates) fn exact_conjunction(
    program: &dyn crate::CheckingInput,
    application: &PredicateApplication<'_>,
) -> Option<Vec<PredicateOrderingConsequence>> {
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
    for expression in expressions {
        proof.spend()?;
        let Some(binary) = program.expression_table.binary_node(expression) else {
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
        ) || !has_builtin_binary_expression_meaning(program, machine, Some(entry), expression)
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
        if candidates
            .iter()
            .any(|(previous, _, _)| *previous == expression)
        {
            continue;
        }
        let reads = matches!(left, super::super::PredicateIntegerOperand::Read(_))
            || matches!(right, super::super::PredicateIntegerOperand::Read(_));
        candidates.push((
            expression,
            reads,
            PredicateOrderingConsequence {
                left,
                right,
                operator: binary.operator,
            },
        ));
    }
    // Pure guards precede any operation whose bounds they can establish. A
    // more complex dependency which this order cannot form simply refuses.
    candidates.sort_by_key(|(_, reads, _)| *reads);
    let mut selected = Vec::new();
    for (expression, _, condition) in candidates {
        let binary = program.expression_table.binary_node(expression)?;
        let mut necessary = true;
        for (path, goal) in proof.inert_comparison(&frame, initial.clone(), binary)? {
            if proof.generalized_activation(
                application.machine,
                0,
                arguments.clone(),
                path,
                &goal,
                Judgment::Consequence,
                0,
            ) != Some(true)
            {
                necessary = false;
                break;
            }
        }
        if necessary {
            selected.push((expression, condition));
        }
    }
    if selected.is_empty() {
        return None;
    }
    let mut paths = vec![initial];
    for (expression, _) in &selected {
        let mut next = Vec::new();
        for path in paths {
            // The ordinary expression reader independently checks each index
            // under only the preceding conjunction. Inert reads cannot pass here.
            for (path, value) in proof.expression(&frame, path, *expression, 0)? {
                let Value::Boolean(value) = value else {
                    return None;
                };
                if let Some(path) = proof.assume(path, &value, true)? {
                    next.push(path);
                }
            }
        }
        paths = next;
    }
    for path in paths {
        if proof.generalized_activation(
            application.machine,
            0,
            arguments.clone(),
            path,
            &Boolean::Constant(true),
            Judgment::RequiredResult,
            0,
        ) != Some(true)
        {
            return None;
        }
    }
    Some(
        selected
            .into_iter()
            .map(|(_, condition)| condition)
            .collect(),
    )
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
    fn inert_comparison(
        &mut self,
        frame: &Frame,
        path: Path,
        binary: &typed_trees::expression::TableBinaryExpression,
    ) -> Option<Vec<(Path, Boolean)>> {
        let mut rows = Vec::new();
        for (path, left) in self.inert_integer(frame, path, binary.left)? {
            for (path, right) in self.inert_integer(frame, path, binary.right)? {
                rows.push((
                    path,
                    Boolean::Comparison(compare(binary.operator, left.clone(), right)),
                ));
            }
        }
        (!rows.is_empty()).then_some(rows)
    }
}
