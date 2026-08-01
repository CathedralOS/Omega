//! Deterministic semantic identity for typed type references.
//!
//! Diagnostic rendering is deliberately not an identity oracle. In
//! particular, a domain conjunction is commutative and idempotent, while its
//! authored spelling has an order and may repeat terms. This module owns the
//! canonical form used by type equality, specialization, and published plan
//! identities (decision 19 / DOM4).

use crate::TypedTrees;
use crate::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use omega_core::arena::HandleSpan;
use omega_core::symbols::SymbolHandle;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedTypeIdentity(String);

impl NormalizedTypeIdentity {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Display for NormalizedTypeIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// The canonical conjunction of semantic domains selected on one binding.
///
/// Terms are sorted and deduplicated. Declared domains are keyed by their
/// normalizer-owned semantic name, not by source order, local arena handles, or
/// a carrier-specialized declaration symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NormalizedDomainExpression {
    terms: Vec<NormalizedDomainTerm>,
}

impl NormalizedDomainExpression {
    pub fn terms(&self) -> &[NormalizedDomainTerm] {
        &self.terms
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    fn from_constraints(program: &TypedTrees, constraints: HandleSpan<TypeConstraintNode>) -> Self {
        let mut terms = program
            .type_reference_table
            .constraints(constraints)
            .iter()
            .filter_map(|constraint| normalized_domain_term(program, constraint))
            .collect::<Vec<_>>();
        terms.sort();
        terms.dedup();
        Self { terms }
    }
}

/// The canonical set of domains that may select a named-machine or
/// requirement overload from its expected result type.
///
/// Unlike [`NormalizedDomainExpression`], this set expands transparent domain
/// aliases and omits predicate-only refinements. Arithmetic policies, domains
/// with a semantic role or establishment route, and explicit empty tags remain
/// dispatch-bearing. Terms are sorted and deduplicated so authored conjunction
/// order cannot affect overload identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct NormalizedResultDispatchSet {
    terms: Vec<NormalizedDomainTerm>,
}

impl NormalizedResultDispatchSet {
    pub fn terms(&self) -> &[NormalizedDomainTerm] {
        &self.terms
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn identity(&self) -> String {
        self.terms
            .iter()
            .map(|term| match term {
                NormalizedDomainTerm::Arithmetic(name) => format!("arithmetic:{name}"),
                NormalizedDomainTerm::Declared(name) => format!("declared:{name}"),
            })
            .collect::<Vec<_>>()
            .join("&")
    }
}

/// Canonical overload identity for an explicit named machine or requirement.
/// The result carrier and predicate-only refinements are deliberately absent:
/// a declaration is selected by path, parameter signature, and the exact set
/// of dispatch-bearing result domains.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedNamedCallableIdentity {
    path: String,
    parameters: String,
    result_dispatch: NormalizedResultDispatchSet,
}

impl NormalizedNamedCallableIdentity {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn parameters(&self) -> &str {
        &self.parameters
    }

    pub fn result_dispatch(&self) -> &NormalizedResultDispatchSet {
        &self.result_dispatch
    }

    pub fn identity(&self) -> String {
        compound(
            "named-callable",
            [
                atom("path", &self.path),
                atom("parameters", &self.parameters),
                atom("result-dispatch", &self.result_dispatch.identity()),
            ],
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NormalizedDomainTerm {
    Arithmetic(String),
    Declared(String),
}

impl TypedTrees {
    pub fn normalized_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            type_reference,
            &TypeIdentityContext::default(),
        ))
    }

