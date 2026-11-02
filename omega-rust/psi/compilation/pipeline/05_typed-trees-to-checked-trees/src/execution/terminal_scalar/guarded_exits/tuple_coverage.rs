//! Finite guard columns share one coverage algorithm across pure and computed operands.
/// One row of a decomposed multi-subject guard: the finite columns an
/// authored tuple arm constrains. `Or` unions widen a single column;
/// conjuncts outside the column vocabulary leave `residual` — the row
/// then proves no cylinder and can only consume coverage other rows
/// provide.
#[derive(Default)]
pub(super) struct TupleRow {
    columns: Vec<(TupleSubject, Vec<TupleValue>)>,
    residual: bool,
}

/// The operand one matrix column selects on. A case column is a
/// structural path whose active variant the arm requires; a Boolean
/// column is a scalar operand the arm pins to a literal.
#[derive(PartialEq)]
enum TupleSubject {
    Case(checked_trees::CheckedStructuralCaseSubject),
    ComputedCase {
        subject: checked_trees::CheckedScalarComputationStructuralArgument,
        owner: symbols::SymbolHandle,
    },
    Boolean(checked_trees::CheckedBooleanExpression),
}

#[derive(Clone, PartialEq)]
enum TupleValue {
    Case(checked_trees::CheckedMemberIdentity),
    Boolean(bool),
}

/// Conjunction semantics merge on a shared subject: two columns naming
/// the same operand intersect their admitted values.
fn merge_column(row: &mut TupleRow, subject: TupleSubject, allowed: Vec<TupleValue>) {
    if let Some((_, present)) = row
        .columns
        .iter_mut()
        .find(|(candidate, _)| *candidate == subject)
    {
        present.retain(|value| allowed.contains(value));
    } else {
        row.columns.push((subject, allowed));
    }
}

/// Decompose one `when` guard into its tuple-matrix row. Recognized
/// conjuncts are exact case memberships, same-subject `Or` unions of
/// them, and `operand == <Boolean literal>` equalities; anything else is
/// a residual the coverage pass cannot model.
pub(super) fn tuple_row(boolean: &checked_trees::CheckedBooleanExpression) -> TupleRow {
    let mut row = TupleRow::default();
    tuple_conjunction(boolean, &mut row);
    row
}

fn tuple_conjunction(boolean: &checked_trees::CheckedBooleanExpression, row: &mut TupleRow) {
    use checked_trees::CheckedBooleanExpression as Boolean;
    match boolean {
        Boolean::And { left, right } => {
            tuple_conjunction(left, row);
            tuple_conjunction(right, row);
        }
        Boolean::StructuralCaseMembership { subject, case } => {
            merge_column(
                row,
                TupleSubject::Case(subject.clone()),
                vec![TupleValue::Case(case.clone())],
            );
        }
        Boolean::Or { .. } => {
            // A domain membership lowers to a case disjunction on one
            // subject; mixing subjects or non-membership leaves is not a
            // column.
            let mut pending = vec![boolean];
            let mut subject: Option<&checked_trees::CheckedStructuralCaseSubject> = None;
            let mut cases = Vec::new();
            while let Some(node) = pending.pop() {
                match node {
                    Boolean::Or { left, right } => {
                        pending.push(left);
                        pending.push(right);
                    }
                    Boolean::StructuralCaseMembership {
                        subject: leaf,
                        case,
                    } => {
                        if subject.is_some_and(|prior| *prior != *leaf) {
                            row.residual = true;
                            return;
                        }
                        subject = Some(leaf);
                        if !cases.contains(case) {
                            cases.push(case.clone());
                        }
                    }
                    _ => {
                        row.residual = true;
                        return;
                    }
                }
            }
            let Some(subject) = subject else {
                row.residual = true;
                return;
            };
            merge_column(
                row,
                TupleSubject::Case(subject.clone()),
                cases.into_iter().map(TupleValue::Case).collect(),
            );
        }
        Boolean::Equal { left, right } => {
            let (operand, literal) = match (left.as_ref(), right.as_ref()) {
                (operand, Boolean::Constant(literal)) | (Boolean::Constant(literal), operand) => {
                    (operand, *literal)
                }
                _ => {
                    row.residual = true;
                    return;
                }
            };
            merge_column(
                row,
                TupleSubject::Boolean(operand.clone()),
                vec![TupleValue::Boolean(literal)],
            );
        }
        _ => row.residual = true,
    }
}

