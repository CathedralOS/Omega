use language_semantics::byte_predicates::ByteSequencePredicate;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::{SymbolHandle, SymbolKind, SymbolTable};

use super::expressions::assign_membership_symbol;
use super::lookup::diagnostic_path_source_span;
use super::targets::resolve_free_machine_entry_state_symbol;

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
                            let name = members
                                .iter()
                                .map(|member| member.as_str())
                                .collect::<Vec<_>>()
                                .join("::");
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
    // application's arguments (`self.slot in Resident<P, T>`) resolve against
    // the owning binders. Signature contracts resolve theirs in the machine
    // or trait scope during contract-reference assignment, which runs first;
    // `None` leaves that assignment untouched here.
    let mut proof_fact_scopes = program
        .domain_definitions
        .iter()
        .map(|domain| {
            // Predicate operands use the same declaration-local telescope as
            // membership arguments. Reserved `self` denotes the classified
            // value, not an index binder or a surrounding declaration.
            let local_symbols = program
                .tables
                .declarations
                .data_type_parameters
                .span_or_empty(domain.type_parameters)
                .iter()
                .filter(|parameter| parameter.name.as_str() != "self")
                .map(|parameter| (parameter.name.as_str().to_owned(), parameter.symbol))
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
        for fact in proof_facts.span_mut_or_empty(facts) {
            match fact {
                symbol_resolved_trees::domain::ProofFact::Membership(membership) => {
                    if let Some(type_parameters) = type_parameters {
                        let local_type_parameters =
                            data_type_parameters.span_or_empty(type_parameters);
                        for offset in 0..membership.domain_arguments.count() {
                            let start = membership.domain_arguments.start();
                            let handle = arena::Handle::from_parts(
                                start.arena_index() + offset,
                                start.generation(),
                            );
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
                        &mut program.tables.bodies.expressions,
                        membership.value,
                    );
                    let members = domain_path_members.span_or_empty(membership.domain);
                    let name = members
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
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
                        &mut program.tables.bodies.expressions,
                        *expression,
                    );
                }
            }
        }
    }

    update_data_membership_zero_gates(program);
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
        | symbol_resolved_trees::expression::ExpressionNode::Integer(_)
        | symbol_resolved_trees::expression::ExpressionNode::String(_)
        | symbol_resolved_trees::expression::ExpressionNode::ZeroValue(_) => {}
    }
}

/// Membership facts begin conservatively zero-gated during declaration
/// lowering because their domain symbols do not exist yet. Once all symbols
/// are assigned, clear that conservative contribution exactly when every
/// membership names a recognized byte-predicate fact that accepts the empty byte
/// sequence. Expression facts have already contributed their independent
/// zero result, so this pass only ever preserves or clears membership gating.
fn update_data_membership_zero_gates(program: &mut SymbolResolvedTrees) {
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
                    crate::lowering::data::zero_fold(&program.tables.bodies.expressions, expression)
                        .is_none_or(|value| value == 0)
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
    let domain = program
        .domain_definitions
        .iter()
        .find(|domain| domain.symbol == domain_symbol)?;
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
    matches!(members, [member] if member.as_str() == "self").then_some(predicate)
}

fn assign_proof_expression_symbols(
    symbols: &SymbolTable,
    domain_symbols: &[(String, SymbolHandle, language_semantics::SemanticDomainId)],
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    let expression_node = expression_table.expression(expression).clone();
    match expression_node {
        symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                dispatch.subject,
            );
            for arm in expression_table.match_arms(dispatch.arms).to_vec() {
                if let symbol_resolved_trees::expression::MatchPattern::Value(pattern) = arm.pattern
                {
                    assign_proof_expression_symbols(
                        symbols,
                        domain_symbols,
                        expression_table,
                        pattern,
                    );
                }
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    arm.value,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                atomic.value,
            );
            if atomic.result.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    atomic.result,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
            for value in expression_table.expression_handles(values).to_vec() {
                assign_proof_expression_symbols(symbols, domain_symbols, expression_table, value);
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
            assign_proof_expression_symbols(symbols, domain_symbols, expression_table, binary.left);
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                binary.right,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    call.receiver,
                );
            }
            for argument in expression_table.expression_handles(call.arguments).to_vec() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    argument,
                );
            }
            if !call.receiver.is_valid() && !call.target_symbol.is_valid() {
                let target_symbol = resolve_free_machine_entry_state_symbol(symbols, &call.target);
                if let symbol_resolved_trees::expression::ExpressionNode::Call(call) =
                    expression_table.expression_mut(expression)
                {
                    call.target_symbol = target_symbol;
                }
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
            assign_proof_expression_symbols(symbols, domain_symbols, expression_table, cast.value);
        }
        symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                indexed.collection,
            );
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                indexed.index,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
            if range.start.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    range.start,
                );
            }
            if range.end.is_valid() {
                assign_proof_expression_symbols(
                    symbols,
                    domain_symbols,
                    expression_table,
                    range.end,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                membership.value,
            );
            let (name, reference_span) = {
                let members = expression_table.name_path_members(membership.domain);
                (
                    members
                        .iter()
                        .map(|member| member.as_str())
                        .collect::<Vec<_>>()
                        .join("::"),
                    diagnostic_path_source_span(members),
                )
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
                expression_table,
                member.receiver,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
                expression_table,
                inner.target,
            );
        }
        symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
            assign_proof_expression_symbols(
                symbols,
                domain_symbols,
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
                    expression_table,
                    field.value,
                );
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
            // Fact-position names the local pass could not bind are not
            // fields or parameters: a case constraint's `T == i32` mentions
            // a TYPE. Resolve single-segment leftovers against top-level type
            // symbols so seeded-instance replay compares exact identities
            // rather than spellings.
            if !path.head_symbol.is_valid()
                && !path.symbol.is_valid()
                && let [member] = expression_table.name_path_members(path.members)
            {
                let symbol = super::lookup::top_level_type_symbol_for_source(symbols, member);
                if symbol.is_valid()
                    && let symbol_resolved_trees::expression::ExpressionNode::Name(path) =
                        expression_table.expression_mut(expression)
                {
                    path.head_symbol = symbol;
                    path.symbol = symbol;
                }
            }
        }
        symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
        | symbol_resolved_trees::expression::ExpressionNode::Float(_)
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
    let pool = if local.is_empty() { matches } else { local };
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
