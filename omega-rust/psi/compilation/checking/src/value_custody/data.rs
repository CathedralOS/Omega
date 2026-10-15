use crate::declarations::symbols::TopLevelSymbols;
use crate::proof_contracts::proof_facts::{ProofFactOwner, validate_proof_facts};
use crate::value_custody::type_references::{
    TypeReferenceOwner, type_reference_label, type_references_match,
    validate_generic_instance_argument_bounds, validate_type_reference_handle,
};
use diagnostics::Diagnostic;
use numerics::bignum::BigInt;
use typed_trees::data::DataMember;
use typed_trees::types::{
    DomainConstraintSubject, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

pub(crate) fn validate_data_field_types(
    program: &dyn crate::CheckingInput,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for data_definition in program.data_definitions() {
        // BUILTIN-NAME SHADOW FENCE (probed 2026-07-16): type lookup searches
        // BuiltinType before Data, so a definition named like a builtin type
        // (`data Int`) compiles while every reference binds to the BUILTIN --
        // the definition is silently orphaned, and contract lemmas typed
        // against the builtin stand down at the polynomial engine as
        // out-of-language, certifying UNVALIDATED (possibly false) claims.
        // Refuse the collision at the declaration.
        if symbols::builtin_type_symbols()
            .iter()
            .any(|(_, name)| name.as_str() == data_definition.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` shadows the builtin type of the same name -- references \
                 would silently bind to the builtin and this definition would never \
                 be used (its contracts would go unvalidated). Pick a different name.",
                data_definition.name
            )));
        }
        let data_members = program.data_members(data_definition);
        let type_parameters = program.data_type_parameters(data_definition);
        crate::value_custody::type_references::validate_const_binder_carriers(
            program,
            symbols,
            &format!("data `{}`", data_definition.name),
            type_parameters,
            crate::value_custody::type_references::TypeParameterScope {
                operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                lexical_owner: data_definition.symbol,
                type_parameters,
                lifetime_parameters: &data_definition.lifetime_parameters,
            },
            diagnostics,
        );
        validate_generic_instance_argument_bounds(program, data_definition, diagnostics);
        validate_data_member_names(data_definition, data_members, diagnostics);
        validate_data_default_domain(program, data_definition, data_members, diagnostics);

        for member in data_members {
            let payload_fields = match member {
                DataMember::Field(field) => std::slice::from_ref(field),
                DataMember::Variant(variant) => {
                    validate_payload_field_names(data_definition, variant, program, diagnostics);
                    program.data_payload_fields(variant)
                }
            };

            for field in payload_fields {
                validate_type_reference_handle(
                    program,
                    field.type_reference,
                    symbols,
                    diagnostics,
                    TypeReferenceOwner::DataField {
                        data: data_definition.name.as_str(),
                        field: field.name.as_str(),
                        generic_depth: 0,
                    },
                    crate::value_custody::type_references::TypeParameterScope {
                        operand_lifetime_owner: symbols::SymbolHandle::invalid(),
                        lexical_owner: data_definition.symbol,
                        type_parameters,
                        lifetime_parameters: &data_definition.lifetime_parameters,
                    },
                );
                validate_stored_field_has_a_representation(
                    program,
                    data_definition,
                    field,
                    diagnostics,
                );
            }
        }
    }
}

/// A relevant field must have a runtime representation. `UInt` and `Int` are
/// the unbounded builtin integers: they carry no width, so no layout exists
/// for them and shape construction declines the whole declaration.
///
/// Without this the decline is silent. It propagates outward -- the owning
/// data has no shape, so an attached machine has no signature, so its unit
/// plan is omitted -- and surfaces far away as "selected ProgramEntry
/// establishment rejoins 0 Terminal attachment identities ... omitted at local
/// construction at `state graph: state signature: parameter signature:
/// attached data shape`", which names neither the field nor its type. The
/// dungeon-crawler sample reached that message through `Main.game` ->
/// `Game.dungeon` -> `Dungeon.random` -> `RandomState.calls: UInt`.
///
/// An `[erased]` field is proof-only and keeps no representation, so it may
/// name either type.
fn validate_stored_field_has_a_representation(
    program: &dyn crate::CheckingInput,
    data_definition: &typed_trees::data::DataDefinition,
    field: &typed_trees::data::DataField,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if field.relevance.is_erased() {
        return;
    }
    let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(field.type_reference)
    else {
        return;
    };
    // The resolved symbol decides this, not the spelling: a name-keyed test
    // would answer for whatever a later declaration spelled `UInt`.
    let Some(builtin) = [symbols::BuiltinType::UInt, symbols::BuiltinType::Int]
        .into_iter()
        .find(|candidate| program.symbols.builtin_type_symbol(*candidate) == Some(*symbol))
    else {
        return;
    };
    diagnostics.push(Diagnostic::error(format!(
 "data `{}` field `{}` stores unbounded integer type `{}`, which has no runtime representation; use a sized integer such as `u64` or `i64`, or declare the field `[erased]` to keep it proof-only",
        data_definition.name.as_str(),
        field.name.as_str(),
        builtin.name(),
    )));
}

/// Whether zero-filled storage is already an established value of this type.
/// Representation stays zero-expressible; a range/default domain that excludes
/// zero gates VALUE establishment instead of making the declaration illegal.
/// The check composes through record fields, arrays, and the payload of the
/// zero-tag (first) sum case. Later cases do not inhabit zero storage.
pub fn type_requires_establishment(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    type_requires_establishment_inner(program, type_reference, &mut EstablishmentIndex::new())
}

/// One walk's working set: the active-path `seen` set and the absolute
/// answers already proven for completed definitions — a definition's result
/// does not depend on the path that reached it, so it is memoized. Named
/// symbol lookups read the build-scope data-definition index rather than a
/// per-walk declaration table.
///
/// Every key retains both the declaration and selected application. Derived
/// applications keep their template's symbol but substitute different fields;
/// those rows must not share an answer. Neither spelling nor the base symbol
/// alone identifies the storage being judged. A
/// `Named` node whose symbol never resolved keeps the spelling route, which
/// scans rather than indexing, because an unresolved reference has no
/// declaration identity to key on.
struct EstablishmentIndex {
    seen: Vec<EstablishmentIdentity>,
    computed: Vec<(EstablishmentIdentity, bool)>,
}

impl EstablishmentIndex {
    fn new() -> Self {
        Self {
            seen: Vec::new(),
            computed: Vec::new(),
        }
    }
}

/// The declaration and exact retained application, without source-name identity.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct EstablishmentIdentity {
    declaration: symbols::SymbolHandle,
    application: TypeReferenceHandle,
}

impl EstablishmentIdentity {
    pub(super) fn of(definition: &typed_trees::data::DataDefinition) -> Self {
        Self {
            declaration: definition.symbol,
            application: definition.generic_instance.unwrap_or_default(),
        }
    }

    pub(super) fn order(self) -> (u32, u32, u32, u32) {
        (
            self.declaration.arena_index(),
            self.declaration.generation(),
            self.application.arena_index(),
            self.application.generation(),
        )
    }
}

fn validate_data_default_domain(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let facts = program.proof_facts.span_or_empty(definition.where_facts);
    validate_proof_facts(
        program,
        facts,
        diagnostics,
        ProofFactOwner::DataDefaultDomain(definition.name.as_str()),
    );

    for fact in facts {
        let Some(membership) = fact.as_membership() else {
            continue;
        };
        let Some(domain) = program.domain_definition(membership.domain_symbol) else {
            continue;
        };
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(membership.value)
        else {
            continue;
        };
        let Some(field_name) = program
            .expression_table
            .name_path_members(path.members)
            .last()
        else {
            continue;
        };
        let Some(field_type) = members.iter().find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == field_name.as_str() => {
                Some(field.type_reference)
            }
            DataMember::Field(_) | DataMember::Variant(_) => None,
        }) else {
            continue;
        };
        let field_carrier =
            crate::value_custody::places::unwrapped_type_reference(program, field_type)
                .unwrap_or(field_type);
        let domain_carrier =
            crate::value_custody::places::unwrapped_type_reference(program, domain.target_type)
                .unwrap_or(domain.target_type);
        if type_references_match(program, field_carrier, domain_carrier) {
            continue;
        }
        diagnostics.push(Diagnostic::error(format!(
            "data `{}` default-domain fact puts field `{}` in domain `{}`, but the field carrier is `{}` while the domain classifies `{}`",
            definition.name.as_str(),
            field_name.as_str(),
            domain.name.as_str(),
            type_reference_label(program, field_carrier),
            type_reference_label(program, domain_carrier),
        )));
    }
}

fn type_requires_establishment_inner(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    index: &mut EstablishmentIndex,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    if let Some(interval) = crate::proof_contracts::arithmetic_domains::range_constraint_interval(
        program,
        type_reference,
    ) && (interval.low().is_some_and(|low| low > 0)
        || interval.high().is_some_and(|high| high < 0))
    {
        return true;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            // Representation zero is not declared-domain establishment. Only
            // an exact, route-free zero judgment can discharge a qualification;
            // a partial interval of its predicates cannot waive the remainder.
            program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .any(|constraint| {
                    constraint_requires_establishment(program, *base_type, constraint)
                })
                || type_requires_establishment_inner(program, *base_type, index)
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            // An empty array contains no element values. Propagating an
            // element's gate would fabricate unfinished construction and can
            // suppress a containing owner's otherwise required cleanup hook.
            !matches!(length, typed_trees::types::FixedArrayLength::Literal(0))
                && type_requires_establishment_inner(program, *element_type, index)
        }
        TypeReferenceNode::Named { symbol, name } => {
            // A stored open payload has no zero certificate. An unused binder
            // in a non-home sum case is never visited by the declaration walk.
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
            {
                return true;
            }
            let definition = program.closed_data_definition(type_reference).or_else(|| {
                if symbol.is_valid() {
                    crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                } else {
                    program.data_definition_by_name_str(name.as_str())
                }
            });
            definition.is_some_and(|definition| {
                data_requires_establishment_inner(program, definition, index)
            })
        }
        TypeReferenceNode::Generic { base_symbol, .. } => {
            // Prefer substituted fields: a selected child can exclude zero.
            // For an open declaration, inspect only common/home-case storage.
            // Unknown stored binders gate that walk; unused type arguments do
            // not invalidate an empty home case or a provisioned opaque carrier.
            program
                .closed_data_definition(type_reference)
                .or_else(|| {
                    crate::declarations::data_queries::data_definition_by_symbol(
                        program,
                        *base_symbol,
                    )
                })
                .is_none_or(|definition| {
                    data_requires_establishment_inner(program, definition, index)
                })
        }
        _ => false,
    }
}

