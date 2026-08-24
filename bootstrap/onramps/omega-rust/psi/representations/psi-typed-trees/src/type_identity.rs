//! Deterministic semantic identity for typed type references.
//!
//! Diagnostic rendering is deliberately not an identity oracle. In
//! particular, a domain conjunction is commutative and idempotent, while its
//! authored spelling has an order and may repeat terms. This module owns the
//! canonical form used by type equality, specialization, and published plan
//! identities (decision 19 / DOM4).

use crate::TypedTrees;
use crate::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use crate::types::{
    DomainConstraint, FixedArrayLength, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use std::fmt;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
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
            &TypeIdentityContext {
                binders,
                substitutions: &[],
                qualification: TypeIdentityQualification::Ordinary,
            },
        ))
    }

    /// Canonical type identity for a package graph. Every non-binder nominal
    /// carries both its stable declaration path and its exact source owner:
    /// the managed package-key digest, the toolchain marker, or an explicit
    /// unresolved marker. Ordinary type identity deliberately remains local
    /// to one compilation and is unchanged by this stronger form.
    pub fn package_qualified_type_identity(
        &self,
        type_reference: TypeReferenceHandle,
    ) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            type_reference,
            &TypeIdentityContext {
                qualification: TypeIdentityQualification::PackageQualified,
                ..TypeIdentityContext::default()
            },
        ))
    }

    /// Binder-aware package-graph identity. Binder substitutions are applied
    /// before owner qualification, preserving alpha-normalization without
    /// falsely assigning a package owner to a local telescope variable.
    pub fn package_qualified_type_identity_with_binders(
        &self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
    ) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            type_reference,
            &TypeIdentityContext {
                binders,
                substitutions: &[],
                qualification: TypeIdentityQualification::PackageQualified,
            },
        ))
    }

    /// Binder-aware identity after replacing exact type-parameter symbols with
    /// concrete type references. This is used when a closed structural
    /// instance retains the semantic identity of an erased field whose type is
    /// intentionally absent from the executable layout vocabulary.
    pub fn normalized_type_identity_with_binders_and_substitutions(
        &self,
        type_reference: TypeReferenceHandle,
        binders: &[(SymbolHandle, String)],
        substitutions: &[(SymbolHandle, TypeReferenceHandle)],
    ) -> NormalizedTypeIdentity {
        NormalizedTypeIdentity(normalize_type_reference(
            self,
            type_reference,
            &TypeIdentityContext {
                binders,
                substitutions,
                qualification: TypeIdentityQualification::Ordinary,
            },
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

    /// Resolve one canonical machine-overload identity exactly. A missing or
    /// duplicate identity returns `None`; callers must not fall back to a
    /// short machine spelling.
    pub fn machine_by_normalized_overload_identity(
        &self,
        identity: &str,
    ) -> Option<&crate::machine::Machine> {
        let mut matches = self.machines().iter().filter(|machine| {
            self.normalized_machine_overload_identity(machine)
                .is_some_and(|candidate| candidate.identity() == identity)
        });
        let machine = matches.next()?;
        matches.next().is_none().then_some(machine)
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

    /// Canonical identity of one compile-time machine-parameter callable.
    /// The declaring machine and all of its generic binders participate in
    /// the identity, followed by binders authored directly on the callable
    /// contract. This is the requirement-side analogue of a top-level named
    /// machine overload; consumers must not reconstruct it from the parameter
    /// spelling alone.
    pub fn normalized_machine_parameter_overload_identity(
        &self,
        declaring_machine: &crate::machine::Machine,
        requirement: &crate::signature::StateSignature,
    ) -> NormalizedNamedCallableIdentity {
        let mut type_parameters = self.machine_type_parameters(declaring_machine).to_vec();
        type_parameters.extend_from_slice(self.state_signature_type_parameters(requirement));
        self.normalized_named_callable_identity(
            &format!("{}::{}", declaring_machine.name, requirement.name),
            declaring_machine.symbol,
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
        | TypeReferenceNode::ConstExpression(_)
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
                    classification: constituent_definition.classification,
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
    semantic_id: psi_language_semantics::SemanticDomainId,
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
    substitutions: &'binders [(SymbolHandle, TypeReferenceHandle)],
    qualification: TypeIdentityQualification,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum TypeIdentityQualification {
    #[default]
    Ordinary,
    PackageQualified,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PackageQualifiedNominalOwner {
    Package([u8; 32]),
    Toolchain,
    Unresolved,
}

impl PackageQualifiedNominalOwner {
    fn encode(self) -> String {
        match self {
            Self::Package(digest) => byte_atom("package-owner", &digest),
            Self::Toolchain => "toolchain-owner".to_owned(),
            Self::Unresolved => "unresolved-owner".to_owned(),
        }
    }
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
        // Open index expressions are parsed before their enclosing const
        // telescope is available, so a direct binder leaf can still carry an
        // invalid expression symbol. Binder-aware template identity recovers
        // that exact local by its symbol-table name; concrete qualified names
        // never match this one-segment route.
        if !symbol.is_valid()
            && !fallback.contains("::")
            && let Some((_, replacement)) = self.binders.iter().find(|(candidate, _)| {
                candidate.is_valid() && program.symbols.name(*candidate) == fallback
            })
        {
            return replacement.clone();
        }
        if symbol.is_valid() {
            let path = program.symbols.display_path(symbol, "::");
            if !path.is_empty() {
                return self.qualify_non_binder_name(program, symbol, path);
            }
        }
        self.qualify_non_binder_name(program, symbol, fallback.to_owned())
    }

    fn qualify_non_binder_name(
        &self,
        program: &TypedTrees,
        symbol: SymbolHandle,
        path: String,
    ) -> String {
        if self.qualification == TypeIdentityQualification::Ordinary {
            return path;
        }
        let owner = if let Some(package) = program.symbols.symbol_package_identity(symbol) {
            PackageQualifiedNominalOwner::Package(package.digest())
        } else if program.symbols.symbol_source_origin(symbol)
            == Some(psi_source::SourceOrigin::Toolchain)
        {
            PackageQualifiedNominalOwner::Toolchain
        } else {
            PackageQualifiedNominalOwner::Unresolved
        };
        package_qualified_nominal_name(owner, &path)
    }
}

fn package_qualified_nominal_name(owner: PackageQualifiedNominalOwner, path: &str) -> String {
    compound("nominal", [owner.encode(), atom("path", path)])
}

fn normalize_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    if let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
        && let Some((_, replacement)) = context
            .substitutions
            .iter()
            .rev()
            .find(|(candidate, _)| candidate == symbol)
        && *replacement != type_reference
    {
        return normalize_type_reference(program, *replacement, context);
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime: _,
        } => compound(
            match access {
                psi_language_core::ReferenceAccess::Shared => "ref",
                psi_language_core::ReferenceAccess::Mutable => "ref-mut",
                psi_language_core::ReferenceAccess::WriteOnly => "ref-write",
            },
            [normalize_type_reference(program, *referee, context)],
        ),
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            let (base, mut all_constraints) =
                normalize_constrained_base(program, *base_type, context);
            all_constraints.extend(normalized_constraints(program, *constraints, context));
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
        TypeReferenceNode::ConstExpression(expression) => compound(
            "index-expression",
            [normalize_index_expression(program, *expression, context)],
        ),
        TypeReferenceNode::DynamicTrait {
            symbol,
            name,
            conformance,
            conformance_carrier,
            conformance_name,
        } => {
            let mut identity = vec![atom("name", &context.name(program, *symbol, name.as_str()))];
            if let (Some(carrier), Some(selection)) =
                (conformance_carrier.as_ref(), conformance_name.as_ref())
            {
                let fallback = format!("{carrier}::{selection}");
                identity.push(atom(
                    "conformance",
                    &context.name(
                        program,
                        conformance.unwrap_or_else(SymbolHandle::invalid),
                        &fallback,
                    ),
                ));
            }
            compound("dynamic-trait", identity)
        }
        TypeReferenceNode::Named { symbol, name } => compound(
            "named",
            [atom("name", &context.name(program, *symbol, name.as_str()))],
        ),
        TypeReferenceNode::Unit => "unit".to_owned(),
    }
}

fn normalize_index_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    context: &TypeIdentityContext<'_>,
) -> String {
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let fallback = members
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            atom(
                "const-name",
                &context.name(program, path.symbol, fallback.as_str()),
            )
        }
        ExpressionNode::Binary(binary) => {
            let Some(selection) = open_index_operation_selection(program, expression) else {
                return compound(
                    index_binary_operator_name(binary.operator),
                    [
                        normalize_index_expression(program, binary.left, context),
                        normalize_index_expression(program, binary.right, context),
                    ],
                );
            };
            let mut operands = Vec::new();
            collect_licensed_ac_operands(
                program,
                binary.left,
                binary.operator,
                selection,
                context,
                &mut operands,
            );
            collect_licensed_ac_operands(
                program,
                binary.right,
                binary.operator,
                selection,
                context,
                &mut operands,
            );
            operands.sort();
            compound(
                index_binary_operator_name(binary.operator),
                std::iter::once(atom(
                    "operation",
                    &open_index_operation_identity(program, selection, context),
                ))
                .chain(std::iter::once(atom(
                    "algebra",
                    &open_index_algebra_identity(program, selection, context),
                )))
                .chain(operands),
            )
        }
        ExpressionNode::Integer(value) => atom("integer", &value.to_string()),
        ExpressionNode::Unary(unary) => compound(
            match unary.operator {
                crate::expression::UnaryOperator::BitwiseNot => "bitwise-not",
                crate::expression::UnaryOperator::LogicalNot => "logical-not",
            },
            [normalize_index_expression(program, unary.operand, context)],
        ),
        // These shapes are rejected by PDI3 index validation. Keep their
        // provisional identity structural and independent of diagnostic
        // rendering so even a rejected tree never makes display text an
        // equality oracle.
        ExpressionNode::Boolean(value) => atom("boolean", &value.to_string()),
        ExpressionNode::Float(value) => atom("float", &value.to_string()),
        ExpressionNode::String(value) => byte_atom("string", value),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Cast(_)
        | ExpressionNode::Call(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::ZeroValue(_) => "unsupported-index-expression".to_owned(),
    }
}

fn open_index_operation_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<&crate::typed_trees::OpenIndexOperationSelection> {
    program
        .open_index_normalizations
        .iter()
        .flat_map(|normalization| &normalization.operations)
        .find(|selection| selection.expression == expression)
}

fn open_index_algebra_identity(
    program: &TypedTrees,
    selection: &crate::typed_trees::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> String {
    if context.qualification == TypeIdentityQualification::Ordinary {
        return format!(
            "{}::{} as {}",
            program.symbols.display_path(selection.algebra_trait, "::"),
            selection.algebra_requirement,
            selection.algebra_alias.as_deref().unwrap_or("<default>")
        );
    }
    compound(
        "open-index-algebra",
        [
            atom(
                "provider",
                &context.name(program, selection.provider, "<unresolved-provider>"),
            ),
            atom(
                "trait",
                &context.name(
                    program,
                    selection.algebra_trait,
                    "<unresolved-algebra-trait>",
                ),
            ),
            atom("requirement", &selection.algebra_requirement),
            atom(
                "alias",
                selection.algebra_alias.as_deref().unwrap_or("<default>"),
            ),
        ],
    )
}

fn open_index_operation_identity(
    program: &TypedTrees,
    selection: &crate::typed_trees::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
) -> String {
    if context.qualification == TypeIdentityQualification::Ordinary {
        return selection.operation_contract_identity.clone();
    }
    compound(
        "open-index-operation",
        [
            atom(
                "symbol",
                &context.name(
                    program,
                    selection.operator,
                    selection.operation_contract_identity.as_str(),
                ),
            ),
            atom("contract", &selection.operation_contract_identity),
        ],
    )
}

fn same_open_index_ac_authority(
    left: &crate::typed_trees::OpenIndexOperationSelection,
    right: &crate::typed_trees::OpenIndexOperationSelection,
) -> bool {
    left.operator == right.operator
        && left.operation_contract_identity == right.operation_contract_identity
        && left.algebra_trait == right.algebra_trait
        && left.algebra_requirement == right.algebra_requirement
        && left.algebra_alias == right.algebra_alias
}

fn collect_licensed_ac_operands(
    program: &TypedTrees,
    expression: ExpressionHandle,
    operator: BinaryOperator,
    authority: &crate::typed_trees::OpenIndexOperationSelection,
    context: &TypeIdentityContext<'_>,
    operands: &mut Vec<String>,
) {
    if let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
        && binary.operator == operator
        && open_index_operation_selection(program, expression)
            .is_some_and(|selection| same_open_index_ac_authority(authority, selection))
    {
        collect_licensed_ac_operands(program, binary.left, operator, authority, context, operands);
        collect_licensed_ac_operands(
            program,
            binary.right,
            operator,
            authority,
            context,
            operands,
        );
    } else {
        operands.push(normalize_index_expression(program, expression, context));
    }
}

fn index_binary_operator_name(operator: BinaryOperator) -> &'static str {
    match operator {
        BinaryOperator::Add => "add",
        BinaryOperator::And => "and",
        BinaryOperator::BitwiseAnd => "bitwise-and",
        BinaryOperator::BitwiseOr => "bitwise-or",
        BinaryOperator::BitwiseXor => "bitwise-xor",
        BinaryOperator::Divide => "divide",
        BinaryOperator::Equal => "equal",
        BinaryOperator::Greater => "greater",
        BinaryOperator::GreaterOrEqual => "greater-or-equal",
        BinaryOperator::Less => "less",
        BinaryOperator::LessOrEqual => "less-or-equal",
        BinaryOperator::Modulo => "modulo",
        BinaryOperator::Multiply => "multiply",
        BinaryOperator::NotEqual => "not-equal",
        BinaryOperator::Or => "or",
        BinaryOperator::ShiftLeft => "shift-left",
        BinaryOperator::ShiftRight => "shift-right",
        BinaryOperator::Subtract => "subtract",
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
            all_constraints.extend(normalized_constraints(program, *constraints, context));
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
    context: &TypeIdentityContext<'_>,
) -> Vec<NormalizedConstraint> {
    program
        .type_reference_table
        .constraints(constraints)
        .iter()
        .map(|constraint| match constraint {
            TypeConstraintNode::Named(name) => {
                NormalizedConstraint::Named(name.as_str().to_owned())
            }
            TypeConstraintNode::Range { minimum, maximum } => {
                let (minimum, maximum) =
                    if context.qualification == TypeIdentityQualification::PackageQualified {
                        (
                            normalize_index_expression(program, *minimum, context),
                            normalize_index_expression(program, *maximum, context),
                        )
                    } else {
                        (
                            program.expression_table.display_name(*minimum),
                            program.expression_table.display_name(*maximum),
                        )
                    };
                NormalizedConstraint::Range { minimum, maximum }
            }
            TypeConstraintNode::ArithmeticDomain(domain) => {
                NormalizedConstraint::Arithmetic(domain.name().to_owned())
            }
            TypeConstraintNode::Domain(domain) => NormalizedConstraint::DeclaredDomain(
                normalized_declared_domain_identity(program, domain, context),
            ),
        })
        .collect()
}

fn normalized_declared_domain_identity(
    program: &TypedTrees,
    domain: &DomainConstraint,
    context: &TypeIdentityContext<'_>,
) -> String {
    let ordinary_name = declared_domain_identity(program, domain);
    if context.qualification == TypeIdentityQualification::Ordinary {
        return ordinary_name;
    }
    compound(
        "declared-domain",
        std::iter::once(atom(
            "name",
            &context.name(program, domain.symbol, &ordinary_name),
        ))
        .chain(
            domain
                .arguments
                .iter()
                .map(|argument| normalize_type_reference(program, *argument, context)),
        ),
    )
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

fn byte_atom(tag: &str, value: &[u8]) -> String {
    let mut output = String::with_capacity(tag.len() + value.len().saturating_mul(2) + 24);
    output.push_str(tag);
    output.push('(');
    output.push_str(&value.len().to_string());
    output.push(':');
    for byte in value {
        output.push_str(&format!("{byte:02x}"));
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
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::{
        NormalizedDomainTerm, NormalizedTypeIdentity, PackageQualifiedNominalOwner,
        package_qualified_nominal_name,
    };
    use crate::TypedTrees;
    use crate::domain::{DomainAliasConstituent, DomainAliasDefinition, DomainDefinition};
    use crate::expression::{
        BinaryExpression, BinaryOperator, Expression, ExpressionHandle, ExpressionNode, NamePath,
    };
    use crate::name::Identifier;
    use crate::typed_trees::{OpenIndexNormalization, OpenIndexOperationSelection};
    use crate::types::{DomainConstraint, TypeConstraintNode, TypeReferenceNode};
    use psi_language_core::operator_spelling::OperatorSpelling;
    use psi_language_semantics::{
        DomainEstablishmentRoute, DomainPredicateBody, DomainSemanticRoles, ReferenceAccess,
        SemanticDomainId,
    };
    use psi_source::{SourceMap, SourceOrigin, SourceSpan, Span};
    use psi_symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

    #[test]
    fn same_spelled_package_nominals_have_distinct_qualified_identities() {
        let path = "shared::Packet";
        let first =
            package_qualified_nominal_name(PackageQualifiedNominalOwner::Package([0x11; 32]), path);
        let second =
            package_qualified_nominal_name(PackageQualifiedNominalOwner::Package([0x22; 32]), path);

        assert_ne!(first, second);
        assert!(first.contains(path));
        assert!(second.contains(path));
        assert!(first.contains(&"11".repeat(32)), "{first}");
        assert!(second.contains(&"22".repeat(32)), "{second}");
    }

    #[test]
    fn package_qualified_binder_identity_is_alpha_normalized_without_an_owner() {
        let mut program = TypedTrees::default();
        let first_symbol = SymbolHandle::from_arena_index(81);
        let second_symbol = SymbolHandle::from_arena_index(82);
        let first = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: first_symbol,
                name: Identifier::generated("Element"),
            });
        let second = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: second_symbol,
                name: Identifier::generated("RenamedElement"),
            });

        let first = program.package_qualified_type_identity_with_binders(
            first,
            &[(first_symbol, "$T0".to_owned())],
        );
        let second = program.package_qualified_type_identity_with_binders(
            second,
            &[(second_symbol, "$T0".to_owned())],
        );

        assert_eq!(first, second);
        assert_eq!(first.as_str(), "named(name($T0))");
        assert!(!first.as_str().contains("owner"));
    }

    #[test]
    fn package_qualified_nominals_mark_toolchain_and_unresolved_owners() {
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("toolchain/types.omg"),
                String::from("Packet"),
                PathBuf::from("toolchain"),
                None,
                SourceOrigin::Toolchain,
            )
            .source_id;
        let mut symbols = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let packet = SymbolTableBuilder::child_handles(symbols.insert_children(
            root,
            [(
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 6))),
            )],
        ))
        .next()
        .expect("toolchain nominal symbol");
        let mut program = TypedTrees {
            symbols: symbols.finish(),
            ..TypedTrees::default()
        };
        let toolchain = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: packet,
                name: Identifier::generated("ignored-diagnostic-name"),
            });
        let unresolved = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("Pending"),
            });

        let toolchain = program.package_qualified_type_identity(toolchain);
        let unresolved = program.package_qualified_type_identity(unresolved);
        assert!(
            toolchain.as_str().contains("toolchain-owner"),
            "{toolchain}"
        );
        assert!(toolchain.as_str().contains("Packet"), "{toolchain}");
        assert!(
            !toolchain.as_str().contains("ignored-diagnostic-name"),
            "{toolchain}"
        );
        assert!(
            unresolved.as_str().contains("unresolved-owner"),
            "{unresolved}"
        );
        assert!(unresolved.as_str().contains("Pending"), "{unresolved}");
    }

    #[test]
    fn package_qualified_declared_domains_normalize_their_owner_and_arguments() {
        let mut program = TypedTrees::default();
        let carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("Carrier"),
            });
        let argument = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("Argument"),
            });
        let constraints =
            program
                .type_reference_table
                .insert_constraints([TypeConstraintNode::Domain(DomainConstraint {
                    name: Identifier::generated("Policy"),
                    arguments: vec![argument],
                    ..DomainConstraint::default()
                })]);
        let constrained = program
            .type_reference_table
            .insert(TypeReferenceNode::Constrained {
                base_type: carrier,
                constraints,
            });

        let identity = program.package_qualified_type_identity(constrained);
        assert!(identity.as_str().contains("Carrier"), "{identity}");
        assert!(identity.as_str().contains("Policy"), "{identity}");
        assert!(identity.as_str().contains("Argument"), "{identity}");
        assert_eq!(identity.as_str().matches("unresolved-owner").count(), 3);
    }

    #[test]
    fn package_qualified_open_index_authority_qualifies_every_nominal_symbol() {
        let mut program = TypedTrees::default();
        let expression = program
            .expression_table
            .insert_tree(&Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Boolean(true),
                operator: BinaryOperator::Add,
                right: Expression::Boolean(false),
            })));
        program
            .open_index_normalizations
            .push(OpenIndexNormalization {
                expression,
                index_type: psi_arena::Handle::invalid(),
                operations: vec![OpenIndexOperationSelection {
                    expression,
                    spelling: OperatorSpelling::Add,
                    operator: SymbolHandle::invalid(),
                    operation_contract_identity: "Index::add(i32,i32)->i32".to_owned(),
                    provider: SymbolHandle::invalid(),
                    algebra_trait: SymbolHandle::invalid(),
                    algebra_requirement: "add".to_owned(),
                    algebra_alias: Some("Canonical".to_owned()),
                }],
                normalizer_version: 1,
            });
        let type_reference = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(expression));

        let identity = program.package_qualified_type_identity(type_reference);
        assert!(
            identity.as_str().contains("open-index-operation"),
            "{identity}"
        );
        assert!(
            identity.as_str().contains("open-index-algebra"),
            "{identity}"
        );
        assert_eq!(identity.as_str().matches("unresolved-owner").count(), 3);
    }

    #[test]
    fn reference_access_modes_have_distinct_normalized_identities() {
        let mut program = TypedTrees::default();
        let referee = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated("u8"),
            });
        let shared = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: ReferenceAccess::Shared,
                lifetime: None,
            });
        let mutable = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: ReferenceAccess::Mutable,
                lifetime: None,
            });
        let write_only = program
            .type_reference_table
            .insert(TypeReferenceNode::Reference {
                referee,
                access: ReferenceAccess::WriteOnly,
                lifetime: None,
            });

        let shared = program.normalized_type_identity(shared);
        let mutable = program.normalized_type_identity(mutable);
        let write_only = program.normalized_type_identity(write_only);
        assert!(shared.as_str().starts_with("ref("), "{shared:?}");
        assert!(mutable.as_str().starts_with("ref-mut("), "{mutable:?}");
        assert!(
            write_only.as_str().starts_with("ref-write("),
            "{write_only:?}"
        );
        assert_ne!(shared, mutable);
        assert_ne!(shared, write_only);
        assert_ne!(mutable, write_only);
    }

    fn declared(name: &str, semantic_id: SemanticDomainId) -> TypeConstraintNode {
        TypeConstraintNode::Domain(DomainConstraint {
            name: Identifier::generated(name),
            arguments: Vec::new(),
            symbol: SymbolHandle::invalid(),
            semantic_id,
            classification: None,
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
            classification: None,
            predicate_body,
            semantic_roles,
            establishment_routes,
        })
    }

    fn constrained(
        program: &mut TypedTrees,
        base_type: psi_arena::Handle<TypeReferenceNode>,
        constraints: impl IntoIterator<Item = TypeConstraintNode>,
    ) -> psi_arena::Handle<TypeReferenceNode> {
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
        return_type: psi_arena::Handle<TypeReferenceNode>,
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
    fn open_index_identity_is_structural_not_diagnostic_display() {
        let mut program = TypedTrees::default();
        let expression = |left: &str, right: &str| {
            Expression::Binary(Box::new(BinaryExpression {
                left: Expression::Name(NamePath::unresolved_from_iter([Identifier::generated(
                    left,
                )])),
                operator: BinaryOperator::Divide,
                right: Expression::Name(NamePath::unresolved_from_iter([Identifier::generated(
                    right,
                )])),
            }))
        };
        let a_over_b = program.expression_table.insert_tree(&expression("A", "B"));
        let same_a_over_b = program.expression_table.insert_tree(&expression("A", "B"));
        let b_over_a = program.expression_table.insert_tree(&expression("B", "A"));
        let a_over_b = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(a_over_b));
        let same_a_over_b = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(same_a_over_b));
        let b_over_a = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(b_over_a));

        assert_eq!(
            program.normalized_type_identity(a_over_b),
            program.normalized_type_identity(same_a_over_b)
        );
        assert_ne!(
            program.normalized_type_identity(a_over_b),
            program.normalized_type_identity(b_over_a)
        );
    }

    #[test]
    fn licensed_open_index_identity_flattens_and_sorts_exact_ac_operation() {
        fn name(value: &str) -> Expression {
            Expression::Name(NamePath::unresolved_from_iter([Identifier::generated(
                value,
            )]))
        }
        fn add(left: Expression, right: Expression) -> Expression {
            Expression::Binary(Box::new(BinaryExpression {
                left,
                operator: BinaryOperator::Add,
                right,
            }))
        }
        fn binary_nodes(
            program: &TypedTrees,
            expression: ExpressionHandle,
            output: &mut Vec<ExpressionHandle>,
        ) {
            let ExpressionNode::Binary(binary) = program.expression_table.expression(expression)
            else {
                return;
            };
            output.push(expression);
            binary_nodes(program, binary.left, output);
            binary_nodes(program, binary.right, output);
        }
        fn license(program: &mut TypedTrees, expression: ExpressionHandle, contract: &str) {
            let mut nodes = Vec::new();
            binary_nodes(program, expression, &mut nodes);
            let operations = nodes
                .into_iter()
                .map(|expression| OpenIndexOperationSelection {
                    expression,
                    spelling: OperatorSpelling::Add,
                    operator: SymbolHandle::from_arena_index(71),
                    operation_contract_identity: contract.to_owned(),
                    provider: SymbolHandle::from_arena_index(72),
                    algebra_trait: SymbolHandle::from_arena_index(73),
                    algebra_requirement: "add".to_owned(),
                    algebra_alias: Some("Canonical".to_owned()),
                })
                .collect();
            program
                .open_index_normalizations
                .push(OpenIndexNormalization {
                    expression,
                    index_type: psi_arena::Handle::invalid(),
                    operations,
                    normalizer_version: 1,
                });
        }

        let mut program = TypedTrees::default();
        let left_associated = program
            .expression_table
            .insert_tree(&add(add(name("A"), name("B")), name("C")));
        let reordered = program
            .expression_table
            .insert_tree(&add(name("C"), add(name("B"), name("A"))));
        let different_authority = program
            .expression_table
            .insert_tree(&add(add(name("A"), name("B")), name("C")));
        license(&mut program, left_associated, "IndexAlgebra::plus");
        license(&mut program, reordered, "IndexAlgebra::plus");
        license(&mut program, different_authority, "OtherIndexAlgebra::plus");
        let left_associated = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(left_associated));
        let reordered = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(reordered));
        let different_authority = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(different_authority));

        assert_eq!(
            program.normalized_type_identity(left_associated),
            program.normalized_type_identity(reordered)
        );
        assert_ne!(
            program.normalized_type_identity(left_associated),
            program.normalized_type_identity(different_authority)
        );
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
                    psi_numerics::arithmetic::ArithmeticDomain::Saturating,
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
                    psi_numerics::arithmetic::ArithmeticDomain::Saturating,
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
                psi_numerics::arithmetic::ArithmeticDomain::Saturating,
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
