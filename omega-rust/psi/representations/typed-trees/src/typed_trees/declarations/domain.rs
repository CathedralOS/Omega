use crate::TypedTrees;
use crate::name::Identifier;
use crate::type_identity::TypeIdentityRequest;
use crate::types::TypeReferenceHandle;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

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
            type_parameters: HandleSpan::empty(),
            target_type: TypeReferenceHandle::invalid(),
            index_arguments: Vec::new(),
            is_public: false,
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
        match parameter.kind {
            crate::data::TypeParameterKind::Type => identities.push(format!(
                "type:{}",
                program.normalized_type_identity(*argument)
            )),
            crate::data::TypeParameterKind::Const { type_reference } => {
                let expected = const_index_type_name(program, type_reference)?;
                identities.push(closed_domain_argument_identity(
                    program, *argument, &expected,
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
pub fn supports_symbol_only_proof(program: &TypedTrees, domain_symbol: SymbolHandle) -> bool {
    fn visit(program: &TypedTrees, symbol: SymbolHandle, visited: &mut Vec<SymbolHandle>) -> bool {
        let Some(domain) = program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
        else {
            return false;
        };
        if !index_parameters(program, domain).is_empty() || !domain.index_arguments.is_empty() {
            return false;
        }
        if visited.contains(&symbol) {
            return true;
        }
        visited.push(symbol);
        domain.alias.as_ref().is_none_or(|alias| {
            alias
                .constituents
                .iter()
                .all(|constituent| visit(program, constituent.domain_symbol, visited))
        }) && program.proof_facts(domain).iter().all(|fact| match fact {
            ProofFact::Membership(membership) => {
                membership.domain_arguments.is_empty()
                    && visit(program, membership.domain_symbol, visited)
            }
            _ => true,
        })
    }
    visit(program, domain_symbol, &mut Vec::new())
}

/// Whether an established membership in the `source` domain instance proves
/// membership in the `target` instance of the declared domain graph. This is
/// the indexed proof reader [`supports_symbol_only_proof`] fences off: a
/// declaration-only reader cannot consume an indexed-membership theory, so
/// the same normalized-identity and authored-membership walk as
/// [`declared_domain_implies`] replays here carrying each instance's exact
/// index arguments.
///
/// `source_arguments` and `target_arguments` are the established and required
/// index arguments in `index_parameters` order, matching the convention
/// `ProofMembershipFact::domain_arguments` already uses. An authored
/// `self in Parent<A>` step evaluates `A` under the enclosing domain's
/// established bindings (`TypeIdentityRequest::substitutions` follows a binder
/// to its bound argument transitively), so `Indexed<N>` declaring
/// `self in Parent<N>` contributes the `Parent` instance holding the
/// established argument. Indexed applications of one family have no implicit
/// variance: a chain reaches the required instance only when every argument
/// position normalizes to an identical identity, and an authored argument
/// that stays open after substitution can only prove a required instance
/// spelling the same open argument. The required side is read closed.
pub fn declared_domain_instance_implies(
    program: &TypedTrees,
    source_domain: SymbolHandle,
    source_arguments: &[TypeReferenceHandle],
    target_domain: SymbolHandle,
    target_arguments: &[TypeReferenceHandle],
) -> bool {
    fn visit(
        program: &TypedTrees,
        source_domain: SymbolHandle,
        source_arguments: &[TypeReferenceHandle],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
        target_domain: SymbolHandle,
        target_arguments: &[TypeReferenceHandle],
        visited: &mut Vec<(SymbolHandle, Vec<String>)>,
    ) -> bool {
        if !source_domain.is_valid() || !target_domain.is_valid() {
            return false;
        }
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

        // The source instance's identity interprets its arguments in the
        // parent scope that supplied them; the domain's own index binders
        // then extend that environment for its authored facts.
        let instance: Vec<String> = source_arguments
            .iter()
            .map(|argument| {
                program
                    .type_identity(TypeIdentityRequest {
                        substitutions,
                        ..TypeIdentityRequest::ordinary(*argument)
                    })
                    .as_str()
                    .to_owned()
            })
            .collect();
        if (source_domain == target_domain
            || (source.semantic_id.is_valid() && source.semantic_id == target.semantic_id))
            && instance.len() == target_arguments.len()
            && target_arguments
                .iter()
                .zip(&instance)
                .all(|(required, given)| {
                    *given
                        == program
                            .type_identity(TypeIdentityRequest::ordinary(*required))
                            .as_str()
                })
        {
            return true;
        }
        let key = (source_domain, instance);
        if visited.contains(&key) {
            return false;
        }
        visited.push(key);

        let mut environment = substitutions.to_vec();
        environment.extend(
            index_parameters(program, source)
                .iter()
                .map(|parameter| parameter.symbol)
                .zip(source_arguments.iter().copied()),
        );

        if source.alias.as_ref().is_some_and(|alias| {
            alias.constituents.iter().any(|constituent| {
                visit(
                    program,
                    constituent.domain_symbol,
                    &[],
                    &environment,
                    target_domain,
                    target_arguments,
                    visited,
                )
            })
        }) {
            return true;
        }

        program.proof_facts(source).iter().any(|fact| match fact {
            ProofFact::Membership(membership) => visit(
                program,
                membership.domain_symbol,
                program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments),
                &environment,
                target_domain,
                target_arguments,
                visited,
            ),
            _ => false,
        })
    }

    visit(
        program,
        source_domain,
        source_arguments,
        &[],
        target_domain,
        target_arguments,
        &mut Vec::new(),
    )
}

/// The `self in Parent` propositions in a domain's predicate list: the implied
/// parent a refinement-chain declaration (`domain A::B::C`) desugars to, and
/// any authored self-memberships. A membership in the domain entails each of
/// these, so every reader of the domain's theory walks them the same way.
pub fn self_membership_facts<'a>(
    program: &'a TypedTrees,
    domain: &'a DomainDefinition,
) -> impl Iterator<Item = &'a ProofMembershipFact> + 'a {
    program
        .proof_facts(domain)
        .iter()
        .filter_map(|fact| match fact {
            ProofFact::Membership(membership) => {
                let self_member = matches!(
                    program.expression_table.expression(membership.value),
                    crate::expression::ExpressionNode::Name(path)
                        if matches!(
                            program.expression_table.name_path_members(path.members),
                            [member] if member.as_str() == "self"
                        )
                );
                self_member.then_some(membership)
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

#[cfg(test)]
mod symbol_only_proof_tests {
    use super::{
        DomainDefinition, HandleSpan, ProofFact, ProofMembershipFact, SymbolHandle,
        TypeReferenceHandle, TypedTrees, declared_domain_implies, supports_symbol_only_proof,
    };

    #[test]
    fn indexed_membership_cannot_enter_a_symbol_only_implication_chain() {
        let mut program = TypedTrees::default();
        let symbol = |index| SymbolHandle::from_arena_index(index);
        let mut plain = DomainDefinition {
            symbol: symbol(1),
            ..Default::default()
        };
        let mut indexed = DomainDefinition {
            symbol: symbol(2),
            ..Default::default()
        };
        program.push_domain_type_parameter(
            &mut indexed,
            crate::data::TypeParameter {
                kind: crate::data::TypeParameterKind::Const {
                    type_reference: TypeReferenceHandle::invalid(),
                },
                ..Default::default()
            },
        );
        let target = DomainDefinition {
            symbol: symbol(3),
            ..Default::default()
        };
        program.proof_facts.append_to_span(
            &mut plain.facts,
            ProofFact::Membership(ProofMembershipFact {
                domain_symbol: indexed.symbol,
                ..Default::default()
            }),
        );
        program.proof_facts.append_to_span(
            &mut indexed.facts,
            ProofFact::Membership(ProofMembershipFact {
                domain_symbol: target.symbol,
                ..Default::default()
            }),
        );
        program.push_domain_definition(plain);
        program.push_domain_definition(indexed);
        program.push_domain_definition(target);
        assert!(
            declared_domain_implies(&program, symbol(1), symbol(3)),
            "metadata still sees the declared relationship"
        );
        assert!(
            !supports_symbol_only_proof(&program, symbol(1)),
            "an intermediate indexed family cannot grant a proof"
        );
        assert!(
            !supports_symbol_only_proof(&program, symbol(2)),
            "an indexed source cannot lose its instance"
        );
        assert!(
            supports_symbol_only_proof(&program, symbol(3)),
            "an unrelated unindexed theory remains eligible"
        );
    }

    #[test]
    fn retained_membership_arguments_cannot_be_erased_after_specialization() {
        let mut program = TypedTrees::default();
        let symbol = |index| SymbolHandle::from_arena_index(index);
        let mut source = DomainDefinition {
            symbol: symbol(1),
            ..Default::default()
        };
        let target = DomainDefinition {
            symbol: symbol(2),
            ..Default::default()
        };
        program.proof_facts.append_to_span(
            &mut source.facts,
            ProofFact::Membership(ProofMembershipFact {
                domain_symbol: target.symbol,
                domain_arguments: HandleSpan::from_parts(arena::Handle::from_arena_index(1), 1),
                ..Default::default()
            }),
        );
        program.push_domain_definition(source);
        program.push_domain_definition(target);
        assert!(!supports_symbol_only_proof(&program, symbol(1)));
        assert!(supports_symbol_only_proof(&program, symbol(2)));
    }
}

#[cfg(test)]
mod domain_instance_proof_tests {
    use super::{
        DomainDefinition, ProofFact, ProofMembershipFact, SymbolHandle, TypeReferenceHandle,
        TypedTrees, declared_domain_instance_implies, supports_symbol_only_proof,
    };
    use crate::name::Identifier;
    use crate::types::TypeReferenceNode;

    fn named_argument(
        program: &mut TypedTrees,
        symbol: SymbolHandle,
        name: &str,
    ) -> TypeReferenceHandle {
        program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol,
                name: Identifier::generated(name),
            })
    }

    /// A domain family with one const index parameter, as `Family<const N: u64>`.
    fn const_indexed_domain(
        program: &mut TypedTrees,
        symbol: SymbolHandle,
        parameter_symbol: SymbolHandle,
    ) -> DomainDefinition {
        let mut domain = DomainDefinition {
            symbol,
            ..Default::default()
        };
        program.push_domain_type_parameter(
            &mut domain,
            crate::data::TypeParameter {
                symbol: parameter_symbol,
                name: Identifier::generated("N"),
                kind: crate::data::TypeParameterKind::Const {
                    type_reference: TypeReferenceHandle::invalid(),
                },
                ..Default::default()
            },
        );
        domain
    }

    fn membership(
        program: &mut TypedTrees,
        source: &mut DomainDefinition,
        domain_symbol: SymbolHandle,
        arguments: &[TypeReferenceHandle],
    ) {
        let domain_arguments = program
            .type_reference_table
            .insert_type_reference_handles(arguments.iter().copied());
        program.proof_facts.append_to_span(
            &mut source.facts,
            ProofFact::Membership(ProofMembershipFact {
                domain_symbol,
                domain_arguments,
                ..Default::default()
            }),
        );
    }

    #[test]
    fn same_family_requires_exactly_equal_arguments() {
        let mut program = TypedTrees::default();
        let symbol = |index| SymbolHandle::from_arena_index(index);
        let domain = const_indexed_domain(&mut program, symbol(1), symbol(10));
        program.push_domain_definition(domain);
        let seven = named_argument(&mut program, SymbolHandle::invalid(), "7");
        let eight = named_argument(&mut program, SymbolHandle::invalid(), "8");

        assert!(declared_domain_instance_implies(
            &program,
            symbol(1),
            &[seven],
            symbol(1),
            &[seven]
        ));
        assert!(
            !declared_domain_instance_implies(&program, symbol(1), &[seven], symbol(1), &[eight]),
            "indexed applications of one family have no variance"
        );
        assert!(
            !declared_domain_instance_implies(&program, symbol(1), &[], symbol(1), &[seven]),
            "the bare family never covers an applied instance"
        );
    }

    #[test]
    fn authored_membership_substitutes_the_established_argument() {
        let mut program = TypedTrees::default();
        let symbol = |index| SymbolHandle::from_arena_index(index);
        // `Indexed<N>` declares `self in Parent<N>`; the established
        // `Indexed<7>` membership contributes the `Parent<7>` instance.
        let mut indexed = const_indexed_domain(&mut program, symbol(1), symbol(10));
        let parent = const_indexed_domain(&mut program, symbol(2), symbol(11));
        let binder = named_argument(&mut program, symbol(10), "N");
        membership(&mut program, &mut indexed, symbol(2), &[binder]);
        program.push_domain_definition(indexed);
        program.push_domain_definition(parent);
        let seven = named_argument(&mut program, SymbolHandle::invalid(), "7");
        let eight = named_argument(&mut program, SymbolHandle::invalid(), "8");

        assert!(declared_domain_instance_implies(
            &program,
            symbol(1),
            &[seven],
            symbol(2),
            &[seven]
        ));
        assert!(!declared_domain_instance_implies(
            &program,
            symbol(1),
            &[seven],
            symbol(2),
            &[eight]
        ));
        assert!(
            !supports_symbol_only_proof(&program, symbol(1)),
            "the same theory stays fenced from the symbol-only reader"
        );
    }

    #[test]
    fn closed_membership_enters_an_indexed_instance() {
        let mut program = TypedTrees::default();
        let symbol = |index| SymbolHandle::from_arena_index(index);
        // An unindexed theory may still declare `self in Capped<3>`; the
        // indexed reader reaches exactly that instance and no other.
        let mut small = DomainDefinition {
            symbol: symbol(1),
            ..Default::default()
        };
        let capped = const_indexed_domain(&mut program, symbol(2), symbol(11));
        let three = named_argument(&mut program, SymbolHandle::invalid(), "3");
        membership(&mut program, &mut small, symbol(2), &[three]);
        program.push_domain_definition(small);
        program.push_domain_definition(capped);

        assert!(declared_domain_instance_implies(
            &program,
            symbol(1),
            &[],
            symbol(2),
            &[three]
        ));
        let four = named_argument(&mut program, SymbolHandle::invalid(), "4");
        assert!(!declared_domain_instance_implies(
            &program,
            symbol(1),
            &[],
            symbol(2),
            &[four]
        ));
    }

    #[test]
    fn unindexed_chains_still_imply() {
        let mut program = TypedTrees::default();
        let symbol = |index| SymbolHandle::from_arena_index(index);
        let mut source = DomainDefinition {
            symbol: symbol(1),
            ..Default::default()
        };
        let target = DomainDefinition {
            symbol: symbol(2),
            ..Default::default()
        };
        membership(&mut program, &mut source, symbol(2), &[]);
        program.push_domain_definition(source);
        program.push_domain_definition(target);

        assert!(declared_domain_instance_implies(
            &program,
            symbol(1),
            &[],
            symbol(2),
            &[]
        ));
    }

    #[test]
    fn shared_semantic_identity_with_equal_arguments_implies() {
        let mut program = TypedTrees::default();
        let symbol = |index| SymbolHandle::from_arena_index(index);
        // Capacity specializations keep distinct symbols under one semantic
        // identity; the instances imply only when the index arguments match.
        let semantic = program.semantic_domains.intern("Theory");
        let mut left = const_indexed_domain(&mut program, symbol(1), symbol(10));
        left.semantic_id = semantic;
        let mut right = const_indexed_domain(&mut program, symbol(2), symbol(11));
        right.semantic_id = semantic;
        program.push_domain_definition(left);
        program.push_domain_definition(right);
        let seven = named_argument(&mut program, SymbolHandle::invalid(), "7");
        let nine = named_argument(&mut program, SymbolHandle::invalid(), "9");

        assert!(declared_domain_instance_implies(
            &program,
            symbol(1),
            &[seven],
            symbol(2),
            &[seven]
        ));
        assert!(!declared_domain_instance_implies(
            &program,
            symbol(1),
            &[seven],
            symbol(2),
            &[nine]
        ));
    }
}
