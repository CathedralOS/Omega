//! Data definitions: members, type parameters, and generic-instance custody.
//!
//! `lower_type_parameters` is shared by every parameterized declaration.
//! Generic instances retain the derived const-argument origins of their
//! application while their members lower. Case facts on generic data pass a
//! narrow support gate before they are retained.

use crate::lowering::type_reference::{lower_child_type_references, lower_type_reference_handle};
use crate::resolution::lowerer::Lowerer;
use arena::HandleSpan;
use diagnostics::Diagnostic;
use std::collections::HashSet;
use symbol_resolved_trees::data::{
    DataDefinition, DataDefinitionStorage, DataField, DataMember, DataProperties, DataVariant,
    QuotientDefinition, TypeParameter, TypeParameterKind,
};
use symbols::SymbolHandle;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let prior_origins = lowerer.derived_const_argument_origins.len();
    let prior_expressions = lowerer.derived_const_argument_expressions.len();
    let prior_operators = lowerer.derived_const_argument_builtin_operators.len();
    if let Some(application) = data_definition.generic_instance {
        retain_derived_const_argument_origins(lowerer, syntax_trees, application);
    }
    let result =
        lower_data_definition_with_argument_origins(lowerer, syntax_trees, data_definition);
    lowerer
        .derived_const_argument_origins
        .truncate(prior_origins);
    lowerer
        .derived_const_argument_builtin_operators
        .truncate(prior_operators);
    lowerer
        .derived_const_argument_expressions
        .truncate(prior_expressions);
    result
}

/// A substituted argument does not become a new authored occurrence in the
/// template's public fields. Exclude only exact occurrences carried by this
/// instance's arguments; independent template references still retain exposure.
fn retain_derived_const_argument_origins(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    application: syntax::types::TypeReferenceHandle,
) {
    use syntax::types::TypeReferenceNode;
    let mut pending = vec![application];
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        let table = &syntax_trees.type_references;
        if let Some(normalization) = table.const_argument_normalization(handle) {
            if normalization.authored_expression.is_valid() {
                lowerer
                    .derived_const_argument_expressions
                    .push(normalization.authored_expression);
            }
            lowerer
                .derived_const_argument_origins
                .extend_from_slice(table.const_argument_origins(normalization.selections));
            lowerer
                .derived_const_argument_builtin_operators
                .extend_from_slice(
                    table.const_argument_builtin_operators(normalization.builtin_operators),
                );
        }
        let authored = table.generic_application_origin(handle);
        if authored.is_valid() {
            pending.push(authored);
        }
        match table.type_reference(handle) {
            TypeReferenceNode::Generic { arguments, .. } => {
                pending.extend_from_slice(table.type_reference_handles(*arguments));
            }
            TypeReferenceNode::Reference { referee, .. } => pending.push(*referee),
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                pending.push(*base_type);
                for constraint in table.constraints(*constraints) {
                    if let syntax::types::TypeConstraintNode::Domain(domain) = constraint {
                        pending.extend_from_slice(table.type_reference_handles(domain.arguments));
                    }
                }
            }
            TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type } => pending.push(*element_type),
            _ => {}
        }
    }
}

