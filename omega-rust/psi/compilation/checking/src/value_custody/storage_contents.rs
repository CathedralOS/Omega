//! Distinguish owned storage from contents stable under shared observation.
//!
//! Both questions need the same closed generic substitution and recursive-data
//! checks. A shared loan can freeze ordinary contents without owning them; that
//! never authorizes no-code moves, cleanup erasure, or a copied reference ABI.
//! Observation permits only recursively stable contents and understood value
//! refinements. Predicate-only value facts share callable contracts' complete
//! subject-bound meaning; they add no storage or cleanup. Classifying their
//! carrier proves no membership and preserves their invariant-window custody.
//! Unknown qualifications and mutation-capable loans stay opaque.

use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::data::{DataField, DataMember};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

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
    Data {
        symbol: SymbolHandle,
        application: TypeReferenceHandle,
        arguments: Vec<ContentType>,
    },
    /// A shared-view member — `&[T]` or `&'a V` — under
    /// [`ContentsRequirement::OwnedOrSharedView`], or a reference carrier under
    /// [`ContentsRequirement::DescriptorCleanup`]. This is a terminal leaf:
    /// the referee remains borrowed and its contents never descend into the
    /// carrier's classification. The operation retains loan access and closure.
    View,
    /// A closed const argument. It fills a const parameter's slot so data
    /// arguments stay positional, and is never itself a member's contents.
    ConstValue,
    /// An admitted value index is an application operand, never a stored type.
    RuntimeIndex,
    /// An existing provider handle; borrowing it neither installs a provider
    /// nor authorizes erasure. Shape collection retains that separate receipt.
    Provider,
}

impl ContentType {
    fn size(&self) -> usize {
        match self {
            Self::Scalar(_)
            | Self::Atomic(_)
            | Self::ConstValue
            | Self::RuntimeIndex
            | Self::View
            | Self::Provider => 1,
            Self::Array(element, _) => 1 + element.size(),
            Self::Slice(element) | Self::Shared(element) => 1 + element.size(),
            Self::Data { arguments, .. } => 1 + arguments.iter().map(Self::size).sum::<usize>(),
        }
    }
}

