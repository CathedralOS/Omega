//! Establish the fields of a lifetime-parametric record constructor. An omitted
//! aggregate recursively owes the predicates of every active zero value.
use crate::FieldOrigin;
use omega_psi::{
    ContractClause, DeclarationId, DeclarationKind, LoweredPackage, MachineTemplate, MachineTemplateBinder, Multiplicity, PredicateOperator, PredicateStep,
    ScalarType, TemplateAccess, TemplateBinder, TemplateConstruct, TypeExpression, TypeExpressionStep as Step, TypeId, TypeShape,
};
use std::collections::BTreeSet;

type Check<T> = Result<T, &'static str>;
type Fields<'a> = Vec<(&'a DeclarationId, TypeExpression)>;

struct Record<'a> {
    declaration: &'a DeclarationId,
    fields: Fields<'a>,
    clauses: &'a [ContractClause],
    multiplicity: Multiplicity,
}

#[derive(Clone, Copy)]
enum Selection<'a> {
    Record,
    Zero,
    Case(&'a DeclarationId),
    /// Moving an established input preserves its active case.
    WholeInput,
}

/// For a record every field is active. A sum zero owns only its first case,
/// including the fields of that case in declaration order.
fn active_fields<'a, F, C>(
    declaration: &DeclarationId,
    fields: &'a [F],
    cases: &'a [C],
    field_id: impl Fn(&'a F) -> &'a DeclarationId,
    case_parts: impl Fn(&'a C) -> (&'a DeclarationId, &'a [F]),
    selection: Selection<'_>,
) -> Check<&'a [F]> {
    if !fields.is_empty() && !cases.is_empty() {
        return Err("data has both fields and cases");
    }
    verify_members(declaration, DeclarationKind::Field, fields.iter().map(&field_id))?;
    verify_members(declaration, DeclarationKind::Case, cases.iter().map(|case| case_parts(case).0))?;
    for (case, payload) in cases.iter().map(&case_parts) {
        verify_members(case, DeclarationKind::CaseField, payload.iter().map(&field_id))?;
    }
    if matches!(selection, Selection::WholeInput) {
        return Ok(&[]);
    }
    if let Selection::Case(expected) = selection {
        return cases
            .iter()
            .map(&case_parts)
            .find(|(id, _)| *id == expected)
            .map(|(_, fields)| fields)
            .ok_or("case is absent from its constructed type");
    }
    match cases.first() {
        None => Ok(fields),
        Some(case) if matches!(selection, Selection::Zero) => Ok(case_parts(case).1),
        _ => Err("expected a record or the active case of a sum zero"),
    }
}

fn verify_members<'a>(
    owner: &DeclarationId,
    kind: DeclarationKind,
    members: impl Iterator<Item = &'a DeclarationId>,
) -> Check<()> {
    let mut names = BTreeSet::new();
    for id in members {
        let Some((parent, name)) = id.path.rsplit_once("::") else {
            return Err("data member has no enclosing name");
        };
        if id.kind != kind
            || id.package != owner.package
            || id.enclosing_machine != owner.enclosing_machine
            || parent != owner.path
            || name.is_empty()
            || !names.insert(name)
        {
            return Err("constructor data has ambiguous or foreign member identities");
        }
    }
    Ok(())
}