fn lower_data_definition_with_argument_origins(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, data_definition.type_parameters)?;
    let case_fact_gate = GenericCaseFactGate::new(syntax_trees, data_definition.type_parameters);
    let members = lower_data_members(
        lowerer,
        syntax_trees,
        data_definition.members,
        &case_fact_gate,
    )?;
    let retired_identities = syntax_trees
        .items
        .data_members(data_definition.members)
        .iter()
        .filter_map(|member| match member {
            syntax::item::DataMember::Retired(identity) => Some(*identity),
            _ => None,
        })
        .collect();
    let quotient = data_definition
        .quotient
        .as_ref()
        .map(|quotient| {
            Ok::<_, Diagnostic>(QuotientDefinition {
                carrier: lower_type_reference_handle(lowerer, syntax_trees, quotient.carrier)?,
                relation: syntax_trees
                    .items
                    .identifier_path_members(quotient.relation)
                    .iter()
                    .map(crate::lowering::name::lower_name)
                    .collect(),
                relation_symbol: SymbolHandle::invalid(),
                equivalence: quotient
                    .equivalence
                    .as_ref()
                    .map(|selection| {
                        Ok::<_, Diagnostic>(
                            symbol_resolved_trees::data::QuotientEquivalenceSelection {
                                relation: syntax_trees
                                    .items
                                    .identifier_path_members(selection.relation)
                                    .iter()
                                    .map(crate::lowering::name::lower_name)
                                    .collect(),
                                relation_symbol: SymbolHandle::invalid(),
                                trait_name: crate::lowering::name::lower_name(
                                    &selection.trait_name,
                                ),
                                trait_symbol: SymbolHandle::invalid(),
                                trait_arguments: lower_child_type_references(
                                    lowerer,
                                    syntax_trees,
                                    selection.trait_arguments,
                                )?,
                                conformance_name: crate::lowering::name::lower_name(
                                    &selection.conformance_name,
                                ),
                                conformance_symbol: SymbolHandle::invalid(),
                            },
                        )
                    })
                    .transpose()?,
            })
        })
        .transpose()?;
    // R2 rung 2 slice 1 (ch12 gating): lower the default-domain facts and
    // classify AT ZERO. Zero-satisfying facts are admitted -- the value is
    // born established (the zero-constructible tier); they stay INERT until
    // rung 3 wires entailment hypotheses and write obligations ATOMICALLY.
    // A GATED type (zero violates the domain) refuses until rung 2b lands
    // construction-mandatory fields; a fact the folder cannot evaluate at
    // zero refuses as unsupported (v1 fence). Never a silent drop.
    let type_equations = crate::preparation::generic_data::template_type_equation_offsets(
        syntax_trees,
        data_definition,
    )?;
    let where_facts = crate::lowering::domain::lower_proof_facts_excluding(
        lowerer,
        syntax_trees,
        data_definition.where_facts,
        &type_equations,
    )?;
    let mut zero_gated = false;
    for fact in lowerer.symbol_resolved_trees.proof_facts(where_facts) {
        match fact {
            symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                match zero_fold(
                    &lowerer.symbol_resolved_trees.tables.bodies.expressions,
                    *expression,
                ) {
                    Some(value) if value != 0 => {}
                    // R2 rung 2b: zero violates the domain -- the type is GATED
                    // (admitted; its literals must PROVE the domain, and rung 3's
                    // access gate covers zeroed storage).
                    Some(_) => zero_gated = true,
                    // A generic template has no concrete zero value yet.
                    // Retain its obligation conservatively; normalization
                    // discharges closed const facts on each instance, whose
                    // remaining default-domain facts still pass this check.
                    None if !type_parameters.is_empty() => zero_gated = true,
                    None => {
                        return Err(Diagnostic::error(format!(
                            "data `{}`: a default-domain `where` fact is outside the v1 \
                             zero-foldable fragment (field names, integer literals, + - *, \
                             comparisons, && ) -- simplify the fact for now (R2)",
                            data_definition.name.as_str()
                        )));
                    }
                }
            }
            // Membership symbols and domain facts are assigned after every
            // top-level declaration exists. Start conservative; the symbol
            // pass below clears the gate only when it can prove that the
            // referenced domain admits the carrier's zero value.
            symbol_resolved_trees::domain::ProofFact::Membership(_) => {
                zero_gated = true;
            }
        }
    }

    Ok(DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::lowering::name::lower_name(&data_definition.name),
        is_public: data_definition.is_public,
        storage: DataDefinitionStorage {
            supply_mode: data_definition.supply_mode,
            lifetime_parameters: data_definition
                .lifetime_parameters
                .iter()
                .map(crate::lowering::name::lower_name)
                .collect(),
            type_parameters,
            generic_instance: data_definition
                .generic_instance
                .map(|origin| {
                    crate::lowering::type_reference::lower_generic_application_origin(
                        lowerer,
                        syntax_trees,
                        origin,
                    )
                })
                .transpose()?,
            properties: DataProperties {
                carry: data_definition.properties.carry,
                multiplicity: data_definition.properties.multiplicity,
            },
            quotient,
            where_facts,
            zero_gated,
            retired_identities,
            members,
        },
    })
}

