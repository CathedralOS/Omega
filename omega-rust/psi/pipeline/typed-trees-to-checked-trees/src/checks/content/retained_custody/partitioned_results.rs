//! A bodyless partition promise must account for every returned authority.
//!
//! The borrow-source gate only establishes that owned input is available. It
//! cannot justify turning that input into several claims. Before accepting such
//! a boundary promise, its authored conservation equation must
//! cover the exact owned output places and retain the same qualification and
//! owner projection on consumed input places. Equal algebras alone cannot
//! exchange authority between domains.
//!
//! This is declaration validation, not a call receipt or an issuance rule.
//! Unique identity forwarding and checked-body composition retain their existing
//! checks. Calls still owe live actuals, exact substitution, result correspondence
//! and provider admission; these checks do not publish qualification facts.

use super::structural_sources::{ContentSource, SourceSegment, domain_sources, parameter_sources};
use checked_trees::CheckFacts;
use diagnostics::Diagnostic;
use language_semantics::content::{
    ContentConservationTerm, ContentPlaceRoot, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionPlan, ContentStructuralPlace,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::signature::{SignatureContract, StateParameter};
use typed_trees::types::TypeReferenceHandle;

#[allow(clippy::too_many_arguments)]
pub(crate) fn check_boundary_partition_results(
    program: &TypedTrees,
    facts: &CheckFacts,
    label: &str,
    callable: SymbolHandle,
    parameters: &[StateParameter],
    return_type: TypeReferenceHandle,
    contracts: &[&SignatureContract],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (sources, complete) = domain_sources(program, return_type);
    let outputs: Vec<_> = sources
        .iter()
        .filter(|source| {
            !source.borrowed
                && source.nested
                && facts
                    .qualifications
                    .content
                    .for_semantic_domain(source.domain.semantic_domain)
                    .is_some()
                && program.domain_definitions().iter().any(|domain| {
                    domain.symbol == source.domain.symbol && !domain.establishment_routes.is_empty()
                })
        })
        .collect();
    // Cases are alternatives, not simultaneous supply. Array occurrences count
    // without enumerating the array or assuming a private size limit.
    let partition_outputs: Vec<_> = outputs
        .iter()
        .copied()
        .filter(|source| {
            occurrence_count(source).is_none_or(|count| count > 1)
                || outputs.iter().any(|other| {
                    source.path != other.path
                        && coexist(&source.path, &other.path)
                        && facts
                            .qualifications
                            .content
                            .for_semantic_domain(source.domain.semantic_domain)
                            .zip(
                                facts
                                    .qualifications
                                    .content
                                    .for_semantic_domain(other.domain.semantic_domain),
                            )
                            .is_some_and(|(left, right)| left.algebra == right.algebra)
                })
        })
        .collect();
    if partition_outputs.is_empty() {
        return;
    }
    if partition_outputs.iter().any(|source| {
        outputs
            .iter()
            .any(|other| !coexist(&source.path, &other.path))
    }) {
        diagnostics.push(Diagnostic::error(format!(
            "callable `{label}` has a structural content result requiring outcome-specific partition conservation; checking alternative case frontiers is not implemented",
        )));
        return;
    }
    let inputs: Vec<_> = parameters
        .iter()
        .map(|parameter| parameter_sources(program, parameter, contracts))
        .collect();
    let valid = complete
        && partition_outputs.iter().all(|output| {
            let Some(projection) = facts
                .qualifications
                .content
                .for_semantic_domain(output.domain.semantic_domain)
            else {
                return false;
            };
            facts
                .qualifications
                .content
                .conservation_plans
                .iter()
                .any(|plan| {
                    if plan.callable != callable || plan.algebra != projection.algebra {
                        return false;
                    }
                    [
                        (plan.equation.left(), plan.equation.right()),
                        (plan.equation.right(), plan.equation.left()),
                    ]
                    .into_iter()
                    .any(|(entry, result)| {
                        let mut entry_places = Vec::new();
                        let mut result_places = Vec::new();
                        collect_places(
                            entry,
                            projection,
                            ContentPlaceVersion::Entry,
                            &mut entry_places,
                        ) && collect_places(
                            result,
                            projection,
                            ContentPlaceVersion::Current,
                            &mut result_places,
                        ) && !entry_places.is_empty()
                            && !result_places.is_empty()
                            && distinct_coexisting_places(&entry_places)
                            && distinct_coexisting_places(&result_places)
                            && entry_places
                                .iter()
                                .all(|place| owned_input(parameters, &inputs, projection, place))
                            && result_places.iter().all(|place| {
                                place.root == ContentPlaceRoot::Result
                                    && outputs.iter().any(|declared| {
                                        declared.domain.semantic_domain
                                            == projection.semantic_domain
                                            && path_matches(&declared.path, &place.segments)
                                    })
                            })
                            && outputs
                                .iter()
                                .filter(|declared| {
                                    declared.domain.semantic_domain == projection.semantic_domain
                                })
                                .all(|declared| {
                                    occurrence_count(declared)
                                        == Some(
                                            result_places
                                                .iter()
                                                .filter(|place| {
                                                    path_matches(&declared.path, &place.segments)
                                                })
                                                .count(),
                                        )
                                })
                    })
                })
        });
    if !valid {
        diagnostics.push(Diagnostic::error(format!(
            "callable `{label}` has a structural content result without an exact conservation equation covering its owned qualified output paths from consumed inputs of the same qualification; result annotations and equal content algebras do not establish partitioned authority",
        )));
    }
}

fn occurrence_count(source: &ContentSource) -> Option<usize> {
    source
        .path
        .iter()
        .try_fold(1usize, |count, segment| match segment {
            SourceSegment::Elements(length) => count.checked_mul((*length)?),
            _ => Some(count),
        })
}

fn coexist(left: &[SourceSegment], right: &[SourceSegment]) -> bool {
    for (left, right) in left.iter().zip(right) {
        if left == right {
            continue;
        }
        return !matches!(
            (left, right),
            (SourceSegment::Case(_), SourceSegment::Case(_))
        );
    }
    true
}

fn path_matches(path: &[SourceSegment], concrete: &[ContentPlaceSegment]) -> bool {
    path.len() == concrete.len()
        && path
            .iter()
            .zip(concrete)
            .all(|(declared, actual)| match (declared, actual) {
                (SourceSegment::Field(symbol), ContentPlaceSegment::Field(field)) => {
                    *symbol == field.symbol
                }
                (SourceSegment::Case(symbol), ContentPlaceSegment::Case(case)) => {
                    *symbol == case.symbol
                }
                (
                    SourceSegment::Elements(Some(length)),
                    ContentPlaceSegment::FixedIndex(element),
                ) => usize::try_from(*element).is_ok_and(|element| element < *length),
                _ => false,
            })
}

fn collect_places<'term>(
    term: &'term ContentConservationTerm,
    projection: &ContentProjectionPlan,
    version: ContentPlaceVersion,
    places: &mut Vec<&'term ContentStructuralPlace>,
) -> bool {
    match term {
        ContentConservationTerm::Projection {
            domain,
            semantic_domain,
            projection_machine,
            subject,
            ..
        } => {
            if *domain != projection.domain
                || *semantic_domain != projection.semantic_domain
                || *projection_machine != projection.machine
                || subject.version != version
            {
                return false;
            }
            places.push(subject);
            true
        }
        ContentConservationTerm::Separate(children) => children
            .iter()
            .all(|child| collect_places(child, projection, version, places)),
    }
}