/// The storage question a caller asks of a type's contents. Each answer is a
/// storage-shape judgment only; the caller keeps every custody, cleanup and
/// qualification obligation the variant names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentsRequirement {
    PlainOwned,
    /// Runtime scalar/record storage; erased members retain their resolved
    /// semantic type without imposing runtime contents on this classification.
    /// Primitive numeric restrictions are allowed; callers still owe exact
    /// source type and value-constraint correspondence, and this does not
    /// extend structural-return or construction routes that erase them.
    NumericOwned,
    /// An already established owned value whose exact constrained type survives
    /// the transfer. Bounded byte predicates describe live contents as well as
    /// storage; accepting their shape does not prove a fresh initializer.
    /// This grants neither construction authority nor loans: the caller must
    /// independently rejoin the established value, complete type, and
    /// copy/transfer custody, including live byte lengths.
    RepresentedOwned,
    /// Finite storage with no retained reference descriptors. Nominal cleanup,
    /// linear custody and represented predicates are independent obligations;
    /// none prolongs the loans used to produce this value. This proves neither
    /// copy permission nor cleanup eligibility. Erased fields are classified
    /// too: unresolved or reference-bearing subjects stay opaque.
    BorrowFree,
    /// Finite owned storage whose linear claims and nominal cleanup remain
    /// explicit custody obligations of the consuming operation. Classifying
    /// whole storage grants neither partial moves nor automatic disposition;
    /// those require the exact owner's independent custody and cleanup plans.
    /// References, slices and recursive by-value storage remain excluded.
    LinearOwned,
    /// Owned storage a consuming machine may carry through a `drop(value)`
    /// call: like `NumericOwned` it forbids loans, recursive carriers, and
    /// unclassified storage, but the declaration may own an attached `::drop`
    /// hook — the caller's transfer edge, not the classifier, proves the hook
    /// is invoked — and `[erased]` members are admitted because they never
    /// produce runtime contents. An erased member is still resolved so its
    /// declared type exists; its contents never materialize.
    CleanupOwned,
    /// Finite owned contents a structural literal establishes. Linear claims
    /// and nominal cleanup remain obligations of the completed operands and
    /// construction, never permissions granted by this shape classification.
    /// A scalar member
    /// may additionally carry a complete plain scalar predicate: the field
    /// retains its restriction and construction proves it for that operand.
    /// Bounded byte members also retain represented primitive predicates; the
    /// constructor must prove these from each completed operand, independently
    /// of this storage classification. Other `CleanupOwned` callers keep
    /// refusing declared domains. A declared domain on nominal data leaves
    /// the physical contents unchanged; checking proves the completed child's
    /// membership separately and lowering retains that occurrence's decision.
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
    /// Closed owned storage and reference descriptors whose removal executes
    /// no nominal cleanup and settles no conserved claim. A descriptor is a
    /// terminal leaf: its referee remains borrowed, never owned or disposed.
    /// Shared and exclusive carriers retain exact access and child closure at
    /// each replacement; this judgment grants no copy or observation authority.
    DescriptorCleanup,
    /// Established storage lent in place, with shared descriptors and provider
    /// handles classified per member. This is neither a copy nor a stable
    /// observation judgment. Atomic members retain their original cells, not
    /// scalar snapshots or ordinary byte-access authority. Exact access,
    /// lifetime, qualifications and provider custody remain with the borrowing
    /// operation. Authored reference access
    /// is inspected before lowering erases it from slice shapes.
    BorrowedStorage,
    /// Value contents that an immutable binding can retain as an entry
    /// observation while its loans remain valid. This proves neither binding
    /// immutability nor loan validity; those remain the caller's obligations.
    /// Shared contents are not necessarily owned, and synchronized/interior
    /// mutation or unclassified qualifications cannot supply a stable
    /// observation.
    StableObservation,
    /// Contents an opaque `Binding<R>`/boundary-trait call seam may marshal as
    /// a caller-owned ABI copy while the observed place keeps custody of the
    /// whole value. Runtime contents are exactly `StableObservation`'s, but an
    /// `[erased]` member never materializes and never crosses the seam: it
    /// stays with the observed place, so it only resolves to its declared
    /// storage instead of needing stable contents of its own.
    SeamMarshal,
}

/// Ask the same contents judgment under the exact retained type telescope.
/// Unknown actuals remain opaque; substitutions never supply live custody.
pub fn has_contents(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    requirement: ContentsRequirement,
) -> bool {
    check_contents_requirement(program, reference, substitutions, requirement)
}