    /// Binder-aware form for generic template identity. A parameter symbol is
    /// replaced before serialization, so renaming the source binder cannot
    /// change the normalized contract while concrete declaration paths remain
    /// fully qualified.
    pub fn normalized_type_identity_with_binders(
        &self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            type_reference,
            &TypeIdentityContext { binders },
        ))
    }

    pub fn normalized_domain_expression(
        &self,
        constraints: HandleSpan<TypeConstraintNode>,
    ) -> NormalizedDomainExpression {
        NormalizedDomainExpression::from_constraints(self, constraints)
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

    /// Canonical identity of one top-level named machine overload. A machine's
    /// first state is its callable entry signature; explicit sibling states do
    /// not create additional top-level overloads.
    pub fn normalized_machine_overload_identity(
        &self,
        machine: &crate::machine::Machine,
    ) -> Option<NormalizedNamedCallableIdentity> {
        let entry = self.machine_states(machine).first()?;
        Some(self.normalized_named_callable_identity(
            machine.name.as_str(),
            machine.symbol,
            self.machine_type_parameters(machine),
            self.state_parameters(entry),
            entry.return_type,
        ))
    }

    /// Canonical identity of one trait machine requirement overload.
    pub fn normalized_trait_requirement_overload_identity(
        &self,
        trait_definition: &crate::trait_definition::TraitDefinition,
        requirement: &crate::signature::StateSignature,
    ) -> NormalizedNamedCallableIdentity {
        let mut type_parameters = self.trait_type_parameters(trait_definition).to_vec();
        type_parameters.extend_from_slice(self.state_signature_type_parameters(requirement));
        self.normalized_named_callable_identity(
            &format!("{}::{}", trait_definition.name, requirement.name),
            trait_definition.symbol,
            &type_parameters,
            self.state_signature_parameters(requirement),
            requirement.return_type,
        )
    }

    /// Canonical identity of one explicitly named operator requirement.
    /// Unspelled boundary operators participate in the same result-domain
    /// lookup as other named requirements; fixed spellings remain
    /// operand-directed and use this only for declaration diagnostics.
    pub fn normalized_operator_overload_identity(
        &self,
        operator: &crate::operator::OperatorDefinition,
    ) -> NormalizedNamedCallableIdentity {
        let path = self
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::");
        let flags = self
            .operator_parameters(operator)
            .iter()
            .map(|parameter| {
                format!(
                    "self={};mutable={};const={}",
                    parameter.is_self, parameter.is_mutable, parameter.is_const
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        NormalizedNamedCallableIdentity {
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

    fn normalized_named_callable_identity(
        &self,
        path: &str,
        owner_symbol: SymbolHandle,
        type_parameters: &[crate::data::TypeParameter],
        parameters: &[crate::signature::StateParameter],
        return_type: TypeReferenceHandle,
    ) -> NormalizedNamedCallableIdentity {
        let mut binders = Vec::with_capacity(type_parameters.len() + 1);
        if owner_symbol.is_valid() {
            binders.push((owner_symbol, "$Self".to_owned()));
        }
        binders.extend(
            type_parameters
                .iter()
                .enumerate()
                .filter(|(_, parameter)| parameter.symbol.is_valid())
                .map(|(index, parameter)| (parameter.symbol, format!("$T{index}"))),
        );
        let parameters = parameters
            .iter()
            .map(|parameter| {
                compound(
                    "parameter",
                    [
                        atom("self", if parameter.is_self { "yes" } else { "no" }),
                        atom("mutable", if parameter.is_mutable { "yes" } else { "no" }),
                        atom("const", if parameter.is_const { "yes" } else { "no" }),
                        self.normalized_type_identity_with_binders(
                            parameter.type_reference,
                            &binders,
                        )
                        .into_string(),
                    ],
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        NormalizedNamedCallableIdentity {
            path: path.to_owned(),
            parameters,
            result_dispatch: self.normalized_result_dispatch_set(return_type),
        }
    }
}

fn collect_result_dispatch_terms(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    terms: &mut Vec<NormalizedDomainTerm>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    if !type_reference.is_valid() {
        return;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            collect_result_dispatch_terms(program, *referee, terms, alias_stack);
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            collect_result_dispatch_terms(program, *base_type, terms, alias_stack);
            for constraint in program.type_reference_table.constraints(*constraints) {
                match constraint {
                    TypeConstraintNode::ArithmeticDomain(domain) => {
                        terms.push(NormalizedDomainTerm::Arithmetic(domain.name().to_owned()))
                    }
                    TypeConstraintNode::Domain(domain) => {
                        collect_declared_result_dispatch_terms(program, domain, terms, alias_stack);
                    }
                    TypeConstraintNode::Named(_) | TypeConstraintNode::Range { .. } => {}
                }
            }
        }
        TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Unit => {}
    }
}

fn collect_declared_result_dispatch_terms(
    program: &TypedTrees,
    constraint: &DomainConstraint,
    terms: &mut Vec<NormalizedDomainTerm>,
    alias_stack: &mut Vec<SymbolHandle>,
) {
    let definition = constraint.symbol.is_valid().then(|| {
        program
            .domain_definitions()
            .iter()
            .find(|definition| definition.symbol == constraint.symbol)
    });
    if let Some(Some(definition)) = definition {
        if let Some(alias) = definition.alias.as_ref() {
            if alias_stack.contains(&definition.symbol) {
                return;
            }
            alias_stack.push(definition.symbol);
            for constituent in &alias.constituents {
                let Some(constituent_definition) = program
                    .domain_definitions()
                    .iter()
                    .find(|candidate| candidate.symbol == constituent.domain_symbol)
                else {
                    continue;
                };
                let constituent_constraint = DomainConstraint {
                    name: constituent_definition.name.clone(),
                    arguments: Vec::new(),
                    symbol: constituent_definition.symbol,
                    semantic_id: constituent_definition.semantic_id,
                    predicate_body: constituent_definition.predicate_body,
                    semantic_roles: constituent_definition.semantic_roles,
                    establishment_routes: constituent_definition.establishment_routes.clone(),
                };
                collect_declared_result_dispatch_terms(
                    program,
                    &constituent_constraint,
                    terms,
                    alias_stack,
                );
            }
            alias_stack.pop();
            return;
        }
        if definition.predicate_body.is_present()
            && definition.semantic_roles.is_empty()
            && definition.establishment_routes.is_empty()
        {
            return;
        }
        terms.push(NormalizedDomainTerm::Declared(declared_domain_name(
            program,
            definition.semantic_id,
            definition.name.as_str(),
        )));
        return;
    }

    // Compiler-known or partially constructed constraints may not have a
    // declaration record yet. Their copied normalized metadata is still the
    // authority for dispatch-bearing classification.
    if constraint.predicate_body.is_present()
        && constraint.semantic_roles.is_empty()
        && constraint.establishment_routes.is_empty()
    {
        return;
    }
    terms.push(NormalizedDomainTerm::Declared(declared_domain_name(
        program,
        constraint.semantic_id,
        constraint.name.as_str(),
    )));
}

fn declared_domain_name(
    program: &TypedTrees,
    semantic_id: omega_core::semantics::SemanticDomainId,
    fallback: &str,
) -> String {
    semantic_id
        .is_valid()
        .then(|| program.semantic_domains.name(semantic_id))
        .flatten()
        .unwrap_or(fallback)
        .to_owned()
}

#[derive(Default)]
struct TypeIdentityContext<'binders> {
    binders: &'binders [(SymbolHandle, String)],
}

impl TypeIdentityContext<'_> {
    fn name(&self, program: &TypedTrees, symbol: SymbolHandle, fallback: &str) -> String {
        if let Some((_, replacement)) = self
            .binders
            .iter()
            .find(|(candidate, _)| *candidate == symbol)
        {
            return replacement.clone();
        }
        if symbol.is_valid() {
            let path = program.symbols.display_path(symbol, "::");
            if !path.is_empty() {
                return path;
            }
        }
        fallback.to_owned()
    }
}

fn normalize_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            is_mutable,
            lifetime: _,
        } => compound(
            if *is_mutable { "ref-mut" } else { "ref" },
            [normalize_type_reference(program, *referee, context)],
        ),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let (base, mut all_constraints) =
                normalize_constrained_base(program, *base_type, context);
            all_constraints.extend(normalized_constraints(program, *constraints));
            all_constraints.sort();
            all_constraints.dedup();
            compound(
                "constrained",
                std::iter::once(base).chain(
                    all_constraints
                        .into_iter()
                        .map(NormalizedConstraint::encode),
                ),
            )
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => compound(
            "array",
            [
                normalize_type_reference(program, *element_type, context),
                normalize_array_length(program, length, context),
            ],
        ),
        TypeReferenceNode::Slice { element_type } => compound(
            "slice",
            [normalize_type_reference(program, *element_type, context)],
        ),
        TypeReferenceNode::Generic {
            base_symbol,
            base_name,
            lifetime_arguments: _,
            arguments,
            ..
        } => compound(
            "generic",
            std::iter::once(atom(
                "name",
                &context.name(program, *base_symbol, base_name.as_str()),
            ))
            .chain(
                program
                    .type_reference_table
                    .type_reference_handles(*arguments)
                    .iter()
                    .map(|argument| normalize_type_reference(program, *argument, context)),
            ),
        ),
        TypeReferenceNode::DynamicTrait { symbol, name } => compound(
            "dynamic-trait",
            [atom("name", &context.name(program, *symbol, name.as_str()))],
        ),
        TypeReferenceNode::Named { symbol, name } => compound(
            "named",
            [atom("name", &context.name(program, *symbol, name.as_str()))],
        ),
        TypeReferenceNode::Unit => "unit".to_owned(),
    }
}

fn normalize_constrained_base(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> (String, Vec<NormalizedConstraint>) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let (base, mut all_constraints) =
                normalize_constrained_base(program, *base_type, context);
            all_constraints.extend(normalized_constraints(program, *constraints));
            (base, all_constraints)
        }
        _ => (
            normalize_type_reference(program, type_reference, context),
            Vec::new(),
        ),
    }
}