/// R2 rung 2 slice 1: fold one default-domain fact AT THE ZERO VALUE --
/// every field name reads 0, literals read themselves, `+ - *` fold,
/// comparisons and `&&`/`||` yield 1/0. `None` = outside the fragment.
pub(crate) fn zero_fold(
    expressions: &symbol_resolved_trees::expression::ExpressionTable,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<i128> {
    use symbol_resolved_trees::expression::{BinaryOperator, ExpressionNode};
    match expressions.expression(expression) {
        ExpressionNode::Name(_) => Some(0),
        ExpressionNode::Member(member) if matches!(member.member.as_str(), "len" | "capacity") => {
            // The ZII value of every builtin sequence carrier is empty; both
            // standing measures are therefore exactly zero.
            Some(0)
        }
        ExpressionNode::Integer(literal) => literal.text().parse::<i128>().ok(),
        ExpressionNode::Binary(binary) => {
            let left = zero_fold(expressions, binary.left)?;
            let right = zero_fold(expressions, binary.right)?;
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

pub(crate) fn lower_type_parameters(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    type_parameters: HandleSpan<syntax::item::TypeParameter>,
) -> Result<HandleSpan<TypeParameter>, Diagnostic> {
    let mut lowered = Vec::new();
    for parameter in syntax_trees.items.type_parameters(type_parameters) {
        let (kind, pending_service_reaches) = match &parameter.kind {
            syntax::item::TypeParameterKind::Type => (TypeParameterKind::Type, None),
            syntax::item::TypeParameterKind::Const { type_reference } => (
                TypeParameterKind::Const {
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        *type_reference,
                    )?,
                },
                None,
            ),
            syntax::item::TypeParameterKind::Value { type_reference } => (
                TypeParameterKind::Value {
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        *type_reference,
                    )?,
                },
                None,
            ),
            syntax::item::TypeParameterKind::Machine { contract } => {
                let contract = contract.as_ref().ok_or_else(|| {
                    Diagnostic::error(format!(
                        "machine parameter `{}` reached symbol resolution without its mandatory `where machine` contract",
                        parameter.name.as_str()
                    ))
                })?;
                match contract {
                    syntax::item::MachineParameterContract::RequirementIdentity => (
                        TypeParameterKind::Machine {
                            contract: symbol_resolved_trees::data::MachineParameterContract::RequirementIdentity,
                        },
                        None,
                    ),
                    syntax::item::MachineParameterContract::Structural(contract) => {
                        let lowered_contract = crate::lowering::state::lower_state_signature_parts(lowerer, syntax_trees, crate::lowering::state::StateSignatureParts { name: &contract.name,
spelling: contract.spelling,
lifetime_parameters: &contract.lifetime_parameters,
type_parameters: contract.type_parameters,
parameters: contract.parameters,
native_callback_parameters: &contract.native_callback_parameters,
return_type_handle: contract.return_type,
is_default: contract.is_default,
service_reach_is_installation_bound: false,
service_reach_keyword_source_spans: &contract.service_reach_keyword_source_spans,
service_reaches: contract.service_reaches,
invokes: contract.invokes,
suspends_keyword_source_spans: &contract.suspends_keyword_source_spans,
blocks_keyword_source_spans: &contract.blocks_keyword_source_spans,
suspends: contract.suspends,
blocks: contract.blocks,
contracts: contract.contracts,
terminates_guarantee: contract.terminates_guarantee,
where_facts: contract.where_facts })?;
                        (
                            TypeParameterKind::Machine {
                                contract: symbol_resolved_trees::data::MachineParameterContract::Structural(
                                    lowered_contract.signature,
                                ),
                            },
                            Some((
                                lowered_contract.service_reach_keyword_source_spans,
                                lowered_contract.service_reaches,
                            )),
                        )
                    }
                    syntax::item::MachineParameterContract::Nominal { requirement } => (
                        TypeParameterKind::Machine {
                            contract: symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                                requirement: syntax_trees
                                    .items
                                    .identifier_path_members(*requirement)
                                    .iter()
                                    .map(crate::lowering::name::lower_name)
                                    .collect(),
                            },
                        },
                        None,
                    ),
                }
            }
            syntax::item::TypeParameterKind::Proposition { contract } => {
                let contract = contract.as_ref().ok_or_else(|| {
                    Diagnostic::error(format!(
                        "proposition parameter `{}` reached symbol resolution without its mandatory authored signature",
                        parameter.name.as_str()
                    ))
                })?;
                (
                    TypeParameterKind::Proposition {
                        contract: symbol_resolved_trees::data::PropositionParameterSignature {
                            name: crate::lowering::name::lower_name(&contract.name),
                            parameters: crate::lowering::state::lower_state_parameters(
                                lowerer,
                                syntax_trees,
                                contract.parameters,
                            )?,
                        },
                    },
                    None,
                )
            }
        };
        lowered.push((
            TypeParameter {
                symbol: SymbolHandle::invalid(),
                name: crate::lowering::name::lower_name(&parameter.name),
                kind,
                bounds: DataProperties {
                    carry: parameter.bounds.carry,
                    multiplicity: parameter.bounds.multiplicity,
                },
            },
            pending_service_reaches,
        ));
    }

    let (parameters, pending_service_reaches): (Vec<_>, Vec<_>) = lowered.into_iter().unzip();
    let span = lowerer
        .symbol_resolved_trees
        .tables
        .declarations
        .data_type_parameters
        .insert_many(parameters);
    if !span.is_empty() {
        for (index, authored) in pending_service_reaches.into_iter().enumerate() {
            let Some((keyword_source_spans, authored)) = authored else {
                continue;
            };
            let arena_index = span
                .start()
                .arena_index()
                .checked_add(u32::try_from(index).expect("type-parameter span fits u32"))
                .expect("type-parameter arena index overflow");
            let handle = arena::Handle::from_parts(arena_index, span.start().generation());
            let owner = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .data_type_parameters
                .get(handle)
                .name
                .clone();
            lowerer.pending_signature_service_reaches.push(
                crate::resolution::lowerer::PendingSignatureServiceReach {
                    location:
                        crate::resolution::lowerer::PendingSignatureLocation::MachineParameter(
                            handle,
                        ),
                    owner: crate::resolution::lowerer::PendingSignatureOwner::Requirement(owner),
                    keyword_source_spans,
                    authored,
                },
            );
        }
    }

    Ok(span)
}

fn lower_data_members(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    members: HandleSpan<syntax::item::DataMember>,
    case_fact_gate: &GenericCaseFactGate,
) -> Result<HandleSpan<DataMember>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.data_members(members) {
        if matches!(member, syntax::item::DataMember::Retired(_)) {
            continue;
        }
        let member = lower_data_member(lowerer, syntax_trees, member, case_fact_gate)?;
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .data_members
            .append_to_span(&mut span, member);
    }

    Ok(span)
}

