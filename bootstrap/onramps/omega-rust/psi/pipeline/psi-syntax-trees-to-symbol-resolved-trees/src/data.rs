use crate::lowerer::Lowerer;
use crate::type_reference::{lower_child_type_references, lower_type_reference_handle};
use psi_arena::HandleSpan;
use psi_diagnostics::Diagnostic;
use psi_symbol_resolved_trees::data::{
    DataDefinition, DataDefinitionStorage, DataField, DataMember, DataProperties, DataVariant,
    QuotientDefinition, TypeParameter, TypeParameterKind,
};
use psi_symbols::SymbolHandle;
use psi_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_data_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    data_definition: &syntax::item::DataDefinition,
) -> Result<DataDefinition, Diagnostic> {
    let type_parameters =
        lower_type_parameters(lowerer, syntax_trees, data_definition.type_parameters)?;
    let members = lower_data_members(lowerer, syntax_trees, data_definition.members)?;
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
                    .map(crate::name::lower_name)
                    .collect(),
                relation_symbol: SymbolHandle::invalid(),
                equivalence: quotient
                    .equivalence
                    .as_ref()
                    .map(|selection| {
                        Ok::<_, Diagnostic>(
                            psi_symbol_resolved_trees::data::QuotientEquivalenceSelection {
                                relation: syntax_trees
                                    .items
                                    .identifier_path_members(selection.relation)
                                    .iter()
                                    .map(crate::name::lower_name)
                                    .collect(),
                                relation_symbol: SymbolHandle::invalid(),
                                trait_name: crate::name::lower_name(&selection.trait_name),
                                trait_symbol: SymbolHandle::invalid(),
                                trait_arguments: lower_child_type_references(
                                    lowerer,
                                    syntax_trees,
                                    selection.trait_arguments,
                                )?,
                                conformance_name: crate::name::lower_name(
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
    let where_facts =
        crate::domain::lower_proof_facts(lowerer, syntax_trees, data_definition.where_facts)?;
    let mut zero_gated = false;
    for fact in lowerer.symbol_resolved_trees.proof_facts(where_facts) {
        match fact {
            psi_symbol_resolved_trees::domain::ProofFact::Expression(expression) => {
                match zero_fold(
                    &lowerer.symbol_resolved_trees.tables.bodies.expressions,
                    *expression,
                ) {
                    Some(value) if value != 0 => {}
                    // R2 rung 2b: zero violates the domain -- the type is GATED
                    // (admitted; its literals must PROVE the domain, and rung 3's
                    // access gate covers zeroed storage).
                    Some(_) => zero_gated = true,
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
            psi_symbol_resolved_trees::domain::ProofFact::Membership(_) => {
                zero_gated = true;
            }
        }
    }

    Ok(DataDefinition {
        symbol: SymbolHandle::invalid(),
        name: crate::name::lower_name(&data_definition.name),
        is_public: data_definition.is_public,
        storage: DataDefinitionStorage {
            supply_mode: data_definition.supply_mode,
            lifetime_parameters: data_definition
                .lifetime_parameters
                .iter()
                .map(crate::name::lower_name)
                .collect(),
            type_parameters,
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
    expressions: &psi_symbol_resolved_trees::expression::ExpressionTable,
    expression: psi_symbol_resolved_trees::expression::ExpressionHandle,
) -> Option<i128> {
    use psi_symbol_resolved_trees::expression::{BinaryOperator, ExpressionNode};
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
            syntax::item::TypeParameterKind::Machine { contract } => {
                let contract = contract.as_ref().ok_or_else(|| {
                    Diagnostic::error(format!(
                        "machine parameter `{}` reached symbol resolution without its mandatory `where machine` contract",
                        parameter.name.as_str()
                    ))
                })?;
                match contract {
                    syntax::item::MachineParameterContract::Structural(contract) => {
                        let lowered_contract = crate::state::lower_state_signature_parts(
                            lowerer,
                            syntax_trees,
                            &contract.name,
                            contract.spelling,
                            &contract.lifetime_parameters,
                            contract.type_parameters,
                            contract.parameters,
                            contract.return_type,
                            contract.is_default,
                            false,
                            contract.service_reaches,
                            contract.invokes,
                            contract.suspends,
                            contract.blocks,
                            contract.contracts,
                            contract.terminates_guarantee,
                        )?;
                        (
                            TypeParameterKind::Machine {
                                contract: psi_symbol_resolved_trees::data::MachineParameterContract::Structural(
                                    lowered_contract.signature,
                                ),
                            },
                            Some(lowered_contract.service_reaches),
                        )
                    }
                    syntax::item::MachineParameterContract::Nominal { requirement } => (
                        TypeParameterKind::Machine {
                            contract: psi_symbol_resolved_trees::data::MachineParameterContract::AuthoredNominal {
                                requirement: syntax_trees
                                    .items
                                    .identifier_path_members(*requirement)
                                    .iter()
                                    .map(crate::name::lower_name)
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
                        contract: psi_symbol_resolved_trees::data::PropositionParameterSignature {
                            name: crate::name::lower_name(&contract.name),
                            parameters: crate::state::lower_state_parameters(
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
                name: crate::name::lower_name(&parameter.name),
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
            let Some(authored) = authored else {
                continue;
            };
            let arena_index = span
                .start()
                .arena_index()
                .checked_add(u32::try_from(index).expect("type-parameter span fits u32"))
                .expect("type-parameter arena index overflow");
            let handle = psi_arena::Handle::from_parts(arena_index, span.start().generation());
            let owner = lowerer
                .symbol_resolved_trees
                .tables
                .declarations
                .data_type_parameters
                .get(handle)
                .name
                .clone();
            lowerer.pending_signature_service_reaches.push(
                crate::lowerer::PendingSignatureServiceReach {
                    location: crate::lowerer::PendingSignatureLocation::MachineParameter(handle),
                    owner: crate::lowerer::PendingSignatureOwner::Requirement(owner),
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
) -> Result<HandleSpan<DataMember>, Diagnostic> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.data_members(members) {
        if matches!(member, syntax::item::DataMember::Retired(_)) {
            continue;
        }
        let member = lower_data_member(lowerer, syntax_trees, member)?;
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
) -> Result<DataMember, Diagnostic> {
    match member {
        syntax::item::DataMember::Field(field) => Ok(DataMember::Field(DataField {
            identity: field.identity,
            symbol: SymbolHandle::invalid(),
            name: crate::name::lower_name(&field.name),
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
                    name: crate::name::lower_name(&field.name),
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
            Ok(DataMember::Variant(DataVariant {
                identity: variant.identity,
                symbol: SymbolHandle::invalid(),
                name: crate::name::lower_name(&variant.name),
                payload,
                retired_payload_identities: variant.retired_payload_identities.clone(),
            }))
        }
        syntax::item::DataMember::Retired(_) => unreachable!("retired identities are metadata"),
    }
}