fn normalized_constraints(
    program: &TypedTrees,
    constraints: HandleSpan<TypeConstraintNode>,
) -> Vec<NormalizedConstraint> {
    program
        .type_reference_table
        .constraints(constraints)
        .iter()
        .map(|constraint| match constraint {
            TypeConstraintNode::Named(name) => {
                NormalizedConstraint::Named(name.as_str().to_owned())
            }
            TypeConstraintNode::Range { minimum, maximum } => NormalizedConstraint::Range {
                minimum: program.expression_table.display_name(*minimum),
                maximum: program.expression_table.display_name(*maximum),
            },
            TypeConstraintNode::ArithmeticDomain(domain) => {
                NormalizedConstraint::Arithmetic(domain.name().to_owned())
            }
            TypeConstraintNode::Domain(domain) => {
                NormalizedConstraint::DeclaredDomain(declared_domain_identity(program, domain))
            }
        })
        .collect()
}

fn normalized_domain_term(
    program: &TypedTrees,
    constraint: &TypeConstraintNode,
) -> Option<NormalizedDomainTerm> {
    match constraint {
        TypeConstraintNode::ArithmeticDomain(domain) => {
            Some(NormalizedDomainTerm::Arithmetic(domain.name().to_owned()))
        }
        TypeConstraintNode::Domain(domain) => Some(NormalizedDomainTerm::Declared(
            declared_domain_identity(program, domain),
        )),
        TypeConstraintNode::Named(_) | TypeConstraintNode::Range { .. } => None,
    }
}