fn lower_data_member(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    member: &syntax::item::DataMember,
    case_fact_gate: &GenericCaseFactGate,
) -> Result<DataMember, Diagnostic> {
    match member {
        syntax::item::DataMember::Field(field) => Ok(DataMember::Field(DataField {
            identity: field.identity,
            symbol: SymbolHandle::invalid(),
            name: crate::lowering::name::lower_name(&field.name),
            relevance: field.relevance,
            type_reference: lower_type_reference_handle(
                lowerer,
                syntax_trees,
                field.type_reference,
            )?,
        })),
        syntax::item::DataMember::Variant(variant) => {
            let mut payload = HandleSpan::empty();
            for field in syntax_trees.items.data_payload_fields(variant.payload) {
                let lowered = DataField {
                    identity: field.identity,
                    symbol: SymbolHandle::invalid(),
                    name: crate::lowering::name::lower_name(&field.name),
                    relevance: field.relevance,
                    type_reference: lower_type_reference_handle(
                        lowerer,
                        syntax_trees,
                        field.type_reference,
                    )?,
                };
                lowerer
                    .symbol_resolved_trees
                    .tables
                    .declarations
                    .data_payload_fields
                    .append_to_span(&mut payload, lowered);
            }
            // CASE-CONSTRAINTS generic case-data synthesis: a generic case's
            // `where` facts deep-copy onto each synthesized instance
            // (generic_data/substitution.rs). The copy is faithful for facts
            // over payload fields, literals, and top-level bindings, and a
            // `const` binder mention is rewritten to its literal argument in
            // expression position. A `type` binder survives only inside a
            // decidable `T == name` / `T != name` conjunct, which synthesis
            // decides against the closed argument identity; any other binder
            // with no fact-position substitution (value, machine,
            // proposition) or a `const` binder inside a membership
            // value/domain path still refuses rather than drop the fact or
            // dangle a name on the instance.
            if generic_case_facts_unsupported(syntax_trees, variant.where_facts, case_fact_gate) {
                return Err(Diagnostic::error(
                    "case constraints on generic data may not mention generic parameters yet",
                ));
            }
            let where_facts = crate::lowering::domain::lower_proof_facts(
                lowerer,
                syntax_trees,
                variant.where_facts,
            )?;
            Ok(DataMember::Variant(DataVariant {
                identity: variant.identity,
                symbol: SymbolHandle::invalid(),
                name: crate::lowering::name::lower_name(&variant.name),
                payload,
                where_facts,
                retired_payload_identities: variant.retired_payload_identities.clone(),
            }))
        }
        syntax::item::DataMember::Retired(_) => unreachable!("retired identities are metadata"),
    }
}

