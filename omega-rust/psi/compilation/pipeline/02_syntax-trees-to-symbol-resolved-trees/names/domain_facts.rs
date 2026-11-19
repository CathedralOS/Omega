/*
    Declaration-scoped facts bind lexical subjects before selecting remaining
    closed type names. Complete source-visible paths retain semantic namespace
    ancestry, including declarations physically stored at root, so authored
    selection custody survives generic equation discharge.
*/
use crate::resolution::tables::body_expressions;
use crate::resolution::tables::resolved_declarations;
use crate::source_resolved::SymbolResolvedTrees;
use crate::source_symbols::{JoinedPath, SymbolTable};
use declaration_contracts::byte_predicates::ByteSequencePredicate;
use symbols::{SymbolHandle, SymbolKind};

use super::expressions::assign_membership_symbol;
use super::expressions::assign_struct_literal_symbols;
use super::lookup::MembershipSelection;
use super::lookup::diagnostic_path_source_span;
use super::lookup::membership_selection;
use super::targets::resolve_free_call_target_symbol;
use crate::declaration_selection::CollectionMeasure;
use crate::source_resolved::data::DataDefinition;
use crate::source_resolved::data::DataMember;
use crate::source_resolved::data::TypeParameter;
use crate::source_resolved::data::TypeParameterKind;
use crate::source_resolved::domain::ProofFact;
use crate::source_resolved::expression::BinaryOperator;
use crate::source_resolved::expression::ExpressionHandle;
use crate::source_resolved::expression::ExpressionNode;
use crate::source_resolved::expression::ExpressionTable;
use crate::source_resolved::expression::TableNamePath;
use crate::source_resolved::name::DiagnosticName;
use crate::source_resolved::types::TypeConstraint;
use std::iter::once;

mod case_members {
    /*
        Guarded payload names retain the selected declaration, not proof of access.
        Specification formation independently checks the preceding conjunct and its
        exact receiver before admitting a payload projection.
    */

    use crate::source_resolved::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
    use crate::source_resolved::name::DiagnosticName;
    use symbols::{SymbolHandle, SymbolKind};

    use super::ProofExpressionContext;

    pub(super) struct CaseSelection {
        subject: ExpressionHandle,
        variant: SymbolHandle,
    }

    impl ProofExpressionContext<'_> {
        pub(super) fn collect_case_selections(&mut self, expression: ExpressionHandle) {
            match self.expression_table.expression(expression).clone() {
                ExpressionNode::Membership(membership) if membership.case_symbol.is_valid() => {
                    self.case_selections.push(CaseSelection {
                        subject: membership.value,
                        variant: membership.case_symbol,
                    });
                }
                ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
                    self.collect_case_selections(binary.left);
                    self.collect_case_selections(binary.right);
                }
                _ => {}
            }
        }

        pub(super) fn assign_guarded_case_member(&mut self, expression: ExpressionHandle) {
            let ExpressionNode::Member(member) = self.expression_table.expression(expression)
            else {
                return;
            };
            if member.member_symbol.is_valid() || member.case_variant.is_some() {
                return;
            }
            let mut selections = self
                .case_selections
                .iter()
                .filter(|selection| self.same_case_subject(selection.subject, member.receiver));
            let Some(selection) = selections.next() else {
                return;
            };
            let variant = selection.variant;
            if selections.any(|other| other.variant != variant) {
                return;
            }
            let Some(children) = self.symbols.child_handles(variant) else {
                return;
            };
            let mut fields = children.filter(|symbol| {
                self.symbols.get(*symbol).kind == SymbolKind::Field
                    && self.symbols.name(*symbol) == member.member.as_str()
            });
            let Some(field) = fields.next() else {
                return;
            };
            if fields.next().is_some() {
                return;
            }
            let case_name = DiagnosticName::generated(self.symbols.name(variant));
            if let ExpressionNode::Member(member) = self.expression_table.expression_mut(expression)
            {
                member.member_symbol = field;
                member.case_variant = Some(case_name);
            }
        }

        /*
            Only exact resolved storage paths share a selection. Equal spellings or
            independently evaluated calls do not identify the same subject.
        */
        fn same_case_subject(&self, left: ExpressionHandle, right: ExpressionHandle) -> bool {
            match (
                self.expression_table.expression(left),
                self.expression_table.expression(right),
            ) {
                (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
                    let left_members = self
                        .expression_table
                        .name_path_member_symbols(left.member_symbols);
                    let right_members = self
                        .expression_table
                        .name_path_member_symbols(right.member_symbols);
                    left.head_symbol.is_valid()
                        && left.symbol.is_valid()
                        && left.head_symbol == right.head_symbol
                        && left.symbol == right.symbol
                        && left_members.len() == left.members.count() as usize
                        && right_members.len() == right.members.count() as usize
                        && left_members.iter().all(|symbol| symbol.is_valid())
                        && left_members == right_members
                }
                (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
                    left.member_symbol.is_valid()
                        && left.member_symbol == right.member_symbol
                        && self.same_case_subject(left.receiver, right.receiver)
                }
                _ => false,
            }
        }
    }
}

