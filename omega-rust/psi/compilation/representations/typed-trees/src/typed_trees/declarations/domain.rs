use crate::TypedTrees;
use crate::name::Identifier;
use crate::type_identity::{IndexIdentityArgument, normalized_index_identity};
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub subject_name: Identifier,
    pub subject_symbol: SymbolHandle,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub target_type: TypeReferenceHandle,
    pub index_arguments: Vec<TypeReferenceHandle>,
    pub is_public: bool,
    /// Selected machine identity and its full declaration-site application.
    pub cleanup_selection: Option<CleanupSelection>,
    /// Authored transparent alias theory. Semantic consumers expand this
    /// record before normalization rather than treating the alias as evidence.
    pub alias: Option<DomainAliasDefinition>,
    /// Explicit closed classification copied from the resolved declaration.
    pub classification: Option<language_semantics::DomainClassification>,
    /// Explicit predicate-body presence copied from the resolved theory.
    pub predicate_body: language_semantics::DomainPredicateBody,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub semantic_clause_token_count: usize,
    /// STR4 checked plans, slice 1: the normalized semantic identity from
    /// the program's SemanticDomainTable (populated ONCE at
    /// syntax->resolved, copied downstream; NULL only pre-lowering).
    pub semantic_id: language_semantics::SemanticDomainId,
    /// Role-keyed semantic contributions copied from the resolved declaration
    /// without re-derivation.
    pub semantic_roles: language_semantics::DomainSemanticRoles,
    /// Normalized authored introduction relationships copied from the resolved
    /// domain theory without re-derivation.
    pub establishment_routes: Vec<language_semantics::DomainEstablishmentRoute>,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            subject_name: Identifier::default(),
            subject_symbol: SymbolHandle::invalid(),
            type_parameters: HandleSpan::empty(),
            target_type: TypeReferenceHandle::invalid(),
            index_arguments: Vec::new(),
            is_public: false,
            cleanup_selection: None,
            alias: None,
            classification: None,
            predicate_body: language_semantics::DomainPredicateBody::Bodyless,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 0,
            semantic_id: language_semantics::SemanticDomainId::NULL,
            semantic_roles: language_semantics::DomainSemanticRoles::default(),
            establishment_routes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainAliasDefinition {
    pub constituents: Vec<DomainAliasConstituent>,
}

#[derive(Debug, Clone, Default, Copy, PartialEq, Eq)]
pub struct DomainAliasConstituent {
    pub domain: HandleSpan<Identifier>,
    pub domain_symbol: SymbolHandle,
}

/// Deterministic identity of one closed indexed-domain instance. Both initial
/// typed lowering and later generic-machine specialization call this helper so
/// replacing a direct const binder with its canonical value cannot mint a
/// second spelling-dependent identity for the same instance.
pub fn indexed_domain_instance_name(
    program: &TypedTrees,
    domain: &DomainDefinition,
    parameters: &[crate::data::TypeParameter],
    arguments: &[TypeReferenceHandle],
) -> Result<String, Diagnostic> {
    indexed_domain_instance_name_with_substitutions(program, domain, parameters, arguments, &[])
}

