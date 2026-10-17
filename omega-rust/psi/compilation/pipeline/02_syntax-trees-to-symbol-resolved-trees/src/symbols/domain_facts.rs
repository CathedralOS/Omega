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
                .collect();
            (domain.facts, local_symbols, Some(domain.type_parameters))
        })
        .collect::<Vec<_>>();
    proof_fact_scopes.extend(
        program
            .tables
            .declarations
            .signature_contracts
            .iter()
            .map(|(_, contract)| (contract.facts, Vec::new(), None)),
    );
    proof_fact_scopes.extend(program.data_definitions.iter().flat_map(|definition| {
        let mut local_symbols = program
            .tables
            .declarations
            .data_type_parameters
            .span_or_empty(definition.type_parameters)
            .iter()
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
                    Some((variant.where_facts, scope, Some(definition.type_parameters)))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        std::iter::once((
            definition.where_facts,
            local_symbols,
            Some(definition.type_parameters),
        ))
        .chain(variant_scopes)
    }));
    let domain_path_members = &program.tables.declarations.domain_path_members;
    let data_type_parameters = &program.tables.declarations.data_type_parameters;
    let child_type_references = &mut program.tables.declarations.child_type_references;
    let type_constraints = &program.tables.types.constraints;
    let proof_facts = &mut program.tables.declarations.proof_facts;

    for (facts, local_symbols, type_parameters) in proof_fact_scopes {
        let local_type_parameters =
            type_parameters.map(|parameters| data_type_parameters.span_or_empty(parameters));
        for fact in proof_facts.span_mut_or_empty(facts) {
            match fact {
                symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    if let Some(type_parameters) = type_parameters {
                        let local_type_parameters =
                            data_type_parameters.span_or_empty(type_parameters);
                        for handle in membership.domain_arguments.handles() {
                            let mut argument = child_type_references.get(handle).clone();
                            super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                                symbols,
                                child_type_references,
                                type_constraints,
                                local_type_parameters,
                                &mut argument,
                            );
                            *child_type_references.get_mut(handle) = argument;
                        }
                    }
                    assign_data_fact_local_symbols(
                        &local_symbols,
                        &mut program.tables.bodies.expressions,
                        membership.value,
                    );
                    assign_proof_expression_symbols(
                        symbols,
                        &domain_symbols,
                        child_type_references,
                        type_constraints,
                        local_type_parameters,
                        &local_symbols,
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
                    assign_data_fact_local_symbols(
                        &local_symbols,
                        &mut program.tables.bodies.expressions,
                        *expression,
                    );
                    assign_proof_expression_symbols(
                        symbols,
                        &domain_symbols,
                        child_type_references,
                        type_constraints,
                        local_type_parameters,
                        &local_symbols,
                        &mut program.tables.bodies.expressions,
                        *expression,
                    );
                }
            }
        }
    }

    update_data_zero_gates(program);
}

