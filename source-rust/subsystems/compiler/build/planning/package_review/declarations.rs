//! Reads declaration facts for review from the source the package judgment
//! accepted. It infers no behavior from syntax: callable behavior is read
//! separately from the retained module, and an unhandled declaration form makes
//! the review incomplete instead of vanishing from its inventory.

use super::input::PolicySource;
use diagnostics::Diagnostic;
use review_evidence::record::{
    PackagePolicyDataShape, PackagePolicyDomainShape, PackageReviewByteSequencePredicate,
    PackageReviewContractBinaryOperator, PackageReviewContractCallTarget,
    PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewContractOperatorMeaning, PackageReviewDataField, PackageReviewDataKind,
    PackageReviewDataMember, PackageReviewDataProperties, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewSemanticDependencyExposure,
    PackageReviewToolchainSourceIdentity, PackageReviewTypeIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::representations::source_state::TypedTrees;
use source_compiler::representations::source_state::data::{DataField, DataMember};
use source_compiler::representations::source_state::domain::DomainDefinition;
use source_compiler::representations::source_state::evidence::byte_predicates::{
    ByteSequencePredicate, byte_domain_predicate,
};
use source_compiler::representations::source_state::expression::{
    BinaryOperator, CollectionMeasure, ExpressionHandle, ExpressionNode, MemberSelection,
    TableIndexedExpression,
};
use source_compiler::representations::source_state::types::TypeReferenceHandle;
use symbols::SymbolHandle;

/// Names in a reviewed clause come from its retained declaration, never from
/// their spelling. Domain predicates and callable clauses share this projection.
#[derive(Clone, Copy)]
pub(super) struct ContractScope<'a> {
    pub owner: SymbolHandle,
    pub subject: Option<(SymbolHandle, TypeReferenceHandle)>,
    pub statics:
        arena::RootRows<'a, source_compiler::representations::source_state::data::TypeParameter>,
    pub parameters: arena::RootRows<
        'a,
        source_compiler::representations::source_state::signature::StateParameter,
    >,
    pub result: Option<TypeReferenceHandle>,
}

pub(super) fn unavailable(what: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package review cannot retain {what} from the accepted declaration"
    ))]
}

/// Both accepted source owners prepare their exact toolchain roster once.
/// Nominal projection keeps that owner with the declaration; a later source
/// context may not substitute its source numbers or diagnostic spelling.
pub(super) fn nominal_identity(
    typed: &TypedTrees,
    toolchain_sources: &[(source_text::SourceId, [u8; 32])],
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
    let path = typed
        .authored_declaration_path(symbol)
        .map_err(|reason| unavailable(&reason))?;
    if path.is_empty() {
        return Err(unavailable("a declaration without a stable path"));
    }
    Ok(PackageReviewNominalIdentity { owner, path })
}

impl<'a> PolicySource<'a> {
    pub(super) fn owns(
        &self,
        symbol: SymbolHandle,
        package: PackageKeyIdentity,
    ) -> Result<bool, Vec<Diagnostic>> {
        source_compiler::representations::source_state::source_subjects::contains(
            self.typed(),
            symbol,
            &[Some(package)],
            source_provenance::DependencyScope::Product,
        )
        .map_err(unavailable)
    }

    pub(super) fn nominal(
        &self,
        symbol: SymbolHandle,
    ) -> Result<PackageReviewNominalIdentity, Vec<Diagnostic>> {
        nominal_identity(
            self.typed(),
            self.sources().exact_toolchain_sources(),
            symbol,
        )
    }

    pub(super) fn type_identity(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        if !reference.is_valid() {
            return Err(unavailable("an absent declared type"));
        }
        self.require_type_dependencies(
            owner,
            reference,
            PackageReviewSemanticDependencyExposure::PublicInterface,
        )?;
        self.exact_signature_type_identity(reference)
    }

