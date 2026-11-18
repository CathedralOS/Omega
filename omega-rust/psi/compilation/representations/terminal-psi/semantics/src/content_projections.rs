//! Expand a retained content definition over exact declared fields.
//! These are mathematical terms and formation questions, not content claims
//! or permission to read runtime storage. The consumer resolves each field at
//! its own storage cut and proves every required relation independently.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, ContentPlaceVersion, ContentProjectionScalar,
    ContentStructuralPlace, IntegerMathTerm, IntegerValue, PlaceId, ScalarTerm, ScalarType,
    StructuralTypeId,
};
use terminal_psi::{
    MachineContract, StructuralPathSegment, StructuralTypeDeclaration, TerminalIdentityTable,
};

pub fn field_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    carrier: StructuralTypeId,
    fields: &[String],
) -> Result<(Vec<Segment>, ScalarType), &'static str> {
    if fields.is_empty() {
        return Err("content field projection is empty");
    }
    let path = fields
        .iter()
        .map(|identity| {
            identities
                .index_of(identity)
                .map(StructuralPathSegment::Field)
                .ok_or("content field lost its exact declaration identity")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scalar = crate::primitive_read_projection_type(types.clone(), carrier, &path)
        .ok_or("content field is not a declared primitive leaf of its exact carrier")?;
    let path = crate::canonical_primitive_read_path(types, carrier, &path)
        .ok_or("content field has no exact static storage projection")?;
    Ok((path, scalar))
}

pub fn subject<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    contract: &MachineContract,
    subject: &ContentStructuralPlace,
) -> Result<(Vec<Segment>, &'a StructuralTypeDeclaration), &'static str> {
    let root = contract
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == subject.root)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            contract
                .result
                .structural()
                .filter(|result| result.place == subject.root)
                .map(|result| result.structural_type)
        })
        .ok_or("content subject is not an exact formal or result of this contract")?;
    subject_from_root(types, identities, root, subject)
}

pub fn subject_from_root<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    subject: &ContentStructuralPlace,
) -> Result<(Vec<Segment>, &'a StructuralTypeDeclaration), &'static str> {
    let path = crate::content_transformations::structural_path(
        types.clone(),
        identities,
        root,
        &subject.segments,
    )
    .ok_or("content subject path does not belong to its exact declared carrier")?;
    let (path, carrier) = crate::canonical_static_projection(types.clone(), root, &path)
        .ok_or("content subject has no exact inline structural projection")?;
    // A closed literal index can be formed here. A symbolic length still needs
    // the ordinary bound judgment before this reader can expose that element.
    for (position, step) in path.iter().enumerate() {
        if let Segment::FixedIndex(index) = step {
            let selected =
                crate::canonical_structural_path_tip(types.clone(), root, &path[..position])
                    .ok_or("content index lost its containing storage")?;
            let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = selected.shape
            else {
                return Err("content index has no declared fixed array");
            };
            let Some(length) = length.literal() else {
                return Err("content index needs a checked symbolic bound");
            };
            if *index >= length {
                return Err("content index is outside its declared array");
            }
        }
    }
    Ok((path, carrier))
}

pub fn scalar<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    identities: &TerminalIdentityTable,
    carrier: StructuralTypeId,
    root: PlaceId,
    prefix: &[Segment],
    version: ContentPlaceVersion,
    expression: &ContentProjectionScalar,
) -> Result<IntegerMathTerm, &'static str> {
    use ContentProjectionScalar as S;
    let mut pending = vec![(expression, false)];
    let mut values = Vec::new();
    while let Some((expression, finish)) = pending.pop() {
        if finish {
            let right = values
                .pop()
                .ok_or("content arithmetic lost its endpoint operand")?;
            let value = match expression {
                S::Successor(_) => IntegerMathTerm::Add(
                    Box::new(right),
                    Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
                ),
                S::Add(..) | S::Subtract(..) | S::Multiply(..) => {
                    let left = values
                        .pop()
                        .ok_or("content arithmetic lost its endpoint operand")?;
                    match expression {
                        S::Add(..) => IntegerMathTerm::Add(Box::new(left), Box::new(right)),
                        S::Subtract(..) => {
                            IntegerMathTerm::Subtract(Box::new(left), Box::new(right))
                        }
                        _ => IntegerMathTerm::Multiply(Box::new(left), Box::new(right)),
                    }
                }
                _ => return Err("content arithmetic completion has no operation"),
            };
            values.push(value);
            continue;
        }
        match expression {
            S::Natural(value) => {
                if value.is_empty()
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                    || (value != "0" && value.starts_with('0'))
                {
                    return Err("content natural literal is not canonical");
                }
                let value = value.parse::<u128>().map_err(
                    |_| "content natural literal exceeds the available integer term encoding",
                )?;
                values.push(IntegerMathTerm::literal(IntegerValue::Unsigned(value)));
            }
            S::SubjectField(fields) | S::RuntimeScalarEmbedding(fields) => {
                let (path, ty) = field_path(types.clone(), identities, carrier, fields)?;
                let ScalarType::Integer(scalar_type) = ty else {
                    return Err("content arithmetic reads a noninteger field");
                };
                let mut full = prefix.to_vec();
                full.extend(path);
                let field = ScalarTerm::IntegerField {
                    version,
                    root,
                    path: full,
                    scalar_type,
                };
                values.push(
                    IntegerMathTerm::embed(field)
                        .ok_or("content field has no exact integer embedding")?,
                );
            }
            S::Successor(value) => {
                pending.push((expression, true));
                pending.push((value, false));
            }
            S::Add(left, right) | S::Subtract(left, right) | S::Multiply(left, right) => {
                pending.push((expression, true));
                pending.push((right, false));
                pending.push((left, false));
            }
        }
    }
    let result = values.pop().ok_or("content expression has no endpoint")?;
    if !values.is_empty() {
        return Err("content expression has extra endpoint values");
    }
    Ok(result)
}