/// Which template parameter names a case `where` fact may still not mention.
/// Generic-instance synthesis deep-copies each case fact onto the instance
/// (`generic_data/substitution.rs`); a `const` binder mention survives as its
/// literal argument only in expression position, while a type, value, machine,
/// or proposition binder has no fact-position substitution at all.
#[derive(Default)]
struct GenericCaseFactGate {
    /// Binder names with no fact-position substitution. A mention anywhere in
    /// a case fact refuses the case -- except a `type` binder standing in a
    /// decided type-equality conjunct (see `case_fact_type_equality`).
    unsubstituted: HashSet<String>,
    /// `type` binder names: a subset of `unsubstituted` that may appear on one
    /// side of a top-level `==`/`!=` conjunct, where synthesis decides the
    /// equation against the closed argument identity.
    types: HashSet<String>,
    /// `const` binder names: rewritten to their literal argument inside
    /// expression position only, so a mention in a membership value or domain
    /// path, or in a nested type reference's name position, still refuses.
    consts: HashSet<String>,
}

impl GenericCaseFactGate {
    fn new(
        syntax_trees: &SyntaxTrees,
        parameters: HandleSpan<syntax::item::TypeParameter>,
    ) -> Self {
        let mut gate = Self::default();
        for parameter in syntax_trees.items.type_parameters(parameters) {
            let name = parameter.name.as_str().to_owned();
            match parameter.kind {
                syntax::item::TypeParameterKind::Const { .. } => {
                    gate.consts.insert(name);
                }
                syntax::item::TypeParameterKind::Type => {
                    gate.types.insert(name.clone());
                    gate.unsubstituted.insert(name);
                }
                _ => {
                    gate.unsubstituted.insert(name);
                }
            }
        }
        gate
    }

