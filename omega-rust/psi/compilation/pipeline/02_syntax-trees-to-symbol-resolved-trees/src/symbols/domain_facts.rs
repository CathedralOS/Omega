//! Declaration-scoped facts bind lexical subjects before selecting remaining
//! closed type names. Complete source-visible paths retain semantic namespace
//! ancestry, including declarations physically stored at root, so authored
//! selection custody survives generic equation discharge.
use language_semantics::byte_predicates::ByteSequencePredicate;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{JoinedPath, SymbolHandle, SymbolKind, SymbolTable};

use super::expressions::assign_membership_symbol;
use super::lookup::diagnostic_path_source_span;
use super::targets::resolve_free_call_target_symbol;

pub(super) fn assign_domain_fact_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
    let domain_symbols = program
        .domain_definitions
        .iter()
        .map(|domain| {
            (
                domain.name.as_str().to_owned(),
                domain.symbol,
                domain.semantic_id,
            )
        })
        .collect::<Vec<_>>();
    let alias_symbols = program
        .domain_definitions
        .iter()
        .map(|domain| {
            domain
                .alias
                .as_ref()
                .map(|alias| {
                    alias
                        .constituents
                        .iter()
                        .map(|constituent| {
                            let members = program.domain_path_members(constituent.domain);
                            let name = members.joined_path();
                            resolve_domain_symbol(
                                symbols,
                                &domain_symbols,
                                &name,
                                diagnostic_path_source_span(members),
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let mut alias_symbols = alias_symbols.into_iter();
    program.domain_definitions.for_each_mut(|domain| {
        let resolved = alias_symbols
            .next()
            .expect("one alias-resolution set per domain");
        let Some(alias) = domain.alias.as_mut() else {
            return;
        };
        for (constituent, symbol) in alias.constituents.iter_mut().zip(resolved) {
            constituent.domain_symbol = symbol;
        }
    });
    // Each scope carries its declaration's generic telescope so an indexed
    // application's arguments (`value.slot in Resident<P, T>`) resolve against
    // the owning binders. Signature contracts resolve theirs in the machine
    // or trait scope during contract-reference assignment, which runs first;
    // `None` leaves that assignment untouched here.
    let mut proof_fact_scopes = program
        .domain_definitions
        .iter()
        .map(|domain| {
            // The carrier subject is a value binder, not a generic index.
            // Both use this declaration's lexical scope; indexed membership
            // arguments retain only the generic telescope below.
            let local_symbols = program
                .tables
                .declarations
                .data_type_parameters
                .span_or_empty(domain.type_parameters)
                .iter()
                .map(|parameter| (parameter.name.as_str().to_owned(), parameter.symbol))
                .chain(std::iter::once((
                    domain.subject_name.as_str().to_owned(),
                    domain.subject_symbol,
                )))
                .chain(
                    std::iter::once(("self".to_owned(), domain.subject_symbol))
                        .filter(|_| domain.subject_name.as_str() != "self"),
                )
                .collect();
            (
                domain.facts,
                local_symbols,
                Some(domain.type_parameters),
                false,
            )
        })
        .collect::<Vec<_>>();
    proof_fact_scopes.extend(
        program
            .tables
            .declarations
            .signature_contracts
            .iter()
            .map(|(_, contract)| (contract.facts, Vec::new(), None, false)),
    );
    proof_fact_scopes.extend(program.data_definitions.iter().flat_map(|definition| {
        // Value binders have leading descriptor fields, in telescope order
        // (lowering::data). A predicate observes that captured field, not a
        // second same-spelled type parameter. Keep only this producer-owned
        // association out of the lexical roster; authored duplicate fields
        // still remain ambiguous, and const/type parameters keep their roles.
        let descriptor_parameters = program
            .data_type_parameters(definition.type_parameters)
            .iter()
            .filter(|parameter| {
                matches!(
                    parameter.kind,
                    symbol_resolved_trees::data::TypeParameterKind::Value { .. }
                )
            })
            .zip(program.data_members(definition.members))
            .filter_map(|(parameter, member)| match member {
                symbol_resolved_trees::data::DataMember::Field(field)
                    if field.name == parameter.name =>
                {
                    Some(parameter.symbol)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut local_symbols = program
            .tables
            .declarations
            .data_type_parameters
            .span_or_empty(definition.type_parameters)
            .iter()
            .filter(|parameter| !descriptor_parameters.contains(&parameter.symbol))
            .map(|parameter| (parameter.name.as_str().to_owned(), parameter.symbol))
            .collect::<Vec<_>>();
        local_symbols.extend(
            program
                .tables
                .declarations
                .data_members
                .span_or_empty(definition.members)
                .iter()
                .filter_map(|member| match member {
                    symbol_resolved_trees::data::DataMember::Field(field) => {
                        Some((field.name.as_str().to_owned(), field.symbol))
                    }
                    symbol_resolved_trees::data::DataMember::Variant(_) => None,
                }),
        );
        // A case's own `where` facts resolve in the definition scope PLUS the
        // case's payload fields; payload names shadow same-named common
        // entries (the scope lookup refuses ambiguous duplicates, so shadowed
        // commons are dropped rather than listed twice).
        let variant_scopes = program
            .tables
            .declarations
            .data_members
            .span_or_empty(definition.members)
            .iter()
            .filter_map(|member| match member {
                symbol_resolved_trees::data::DataMember::Variant(variant)
                    if !variant.where_facts.is_empty() =>
                {
                    let mut scope = program
                        .tables
                        .declarations
                        .data_payload_fields
                        .span_or_empty(variant.payload)
                        .iter()
                        .map(|field| (field.name.as_str().to_owned(), field.symbol))
                        .collect::<Vec<_>>();
                    let inherited = local_symbols
                        .iter()
                        .filter(|(name, _)| {
                            !scope.iter().any(|(payload_name, _)| payload_name == name)
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    scope.extend(inherited);
                    Some((
                        variant.where_facts,
                        scope,
                        Some(definition.type_parameters),
                        true,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        std::iter::once((
            definition.where_facts,
            local_symbols,
            Some(definition.type_parameters),
            true,
        ))
        .chain(variant_scopes)
    }));
    let domain_path_members = &program.tables.declarations.domain_path_members;
    let data_type_parameters = &program.tables.declarations.data_type_parameters;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    let type_constraints = &program.tables.types.constraints;
    let proof_facts = &mut program.tables.declarations.proof_facts;

    for (facts, local_symbols, type_parameters, logical_data_subject) in proof_fact_scopes {
        let local_type_parameters =
            type_parameters.map(|parameters| data_type_parameters.span_or_empty(parameters));
        for fact in proof_facts.span_mut_or_empty(facts) {
            match fact {
                symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    if let Some(type_parameters) = type_parameters {
                        let local_type_parameters =
                            data_type_parameters.span_or_empty(type_parameters);
                        super::type_references::assign_type_reference_argument_symbols_with_constraints(
                            symbols,
                            child_type_references,
                            type_constraints,
                            local_type_parameters,
                            membership.domain_arguments,
                        );
                    }
                    assign_proof_expression_symbols(
                        symbols,
                        &domain_symbols,
                        child_type_references,
                        type_constraints,
                        local_type_parameters,
                        &local_symbols,
                        logical_data_subject,
                        &mut program.tables.bodies.expressions,
                        membership.value,
                    );
                    let members = domain_path_members.span_or_empty(membership.domain);
                    let name = members.joined_path();
                    membership.domain_symbol = resolve_domain_symbol(
                        symbols,
                        &domain_symbols,
                        &name,
                        diagnostic_path_source_span(members),
                    );
                }
                symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                    assign_proof_expression_symbols(
                        symbols,
                        &domain_symbols,
                        child_type_references,
                        type_constraints,
                        local_type_parameters,
                        &local_symbols,
                        logical_data_subject,
                        &mut program.tables.bodies.expressions,
                        *expression,
                    );
                }
            }
        }
    }

    update_data_zero_gates(program);
}

/// Resolve the conservative declaration gate after lexical subjects and domains
/// are bound. Only stored fields are born zero; static binders remain unknown
/// until typed application preparation retains their actual arguments.
fn update_data_zero_gates(program: &mut SymbolResolvedTrees) {
    let membership_results = program
        .data_definitions
        .iter()
        .map(|definition| {
            let facts = program
                .tables
                .declarations
                .proof_facts
                .span_or_empty(definition.where_facts);
            let mut saw_membership = false;
            let mut memberships_admit_zero = true;
            for fact in facts {
                let symbol_resolved_trees::domain::ProofFact::Membership(membership) = fact else {
                    continue;
                };
                saw_membership = true;
                memberships_admit_zero &=
                    resolved_domain_byte_predicate(program, membership.domain_symbol)
                        .is_some_and(|predicate| predicate.holds_for(&[]));
            }
            let expressions_gate_zero = facts
                .iter()
                .filter_map(|fact| match fact {
                    symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                        Some(*expression)
                    }
                    symbol_resolved_trees::domain::ProofFact::Membership(_) => None,
                })
                .any(|expression| {
                    zero_fold(program, definition, expression).is_none_or(|value| value == 0)
                });
            // CASE-CONSTRAINTS first-case zero gating: the zero tag is the
            // FIRST variant. When one of its `where` facts is a literal false
            // witness (a `T == i32` refuted by this instance's argument, or an
            // authored `where false`), the case can never hold, so the zero
            // value violates the case constraint and the data cannot be born
            // established. A fact merely false AT ZERO keeps its ordinary
            // construction-side proof instead of this gate.
            let first_variant_gates_zero = program
                .tables
                .declarations
                .data_members
                .span_or_empty(definition.members)
                .iter()
                .filter_map(|member| match member {
                    symbol_resolved_trees::data::DataMember::Variant(variant) => Some(variant),
                    symbol_resolved_trees::data::DataMember::Field(_) => None,
                })
                .next()
                .is_some_and(|variant| {
                    program
                        .tables
                        .declarations
                        .proof_facts
                        .span_or_empty(variant.where_facts)
                        .iter()
                        .any(|fact| match fact {
                            symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                                fact_expression_is_constant_false(program, *expression)
                            }
                            symbol_resolved_trees::domain::ProofFact::Membership(_) => false,
                        })
                });
            (
                saw_membership,
                memberships_admit_zero,
                expressions_gate_zero,
                first_variant_gates_zero,
            )
        })
        .collect::<Vec<_>>();

    let mut membership_results = membership_results.into_iter();
    program.data_definitions.for_each_mut(|definition| {
        let (
            saw_membership,
            memberships_admit_zero,
            expressions_gate_zero,
            first_variant_gates_zero,
        ) = membership_results
            .next()
            .expect("one zero result per data definition");
        definition.zero_gated = expressions_gate_zero
            || first_variant_gates_zero
            || (saw_membership && !memberships_admit_zero);
    });
}

fn zero_fold(
    program: &SymbolResolvedTrees,
    definition: &symbol_resolved_trees::data::DataDefinition,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<i128> {
    use symbol_resolved_trees::expression::{BinaryOperator, ExpressionNode};
    match program.tables.bodies.expressions.expression(expression) {
        ExpressionNode::Name(path) => program
            .tables
            .declarations
            .data_members
            .span_or_empty(definition.members)
            .iter()
            .any(|member| {
                matches!(member, symbol_resolved_trees::data::DataMember::Field(field)
                    if path.symbol.is_valid() && field.symbol == path.symbol)
            })
            .then_some(0),
        ExpressionNode::Member(member)
            if language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
                member.member.as_str(),
            )
            .is_some() =>
        {
            // A stored zero sequence is empty. A call or static subject is
            // not a field valuation merely because its member is named len.
            (zero_fold(program, definition, member.receiver)? == 0).then_some(0)
        }
        ExpressionNode::Integer(literal) => literal.decimal_value_i128(),
        ExpressionNode::Binary(binary) => {
            let left = zero_fold(program, definition, binary.left)?;
            let right = zero_fold(program, definition, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => left.checked_add(right),
                BinaryOperator::Subtract => left.checked_sub(right),
                BinaryOperator::Multiply => left.checked_mul(right),
                BinaryOperator::LessOrEqual => Some(i128::from(left <= right)),
                BinaryOperator::Less => Some(i128::from(left < right)),
                BinaryOperator::GreaterOrEqual => Some(i128::from(left >= right)),
                BinaryOperator::Greater => Some(i128::from(left > right)),
                BinaryOperator::Equal => Some(i128::from(left == right)),
                BinaryOperator::NotEqual => Some(i128::from(left != right)),
                BinaryOperator::And => Some(i128::from(left != 0 && right != 0)),
                BinaryOperator::Or => Some(i128::from(left != 0 || right != 0)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A proof-fact expression that is a literal FALSE constant: the `0` witness
/// synthesis leaves on an instance case whose `T == name` refuted, or an
/// authored `where false` / `where 0`. Anything requiring evaluation is not
/// counted -- an unprovable fact is not a contradiction.
fn fact_expression_is_constant_false(
    program: &SymbolResolvedTrees,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) -> bool {
    match program.tables.bodies.expressions.expression(expression) {
        symbol_resolved_trees::expression::ExpressionNode::Integer(literal) => {
            literal.decimal_value_i128() == Some(0)
        }
        symbol_resolved_trees::expression::ExpressionNode::Boolean(value) => !*value,
        _ => false,
    }
}

fn resolved_domain_byte_predicate(
    program: &SymbolResolvedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program.domain_definition_by_symbol(domain_symbol)?;
    let [symbol_resolved_trees::domain::ProofFact::Expression(expression)] =
        program.proof_facts(domain.facts)
    else {
        return None;
    };
    let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
        program.tables.bodies.expressions.expression(*expression)
    else {
        return None;
    };
    if call.receiver.is_valid() {
        return None;
    }
    let predicate = ByteSequencePredicate::from_name(call.target.as_str())?;
    let [argument] = program
        .tables
        .bodies
        .expressions
        .expression_handles(call.arguments)
    else {
        return None;
    };
    let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
        program.tables.bodies.expressions.expression(*argument)
    else {
        return None;
    };
    let members = program
        .tables
        .bodies
        .expressions
        .name_path_members(path.members);
    matches!(members, [member] if member.is_self_receiver()).then_some(predicate)
}

/// One proof-surface expression walk: declaration predicates (membership
/// values and `where` facts) carry their declaration's generic scope, its
/// lexical subjects, and the domain roster through every child visit. The
/// walk carries that context instead of re-threading it through each
/// recursive call.
struct ProofExpressionContext<'visit> {
    symbols: &'visit SymbolTable,
    domain_symbols: &'visit [(String, SymbolHandle, language_semantics::SemanticDomainId)],
    child_type_references: &'visit mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &'visit arena::Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: Option<&'visit [symbol_resolved_trees::data::TypeParameter]>,
    local_symbols: &'visit [(String, SymbolHandle)],
    logical_data_subject: bool,
    expression_table: &'visit mut symbol_resolved_trees::expression::ExpressionTable,
}

fn assign_proof_expression_symbols(
    symbols: &SymbolTable,
    domain_symbols: &[(String, SymbolHandle, language_semantics::SemanticDomainId)],
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &arena::Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: Option<&[symbol_resolved_trees::data::TypeParameter]>,
    local_symbols: &[(String, SymbolHandle)],
    logical_data_subject: bool,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    ProofExpressionContext {
        symbols,
        domain_symbols,
        child_type_references,
        type_constraints,
        local_type_parameters,
        local_symbols,
        logical_data_subject,
        expression_table,
    }
    .assign(expression);
}

impl ProofExpressionContext<'_> {
    /// Bind the fact's lexical subject: the single local or fact-parameter
    /// whose spelling matches the path's leaf. Several candidates stay
    /// unresolved rather than exposing an outer declaration, and an
    /// already-bound path is left alone.
    fn assign_lexical_subject(
        &mut self,
        members: arena::HandleSpan<symbol_resolved_trees::name::DiagnosticName>,
        member_symbols: arena::HandleSpan<SymbolHandle>,
        expression: symbol_resolved_trees::expression::ExpressionHandle,
    ) -> bool {
        let Some(name) = self.expression_table.name_path_members(members).first() else {
            return false;
        };
        let matching = {
            let mut matches = self
                .local_symbols
                .iter()
                .filter(|(candidate, symbol)| candidate == name.as_str() && symbol.is_valid());
            match (matches.next(), matches.next()) {
                (Some((_, symbol)), None) => Some(*symbol),
                _ => None,
            }
        };
        let Some(symbol) = matching else {
            return false;
        };
        if let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
            self.expression_table.expression_mut(expression)
        {
            path.head_symbol = symbol;
            if members.count() == 1 {
                path.symbol = symbol;
            }
        }
        if member_symbols.count() != 0 {
            self.expression_table
                .set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
        }
        true
    }

    fn assign(&mut self, expression: symbol_resolved_trees::expression::ExpressionHandle) {
        let expression_node = self.expression_table.expression(expression).clone();
        match expression_node {
            symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
                self.assign(dispatch.subject);
                for arm in self.expression_table.match_arms(dispatch.arms).to_vec() {
                    for pattern in self
                        .expression_table
                        .match_pattern_dependencies(arm.pattern)
                        .collect::<Vec<_>>()
                    {
                        self.assign(pattern);
                    }
                    self.assign(arm.value);
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::SavedSubject(_) => {}
            symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
                self.assign(atomic.value);
                if atomic.result.is_valid() {
                    self.assign(atomic.result);
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                for value in self.expression_table.expression_handles(values).to_vec() {
                    self.assign(value);
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
                self.assign(binary.left);
                self.assign(binary.right);
            }
            symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    self.assign(call.receiver);
                }
                for argument in self
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec()
                {
                    self.assign(argument);
                }
                if !call.receiver.is_valid() && !call.target_symbol.is_valid() {
                    // Declaration predicates select the same callable identities
                    // as machine contracts: authored declarations shadow builtins,
                    // and ambiguity stays unresolved. A machine-only lookup loses
                    // mathematical definitions and compiler proof functions before
                    // their independent proof-term admission can judge the call.
                    // A lexical binder also shadows a callable even when the
                    // binder is not callable. Duplicate binders stay unresolved;
                    // they must not expose the outer builtin through ambiguity.
                    let mut lexical_matches = self
                        .local_symbols
                        .iter()
                        .filter(|(name, _)| name == call.target.as_str());
                    let target_symbol = match (lexical_matches.next(), lexical_matches.next()) {
                        (Some((_, symbol)), None) => *symbol,
                        (Some(_), Some(_)) => SymbolHandle::invalid(),
                        (None, _) => resolve_free_call_target_symbol(self.symbols, &call.target),
                    };
                    if let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                        self.expression_table.expression_mut(expression)
                    {
                        call.target_symbol = target_symbol;
                    }
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
                // Declaration predicates have the same type-bearing cast syntax as
                // machine contracts. Resolve its target and qualification indices
                // in this declaration's generic scope, not from its later spelling.
                // Signature contracts already used their machine/trait scope.
                if let Some(parameters) = self.local_type_parameters {
                    for reference in std::iter::once(cast.target_type)
                        .chain(cast.semantic_domain_arguments.handles())
                    {
                        let mut target = self.child_type_references.get(reference).clone();
                        super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                        self.symbols,
                        self.child_type_references,
                        self.type_constraints,
                        parameters,
                        &mut target,
                    );
                        *self.child_type_references.get_mut(reference) = target;
                    }
                }
                self.assign(cast.value);
            }
            symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
                self.assign(indexed.collection);
                self.assign(indexed.index);
            }
            symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    self.assign(range.start);
                }
                if range.end.is_valid() {
                    self.assign(range.end);
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
                self.assign(membership.value);
                let (name, reference_span) = {
                    let members = self.expression_table.name_path_members(membership.domain);
                    (members.joined_path(), diagnostic_path_source_span(members))
                };
                // Proof expressions live in the shared body-expression table, but
                // are not visited by the ordinary machine-expression resolver.
                // Assign the same exact declared-domain or Type::Case identities
                // here, at their symbol-resolution owner, before reconciling the
                // specialized domain lookup below.
                assign_membership_symbol(
                    self.symbols,
                    self.expression_table,
                    membership.domain,
                    expression,
                );
                let domain_symbol =
                    resolve_domain_symbol(self.symbols, self.domain_symbols, &name, reference_span);
                if let symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
                    self.expression_table.expression_mut(expression)
                {
                    membership.domain_symbol = domain_symbol;
                    if domain_symbol.is_valid() {
                        membership.case_type_symbol = SymbolHandle::invalid();
                        membership.case_symbol = SymbolHandle::invalid();
                    }
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
                // A declaration predicate's logical subject projects to its
                // exact field binder. No runtime receiver is introduced.
                let logical_receiver = self.logical_data_subject
                    && matches!(self.expression_table.expression(member.receiver),
                        symbol_resolved_trees::expression::ExpressionNode::Name(path)
                        if self.expression_table.name_path_members(path.members).len() == 1
                            && self.expression_table.name_path_members(path.members)[0].as_str() == "self");
                if logical_receiver {
                    let candidates = self
                        .local_symbols
                        .iter()
                        .filter(|(name, symbol)| {
                            name == member.member.as_str()
                                && self.symbols.get(*symbol).kind == SymbolKind::Field
                        })
                        .map(|(_, symbol)| *symbol)
                        .collect::<Vec<_>>();
                    if let [symbol] = candidates.as_slice() {
                        let members = self.expression_table.reserve_name_path_members(1);
                        self.expression_table.set_name_path_member_at_offset(
                            members,
                            0,
                            member.member,
                        );
                        let member_symbols =
                            self.expression_table.reserve_name_path_member_symbols(1);
                        self.expression_table.set_name_path_member_symbol_at_offset(
                            member_symbols,
                            0,
                            *symbol,
                        );
                        *self.expression_table.expression_mut(expression) =
                            symbol_resolved_trees::expression::ExpressionNode::Name(
                                symbol_resolved_trees::expression::TableNamePath::new(
                                    members,
                                    member_symbols,
                                    *symbol,
                                    *symbol,
                                ),
                            );
                        return;
                    }
                }
                self.assign(member.receiver);
            }
            symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
                self.assign(inner.target);
            }
            symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
                self.assign(unary.operand);
            }
            symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
                for field in self
                    .expression_table
                    .struct_fields(struct_literal.fields)
                    .to_vec()
                {
                    self.assign(field.value);
                }
                super::expressions::assign_struct_literal_symbols(
                    self.symbols,
                    self.expression_table,
                    expression,
                );
            }
            symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
                // Lexical subjects keep their exact local identity: an
                // already-bound path is left alone, a sole matching subject
                // binds below, and remaining static type names select the
                // complete authored path through source visibility; namespace
                // prefixes retain the selected declaration's ancestry, never
                // independent leaf or prefix lookups that lose narrow imports.
                if path.head_symbol.is_valid() || path.symbol.is_valid() {
                    return;
                }
                // Declaration facts bind their lexical subjects first, then use
                // the ordinary value constructor selection. A payload-free case
                // is a value here, not a type name or a membership classifier.
                if self.assign_lexical_subject(path.members, path.member_symbols, expression) {
                    return;
                }
                if super::expressions::normalize_qualified_case_value(
                    self.symbols,
                    self.expression_table,
                    &path,
                    expression,
                ) {
                    return;
                }
                let members = self.expression_table.name_path_members(path.members);
                if members.is_empty()
                    || matches!(members, [member] if language_core::is_reserved_result(member.as_str()))
                {
                    return;
                }
                let name = symbol_resolved_trees::name::DiagnosticName::from_str(
                    &members.joined_path(),
                    diagnostic_path_source_span(members),
                );
                // Facts contain mathematical values as well as type names. A
                // type-only lookup drops members such as the selected address
                // model's bound before checking can consume their identity.
                // Select the complete visible declaration, retaining lexical
                // shadowing above and the fixed core vocabulary's ownership.
                let fixed = symbols::FixedCoreDeclaration::from_name(name.as_str())
                    .and_then(|declaration| self.symbols.fixed_core_declaration_symbol(declaration))
                    .unwrap_or_default();
                let (head, symbol, member_symbols) = if fixed.is_valid() {
                    let mut selected = fixed;
                    let mut member_symbols = Vec::with_capacity(members.len());
                    for _ in members {
                        if !selected.is_valid() || selected == self.symbols.root() {
                            return;
                        }
                        member_symbols.push(selected);
                        selected = self.symbols.namespace_parent(selected);
                    }
                    member_symbols.reverse();
                    (member_symbols[0], fixed, member_symbols)
                } else {
                    let (head, symbol) = super::scoped_paths::resolve_state_scoped_table_path(
                        self.symbols,
                        SymbolHandle::invalid(),
                        SymbolHandle::invalid(),
                        self.expression_table,
                        &path,
                        None,
                    );
                    let members =
                        super::scoped_paths::resolve_state_scoped_table_path_member_symbols(
                            self.symbols,
                            SymbolHandle::invalid(),
                            SymbolHandle::invalid(),
                            self.expression_table,
                            &path,
                        );
                    (head, symbol, members)
                };
                if !symbol.is_valid() {
                    return;
                }
                for (offset, selected) in (0u32..).zip(member_symbols) {
                    self.expression_table.set_name_path_member_symbol_at_offset(
                        path.member_symbols,
                        offset,
                        selected,
                    );
                }
                if let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                    self.expression_table.expression_mut(expression)
                {
                    path.head_symbol = head;
                    path.symbol = symbol;
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
            | symbol_resolved_trees::expression::ExpressionNode::Float(_)
            | symbol_resolved_trees::expression::ExpressionNode::UndeterminedNaN(_)
            | symbol_resolved_trees::expression::ExpressionNode::Integer(_)
            | symbol_resolved_trees::expression::ExpressionNode::String(_)
            | symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_) => {}
        }
    }
}

fn resolve_domain_symbol(
    symbols: &SymbolTable,
    domain_symbols: &[(String, SymbolHandle, language_semantics::SemanticDomainId)],
    name: &str,
    reference: source::SourceSpan,
) -> SymbolHandle {
    // One selection law for qualified, relative, and leaf spellings: the
    // complete logical path always selects exactly, while a relative or leaf
    // spelling must reach the declaration through its module exposure. A
    // same-module declaration outranks every other tier. Logical paths can be
    // equal across packages: a full-path match must rejoin ordinary source-owned
    // lookup, not add every public declaration with that display name. Exact
    // selected imports still compete with independently exposed attachments.
    let selected = symbols
        .find_top_level_by_name_and_kinds_from_source(name, &[SymbolKind::Domain], reference)
        .unwrap_or_else(SymbolHandle::invalid);
    let matches = domain_symbols
        .iter()
        .filter(|(candidate, symbol, _)| {
            symbols.source_reference_can_see_symbol(reference, *symbol)
                && (*symbol == selected
                    || (!(name.contains("::") && symbols.display_path(*symbol, "::") == name)
                        && super::lookup::domain_name_reaches(
                            symbols, *symbol, candidate, name, reference,
                        )))
        })
        .map(|(name, symbol, semantic_id)| (name.as_str(), *symbol, *semantic_id))
        .collect::<Vec<_>>();
    let reference_module = if reference.span.start == reference.span.end {
        SymbolHandle::invalid()
    } else {
        symbols.source_module(reference.source_id)
    };
    let local = matches
        .iter()
        .copied()
        .filter(|(_, symbol, _)| {
            reference_module.is_valid() && symbols.symbol_module(*symbol) == reference_module
        })
        .collect::<Vec<_>>();
    // Tiered ownership law, mirroring type-position selection: the
    // occurrence's own module outranks every other declaration, then its own
    // package outranks foreign packages. A foreign-only pool stays contested
    // — distinct owners sharing a leaf still fail closed below rather than
    // silently selecting a declaration the occurrence never owned.
    let pool = if !local.is_empty() {
        local
    } else if reference.span.start != reference.span.end {
        let own_package = matches
            .iter()
            .copied()
            .filter(|(_, symbol, _)| {
                symbols
                    .symbol_provenance_source_span(*symbol)
                    .is_some_and(|declaration| symbols.same_source_package(reference, declaration))
            })
            .collect::<Vec<_>>();
        if own_package.is_empty() {
            matches
        } else {
            own_package
        }
    } else {
        matches
    };
    if let Some((first_name, symbol, first_semantic_id)) = pool.first() {
        // Capacity-specialized carriers normalize their domain owner to the
        // same published name (`[u8; N]::Utf8`). Multiple declarations with
        // that exact normalized name are one semantic lookup candidate, not
        // an ambiguous set. Distinct owners that merely share the trailing
        // presentation name remain ambiguous and fail closed.
        if pool.iter().any(|(candidate, _, semantic_id)| {
            candidate != first_name || semantic_id != first_semantic_id
        }) {
            SymbolHandle::invalid()
        } else {
            *symbol
        }
    } else if name.contains("::") {
        symbols
            .find_top_level_by_name_and_kinds_from_source(name, &[SymbolKind::Domain], reference)
            .unwrap_or_else(SymbolHandle::invalid)
    } else {
        SymbolHandle::invalid()
    }
}
