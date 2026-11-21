//! Read count predicates owed by an open constructor and a zero-initialized record field.
use omega_psi::{
    ContractClause, DeclarationId, LoweredPackage, MachineTemplate, PredicateOperator, PredicateStep, ScalarType, TemplateOperand, TemplateResult,
    TemplateStep, TypeExpressionStep, TypeShape,
};

#[derive(Debug, PartialEq, Eq)]
pub struct ZeroCountClause<'a> {
    pub parent: Option<&'a DeclarationId>,
    pub field: &'a DeclarationId,
    pub comparison: PredicateOperator,
    pub limit: u64,
}

fn count_clause<'a>(
    clause: &'a ContractClause,
    parent: Option<&'a DeclarationId>,
) -> Option<ZeroCountClause<'a>> {
    let [
        PredicateStep::SelfValue,
        PredicateStep::Field(field),
        PredicateStep::Integer(limit),
        PredicateStep::Apply(comparison),
    ] = clause.predicate.as_slice()
    else {
        return None;
    };
    Some(ZeroCountClause {
        parent,
        field,
        comparison: *comparison,
        limit: limit.try_into().ok()?,
    })
}

pub fn returned_counts<'a>(
    package: &'a LoweredPackage,
    machine: &'a MachineTemplate,
) -> Option<Vec<ZeroCountClause<'a>>> {
    let Some(TemplateResult::Operand(TemplateOperand::Step(result))) = &machine.result else {
        return None;
    };
    let TemplateStep::Construct(construction) = machine.body.get(*result)? else {
        return None;
    };
    let template = package
        .data_templates
        .iter()
        .find(|data| data.declaration == construction.declaration)?;
    if !construction.establishes || !template.cases.is_empty() {
        return None;
    }
    let unsigned = ScalarType::Integer { signed: false, width: 64 };
    let mut clauses = Vec::new();
    for clause in &template.requires {
        let clause = count_clause(clause, None)?;
        let field = template
            .fields
            .iter()
            .find(|field| &field.declaration == clause.field)?;
        let (_, operand) = construction.fields.iter().find(|(id, _)| id == clause.field)?;
        if field.ty.0 != [TypeExpressionStep::Scalar(unsigned)] || !matches!(operand, TemplateOperand::Zero(ty) if ty == &field.ty) {
            return None;
        }
        clauses.push(clause);
    }
    for (id, operand) in &construction.fields {
        let TemplateOperand::Zero(ty) = operand else { continue };
        let [
            TypeExpressionStep::Named {
                declaration,
                argument_count: 0,
            },
        ] = ty.0.as_slice()
        else {
            continue;
        };
        let field = template.fields.iter().find(|field| &field.declaration == id)?;
        let data = package
            .data
            .iter()
            .find(|data| &data.declaration == declaration)?;
        if &field.ty != ty || !data.cases.is_empty() {
            return None;
        }
        for clause in &data.requires {
            let clause = count_clause(clause, Some(id))?;
            let count = data
                .fields
                .iter()
                .find(|field| &field.declaration == clause.field)?;
            if package.types.get(count.carrier) != Some(&TypeShape::PrimitiveScalar(unsigned)) {
                return None;
            }
            clauses.push(clause);
        }
    }
    Some(clauses)
}