    /// A name position the literal rewrite cannot reach: every binder refuses.
    fn mentions(&self, name: &str) -> bool {
        self.unsubstituted.contains(name) || self.consts.contains(name)
    }

    /// An expression `Name` leaf: only binders without a substitution refuse;
    /// `const` binders arrive on the instance as their literal argument.
    fn expression_mentions(&self, name: &str, const_mentions_allowed: bool) -> bool {
        self.unsubstituted.contains(name) || (!const_mentions_allowed && self.consts.contains(name))
    }
}

/// Whether any of a case's `where` facts names a parameter the synthesis copy
/// cannot carry honestly. Monomorphic definitions build an empty gate, so this
/// always admits them.
fn generic_case_facts_unsupported(
    syntax_trees: &SyntaxTrees,
    facts: HandleSpan<syntax::item::ProofFact>,
    gate: &GenericCaseFactGate,
) -> bool {
    if gate.unsubstituted.is_empty() && gate.consts.is_empty() {
        return false;
    }
    syntax_trees
        .items
        .proof_facts(facts)
        .iter()
        .any(|fact| match fact {
            syntax::item::ProofFact::Expression(expression) => {
                // Each top-level `and` conjunct stands alone: a decided
                // type-parameter equality (`T == i32`) is carried by its
                // instance as a decided literal witness, so it admits a `type`
                // binder where a value-position mention would dangle.
                let mut conjuncts = Vec::new();
                flatten_case_fact_conjuncts(syntax_trees, *expression, &mut conjuncts);
                conjuncts.iter().any(|conjunct| {
                    !case_fact_type_equality(syntax_trees, *conjunct, gate)
                        && case_fact_expression_mentions(syntax_trees, *conjunct, gate, true)
                })
            }
            // The membership value must stay a place/name the
            // construction-side domain check can own, and the domain path is
            // a fixed declaration spelling: no binder may appear in either.
            syntax::item::ProofFact::Membership(membership) => {
                case_fact_expression_mentions(syntax_trees, membership.value, gate, false)
                    || syntax_trees
                        .items
                        .identifier_path_members(membership.domain)
                        .iter()
                        .any(|member| gate.mentions(member.as_str()))
                    || syntax_trees
                        .type_references
                        .type_reference_handles(membership.domain_arguments)
                        .iter()
                        .any(|argument| membership_argument_mentions(syntax_trees, *argument, gate))
            }
        })
}

/// An indexed application's argument is a type-position leaf: a named binder
/// refuses like the domain path, and an open const expression refuses like the
/// membership value.
fn membership_argument_mentions(
    syntax_trees: &SyntaxTrees,
    argument: syntax::types::TypeReferenceHandle,
    gate: &GenericCaseFactGate,
) -> bool {
    match syntax_trees.type_references.type_reference(argument) {
        syntax::types::TypeReferenceNode::Named(name) => gate.mentions(name.as_str()),
        syntax::types::TypeReferenceNode::ConstExpression(expression) => {
            case_fact_expression_mentions(syntax_trees, *expression, gate, false)
        }
        _ => false,
    }
}

/// Split a case-fact expression into its top-level `and` conjuncts.
fn flatten_case_fact_conjuncts(
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
    conjuncts: &mut Vec<syntax::expression::ExpressionHandle>,
) {
    use syntax::expression::ExpressionNode;
    if let ExpressionNode::Binary(binary) = syntax_trees.expressions.expression(expression)
        && binary.operator == syntax::expression::BinaryOperator::And
    {
        flatten_case_fact_conjuncts(syntax_trees, binary.left, conjuncts);
        flatten_case_fact_conjuncts(syntax_trees, binary.right, conjuncts);
        return;
    }
    conjuncts.push(expression);
}