fn constraint_requires_establishment(
    program: &dyn crate::CheckingInput,
    carrier: TypeReferenceHandle,
    constraint: &TypeConstraintNode,
) -> bool {
    match constraint {
        TypeConstraintNode::Domain(domain) => match domain.subject {
            DomainConstraintSubject::Declared => {
                // A route-/predicate-free declaration adds no membership
                // obligation. Alias constituents still pass the same shared
                // judgment; carrier validity is checked by the recursive walk.
                if crate::value_custody::field_domains::domain_is_vacuous(
                    program,
                    domain.symbol,
                    &mut Vec::new(),
                ) {
                    return false;
                }
                let zero_integer =
                    crate::proof_contracts::domains::integer_domain_membership(
                        program,
                        domain.symbol,
                        &domain.arguments,
                        domain.semantic_id,
                        &crate::proof_contracts::contract_entailment::ScopedArithmeticValue::Integer(BigInt::zero()),
                    ) == crate::proof_contracts::contract_entailment::StrictArithmeticImplicationJudgment::Proven;
                let zero_bytes = domain.establishment_routes.is_empty()
                    && crate::value_custody::field_domains::domain_admits_empty_byte_sequence(
                        program,
                        domain.symbol,
                    );
                !zero_integer && !zero_bytes
            }
            // These constrain the carrier, or exclude non-finite floats;
            // they do not require value establishment beyond the base type.
            DomainConstraintSubject::Carry(_)
            | DomainConstraintSubject::OmegaLayout { .. }
            | DomainConstraintSubject::Value(
                language_semantics::value_domain::ValueDomain::Finite,
            ) => false,
        },
        TypeConstraintNode::Named(_) => true,
        TypeConstraintNode::Range {
            minimum,
            maximum,
            end_inclusive,
        } => {
            // IEEE endpoints keep their declared format and boundary kind.
            // Integer predecessor arithmetic cannot decide a fractional bound
            // or whether +0 lies beneath an exclusive signed-zero endpoint.
            if let Some(
                primitive @ (typed_trees::types::PrimitiveType::F32
                | typed_trees::types::PrimitiveType::F64),
            ) = program.primitive_type_reference(carrier)
            {
                let Some(minimum) =
                    crate::closed_float_range_endpoint(program, *minimum, primitive)
                else {
                    return true;
                };
                let Some(maximum) =
                    crate::closed_float_range_endpoint(program, *maximum, primitive)
                else {
                    return true;
                };
                let zero = if primitive == typed_trees::types::PrimitiveType::F32 {
                    semantic_vocabulary::IeeeFloatValue::Binary32(0)
                } else {
                    semantic_vocabulary::IeeeFloatValue::Binary64(0)
                };
                return !crate::ieee_float_range_ordered(minimum, zero)
                    || !crate::ieee_float_range_ordered(zero, maximum)
                    || (!end_inclusive && crate::ieee_float_range_ordered(maximum, zero));
            }
            let Some(minimum) = crate::closed_integer_range_bound(program, *minimum) else {
                return true;
            };
            let Some(maximum) =
                crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)
            else {
                return true;
            };
            minimum > BigInt::zero() || maximum < BigInt::zero()
        }
        TypeConstraintNode::ArithmeticDomain(_) => false,
    }
}

