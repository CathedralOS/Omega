//! Retain content definitions and authored equations as checking questions.
//! Reading never establishes interval separation, conservation or authority.

use declaration_contracts::content as source;
use semantic_vocabulary::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentProjectionExpression, ContentProjectionFormation, ContentProjectionIdentity,
    ContentProjectionScalar, ContentStructuralPlace, ContentTerm, MachineId, Proposition,
};
use terminal_check_tables::{Checked, Construct};
use terminal_psi::StructuralContentProjection;

use super::Program;
use super::contracts::{ClauseScope, ResultTerm, formal_place};
use crate::lowering::body::{DomainInstance, Qualification};
use crate::lowering::{Unserved, refuse};
use crate::source_state::content_projections::{
    ContentProjection, ContentProjectionFormation as SourceFormation,
};
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::type_system::source_projections::SourceTypeCursor;
use crate::source_state::types::TypeReferenceNode;

fn algebra(value: &source::ContentAlgebraIdentity) -> ContentAlgebra {
    match value {
        source::ContentAlgebraIdentity::IntervalSet { coordinate_space } => ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: coordinate_space.clone(),
        },
        source::ContentAlgebraIdentity::CountedQuantity { unit } => ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: unit.clone(),
        },
    }
}

fn scalar(value: &source::ContentScalarExpression) -> ContentProjectionScalar {
    use source::ContentScalarExpression as S;
    match value {
        S::SubjectField(path) => ContentProjectionScalar::SubjectField(
            path.iter().map(|field| field.name.clone()).collect(),
        ),
        S::RuntimeScalarEmbedding(path) => ContentProjectionScalar::RuntimeScalarEmbedding(
            path.iter().map(|field| field.name.clone()).collect(),
        ),
        S::Natural(value) => ContentProjectionScalar::Natural(value.clone()),
        S::Successor(value) => ContentProjectionScalar::Successor(Box::new(scalar(value))),
        S::Arithmetic {
            operator,
            left,
            right,
        } => {
            let left = Box::new(scalar(left));
            let right = Box::new(scalar(right));
            match operator {
                source::ContentArithmeticOperator::Add => ContentProjectionScalar::Add(left, right),
                source::ContentArithmeticOperator::Subtract => {
                    ContentProjectionScalar::Subtract(left, right)
                }
                source::ContentArithmeticOperator::Multiply => {
                    ContentProjectionScalar::Multiply(left, right)
                }
            }
        }
    }
}

fn published_projection(
    domain: ContentDomainId,
    source: &ContentProjection,
) -> StructuralContentProjection {
    let algebra = algebra(&source.algebra);
    let expression = match &source.expression {
        source::ContentProjectionExpression::IntervalSet { members } => {
            ContentProjectionExpression::IntervalSet(
                members
                    .iter()
                    .map(|member| (scalar(member.start()), scalar(member.end())))
                    .collect(),
            )
        }
        source::ContentProjectionExpression::CountedQuantity { magnitude } => {
            ContentProjectionExpression::CountedQuantity(scalar(magnitude))
        }
    };
    let identity = ContentProjectionIdentity {
        domain,
        projection_report_fingerprint:
            declaration_contracts::content::terminal_projection_report_fingerprint(
                &algebra,
                &expression,
            ),
    };
    let formation = source
        .formation
        .iter()
        .map(|question| match question {
            SourceFormation::Natural(value) => ContentProjectionFormation::Natural(scalar(value)),
            SourceFormation::OrderedInterval { start, end } => {
                ContentProjectionFormation::OrderedInterval {
                    start: scalar(start),
                    end: scalar(end),
                }
            }
        })
        .collect();
    StructuralContentProjection {
        identity,
        algebra,
        expression,
        formation,
    }
}