/// Domain predicates and data DEFAULT-DOMAIN facts use declaration scope,
/// not machine-body scope. Stamp their field and static-parameter references
/// here, while the exact
/// owning declarations are available, so later proof and package passes never
/// recover authority-bearing identities from authored spelling.
fn assign_data_fact_local_symbols(
    local_symbols: &[(String, SymbolHandle)],
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    if local_symbols.is_empty() {
        return;
    }

    let expression_node = expression_table.expression(expression).clone();
    match expression_node {
        symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, dispatch.subject);
            for arm in expression_table.match_arms(dispatch.arms).to_vec() {
                if let symbol_resolved_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                {
                    assign_data_fact_local_symbols(local_symbols, expression_table, pattern);
                }
                assign_data_fact_local_symbols(local_symbols, expression_table, arm.value);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, atomic.value);
            if atomic.result.is_valid() {
                assign_data_fact_local_symbols(local_symbols, expression_table, atomic.result);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in expression_table.expression_handles(values).to_vec() {
                assign_data_fact_local_symbols(local_symbols, expression_table, value);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, binary.left);
            assign_data_fact_local_symbols(local_symbols, expression_table, binary.right);
        }
        symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_data_fact_local_symbols(local_symbols, expression_table, call.receiver);
            }
            for argument in expression_table.expression_handles(call.arguments).to_vec() {
                assign_data_fact_local_symbols(local_symbols, expression_table, argument);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, cast.value);
        }
        symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, indexed.collection);
            assign_data_fact_local_symbols(local_symbols, expression_table, indexed.index);
        }
        symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_data_fact_local_symbols(local_symbols, expression_table, range.start);
            }
            if range.end.is_valid() {
                assign_data_fact_local_symbols(local_symbols, expression_table, range.end);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, membership.value);
        }
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, member.receiver);
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, inner.target);
        }
        symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_data_fact_local_symbols(local_symbols, expression_table, unary.operand);
        }
        symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            for field in expression_table
                .struct_fields(struct_literal.fields)
                .to_vec()
            {
                assign_data_fact_local_symbols(local_symbols, expression_table, field.value);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            if path.head_symbol.is_valid() || path.symbol.is_valid() {
                return;
            }
            let Some(name) = expression_table.name_path_members(path.members).first() else {
                return;
            };
            let mut matches = local_symbols
                .iter()
                .filter(|(candidate, symbol)| candidate == name.as_str() && symbol.is_valid());
            let Some((_, symbol)) = matches.next() else {
                return;
            };
            if matches.next().is_some() {
                return;
            }
            let symbol = *symbol;
            let member_symbols = path.member_symbols;
            let is_single_member = path.members.count() == 1;
            if let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
            {
                path.head_symbol = symbol;
                if is_single_member {
                    path.symbol = symbol;
                }
            }
            if member_symbols.count() != 0 {
                expression_table.set_name_path_member_symbol_at_offset(member_symbols, 0, symbol);
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
        ExpressionNode::Integer(literal) => literal.text().parse::<i128>().ok(),
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
            literal.text().parse::<i128>() == Ok(0)
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

fn assign_proof_expression_symbols(
    symbols: &SymbolTable,
    domain_symbols: &[(String, SymbolHandle, language_semantics::SemanticDomainId)],
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    type_constraints: &arena::Arena<symbol_resolved_trees::types::TypeConstraint>,
    local_type_parameters: Option<&[symbol_resolved_trees::data::TypeParameter]>,
    local_symbols: &[(String, SymbolHandle)],
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    let expression_node = expression_table.expression(expression).clone();
    match expression_node {
        symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                dispatch.subject,
            );
            for arm in expression_table.match_arms(dispatch.arms).to_vec() {
                if let symbol_resolved_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                {
                    assign_proof_expression_symbols(
                        symbols,
                        domain_symbols,
                        child_type_references,
                        type_constraints,
                        local_type_parameters,
                        local_symbols,
                        expression_table,
                        pattern,
                    );
                }
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    arm.value,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                atomic.value,
            );
            if atomic.result.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    atomic.result,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in expression_table.expression_handles(values).to_vec() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    value,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                binary.left,
            );
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                binary.right,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    call.receiver,
                );
            }
            for argument in expression_table.expression_handles(call.arguments).to_vec() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    argument,
                );
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
                let mut lexical_matches = local_symbols
                    .iter()
                    .filter(|(name, _)| name == call.target.as_str());
                let target_symbol = match (lexical_matches.next(), lexical_matches.next()) {
                    (Some((_, symbol)), None) => *symbol,
                    (Some(_), Some(_)) => SymbolHandle::invalid(),
                    (None, _) => resolve_free_call_target_symbol(symbols, &call.target),
                };
                if let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                    expression_table.expression_mut(expression)
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
            if let Some(parameters) = local_type_parameters {
                for reference in std::iter::once(cast.target_type)
                    .chain(cast.semantic_domain_arguments.handles())
                {
                    let mut target = child_type_references.get(reference).clone();
                    super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                        symbols,
                        child_type_references,
                        type_constraints,
                        parameters,
                        &mut target,
                    );
                    *child_type_references.get_mut(reference) = target;
                }
            }
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                cast.value,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                indexed.collection,
            );
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                indexed.index,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    range.start,
                );
            }
            if range.end.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    range.end,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                membership.value,
            );
            let (name, reference_span) = {
                let members = expression_table.name_path_members(membership.domain);
                (members.joined_path(), diagnostic_path_source_span(members))
            };
            // Proof expressions live in the shared body-expression table, but
            // are not visited by the ordinary machine-expression resolver.
            // Assign the same exact declared-domain or Type::Case identities
            // here, at their symbol-resolution owner, before reconciling the
            // specialized domain lookup below.
            assign_membership_symbol(symbols, expression_table, membership.domain, expression);
            let domain_symbol =
                resolve_domain_symbol(symbols, domain_symbols, &name, reference_span);
            if let symbol_resolved_trees::expression::ExpressionNode::Membership(membership) =
                expression_table.expression_mut(expression)
            {
                membership.domain_symbol = domain_symbol;
                if domain_symbol.is_valid() {
                    membership.case_type_symbol = SymbolHandle::invalid();
                    membership.case_symbol = SymbolHandle::invalid();
                }
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                member.receiver,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                inner.target,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                child_type_references,
                type_constraints,
                local_type_parameters,
                local_symbols,
                expression_table,
                unary.operand,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
            for field in expression_table
                .struct_fields(struct_literal.fields)
                .to_vec()
            {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    child_type_references,
                    type_constraints,
                    local_type_parameters,
                    local_symbols,
                    expression_table,
                    field.value,
                );
            }
            super::expressions::assign_struct_literal_symbols(
                symbols,
                expression_table,
                expression,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            // Local binders were assigned first. Remaining static type names
            // select the complete authored path through source visibility;
            // namespace prefixes retain the selected declaration's ancestry,
            // never independent leaf or prefix lookups that lose narrow imports.
            if path.head_symbol.is_valid() || path.symbol.is_valid() {
                return;
            }
            // Declaration facts bind their lexical subjects first, then use
            // the ordinary value constructor selection. A payload-free case
            // is a value here, not a type name or a membership classifier.
            if super::expressions::normalize_qualified_case_value(
                symbols,
                expression_table,
                &path,
                expression,
            ) {
                return;
            }
            let members = expression_table.name_path_members(path.members);
            if members.is_empty()
                || matches!(members, [member] if language_core::is_reserved_result(member.as_str()))
            {
                return;
            }
            let name = symbol_resolved_trees::name::DiagnosticName::from_str(
                &members.joined_path(),
                diagnostic_path_source_span(members),
            );
            let symbol = super::lookup::top_level_type_symbol_for_source(symbols, &name);
            if !symbol.is_valid() {
                return;
            }
            let mut selected = symbol;
            let mut member_symbols = Vec::with_capacity(members.len());
            for _ in members {
                if !selected.is_valid() || selected == symbols.root() {
                    return;
                }
                member_symbols.push(selected);
                // Authored module ownership differs from physical root
                // storage for declarations; preserve semantic ancestry.
                selected = symbols.namespace_parent(selected);
            }
            member_symbols.reverse();
            let head = member_symbols[0];
            for (offset, selected) in (0u32..).zip(member_symbols) {
                expression_table.set_name_path_member_symbol_at_offset(
                    path.member_symbols,
                    offset,
                    selected,
                );
            }
            if let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                expression_table.expression_mut(expression)
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