/// Whether a case-fact conjunct is a decidable type-parameter equality:
/// `T == name`, `name == T`, or the `!=` form, where `T` is a `type` binder
/// and the other side is a single-segment name that mentions no binder at
/// all. Generic-instance synthesis decides the equation against the closed
/// argument identity (`generic_data/substitution.rs`), so the parameter name
/// never reaches an instance. A binder on BOTH sides (`T == U`), a nested or
/// negated equality, or a non-name opposite side cannot be decided honestly
/// and stays refused.
fn case_fact_type_equality(
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
    gate: &GenericCaseFactGate,
) -> bool {
    use syntax::expression::ExpressionNode;
    let ExpressionNode::Binary(binary) = syntax_trees.expressions.expression(expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        syntax::expression::BinaryOperator::Equal | syntax::expression::BinaryOperator::NotEqual
    ) {
        return false;
    }
    let binder_side = |side| -> Option<bool> {
        let ExpressionNode::Name(path) = syntax_trees.expressions.expression(side) else {
            return None;
        };
        let members = syntax_trees.expressions.identifier_path_members(*path);
        let [member] = members else {
            return None;
        };
        if gate.types.contains(member.as_str()) {
            Some(true)
        } else if !gate.mentions(member.as_str()) {
            Some(false)
        } else {
            None
        }
    };
    matches!(
        (binder_side(binary.left), binder_side(binary.right)),
        (Some(true), Some(false)) | (Some(false), Some(true))
    )
}