pub(super) fn assign_domain_fact_symbols(program: &mut SymbolResolvedTrees, symbols: &SymbolTable) {
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
    let mut proof_fact_scopes = program
        .domain_definitions
        .iter()
        .map(|domain| {
            let local_symbols = resolved_declarations(program)
                .data_type_parameters
                .span_or_empty(domain.type_parameters)
                .iter()
                .map(|parameter| (parameter.name.as_str().to_owned(), parameter.symbol))
                .chain(once((
                    domain.subject_name.as_str().to_owned(),
                    domain.subject_symbol,
                )))
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
        resolved_declarations(program)
            .signature_contracts
            .iter()
            .map(|(_, contract)| (contract.facts, Vec::new(), None, false)),
    );
    proof_fact_scopes.extend(program.data_definitions.iter().flat_map(|definition| {
        let descriptor_parameters = program
            .data_type_parameters(definition.type_parameters)
            .iter()
            .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
            .zip(program.data_members(definition.members))
            .filter_map(|(parameter, member)| match member {
                DataMember::Field(field) if field.name == parameter.name => Some(parameter.symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut local_symbols = resolved_declarations(program)
            .data_type_parameters
            .span_or_empty(definition.type_parameters)
            .iter()
            .filter(|parameter| !descriptor_parameters.contains(&parameter.symbol))
            .map(|parameter| (parameter.name.as_str().to_owned(), parameter.symbol))
            .collect::<Vec<_>>();
        local_symbols.extend(
            resolved_declarations(program)
                .data_members
                .span_or_empty(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) => {
                        Some((field.name.as_str().to_owned(), field.symbol))
                    }
                    DataMember::Variant(_) => None,
                }),
        );
        if definition.predicate_subject_symbol.is_valid() {
            local_symbols.push(("self".to_owned(), definition.predicate_subject_symbol));
        }
        let variant_scopes = resolved_declarations(program)
            .data_members
            .span_or_empty(definition.members)
            .iter()
            .filter_map(|member| match member {
                DataMember::Variant(variant) if !variant.requires_facts.is_empty() => {
                    let mut scope = resolved_declarations(program)
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
                        variant.requires_facts,
                        scope,
                        Some(definition.type_parameters),
                        true,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        once((
            definition.requires_facts,
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
                ProofFact::Membership(membership) => {
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
                    membership.domain_symbol = proof_fact_membership_domain(
                        symbols,
                        &name,
                        diagnostic_path_source_span(members),
                    );
                    if child_type_references
                        .receiving_applications
                        .is_valid(membership.receiving_application)
                    {
                        let retained_name = child_type_references
                            .receiving_applications
                            .get(membership.receiving_application)
                            .name
                            .clone();
                        crate::resolution::constants::bind_receiving_application(
                            child_type_references,
                            membership.receiving_application,
                            &retained_name,
                            membership.domain_symbol,
                            membership.domain_arguments,
                        );
                    }
                }
                ProofFact::Expression(expression) => {
                    assign_proof_expression_symbols(
                        symbols,
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

/*
    Resolve the conservative declaration gate after lexical subjects and domains
    are bound. Only stored fields are born zero; static binders remain unknown
    until typed application preparation retains their actual arguments.
*/
fn update_data_zero_gates(program: &mut SymbolResolvedTrees) {
    let membership_results = program
        .data_definitions
        .iter()
        .map(|definition| {
            let facts = resolved_declarations(program)
                .proof_facts
                .span_or_empty(definition.requires_facts);
            let mut saw_membership = false;
            let mut memberships_admit_zero = true;
            for fact in facts {
                let ProofFact::Membership(membership) = fact else {
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
                    ProofFact::Expression(expression) => Some(*expression),
                    ProofFact::Membership(_) => None,
                })
                .any(|expression| {
                    zero_fold(program, definition, expression).is_none_or(|value| value == 0)
                });
            let first_variant_gates_zero = resolved_declarations(program)
                .data_members
                .span_or_empty(definition.members)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Variant(variant) => Some(variant),
                    DataMember::Field(_) => None,
                })
                .next()
                .is_some_and(|variant| {
                    resolved_declarations(program)
                        .proof_facts
                        .span_or_empty(variant.requires_facts)
                        .iter()
                        .any(|fact| match fact {
                            ProofFact::Expression(expression) => {
                                fact_expression_is_constant_false(program, *expression)
                            }
                            ProofFact::Membership(_) => false,
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
    definition: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<i128> {
    use crate::source_resolved::expression::{BinaryOperator, ExpressionNode};
    match program.tables.bodies.expressions.expression(expression) {
        ExpressionNode::Name(path) => resolved_declarations(program)
            .data_members
            .span_or_empty(definition.members)
            .iter()
            .any(|member| {
                matches!(member, DataMember::Field(field)
                if path.symbol.is_valid() && field.symbol == path.symbol)
            })
            .then_some(0),
        ExpressionNode::Member(member)
            if CollectionMeasure::from_authored_spelling(member.member.as_str()).is_some() =>
        {
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

/*
    A proof-fact expression that is a literal FALSE constant: the `0` witness
    synthesis leaves on an instance case whose `T == name` refuted, or an
    authored `where false` / `where 0`. Anything requiring evaluation is not
    counted -- an unprovable fact is not a contradiction.
*/
fn fact_expression_is_constant_false(
    program: &SymbolResolvedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.tables.bodies.expressions.expression(expression) {
        ExpressionNode::Integer(literal) => literal.decimal_value_i128() == Some(0),
        ExpressionNode::Boolean(value) => !*value,
        _ => false,
    }
}

fn resolved_domain_byte_predicate(
    program: &SymbolResolvedTrees,
    domain_symbol: SymbolHandle,
) -> Option<ByteSequencePredicate> {
    let domain = program.domain_definition_by_symbol(domain_symbol)?;
    let [ProofFact::Expression(expression)] = program.proof_facts(domain.facts) else {
        return None;
    };
    let ExpressionNode::Call(call) = program.tables.bodies.expressions.expression(*expression)
    else {
        return None;
    };
    if !call.target_symbol.is_valid()
        || !call.machine_arguments.is_empty()
        || !call.evidence_arguments.is_empty()
    {
        return None;
    }
    let target = if program.symbols.get(call.target_symbol).kind == symbols::SymbolKind::State {
        program.symbols.get(call.target_symbol).parent
    } else {
        call.target_symbol
    };
    let Some(language_core::sealed_intrinsics::SealedIntrinsic::CompilerPrimitive(
        language_core::sealed_intrinsics::CompilerPrimitive::ByteObservation(predicate),
    )) = program
        .symbols
        .sealed_declaration(target, program.symbols.name(target))
    else {
        return None;
    };
    let [argument] = body_expressions(program).expression_handles(call.arguments) else {
        return None;
    };
    let mut argument = *argument;
    if !matches!(
        program.tables.bodies.expressions.expression(argument),
        ExpressionNode::Borrow(_)
    ) {
        return None;
    }
    if let ExpressionNode::Borrow(borrow) = program.tables.bodies.expressions.expression(argument) {
        if borrow.access != language_core::ReferenceAccess::Shared {
            return None;
        }
        argument = borrow.target;
        if let ExpressionNode::Indexed(indexed) =
            program.tables.bodies.expressions.expression(argument)
            && let ExpressionNode::Range(range) =
                program.tables.bodies.expressions.expression(indexed.index)
        {
            if range.start.is_valid() || range.end.is_valid() || range.end_inclusive {
                return None;
            }
            argument = indexed.collection;
        }
    }
    let ExpressionNode::Name(path) = program.tables.bodies.expressions.expression(argument) else {
        return None;
    };
    let members = body_expressions(program).name_path_members(path.members);
    matches!(members, [member] if member.is_self_receiver()).then_some(predicate)
}

/*
    One proof-surface expression walk: declaration predicates (membership
    values and `requires` facts) carry their declaration's generic scope and
    its lexical subjects through every child visit. Domain spellings are
    read from the membership lookup at the node, not from a second roster.
*/
struct ProofExpressionContext<'visit> {
    symbols: &'visit SymbolTable,
    child_type_references: &'visit mut crate::source_resolved::types::TypeReferenceArena,
    type_constraints: &'visit arena::Arena<TypeConstraint>,
    local_type_parameters: Option<&'visit [TypeParameter]>,
    local_symbols: &'visit [(String, SymbolHandle)],
    logical_data_subject: bool,
    expression_table: &'visit mut ExpressionTable,
    case_selections: Vec<case_members::CaseSelection>,
}

fn assign_proof_expression_symbols(
    symbols: &SymbolTable,
    child_type_references: &mut crate::source_resolved::types::TypeReferenceArena,
    type_constraints: &arena::Arena<TypeConstraint>,
    local_type_parameters: Option<&[TypeParameter]>,
    local_symbols: &[(String, SymbolHandle)],
    logical_data_subject: bool,
    expression_table: &mut ExpressionTable,
    expression: ExpressionHandle,
) {
    ProofExpressionContext {
        symbols,
        child_type_references,
        type_constraints,
        local_type_parameters,
        local_symbols,
        logical_data_subject,
        expression_table,
        case_selections: Vec::new(),
    }
    .assign(expression);
}

impl ProofExpressionContext<'_> {
    /// Declaration facts own static operands just as callable expressions do.
    /// The shared resolver owns type/static binders and source visibility;
    /// remaining value subjects use this declaration's exact lexical roster.
    fn assign_static_argument(
        &mut self,
        argument: &mut crate::source_resolved::expression::StaticMachineArgument,
    ) {
        if argument.type_reference.is_valid() {
            let mut reference = self
                .child_type_references
                .get(argument.type_reference)
                .clone();
            super::type_references::assign_type_reference_symbol_with_locals_and_constraints(
                self.symbols,
                self.child_type_references,
                self.type_constraints,
                self.local_type_parameters.unwrap_or(&[]),
                &mut reference,
            );
            *self.child_type_references.get_mut(argument.type_reference) = reference;
            argument.symbol = SymbolHandle::invalid();
        } else if !argument.symbol.is_valid()
            && argument.const_literal.is_none()
            && argument.evidence_projection.is_none()
        {
            super::targets::assign_static_argument_symbol(
                self.symbols,
                SymbolHandle::invalid(),
                self.local_type_parameters.unwrap_or(&[]),
                argument,
                true,
            );
            let lexical = if !argument.symbol.is_valid()
                && let [name] = argument.path.as_ref()
            {
                let mut matching = self
                    .local_symbols
                    .iter()
                    .filter(|(candidate, _)| candidate == name.as_str());
                match (matching.next(), matching.next()) {
                    (None, _) => None,
                    (Some((_, symbol)), None) => Some(*symbol),
                    (Some(_), Some(_)) => Some(SymbolHandle::invalid()),
                }
            } else {
                None
            };
            if let Some(symbol) = lexical {
                argument.symbol = symbol;
            }
        }
        if let Some(application) = &mut argument.application {
            for nested in &mut application.arguments {
                self.assign_static_argument(nested);
            }
        }
    }
    /// Bind the fact's lexical subject: the single local or fact-parameter
    /// whose spelling matches the path's leaf. Several candidates stay
    /// unresolved rather than exposing an outer declaration, and an
    /// already-bound path is left alone.
    fn assign_lexical_subject(
        &mut self,
        members: arena::HandleSpan<DiagnosticName>,
        member_symbols: arena::HandleSpan<SymbolHandle>,
        expression: ExpressionHandle,
    ) -> bool {
        let Some(name) = self.expression_table.name_path_members(members).first() else {
            return false;
        };
        let matching = {
            let mut matches = self
                .local_symbols
                .iter()
                .filter(|(candidate, _)| candidate == name.as_str());
            match (matches.next(), matches.next()) {
                (Some((_, symbol)), None) => Some(Ok(*symbol)),
                (Some(_), Some(_)) => Some(Err(crate::source_symbols::UnresolvedName::Ambiguous)),
                (None, _) => None,
            }
        };
        let Some(symbol) = matching else {
            return false;
        };
        let symbol = match symbol {
            Ok(symbol) if symbol.is_valid() => symbol,
            Ok(_) => return true,
            Err(outcome) => {
                self.expression_table
                    .set_unresolved_name(expression, outcome);
                return true;
            }
        };
        if let ExpressionNode::Name(path) = self.expression_table.expression_mut(expression) {
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

    fn assign(&mut self, expression: ExpressionHandle) {
        let expression_node = self.expression_table.expression(expression).clone();
        match expression_node {
            ExpressionNode::Match(dispatch) => {
                self.assign(dispatch.subject);
                for arm in self.expression_table.match_arms(dispatch.arms).to_vec() {
                    for pattern in self
                        .expression_table
                        .match_pattern_dependencies(arm.pattern)
                        .collect::<Vec<_>>()
                    {
                        self.assign(pattern);
                    }
                    if let crate::source_resolved::expression::MatchPattern::Destructure(pattern) =
                        arm.pattern
                    {
                        super::expression_paths::assign_pattern_field_symbols(
                            self.symbols,
                            self.expression_table,
                            pattern,
                        );
                    }
                    self.assign(arm.value);
                }
            }
            ExpressionNode::SavedSubject(_) => {}
            ExpressionNode::Atomic(atomic) => {
                self.assign(atomic.value);
                if atomic.result.is_valid() {
                    self.assign(atomic.result);
                }
            }
            ExpressionNode::ArrayLiteral(values) => {
                for value in self.expression_table.expression_handles(values).to_vec() {
                    self.assign(value);
                }
            }
            ExpressionNode::Binary(binary) => {
                self.assign(binary.left);
                let selection_count = self.case_selections.len();
                if binary.operator == BinaryOperator::And {
                    self.collect_case_selections(binary.left);
                }
                self.assign(binary.right);
                self.case_selections.truncate(selection_count);
            }
            ExpressionNode::Call(call) => {
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
                // A declaration predicate takes the same source-scoped
                // static namespace as an ordinary body or callable clause.
                let normalized = super::targets::normalize_expression_module_call(
                    self.symbols,
                    self.expression_table,
                    expression,
                );
                if normalized.is_none()
                    && !call.receiver.is_valid()
                    && !call.target_symbol.is_valid()
                {
                    let mut lexical_matches = self
                        .local_symbols
                        .iter()
                        .filter(|(name, _)| name == call.target.as_str());
                    let (target_symbol, unresolved_name) =
                        match (lexical_matches.next(), lexical_matches.next()) {
                            (Some((_, symbol)), None) => {
                                (*symbol, crate::source_symbols::UnresolvedName::Unserved)
                            }
                            (Some(_), Some(_)) => (
                                SymbolHandle::invalid(),
                                crate::source_symbols::UnresolvedName::Ambiguous,
                            ),
                            (None, _) => {
                                let selected =
                                    resolve_free_call_target_symbol(self.symbols, &call.target);
                                let outcome = if selected.is_valid() {
                                    Default::default()
                                } else {
                                    super::paths::unresolved_scoped_call_name(
                                        self.symbols,
                                        SymbolHandle::invalid(),
                                        &[],
                                        &call.target,
                                    )
                                };
                                (selected, outcome)
                            }
                        };
                    self.expression_table
                        .set_unresolved_name(expression, unresolved_name);
                    if let ExpressionNode::Call(call) =
                        self.expression_table.expression_mut(expression)
                    {
                        call.target_symbol = target_symbol;
                    }
                }
                let mut arguments = call.machine_arguments;
                for argument in &mut arguments {
                    self.assign_static_argument(argument);
                }
                if let ExpressionNode::Call(call) = self.expression_table.expression_mut(expression)
                {
                    call.machine_arguments = arguments;
                }
            }
            ExpressionNode::Cast(cast) => {
                if let Some(parameters) = self.local_type_parameters {
                    for reference in
                        once(cast.target_type).chain(cast.semantic_domain_arguments.handles())
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
            ExpressionNode::Indexed(indexed) => {
                self.assign(indexed.collection);
                self.assign(indexed.index);
            }
            ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    self.assign(range.start);
                }
                if range.end.is_valid() {
                    self.assign(range.end);
                }
            }
            ExpressionNode::Membership(membership) => {
                self.assign(membership.value);
                let (name, reference_span) = {
                    let members = self.expression_table.name_path_members(membership.domain);
                    (members.joined_path(), diagnostic_path_source_span(members))
                };
                assign_membership_symbol(
                    self.symbols,
                    self.expression_table,
                    membership.domain,
                    expression,
                );
                let domain_symbol =
                    proof_fact_membership_domain(self.symbols, &name, reference_span);
                if let ExpressionNode::Membership(membership) =
                    self.expression_table.expression_mut(expression)
                {
                    membership.domain_symbol = domain_symbol;
                    if domain_symbol.is_valid() {
                        membership.case_type_symbol = SymbolHandle::invalid();
                        membership.case_symbol = SymbolHandle::invalid();
                    }
                }
            }
            ExpressionNode::Member(member) => {
                let logical_receiver = self.logical_data_subject
                    && matches!(self.expression_table.expression(member.receiver),
                    ExpressionNode::Name(path)
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
                        *self.expression_table.expression_mut(expression) = ExpressionNode::Name(
                            TableNamePath::new(members, member_symbols, *symbol, *symbol),
                        );
                        return;
                    }
                }
                self.assign(member.receiver);
                self.assign_guarded_case_member(expression);
            }
            ExpressionNode::Borrow(inner) => {
                self.assign(inner.target);
            }
            ExpressionNode::Unary(unary) => {
                self.assign(unary.operand);
            }
            ExpressionNode::StructLiteral(struct_literal) => {
                for field in self
                    .expression_table
                    .struct_fields(struct_literal.fields)
                    .to_vec()
                {
                    self.assign(field.value);
                }
                assign_struct_literal_symbols(self.symbols, self.expression_table, expression);
            }
            ExpressionNode::Name(path) => {
                if path.static_application.is_valid() {
                    return;
                }
                if path.head_symbol.is_valid() || path.symbol.is_valid() {
                    return;
                }
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
                let name = DiagnosticName::from_str(
                    &members.joined_path(),
                    diagnostic_path_source_span(members),
                );
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
                    let (head, symbol) = super::paths::resolve_state_scoped_table_path(
                        self.symbols,
                        SymbolHandle::invalid(),
                        SymbolHandle::invalid(),
                        self.expression_table,
                        &path,
                        None,
                    );
                    let members = super::paths::resolve_state_scoped_table_path_member_symbols(
                        self.symbols,
                        SymbolHandle::invalid(),
                        SymbolHandle::invalid(),
                        self.expression_table,
                        &path,
                    );
                    (head, symbol, members)
                };
                if !symbol.is_valid() {
                    let outcome = super::paths::unresolved_scoped_name(
                        self.symbols,
                        SymbolHandle::invalid(),
                        SymbolHandle::invalid(),
                        self.expression_table.name_path_members(path.members),
                    );
                    self.expression_table
                        .set_unresolved_name(expression, outcome);
                    return;
                }
                for (offset, selected) in (0u32..).zip(member_symbols) {
                    self.expression_table.set_name_path_member_symbol_at_offset(
                        path.member_symbols,
                        offset,
                        selected,
                    );
                }
                if let ExpressionNode::Name(path) = self.expression_table.expression_mut(expression)
                {
                    path.head_symbol = head;
                    path.symbol = symbol;
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}

/*
    The domain an alias or other non-membership spelling names. This is the
    membership pool: module-local owners outrank other tiers, and equal
    display paths are one candidate. A signature-free unique hit does not
    get its own answer.
*/
fn resolve_domain_symbol(
    symbols: &SymbolTable,
    name: &str,
    reference: source_text::SourceSpan,
) -> SymbolHandle {
    match super::lookup::membership_domain_lookup(symbols, name, reference) {
        super::lookup::DomainMembership::Unique(symbol) => symbol,
        super::lookup::DomainMembership::NotFound
        | super::lookup::DomainMembership::Ambiguous { .. }
        | super::lookup::DomainMembership::Contested => SymbolHandle::invalid(),
    }
}

/*
    A proof-fact membership uses that same domain, then still rejects a
    competing case. Binding the domain and erasing the case would accept a
    spelling expression membership refuses.
*/
fn proof_fact_membership_domain(
    symbols: &SymbolTable,
    name: &str,
    reference: source_text::SourceSpan,
) -> SymbolHandle {
    let domain = resolve_domain_symbol(symbols, name, reference);
    if !domain.is_valid() {
        return SymbolHandle::invalid();
    }
    match membership_selection(symbols, name, reference) {
        Ok(Some(MembershipSelection::Domain(symbol))) => symbol,
        Ok(Some(MembershipSelection::Case { .. })) | Ok(None) | Err(_) => SymbolHandle::invalid(),
    }
}
