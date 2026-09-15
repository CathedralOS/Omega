//! Distinguish owned storage from contents stable under shared observation.
//!
//! Both questions need the same closed generic substitution and recursive-data
//! checks. A shared loan can freeze ordinary contents without owning them; that
//! never authorizes no-code moves, cleanup erasure, or a copied reference ABI.
//! Observation permits only recursively stable contents and understood value
//! refinements. Unknown qualifications and mutation-capable loans stay opaque.

use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

/// Temporary closed type arguments keep nested instantiations in their lexical
/// environment. Reusing a declaration's parameter symbol must not capture an
/// outer instantiation's argument or turn a stored reference into owned data.
#[derive(Clone, PartialEq, Eq)]
enum ContentType {
    Scalar(PrimitiveType),
    Array(Box<ContentType>, usize),
    Slice(Box<ContentType>),
    Shared(Box<ContentType>),
    Data(SymbolHandle, Vec<ContentType>),
}

impl ContentType {
    fn size(&self) -> usize {
        match self {
            Self::Scalar(_) => 1,
            Self::Array(element, _) => 1 + element.size(),
            Self::Slice(element) | Self::Shared(element) => 1 + element.size(),
            Self::Data(_, arguments) => 1 + arguments.iter().map(Self::size).sum::<usize>(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContentsRequirement {
    PlainOwned,
    NumericOwned,
    StableObservation,
}

pub fn has_plain_owned_contents(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    has_plain_owned_contents_with_substitutions(program, reference, &[])
}

pub fn has_plain_owned_contents_with_substitutions(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> bool {
    check_contents_requirement(
        program,
        reference,
        substitutions,
        ContentsRequirement::PlainOwned,
    )
}

/// Classify no-code owned storage while allowing primitive numeric restrictions.
/// Callers still owe exact source type and value-constraint correspondence. This
/// does not extend structural-return or construction routes that erase them.
pub fn has_plain_owned_contents_with_numeric_constraints(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    check_contents_requirement(program, reference, &[], ContentsRequirement::NumericOwned)
}

/// Classify value contents that an immutable binding can retain as an entry
/// observation while its loans remain valid. This proves neither binding
/// immutability nor loan validity; those remain the caller's obligations.
/// Shared contents are not necessarily owned, and synchronized/interior mutation
/// or unclassified qualifications cannot supply a stable observation.
pub fn has_stable_observable_contents(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    check_contents_requirement(
        program,
        reference,
        &[],
        ContentsRequirement::StableObservation,
    )
}

fn check_contents_requirement(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    requirement: ContentsRequirement,
) -> bool {
    let mut arguments = Vec::new();
    for (symbol, argument) in substitutions {
        let Some(argument) = resolve(program, *argument, &arguments, requirement) else {
            return false;
        };
        arguments.push((*symbol, argument));
    }
    resolve(program, reference, &arguments, requirement).is_some_and(|resolved| {
        check_contents(
            program,
            &resolved,
            &mut Vec::new(),
            &mut Vec::new(),
            requirement,
        )
    })
}

fn resolve(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    arguments: &[(SymbolHandle, ContentType)],
    requirement: ContentsRequirement,
) -> Option<ContentType> {
    if !reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some((_, argument)) = arguments
                .iter()
                .rev()
                .find(|(parameter, _)| parameter == symbol)
            {
                return Some(argument.clone());
            }
            if let Some(primitive) = program.primitive_type_reference(reference) {
                return Some(ContentType::Scalar(primitive));
            }
            let data = definition(program, *symbol)?;
            program
                .data_type_parameters(data)
                .is_empty()
                .then_some(ContentType::Data(*symbol, Vec::new()))
        }
        TypeReferenceNode::Generic {
            base_symbol,
            arguments: source_arguments,
            ..
        } => {
            let data = definition(program, *base_symbol)?;
            let parameters = program.data_type_parameters(data);
            let source_arguments = program
                .type_reference_table
                .type_reference_handles(*source_arguments);
            if parameters.len() != source_arguments.len()
                || parameters.iter().any(|parameter| {
                    !matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                })
            {
                return None;
            }
            let resolved = source_arguments
                .iter()
                .map(|reference| resolve(program, *reference, arguments, requirement))
                .collect::<Option<Vec<_>>>()?;
            Some(ContentType::Data(*base_symbol, resolved))
        }
        // Extent zero does not introduce loans or cleanup. Retain and classify
        // the element carrier just as for nonempty arrays; operation-specific
        // layout and indexing requirements are checked by their consumers.
        TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } => Some(ContentType::Array(
            Box::new(resolve(program, *element_type, arguments, requirement)?),
            *length,
        )),
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            referee,
            ..
        } if requirement == ContentsRequirement::StableObservation => Some(ContentType::Shared(
            Box::new(resolve(program, *referee, arguments, requirement)?),
        )),
        TypeReferenceNode::Slice { element_type }
            if requirement == ContentsRequirement::StableObservation =>
        {
            Some(ContentType::Slice(Box::new(resolve(
                program,
                *element_type,
                arguments,
                requirement,
            )?)))
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let rows = program.type_reference_table.constraints(*constraints);
            if requirement == ContentsRequirement::PlainOwned
                || rows.len() != constraints.count() as usize
                || rows.is_empty()
            {
                return None;
            }
            let contents = resolve(program, *base_type, arguments, requirement)?;
            let understood = rows.iter().all(|constraint| match constraint {
                typed_trees::types::TypeConstraintNode::Range { .. }
                | typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => {
                    matches!(contents, ContentType::Scalar(_))
                }
                typed_trees::types::TypeConstraintNode::Domain(domain) => {
                    requirement == ContentsRequirement::StableObservation
                        && is_plain_value_domain(program, domain)
                }
                typed_trees::types::TypeConstraintNode::Named(_) => false,
            });
            understood.then_some(contents)
        }
        // These require retained loans, qualifications, or a different carrier.
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => None,
    }
}

fn is_plain_value_domain(
    program: &TypedTrees,
    domain: &typed_trees::types::DomainConstraint,
) -> bool {
    // Pure value refinements do not add storage or mutation authority. Their
    // predicates still need ordinary formation/proof; stability does not prove
    // membership. Inspect normalized identities, never a spelling such as Utf8.
    // Routed, semantic-role-bearing and alias theories remain conservative until
    // their independent obligations can be transported by the entry-value path.
    if domain.subject != typed_trees::types::DomainConstraintSubject::Declared
        || !domain.symbol.is_valid()
        || domain.classification.is_some()
        || !domain.semantic_roles.is_empty()
        || !domain.establishment_routes.is_empty()
    {
        return false;
    }
    program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain.symbol)
        .is_some_and(|definition| {
            definition.alias.is_none()
                && definition.classification.is_none()
                && definition.semantic_roles.is_empty()
                && definition.establishment_routes.is_empty()
        })
}