pub fn data_requires_establishment(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
) -> bool {
    // Build descriptions may query before preparation; final entry admission
    // consumes the prepared, checked program. Both use the semantic predicate
    // judgment, not the syntax-stage conservative flag as a failed proof.
    // Applied predicates read their retained static arguments; the recursive
    // walk below still owns contained fields and case payloads.
    if definition.zero_gated && !crate::data_predicates_admit_zero(program, definition) {
        return true;
    }
    // A fresh index answers each top-level query, so the answer depends only
    // on the program and the definition and memoizes for the checked build.
    crate::value_custody::type_properties::memoized_data_requires_establishment(
        program,
        definition,
        || data_requires_establishment_inner(program, definition, &mut EstablishmentIndex::new()),
    )
}

fn data_requires_establishment_inner(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::data::DataDefinition,
    index: &mut EstablishmentIndex,
) -> bool {
    if definition.zero_gated && !crate::data_predicates_admit_zero(program, definition) {
        return true;
    }
    let key = EstablishmentIdentity::of(definition);
    if index.seen.contains(&key) {
        return false;
    }
    if let Ok(position) = index
        .computed
        .binary_search_by_key(&key.order(), |(identity, _)| identity.order())
    {
        return index.computed[position].1;
    }
    index.seen.push(key);

    let members = program.data_members(definition);
    let common_gated = members.iter().any(|member| match member {
        DataMember::Field(field) => {
            type_requires_establishment_inner(program, field.type_reference, index)
        }
        DataMember::Variant(_) => false,
    });
    // The zero tag selects only the first case. A clause that is satisfiable
    // for some payload still needs establishment when its actual zero payload
    // does not prove it; the syntax-stage flag is not a positive certificate.
    let zero_case_gated = members
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) => Some(variant),
            DataMember::Field(_) => None,
        })
        .is_some_and(|variant| {
            !super::struct_literals::case_predicates_admit_zero(program, definition, variant)
                || program.data_payload_fields(variant).iter().any(|field| {
                    type_requires_establishment_inner(program, field.type_reference, index)
                })
        });

    index.seen.pop();
    let result = common_gated || zero_case_gated;
    match index
        .computed
        .binary_search_by_key(&key.order(), |(identity, _)| identity.order())
    {
        Ok(position) => index.computed[position].1 = result,
        Err(position) => index.computed.insert(position, (key, result)),
    }
    result
}

