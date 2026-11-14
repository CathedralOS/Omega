//! Declaration facts are read from the source the package judgment accepted.
//! This projection does not infer behavior from syntax: callable behavior is
//! read separately from the retained module, and an unserved declaration form
//! prevents a complete review instead of disappearing from its inventory.

use super::PackageReviewInput;
use crate::record::{
    PackagePolicyDataShape, PackagePolicyDomainShape, PackageReviewContractBinaryOperator,
    PackageReviewContractExpression, PackageReviewContractFact,
    PackageReviewContractOperatorMeaning, PackageReviewDataField, PackageReviewDataKind,
    PackageReviewDataMember, PackageReviewDataProperties, PackageReviewNominalIdentity,
    PackageReviewNominalOwner, PackageReviewToolchainSourceIdentity, PackageReviewTypeIdentity,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::{DataField, DataMember};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::types::TypeReferenceHandle;

pub(super) fn unavailable(what: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "package review cannot retain {what} from the accepted declaration"
    ))]
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
        let symbols = &self.typed().symbols;
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
            let toolchain_sources = self.compilation().sources().exact_toolchain_sources();
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

    pub(super) fn type_identity(
        &self,
        reference: TypeReferenceHandle,
    ) -> Result<PackageReviewTypeIdentity, Vec<Diagnostic>> {
        if !reference.is_valid() {
            return Err(unavailable("an absent declared type"));
        }
        self.require_type_dependencies(reference)?;
        let canonical = self
            .typed()
            .exact_owner_type_identity(typed_trees::type_identity::ExactOwnerTypeIdentityRequest {
                type_reference: reference,
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
                Some(_) => {
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
                None => {
                    self.nominal(symbol)?;
                }
            }
        }
        Ok(())
    }

    fn field(&self, field: &DataField) -> Result<PackageReviewDataField, Vec<Diagnostic>> {
        Ok(PackageReviewDataField {
            identity: field.identity,
            name: field.name.as_str().to_owned(),
            relevance: field.relevance,
            type_identity: self.type_identity(field.type_reference)?,
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
            if !definition.type_parameters.is_empty()
                || !definition.lifetime_parameters.is_empty()
                || !definition.conformance_bounds.is_empty()
                || definition.quotient.is_some()
                || definition.cleanup_selection.is_some()
                || definition.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            {
                return Err(unavailable(
                    "this data declaration's complete generic or cleanup signature",
                ));
            }
            let mut members = Vec::new();
            for member in self.typed().data_members(definition) {
                members.push(match member {
                    DataMember::Field(field) => PackageReviewDataMember::Field(self.field(field)?),
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
                                .map(|field| self.field(field))
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
                    definition.subject_symbol,
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

    /// An operator's spelling alone does not identify its meaning. Preserve
    /// the one selection attached to this exact accepted expression; an
    /// unserved overload must not be rendered as a builtin operation.
    fn builtin_operator(&self, expression: ExpressionHandle) -> Result<(), Vec<Diagnostic>> {
        use language_semantics::declaration_selection::{
            AuthoredDeclarationSelectionIntrinsic as Intrinsic,
            AuthoredDeclarationSelectionKind as Kind, AuthoredDeclarationSelectionTarget as Target,
        };
        let mut found = None;
        for occurrence in self
            .typed()
            .expression_table
            .authored_selection_occurrences(expression)
        {
            let selection = self
                .typed()
                .authored_declaration_selections()
                .get(occurrence)
                .ok_or_else(|| unavailable("a dangling contract operator selection"))?;
            if selection.kind() != Kind::Operator {
                continue;
            }
            if found.replace(selection).is_some() {
                return Err(unavailable("an ambiguous contract operator selection"));
            }
        }
        match found.map(|selection| selection.target()) {
            Some(Target::Intrinsic(Intrinsic::BuiltinOperator)) => Ok(()),
            _ => Err(unavailable(
                "a missing or non-builtin contract operator selection",
            )),
        }
    }

    pub(super) fn expression(
        &self,
        expression: ExpressionHandle,
        subject: SymbolHandle,
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
            ExpressionNode::Name(name) if name.symbol == subject && subject.is_valid() => {
                Expression::DomainSubject
            }
            ExpressionNode::Binary(binary) => {
                self.builtin_operator(expression)?;
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
                    left: Box::new(self.expression(binary.left, subject, depth + 1)?),
                    right: Box::new(self.expression(binary.right, subject, depth + 1)?),
                }
            }
            _ => return Err(unavailable("this contract expression form")),
        })
    }
}