    pub(super) fn type_identity_for_private_contract(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        self.require_type_dependencies(
            owner,
            reference,
            PackageReviewSemanticDependencyExposure::PrivateImplementation,
        )?;
        self.exact_signature_type_identity(reference)
    }

    fn exact_signature_type_identity(
        &self,
        reference: TypeReferenceHandle,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        let canonical = self
            .typed()
            .exact_owner_type_identity(
                source_compiler::representations::source_state::type_identity::ExactOwnerTypeIdentityRequest {
                    type_reference: reference,
                    lifetime_scope: None,
                    binders: &[],
                    substitutions: &[],
                    exact_toolchain_sources: self.sources().exact_toolchain_sources(),
                },
            )
            .and_then(|identity| self.typed().type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| unavailable("a type without a package-qualified identity"))?;
        Ok(PackageReviewTypeIdentity { canonical })
    }

    fn contract_declaration(&self, owner: SymbolHandle) -> SymbolHandle {
        let source = self.typed();
        let template = source
            .closed_requirement_application(owner)
            .map_or(owner, |application| application.template);
        source
            .trait_machine_signature(template)
            .map_or(template, |(service, _)| service.symbol)
    }

    fn exposes_contract(&self, declaration: SymbolHandle) -> Result<bool, Vec<Diagnostic>> {
        use source_compiler::representations::source_state::visibility::declaration_visibility;
        let source = self.typed();
        if let Some(machine) = source.machine_by_symbol(declaration) {
            return Ok(super::callables::exposes_machine_interface(machine));
        }
        declaration_visibility(source, declaration)
            .map(|visibility| visibility.is_public())
            .ok_or_else(|| unavailable("the visibility of a reviewed contract's declaration"))
    }

    pub(super) fn require_type_dependencies(
        &self,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
        exposure: PackageReviewSemanticDependencyExposure,
    ) -> Result<(), Vec<Diagnostic>> {
        use source_compiler::representations::source_state::{
            references::collect_type_symbol_dependencies, visibility::declaration_visibility,
        };
        let package = self
            .sources()
            .package_identity()
            .ok_or_else(|| unavailable("a type outside the reviewed package subject"))?;
        let source = self.typed();
        let declaration = self.contract_declaration(owner);
        // Reading the supplier's schema establishes the meaning of a grant,
        // but only a declaration owned by this Product contributes a use.
        if !self.owns(declaration, package)? {
            return Ok(());
        }
        let exposure = if exposure == PackageReviewSemanticDependencyExposure::PublicInterface
            && !self.exposes_contract(declaration)?
        {
            PackageReviewSemanticDependencyExposure::PrivateImplementation
        } else {
            exposure
        };
        let symbols = &source.symbols;
        for symbol in collect_type_symbol_dependencies(source, reference) {
            if !symbols.symbols().nodes().is_valid(symbol) {
                return Err(unavailable("a stale declaration in a public type"));
            }
            if matches!(symbols.get(symbol).kind, symbols::SymbolKind::BuiltinType | symbols::SymbolKind::TypeParameter | symbols::SymbolKind::ConformanceParameter) {
                continue;
            }
            let dependency_owner = symbols.symbol_package_identity(symbol);
            if dependency_owner.is_some()
                && exposure == PackageReviewSemanticDependencyExposure::PublicInterface
                && !declaration_visibility(source, symbol)
                    .is_some_and(|declaration| declaration.is_public())
            {
                return Err(unavailable("a non-public declaration in a public type"));
            }
            let dependency = self.nominal(symbol)?;
            if dependency_owner.is_some_and(|owner| owner != package) {
                self.carried_types.retain(dependency, exposure)?;
            }
        }
        Ok(())
    }

    fn field(
        &self,
        field: &DataField,
        owner: SymbolHandle,
    ) -> Result<PackageReviewDataField, Vec<Diagnostic>> {
        Ok(PackageReviewDataField {
            identity: field.identity,
            name: field.name.as_str().to_owned(),
            relevance: field.relevance,
            type_identity: self.declared_type(owner, field.type_reference)?,
        })
    }