/// Whether the decomposed tuple rows provably tile their subjects'
/// finite domains. Each case subject's domain enumerates through the
/// data owner its path resolves to; a Boolean subject's domain is the
/// two literals. A row covers a tuple when every one of its columns
/// admits the tuple's value — a subject the row does not constrain is a
/// wildcard — and rows carrying a residual prove no slice.
pub(super) fn tuple_rows_cover(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    rows: &[Option<TupleRow>],
) -> Option<()> {
    let rows: Vec<&TupleRow> = rows
        .iter()
        .map(Option::as_ref)
        .collect::<Option<Vec<_>>>()?;
    let mut subjects = Vec::new();
    for row in &rows {
        for (subject, _) in &row.columns {
            crate::collections::push_unique(&mut subjects, subject);
        }
    }
    if subjects.is_empty() {
        return None;
    }
    let parameters = program.state_parameters(state);
    let mut domains = Vec::with_capacity(subjects.len());
    for subject in &subjects {
        match subject {
            TupleSubject::Case(field) => {
                let field = field.parameter_field()?;
                let (_, _, receiver, _) = crate::values::resolve_structural_parameter_path(
                    program,
                    parameters,
                    field.parameter_position,
                    &field.path,
                )?;
                let definition = crate::values::structural_data(program, receiver)?;
                let cases: Vec<TupleValue> = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| match member {
                        typed_trees::data::DataMember::Variant(variant) => Some(TupleValue::Case(
                            checked_trees::CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            ),
                        )),
                        typed_trees::data::DataMember::Field(_) => None,
                    })
                    .collect::<Vec<_>>();
                if cases.is_empty() {
                    return None;
                }
                domains.push(cases);
            }
            TupleSubject::ComputedCase { owner, .. } => {
                let definition = program.data_definition_by_symbol(*owner)?;
                let cases = program
                    .data_members(definition)
                    .iter()
                    .filter_map(|member| {
                        let variant = member.as_variant()?;
                        Some(TupleValue::Case(
                            checked_trees::CheckedMemberIdentity::of_member(
                                variant.identity,
                                variant.name.clone(),
                            ),
                        ))
                    })
                    .collect::<Vec<_>>();
                if cases.is_empty() {
                    return None;
                }
                domains.push(cases);
            }
            TupleSubject::Boolean(_) => {
                domains.push(vec![TupleValue::Boolean(false), TupleValue::Boolean(true)]);
            }
        }
    }
    // Exhaustive tuple-matrix checks stay small by construction; a
    // product beyond the cap is out of scope for this pass.
    let extent: usize = domains.iter().map(Vec::len).product();
    if extent == 0 || extent > 4096 {
        return None;
    }
    let mut indexes = vec![0usize; domains.len()];
    loop {
        let covered = rows.iter().any(|row| {
            !row.residual
                && row.columns.iter().all(|(subject, allowed)| {
                    subjects
                        .iter()
                        .position(|candidate| *candidate == subject)
                        .is_some_and(|position| {
                            allowed.contains(&domains[position][indexes[position]])
                        })
                })
        });
        if !covered {
            return None;
        }
        let mut axis = domains.len();
        loop {
            if axis == 0 {
                return Some(());
            }
            axis -= 1;
            indexes[axis] += 1;
            if indexes[axis] < domains[axis].len() {
                break;
            }
            indexes[axis] = 0;
        }
    }
}