fn definition(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    if !symbol.is_valid() {
        return None;
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == symbol);
    let data = definitions.next()?;
    definitions.next().is_none().then_some(data)
}

fn check_contents(
    program: &TypedTrees,
    resolved: &ContentType,
    active: &mut Vec<ContentType>,
    complete: &mut Vec<ContentType>,
    requirement: ContentsRequirement,
) -> bool {
    if complete.contains(resolved) {
        return true;
    }
    let ContentType::Data(symbol, arguments) = resolved else {
        return match resolved {
            ContentType::Scalar(_) => true,
            ContentType::Array(element, _)
            | ContentType::Slice(element)
            | ContentType::Shared(element) => {
                check_contents(program, element, active, complete, requirement)
            }
            ContentType::Data(..) => unreachable!(),
        };
    };
    let Some(data) = definition(program, *symbol) else {
        return false;
    };
    // A repeated declaration must consume finite type-argument structure, as
    // in Wrapper<Wrapper<Value>>. Expanding recursive by-value data is not a
    // finite owned carrier and must not make this classifier recurse forever.
    if data.properties.multiplicity == Multiplicity::Linear
        || active.iter().any(|ancestor| {
            matches!(ancestor, ContentType::Data(owner, _)
            if owner == symbol && resolved.size() >= ancestor.size())
        })
        || program.machines().iter().any(|machine| {
            machine.attached_data_symbol == *symbol && machine.name.as_str().ends_with("::drop")
        })
    {
        return false;
    }
    let parameters = program.data_type_parameters(data);
    if parameters.len() != arguments.len() {
        return false;
    }
    let substitutions = parameters
        .iter()
        .zip(arguments)
        .map(|(parameter, argument)| (parameter.symbol, argument.clone()))
        .collect::<Vec<_>>();
    active.push(resolved.clone());
    let mut check_field = |field: &typed_trees::data::DataField| {
        !field.relevance.is_erased()
            && resolve(program, field.type_reference, &substitutions, requirement)
                .is_some_and(|field| check_contents(program, &field, active, complete, requirement))
    };
    let supported = program
        .data_members(data)
        .iter()
        .all(|member| match member {
            DataMember::Field(field) => check_field(field),
            DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .all(&mut check_field),
        });
    active.pop();
    if supported {
        complete.push(resolved.clone());
    }
    supported
}
