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
    /// Scalar-sized storage with synchronized mutation through shared access.
    /// Its layout does not make the resident an immutable entry observation.
    Atomic(PrimitiveType),
    Array(Box<ContentType>, usize),
    Slice(Box<ContentType>),
    Shared(Box<ContentType>),
    Data(SymbolHandle, Vec<ContentType>),
    /// A shared-view member — `&[T]` or `&'a V` — under
    /// [`ContentsRequirement::OwnedOrSharedView`]. A terminal descriptor leaf:
    /// the carrier copies base and extent while the referent's loan stays
    /// with the view's owner, so the referee's own contents never descend
    /// into this classification.
    View,
    /// A closed const argument. It fills a const parameter's slot so data
    /// arguments stay positional, and is never itself a member's contents.
    ConstValue,
    /// An existing provider handle; borrowing it neither installs a provider
    /// nor authorizes erasure. Shape collection retains that separate receipt.
    Provider,
}

impl ContentType {
    fn size(&self) -> usize {
        match self {
            Self::Scalar(_) | Self::Atomic(_) | Self::ConstValue | Self::View | Self::Provider => 1,
            Self::Array(element, _) => 1 + element.size(),
            Self::Slice(element) | Self::Shared(element) => 1 + element.size(),
            Self::Data(_, arguments) => 1 + arguments.iter().map(Self::size).sum::<usize>(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContentsRequirement {
    PlainOwned,
    /// Runtime scalar/record storage; erased members retain their resolved
    /// semantic type without imposing runtime contents on this classification.
    NumericOwned,
    /// An already established owned value whose exact constrained type survives
    /// the transfer. Bounded byte predicates describe live contents as well as
    /// storage; accepting their shape does not prove a fresh initializer.
    RepresentedOwned,
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
    /// `CleanupOwned` contents a structural literal establishes. A scalar member
    /// may additionally carry a complete plain scalar predicate: the field
    /// retains its restriction and construction proves it for that operand.
    /// Bounded byte members also retain represented primitive predicates; the
    /// constructor must prove these from each completed operand, independently
    /// of this storage classification. Other `CleanupOwned` callers keep
    /// refusing declared domains.
    StructuralConstruction,
    /// `NumericOwned` contents that may carry shared-view members at any
    /// depth: a `&[T]` or `&'a V` member resolves to a terminal
    /// [`ContentType::View`] descriptor instead of declining, and a view
    /// carrying declared domains on its referee (`&[u8] in Utf8`)
    /// classifies the same — the descriptor leaf rides its source loan's
    /// lineage and this classification neither proves nor extends the
    /// domain. Every other obligation is `NumericOwned`'s — no loans that
    /// mutate, no erased or generic members, no linear declarations, no
    /// drop carriers.
    OwnedOrSharedView,
    /// Established storage lent in place, with shared descriptors and provider
    /// handles classified per member. This is neither a copy nor a stable
    /// observation judgment. Exact access, lifetime, qualifications and provider
    /// custody remain with the borrowing operation.
    BorrowedStorage,
    StableObservation,
    /// Contents an opaque `Binding<R>`/boundary-trait call seam may marshal as
    /// a caller-owned ABI copy while the observed place keeps custody of the
    /// whole value. Runtime contents are exactly `StableObservation`'s, but an
    /// `[erased]` member never materializes and never crosses the seam: it
    /// stays with the observed place, so it only resolves to its declared
    /// storage instead of needing stable contents of its own.
    SeamMarshal,
}

/// Classify an existing aggregate's supported contents for an in-place loan.
/// Shared descriptors keep their backing loans; providers keep their installation
/// custody; nominal cleanup stays with the owner. Inspect authored reference
/// access before lowering erases it from slice shapes. This grants no copy,
/// construction, stable-observation or lifetime-extension authority.
pub fn has_borrowed_storage_contents(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    check_contents_requirement(
        program,
        reference,
        &[],
        ContentsRequirement::BorrowedStorage,
    )
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

/// Classify an owned snapshot whose consumers retain its exact represented
/// constraints. In addition to numeric restrictions, finite byte owners may
/// retain primitive byte predicates. This grants neither construction authority
/// nor loans: the caller must independently rejoin the established value,
/// complete type, and copy/transfer custody, including live byte lengths.
pub fn has_plain_owned_contents_with_represented_constraints(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    check_contents_requirement(
        program,
        reference,
        &[],
        ContentsRequirement::RepresentedOwned,
    )
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

/// Classify the contents a record or case literal establishes: exactly
/// [`has_cleanup_owned_contents`], plus scalar members under a plain domain
/// whose complete scalar predicate is represented. This is the same storage judgment for both
/// forms; bounded byte members may retain primitive byte predicates too. Each
/// construction still proves its members' scalar or byte-predicate obligations;
/// admitting storage never establishes a member's declared qualification.
pub fn has_structural_construction_contents(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    check_contents_requirement(
        program,
        reference,
        &[],
        ContentsRequirement::StructuralConstruction,
    )
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

/// Classify value contents that may nest shared-view members anywhere inside
/// them — the whole-contents form of [`has_owned_or_shared_view_fields`]. A
/// `&[T]` or `&'a V` member is a terminal view descriptor at any depth:
/// copying its enclosing carrier copies the descriptor, while the referent's
/// loan stays with the view's owner. Declared domains on the view's referee
/// (`&[u8] in Utf8`) classify identically — the constraint shells ride the
/// declared type and change neither the descriptor shape nor the loan.
/// Sums whose payloads are plain records or view-carrying records therefore
/// classify beside plain records. Exclusive borrows, erased members, linear
/// declarations, drop carriers, and genuinely generic members keep the type
/// unclassified, as does a member with no shared view of the `&[T]`/`&'a V`
/// shape.
pub fn has_owned_or_shared_view_contents(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    check_contents_requirement(
        program,
        reference,
        &[],
        ContentsRequirement::OwnedOrSharedView,
    )
}

/// Classify a record whose fields are each either plain owned contents or a
/// shared view — a `&[T]` or `&'a V` member whose `&` shell carries the
/// field's `Unrestricted` multiplicity while its referent's loan stays with
/// the view's owner. Declared domains on the view's referee (`&[u8] in Utf8`)
/// classify identically — a whole copy duplicates each view descriptor with
/// its source loan's lineage, and the domains hold because every write into
/// the source preserved them. The record itself remains an owned carrier:
/// copying it copies its scalars and its view descriptors whole, the same way
/// a stored view leaf copies out of a `&self` projection. Exclusive/`&mut`
/// members, erased members, and genuinely generic fields keep the record on
/// its own custody family. Copy permission neither establishes the declared
/// predicates nor extends the loan, so this answers only whether a whole copy
/// is a value transfer.
pub fn has_owned_or_shared_view_fields(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    owned_or_shared_view_fields(program, reference)
}

fn owned_or_shared_view_fields(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
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
    let Some(data) = definition(program, symbol) else {
        return false;
    };
    // A linear declaration is not plain owned storage and a `::drop` carrier
    // belongs to the cleanup family — the shared views change neither.
    if data.properties.multiplicity == Multiplicity::Linear || owns_drop_hook(program, symbol) {
        return false;
    }
    let shared_view = |field: &DataField| {
        let TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            referee,
            ..
        } = program
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            return false;
        };
        matches!(
            program
                .type_reference_table
                .type_reference(peeled_referee(program, *referee)),
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::Named { .. }
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
    if requirement == ContentsRequirement::BorrowedStorage
        && typed_trees::service::exact_bound_service_requirement(program, reference).is_some()
    {
        return Some(ContentType::Provider);
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
                let atomic = match program.symbols.builtin_type_atom(*symbol) {
                    Some(atom) => atom.is_atomic(),
                    None => program
                        .placed_field_plan_for_type_reference(reference)
                        .is_some_and(|field| {
                            matches!(field.access, access_plans::FieldAccess::Atomic { .. })
                        }),
                };
                if atomic {
                    return Some(ContentType::Atomic(primitive));
                }
                return Some(ContentType::Scalar(primitive));
            }
            if requirement == ContentsRequirement::BorrowedStorage
                && is_boundary_provider(program, *symbol)
            {
                return Some(ContentType::Provider);
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
            lifetime_arguments,
            ..
        } => {
            let data = definition(program, *base_symbol)?;
            let parameters = program.data_type_parameters(data);
            let source_arguments = program
                .type_reference_table
                .type_reference_handles(*source_arguments);
            if parameters.len() != source_arguments.len() {
                return None;
            }
            // A closed const argument is a compile-time value, not storage: it
            // owns no loans, cleanup, or contents. It is closed in the same
            // normalized form `validate_closed_const_argument` requires, a
            // symbol-free value leaf; an open binder or unevaluated expression
            // stays unclassified. A member that sizes storage by it needs an
            // independently retained closed instance below; this classifier
            // never evaluates const arguments or guesses array lengths.
            // Runtime-capable value binders and machine parameters stay out.
            let resolved = parameters
                .iter()
                .zip(source_arguments)
                .map(|(parameter, reference)| match parameter.kind {
                    typed_trees::data::TypeParameterKind::Type => {
                        resolve(program, *reference, arguments, requirement)
                    }
                    typed_trees::data::TypeParameterKind::Const { .. }
                        if matches!(
                            program.type_reference_table.type_reference(*reference),
                            TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid()
                        ) =>
                    {
                        Some(ContentType::ConstValue)
                    }
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?;
            if lifetime_arguments.is_empty()
                && let Some(instance) = program.closed_data_definition(reference)
            {
                // The retained instance's fields already contain exact
                // substituted extents. Reuse them only under its complete
                // original application identity, not its synthetic name.
                // Ambiguous/stale owners cannot select arbitrary contents.
                if instance.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                    || instance.properties != data.properties
                    || (!matches!(
                        requirement,
                        ContentsRequirement::CleanupOwned
                            | ContentsRequirement::StructuralConstruction
                            | ContentsRequirement::BorrowedStorage
                    ) && owns_drop_hook(program, *base_symbol))
                {
                    return None;
                }
                return Some(ContentType::Data(instance.symbol, Vec::new()));
            }
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
        // A shared view of slice or named shape is a member's terminal
        // descriptor leaf — the same shape [`has_owned_or_shared_view_fields`]
        // admits one record deep. The referee's declared storage stays its
        // owner's custody, so it is never resolved here; the loan's custody
        // remains the caller's obligation. `&mut` and `&T` scalar loans do not
        // match.
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Shared,
            referee,
            ..
        } if matches!(
            requirement,
            ContentsRequirement::OwnedOrSharedView
                | ContentsRequirement::StructuralConstruction
                | ContentsRequirement::BorrowedStorage
        ) && matches!(
            program
                .type_reference_table
                .type_reference(peeled_referee(program, *referee,)),
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::Named { .. }
        ) =>
        {
            Some(ContentType::View)
        }
        TypeReferenceNode::Reference {
            access: language_semantics::ReferenceAccess::Mutable,
            referee,
            ..
        } if requirement == ContentsRequirement::BorrowedStorage => matches!(
            resolve(program, *referee, arguments, requirement)?,
            ContentType::Provider
        )
        .then_some(ContentType::Provider),
        TypeReferenceNode::DynamicTrait { symbol, .. }
            if requirement == ContentsRequirement::BorrowedStorage
                && is_boundary_provider(program, *symbol) =>
        {
            Some(ContentType::Provider)
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
                typed_trees::types::TypeConstraintNode::Range { .. } => {
                    matches!(contents, ContentType::Scalar(_) | ContentType::Atomic(_))
                }
                // Operation policy changes neither storage nor custody. The
                // underlying contents have already been classified recursively;
                // keeping their policy cannot grant loans or erase cleanup.
                // Exact type correspondence remains the caller's obligation.
                typed_trees::types::TypeConstraintNode::ArithmeticDomain(_) => true,
                typed_trees::types::TypeConstraintNode::Domain(domain) => {
                    (matches!(
                        requirement,
                        ContentsRequirement::StableObservation | ContentsRequirement::SeamMarshal
                    ) && is_plain_value_domain(program, domain))
                        // A complete scalar predicate adds no storage or
                        // cleanup. The field catalog retains it, construction
                        // and replacement prove it, and source replay checks
                        // the exact declaration independently. Lending existing
                        // storage preserves that invariant without proving new
                        // membership or extending any backing loan.
                        || (matches!(
                            requirement,
                            ContentsRequirement::NumericOwned
                                | ContentsRequirement::RepresentedOwned
                                | ContentsRequirement::StructuralConstruction
                                | ContentsRequirement::BorrowedStorage
                        ) && matches!(contents, ContentType::Scalar(_) | ContentType::Atomic(_))
                            && is_plain_value_domain(program, domain)
                            && (crate::exact_declared_domain_interval(program, domain).is_some()
                                || matches!(contents, ContentType::Scalar(primitive)
                                    if crate::scalar_field_domain_predicate(program, domain, primitive).is_some())))
                        || (matches!(requirement,
                                ContentsRequirement::RepresentedOwned
                                    | ContentsRequirement::StructuralConstruction
                                    | ContentsRequirement::BorrowedStorage)
                            && matches!(&contents, ContentType::Array(element, _)
                                if matches!(element.as_ref(), ContentType::Scalar(PrimitiveType::U8)))
                            && is_plain_value_domain(program, domain)
                            && domain.arguments.is_empty()
                            && typed_trees::byte_predicates::domain_byte_predicate(
                                program, domain.symbol,
                            ).is_some())
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

/// A view referee's declared-domain shells peeled exactly as
/// `owned_or_shared_view_fields` peels them one record deep: constraint
/// wrappers ride the loan's declared type and change neither the viewed
/// shape nor which owner keeps the backing.
fn peeled_referee(program: &TypedTrees, mut referee: TypeReferenceHandle) -> TypeReferenceHandle {
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(referee)
    {
        referee = *base_type;
    }
    referee
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

fn is_boundary_provider(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    program
        .traits()
        .iter()
        .any(|definition| definition.symbol == symbol && definition.is_boundary)
}

fn definition(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    if !symbol.is_valid() {
        return None;
    }
    match crate::program_scope::data_definitions::memoized_data_definition_lookup(program, symbol) {
        crate::program_scope::data_definitions::DataDefinitionLookup::Unique(position) => {
            program.data_definitions().get(position as usize)
        }
        _ => None,
    }
}

/// Whether a machine attached to `symbol` realizes `::drop`, resolved
/// through the build-scope memo when one is open.
fn owns_drop_hook(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    crate::program_scope::value_properties::memoized_owns_drop_hook(program, symbol)
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
            // Shared atomic access may change the resident without invalidating
            // the loan. Owned storage classification is not observation authority.
            ContentType::Atomic(_) => !matches!(
                requirement,
                ContentsRequirement::StableObservation
                    | ContentsRequirement::SeamMarshal
                    | ContentsRequirement::BorrowedStorage
            ),
            ContentType::Array(element, _)
            | ContentType::Slice(element)
            | ContentType::Shared(element) => {
                check_contents(program, element, active, complete, requirement)
            }
            // A view descriptor is a terminal leaf — the member's custody is
            // the descriptor alone.
            ContentType::View | ContentType::Provider => true,
            // A const parameter named where a member type belongs is not
            // storage the classifier understands.
            ContentType::ConstValue => false,
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
        || (!matches!(
            requirement,
            ContentsRequirement::CleanupOwned
                | ContentsRequirement::StructuralConstruction
                | ContentsRequirement::BorrowedStorage
        ) && owns_drop_hook(program, *symbol))
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
            // Runtime construction/storage and in-place loans do not execute,
            // lend, or dispose of the erased subject. Its proof, multiplicity
            // and provenance remain obligations of semantic checking. Copy and
            // stable-observation classifiers keep their separate admission.
            return matches!(
                requirement,
                ContentsRequirement::NumericOwned
                    | ContentsRequirement::CleanupOwned
                    | ContentsRequirement::StructuralConstruction
                    | ContentsRequirement::BorrowedStorage
                    | ContentsRequirement::SeamMarshal
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
