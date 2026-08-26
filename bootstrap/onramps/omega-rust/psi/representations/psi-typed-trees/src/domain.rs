use crate::TypedTrees;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainDefinition {
    pub symbol: SymbolHandle,
    pub name: Identifier,
    pub type_parameters: HandleSpan<crate::data::TypeParameter>,
    pub target_type: TypeReferenceHandle,
    pub index_arguments: Vec<TypeReferenceHandle>,
    pub is_public: bool,
    /// Authored transparent alias theory. Semantic consumers expand this
    /// record before normalization rather than treating the alias as evidence.
    pub alias: Option<DomainAliasDefinition>,
    /// Explicit closed classification copied from the resolved declaration.
    pub classification: Option<psi_language_semantics::DomainClassification>,
    /// Explicit predicate-body presence copied from the resolved theory.
    pub predicate_body: psi_language_semantics::DomainPredicateBody,
    pub facts: HandleSpan<ProofFact>,
    pub operators: HandleSpan<crate::operator::OperatorDefinition>,
    pub semantic_clause_token_count: usize,
    /// STR4 checked plans, slice 1: the normalized semantic identity from
    /// the program's SemanticDomainTable (populated ONCE at
    /// syntax->resolved, copied downstream; NULL only pre-lowering).
    pub semantic_id: psi_language_semantics::SemanticDomainId,
    /// Role-keyed semantic contributions copied from the resolved declaration
    /// without re-derivation.
    pub semantic_roles: psi_language_semantics::DomainSemanticRoles,
    /// Normalized authored introduction relationships copied from the resolved
    /// domain theory without re-derivation.
    pub establishment_routes: Vec<psi_language_semantics::DomainEstablishmentRoute>,
}

impl Default for DomainDefinition {
    fn default() -> Self {
        Self {
            symbol: SymbolHandle::invalid(),
            name: Identifier::default(),
            type_parameters: HandleSpan::empty(),
            target_type: TypeReferenceHandle::invalid(),
            index_arguments: Vec::new(),
            is_public: false,
            alias: None,
            classification: None,
            predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
            facts: HandleSpan::empty(),
            operators: HandleSpan::empty(),
            semantic_clause_token_count: 0,
            semantic_id: psi_language_semantics::SemanticDomainId::NULL,
            semantic_roles: psi_language_semantics::DomainSemanticRoles::default(),
            establishment_routes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DomainAliasDefinition {
    pub constituents: Vec<DomainAliasConstituent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DomainAliasConstituent {
    pub domain: HandleSpan<Identifier>,
    pub domain_symbol: SymbolHandle,
}

impl Default for DomainAliasConstituent {
    fn default() -> Self {
        Self {
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::invalid(),
        }
    }
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
    if arguments.is_empty() {
        return Ok(program
            .semantic_domains
            .name(domain.semantic_id)
            .unwrap_or(domain.name.as_str())
            .to_owned());
    }
    let mut identities = Vec::with_capacity(arguments.len());
    for (parameter, argument) in parameters.iter().zip(arguments) {
        let crate::data::TypeParameterKind::Const { type_reference } = parameter.kind else {
            return Err(Diagnostic::error(format!(
                "domain family `{}` has a non-const index binder `{}`",
                domain.name, parameter.name
            )));
        };
        let expected = const_index_type_name(program, type_reference)?;
        identities.push(closed_domain_argument_identity(
            program, *argument, &expected,
        )?);
    }
    let base = program
        .semantic_domains
        .name(domain.semantic_id)
        .unwrap_or(domain.name.as_str());
    Ok(format!("{base}<{}>", identities.join(",")))
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
                program.normalized_type_identity(argument)
            ));
        }
        return Err(Diagnostic::error(
            "indexed-domain arguments must be canonical const values, direct const binders, or supported open const expressions",
        ));
    };
    if let Some(value) =
        psi_language_semantics::const_value::CanonicalConstValue::from_atom(name.as_str())
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

impl Default for ProofFact {
    fn default() -> Self {
        Self::Expression(crate::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, Eq)]
pub struct ProofMembershipFact {
    pub value: crate::expression::ExpressionHandle,
    pub domain: HandleSpan<Identifier>,
    pub domain_symbol: SymbolHandle,
    /// Exact authored terminal domain selection. Generated facts carry the
    /// empty span; this is review provenance, never proof identity.
    pub domain_use_span: psi_source::SourceSpan,
}

impl PartialEq for ProofMembershipFact {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.domain == other.domain
            && self.domain_symbol == other.domain_symbol
    }
}

impl Default for ProofMembershipFact {
    fn default() -> Self {
        Self {
            value: crate::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol: SymbolHandle::invalid(),
            domain_use_span: psi_source::SourceSpan::default(),
        }
    }
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

        let Some(source) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == source_domain)
        else {
            return false;
        };
        let Some(target) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == target_domain)
        else {
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
