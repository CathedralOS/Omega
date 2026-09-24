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
use typed_trees::data::{DataField, DataMember};
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
    /// The same no-code owned storage as `NumericOwned`, except a data
    /// declaration marked `[linear]` remains a valid carrier: linear custody
    /// keeps its own claim accounting, so a selection join only needs to know
    /// the value holds no loans, nominal cleanup, or recursive storage. Nested
    /// linear children are equally plain linear carriers, never a second kind
    /// of obligation the join would have to name separately.
    LinearOwned,
    /// Owned storage a consuming machine may carry through a `drop(value)`
    /// call: like `NumericOwned` it forbids loans, recursive carriers, and
    /// unclassified storage, but the declaration may own an attached `::drop`
    /// hook — the caller's transfer edge, not the classifier, proves the hook
    /// is invoked — and `[erased]` members are admitted because they never
    /// produce runtime contents. An erased member is still resolved so its
    /// declared type exists; its contents never materialize.
    CleanupOwned,
    StableObservation,
    /// Contents an opaque `Binding<R>`/boundary-trait call seam may marshal as
    /// a caller-owned ABI copy while the observed place keeps custody of the
    /// whole value. Runtime contents are exactly `StableObservation`'s, but an
    /// `[erased]` member never materializes and never crosses the seam: it
    /// stays with the observed place, so it only resolves to its declared
    /// storage instead of needing stable contents of its own.
    SeamMarshal,
}

pub fn has_plain_owned_contents(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    has_plain_owned_contents_with_substitutions(program, reference, &[])
}

/// Owned storage a linear selection join can carry whole: no references,
/// slices, nominal cleanup, or recursive data, with the numeric-constraint
/// tolerance of [`has_plain_owned_contents_with_numeric_constraints`]. Linear
/// declarations remain carriers because the claim itself is tracked by the
/// caller's own custody machinery.
pub fn has_linear_owned_contents(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    check_contents_requirement(program, reference, &[], ContentsRequirement::LinearOwned)
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

/// Classify owned storage an affine caller may carry toward a consuming
/// machine: plain numeric-owned contents plus declarations owning an attached
/// `::drop` hook and `[erased]` members. The hook is admitted because cleanup
/// attaches at the consuming edge, and erased members are admitted because
/// they never produce runtime contents. This answers storage shape only —
/// it does not prove the caller's transfer or the hook's invocation.
pub fn has_cleanup_owned_contents(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    check_contents_requirement(program, reference, &[], ContentsRequirement::CleanupOwned)
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

/// Classify value contents an opaque `Binding<R>`/boundary-trait call seam may
/// marshal as a caller-owned ABI copy while the observed borrowed place keeps
/// custody of the whole value. Runtime (non-erased) contents must be stable
/// under observation exactly as for [`has_stable_observable_contents`]. An
/// `[erased]` member contributes no runtime contents, so it never crosses the
/// seam — it stays with the observed place — and needs only to resolve to its
/// declared storage. This answers marshal shape only; the caller's custody and
/// the provider's own establishment of the erased member remain their own
/// obligations.
pub fn has_service_seam_contents(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    check_contents_requirement(program, reference, &[], ContentsRequirement::SeamMarshal)
}

/// Classify a record whose fields are each either plain owned contents or a
/// shared view — a `&[T]` or `&'a V` member whose `&` shell carries the
/// field's `Unrestricted` multiplicity while its referent's loan stays with
/// the view's owner. The record itself remains an owned carrier: copying it
/// copies its scalars and its view descriptors whole, the same way a stored
/// view leaf copies out of a `&self` projection. Exclusive/`&mut` members,
/// erased members, and genuinely generic fields keep the record on its own
/// custody family.
pub fn has_owned_or_shared_view_fields(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    let symbol = match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => *symbol,
        // A lifetime-parameterized record is a `Generic` node whose type
        // arguments are empty — its fields bind lifetimes, not types.
        TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } if program
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty() =>
        {
            *base_symbol
        }
        _ => return false,
    };
    if !symbol.is_valid() {
        return false;
    }
    let mut definitions = program
        .data_definitions()
        .iter()
        .filter(|data| data.symbol == symbol);
    let Some(data) = definitions.next() else {
        return false;
    };
    if definitions.next().is_some() {
        return false;
    }
    // A linear declaration is not plain owned storage and a `::drop` carrier
    // belongs to the cleanup family — the shared views change neither.
    if data.properties.multiplicity == Multiplicity::Linear
        || program.machines().iter().any(|machine| {
            machine.attached_data_symbol == symbol && machine.name.as_str().ends_with("::drop")
        })
    {
        return false;
    }
    let shared_view = |field: &DataField| {
        matches!(
            program.type_reference_table.type_reference(field.type_reference),
            TypeReferenceNode::Reference {
                access: language_semantics::ReferenceAccess::Shared,
                referee,
                ..
            } if matches!(
                program.type_reference_table.type_reference(*referee),
                TypeReferenceNode::Slice { .. } | TypeReferenceNode::Named { .. }
            )
        )
    };
    let mut has_view = false;
    program.data_members(data).iter().all(|member| {
        let field = match member {
            DataMember::Field(field) => field,
            _ => return false,
        };
        has_view |= shared_view(field);
        !field.relevance.is_erased()
            && (has_plain_owned_contents_with_numeric_constraints(program, field.type_reference)
                || shared_view(field))
    }) && has_view
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
        } if matches!(
            requirement,
            ContentsRequirement::StableObservation | ContentsRequirement::SeamMarshal
        ) =>
        {
            Some(ContentType::Shared(Box::new(resolve(
                program,
                *referee,
                arguments,
                requirement,
            )?)))
        }
        TypeReferenceNode::Slice { element_type }
            if matches!(
                requirement,
                ContentsRequirement::StableObservation | ContentsRequirement::SeamMarshal
            ) =>
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
                    matches!(
                        requirement,
                        ContentsRequirement::StableObservation | ContentsRequirement::SeamMarshal
                    ) && is_plain_value_domain(program, domain)
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

pub(crate) fn is_plain_value_domain(
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
    // A linear declaration is not plain owned storage, but under LinearOwned it
    // is still a finite carrier whose custody the caller tracks explicitly.
    if (data.properties.multiplicity == Multiplicity::Linear
        && requirement != ContentsRequirement::LinearOwned)
        || active.iter().any(|ancestor| {
            matches!(ancestor, ContentType::Data(owner, _)
            if owner == symbol && resolved.size() >= ancestor.size())
        })
        || (requirement != ContentsRequirement::CleanupOwned
            && program.machines().iter().any(|machine| {
                machine.attached_data_symbol == *symbol && machine.name.as_str().ends_with("::drop")
            }))
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
        if field.relevance.is_erased() {
            // An erased member never materializes runtime contents; it still
            // must resolve to declared storage so the semantic member exists.
            // A drop carrier keeps it as owned cargo and a service seam leaves
            // it inside the observed place, so neither asks its contents to be
            // stable or cleanup-owned — only resolvable.
            return matches!(
                requirement,
                ContentsRequirement::CleanupOwned | ContentsRequirement::SeamMarshal
            ) && resolve(program, field.type_reference, &substitutions, requirement)
                .is_some();
        }
        resolve(program, field.type_reference, &substitutions, requirement)
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