fn concrete_type(
    unit: &LoweredPackage,
    mut carrier: TypeId,
) -> Check<TypeExpression> {
    let mut arrays = Vec::new();
    let mut steps = loop {
        match unit.types.get(carrier) {
            Some(TypeShape::FixedArray { element, length }) if element.get() < carrier.get() => {
                arrays.push(*length);
                carrier = *element;
            }
            Some(TypeShape::PrimitiveScalar(scalar)) => break vec![Step::Scalar(*scalar)],
            Some(TypeShape::SharedByteView) => {
                break vec![
                    Step::Scalar(ScalarType::Integer { signed: false, width: 8 }),
                    Step::Slice,
                    Step::Reference(TemplateAccess::Shared),
                ];
            }
            Some(TypeShape::Nominal(id)) => {
                let data = unit
                    .data
                    .get(id.get() - 1)
                    .filter(|data| data.id == *id && data.carrier == carrier)
                    .ok_or("field type has no matching concrete data declaration")?;
                if data.lifetime_count != 0 || !data.type_arguments.is_empty() || !data.value_indices.is_empty() {
                    return Err("concrete field type requires its application");
                }
                break vec![Step::Named {
                    declaration: data.declaration.clone(),
                    argument_count: 0,
                }];
            }
            _ => return Err("unsupported field carrier in constructor data"),
        }
    };
    steps.extend(arrays.into_iter().rev().map(Step::Array));
    Ok(TypeExpression(steps))
}

fn record<'a>(
    unit: &'a LoweredPackage,
    ty: &[Step],
    selection: Selection<'_>,
) -> Check<Record<'a>> {
    let Some((Step::Named { declaration, argument_count }, arguments)) = ty.split_last() else {
        return Err("constructor field needs its nominal type");
    };
    if declaration.kind != DeclarationKind::Data
        || *argument_count != arguments.len()
        || !arguments
            .iter()
            .all(|arg| matches!(arg, Step::GenericParameter { .. }))
    {
        return Err("unsupported nominal application in constructor data");
    }
    if unit
        .aliases
        .iter()
        .any(|alias| &alias.declaration == declaration)
    {
        return Err("constructor data declaration also identifies an alias");
    }
    let mut templates = unit
        .data_templates
        .iter()
        .filter(|item| &item.declaration == declaration);
    let mut concrete = unit.data.iter().filter(|item| &item.declaration == declaration);
    let result = match (templates.next(), concrete.next()) {
        (Some(data), None) if templates.next().is_none() => {
            if data.parameters.len() != *argument_count
                || !data
                    .parameters
                    .iter()
                    .all(|p| matches!(p, TemplateBinder::Lifetime))
                || !data.formation.is_empty()
                || !data.equations.is_empty()
                || data.witness_count != 0
                || !data.enclosing_parameters.is_empty()
            {
                return Err("constructor data requires unsupported formation evidence");
            }
            let selected = active_fields(
                declaration,
                &data.fields,
                &data.cases,
                |field| &field.declaration,
                |case| (&case.declaration, &case.fields),
                selection,
            )?;
            let fields = selected
                .iter()
                .map(|field| {
                    let steps = field
                        .ty
                        .0
                        .iter()
                        .map(|step| match step {
                            Step::GenericParameter { owner, index } if owner == declaration => arguments
                                .get(*index)
                                .cloned()
                                .ok_or("field refers to a missing data binder"),
                            Step::GenericParameter { .. } => Err("field refers to a foreign data binder"),
                            other => Ok(other.clone()),
                        })
                        .collect::<Check<Vec<_>>>()?;
                    Ok((&field.declaration, TypeExpression(steps)))
                })
                .collect::<Check<_>>()?;
            Record {
                declaration: &data.declaration,
                fields,
                clauses: &data.requires,
                multiplicity: data.multiplicity,
            }
        }
        (None, Some(data)) if concrete.next().is_none() => {
            if unit
                .data
                .get(data.id.get() - 1)
                .is_none_or(|row| row.declaration != data.declaration)
                || unit.types.get(data.carrier) != Some(&TypeShape::Nominal(data.id))
                || data.lifetime_count != *argument_count
                || !data.type_arguments.is_empty()
                || !data.value_indices.is_empty()
                || data.cleanup_target.is_some()
            {
                return Err("unsupported concrete schema for constructor field");
            }
            let selected = active_fields(
                declaration,
                &data.fields,
                &data.cases,
                |field| &field.declaration,
                |case| (&case.declaration, &case.fields),
                selection,
            )?;
            let fields = selected
                .iter()
                .map(|field| {
                    let mut ty = concrete_type(unit, field.carrier)?;
                    if let Some(region) = field.lifetime {
                        if !matches!(ty.0.last(), Some(Step::Reference(_))) {
                            return Err("a stored region must bind a reference field");
                        }
                        ty.0.push(
                            arguments
                                .get(region)
                                .cloned()
                                .ok_or("field refers to a missing data region")?,
                        );
                        ty.0.push(Step::AtLifetime);
                    }
                    Ok((&field.declaration, ty))
                })
                .collect::<Check<_>>()?;
            Record {
                declaration: &data.declaration,
                fields,
                clauses: &data.requires,
                multiplicity: data.multiplicity,
            }
        }
        _ => return Err("constructor requires one matching data declaration"),
    };
    Ok(result)
}

