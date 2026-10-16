//! Deterministic semantic identity for typed type references.
//!
//! Diagnostic rendering is deliberately not an identity oracle. In
//! particular, a domain conjunction is commutative and idempotent, while its
//! authored spelling has an order and may repeat terms. This module owns the
//! canonical form used by type equality, specialization, and published plan
//! identities (decision 19 / DOM4).
//! Ordinary identities distinguish authored and toolchain declaration
//! namespaces even when their visible paths agree. Package-review identities
//! additionally retain exact package owners; neither route uses a display name
//! to merge unrelated nominal carriers.
//!
//! This file owns the `TypedTrees` identity queries. `normalized_identities.rs`
//! holds the identity carriers, `callable_identity.rs` interns named-callable
//! identities and hands out their handles, `identity_context.rs` normalizes
//! type references in context, `constraint_identity.rs` normalizes constraints
//! and domains, `open_index_identity.rs` identifies open index operations and
//! `result_dispatch_terms.rs` collects result dispatch terms.

mod callable_identity;
mod constraint_identity;
mod derived_declarations;
mod identity_context;
mod lifetime_parameters;
mod normalized_identities;
mod open_index_identity;
mod result_dispatch_terms;
mod storage_identity;
mod substitution;

pub use callable_identity::{
    CallableIdentityId, CallableIdentityMemo, CallableIdentityTable, CallableParametersId,
    CallablePathId, CallableResultDispatchId,
};
pub use language_semantics::type_identity::{
    TypeIdentityPackageOwnerVisitor, TypeIdentityVisitError, visit_type_identity_package_owners,
};
pub use lifetime_parameters::{
    LifetimeParameterIdentity, applied_lifetime_parameter_identity,
    applied_lifetime_parameter_identity_with_supplier, lifetime_parameter_identity,
    lifetime_parameter_name, portable_lifetime_parameter_identity,
};
pub use normalized_identities::{
    NormalizedDomainExpression, NormalizedDomainTerm, NormalizedResultDispatchSet,
    NormalizedTypeIdentity, TypeIdentityId, TypeIdentityTable,
};

use crate::TypedTrees;

/// Mint or rejoin the canonical handle for one rendered structural key. All
/// rendering happens outside the interner: producers compute their text,
/// then intern it here.
fn program_intern_type_identity(program: &TypedTrees, identity: String) -> TypeIdentityId {
    // The rendered text is the structural key, and the table only grows, so
    // the same key mints the same handle for the program's lifetime, even
    // across arena revisions. See `arena::OwnedInterner`.
    program
        .type_identities
        .with_mut(|table| table.intern(&identity))
}
use crate::typed_trees::type_system::type_identity::constraint_identity::atom;
use crate::typed_trees::type_system::type_identity::constraint_identity::compound;
use crate::typed_trees::type_system::type_identity::identity_context::SubstitutionSemantics;
use crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityContext;
pub use crate::typed_trees::type_system::type_identity::identity_context::TypeIdentityQualification;
use crate::typed_trees::type_system::type_identity::identity_context::normalize_type_reference;
use crate::typed_trees::type_system::type_identity::result_dispatch_terms::collect_result_dispatch_terms;
use crate::types::TypeReferenceHandle;
use std::cell::Cell;
use symbols::SymbolHandle;

/// An index operand or a simultaneous caller-side replacement. These are
/// borrowed views over existing custody, never synthetic expression nodes.
#[derive(Clone, Copy)]
pub enum IndexIdentityArgument<'argument> {
    /// A type-role operand uses type identity, not the symbolic index-name
    /// grammar. Bare nominal types must agree with their source static form.
    TypeArgument(TypeReferenceHandle),
    TypeReference(TypeReferenceHandle),
    Expression(crate::expression::ExpressionHandle),
    Subject(SymbolHandle),
    StaticArgument(&'argument crate::expression::StaticMachineArgument),
}

/// One simultaneous declaration telescope. Its operands belong to the outer
/// scope, never to another binder in this same telescope.
#[derive(Clone, Copy)]
pub struct AppliedTypeIdentityScope<'argument> {
    pub declaration: SymbolHandle,
    pub arguments: &'argument [(SymbolHandle, IndexIdentityArgument<'argument>)],
    pub lifetimes: &'argument [(String, String)],
}

/// Local application equality retains erased regions as lexical-owner and
/// declaration-order identities. It shares the ordinary structural/index
/// normalizer, but is not a replacement for runtime-carrier identity or a new
/// published commitment encoding. Missing region custody fails closed.
pub fn applied_type_identity(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_type_identity(
        program,
        reference,
        lexical_owner,
        lexical_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
    )
}

/// Compare an authored receiving type under operands supplied by another
/// declaration. The two owners remain distinct throughout region lookup.
pub fn applied_type_identity_in_scope(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    receiving_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_type_identity(
        program,
        reference,
        receiving_owner,
        argument_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
    )
}

/// Identify a value-role operand under its supplying telescope. A named const
/// and its canonical literal denote the same value; neither is a nominal type.
/// Carrier and width admission remain the receiving declaration's obligation.
pub fn applied_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_index_identity(
        program,
        argument,
        lexical_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
    )
}

/// Publish the same value-role operand identity with exact package ownership.
/// Carrier admission remains separate; a const symbol is a value occurrence,
/// never a nominal type merely because it has retained source metadata.
pub fn portable_applied_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_index_identity(
        program,
        argument,
        lexical_owner,
        scopes,
        TypeIdentityQualification::PackageQualified,
    )
}