/// A shape collector can hold a selected data declaration and its exact
/// parameter telescope without manufacturing a source TypeReference. Reuse
/// the contents engine on that declaration input; unrelated lexical binders
/// do not become arguments of the data application.
pub fn data_has_contents(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    requirement: ContentsRequirement,
) -> bool {
    // A closed instance retains its source application. Its base declaration
    // owns nominal cleanup even when the instantiated shell has another symbol.
    // That generated concrete origin is already resolved; outer lexical binders
    // are not additional arguments of its retained closed application.
    if let Some(application) = data.generic_instance {
        return has_contents(program, application, &[], requirement);
    }
    let parameters = program.data_type_parameters(data);
    let arguments = if parameters.is_empty() {
        Vec::new()
    } else {
        let Some(arguments) = contents_arguments(program, substitutions, parameters, requirement)
        else {
            return false;
        };
        let Some(arguments) = parameters
            .iter()
            .map(|parameter| {
                arguments
                    .iter()
                    .rev()
                    .find(|(symbol, _)| *symbol == parameter.symbol)
                    .map(|(_, argument)| (parameter.symbol, argument.clone()))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        arguments
    };
    let resolved = ContentType::Data {
        symbol: data.symbol,
        // This declaration input has no authored application row. Its exact
        // telescope supplies the fields; a fabricated reference must not select
        // a retained closed view belonging to another application.
        application: TypeReferenceHandle::invalid(),
        arguments: arguments
            .into_iter()
            .map(|(_, argument)| argument)
            .collect(),
    };
    check_contents(
        program,
        &resolved,
        &mut Vec::new(),
        &mut Vec::new(),
        requirement,
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
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    owned_or_shared_view_fields(program, reference)
}

fn owned_or_shared_view_fields(
    program: &dyn crate::CheckingInput,
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
                .type_reference(crate::peel_declared_constraints(program, *referee)),
            TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Generic { .. }
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
            && (has_contents(
                program,
                field.type_reference,
                &[],
                ContentsRequirement::NumericOwned,
            ) || shared_view(field))
    }) && has_view
}

fn check_contents_requirement(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    requirement: ContentsRequirement,
) -> bool {
    let Some(arguments) = contents_arguments(program, substitutions, &[], requirement) else {
        return false;
    };
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

/// Resolve each actual under the retained earlier telescope, as the source
/// application did. Data and reference input modes share this one environment.
fn contents_arguments(
    program: &dyn crate::CheckingInput,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    parameters: &[typed_trees::data::TypeParameter],
    requirement: ContentsRequirement,
) -> Option<Vec<(SymbolHandle, ContentType)>> {
    let mut arguments = Vec::new();
    for (symbol, argument) in substitutions {
        let argument = match parameters
            .iter()
            .find(|parameter| parameter.symbol == *symbol)
        {
            Some(parameter) => {
                resolve_parameter_argument(program, parameter, *argument, &arguments, requirement)
            }
            None => resolve(program, *argument, &arguments, requirement),
        }?;
        arguments.push((*symbol, argument));
    }
    Some(arguments)
}

fn resolve_parameter_argument(
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::data::TypeParameter,
    reference: TypeReferenceHandle,
    arguments: &[(SymbolHandle, ContentType)],
    requirement: ContentsRequirement,
) -> Option<ContentType> {
    match parameter.kind {
        // Argument admission and captured-subject equality are separate
        // checks. Contents follow the substituted fields, so this marker
        // cannot supply a field type, extent, loan, or cleanup permission.
        typed_trees::data::TypeParameterKind::Value { .. } => Some(ContentType::RuntimeIndex),
        typed_trees::data::TypeParameterKind::Type => {
            resolve(program, reference, arguments, requirement)
        }
        typed_trees::data::TypeParameterKind::Const { .. }
            if matches!(program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. } if !symbol.is_valid()) =>
        {
            Some(ContentType::ConstValue)
        }
        _ => None,
    }
}

fn resolve(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    arguments: &[(SymbolHandle, ContentType)],
    requirement: ContentsRequirement,
) -> Option<ContentType> {
    if !reference.is_valid() {
        return None;
    }
    if requirement == ContentsRequirement::BorrowedStorage
        && typed_trees::service::service_requirement_declaration(program, reference).is_some()
    {
        return Some(ContentType::Provider);
    }
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            if let Some(argument) = crate::substitutions::actual_for(arguments, *symbol) {
                return Some(argument);
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
                .then_some(ContentType::Data {
                    symbol: *symbol,
                    application: reference,
                    arguments: Vec::new(),
                })
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
            // Runtime indices likewise carry no storage here; their ordinary
            // represented fields must independently satisfy this judgment.
            let resolved = parameters
                .iter()
                .zip(source_arguments)
                .map(|(parameter, reference)| {
                    resolve_parameter_argument(
                        program,
                        parameter,
                        *reference,
                        arguments,
                        requirement,
                    )
                })
                .collect::<Option<Vec<_>>>()?;
            if let Some(instance) = program.closed_data_definition(reference) {
                // The retained instance's fields already contain exact
                // substituted extents. Reuse them only under its complete
                // original application identity, not its synthetic name.
                // Ambiguous/stale owners cannot select arbitrary contents.
                if instance.supply_mode != language_semantics::DataSupplyMode::CheckedShape
                    || instance.properties != data.properties
                    || (!matches!(
                        requirement,
                        ContentsRequirement::LinearOwned
                            | ContentsRequirement::BorrowFree
                            | ContentsRequirement::CleanupOwned
                            | ContentsRequirement::StructuralConstruction
                            | ContentsRequirement::BorrowedStorage
                    ) && owns_drop_hook(program, *base_symbol))
                {
                    return None;
                }
                return Some(ContentType::Data {
                    symbol: instance.symbol,
                    application: reference,
                    arguments: resolved,
                });
            }
            Some(ContentType::Data {
                symbol: *base_symbol,
                application: reference,
                arguments: resolved,
            })
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
        TypeReferenceNode::Reference { .. }
            if requirement == ContentsRequirement::DescriptorCleanup =>
        {
            Some(ContentType::View)
        }
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
        // A shared view of a slice or nominal application is a member's terminal
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
                .type_reference(crate::peel_declared_constraints(program, *referee)),
            TypeReferenceNode::Slice { .. }
                | TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Generic { .. }
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
        // A selector is one plain word naming an implementation table: it
        // owns and borrows nothing. Local tables never cross a component
        // seam, so it has no marshaled form.
        TypeReferenceNode::DynamicTrait { .. }
            if requirement != ContentsRequirement::SeamMarshal =>
        {
            Some(ContentType::Scalar(PrimitiveType::Selector))
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
                    // These operations retain the complete structural type and
                    // its independent membership/custody judgments. A routed
                    // atom changes those judgments, not the recursively owned
                    // carrier. Copy, erasure and stable-observation permission
                    // remain separate; scalar predicates keep their codecs below.
                    if matches!(requirement,
                        ContentsRequirement::StructuralConstruction
                            | ContentsRequirement::RepresentedOwned
                            | ContentsRequirement::BorrowedStorage
                            | ContentsRequirement::BorrowFree)
                        && matches!(contents, ContentType::Data { .. })
                        && domain.subject == typed_trees::types::DomainConstraintSubject::Declared
                        && domain.semantic_id.is_valid()
                        && typed_trees::domain::domain_by_symbol(program, domain.symbol).is_some()
                    {
                        return true;
                    }
                    // Predicate-only value facts have no semantic atom, new
                    // storage or cleanup action. Every content judgment keeps
                    // the same carrier; invariant-window checking still owns
                    // its validity. Declared theories retain the narrower
                    // custody rules below, including routes and semantic roles.
                    if matches!(domain.subject,
                        typed_trees::types::DomainConstraintSubject::Value(_))
                    {
                        return matches!(contents, ContentType::Scalar(primitive)
                            if crate::scalar_field_domain_predicate(program, domain, primitive).is_some());
                    }
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
                                | ContentsRequirement::BorrowFree
                                | ContentsRequirement::StructuralConstruction
                                | ContentsRequirement::BorrowedStorage
                        ) && matches!(contents, ContentType::Scalar(_) | ContentType::Atomic(_))
                            && is_plain_value_domain(program, domain)
                            && (crate::exact_declared_domain_interval(program, domain).is_some()
                                || matches!(contents, ContentType::Scalar(primitive)
                                    if crate::scalar_field_domain_predicate(program, domain, primitive).is_some())))
                        || (matches!(requirement,
                                ContentsRequirement::RepresentedOwned
                                    | ContentsRequirement::BorrowFree
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

pub(crate) fn is_plain_value_domain(
    program: &dyn crate::CheckingInput,
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
        .domain_definition(domain.symbol)
        .is_some_and(|definition| {
            definition.alias.is_none()
                && definition.classification.is_none()
                && definition.semantic_roles.is_empty()
                && definition.establishment_routes.is_empty()
        })
}

fn is_boundary_provider(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
    program
        .traits()
        .iter()
        .any(|definition| definition.symbol == symbol && definition.is_boundary)
}

fn definition(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    if !symbol.is_valid() {
        return None;
    }
    match crate::declarations::data_queries::memoized_data_definition_lookup(program, symbol) {
        crate::declarations::data_queries::DataDefinitionLookup::Unique(position) => {
            program.data_definitions().get(position as usize)
        }
        _ => None,
    }
}

/// Whether a machine attached to `symbol` realizes `::drop`, resolved
/// through the build-scope memo when one is open.
fn owns_drop_hook(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
    crate::value_custody::type_properties::memoized_owns_drop_hook(program, symbol)
}

fn check_contents(
    program: &dyn crate::CheckingInput,
    resolved: &ContentType,
    active: &mut Vec<ContentType>,
    complete: &mut Vec<ContentType>,
    requirement: ContentsRequirement,
) -> bool {
    if complete.contains(resolved) {
        return true;
    }
    let ContentType::Data {
        symbol,
        application,
        arguments,
    } = resolved
    else {
        return match resolved {
            ContentType::Scalar(_) => true,
            // Shared atomic access may change the resident without invalidating
            // the loan. Owning or lending the original cell grants neither
            // stable observation nor ordinary scalar/byte access to its resident.
            ContentType::Atomic(_) => !matches!(
                requirement,
                ContentsRequirement::StableObservation
                    | ContentsRequirement::SeamMarshal
                    | ContentsRequirement::DescriptorCleanup
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
            ContentType::ConstValue | ContentType::RuntimeIndex => false,
            ContentType::Data { .. } => unreachable!(),
        };
    };
    // A prepared view may keep its template's symbol. The exact application,
    // not a nominal lookup, owns its closed fields. Scoped template arguments
    // remain scratch for applications that preparation could not close.
    let (data, substitutions) = if let Some(data) = program.closed_data_definition(*application) {
        (data, Vec::new())
    } else {
        let Some(data) = definition(program, *symbol) else {
            return false;
        };
        let parameters = program.data_type_parameters(data);
        if parameters.len() != arguments.len() {
            return false;
        }
        let substitutions = parameters
            .iter()
            .zip(arguments)
            .map(|(parameter, argument)| (parameter.symbol, argument.clone()))
            .collect::<Vec<_>>();
        (data, substitutions)
    };
    // A repeated declaration must consume finite type-argument structure, as
    // in Wrapper<Wrapper<Value>>. Expanding recursive by-value data is not a
    // finite owned carrier and must not make this classifier recurse forever.
    // A linear declaration is not plain owned storage. Explicit custody
    // operations and structural construction retain its finite carrier.
    // Lending established storage in place likewise grants no copy, transfer
    // or cleanup authority; the exact loan retains all owner obligations.
    if (data.properties.multiplicity == Multiplicity::Linear
        && !matches!(
            requirement,
            ContentsRequirement::LinearOwned
                | ContentsRequirement::BorrowFree
                | ContentsRequirement::StructuralConstruction
                | ContentsRequirement::BorrowedStorage
        ))
        || active.iter().any(|ancestor| {
            matches!(ancestor, ContentType::Data { symbol: owner, .. }
            if owner == symbol && resolved.size() >= ancestor.size())
        })
        || (!matches!(
            requirement,
            ContentsRequirement::LinearOwned
                | ContentsRequirement::BorrowFree
                | ContentsRequirement::CleanupOwned
                | ContentsRequirement::StructuralConstruction
                | ContentsRequirement::BorrowedStorage
        ) && owns_drop_hook(program, *symbol))
    {
        return false;
    }
    active.push(resolved.clone());
    let mut check_field = |field: &typed_trees::data::DataField| {
        if field.relevance.is_erased() {
            if requirement == ContentsRequirement::BorrowFree {
                // Do not infer absence of loans from erasure alone. Classify
                // the complete subject conservatively, including descendants.
                return resolve(program, field.type_reference, &substitutions, requirement)
                    .is_some_and(|field| {
                        check_contents(program, &field, active, complete, requirement)
                    });
            }
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
