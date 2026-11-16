//! Reads declaration facts for review from the source the package judgment
//! accepted. It infers no behavior from syntax: callable behavior is read
//! separately from the retained module, and an unhandled declaration form makes
//! the review incomplete instead of vanishing from its inventory.

use super::PackageReviewInput;
use crate::record::{
    PackagePolicyDataShape, PackagePolicyDomainShape, PackageReviewContractBinaryOperator,
    PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewContractOperatorMeaning, PackageReviewDataField, PackageReviewDataKind,
    PackageReviewDataMember, PackageReviewDataProperties, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewSemanticDependencyExposure,
    PackageReviewToolchainSourceIdentity, PackageReviewTypeIdentity,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataField, DataMember};
use typed_trees::domain::DomainDefinition;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

/// Names in a reviewed clause come from its retained declaration, never from
/// their spelling. Domain predicates and callable clauses share this projection.
#[derive(Clone, Copy)]
pub(super) struct ContractScope<'a> {
    pub owner: SymbolHandle,
    pub subject: Option<(SymbolHandle, TypeReferenceHandle)>,
    pub parameters: arena::RootRows<'a, typed_trees::signature::StateParameter>,
    pub result: Option<TypeReferenceHandle>,
}

pub(super) fn unavailable(what: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package review cannot retain {what} from the accepted declaration"
    ))]
}