/// Read Boolean composition in the retained computation's own operand namespace.
/// Unsupported operations leave a residual, just as in a pure Boolean row.
pub(super) fn computation_row(
    program: &dyn validation::CheckingInput,
    plans: &checked_trees::CheckedScalarComputationPlans,
    root: checked_trees::CheckedScalarComputationHandle,
) -> TupleRow {
    fn append(target: &mut TupleRow, source: TupleRow) {
        target.residual |= source.residual;
        for (subject, values) in source.columns {
            merge_column(target, subject, values);
        }
    }
    fn union(mut left: TupleRow, right: TupleRow) -> TupleRow {
        if !left.residual
            && !right.residual
            && let ([(subject, values)], [(other, extra)]) =
                (left.columns.as_mut_slice(), right.columns.as_slice())
            && subject == other
        {
            for value in extra {
                if !values.contains(value) {
                    values.push(value.clone());
                }
            }
            return left;
        }
        TupleRow {
            residual: true,
            ..Default::default()
        }
    }
    fn constant(
        plans: &checked_trees::CheckedScalarComputationPlans,
        root: checked_trees::CheckedScalarComputationHandle,
    ) -> Option<bool> {
        use checked_trees::{
            CheckedBooleanExpression as Boolean, CheckedScalarComputationKind as Kind,
            CheckedScalarExpression as Scalar,
        };
        if !plans.nodes.is_valid(root) {
            return None;
        }
        match &plans.nodes.get(root).kind {
            Kind::Value(Scalar::Boolean(boolean)) => match boolean.as_ref() {
                Boolean::Constant(value) => Some(*value),
                _ => None,
            },
            _ => None,
        }
    }
    fn template(
        program: &dyn validation::CheckingInput,
        plans: &checked_trees::CheckedScalarComputationPlans,
        boolean: &checked_trees::CheckedBooleanExpression,
        operands: &[checked_trees::CheckedScalarComputationHandle],
        active: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
    ) -> TupleRow {
        use checked_trees::CheckedBooleanExpression as Boolean;
        match boolean {
            Boolean::Parameter { position } => operands
                .get(*position)
                .map(|operand| collect(program, plans, *operand, active))
                .unwrap_or_else(|| TupleRow {
                    residual: true,
                    ..Default::default()
                }),
            Boolean::And { left, right } => {
                let mut row = template(program, plans, left, operands, active);
                append(&mut row, template(program, plans, right, operands, active));
                row
            }
            Boolean::Or { left, right } => union(
                template(program, plans, left, operands, active),
                template(program, plans, right, operands, active),
            ),
            _ => TupleRow {
                residual: true,
                ..Default::default()
            },
        }
    }
    fn collect(
        program: &dyn validation::CheckingInput,
        plans: &checked_trees::CheckedScalarComputationPlans,
        root: checked_trees::CheckedScalarComputationHandle,
        active: &mut Vec<checked_trees::CheckedScalarComputationHandle>,
    ) -> TupleRow {
        use checked_trees::{
            CheckedScalarComputationKind as Kind, CheckedScalarExpression as Scalar,
        };
        if !plans.nodes.is_valid(root) || active.contains(&root) {
            return TupleRow {
                residual: true,
                ..Default::default()
            };
        }
        active.push(root);
        let row = match &plans.nodes.get(root).kind {
            Kind::CaseMembership { subject, case, .. } => {
                let owner = program.symbols.get(*case).parent;
                let variant = program.data_definition_by_symbol(owner).and_then(|data| {
                    program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| member.as_variant())
                        .find(|variant| variant.symbol == *case)
                });
                if let Some(variant) = variant {
                    TupleRow {
                        columns: vec![(
                            TupleSubject::ComputedCase {
                                subject: subject.clone(),
                                owner,
                            },
                            vec![TupleValue::Case(
                                checked_trees::CheckedMemberIdentity::of_member(
                                    variant.identity,
                                    variant.name.clone(),
                                ),
                            )],
                        )],
                        residual: false,
                    }
                } else {
                    TupleRow {
                        residual: true,
                        ..Default::default()
                    }
                }
            }
            // Short-circuit Boolean operations retain their ordinary Select
            // graph: A && B selects B/false; A || B selects true/B.
            Kind::Select {
                condition,
                when_true,
                when_false,
                ..
            } if constant(plans, *when_false) == Some(false) => {
                let mut row = collect(program, plans, *condition, active);
                append(&mut row, collect(program, plans, *when_true, active));
                row
            }
            Kind::Select {
                condition,
                when_true,
                when_false,
                ..
            } if constant(plans, *when_true) == Some(true) => union(
                collect(program, plans, *condition, active),
                collect(program, plans, *when_false, active),
            ),
            Kind::Value(Scalar::Boolean(boolean)) => tuple_row(boolean),
            Kind::Apply {
                expression: Scalar::Boolean(boolean),
                operands,
                ..
            } => {
                if let Some(operands) = plans.operands.span(*operands) {
                    template(program, plans, boolean, operands, active)
                } else {
                    TupleRow {
                        residual: true,
                        ..Default::default()
                    }
                }
            }
            _ => TupleRow {
                residual: true,
                ..Default::default()
            },
        };
        active.pop();
        row
    }
    collect(program, plans, root, &mut Vec::new())
}