fn declared_domain_identity(program: &TypedTrees, domain: &DomainConstraint) -> String {
    if domain.semantic_id.is_valid()
        && let Some(name) = program.semantic_domains.name(domain.semantic_id)
    {
        return name.to_owned();
    }
    domain.name.as_str().to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum NormalizedConstraint {
    Arithmetic(String),
    DeclaredDomain(String),
    Named(String),
    Range { minimum: String, maximum: String },
}

impl NormalizedConstraint {
    fn encode(self) -> String {
        match self {
            Self::Arithmetic(name) => compound("arithmetic-domain", [atom("name", &name)]),
            Self::DeclaredDomain(name) => compound("declared-domain", [atom("name", &name)]),
            Self::Named(name) => compound("named-constraint", [atom("name", &name)]),
            Self::Range { minimum, maximum } => compound(
                "range",
                [atom("minimum", &minimum), atom("maximum", &maximum)],
            ),
        }
    }
}

fn normalize_array_length(
    program: &TypedTrees,
    length: &FixedArrayLength,
    context: &TypeIdentityContext<'_>,
) -> String {
    match length {
        FixedArrayLength::Literal(value) => atom("literal", &value.to_string()),
        FixedArrayLength::ConstParameter { symbol, name } => atom(
            "const-parameter",
            &context.name(program, *symbol, name.as_str()),
        ),
        FixedArrayLength::ConstCall { name } => atom("const-call", name.as_str()),
    }
}

fn atom(tag: &str, value: &str) -> String {
    let mut output = String::with_capacity(tag.len() + value.len() + 2);
    output.push_str(tag);
    output.push('(');
    for character in value.chars() {
        if matches!(character, '\\' | '(' | ')' | ',') {
            output.push('\\');
        }
        output.push(character);
    }
    output.push(')');
    output
}

fn compound(tag: &str, parts: impl IntoIterator<Item = String>) -> String {
    let parts = parts.into_iter().collect::<Vec<_>>();
    let mut output =
        String::with_capacity(tag.len() + parts.iter().map(String::len).sum::<usize>() + 2);
    output.push_str(tag);
    output.push('(');
    for (index, part) in parts.into_iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&part);
    }
    output.push(')');
    output
}