/// Both accepted source owners prepare their exact toolchain roster once.
/// Nominal projection keeps that owner with the declaration; a later source
/// context may not substitute its source numbers or diagnostic spelling.
pub(super) fn nominal_identity(
    typed: &TypedTrees,
    toolchain_sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
    let symbols = &typed.symbols;
    if !symbols.symbols().nodes().is_valid(symbol) {
        return Err(unavailable("a stale nominal declaration"));
    }
    let owner = if let Some(package) = symbols.symbol_package_identity(symbol) {
        PackageReviewNominalOwner::Package(package)
    } else {
        let file = symbols
            .symbol_provenance_source_span(symbol)
            .and_then(|span| symbols.source_file(span))
            .filter(|file| file.origin.is_toolchain())
            .ok_or_else(|| unavailable("a declaration without package or toolchain custody"))?;
        // Source custody already prepared this roster once. Nominal
        // projection borrows it rather than hashing source per row.
        let position = toolchain_sources
            .binary_search_by_key(&file.source_id.0, |(source, _)| source.0)
            .map_err(|_| unavailable("a toolchain declaration without its exact source"))?;
        PackageReviewNominalOwner::ToolchainSource(PackageReviewToolchainSourceIdentity {
            digest: toolchain_sources[position].1,
        })
    };
    let path = symbols.display_path(symbol, "::");
    if path.is_empty() {
        return Err(unavailable("a declaration without a stable path"));
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

impl<'a> PackageReviewInput<'a> {
    pub(super) fn typed(&self) -> &'a TypedTrees {
        self.candidate()
    }

    pub(super) fn owns(
        &self,
        symbol: SymbolHandle,
        package: PackageKeyIdentity,
    ) -> Result<bool, Vec<Diagnostic>> {
        typed_trees::source_subjects::contains(
            self.typed(),
            symbol,
            &[Some(package)],
            source::DependencyScope::Product,
        )
        .map_err(unavailable)
    }

    pub(super) fn nominal(
        &self,
        symbol: SymbolHandle,
    ) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
        nominal_identity(
            self.typed(),
            self.compilation().sources().exact_toolchain_sources(),
            symbol,
        )
    }

    pub(super) fn type_identity(
        &self,
        reference: TypeReferenceHandle,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        self.type_identity_with_lifetimes(reference, None)
    }

    fn type_identity_with_lifetimes(
        &self,
        reference: TypeReferenceHandle,
        lifetime_scope: Option<(
            SymbolHandle,
            &[(typed_trees::type_identity::LifetimeParameterIdentity, usize)],
        )>,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        if !reference.is_valid() {
            return Err(unavailable("an absent declared type"));
        }
        self.require_type_dependencies(
            reference,
            PackageReviewSemanticDependencyExposure::PublicInterface,
        )?;
        self.exact_signature_type_identity(reference, lifetime_scope)
    }

    pub(super) fn type_identity_for_private_contract(
        &self,
        reference: TypeReferenceHandle,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        self.require_type_dependencies(
            reference,
            PackageReviewSemanticDependencyExposure::PrivateImplementation,
        )?;
        self.exact_signature_type_identity(reference, None)
    }

    fn exact_signature_type_identity(
        &self,
        reference: TypeReferenceHandle,
        lifetime_scope: Option<(
            SymbolHandle,
            &[(typed_trees::type_identity::LifetimeParameterIdentity, usize)],
        )>,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        let canonical = self
            .typed()
            .exact_owner_type_identity(typed_trees::type_identity::ExactOwnerTypeIdentityRequest {
                type_reference: reference,
                lifetime_scope,
                binders: &[],
                substitutions: &[],
                exact_toolchain_sources: self.compilation().sources().exact_toolchain_sources(),
            })
            .and_then(|identity| self.typed().type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| unavailable("a type without a package-qualified identity"))?;
        Ok(PackageReviewTypeIdentity { canonical })
    }

    fn require_type_dependencies(
        &self,
        reference: TypeReferenceHandle,
        exposure: PackageReviewSemanticDependencyExposure,
    ) -> Result<(), Vec<Diagnostic>> {
        let package = self
            .compilation()
            .sources()
            .package_identity()
            .ok_or_else(|| unavailable("a type outside the reviewed package subject"))?;
        let symbols = &self.typed().symbols;
        for symbol in
            typed_trees::references::collect_type_symbol_dependencies(self.typed(), reference)
        {
            if !symbols.symbols().nodes().is_valid(symbol) {
                return Err(unavailable("a stale declaration in a public type"));
            }
            if symbols.get(symbol).kind == symbols::SymbolKind::BuiltinType {
                continue;
            }
            match symbols.symbol_package_identity(symbol) {
                Some(owner) if owner != package => {
                    return Err(unavailable(
                        "an imported public type's semantic dependency contract",
                    ));
                }
                Some(_) if exposure == PackageReviewSemanticDependencyExposure::PublicInterface => {
                    let retained = self
                        .typed()
                        .data_definition_by_symbol(symbol)
                        .is_some_and(|definition| definition.is_public)
                        || self
                            .typed()
                            .domain_definition(symbol)
                            .is_some_and(|definition| definition.is_public);
                    if !retained {
                        return Err(unavailable(
                            "a private semantic dependency of a public type",
                        ));
                    }
                }
                _ => {
                    self.nominal(symbol)?;
                }
            }
        }
        Ok(())
    }

    fn field(
        &self,
        field: &DataField,
        lifetimes: (
            SymbolHandle,
            &[(typed_trees::type_identity::LifetimeParameterIdentity, usize)],
        ),
    ) -> Result<PackageReviewDataField, Vec<Diagnostic>> {
        Ok(PackageReviewDataField {
            identity: field.identity,
            name: field.name.as_str().to_owned(),
            relevance: field.relevance,
            type_identity: self
                .type_identity_with_lifetimes(field.type_reference, Some(lifetimes))?,
        })
    }

    pub(super) fn public_data(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<PackagePolicyDataShape>, Vec<Diagnostic>> {
        let mut rows = Vec::new();
        for definition in self
            .typed()
            .data_definitions()
            .iter()
            .filter(|row| row.is_public)
        {
            if !self.owns(definition.symbol, package)? {
                continue;
            }
            // Lifetime binders are retained by this declaration's exact
            // owner and count; field type identities name the ordinals in
            // that scope. Other generic and cleanup contracts
            // still need a complete projection before they can be reviewed.
            if !definition.type_parameters.is_empty()
                || !definition.conformance_bounds.is_empty()
                || definition.quotient.is_some()
                || definition.cleanup_selection.is_some()
                || definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            {
                return Err(unavailable(
                    "this data declaration's complete generic or cleanup signature",
                ));
            }
            let lifetimes = (0..definition.lifetime_parameters.len())
                .map(|ordinal| {
                    (
                        typed_trees::type_identity::LifetimeParameterIdentity {
                            declaration: definition.symbol,
                            ordinal,
                        },
                        ordinal,
                    )
                })
                .collect::<Vec<_>>();
            let mut members = Vec::new();
            for member in self.typed().data_members(definition) {
                members.push(match member {
                    DataMember::Field(field) => PackageReviewDataMember::Field(
                        self.field(field, (definition.symbol, &lifetimes))?,
                    ),
                    DataMember::Variant(variant) => {
                        let mut retired = variant.retired_payload_identities.clone();
                        retired.sort_unstable();
                        retired.dedup();
                        PackageReviewDataMember::Variant {
                            identity: variant.identity,
                            name: variant.name.as_str().to_owned(),
                            payload: self
                                .typed()
                                .data_payload_fields(variant)
                                .iter()
                                .map(|field| self.field(field, (definition.symbol, &lifetimes)))
                                .collect::<Result<_, _>>()?,
                            retired_payload_identities: retired,
                        }
                    }
                });
            }
            let mut retired_identities = definition.retired_identities.clone();
            retired_identities.sort_unstable();
            retired_identities.dedup();
            if !definition.requires_facts.is_empty() {
                return Err(unavailable("data invariant facts"));
            }
            rows.push(PackagePolicyDataShape {
                identity: self.nominal(definition.symbol)?,
                kind: PackageReviewDataKind::Ordinary,
                supply: definition.supply_mode,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters: Vec::new(),
                properties: PackageReviewDataProperties {
                    multiplicity: definition.properties.multiplicity,
                    carry: definition.properties.carry,
                },
                zero_gated: definition.zero_gated,
                invariants: Vec::new(),
                retired_identities,
                members,
            });
        }
        rows.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(rows)
    }

    pub(super) fn public_domains(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<PackagePolicyDomainShape>, Vec<Diagnostic>> {
        let mut rows = Vec::new();
        for definition in self
            .typed()
            .domain_definitions()
            .iter()
            .filter(|row| row.is_public)
        {
            if !self.owns(definition.symbol, package)? {
                continue;
            }
            if !definition.type_parameters.is_empty()
                || !definition.index_arguments.is_empty()
                || definition.alias.is_some()
                || definition.classification.is_some()
                || definition.cleanup_selection.is_some()
                || !definition.operators.is_empty()
            {
                return Err(unavailable(
                    "this domain's complete parameter, alias or role contract",
                ));
            }
            // The legacy route list cannot represent a planned policy's
            // explicit empty issuer set. Never turn that denial into an
            // unrestricted domain while the policy projection is unserved.
            if definition.access_policy.is_some() || !definition.establishment_routes.is_empty() {
                return Err(unavailable("domain issuance authority"));
            }
            if definition.semantic_roles.denotation_dimension.is_some()
                || definition.semantic_roles.arithmetic_policy.is_some()
            {
                return Err(unavailable("domain semantic roles"));
            }
            let mut predicate_facts = Vec::new();
            for handle in definition.facts.handles() {
                let fact = self.typed().proof_facts.get(handle);
                let typed_trees::domain::ProofFact::Expression(expression) = fact else {
                    return Err(unavailable("a non-expression domain predicate fact"));
                };
                predicate_facts.push(PackageReviewContractFact::Expression(self.expression(
                    *expression,
                    definition,
                    0,
                )?));
            }
            predicate_facts.sort();
            predicate_facts.dedup();
            rows.push(PackagePolicyDomainShape {
                identity: self.nominal(definition.symbol)?,
                type_parameters: Vec::new(),
                target_type: self.type_identity(definition.target_type)?,
                index_arguments: Vec::new(),
                predicate_presence: definition.predicate_presence,
                predicate_facts,
                alias_expansion: None,
                classification: None,
                semantic_roles: Vec::new(),
                establishment_routes: Vec::new(),
            });
        }
        rows.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(rows)
    }

    pub(super) fn expression(
        &self,
        expression: ExpressionHandle,
        domain: &DomainDefinition,
        depth: usize,
    ) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
        self.scoped_expression(
            expression,
            ContractScope {
                owner: domain.symbol,
                subject: Some((domain.subject_symbol, domain.target_type)),
                parameters: Default::default(),
                result: None,
            },
            depth,
        )
    }

    /// Selection and operand identity decide a spelled operator's meaning.
    /// The ledger need not contain a row for a fixed builtin connective, and
    /// absence of a row cannot itself turn an unresolved operator into one.
    fn builtin_operator(
        &self,
        expression: ExpressionHandle,
        scope: ContractScope<'_>,
    ) -> Result<(), Vec<Diagnostic>> {
        let typed = self.typed();
        let ExpressionNode::Binary(binary) = typed.expression_table.expression(expression) else {
            return Err(unavailable("a non-binary contract operator"));
        };
        // Conjunction and disjunction are fixed clause connectives, not
        // overloadable operator families. The accepted judgment checked both
        // operands as clauses, and recursive projection still retains them.
        if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
            return Ok(());
        }
        let spelling = typed_trees::operator::binary_operator_spelling(binary.operator)
            .ok_or_else(|| unavailable("an unserved contract operator family"))?;
        let operands = [
            self.operand_type(binary.left, scope)?,
            self.operand_type(binary.right, scope)?,
        ];
        if typed_trees::operator::has_builtin_spelled_expression_meaning(
            typed,
            scope.owner,
            expression,
            spelling,
            &operands,
        ) {
            Ok(())
        } else {
            Err(unavailable(
                "a selected or unresolved contract operator meaning",
            ))
        }
    }

    fn operand_type(
        &self,
        expression: ExpressionHandle,
        scope: ContractScope<'_>,
    ) -> Result<Option<TypeReferenceHandle>, Vec<Diagnostic>> {
        let typed = self.typed();
        let table = &typed.expression_table;
        let retained = table.constructed_value_type_reference(expression);
        if retained.is_valid() && typed.type_reference_table.contains_type_reference(retained) {
            return Ok(Some(retained));
        }
        let (atom, policy) = match table.expression(expression) {
            ExpressionNode::Name(name) => {
                if let Some((symbol, value_type)) = scope.subject
                    && symbol.is_valid()
                    && name.symbol == symbol
                {
                    return Ok(Some(value_type));
                }
                if let Some(parameter) = scope.parameters.iter().find(|p| p.symbol == name.symbol) {
                    return Ok(Some(parameter.type_reference));
                }
                if name.reserved_result_of == scope.owner
                    && scope.owner.is_valid()
                    && let Some(result) = scope.result
                {
                    return Ok(Some(result));
                }
                return Err(unavailable(
                    "a contract name outside its declaration's scope",
                ));
            }
            ExpressionNode::Integer(literal) => {
                let Some(landing) = literal.landing() else {
                    return Ok(None);
                };
                (
                    typed_trees::types::builtin_type_atom(landing.landed_type),
                    landing.domain,
                )
            }
            ExpressionNode::Boolean(_) => (
                symbols::BuiltinTypeAtom::Bool,
                numerics::arithmetic::ArithmeticDomain::Exact,
            ),
            _ => return Err(unavailable("a contract operand's retained type")),
        };
        let reference = typed
            .symbols
            .builtin_atom_symbol(atom)
            .and_then(|symbol| {
                typed
                    .type_reference_table
                    .find_arithmetic_result_type_reference(symbol, policy)
            })
            .ok_or_else(|| unavailable("a literal's canonical declared carrier"))?;
        Ok(Some(reference))
    }

    pub(super) fn scoped_expression(
        &self,
        expression: ExpressionHandle,
        scope: ContractScope<'_>,
        depth: usize,
    ) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
        use PackageReviewContractBinaryOperator as Output;
        use PackageReviewContractExpression as Expression;
        if depth > 128
            || !self
                .typed()
                .expression_table
                .expression_is_valid(expression)
        {
            return Err(unavailable("an invalid or over-deep contract expression"));
        }
        Ok(match self.typed().expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => Expression::Boolean(*value),
            ExpressionNode::Integer(value) => Expression::Integer(value.to_string()),
            ExpressionNode::Name(name) => {
                if scope
                    .subject
                    .is_some_and(|(symbol, _)| symbol.is_valid() && name.symbol == symbol)
                {
                    Expression::DomainSubject
                } else if let Some(position) = scope
                    .parameters
                    .iter()
                    .position(|p| p.symbol == name.symbol)
                {
                    Expression::Parameter(
                        u32::try_from(position)
                            .map_err(|_| unavailable("a contract parameter ordinal"))?,
                    )
                } else if scope.result.is_some()
                    && scope.owner.is_valid()
                    && name.reserved_result_of == scope.owner
                {
                    Expression::Result
                } else {
                    return Err(unavailable(
                        "a contract name outside its declaration's scope",
                    ));
                }
            }
            ExpressionNode::Binary(binary) => {
                self.builtin_operator(expression, scope)?;
                let operator = match binary.operator {
                    BinaryOperator::Add => Output::Add,
                    BinaryOperator::And => Output::And,
                    BinaryOperator::BitwiseAnd => Output::BitwiseAnd,
                    BinaryOperator::BitwiseOr => Output::BitwiseOr,
                    BinaryOperator::BitwiseXor => Output::BitwiseXor,
                    BinaryOperator::Divide => Output::Divide,
                    BinaryOperator::Equal => Output::Equal,
                    BinaryOperator::Greater => Output::Greater,
                    BinaryOperator::GreaterOrEqual => Output::GreaterOrEqual,
                    BinaryOperator::Less => Output::Less,
                    BinaryOperator::LessOrEqual => Output::LessOrEqual,
                    BinaryOperator::Modulo => Output::Modulo,
                    BinaryOperator::Multiply => Output::Multiply,
                    BinaryOperator::NotEqual => Output::NotEqual,
                    BinaryOperator::Or => Output::Or,
                    BinaryOperator::ShiftLeft => Output::ShiftLeft,
                    BinaryOperator::ShiftRight => Output::ShiftRight,
                    BinaryOperator::Subtract => Output::Subtract,
                    BinaryOperator::CaseMembership => {
                        return Err(unavailable("a nominal case test"));
                    }
                };
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
                    return Err(unavailable("an unnormalized arithmetic contract policy"));
                }
                Expression::Binary {
                    meaning: PackageReviewContractOperatorMeaning::Builtin,
                    operator,
                    left: Box::new(self.scoped_expression(binary.left, scope, depth + 1)?),
                    right: Box::new(self.scoped_expression(binary.right, scope, depth + 1)?),
                }
            }
            _ => return Err(unavailable("this contract expression form")),
        })
    }
}