/// One definition occurrence after its full domain identity and carrier have
/// been rejoined. No endpoint value or conservation evidence is retained here.
pub struct ResolvedProjection<'a> {
    pub subject: &'a ContentStructuralPlace,
    pub definition: &'a terminal_psi::StructuralContentProjection,
    pub carrier: StructuralTypeId,
    pub prefix: Vec<Segment>,
}

pub fn resolve<'a>(
    types: &'a [StructuralTypeDeclaration],
    domains: &'a [terminal_psi::StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    roots: impl Iterator<Item = (PlaceId, StructuralTypeId)> + Clone,
    equation: &'a semantic_vocabulary::ContentConservation,
) -> Result<Vec<ResolvedProjection<'a>>, &'static str> {
    use semantic_vocabulary::ContentTerm;
    let mut pending = vec![equation.left(), equation.right()];
    let mut resolved = Vec::new();
    while let Some(term) = pending.pop() {
        match term {
            ContentTerm::Separate(parts) => pending.extend(parts.iter()),
            ContentTerm::Projection {
                projection,
                subject,
            } => {
                let mut candidates = roots.clone().filter(|(root, _)| root == &subject.root);
                let (_, root) = candidates
                    .next()
                    .ok_or("content subject has no contract root")?;
                if candidates.next().is_some() {
                    return Err("content subject has competing contract roots");
                }
                resolved.push(resolve_projection(
                    types,
                    domains,
                    identities,
                    root,
                    equation.algebra(),
                    *projection,
                    subject,
                )?);
            }
        }
    }
    Ok(resolved)
}

/// Resolve one projected subject against its exact declared carrier and algebra.
pub fn resolve_projection<'a>(
    types: &'a [StructuralTypeDeclaration],
    domains: &'a [terminal_psi::StructuralDomainDeclaration],
    identities: &TerminalIdentityTable,
    root: StructuralTypeId,
    algebra: &semantic_vocabulary::ContentAlgebra,
    projection: semantic_vocabulary::ContentProjectionIdentity,
    subject: &'a ContentStructuralPlace,
) -> Result<ResolvedProjection<'a>, &'static str> {
    let mut domains = domains
        .iter()
        .filter(|domain| domain.semantic_domain.get() == projection.domain.get());
    let domain = domains
        .next()
        .ok_or("content projection has no declared domain")?;
    if domains.next().is_some() {
        return Err("content projection domain is ambiguous");
    }
    let definition = domain
        .content_projection
        .as_ref()
        .ok_or("content domain has no retained projection definition")?;
    if definition.identity != projection || &definition.algebra != algebra {
        return Err("content projection changes its exact definition or algebra");
    }
    let (prefix, carrier) = subject_from_root(types.iter(), identities, root, subject)?;
    if carrier.id != domain.carrier {
        return Err("content projection subject differs from its declared carrier");
    }
    Ok(ResolvedProjection {
        subject,
        definition,
        carrier: carrier.id,
        prefix,
    })
}