    pub(super) fn public_type_aliases(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<review_evidence::record::PackagePolicyTypeAlias>, Vec<Diagnostic>> {
        let mut aliases = Vec::new();
        for alias in self
            .typed()
            .type_aliases()
            .iter()
            .filter(|row| row.is_public)
        {
            if !self.owns(alias.symbol, package)? {
                if source_compiler::representations::source_state::source_subjects::contains(
                    self.typed(),
                    alias.symbol,
                    &[Some(package)],
                    source_provenance::DependencyScope::Build,
                )
                .map_err(unavailable)?
                {
                    return Err(unavailable("Build-scope public type alias contracts"));
                }
                continue;
            }
            aliases.push(review_evidence::record::PackagePolicyTypeAlias {
                identity: self.nominal(alias.symbol)?,
                lifetime_parameter_count: alias.lifetime_parameters.len(),
                type_parameters: self.type_parameters(alias.symbol)?,
                conformance_bounds: self.conformance_bounds(
                    alias.symbol,
                    &alias.lifetime_parameters,
                    &alias.conformance_bounds,
                )?,
                expansion: self.declared_type(alias.symbol, alias.expansion)?,
            });
        }
        aliases.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(aliases)
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
            if !definition.conformance_bounds.is_empty()
                || definition.quotient.is_some()
                || definition.cleanup_selection.is_some()
                || definition.supply_mode != language_core::DataSupplyMode::CheckedShape
            {
                return Err(unavailable(
                    "this data declaration's complete generic or cleanup signature",
                ));
            }
            let mut members = Vec::new();
            for member in self.typed().data_members(definition) {
                members.push(match member {
                    DataMember::Field(field) => {
                        PackageReviewDataMember::Field(self.field(field, definition.symbol)?)
                    }
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
                                .map(|field| self.field(field, definition.symbol))
                                .collect::<Result<_, _>>()?,
                            retired_payload_identities: retired,
                        }
                    }
                });
            }
            let mut retired_identities = definition.retired_identities.clone();
            retired_identities.sort_unstable();
            retired_identities.dedup();
            let scope = ContractScope {
                owner: definition.symbol,
                subject: Some((
                    definition.predicate_subject_symbol,
                    definition.predicate_subject_type,
                )),
                statics: self.typed().data_type_parameters(definition),
                parameters: Default::default(),
                result: None,
            };
            let mut invariants = self
                .typed()
                .proof_facts
                .span(definition.requires_facts)
                .ok_or_else(|| unavailable("a data invariant fact span"))?
                .iter()
                .map(|fact| self.review_fact(fact, scope))
                .collect::<Result<Vec<_>, _>>()?;
            invariants.sort();
            invariants.dedup();
            rows.push(PackagePolicyDataShape {
                identity: self.nominal(definition.symbol)?,
                kind: PackageReviewDataKind::Ordinary,
                supply: definition.supply_mode,
                lifetime_parameter_count: definition.lifetime_parameters.len(),
                type_parameters: self.type_parameters(definition.symbol)?,
                properties: PackageReviewDataProperties {
                    multiplicity: definition.properties.multiplicity,
                },
                zero_gated: definition.zero_gated,
                invariants,
                retired_identities,
                members,
            });
        }
        rows.sort_by(|left, right| left.identity.cmp(&right.identity));
        Ok(rows)
    }

    pub(super) fn public_consts(
        &self,
        package: PackageKeyIdentity,
    ) -> Result<Vec<review_evidence::record::PackageReviewConstShape>, Vec<Diagnostic>> {
        use review_evidence::record::{PackageReviewConstMeaning, PackageReviewConstShape};
        let mut rows = Vec::new();
        for declaration in self
            .typed()
            .const_declarations()
            .iter()
            .filter(|row| row.is_public)
        {
            // A public Build-scope constant also belongs to this package's API.
            if self
                .typed()
                .symbols
                .symbol_package_identity(declaration.symbol)
                != Some(package)
            {
                continue;
            }
            let meaning = if declaration.pending_target_observation {
                PackageReviewConstMeaning::TargetDependent(self.scoped_expression(
                    declaration.authored_initializer,
                    ContractScope {
                        owner: declaration.symbol,
                        subject: None,
                        statics: Default::default(),
                        parameters: Default::default(),
                        result: None,
                    },
                    0,
                )?)
            } else {
                let value = declaration
                    .evaluated_value
                    .as_ref()
                    .ok_or_else(|| unavailable("a public constant without an evaluated value"))?;
                PackageReviewConstMeaning::Evaluated(match value {
                    numeric_literals::EvaluatedConstant::Encoded(value) => value.clone(),
                    numeric_literals::EvaluatedConstant::UndeterminedNaN(format) => {
                        format!("semantic-nan:{}", format.name())
                    }
                })
            };
            rows.push(PackageReviewConstShape {
                identity: self.nominal(declaration.symbol)?,
                declared_type: self.type_identity(declaration.symbol, declaration.declared_type)?,
                meaning,
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
                let source_compiler::representations::source_state::domain::ProofFact::Expression(
                    expression,
                ) = fact
                else {
                    return Err(unavailable("a non-expression domain predicate fact"));
                };
                predicate_facts.push(PackageReviewContractFact::Expression(
                    self.domain_predicate(*expression, definition)?,
                ));
            }
            predicate_facts.sort();
            predicate_facts.dedup();
            rows.push(PackagePolicyDomainShape {
                identity: self.nominal(definition.symbol)?,
                type_parameters: Vec::new(),
                target_type: self.type_identity(definition.symbol, definition.target_type)?,
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

    fn domain_predicate(
        &self,
        expression: ExpressionHandle,
        domain: &DomainDefinition,
    ) -> Result<PackageReviewContractExpression, Vec<Diagnostic>> {
        if let Some((fact, predicate)) = byte_domain_predicate(self.typed(), domain)
            && fact == expression
        {
            // The source checker matched the exact builtin observation of
            // the complete domain subject. Record the predicate on those bytes.
            let predicate = match predicate {
                ByteSequencePredicate::ValidUtf8 => PackageReviewByteSequencePredicate::ValidUtf8,
                ByteSequencePredicate::NoNul => PackageReviewByteSequencePredicate::NoNul,
                ByteSequencePredicate::AsciiOnly => PackageReviewByteSequencePredicate::AsciiOnly,
                ByteSequencePredicate::NonEmpty => PackageReviewByteSequencePredicate::NonEmpty,
            };
            return Ok(PackageReviewContractExpression::Call {
                receiver: None,
                target: PackageReviewContractCallTarget::ByteSequencePredicate(predicate),
                static_arguments: Vec::new(),
                evidence_arguments: Vec::new(),
                arguments: vec![PackageReviewContractExpression::DomainSubject],
            });
        }
        self.scoped_expression(
            expression,
            ContractScope {
                owner: domain.symbol,
                subject: Some((domain.subject_symbol, domain.target_type)),
                parameters: Default::default(),
                statics: Default::default(),
                result: None,
            },
            0,
        )
    }

    /// Selection and operand identity decide a spelled operator's meaning.
    /// The ledger need not contain a row for a fixed builtin connective, and
    /// absence of a row cannot itself turn an unresolved operator into one.
    fn builtin_operator(
        &self,
        expression: ExpressionHandle,
        scope: ContractScope<'_>,
        depth: usize,
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
        let spelling =
            source_compiler::representations::source_state::operator::binary_operator_spelling(
                binary.operator,
            )
            .ok_or_else(|| unavailable("an unserved contract operator family"))?;
        let operands = [
            self.operand_type(binary.left, scope, depth + 1)?,
            self.operand_type(binary.right, scope, depth + 1)?,
        ];
        if source_compiler::representations::source_state::operator::has_builtin_spelled_expression_meaning(
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
        depth: usize,
    ) -> Result<Option<TypeReferenceHandle>, Vec<Diagnostic>> {
        let typed = self.typed();
        let table = &typed.expression_table;
        if depth > 128 || !table.expression_is_valid(expression) {
            return Err(unavailable("an invalid or over-deep contract operand"));
        }
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
                if let Some(parameter) = scope.statics.iter().find(|p| p.symbol == name.symbol) {
                    return match parameter.kind {
                        source_compiler::representations::source_state::data::TypeParameterKind::Const {
                            type_reference,
                        }
                        | source_compiler::representations::source_state::data::TypeParameterKind::Value {
                            type_reference,
                        } => Ok(Some(type_reference)),
                        _ => Err(unavailable("a non-value contract binder")),
                    };
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
                if name.symbol.is_valid()
                    && let Some(declaration) = typed.const_declaration_by_symbol(name.symbol)
                    && declaration.is_public
                {
                    return Ok(Some(declaration.declared_type));
                }
                return Err(unavailable(
                    "a contract name outside its declaration's scope",
                ));
            }
            ExpressionNode::Member(member) if member.selection.field_symbol().is_valid() => {
                return self
                    .typed()
                    .unique_declared_field_by_symbol(member.selection.field_symbol())
                    .map(|field| Some(field.type_reference))
                    .ok_or_else(|| unavailable("a contract field's declared type"));
            }
            ExpressionNode::Member(member)
                if member.selection
                    == MemberSelection::CollectionMeasure(CollectionMeasure::Length) =>
            {
                (
                    symbols::BuiltinTypeAtom::U64,
                    numerics::arithmetic::ArithmeticDomain::Exact,
                )
            }
            ExpressionNode::Indexed(indexed) => {
                return self
                    .builtin_index_element_type(expression, indexed, scope, depth)
                    .map(Some);
            }
            ExpressionNode::Binary(binary) if binary.operator.is_arithmetic() => {
                use source_compiler::representations::source_state::{
                    closed_numeric::has_anonymous_numeric_results, operator::core_integer_result,
                };
                if has_anonymous_numeric_results(typed, expression) {
                    return Ok(None);
                }
                // The expression projection independently checks the selected operator.
                // Use the same carrier judgment as typing when its result is an operand.
                return core_integer_result(
                    typed,
                    binary.operator,
                    binary.policy,
                    self.operand_type(binary.left, scope, depth + 1)?,
                    self.operand_type(binary.right, scope, depth + 1)?,
                )
                .map(Some)
                .ok_or_else(|| unavailable("a computed contract operand's exact integer carrier"));
            }
            ExpressionNode::Cast(cast) if cast.result_type.is_valid() => {
                return Ok(Some(cast.result_type));
            }
            ExpressionNode::Integer(literal) => {
                let Some(landing) = literal.landing() else {
                    return Ok(None);
                };
                (
                    source_compiler::representations::source_state::types::builtin_type_atom(
                        landing.landed_type,
                    ),
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
            .ok_or_else(|| unavailable("a contract operand's canonical declared carrier"))?;
        Ok(Some(reference))
    }

    fn builtin_index_element_type(
        &self,
        expression: ExpressionHandle,
        indexed: &TableIndexedExpression,
        scope: ContractScope<'_>,
        depth: usize,
    ) -> Result<TypeReferenceHandle, Vec<Diagnostic>> {
        use source_compiler::representations::source_state::{
            operator::builtin_index_read_is_spelled,
            type_system::source_projections::indexed_element_type_reference,
        };
        let typed = self.typed();
        let collection = self
            .operand_type(indexed.collection, scope, depth + 1)?
            .ok_or_else(|| unavailable("an indexed contract collection's declared type"))?;
        let index = self.operand_type(indexed.index, scope, depth + 1)?;
        if !builtin_index_read_is_spelled(
            typed,
            scope.owner,
            expression,
            &[Some(collection), index],
        ) {
            return Err(unavailable(
                "a selected or unresolved contract index operator",
            ));
        }
        indexed_element_type_reference(typed, collection, &[])
            .ok_or_else(|| unavailable("an indexed contract collection's element type"))
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
            ExpressionNode::Float(literal) => {
                use numeric_literals::FloatFormat;
                use review_evidence::record::PackageReviewFloatLiteral;
                Expression::Float(match literal.landing() {
                    Some(FloatFormat::F32) => PackageReviewFloatLiteral::F32(literal.f32_bits()),
                    Some(FloatFormat::F64) => {
                        PackageReviewFloatLiteral::F64(literal.landed_f64().to_bits())
                    }
                    None => return Err(unavailable("the checked width of a floating literal")),
                })
            }
            ExpressionNode::String(bytes) => Expression::ByteSequence(bytes.to_vec()),
            ExpressionNode::ArrayLiteral(values) => Expression::Array(
                self.typed()
                    .expression_table
                    .expression_handles(*values)
                    .iter()
                    .map(|value| self.scoped_expression(*value, scope, depth + 1))
                    .collect::<Result<_, _>>()?,
            ),
            ExpressionNode::Range(range) => {
                let endpoint = |value: ExpressionHandle| {
                    value
                        .is_valid()
                        .then(|| {
                            self.scoped_expression(value, scope, depth + 1)
                                .map(Box::new)
                        })
                        .transpose()
                };
                Expression::Range {
                    start: endpoint(range.start)?,
                    end: endpoint(range.end)?,
                    end_inclusive: range.end_inclusive,
                }
            }
            ExpressionNode::Borrow(borrow) => Expression::Reference {
                access: reference_access(borrow.access),
                target: Box::new(self.scoped_expression(borrow.target, scope, depth + 1)?),
            },
            ExpressionNode::Cast(cast) => {
                use language_core::cast_form::{CastEntryKind, CastForm};
                use numerics::arithmetic::ArithmeticDomain;
                use review_evidence::record::{
                    PackageReviewArithmeticDomain as Policy, PackageReviewCastForm,
                };
                let (form, target) = match cast.form {
                    CastForm::Value => (PackageReviewCastForm::Value, cast.target_type),
                    CastForm::Recast(access) => (
                        PackageReviewCastForm::Recast(reference_access(access)),
                        cast.target_type,
                    ),
                    CastForm::Entry(entry) => {
                        if !cast.result_type.is_valid() {
                            return Err(unavailable("an unaccounted domain-cast entry"));
                        }
                        let form = match entry.kind {
                            CastEntryKind::Keep => PackageReviewCastForm::KeepDomain,
                            CastEntryKind::Remove => PackageReviewCastForm::RemoveDomain,
                            CastEntryKind::Add => PackageReviewCastForm::AddDomain,
                        };
                        (form, cast.result_type)
                    }
                };
                let semantic_domain = if cast.semantic_domain_symbol.is_valid() {
                    let domain = self
                        .typed()
                        .domain_definition(cast.semantic_domain_symbol)
                        .ok_or_else(|| unavailable("a cast domain without its declaration"))?;
                    let identity = self.nominal(domain.symbol)?;
                    if !domain.is_public {
                        let owner = self.contract_declaration(scope.owner);
                        let declaring = self.nominal(owner)?;
                        if declaring.owner != identity.owner || self.exposes_contract(owner)? {
                            return Err(unavailable(format!(
                                "private domain `{}` in the cast contract of `{}`",
                                domain.name.as_str(),
                                declaring.path
                            )));
                        }
                    }
                    Some(identity)
                } else if !cast.semantic_domain.is_empty() {
                    return Err(unavailable("an unresolved cast qualification"));
                } else {
                    None
                };
                Expression::Cast {
                    value: Box::new(self.scoped_expression(cast.value, scope, depth + 1)?),
                    target: self.declared_type(scope.owner, target)?,
                    arithmetic_domain: match cast.domain {
                        ArithmeticDomain::Exact => Policy::Exact,
                        ArithmeticDomain::Wrapping => Policy::Wrapping,
                        ArithmeticDomain::Saturating => Policy::Saturating,
                        ArithmeticDomain::Trapping => Policy::Trapping,
                    },
                    semantic_domain,
                    semantic_domain_arguments: self
                        .typed()
                        .type_reference_table
                        .type_reference_handles(cast.semantic_domain_arguments)
                        .iter()
                        .map(|r| self.declared_type(scope.owner, *r))
                        .collect::<Result<_, _>>()?,
                    form,
                }
            }
            ExpressionNode::Name(name) => {
                if scope
                    .subject
                    .is_some_and(|(symbol, _)| symbol.is_valid() && name.symbol == symbol)
                {
                    Expression::DomainSubject
                } else if let Some(position) =
                    scope.statics.iter().position(|p| p.symbol == name.symbol)
                {
                    Expression::GenericBinder(
                        u32::try_from(position)
                            .map_err(|_| unavailable("a contract static binder ordinal"))?,
                    )
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
                } else if name.symbol.is_valid()
                    && let Some(member) = self.typed().symbols.builtin_type_member(name.symbol)
                {
                    Expression::BuiltinTypeMember(member)
                } else if name.symbol.is_valid()
                    && self
                        .typed()
                        .const_declaration_by_symbol(name.symbol)
                        .is_some_and(|constant| constant.is_public)
                {
                    Expression::Nominal(self.nominal(name.symbol)?)
                } else {
                    return Err(unavailable(
                        "a contract name outside its declaration's scope",
                    ));
                }
            }
            ExpressionNode::Indexed(indexed) => {
                self.builtin_index_element_type(expression, indexed, scope, depth)?;
                Expression::Indexed {
                    meaning: PackageReviewContractOperatorMeaning::Builtin,
                    collection: Box::new(self.scoped_expression(
                        indexed.collection,
                        scope,
                        depth + 1,
                    )?),
                    index: Box::new(self.scoped_expression(indexed.index, scope, depth + 1)?),
                }
            }
            ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::CaseMembership => {
                let ExpressionNode::Name(case) =
                    self.typed().expression_table.expression(binary.right)
                else {
                    return Err(unavailable("the selected variant of a case test"));
                };
                let symbols = &self.typed().symbols;
                if !symbols.symbols().nodes().is_valid(case.symbol)
                    || symbols.get(case.symbol).kind != symbols::SymbolKind::Variant
                {
                    return Err(unavailable("the nominal declaration of a case test"));
                }
                Expression::CaseMembership {
                    subject: Box::new(self.scoped_expression(binary.left, scope, depth + 1)?),
                    case: self.nominal(case.symbol)?,
                }
            }
            ExpressionNode::Call(call) if call.layout_query.is_some() => {
                use language_core::data_layout_queries::DataLayoutQueryKind;
                use review_evidence::record::PackageReviewLayoutQueryKind;
                use source_compiler::representations::source_state::expression::DataLayoutQueryRequest;
                let Some(DataLayoutQueryRequest::Type { kind, operand }) = &call.layout_query else {
                    return Err(unavailable("the fixed type operand of a public layout observation"));
                };
                if !operand.type_reference.is_valid()
                    || operand.const_literal.is_some()
                    || operand.evidence_projection.is_some()
                    || call.receiver.is_valid()
                    || !call.arguments.is_empty()
                {
                    return Err(unavailable(
                        "the fixed type operand of a public layout observation",
                    ));
                }
                Expression::LayoutQuery {
                    kind: match kind {
                        DataLayoutQueryKind::Size => PackageReviewLayoutQueryKind::Size,
                        DataLayoutQueryKind::Alignment => PackageReviewLayoutQueryKind::Alignment,
                    },
                    operand: self.declared_type(scope.owner, operand.type_reference)?,
                }
            }
            ExpressionNode::Call(call) => {
                if call.runtime_call.is_some()
                    || call.authored_machine_application.is_valid()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.native_code_layout.is_some()
                    || call.exact_coercion_target.is_valid()
                    || call.requirement_application.is_valid()
                    || call.build_selection.is_valid()
                {
                    return Err(unavailable(
                        "the complete static, evidence or builtin arguments of this contract call",
                    ));
                }
                let source = self.typed();
                let target = if let Some(function) = source
                    .symbols
                    .builtin_function_for_symbol(call.target_symbol)
                {
                    PackageReviewContractCallTarget::BuiltinFunction(function)
                } else {
                    let binding = call
                        .authored_call_binding
                        .as_ref()
                        .filter(|binding| {
                            binding.selected == call.target_symbol
                                && binding.original == binding.selected
                        })
                        .ok_or_else(|| {
                            unavailable("the checked nominal target of a contract call")
                        })?;
                    let machine = source
                        .machine(binding.selected)
                        .filter(|machine| machine.is_public && machine.type_parameters.is_empty())
                        .ok_or_else(|| unavailable("a complete public contract-call signature"))?;
                    PackageReviewContractCallTarget::Nominal(self.nominal(machine.symbol)?)
                };
                let receiver = call
                    .receiver
                    .is_valid()
                    .then(|| {
                        self.scoped_expression(call.receiver, scope, depth + 1)
                            .map(Box::new)
                    })
                    .transpose()?;
                let arguments = source
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .map(|a| self.scoped_expression(*a, scope, depth + 1))
                    .collect::<Result<_, _>>()?;
                Expression::Call {
                    receiver,
                    target,
                    static_arguments: Vec::new(),
                    evidence_arguments: Vec::new(),
                    arguments,
                }
            }
            ExpressionNode::Member(member) => {
                let receiver =
                    Box::new(self.scoped_expression(member.receiver, scope, depth + 1)?);
                match member.selection {
                    MemberSelection::Field(field) => {
                        let declared = self
                            .typed()
                            .unique_declared_field_by_symbol(field)
                            .ok_or_else(|| unavailable("the unique selected field declaration"))?;
                        let parent = self.typed().symbols.get(declared.symbol).parent;
                        let case_variant = (self.typed().symbols.get(parent).kind
                            == symbols::SymbolKind::Variant)
                            .then(|| self.nominal(parent))
                            .transpose()?;
                        Expression::Member {
                            receiver,
                            member: self.nominal(field)?,
                            case_variant,
                        }
                    }
                    MemberSelection::CollectionMeasure(CollectionMeasure::Length) => {
                        Expression::CollectionLength {
                            collection: receiver,
                        }
                    }
                    MemberSelection::CollectionMeasure(CollectionMeasure::Capacity) => {
                        Expression::CollectionCapacity {
                            collection: receiver,
                        }
                    }
                    MemberSelection::Unresolved => {
                        return Err(unavailable("an unresolved contract member"));
                    }
                }
            }
            ExpressionNode::Binary(binary) => {
                self.builtin_operator(expression, scope, depth)?;
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

fn reference_access(
    access: language_core::ReferenceAccess,
) -> review_evidence::record::PackageReviewReferenceAccess {
    use review_evidence::record::PackageReviewReferenceAccess as Access;
    match access {
        language_core::ReferenceAccess::Shared => Access::Shared,
        language_core::ReferenceAccess::Mutable => Access::Mutable,
        language_core::ReferenceAccess::WriteOnly => Access::WriteOnly,
    }
}