/// A caller constructing one sum case must supply only that case's payload.
/// Its fields use the lifetime arguments of the constructed type.
pub(super) fn case_fields<'a>(
    unit: &'a LoweredPackage,
    ty: &TypeExpression,
    case: &DeclarationId,
) -> Check<Fields<'a>> {
    let record = record(unit, &ty.0, Selection::Case(case))?;
    if !record.clauses.is_empty() {
        return Err("case construction requires unproved data predicates");
    }
    Ok(record.fields)
}

pub(super) fn check_element_type(
    unit: &LoweredPackage,
    element: &TypeExpression,
) -> Check<()> {
    record(unit, &element.0, Selection::WholeInput).map(|_| ())
}

fn check_clauses(
    record: &Record<'_>,
    supplied_zero: impl Fn(&DeclarationId) -> bool,
) -> Check<()> {
    for clause in record.clauses {
        if clause.predicate == [PredicateStep::Boolean(true)] {
            continue;
        }
        let [
            PredicateStep::SelfValue,
            PredicateStep::Field(field),
            PredicateStep::Integer(limit),
            PredicateStep::Apply(op),
        ] = clause.predicate.as_slice()
        else {
            return Err("unsupported predicate on constructed data");
        };
        let (_, ty) = record
            .fields
            .iter()
            .find(|(id, _)| *id == field)
            .ok_or("predicate selects no active field")?;
        if ty.0 != [Step::Scalar(ScalarType::Integer { signed: false, width: 64 })] || !supplied_zero(field) {
            return Err("constructor predicate needs a known u64 field value");
        }
        let n = u64::try_from(limit).map_err(|_| "predicate literal lies outside its field carrier")?;
        let proved = match op {
            PredicateOperator::LessOrEqual => true,
            PredicateOperator::LessThan | PredicateOperator::NotEqual => n != 0,
            PredicateOperator::Equal | PredicateOperator::GreaterOrEqual => n == 0,
            PredicateOperator::GreaterThan => false,
            _ => return Err("unsupported constructor field predicate operator"),
        };
        if !proved {
            return Err("zero fails the active data predicate");
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Use {
    Copy,
    Zero,
}

fn check_value(
    unit: &LoweredPackage,
    ty: &TypeExpression,
    usage: Use,
) -> Check<()> {
    enum Visit<'a> {
        Enter(TypeExpression),
        Leave(&'a DeclarationId),
    }
    let mut work = vec![Visit::Enter(ty.clone())];
    let mut active = BTreeSet::new();
    let mut checked = BTreeSet::new();
    while let Some(visit) = work.pop() {
        let mut ty = match visit {
            Visit::Enter(ty) => ty,
            Visit::Leave(id) => {
                active.remove(id);
                checked.insert(id);
                continue;
            }
        };
        match ty.0.as_slice() {
            [Step::Scalar(ScalarType::Boolean | ScalarType::Integer { width: 1.., .. })] => {}
            [.., Step::Array(_)] => {
                ty.0.pop();
                work.push(Visit::Enter(ty));
            }
            // Shared byte descriptors carry existing readable storage. No
            // amount of zero-initialization can create that authority.
            [
                Step::Scalar(ScalarType::Integer { signed: false, width: 8 }),
                Step::Slice,
                Step::Reference(TemplateAccess::Shared),
                Step::GenericParameter { .. },
                Step::AtLifetime,
            ] if usage == Use::Copy => {}
            [.., Step::Named { .. }] => {
                let selection = if usage == Use::Zero { Selection::Zero } else { Selection::Record };
                let record = record(unit, &ty.0, selection)?;
                // Only lifetimes vary between these applications. Each schema
                // has the same zero obligations and copyable fields for all of them.
                if checked.contains(record.declaration) {
                    continue;
                }
                if !active.insert(record.declaration) {
                    return Err("constructor has recursive active storage");
                }
                match usage {
                    Use::Zero if record.multiplicity != Multiplicity::Linear => check_clauses(&record, |_| true)?,
                    Use::Copy if record.multiplicity == Multiplicity::Copy => {}
                    _ => return Err("field construction lacks the required custody"),
                }
                work.push(Visit::Leave(record.declaration));
                work.extend(record.fields.into_iter().map(|(_, field)| Visit::Enter(field)));
            }
            _ => return Err("field cannot be copied or zeroed by this constructor"),
        }
    }
    Ok(())
}

pub(crate) fn check_fields(
    unit: &LoweredPackage,
    constructor: &MachineTemplate,
    construct: &TemplateConstruct,
    fields: &[(&DeclarationId, FieldOrigin<'_>)],
) -> Check<()> {
    let schema = record(unit, &construct.ty.0, Selection::Record)?;
    if schema.declaration != &construct.declaration
        || fields.len() != schema.fields.len()
        || schema.multiplicity == Multiplicity::Linear
        || construct.establishes != !schema.clauses.is_empty()
    {
        return Err("constructor does not establish its complete record declaration");
    }
    for step in &construct.ty.0 {
        if let Step::GenericParameter { owner, index } = step
            && (owner != &constructor.declaration || !matches!(constructor.parameters.get(*index), Some(MachineTemplateBinder::Type(TemplateBinder::Lifetime))))
        {
            return Err("constructor result uses a foreign lifetime");
        }
    }
    for (id, expected) in &schema.fields {
        let (_, origin) = fields
            .iter()
            .find(|(field, _)| field == id)
            .ok_or("constructor omitted a field")?;
        match origin {
            FieldOrigin::CopySharedFormal { ty, .. } if *ty == expected => check_value(unit, ty, Use::Copy)?,
            FieldOrigin::MoveFormal { ty, .. } if *ty == expected => {
                let source = record(unit, &ty.0, Selection::WholeInput)?;
                if source.multiplicity == Multiplicity::Linear {
                    return Err("moving a linear input needs claim transfer evidence");
                }
                if schema.multiplicity == Multiplicity::Copy {
                    check_value(unit, ty, Use::Copy)?;
                }
            }
            FieldOrigin::Zero(ty) if *ty == expected => {
                check_value(unit, ty, Use::Zero)?;
                if schema.multiplicity == Multiplicity::Copy {
                    check_value(unit, ty, Use::Copy)?;
                }
            }
            _ => return Err("constructor field origin has a different carrier"),
        }
    }
    check_clauses(&schema, |field| {
        fields
            .iter()
            .any(|&(id, origin)| id == field && matches!(origin, FieldOrigin::Zero(_)))
    })
}

/// Called after field establishment. Follow the declared fields of a zeroed
/// record to the exact numeric subject of a result guarantee.
pub(crate) fn zero_field_integer(
    unit: &LoweredPackage,
    ty: &TypeExpression,
    path: &[&DeclarationId],
) -> Check<()> {
    let mut ty = ty.clone();
    for field in path {
        let schema = record(unit, &ty.0, Selection::Record)?;
        ty = schema
            .fields
            .into_iter()
            .find(|(id, _)| id == field)
            .ok_or("result guarantee selects no such field")?
            .1;
    }
    if matches!(ty.0.as_slice(), [Step::Scalar(ScalarType::Integer { width: 1.., .. })]) {
        Ok(())
    } else {
        Err("result guarantee needs an integer zero")
    }
}