fn qualified_applied_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    qualification: TypeIdentityQualification,
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let identity = substitution::index_argument(
        program,
        argument,
        &TypeIdentityContext {
            application_scopes: scopes,
            application_owner: lexical_owner,
            qualification,
            invalid_application: Some(&invalid),
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(program_intern_type_identity(program, identity))
}

/// Exact application identity for publication. Nominals retain package owners;
/// regions retain their supplying declaration kind, overload and ordinal.
/// This is not a loan, provider admission, or a generic coverage theorem.
pub fn portable_applied_type_identity(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_type_identity(
        program,
        reference,
        lexical_owner,
        lexical_owner,
        scopes,
        TypeIdentityQualification::PackageQualified,
    )
}

fn qualified_applied_type_identity(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    lexical_owner: SymbolHandle,
    argument_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    qualification: TypeIdentityQualification,
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let identity = normalize_type_reference(
        program,
        reference,
        &TypeIdentityContext {
            application_scopes: scopes,
            application_owner: lexical_owner,
            application_argument_owner: argument_owner,
            qualification,
            invalid_application: Some(&invalid),
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(program_intern_type_identity(program, identity))
}

/// Resolve one erased region through the same simultaneous application scopes
/// as its containing type. This is local semantic identity, not loan authority
/// or a portable artifact coordinate.
pub fn applied_lifetime_identity(
    program: &TypedTrees,
    name: &str,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_lifetime_identity(
        program,
        name,
        lexical_owner,
        scopes,
        TypeIdentityQualification::Ordinary,
    )
}

/// Publish one resolved region through the same simultaneous scopes as its
/// containing argument, including unused public lifetime operands.
pub fn portable_applied_lifetime_identity(
    program: &TypedTrees,
    name: &str,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<NormalizedTypeIdentity> {
    qualified_applied_lifetime_identity(
        program,
        name,
        lexical_owner,
        scopes,
        TypeIdentityQualification::PackageQualified,
    )
}

fn qualified_applied_lifetime_identity(
    program: &TypedTrees,
    name: &str,
    lexical_owner: SymbolHandle,
    scopes: &[AppliedTypeIdentityScope<'_>],
    qualification: TypeIdentityQualification,
) -> Option<NormalizedTypeIdentity> {
    let invalid = Cell::new(false);
    let identity = TypeIdentityContext {
        application_scopes: scopes,
        application_owner: lexical_owner,
        qualification,
        invalid_application: Some(&invalid),
        ..TypeIdentityContext::default()
    }
    .lifetime(program, name);
    (!invalid.get()).then_some(program_intern_type_identity(program, identity))
}

/// Intern one complete local application key without exposing declaration or
/// region arena identities as a portable commitment.
pub(crate) fn local_trait_application_identity(
    program: &TypedTrees,
    declaration: SymbolHandle,
    arguments: &[NormalizedTypeIdentity],
    lifetimes: &[NormalizedTypeIdentity],
) -> Option<NormalizedTypeIdentity> {
    let render = |identities: &[NormalizedTypeIdentity]| {
        identities
            .iter()
            .map(|identity| program.type_identity_text(*identity))
            .collect::<Option<Vec<_>>>()
    };
    let identity = compound(
        "local-trait-application",
        [
            atom(
                "declaration",
                &format!("{}:{}", declaration.arena_index(), declaration.generation()),
            ),
            compound("arguments", render(arguments)?),
            compound("lifetimes", render(lifetimes)?),
        ],
    );
    Some(program_intern_type_identity(program, identity))
}

/// Portable counterpart of the local application key. Package-owned nominal
/// coordinates and complete scoped operands replace every arena coordinate.
pub(crate) fn portable_trait_application_identity(
    program: &TypedTrees,
    declaration: SymbolHandle,
    arguments: &[NormalizedTypeIdentity],
    lifetimes: &[NormalizedTypeIdentity],
) -> Option<NormalizedTypeIdentity> {
    let path = program.symbols.display_path(declaration, "::");
    if !declaration.is_valid() || path.is_empty() {
        return None;
    }
    let coordinate = TypeIdentityContext {
        qualification: TypeIdentityQualification::PackageQualified,
        ..TypeIdentityContext::default()
    }
    .qualify_non_binder_name(program, declaration, path);
    let render = |identities: &[NormalizedTypeIdentity]| {
        identities
            .iter()
            .map(|identity| program.type_identity_text(*identity))
            .collect::<Option<Vec<_>>>()
    };
    Some(program_intern_type_identity(
        program,
        compound(
            "trait-application",
            [
                atom("declaration", &coordinate),
                compound("arguments", render(arguments)?),
                compound("lifetimes", render(lifetimes)?),
            ],
        ),
    ))
}

/// Compare static indices and local fact operands through the same licensed
/// normalizer as type identity. Replacements belong to the caller's scope and
/// are therefore simultaneous: a swapped telescope must not bind twice.
/// Invalid custody cannot produce an identity that compares equal to itself.
pub fn normalized_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
) -> Option<String> {
    let invalid = Cell::new(false);
    let identity = substitution::index_argument(
        program,
        argument,
        &TypeIdentityContext {
            index_substitutions: substitutions,
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(identity)
}

/// Normalize a symbolic expression with declaration-local binder identities.
/// This shares the index-expression normalizer: renaming a bound subject does
/// not change a predicate, while references outside the binder list retain
/// their selected declarations. The result describes structure, not a proof
/// that two different expressions are mathematically equivalent.
pub fn normalized_expression_identity(
    program: &TypedTrees,
    expression: crate::expression::ExpressionHandle,
    binders: &[(SymbolHandle, String)],
) -> String {
    identity_context::normalize_index_expression(
        program,
        expression,
        &TypeIdentityContext {
            binders,
            ..TypeIdentityContext::default()
        },
    )
}

/// Proof correspondence needs a complete projection, unlike a diagnostic
/// descriptor. In particular, unresolved static evidence cannot compare equal
/// merely because both operands rendered the same unsupported marker.
pub fn try_normalized_index_identity(
    program: &TypedTrees,
    argument: IndexIdentityArgument<'_>,
    binders: &[(SymbolHandle, String)],
    application_scopes: &[AppliedTypeIdentityScope<'_>],
) -> Option<String> {
    let invalid = Cell::new(false);
    let identity = substitution::index_argument(
        program,
        argument,
        &TypeIdentityContext {
            binders,
            application_scopes,
            invalid_index_argument: Some(&invalid),
            ..TypeIdentityContext::default()
        },
    );
    (!invalid.get()).then_some(identity)
}

/// One request for a normalized type identity: the type reference, the
/// generic binders it may mention, the exact type-parameter substitutions
/// to follow first, and whether nominals carry their package owner.
#[derive(Clone, Copy)]
pub struct TypeIdentityRequest<'a> {
    pub type_reference: TypeReferenceHandle,
    pub binders: &'a [(SymbolHandle, String)],
    pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
    pub qualification: TypeIdentityQualification,
}

impl TypeIdentityRequest<'_> {
    /// The plain local identity of one type reference.
    pub const fn ordinary(type_reference: TypeReferenceHandle) -> Self {
        Self {
            type_reference,
            binders: &[],
            substitutions: &[],
            qualification: TypeIdentityQualification::Ordinary,
        }
    }

    /// The package-graph identity of one type reference.
    pub const fn package_qualified(type_reference: TypeReferenceHandle) -> Self {
        Self {
            type_reference,
            binders: &[],
            substitutions: &[],
            qualification: TypeIdentityQualification::PackageQualified,
        }
    }
}

/// One request for an exact-owner identity: package qualification where every
/// source-backed toolchain nominal must match one of the exact toolchain
/// sources and every other non-binder nominal must have a managed package
/// owner, failing closed otherwise.
#[derive(Clone, Copy)]
pub struct ExactOwnerTypeIdentityRequest<'a> {
    pub type_reference: TypeReferenceHandle,
    pub binders: &'a [(SymbolHandle, String)],
    pub substitutions: &'a [(SymbolHandle, TypeReferenceHandle)],
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
}

impl TypedTrees {
    /// Resolve an unwrapped nominal carrier to its uniquely retained closed
    /// declaration. Named carriers may retain erased lifetime binders, but
    /// neither route returns a declaration with open type parameters.
    /// Applied types join through the exact resolved base and
    /// complete normalized origin, never a generated declaration's spelling.
    ///
    /// This reads retained specialization evidence; it does not specialize an
    /// open declaration, evaluate arguments, discharge predicates, or erase
    /// reference/qualification wrappers. Consumers still check their own
    /// storage and semantic requirements on the returned declaration.
    pub fn closed_data_definition(
        &self,
        reference: TypeReferenceHandle,
    ) -> Option<&crate::data::DataDefinition> {
        use crate::types::TypeReferenceNode;

        if !self.type_reference_table.contains_type_reference(reference) {
            return None;
        }
        if let Some(application) = self.data_application_definition(reference) {
            return self
                .data_application_is_closed(reference)
                .then_some(application);
        }
        let data = match self.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => {
                self.unique_data_definition_by_symbol(*symbol)?
            }
            TypeReferenceNode::Generic {
                base_symbol,
                lifetime_arguments,
                arguments,
                ..
            } => {
                let owner = self.unique_data_definition_by_symbol(*base_symbol)?;
                if lifetime_arguments.len() != owner.lifetime_parameters.len() {
                    return None;
                }
                let arguments = self.type_reference_table.type_reference_handles(*arguments);
                if arguments.is_empty() {
                    // Regions do not specialize runtime layout. A lifetime-only
                    // application names this exact closed declaration; its
                    // borrowing relations are checked separately at each use.
                    owner
                } else {
                    if arguments.len() != self.data_type_parameters(owner).len() {
                        return None;
                    }
                    let identity = self.normalized_type_identity(reference);
                    let mut instances = self.data_definitions().iter().filter(|data| {
                        let Some(origin) = data.generic_instance else {
                            return false;
                        };
                        self.type_reference_table.contains_type_reference(origin)
                            && matches!(self.type_reference_table.type_reference(origin),
                                TypeReferenceNode::Generic { base_symbol: origin_owner, lifetime_arguments, .. }
                                    if origin_owner == base_symbol && lifetime_arguments.is_empty())
                            && self.normalized_type_identity(origin) == identity
                            && data.lifetime_parameters == owner.lifetime_parameters
                    });
                    let instance = instances.next()?;
                    if instances.next().is_some() {
                        return None;
                    }
                    self.unique_data_definition_by_symbol(instance.symbol)?
                }
            }
            _ => return None,
        };
        data.type_parameters.is_empty().then_some(data)
    }

    /// Join an authored constructor owner to its exact prepared carrier.
    /// Specialization changes member storage symbols, not the declaration the
    /// source selected. Only that carrier's retained application can establish
    /// the correspondence; another specialization or a same-named owner cannot.
    pub fn closed_data_constructor_definition(
        &self,
        reference: TypeReferenceHandle,
        authored: SymbolHandle,
    ) -> Option<&crate::data::DataDefinition> {
        let instance = self.closed_data_definition(reference)?;
        if authored == instance.symbol {
            return Some(instance);
        }
        // An occurrence-specific field view may apply regions to an already
        // specialized nominal carrier. Its generic_instance then names that
        // carrier, not the authored template. Constructor provenance belongs
        // to the nominal declaration; field types still come from this exact
        // occurrence's prepared view. Reopening the region-erased origin as a
        // closed application would demand lifetime operands it does not own.
        // Either side can retain the template: a scoped prepared field view
        // keeps it while an authored constructor may already select a closed
        // nominal instance. Join both through their retained application, not
        // only the template-to-instance direction. A sibling application must
        // still match this exact receiving type before its members correspond.
        let identity = self.normalized_type_identity(reference);
        let declaration = |symbol| {
            let nominal = self.unique_data_definition_by_symbol(symbol)?;
            let Some(origin) = nominal.generic_instance else {
                return Some(symbol);
            };
            if !self.type_reference_table.contains_type_reference(origin) {
                return None;
            }
            let crate::types::TypeReferenceNode::Generic { base_symbol, .. } =
                self.type_reference_table.type_reference(origin)
            else {
                return None;
            };
            (self.normalized_type_identity(origin) == identity).then_some(*base_symbol)
        };
        (declaration(instance.symbol)? == declaration(authored)?).then_some(instance)
    }

    /// Select a constructor child through the exact closed declaration's
    /// member correspondence. Source template symbols remain valid selectors
    /// only at the same declaration position under this retained origin.
    /// Common fields and case payloads retain separate coordinates.
    pub fn closed_data_constructor_field(
        &self,
        reference: TypeReferenceHandle,
        authored: SymbolHandle,
        retained: SymbolHandle,
    ) -> Option<&crate::data::DataField> {
        use crate::data::DataMember;
        let instance = self.closed_data_constructor_definition(reference, authored)?;
        if !retained.is_valid() || self.symbols.get(retained).kind != symbols::SymbolKind::Field {
            return None;
        }
        let parent = self.symbols.get(retained).parent;
        let source_owner = if self.symbols.get(parent).kind == symbols::SymbolKind::Variant {
            self.symbols.get(parent).parent
        } else {
            parent
        };
        self.closed_data_constructor_definition(reference, source_owner)?;
        let source = self.unique_data_definition_by_symbol(source_owner)?;
        let corresponding_field =
            |original: &crate::data::DataField, selected: &crate::data::DataField| {
                original.name == selected.name
                    && original.identity == selected.identity
                    && original.relevance == selected.relevance
                    && selected.symbol.is_valid()
                    && self
                        .type_reference_table
                        .contains_type_reference(selected.type_reference)
            };
        for (ordinal, member) in self.data_members(source).iter().enumerate() {
            match member {
                DataMember::Field(original) if original.symbol == retained => {
                    let DataMember::Field(selected) = self.data_members(instance).get(ordinal)?
                    else {
                        return None;
                    };
                    return corresponding_field(original, selected).then_some(selected);
                }
                DataMember::Variant(original_case) => {
                    let Some((field_ordinal, original)) = self
                        .data_payload_fields(original_case)
                        .iter()
                        .enumerate()
                        .find(|(_, field)| field.symbol == retained)
                    else {
                        continue;
                    };
                    let DataMember::Variant(selected_case) =
                        self.data_members(instance).get(ordinal)?
                    else {
                        return None;
                    };
                    if original_case.name != selected_case.name
                        || original_case.identity != selected_case.identity
                    {
                        return None;
                    }
                    let selected = self.data_payload_fields(selected_case).get(field_ordinal)?;
                    return corresponding_field(original, selected).then_some(selected);
                }
                _ => {}
            }
        }
        None
    }

    /// Rejoin a record member to the closed receiver's substituted field.
    /// Normalization may retain the template selection; only the matching
    /// declaration position, identity and relevance authorize its translation.
    /// Foreign and sibling-instance selections cannot be repaired by spelling.
    /// The receiver is unwrapped, and case payloads retain their separate query.
    pub fn closed_data_member_field(
        &self,
        reference: TypeReferenceHandle,
        retained: SymbolHandle,
        member_name: &str,
    ) -> Option<&crate::data::DataField> {
        let instance = self.closed_data_definition(reference)?;
        let mut fields =
            self.data_members(instance)
                .iter()
                .enumerate()
                .filter_map(|(ordinal, member)| {
                    let crate::data::DataMember::Field(field) = member else {
                        return None;
                    };
                    (field.name.as_str() == member_name).then_some((ordinal, field))
                });
        let (ordinal, field) = fields.next()?;
        if fields.next().is_some() {
            return None;
        }
        if !field.symbol.is_valid()
            || !self
                .type_reference_table
                .contains_type_reference(field.type_reference)
            || self.symbols.get(field.symbol).parent != instance.symbol
        {
            return None;
        }
        // An already resolved instance field needs no template translation.
        // Reopening an erased layout origin here loses region-bearing views
        // and rejects finite recursive applications. Only translating a
        // different retained selector owes the origin/member correspondence.
        if retained == field.symbol {
            return Some(field);
        }
        let origin = instance.generic_instance?;
        if !self.type_reference_table.contains_type_reference(origin) {
            return None;
        }
        let crate::types::TypeReferenceNode::Generic { base_symbol, .. } =
            self.type_reference_table.type_reference(origin)
        else {
            return None;
        };
        if self.closed_data_definition(origin)?.symbol != instance.symbol {
            return None;
        }
        let template = self.unique_data_definition_by_symbol(*base_symbol)?;
        let crate::data::DataMember::Field(original) = self.data_members(template).get(ordinal)?
        else {
            return None;
        };
        (field.name == original.name
            && field.identity == original.identity
            && field.relevance == original.relevance
            && original.symbol.is_valid()
            && self.symbols.get(original.symbol).parent == template.symbol
            && (!retained.is_valid() || retained == field.symbol || retained == original.symbol))
            .then_some(field)
    }

    /// Canonical identity for an exact resolved declaration that may cross a
    /// target-neutral/target-closed compiler boundary. Managed package symbols
    /// retain their package digest; toolchain symbols retain their closed
    /// toolchain owner. Local or provenance-free declarations fail closed.
    pub fn normalized_hermetic_symbol_identity(
        &self,
        symbol: SymbolHandle,
    ) -> Result<String, String> {
        if !symbol.is_valid() {
            return Err("a hermetic identity contains an unresolved declaration".to_owned());
        }
        match derived_declarations::nominal_name(
            self,
            symbol,
            &TypeIdentityContext {
                qualification: TypeIdentityQualification::PackageQualified,
                ..Default::default()
            },
            true,
        )
        .map_err(str::to_owned)?
        {
            Some(identity) => return Ok(identity),
            None => {}
        }
        let path = self.symbols.display_path(symbol, "::");
        if let Some(package) = self.symbols.symbol_package_identity(symbol) {
            let mut owner = String::with_capacity(64);
            use std::fmt::Write as _;
            for byte in package.digest() {
                let _ = write!(owner, "{byte:02x}");
            }
            return Ok(format!("package:{owner}::{path}"));
        }
        match self.symbols.symbol_source_origin(symbol) {
            Some(source::SourceOrigin::Toolchain) => Ok(format!("toolchain::{path}")),
            Some(origin) => Err(format!(
                "declaration `{path}` has non-hermetic source origin `{origin:?}`"
            )),
            None => Err(format!(
                "declaration `{path}` has no retained source/package provenance"
            )),
        }
    }

    pub fn normalized_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        self.type_identity(TypeIdentityRequest::ordinary(type_reference))
    }

    /// The retained rendering of one interned type identity: diagnostics
    /// and artifact encoding only. Identity comparisons read the handle.
    pub fn type_identity_text(&self, identity: TypeIdentityId) -> Option<String> {
        self.type_identities
            .with(|table| table.text(identity).map(str::to_owned))
    }

    /// Convenience render of the ordinary identity for boundary encoding
    /// and diagnostics: one query instead of a handle plus a lookup.
    pub fn normalized_type_identity_text(&self, type_reference: TypeReferenceHandle) -> String {
        self.type_identity_text(self.normalized_type_identity(type_reference))
            .unwrap_or_default()
    }

    /// The handle a previously interned text names, or the invalid handle
    /// when the text was never an identity of this program. Boundary readers
    /// rehydrate their `String` payload through this reverse join.
    pub fn type_identity_by_text(&self, text: &str) -> TypeIdentityId {
        self.type_identities.with(|table| table.by_text(text))
    }

    /// The ordinary named-type identity of an exact resolved nominal symbol,
    /// including owners that have no occurrence in the type-reference arena.
    /// Applied owners must normalize their complete type application instead.
    pub fn normalized_nominal_type_identity(
        &self,
        symbol: SymbolHandle,
    ) -> Option<NormalizedTypeIdentity> {
        if !symbol.is_valid() || self.symbols.display_path(symbol, "::").is_empty() {
            return None;
        }
        derived_declarations::nominal_name(self, symbol, &TypeIdentityContext::default(), false)
            .ok()?;
        if let Some(origin) = self
            .data_definition_by_symbol(symbol)
            .and_then(|data| data.generic_instance)
        {
            return Some(self.normalized_type_identity(origin));
        }
        Some(program_intern_type_identity(
            self,
            compound(
                "named",
                [identity_context::normalize_const_or_nominal_name(
                    self,
                    symbol,
                    "",
                    "name",
                    &TypeIdentityContext::default(),
                )],
            ),
        ))
    }

    /// Canonical type identity for a package graph. Every non-binder nominal
    /// carries both its stable declaration path and its exact source owner:
    /// the managed package-key digest, the toolchain marker, or an explicit
    /// unresolved marker. Ordinary type identity remains local to one
    /// compilation but also distinguishes toolchain and authored namespaces.
    pub fn package_qualified_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        self.type_identity(TypeIdentityRequest::package_qualified(type_reference))
    }

    /// The package-qualified identity rendered for boundary encoding.
    pub fn package_qualified_type_identity_text(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> String {
        self.type_identity_text(self.package_qualified_type_identity(type_reference))
            .unwrap_or_default()
    }

    /// The one binder- and substitution-aware identity entry. A binder symbol
    /// is replaced before serialization, so renaming the source binder cannot
    /// change the normalized contract while concrete declaration paths remain
    /// fully qualified; substitutions replace exact type-parameter symbols
    /// with concrete type references first. Binder substitutions are applied
    /// before owner qualification, preserving alpha-normalization without
    /// falsely assigning a package owner to a local telescope variable.
    /// The request identity rendered for boundary encoding or diagnostics.
    pub fn type_identity_request_text(&self, request: TypeIdentityRequest<'_>) -> String {
        self.type_identity_text(self.type_identity(request))
            .unwrap_or_default()
    }

    /// Compare only an array's index under exact declaration substitutions.
    /// Carrier refinement remains a separate judgment over its element type.
    pub fn fixed_array_length_identity(
        &self,
        length: &crate::types::FixedArrayLength,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> Option<NormalizedTypeIdentity> {
        if let crate::types::FixedArrayLength::Expression(reference) = length
            && (!self
                .type_reference_table
                .contains_type_reference(*reference)
                || !matches!(
                    self.type_reference_table.type_reference(*reference),
                    crate::types::TypeReferenceNode::ConstExpression(_)
                ))
        {
            return None;
        }
        let invalid = Cell::new(false);
        let identity = constraint_identity::normalize_array_length(
            self,
            length,
            &TypeIdentityContext {
                binders,
                substitutions,
                invalid_index_argument: Some(&invalid),
                ..Default::default()
            },
        );
        (!invalid.get()).then(|| program_intern_type_identity(self, identity))
    }

    pub fn type_identity(&self, request: TypeIdentityRequest<'_>) -> NormalizedTypeIdentity {
        self.type_identity_with_substitution_semantics(
            request,
            SubstitutionSemantics::Transitive,
            &[],
        )
    }

    /// Identity for a retained storage plan, never application compatibility.
    /// Only declaration-identified value positions proven independent of every
    /// member's complete type may share storage across runtime subjects.
    pub fn storage_type_identity(
        &self,
        request: TypeIdentityRequest<'_>,
    ) -> NormalizedTypeIdentity {
        program_intern_type_identity(
            self,
            normalize_type_reference(
                self,
                request.type_reference,
                &TypeIdentityContext {
                    storage_value_indices: true,
                    binders: request.binders,
                    substitutions: request.substitutions,
                    qualification: request.qualification,
                    ..TypeIdentityContext::default()
                },
            ),
        )
    }

    /// Substitute an application tuple simultaneously. Each replacement is
    /// normalized in its original scope, without applying the tuple again.
    /// This preserves recursive permutations such as `f<A, B>` calling
    /// `f<B, A>` and leaves nested references and qualifications intact.
    pub fn type_identity_with_simultaneous_substitutions(
        &self,
        request: TypeIdentityRequest<'_>,
    ) -> NormalizedTypeIdentity {
        self.type_identity_with_substitution_semantics(
            request,
            SubstitutionSemantics::Simultaneous,
            &[],
        )
    }

    /// Normalize a chain of simultaneous telescopes, ordered outermost first.
    /// Each start identifies the first replacement belonging to that scope;
    /// its operands see only the preceding scopes, never their own siblings.
    pub fn scoped_type_identity(
        &self,
        request: TypeIdentityRequest<'_>,
        scope_starts: &[usize],
    ) -> NormalizedTypeIdentity {
        self.type_identity_with_substitution_semantics(
            request,
            SubstitutionSemantics::Transitive,
            scope_starts,
        )
    }

    /// The ordinary scoped identity only when its entire normalization path
    /// encounters no explicit or elided region. This does not encode regions
    /// portably: it refuses to treat their erased carrier as an exact applied
    /// identity. The shared normalizer follows substitutions and materialized
    /// nominal origins, so compound arguments cannot hide an erased region.
    pub fn region_free_scoped_type_identity(
        &self,
        request: TypeIdentityRequest<'_>,
        scope_starts: &[usize],
    ) -> Option<NormalizedTypeIdentity> {
        if !self
            .type_reference_table
            .contains_type_reference(request.type_reference)
        {
            return None;
        }
        let encountered_regions = Cell::new(false);
        let identity = normalize_type_reference(
            self,
            request.type_reference,
            &TypeIdentityContext {
                binders: request.binders,
                substitutions: request.substitutions,
                substitution_scope_starts: scope_starts,
                encountered_regions: Some(&encountered_regions),
                qualification: request.qualification,
                ..TypeIdentityContext::default()
            },
        );
        (!encountered_regions.get()).then(|| program_intern_type_identity(self, identity))
    }

    fn type_identity_with_substitution_semantics(
        &self,
        request: TypeIdentityRequest<'_>,
        substitution_semantics: SubstitutionSemantics,
        substitution_scope_starts: &[usize],
    ) -> NormalizedTypeIdentity {
        program_intern_type_identity(
            self,
            normalize_type_reference(
                self,
                request.type_reference,
                &TypeIdentityContext {
                    binders: request.binders,
                    substitutions: request.substitutions,
                    substitution_semantics,
                    substitution_scope_starts,
                    active_const_substitutions: &[],
                    exact_toolchain_sources: &[],
                    missing_exact_nominal_owner: None,
                    qualification: request.qualification,
                    ..TypeIdentityContext::default()
                },
            ),
        )
    }

    /// Package-review identity that replaces the generic toolchain marker
    /// with the exact compiler-validated source identity for every source-
    /// backed toolchain nominal. Every other non-binder nominal must have a
    /// managed package owner; unresolved ownership fails closed. `SourceId`
    /// remains an internal join key and never enters the normalized output.
    pub fn exact_owner_type_identity(
        &self,
        request: ExactOwnerTypeIdentityRequest<'_>,
    ) -> Option<NormalizedTypeIdentity> {
        let missing_exact_nominal_owner = Cell::new(false);
        let identity = program_intern_type_identity(
            self,
            normalize_type_reference(
                self,
                request.type_reference,
                &TypeIdentityContext {
                    binders: request.binders,
                    substitutions: request.substitutions,
                    substitution_semantics: SubstitutionSemantics::Transitive,
                    active_const_substitutions: &[],
                    exact_toolchain_sources: request.exact_toolchain_sources,
                    missing_exact_nominal_owner: Some(&missing_exact_nominal_owner),
                    qualification: TypeIdentityQualification::PackageQualified,
                    ..TypeIdentityContext::default()
                },
            ),
        );
        (!missing_exact_nominal_owner.get()).then_some(identity)
    }

    /// Exact-owner identity for one already-resolved nominal declaration.
    /// Compiler builtins use their closed semantic atom; authored nominals
    /// require a managed package owner or exact toolchain source.
    pub fn exact_owner_nominal_identity(
        &self,
        symbol: SymbolHandle,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Option<NormalizedTypeIdentity> {
        if !symbol.is_valid() {
            return None;
        }
        let path = self.symbols.display_path(symbol, "::");
        if path.is_empty() {
            return None;
        }
        let missing_exact_nominal_owner = Cell::new(false);
        let identity = program_intern_type_identity(
            self,
            TypeIdentityContext {
                binders: &[],
                substitutions: &[],
                substitution_semantics: SubstitutionSemantics::Transitive,
                active_const_substitutions: &[],
                exact_toolchain_sources,
                missing_exact_nominal_owner: Some(&missing_exact_nominal_owner),
                qualification: TypeIdentityQualification::PackageQualified,
                ..TypeIdentityContext::default()
            }
            .qualify_non_binder_name(self, symbol, path),
        );
        (!missing_exact_nominal_owner.get()).then_some(identity)
    }

    /// Normalize the dispatch-bearing domain set on the outer result value.
    /// Reference and nested constrained shells are transparent; domains inside
    /// an aggregate element or generic argument belong to that nested value and
    /// do not select the enclosing result overload.
    pub fn normalized_result_dispatch_set(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedResultDispatchSet {
        let mut terms = Vec::new();
        collect_result_dispatch_terms(self, type_reference, &mut terms, &mut Vec::new());
        terms.sort();
        terms.dedup();
        NormalizedResultDispatchSet { terms }
    }

    /// The derived parts of one top-level named machine overload identity. A
    /// machine's first state is its callable entry signature; explicit sibling
    /// states do not create additional top-level overloads. Consumers take the
    /// interned handle from `machine_callable_identity`; this derivation runs
    /// once per declaration per program revision.
    fn machine_callable_identity_parts(
        &self,
        machine: &crate::machine::Machine,
    ) -> Option<callable_identity::CallableIdentityParts> {
        let entry = self.machine_states(machine).first()?;
        // Module ownership lives on the exact symbol, not the authored
        // machine spelling. Module-free and source-free semantic fixtures
        // retain their existing declaration path.
        let module = self.symbols.symbol_module(machine.symbol);
        // A target sibling's symbol spelling carries its target suffix to
        // keep physical slots distinct. That suffix is not part of the
        // authored overload: every body in the family has the same path,
        // including when the declaration belongs to a module.
        let module_path = module.is_valid().then(|| {
            format!(
                "{}::{}",
                self.symbols.display_path(module, "::"),
                machine.name
            )
        });
        Some(self.named_callable_identity_parts(
            module_path.as_deref().unwrap_or(machine.name.as_str()),
            machine.symbol,
            self.machine_type_parameters(machine),
            self.state_parameters(entry),
            entry.return_type,
        ))
    }

    /// The complete entry result type, normalized with the machine's generic
    /// binders. Callable overload identity contains only result-domain dispatch;
    /// bodies implementing one contract must also agree on the actual result.
    pub fn machine_entry_result_type_identity(
        &self,
        machine: &crate::machine::Machine,
    ) -> Option<NormalizedTypeIdentity> {
        let entry = self.machine_states(machine).first()?;
        let binders = callable_type_binders(machine.symbol, self.machine_type_parameters(machine));
        Some(self.type_identity(TypeIdentityRequest {
            binders: &binders,
            ..TypeIdentityRequest::ordinary(entry.return_type)
        }))
    }

    /// Export a nominal data declaration's path without losing its module.
    /// Exact symbol and package custody remain separate from this projection.
    pub fn data_declaration_path(&self, definition: &crate::data::DataDefinition) -> String {
        if self.symbols.symbol_module(definition.symbol).is_valid() {
            self.symbols.display_path(definition.symbol, "::")
        } else {
            definition.name.as_str().to_owned()
        }
    }

    /// Export the attached owner's path using the same rule as data declarations.
    /// Source-free semantic trees retain complete paths in their authored names;
    /// a live module must never fall back to a leaf spelling.
    pub fn attached_data_path(&self, machine: &crate::machine::Machine) -> Option<String> {
        let name = machine.attached_data.as_ref()?;
        Some(
            if self
                .symbols
                .symbol_module(machine.attached_data_symbol)
                .is_valid()
            {
                self.symbols
                    .display_path(machine.attached_data_symbol, "::")
            } else {
                name.as_str().to_owned()
            },
        )
    }

    /// Export one trait's declaration path, retaining module ownership.
    /// Module-free and source-free semantic trees carry their complete path
    /// directly in the declaration, as for named machine identities. Package
    /// identity and exact symbol custody remain separate from this projection.
    pub fn trait_declaration_path(
        &self,
        trait_definition: &crate::trait_definition::TraitDefinition,
    ) -> String {
        if self
            .symbols
            .symbol_module(trait_definition.symbol)
            .is_valid()
        {
            self.symbols.display_path(trait_definition.symbol, "::")
        } else {
            trait_definition.name.as_str().to_owned()
        }
    }

    /// The derived parts of one trait machine requirement overload identity.
    fn trait_requirement_callable_identity_parts(
        &self,
        trait_definition: &crate::trait_definition::TraitDefinition,
        requirement: &crate::signature::StateSignature,
    ) -> callable_identity::CallableIdentityParts {
        let mut type_parameters = self.trait_type_parameters(trait_definition).to_vec();
        type_parameters.extend_from_slice(self.state_signature_type_parameters(requirement));
        // As with concrete machines, a requirement belongs to its full module
        // path. Sibling modules may declare identically shaped boundary traits.
        let mut identity = self.named_callable_identity_parts(
            &format!(
                "{}::{}",
                self.trait_declaration_path(trait_definition),
                requirement.name
            ),
            trait_definition.symbol,
            &type_parameters,
            self.state_signature_parameters(requirement),
            requirement.return_type,
        );
        if let Some(application) = self.closed_requirement_application(requirement.symbol) {
            // The authored overload and complete tuple own instance identity,
            // including unused binders. An equal parameter ABI is insufficient.
            let template =
                self.trait_machine_signature_by_symbol(trait_definition, application.template);
            let template_identity = template
                .map(|template| {
                    self.trait_requirement_callable_identity_text(trait_definition, template)
                })
                .unwrap_or_default();
            // Preparation issued these complete portable keys before creating
            // the instance. Publication cannot reconstruct them from erased
            // carrier arguments or recapture them in the receiving trait.
            let outer = self
                .type_identity_text(application.portable_trait_application_identity)
                .unwrap_or_default();
            let local = application
                .portable_local_arguments
                .iter()
                .map(|identity| self.type_identity_text(*identity).unwrap_or_default())
                .collect::<Vec<_>>();
            identity.parameters = compound(
                "closed-requirement",
                [
                    atom("template", &template_identity),
                    atom("trait-application", &outer),
                    compound("local-arguments", local),
                    atom("parameters", &identity.parameters),
                ],
            );
        }
        identity
    }

    /// The derived parts of one explicitly named operator requirement
    /// identity. Unspelled boundary operators participate in the same
    /// result-domain lookup as other named requirements; fixed spellings
    /// remain operand-directed and use this only for declaration diagnostics.
    fn operator_callable_identity_parts(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> callable_identity::CallableIdentityParts {
        let path = self.operator_path_string(operator.name);
        let flags = self
            .operator_parameters(operator)
            .iter()
            .map(|parameter| {
                format!(
                    "mutable={};const={}",
                    parameter.is_mutable, parameter.is_const
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        callable_identity::CallableIdentityParts {
            path,
            // Operator operand identity already normalizes generic binders by
            // first structural occurrence, including declarations whose type
            // parameter lists are reordered. Retain parameter modes beside it.
            parameters: format!(
                "operands({});flags({flags})",
                crate::operator::operator_operand_signature(self, operator)
            ),
            result_dispatch: self.normalized_result_dispatch_set(operator.return_type),
        }
    }

    fn named_callable_identity_parts(
        &self,
        path: &str,
        owner_symbol: SymbolHandle,
        type_parameters: &[crate::data::TypeParameter],
        parameters: &[crate::signature::StateParameter],
        return_type: TypeReferenceHandle,
    ) -> callable_identity::CallableIdentityParts {
        let binders = callable_type_binders(owner_symbol, type_parameters);
        let parameters = parameters
            .iter()
            .map(|parameter| {
                compound(
                    "parameter",
                    [
                        atom("mutable", if parameter.is_mutable { "yes" } else { "no" }),
                        atom("const", if parameter.is_const { "yes" } else { "no" }),
                        self.type_identity_request_text(TypeIdentityRequest {
                            binders: &binders,
                            ..TypeIdentityRequest::ordinary(parameter.type_reference)
                        }),
                    ],
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        callable_identity::CallableIdentityParts {
            path: path.to_owned(),
            parameters,
            result_dispatch: self.normalized_result_dispatch_set(return_type),
        }
    }
}

fn callable_type_binders(
    owner: SymbolHandle,
    parameters: &[crate::data::TypeParameter],
) -> Vec<(SymbolHandle, String)> {
    let mut binders = Vec::with_capacity(parameters.len() + 1);
    if owner.is_valid() {
        binders.push((owner, "$Self".to_owned()));
    }
    binders.extend(
        parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.symbol.is_valid())
            .map(|(index, parameter)| (parameter.symbol, format!("$T{index}"))),
    );
    binders
}

fn intern_boxed_text<I: Copy, S: std::hash::BuildHasher>(
    text: &str,
    texts: &mut Vec<Box<str>>,
    slots: &mut std::collections::HashMap<Box<str>, I, S>,
    mint: impl Fn(usize) -> I,
) -> I {
    if let Some(interned) = slots.get(text) {
        return *interned;
    }
    let interned = mint(texts.len());
    let text: Box<str> = text.into();
    texts.push(text.clone());
    slots.insert(text, interned);
    interned
}

fn intern_owned<K, I: Copy, S: std::hash::BuildHasher>(
    value: K,
    values: &mut Vec<K>,
    slots: &mut std::collections::HashMap<K, I, S>,
    mint: impl Fn(usize) -> I,
) -> I
where
    K: Eq + std::hash::Hash + Clone,
{
    if let Some(interned) = slots.get(&value) {
        return *interned;
    }
    let interned = mint(values.len());
    values.push(value.clone());
    slots.insert(value, interned);
    interned
}