fn validate_payload_field_names(
    data_definition: &typed_trees::data::DataDefinition,
    variant: &typed_trees::data::DataVariant,
    program: &dyn crate::CheckingInput,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let data_members = program.data_members(data_definition);
    let payload_fields = program.data_payload_fields(variant);
    for (field_index, field) in payload_fields.iter().enumerate() {
        if payload_fields[..field_index]
            .iter()
            .any(|previous| previous.name.as_str() == field.name.as_str())
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` has duplicate payload field `{}`",
                data_definition.name, variant.name, field.name
            )));
        }

        // Mixed shapes: a payload field may not reuse a COMMON field's name.
        // Member access (`value.name`) searches common fields first and then
        // every case's payload, so a collision would silently rebind reads.
        if data_members.iter().any(|member| {
            matches!(member, DataMember::Field(common) if common.name.as_str() == field.name.as_str())
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` case `{}` payload field `{}` collides with the common field of the same name",
                data_definition.name,
                variant.name,
                field.name
            )));
        }
    }
}

fn validate_data_member_names(
    data_definition: &typed_trees::data::DataDefinition,
    data_members: &[DataMember],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (member_index, member) in data_members.iter().enumerate() {
        let member_name = match member {
            DataMember::Field(field) => field.name.as_str(),
            DataMember::Variant(variant) => variant.name.as_str(),
        };

        if data_members[..member_index]
            .iter()
            .any(|previous| data_member_name(previous) == member_name)
        {
            diagnostics.push(Diagnostic::error(format!(
                "data `{}` has duplicate member `{member_name}`",
                data_definition.name
            )));
        }
    }
}

fn data_member_name(member: &DataMember) -> &str {
    match member {
        DataMember::Field(field) => field.name.as_str(),
        DataMember::Variant(variant) => variant.name.as_str(),
    }
}