/// A separation frontier cannot count one subject twice or combine mutually
/// exclusive cases of the same sum occurrence. Distinct parameters, fields and
/// array elements remain independent; display names never identify a subject.
fn distinct_coexisting_places(places: &[&ContentStructuralPlace]) -> bool {
    places.iter().enumerate().all(|(position, place)| {
        places[..position].iter().all(|previous| {
            if !same_root(&previous.root, &place.root) || previous.version != place.version {
                return true;
            }
            let first_difference = previous
                .segments
                .iter()
                .zip(&place.segments)
                .find(|(left, right)| !same_segment(left, right));
            match first_difference {
                Some((ContentPlaceSegment::Case(_), ContentPlaceSegment::Case(_))) => false,
                Some(_) => true,
                None => previous.segments.len() != place.segments.len(),
            }
        })
    })
}

fn same_root(left: &ContentPlaceRoot, right: &ContentPlaceRoot) -> bool {
    match (left, right) {
        (ContentPlaceRoot::Result, ContentPlaceRoot::Result) => true,
        (
            ContentPlaceRoot::Parameter {
                position: left_position,
                symbol: left_symbol,
                is_self: left_self,
                ..
            },
            ContentPlaceRoot::Parameter {
                position: right_position,
                symbol: right_symbol,
                is_self: right_self,
                ..
            },
        ) => {
            left_position == right_position
                && left_symbol == right_symbol
                && left_self == right_self
        }
        _ => false,
    }
}