/// Resolve endpoint arithmetic through the same versioned field binding used
/// by ordinary scalar contracts. Partial conversions remain separate goals.
pub fn bind<Error: From<&'static str>>(
    types: &[StructuralTypeDeclaration],
    identities: &TerminalIdentityTable,
    occurrence: &ResolvedProjection<'_>,
    bindings: &mut dyn crate::substitution::clauses::ClauseBindings<Error = Error>,
) -> Result<
    (
        semantic_vocabulary::ContentIntervalInterpretation,
        Vec<semantic_vocabulary::Proposition>,
    ),
    Error,
> {
    use semantic_vocabulary::{
        ContentProjectionExpression, ContentProjectionFormation, Proposition,
    };
    for (depth, segment) in occurrence.prefix.iter().enumerate() {
        if let Segment::Case(case) = segment {
            if !bindings.case(occurrence.subject.root, &occurrence.prefix[..depth], *case)? {
                return Err("content projection requires an established active case".into());
            }
        }
    }
    let expand =
        |expression: &ContentProjectionScalar,
         bindings: &mut dyn crate::substitution::clauses::ClauseBindings<Error = Error>| {
            let term = scalar(
                types.iter(),
                identities,
                occurrence.carrier,
                occurrence.subject.root,
                &occurrence.prefix,
                occurrence.subject.version,
                expression,
            )
            .map_err(Error::from)?;
            crate::substitution::clauses::substitute_clause_math_term(&term, bindings)
        };
    let ContentProjectionExpression::IntervalSet(intervals) = &occurrence.definition.expression
    else {
        return Err("content checking requires an interval-set projection".into());
    };
    let members = intervals
        .iter()
        .map(|(start, end)| Ok((expand(start, bindings)?, expand(end, bindings)?)))
        .collect::<Result<Vec<_>, Error>>()?;
    let mut formation = Vec::new();
    for question in &occurrence.definition.formation {
        formation.push(match question {
            ContentProjectionFormation::Natural(expression) => Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                expand(expression, bindings)?,
            ),
            ContentProjectionFormation::OrderedInterval { start, end } => {
                Proposition::IntegerMathLessOrEqual(
                    expand(start, bindings)?,
                    expand(end, bindings)?,
                )
            }
        });
    }
    Ok((
        semantic_vocabulary::ContentIntervalInterpretation {
            projection: occurrence.definition.identity,
            subject: occurrence.subject.clone(),
            algebra: occurrence.definition.algebra.clone(),
            members,
        },
        formation,
    ))
}

/// Definedness of each interval-set operand precedes its equality. In
/// particular, a partial separated union cannot justify its own disjointness.
pub fn separation_requirements(
    equation: &semantic_vocabulary::ContentConservation,
    interpretations: &[semantic_vocabulary::ContentIntervalInterpretation],
) -> Result<Vec<semantic_vocabulary::Proposition>, &'static str> {
    use semantic_vocabulary::{ContentTerm, Proposition};
    let mut requirements = Vec::new();
    for side in [equation.left(), equation.right()] {
        let mut pending = vec![side];
        let mut members = Vec::new();
        while let Some(term) = pending.pop() {
            match term {
                ContentTerm::Separate(parts) => pending.extend(parts),
                ContentTerm::Projection {
                    projection,
                    subject,
                } => {
                    let interpretation = interpretations
                        .iter()
                        .find(|meaning| {
                            meaning.projection == *projection && &meaning.subject == subject
                        })
                        .ok_or("content formation lost an exact projection interpretation")?;
                    members.extend(interpretation.members.iter());
                }
            }
        }
        for (index, (start, end)) in members.iter().enumerate() {
            requirements.push(Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Unsigned(0)),
                start.clone(),
            ));
            requirements.push(Proposition::IntegerMathLessOrEqual(
                start.clone(),
                end.clone(),
            ));
            for (other_start, other_end) in &members[..index] {
                requirements.push(Proposition::Disjunction(vec![
                    Proposition::IntegerMathLessOrEqual(end.clone(), start.clone()),
                    Proposition::IntegerMathLessOrEqual(other_end.clone(), other_start.clone()),
                    Proposition::IntegerMathLessOrEqual(end.clone(), other_start.clone()),
                    Proposition::IntegerMathLessOrEqual(other_end.clone(), start.clone()),
                ]));
            }
        }
    }
    Ok(requirements)
}