#[cfg(test)]
mod tests {
    use super::{NormalizedDomainTerm, NormalizedTypeIdentity};
    use crate::TypedTrees;
    use crate::domain::{DomainAliasConstituent, DomainAliasDefinition, DomainDefinition};
    use crate::name::Identifier;
    use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
    use omega_core::semantics::{
        DomainEstablishmentRoute, DomainPredicateBody, DomainSemanticRoles, SemanticDomainId,
    };
    use omega_core::symbols::SymbolHandle;

    fn declared(name: &str, semantic_id: SemanticDomainId) -> TypeConstraintNode {
        TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated(name),
            arguments: Vec::new(),
            symbol: SymbolHandle::invalid(),
            semantic_id,
            predicate_body: DomainPredicateBody::Present,
            semantic_roles: DomainSemanticRoles::default(),
            establishment_routes: Vec::new(),
        })
    }

    fn declared_with_metadata(
        name: &str,
        symbol: SymbolHandle,
        semantic_id: SemanticDomainId,
        predicate_body: DomainPredicateBody,
        semantic_roles: DomainSemanticRoles,
        establishment_routes: Vec<DomainEstablishmentRoute>,
    ) -> TypeConstraintNode {
        TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated(name),
            arguments: Vec::new(),
            symbol,
            semantic_id,
            predicate_body,
            semantic_roles,
            establishment_routes,
        })
    }

    fn constrained(
        program: &mut TypedTrees,
        base_type: omega_core::arena::Handle<TypeReferenceNode>,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> omega_core::arena::Handle<TypeReferenceNode> {
        let constraints = program.type_reference_table.insert_constraints(constraints);
        program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type,
                constraints,
            })
    }

    fn generic_machine(
        program: &mut TypedTrees,
        owner_symbol: SymbolHandle,
        binder_symbol: SymbolHandle,
        binder_name: &str,
        return_type: omega_core::arena::Handle<TypeReferenceNode>,
    ) -> crate::machine::Machine {
        let mut machine = crate::machine::Machine {
            symbol: owner_symbol,
            name: Identifier::generated("I32::from_value"),
            ..crate::machine::Machine::default()
        };
        program.push_machine_type_parameter(
            &mut machine,
            crate::data::TypeParameter {
                symbol: binder_symbol,
                name: Identifier::generated(binder_name),
                ..crate::data::TypeParameter::default()
            },
        );
        let parameter_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: binder_symbol,
                name: Identifier::generated(binder_name),
            });
        let mut entry = crate::state::State {
            symbol: SymbolHandle::from_arena_index(owner_symbol.arena_index() + 100),
            name: Identifier::generated("from_value"),
            return_type,
            ..crate::state::State::default()
        };
        program.push_state_parameter(
            &mut entry,
            crate::signature::StateParameter {
                symbol: SymbolHandle::from_arena_index(owner_symbol.arena_index() + 200),
                name: Identifier::generated("value"),
                type_reference: parameter_type,
                ..crate::signature::StateParameter::default()
            },
        );
        program.push_machine_state(&mut machine, entry);
        machine
    }

    #[test]
    fn domain_conjunction_identity_is_sorted_deduplicated_and_semantic() {
        let mut program = TypedTrees::default();
        let utf8 = program.semantic_domains.intern("Utf8");
        let no_nul = program.semantic_domains.intern("NoNul");
        let constraints = program.type_reference_table.insert_constraints([
            declared("AliasForNoNul", no_nul),
            declared("Utf8", utf8),
            declared("Utf8Again", utf8),
        ]);

        let normalized = program.normalized_domain_expression(constraints);
        assert_eq!(
            normalized.terms(),
            [
                NormalizedDomainTerm::Declared("NoNul".to_owned()),
                NormalizedDomainTerm::Declared("Utf8".to_owned()),
            ]
        );
    }

    #[test]
    fn reordered_flat_and_nested_domain_conjunctions_have_one_type_identity() {
        let mut program = TypedTrees::default();
        let alpha = program.semantic_domains.intern("Alpha");
        let beta = program.semantic_domains.intern("Beta");
        let base = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });

        let flat = constrained(
            &mut program,
            base,
            [declared("Alpha", alpha), declared("Beta", beta)],
        );
        let nested_inner = constrained(&mut program, base, [declared("Beta", beta)]);
        let nested = constrained(
            &mut program,
            nested_inner,
            [declared("AlphaAlias", alpha), declared("Alpha", alpha)],
        );

        assert_eq!(
            program.normalized_type_identity(flat),
            program.normalized_type_identity(nested)
        );
    }

    #[test]
    fn same_constraint_count_does_not_collapse_distinct_domains() {
        let mut program = TypedTrees::default();
        let alpha = program.semantic_domains.intern("Alpha");
        let beta = program.semantic_domains.intern("Beta");
        let base = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let alpha_type = constrained(&mut program, base, [declared("Alpha", alpha)]);
        let beta_type = constrained(&mut program, base, [declared("Beta", beta)]);

        let alpha_identity: NormalizedTypeIdentity = program.normalized_type_identity(alpha_type);
        assert_ne!(alpha_identity, program.normalized_type_identity(beta_type));
    }

    #[test]
    fn result_dispatch_set_partitions_predicates_from_semantic_and_empty_tags() {
        let mut program = TypedTrees::default();
        let predicate = program.semantic_domains.intern("Positive");
        let semantic = program.semantic_domains.intern("Km");
        let routed = program.semantic_domains.intern("Validated");
        let empty = program.semantic_domains.intern("Marker");
        let base = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let route = DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: SymbolHandle::from_arena_index(41),
            requirement: SymbolHandle::from_arena_index(42),
        };
        let result = constrained(
            &mut program,
            base,
            [
                declared_with_metadata(
                    "Positive",
                    SymbolHandle::invalid(),
                    predicate,
                    DomainPredicateBody::Present,
                    DomainSemanticRoles::default(),
                    Vec::new(),
                ),
                declared_with_metadata(
                    "Km",
                    SymbolHandle::invalid(),
                    semantic,
                    DomainPredicateBody::Present,
                    DomainSemanticRoles {
                        denotation_dimension: Some(semantic),
                        arithmetic_policy: None,
                    },
                    Vec::new(),
                ),
                declared_with_metadata(
                    "Validated",
                    SymbolHandle::invalid(),
                    routed,
                    DomainPredicateBody::Present,
                    DomainSemanticRoles::default(),
                    vec![route],
                ),
                declared_with_metadata(
                    "Marker",
                    SymbolHandle::invalid(),
                    empty,
                    DomainPredicateBody::Bodyless,
                    DomainSemanticRoles::default(),
                    Vec::new(),
                ),
                TypeConstraintNode::ArithmeticDomain(
                    omega_core::arithmetic::ArithmeticDomain::Saturating,
                ),
                declared_with_metadata(
                    "MarkerAgain",
                    SymbolHandle::invalid(),
                    empty,
                    DomainPredicateBody::Bodyless,
                    DomainSemanticRoles::default(),
                    Vec::new(),
                ),
                TypeConstraintNode::ArithmeticDomain(
                    omega_core::arithmetic::ArithmeticDomain::Saturating,
                ),
            ],
        );

        let dispatch = program.normalized_result_dispatch_set(result);
        assert_eq!(
            dispatch.terms(),
            [
                NormalizedDomainTerm::Arithmetic("Saturating".to_owned()),
                NormalizedDomainTerm::Declared("Km".to_owned()),
                NormalizedDomainTerm::Declared("Marker".to_owned()),
                NormalizedDomainTerm::Declared("Validated".to_owned()),
            ]
        );
        assert_eq!(
            dispatch.identity(),
            "arithmetic:Saturating&declared:Km&declared:Marker&declared:Validated"
        );
    }

    #[test]
    fn result_dispatch_set_expands_aliases_before_partitioning() {
        let mut program = TypedTrees::default();
        let predicate_id = program.semantic_domains.intern("Positive");
        let marker_id = program.semantic_domains.intern("Marker");
        let alias_id = program.semantic_domains.intern("PositiveMarker");
        let predicate_symbol = SymbolHandle::from_arena_index(51);
        let marker_symbol = SymbolHandle::from_arena_index(52);
        let alias_symbol = SymbolHandle::from_arena_index(53);

        program.push_domain_definition(DomainDefinition {
            symbol: predicate_symbol,
            name: Identifier::generated("Positive"),
            semantic_id: predicate_id,
            predicate_body: DomainPredicateBody::Present,
            ..DomainDefinition::default()
        });
        program.push_domain_definition(DomainDefinition {
            symbol: marker_symbol,
            name: Identifier::generated("Marker"),
            semantic_id: marker_id,
            predicate_body: DomainPredicateBody::Bodyless,
            ..DomainDefinition::default()
        });
        program.push_domain_definition(DomainDefinition {
            symbol: alias_symbol,
            name: Identifier::generated("PositiveMarker"),
            alias: Some(DomainAliasDefinition {
                constituents: vec![
                    DomainAliasConstituent {
                        domain_symbol: predicate_symbol,
                        ..DomainAliasConstituent::default()
                    },
                    DomainAliasConstituent {
                        domain_symbol: marker_symbol,
                        ..DomainAliasConstituent::default()
                    },
                ],
            }),
            semantic_id: alias_id,
            ..DomainDefinition::default()
        });

        let base = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let result = constrained(
            &mut program,
            base,
            [declared_with_metadata(
                "PositiveMarker",
                alias_symbol,
                alias_id,
                DomainPredicateBody::Bodyless,
                DomainSemanticRoles::default(),
                Vec::new(),
            )],
        );

        assert_eq!(
            program.normalized_result_dispatch_set(result).terms(),
            [NormalizedDomainTerm::Declared("Marker".to_owned())]
        );
    }

    #[test]
    fn result_dispatch_set_flattens_qualification_shells_but_not_element_domains() {
        let mut program = TypedTrees::default();
        let outer = program.semantic_domains.intern("Outer");
        let element = program.semantic_domains.intern("Element");
        let base = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let qualified_element = constrained(
            &mut program,
            base,
            [declared_with_metadata(
                "Element",
                SymbolHandle::invalid(),
                element,
                DomainPredicateBody::Bodyless,
                DomainSemanticRoles::default(),
                Vec::new(),
            )],
        );
        let array = program
            .type_reference_table
            .insert(TypeReferenceNode::FixedArray {
                element_type: qualified_element,
                length: crate::types::FixedArrayLength::Literal(4),
            });
        let result = constrained(
            &mut program,
            array,
            [declared_with_metadata(
                "Outer",
                SymbolHandle::invalid(),
                outer,
                DomainPredicateBody::Bodyless,
                DomainSemanticRoles::default(),
                Vec::new(),
            )],
        );

        assert_eq!(
            program.normalized_result_dispatch_set(result).terms(),
            [NormalizedDomainTerm::Declared("Outer".to_owned())]
        );
    }

    #[test]
    fn named_machine_identity_normalizes_binders_and_collapses_predicate_only_results() {
        let mut program = TypedTrees::default();
        let i32_type = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("i32"),
            });
        let positive = program.semantic_domains.intern("Positive");
        let predicate_result = constrained(
            &mut program,
            i32_type,
            [declared_with_metadata(
                "Positive",
                SymbolHandle::invalid(),
                positive,
                DomainPredicateBody::Present,
                DomainSemanticRoles::default(),
                Vec::new(),
            )],
        );
        let saturating_result = constrained(
            &mut program,
            i32_type,
            [TypeConstraintNode::ArithmeticDomain(
                omega_core::arithmetic::ArithmeticDomain::Saturating,
            )],
        );
        let unqualified = generic_machine(
            &mut program,
            SymbolHandle::from_arena_index(61),
            SymbolHandle::from_arena_index(71),
            "T",
            i32_type,
        );
        let predicate_only = generic_machine(
            &mut program,
            SymbolHandle::from_arena_index(62),
            SymbolHandle::from_arena_index(72),
            "Renamed",
            predicate_result,
        );
        let saturating = generic_machine(
            &mut program,
            SymbolHandle::from_arena_index(63),
            SymbolHandle::from_arena_index(73),
            "AnotherName",
            saturating_result,
        );

        let unqualified_identity = program
            .normalized_machine_overload_identity(&unqualified)
            .expect("machine has an entry");
        let predicate_identity = program
            .normalized_machine_overload_identity(&predicate_only)
            .expect("machine has an entry");
        let saturating_identity = program
            .normalized_machine_overload_identity(&saturating)
            .expect("machine has an entry");

        assert_eq!(unqualified_identity, predicate_identity);
        assert_ne!(unqualified_identity, saturating_identity);
        assert_eq!(unqualified_identity.path(), "I32::from_value");
        assert!(unqualified_identity.result_dispatch().is_empty());
        assert_eq!(
            saturating_identity.result_dispatch().identity(),
            "arithmetic:Saturating"
        );
        assert!(unqualified_identity.parameters().contains("$T0"));
        assert!(!unqualified_identity.identity().is_empty());
    }
}