/// Identify the complete instance under one simultaneous call telescope.
/// Replacements remain in the caller's scope, including inside compound types
/// and computed const indices. This query creates no substituted syntax or
/// membership evidence and preserves the ordinary role-sensitive encoding.
pub fn indexed_domain_instance_name_with_substitutions(
    program: &TypedTrees,
    domain: &DomainDefinition,
    parameters: &[crate::data::TypeParameter],
    arguments: &[TypeReferenceHandle],
    substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
) -> Result<String, Diagnostic> {
    if parameters.len() != arguments.len() {
        return Err(Diagnostic::error(
            "indexed-domain argument arity does not match its declaration",
        ));
    }
    if arguments.is_empty() {
        return Ok(program
            .semantic_domains
            .name(domain.semantic_id)
            .unwrap_or(domain.name.as_str())
            .to_owned());
    }
    let mut identities = Vec::with_capacity(arguments.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        match parameter.kind {
            crate::data::TypeParameterKind::Type => identities.push(format!(
                "type:{}",
                substituted_type_identity_text(program, *argument, substitutions)?
            )),
            crate::data::TypeParameterKind::Const { type_reference } => {
                let expected = const_index_type_name(program, type_reference)?;
                identities.push(closed_domain_argument_identity(
                    program,
                    *argument,
                    &expected,
                    substitutions,
                )?);
            }
            _ => {
                return Err(Diagnostic::error(format!(
                    "domain family `{}` has an ineligible index binder `{}`",
                    domain.name, parameter.name
                )));
            }
        }
    }
    let base = program
        .semantic_domains
        .name(domain.semantic_id)
        .unwrap_or(domain.name.as_str());
    Ok(format!("{base}<{}>", identities.join(",")))
}

fn substituted_type_identity_text(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
) -> Result<String, Diagnostic> {
    normalized_index_identity(
        program,
        IndexIdentityArgument::TypeArgument(argument),
        substitutions,
    )
    .ok_or_else(|| Diagnostic::error("indexed-domain argument has no valid substitution identity"))
}

/// Whether the declaration's first type parameter is its generic carrier.
/// Fixed-carrier indexed families instead use every parameter as an invariant
/// index and retain their concrete `target_type`.
pub fn has_generic_carrier(program: &TypedTrees, domain: &DomainDefinition) -> bool {
    let Some(parameter) = program.domain_type_parameters(domain).first() else {
        return false;
    };
    if !matches!(parameter.kind, crate::data::TypeParameterKind::Type) {
        return false;
    }
    let crate::types::TypeReferenceNode::Named { symbol, name } = program
        .type_reference_table
        .type_reference(domain.target_type)
    else {
        return false;
    };
    *symbol == parameter.symbol || name == &parameter.name
}

pub fn index_parameters<'program>(
    program: &'program TypedTrees,
    domain: &DomainDefinition,
) -> &'program [crate::data::TypeParameter] {
    let parameters = program.domain_type_parameters(domain);
    if has_generic_carrier(program, domain) {
        &parameters[1..]
    } else {
        parameters
    }
}

/// The declaration carrying `symbol`, resolving through the program-owned
/// domain index (`TypedTrees::domain_position_by_symbol`) instead of
/// rescanning `domain_definitions()` per query.
pub fn domain_by_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<&DomainDefinition> {
    let position = program.domain_position_by_symbol(symbol)?;
    program.domain_definitions().get(position)
}

fn const_index_type_name(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<String, Diagnostic> {
    use crate::types::{FixedArrayLength, TypeReferenceNode};
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => Ok(name.as_str().to_owned()),
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(length),
        } => Ok(format!(
            "[{}; {length}]",
            const_index_type_name(program, *element_type)?
        )),
        TypeReferenceNode::Constrained { base_type, .. } => {
            const_index_type_name(program, *base_type)
        }
        TypeReferenceNode::Unit => Ok("()".to_owned()),
        _ => Err(Diagnostic::error(
            "indexed-domain const parameter types must have canonical structural identity",
        )),
    }
}