impl Program<'_> {
    pub(super) fn declared_content_projection(
        &self,
        id: MachineId,
        instance: DomainInstance,
    ) -> Checked<Option<StructuralContentProjection>> {
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
        let definitions = self
            .shared
            .content_definitions(self.typed)
            .map_err(unavailable)?;
        let key = |symbol: symbols::SymbolHandle| (symbol.generation(), symbol.arena_index());
        let Ok(position) = definitions
            .binary_search_by_key(&key(instance.declaration), |projection| {
                key(projection.domain)
            })
        else {
            return Ok(None);
        };
        let definition = &definitions[position];
        let actual = self.domain_theory(id, instance)?;
        if self.typed.normalized_type_identity(actual.target_type)
            != self.typed.normalized_type_identity(definition.subject)
        {
            return Err(unavailable(
                "content projection has no exact retained subject application",
            ));
        }
        let domain = ContentDomainId::new(u64::from(instance.identity.0)).ok_or_else(|| {
            unavailable("content projection lost its source application identity")
        })?;
        Ok(Some(published_projection(domain, definition)))
    }

    pub(super) fn content_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Option<Proposition>> {
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
        let typed = self.typed;
        let crate::source_state::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression(expression)
        else {
            return Ok(None);
        };
        if typed
            .symbols
            .fixed_core_declaration_for_symbol(call.target_symbol)
            != Some(symbols::FixedCoreDeclaration::Identity)
        {
            return Ok(None);
        }
        let [argument] = call.machine_arguments.as_ref() else {
            return Ok(None);
        };
        if typed
            .content_algebra_identity(argument.type_reference)
            .map_err(unavailable)?
            .is_none()
        {
            return Ok(None);
        }
        // Other predicates keep their own readers, including the same fixed
        // Identity at a non-content carrier.
        let Some(types) = typed.callable_value_types(scope.owner).map_err(|detail| {
            let mut error = unavailable("content contract lost its callable signature");
            error.detail.push_str(": ");
            error.detail.push_str(&detail);
            error
        })?
        else {
            return Ok(None);
        };
        let Some(clause) = typed
            .content_conservation_clause(scope.owner, types.parameters, expression)
            .map_err(unavailable)?
        else {
            return Ok(None);
        };
        for projection in &clause.projections {
            let definition = typed
                .domain_definition(projection.domain)
                .ok_or_else(|| unavailable("content clause lost its exact qualification owner"))?;
            let carrier = self.value_type(id, projection.subject)?;
            let instance = DomainInstance {
                declaration: projection.domain,
                identity: definition.semantic_id,
            };
            self.declared_domain(
                id,
                instance,
                carrier,
                projection.subject,
                crate::lowering::SourceOrigin::Unpositioned(
                    crate::lowering::Unpositioned::SpanAbsent,
                ),
            )?;
        }
        fn term(
            program: &mut Program<'_>,
            id: MachineId,
            scope: &ClauseScope<'_>,
            types: &crate::source_state::CallableValueTypes<'_>,
            value: &source::ContentConservationTerm,
        ) -> Checked<ContentTerm> {
            let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Contract(detail));
            match value {
                source::ContentConservationTerm::Separate(children) => ContentTerm::separate(
                    children
                        .iter()
                        .map(|child| term(program, id, scope, types, child))
                        .collect::<Checked<Vec<_>>>()?,
                )
                .map_err(|_| {
                    unavailable(
                        "content separation repeats an exact projection or has invalid arity",
                    )
                }),
                source::ContentConservationTerm::Projection {
                    domain,
                    semantic_domain,
                    subject,
                    ..
                } => {
                    let (root, reference) = match &subject.root {
                        source::ContentPlaceRoot::Parameter {
                            position, symbol, ..
                        } => {
                            let position = usize::try_from(*position).map_err(|_| {
                                unavailable("content formal position exceeds the host index")
                            })?;
                            let parameter = types.parameters.get(position).ok_or_else(|| {
                                unavailable("content formal is absent from its actual signature")
                            })?;
                            if parameter.symbol != *symbol
                                || scope
                                    .slots
                                    .get(position)
                                    .is_none_or(|slot| slot.symbol != *symbol || slot.erased)
                            {
                                return Err(unavailable(
                                    "content formal changes its actual owner or relevance",
                                ));
                            }
                            let mut reference = parameter.type_reference;
                            if scope.slots[position].borrowed.is_some() {
                                let TypeReferenceNode::Reference { referee, .. } =
                                    program.typed.type_reference_table.type_reference(reference)
                                else {
                                    return Err(unavailable(
                                        "content borrow lost its declared referent",
                                    ));
                                };
                                reference = *referee;
                            }
                            (formal_place(id, scope.slots, position)?, reference)
                        }
                        source::ContentPlaceRoot::Result => {
                            let ResultTerm::Structural(root, _) = scope.result else {
                                return Err(unavailable(
                                    "content result is not the owning structural result",
                                ));
                            };
                            (root, types.result)
                        }
                    };
                    let mut cursor = SourceTypeCursor::new(reference);
                    let mut path = Vec::with_capacity(subject.segments.len());
                    for segment in &subject.segments {
                        match segment {
                            source::ContentPlaceSegment::Field(field) => {
                                cursor.field(program.typed, field.symbol).ok_or_else(|| unavailable("content field does not belong to its exact applied subject"))?;
                                path.push(ContentPlaceSegment::Field(field.name.clone()));
                            }
                            source::ContentPlaceSegment::Case(case) => {
                                path.push(ContentPlaceSegment::Case(case.name.clone()))
                            }
                            source::ContentPlaceSegment::FixedIndex(index) => {
                                cursor.element(program.typed).ok_or_else(|| {
                                    unavailable("content element has no exact declared collection")
                                })?;
                                path.push(ContentPlaceSegment::FixedIndex(*index));
                            }
                        }
                    }
                    let instance = DomainInstance {
                        declaration: *domain,
                        identity: *semantic_domain,
                    };
                    let actual = program.value_type(id, cursor.current(program.typed))?;
                    let expected = program.domain_theory(id, instance)?.target_type;
                    let expected = program.value_type(id, expected)?;
                    match (
                        actual.qualified(Qualification::NONE),
                        expected.qualified(Qualification::NONE),
                    ) {
                        (Some(actual), Some(expected)) if actual == expected => {}
                        _ => {
                            return Err(unavailable(
                                "content projection subject changes its exact applied carrier",
                            ));
                        }
                    }
                    let projection = program
                        .declared_content_projection(id, instance)?
                        .ok_or_else(|| {
                            unavailable(
                                "content clause has no unique published projection definition",
                            )
                        })?;
                    Ok(ContentTerm::Projection {
                        projection: projection.identity,
                        subject: ContentStructuralPlace {
                            version: match subject.version {
                                source::ContentPlaceVersion::Entry => {
                                    semantic_vocabulary::ContentPlaceVersion::Entry
                                }
                                source::ContentPlaceVersion::Current => {
                                    semantic_vocabulary::ContentPlaceVersion::Current
                                }
                            },
                            root,
                            segments: path,
                        },
                    })
                }
            }
        }
        let left = term(self, id, scope, &types, clause.equation.left())?;
        let right = term(self, id, scope, &types, clause.equation.right())?;
        Ok(Some(Proposition::ContentConservation(
            ContentConservation::new(algebra(&clause.algebra), left, right),
        )))
    }
}