fn case_fact_expression_mentions(
    syntax_trees: &SyntaxTrees,
    expression: syntax::expression::ExpressionHandle,
    gate: &GenericCaseFactGate,
    const_mentions_allowed: bool,
) -> bool {
    use syntax::expression::ExpressionNode;
    match syntax_trees.expressions.expression(expression) {
        ExpressionNode::Name(path) => syntax_trees
            .expressions
            .identifier_path_members(*path)
            .iter()
            .any(|member| gate.expression_mentions(member.as_str(), const_mentions_allowed)),
        ExpressionNode::Binary(binary) => {
            case_fact_expression_mentions(syntax_trees, binary.left, gate, const_mentions_allowed)
                || case_fact_expression_mentions(
                    syntax_trees,
                    binary.right,
                    gate,
                    const_mentions_allowed,
                )
        }
        ExpressionNode::Unary(unary) => {
            case_fact_expression_mentions(syntax_trees, unary.operand, gate, const_mentions_allowed)
        }
        ExpressionNode::Member(member) => case_fact_expression_mentions(
            syntax_trees,
            member.receiver,
            gate,
            const_mentions_allowed,
        ),
        ExpressionNode::Borrow(borrow) => {
            case_fact_expression_mentions(syntax_trees, borrow.target, gate, const_mentions_allowed)
        }
        ExpressionNode::Indexed(indexed) => {
            case_fact_expression_mentions(
                syntax_trees,
                indexed.collection,
                gate,
                const_mentions_allowed,
            ) || case_fact_expression_mentions(
                syntax_trees,
                indexed.index,
                gate,
                const_mentions_allowed,
            )
        }
        ExpressionNode::Range(range) => {
            case_fact_expression_mentions(syntax_trees, range.start, gate, const_mentions_allowed)
                || case_fact_expression_mentions(
                    syntax_trees,
                    range.end,
                    gate,
                    const_mentions_allowed,
                )
        }
        ExpressionNode::ArrayLiteral(elements) => syntax_trees
            .expressions
            .expression_handles(*elements)
            .iter()
            .any(|element| {
                case_fact_expression_mentions(syntax_trees, *element, gate, const_mentions_allowed)
            }),
        ExpressionNode::Membership(membership) => {
            case_fact_expression_mentions(syntax_trees, membership.value, gate, false)
                || syntax_trees
                    .expressions
                    .identifier_path_members(membership.domain)
                    .iter()
                    .any(|member| gate.mentions(member.as_str()))
        }
        ExpressionNode::Cast(cast) => {
            case_fact_expression_mentions(syntax_trees, cast.value, gate, const_mentions_allowed)
                || case_fact_type_mentions(syntax_trees, cast.target_type, gate)
                || syntax_trees
                    .expressions
                    .identifier_path_members(cast.semantic_domain)
                    .iter()
                    .any(|member| gate.mentions(member.as_str()))
                || syntax_trees
                    .type_references
                    .type_reference_handles(cast.semantic_domain_arguments)
                    .iter()
                    .any(|argument| case_fact_type_mentions(syntax_trees, *argument, gate))
        }
        ExpressionNode::ZeroValue(type_reference)
        | ExpressionNode::TypeExpression(type_reference) => {
            case_fact_type_mentions(syntax_trees, *type_reference, gate)
        }
        ExpressionNode::Integer(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::String(_)
        | ExpressionNode::SelfValue => false,
        // Proposition applications, matches, struct literals, and atomics have
        // no generic-instance case-fact carriage yet: refuse rather than copy
        // a shape the seeded replay does not compare.
        ExpressionNode::Call(_)
        | ExpressionNode::Match(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::Atomic(_) => true,
    }
}

/// A type reference nested inside a case fact (`x as T`, `zero<T>()`). Name
/// positions are not rewritten by the `const` literal pass, so every binder
/// mention refuses; embedded expression positions (range bounds, const
/// arguments) do participate in the rewrite.
fn case_fact_type_mentions(
    syntax_trees: &SyntaxTrees,
    reference: syntax::types::TypeReferenceHandle,
    gate: &GenericCaseFactGate,
) -> bool {
    use syntax::types::{FixedArrayLength, TypeConstraintNode, TypeReferenceNode};
    match syntax_trees.type_references.type_reference(reference) {
        TypeReferenceNode::Named(name) => gate.mentions(name.as_str()),
        TypeReferenceNode::Reference { referee, .. } => {
            case_fact_type_mentions(syntax_trees, *referee, gate)
        }
        TypeReferenceNode::Slice { element_type } => {
            case_fact_type_mentions(syntax_trees, *element_type, gate)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            case_fact_type_mentions(syntax_trees, *element_type, gate)
                || match length {
                    FixedArrayLength::Literal(_) => false,
                    FixedArrayLength::ConstParameter(name) | FixedArrayLength::ConstCall(name) => {
                        gate.mentions(name.as_str())
                    }
                }
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            gate.mentions(base_name.as_str())
                || syntax_trees
                    .type_references
                    .type_reference_handles(*arguments)
                    .iter()
                    .any(|argument| case_fact_type_mentions(syntax_trees, *argument, gate))
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            case_fact_type_mentions(syntax_trees, *base_type, gate)
                || syntax_trees
                    .type_references
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| match constraint {
                        TypeConstraintNode::Named(name) => gate.mentions(name.as_str()),
                        TypeConstraintNode::Range {
                            minimum, maximum, ..
                        } => {
                            case_fact_expression_mentions(syntax_trees, *minimum, gate, true)
                                || case_fact_expression_mentions(syntax_trees, *maximum, gate, true)
                        }
                        TypeConstraintNode::ArithmeticDomain(_) => false,
                        TypeConstraintNode::Domain(domain) => {
                            gate.mentions(domain.name.as_str())
                                || syntax_trees
                                    .type_references
                                    .type_reference_handles(domain.arguments)
                                    .iter()
                                    .any(|argument| {
                                        case_fact_type_mentions(syntax_trees, *argument, gate)
                                    })
                        }
                    })
        }
        TypeReferenceNode::ConstExpression(expression) => {
            case_fact_expression_mentions(syntax_trees, *expression, gate, true)
        }
        TypeReferenceNode::DynamicTrait { name, conformance } => {
            gate.mentions(name.as_str())
                || conformance
                    .as_ref()
                    .is_some_and(|conformance| gate.mentions(conformance.as_str()))
        }
        TypeReferenceNode::SelfType | TypeReferenceNode::Unit => false,
    }
}