fn closed_domain_argument_identity(
    program: &TypedTrees,
    argument: TypeReferenceHandle,
    expected: &str,
    substitutions: &[(SymbolHandle, IndexIdentityArgument<'_>)],
) -> Result<String, Diagnostic> {
    let crate::types::TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        if matches!(
            program.type_reference_table.type_reference(argument),
            crate::types::TypeReferenceNode::ConstExpression(_)
        ) {
            // PDI3 retains an open computed index until exact operation and
            // algebra selection. The same structural identity works before
            // selection and incorporates that semantic authority afterward,
            // when the instance identities are refreshed a second time.
            return Ok(format!(
                "expression:{expected}:{}",
                substituted_type_identity_text(program, argument, substitutions)?
            ));
        }
        return Err(Diagnostic::error(
            "indexed-domain arguments must be canonical const values, direct const binders, or supported open const expressions",
        ));
    };
    if let Some((_, replacement)) = substitutions.iter().find(|(binder, _)| binder == symbol) {
        // A replacement belongs to the caller, not this telescope. In
        // particular, swapping two binders must not substitute either twice.
        return match replacement {
            IndexIdentityArgument::TypeReference(reference)
            | IndexIdentityArgument::TypeArgument(reference) => {
                closed_domain_argument_identity(program, *reference, expected, &[])
            }
            IndexIdentityArgument::Subject(subject) if subject.is_valid() => Ok(format!(
                "binder:{}:{expected}",
                program.symbols.display_path(*subject, "::")
            )),
            _ => Err(Diagnostic::error(
                "indexed-domain const argument requires a type reference or exact subject",
            )),
        };
    }
    if let Some(value) =
        language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
    {
        if value.type_name != expected {
            return Err(Diagnostic::error(format!(
                "indexed-domain argument has canonical type `{}`, expected `{expected}`",
                value.type_name
            )));
        }
        return Ok(format!(
            "const:{}:{}:{}",
            value.type_name,
            value.encoding.len(),
            value.encoding
        ));
    }
    if name.as_str().parse::<i128>().is_ok() {
        return Ok(format!("integer:{expected}:{}", name.as_str()));
    }
    // A direct generic const binder remains open only until ordinary machine
    // specialization. It is not PDI3's computed expression surface.
    Ok(format!(
        "binder:{}:{}",
        if symbol.is_valid() {
            program.symbols.display_path(*symbol, "::")
        } else {
            name.as_str().to_owned()
        },
        expected
    ))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofFact {
    Expression(crate::expression::ExpressionHandle),
    Membership(ProofMembershipFact),
    Proposition(crate::proposition::PropositionApplication),
}

impl ProofFact {
    /// The expression handle, if this fact observes one.
    pub fn as_expression(&self) -> Option<&crate::expression::ExpressionHandle> {
        let ProofFact::Expression(expression) = self else {
            return None;
        };
        Some(expression)
    }

    /// The membership fact, if this fact records one.
    pub fn as_membership(&self) -> Option<&ProofMembershipFact> {
        let ProofFact::Membership(membership) = self else {
            return None;
        };
        Some(membership)
    }

    /// The proposition application, if this fact asserts one.
    pub fn as_proposition(&self) -> Option<&crate::proposition::PropositionApplication> {
        let ProofFact::Proposition(proposition) = self else {
            return None;
        };
        Some(proposition)
    }
}

impl Default for ProofFact {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProofMembershipFact {
    pub value: crate::expression::ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
    pub domain_symbol: SymbolHandle,
    /// Exact source instance arguments, retained for ordinary specialization.
    /// The semantic identity is refreshed after their substitution; a nonzero
    /// identity alone does not establish that these arguments are closed.
    pub domain_arguments: HandleSpan<TypeReferenceHandle>,
    pub semantic_domain: language_semantics::SemanticDomainId,
    pub authored_domain_selection:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
}

impl Default for ProofMembershipFact {
    fn default() -> Self {
        Self {
            value: crate::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::invalid(),
            domain_arguments: HandleSpan::empty(),
            semantic_domain: language_semantics::SemanticDomainId::NULL,
            authored_domain_selection: None,
        }
    }
}

/// Whether a legacy declaration-only proof reader can consume this theory.
/// Indexed membership requires exact arguments and cannot be projected to a
/// family symbol, including through an alias or an intermediate membership.
/// Carrier-only generic parameters remain eligible. This is a fail-closed
/// proof-reader fence, not an identity or metadata-normalization judgment.
pub fn has_symbol_only_proof(program: &TypedTrees, domain_symbol: SymbolHandle) -> bool {
    domain_by_symbol(program, domain_symbol)
        .is_some_and(|domain| has_bound_symbol_only_proof(program, domain))
}

/// Complete applied theories have no free index slots. Read the selected
/// definition itself; falling back to its generic source would lose that tuple.
pub fn has_bound_symbol_only_proof(program: &TypedTrees, domain: &DomainDefinition) -> bool {
    fn visit(
        program: &TypedTrees,
        domain: &DomainDefinition,
        visited: &mut Vec<(SymbolHandle, language_semantics::SemanticDomainId)>,
    ) -> bool {
        let symbol = (domain.symbol, domain.semantic_id);
        if !index_parameters(program, domain).is_empty() || !domain.index_arguments.is_empty() {
            return false;
        }
        if visited.contains(&symbol) {
            return true;
        }
        visited.push(symbol);
        domain.alias.as_ref().is_none_or(|alias| {
            alias.constituents.iter().all(|constituent| {
                domain_by_symbol(program, constituent.domain_symbol)
                    .is_some_and(|domain| visit(program, domain, visited))
            })
        }) && program.proof_facts(domain).iter().all(|fact| match fact {
            ProofFact::Membership(membership) => program
                .domain_definition_for_instance(
                    membership.domain_symbol,
                    membership.semantic_domain,
                )
                .is_some_and(|domain| visit(program, domain, visited)),
            _ => true,
        })
    }
    visit(program, domain, &mut Vec::new())
}

/// Matches the declaration-owned subject, independently of its authored name.
pub fn domain_subject_reference(
    program: &TypedTrees,
    domain: &DomainDefinition,
    expression: crate::expression::ExpressionHandle,
) -> bool {
    if !domain.subject_symbol.is_valid()
        || !program.expression_table.expression_is_valid(expression)
    {
        return false;
    }
    let subject = program.symbols.get(domain.subject_symbol);
    let applied_subject = program
        .applied_domain_applications
        .iter()
        .any(|application| {
            application.declaration == domain.symbol
                && application.theory.semantic_id == domain.semantic_id
                && application.theory.subject_symbol == domain.subject_symbol
                && program
                    .domain_definition(domain.symbol)
                    .is_some_and(|declared| {
                        program.symbols.authored_origin(domain.subject_symbol)
                            == declared.subject_symbol
                    })
        });
    if subject.kind != symbols::SymbolKind::Parameter
        || (subject.parent != domain.symbol && !applied_subject)
    {
        return false;
    }
    let crate::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(expression)
    else {
        return false;
    };
    path.symbol == domain.subject_symbol
        && path.head_symbol == domain.subject_symbol
        && path.members.count() == 1
        && program
            .expression_table
            .name_path_member_symbols(path.member_symbols)
            == [domain.subject_symbol]
}

/// The subject-membership propositions in a domain's predicate list: the implied
/// parent a refinement-chain declaration (`domain A::B::C`) desugars to, and
/// any authored subject memberships. A membership in the domain entails each of
/// these, so every reader of the domain's theory walks them the same way.
pub fn subject_membership_facts<'a>(
    program: &'a TypedTrees,
    domain: &'a DomainDefinition,
) -> impl Iterator<Item = &'a ProofMembershipFact> + 'a {
    program
        .proof_facts(domain)
        .iter()
        .filter_map(|fact| match fact {
            ProofFact::Membership(membership) => {
                domain_subject_reference(program, domain, membership.value).then_some(membership)
            }
            _ => None,
        })
}

/// Whether one declared domain implies another by normalized semantic identity
/// or by an explicit domain-membership chain.
///
/// This relation belongs with the normalized typed domain graph rather than
/// with any one checker. Capacity-specialized declarations keep distinct
/// carrier-specific symbols so operator lookup can still select the declaration
/// for `[u8; 8]` versus `[u8; 16]`; their shared `semantic_id` is the proof
/// identity. Validation separately requires declarations sharing that identity
/// to have equal predicate bodies, semantic roles, and normalized fact sets.
pub fn declared_domain_implies(
    program: &TypedTrees,
    source_domain: SymbolHandle,
    target_domain: SymbolHandle,
) -> bool {
    fn inner(
        program: &TypedTrees,
        source_domain: SymbolHandle,
        target_domain: SymbolHandle,
        visited: &mut Vec<SymbolHandle>,
    ) -> bool {
        if !source_domain.is_valid() || !target_domain.is_valid() {
            return false;
        }
        if source_domain == target_domain {
            return true;
        }
        if visited.contains(&source_domain) {
            return false;
        }
        visited.push(source_domain);

        let Some(source) = domain_by_symbol(program, source_domain) else {
            return false;
        };
        let Some(target) = domain_by_symbol(program, target_domain) else {
            return false;
        };
        if source.semantic_id.is_valid() && source.semantic_id == target.semantic_id {
            return true;
        }

        if source.alias.as_ref().is_some_and(|alias| {
            alias.constituents.iter().any(|constituent| {
                inner(program, constituent.domain_symbol, target_domain, visited)
            })
        }) {
            return true;
        }

        program.proof_facts(source).iter().any(|fact| match fact {
            ProofFact::Membership(membership) => {
                inner(program, membership.domain_symbol, target_domain, visited)
            }
            ProofFact::Expression(_) => false,
            ProofFact::Proposition(_) => false,
        })
    }

    inner(program, source_domain, target_domain, &mut Vec::new())
}

/// Whether a type reference statically selects `domain_symbol`'s semantic
/// facet — the operand-side half of chapter 8's rule that declaring a domain
/// operator does not replace the builtin meaning. A domain-owned operator
/// applies only where an operand carries that domain's denotation, so every
/// reader that asks which declaration a spelled use means must ask this the
/// same way: type-directed, with no route to flow facts.
pub fn type_selects_semantic_domain(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    domain_symbol: SymbolHandle,
) -> bool {
    match program.type_reference_table.type_reference(type_reference) {
        crate::types::TypeReferenceNode::Reference { referee, .. } => {
            type_selects_semantic_domain(program, *referee, domain_symbol)
        }
        crate::types::TypeReferenceNode::Constrained { constraints, .. } => program
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .any(|constraint| match constraint {
                crate::types::TypeConstraintNode::Domain(domain) => {
                    domain.symbol == domain_symbol
                        && domain.semantic_roles.denotation_dimension.is_some()
                }
                _ => false,
            }),
        _ => false,
    }
}

/// One owner-authored cleanup interface. The canonical application retains its
/// supplying declaration, static tuple and bounds; executable retargeting never
/// replaces that source identity. The exact subject prevents one shared helper
/// from borrowing the first carrier that happened to select it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CleanupSelection {
    pub application: crate::static_applications::StaticMachineApplicationHandle,
    pub authorization:
        Option<language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId>,
    pub subject: TypeReferenceHandle,
    pub executable: SymbolHandle,
}

/// A complete explicit index telescope applied to one immutable authored theory.
/// Original constraints retain their argument pack and ordinary formation checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedDomainApplication {
    pub declaration: SymbolHandle,
    pub arguments: Vec<TypeReferenceHandle>,
    pub theory: DomainDefinition,
}

impl TypedTrees {
    pub fn domain_definition_for_instance(
        &self,
        symbol: SymbolHandle,
        identity: language_semantics::SemanticDomainId,
    ) -> Option<&DomainDefinition> {
        let declared = self.domain_definition(symbol)?;
        if declared.semantic_id == identity {
            return Some(declared);
        }
        self.applied_domain_applications
            .iter()
            .find(|application| {
                application.declaration == symbol && application.theory.semantic_id == identity
            })
            .map(|application| &application.theory)
    }
}