fn same_segment(left: &ContentPlaceSegment, right: &ContentPlaceSegment) -> bool {
    match (left, right) {
        (ContentPlaceSegment::Field(left), ContentPlaceSegment::Field(right)) => {
            left.symbol == right.symbol
        }
        (ContentPlaceSegment::Case(left), ContentPlaceSegment::Case(right)) => {
            left.symbol == right.symbol
        }
        (ContentPlaceSegment::FixedIndex(left), ContentPlaceSegment::FixedIndex(right)) => {
            left == right
        }
        _ => false,
    }
}

fn owned_input(
    parameters: &[StateParameter],
    inputs: &[(Vec<ContentSource>, bool)],
    projection: &ContentProjectionPlan,
    place: &ContentStructuralPlace,
) -> bool {
    let ContentPlaceRoot::Parameter {
        position,
        symbol,
        is_self,
        ..
    } = place.root
    else {
        return false;
    };
    let Ok(position) = usize::try_from(position) else {
        return false;
    };
    let Some(parameter) = parameters.get(position) else {
        return false;
    };
    let Some((sources, complete)) = inputs.get(position) else {
        return false;
    };
    *complete
        && parameter.symbol == symbol
        && parameter.is_self == is_self
        && sources.iter().any(|source| {
            !source.borrowed
                && source.domain.symbol == projection.domain
                && source.domain.semantic_domain == projection.semantic_domain
                && path_matches(&source.path, &place.segments)
        })
}

#[cfg(test)]
mod tests {
    use super::check_boundary_partition_results;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
    use tokens_to_syntax_trees::parse_syntax_trees;

    #[test]
    fn partition_gate_checks_large_array_coverage_symbolically() {
        let source = r#"
            data ByteUnit {}
            data CountedQuantity<Unit> { magnitude: u64; }
            trait Content<A> { machine project(subject: &Self) -> A; }
            data Region [linear] { length: u64; }
            domain Region::Granted established by RootProvider::grant;
            machine Granted::content(region: &Region) -> CountedQuantity<ByteUnit>
            satisfies Content<CountedQuantity<ByteUnit>>::project
            { CountedQuantity { magnitude: region.length } }
            boundary trait RootProvider {
                machine grant(root: Region) -> Region
                ensures result in Region::Granted;
            }
            data Parts { values: [Region in Granted; 1000000000]; }
            boundary trait Partition {
                machine split(whole: Region in Granted) -> Parts;
            }
        "#;
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let program = lower_symbol_resolved_trees(&resolved).expect("type");
        // Isolate this declaration gate from unrelated whole-checker passes:
        // the borrowed-lifetime frontier currently enumerates array elements.
        let mut facts = checked_trees::CheckFacts::default();
        facts.qualifications.content.plans = validation::build_content_projection_plans(&program);
        let owner = program
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Partition")
            .expect("partition boundary");
        let signature = program
            .trait_machine_signatures(owner)
            .first()
            .expect("split requirement");
        let contracts = program
            .state_signature_contracts(signature)
            .iter()
            .collect::<Vec<_>>();
        let mut diagnostics = Vec::new();
        check_boundary_partition_results(
            &program,
            &facts,
            "Partition::split",
            signature.symbol,
            program.state_signature_parameters(signature),
            signature.return_type,
            &contracts,
            &mut diagnostics,
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural content result")),
            "every large-array result still requires conserved authority: {diagnostics:#?}"
        );
    }
}
