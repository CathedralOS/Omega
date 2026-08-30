use super::open_index_expressions::validate_indexed_domain_arguments;
use super::{
    StateSignatureOwner, TypeParameterScope, TypeReferenceOwner, type_reference_label,
    type_references_match,
};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// A domain constraint `T in Name` is satisfied by a declared `domain <T>::Name`:
/// the name component (after the final `::`) must match AND the domain's TARGET
/// carrier must match `base_type`. Domains are storage-bound (re-declared per
/// carrier), so `[u8] in Utf8` resolves to `domain [u8]::Utf8` and NOT to a
/// `domain String::Utf8` that happens to share the `Utf8` name.
fn domain_is_declared(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    constraint: &psi_typed_trees::types::DomainConstraint,
) -> bool {
    constraint.symbol.is_valid()
        && program.domain_definitions().iter().any(|domain| {
            domain.symbol == constraint.symbol
                && domain.classification == constraint.classification
                && domain.predicate_body == constraint.predicate_body
                && domain.semantic_roles.denotation_dimension.is_some()
                    == constraint.semantic_roles.denotation_dimension.is_some()
                && domain.semantic_roles.arithmetic_policy.is_some()
                    == constraint.semantic_roles.arithmetic_policy.is_some()
                && domain.establishment_routes == constraint.establishment_routes
                && if psi_typed_trees::domain::index_parameters(program, domain).is_empty() {
                    type_references_match(program, base_type, domain.target_type)
                } else {
                    constraint.arguments.len()
                        == psi_typed_trees::domain::index_parameters(program, domain).len()
                }
        })
}

pub(super) fn validate_type_constraints_node(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    constraints: psi_arena::HandleSpan<TypeConstraintNode>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
    type_parameter_scope: TypeParameterScope<'_>,
) {
    let primitive_type = program.type_reference_table.primitive_type(base_type);
    let constraints = program.type_reference_table.constraints(constraints);
    validate_semantic_role_composition(constraints, diagnostics, &owner);

    for constraint in constraints {
        match constraint {
            TypeConstraintNode::Named(name) if name.as_str() == "finite" => {
                let Some(primitive_type) = primitive_type else {
                    continue;
                };

                if !primitive_type.accepts_finite_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `finite` on `{}`, but `finite` is only valid on floats",
                        primitive_type.name()
                    )));
                }
            }
            TypeConstraintNode::Named(_) => {}
            // Carry permissions are compiler-owned, subject-polymorphic
            // positive facts. They classify any carrier and therefore do not
            // resolve through the storage-bound declared-domain table.
            TypeConstraintNode::Domain(name)
                if psi_language_semantics::CarryPermission::from_name(name.as_str()).is_some() => {}
            TypeConstraintNode::Domain(name) if name.as_str().starts_with("Carry::") => {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses unknown compiler carry permission `{name}`; expected \
                     `Carry::AcrossSuspend`, `Carry::AnyCpu`, `Carry::AnyThread`, \
                     `Carry::MovableAddress`, or the `Carry::Portable` alias",
                )));
            }
            // Compiler-known value domains are authored with the honest
            // domain spelling (`f64 in Finite`) but do not require a user
            // `domain` declaration.  Their carrier restrictions remain
            // checked here, at the same declaration fence as the retired
            // bracketed proof spelling.
            TypeConstraintNode::Domain(name)
                if psi_language_semantics::value_domain::ValueDomain::from_name(name.as_str())
                    .is_some() =>
            {
                let value_domain =
                    psi_language_semantics::value_domain::ValueDomain::from_name(name.as_str())
                        .expect("match guard recognized the core value domain");
                match value_domain {
                    psi_language_semantics::value_domain::ValueDomain::Finite
                        if !primitive_type
                            .is_some_and(|primitive| primitive.accepts_finite_constraint()) =>
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} uses `in {value_domain_name}` on `{carrier}`, but `{value_domain_name}` is only valid on floats",
                            value_domain_name = value_domain.name(),
                            carrier = type_reference_label(program, base_type),
                        )));
                    }
                    psi_language_semantics::value_domain::ValueDomain::Finite => {}
                }
            }
            // The OmegaLayout FAMILY (`[u8; N] in OmegaLayout<Save>`; ch20
            // "grammars are layout policies"): compiler-known, never declared.
            // The refinement records what the bytes hold -- the carrier stays a
            // plain byte array (see the layout builder's carrier exclusion).
            TypeConstraintNode::Domain(name)
                if psi_typed_trees::wire::is_layout_domain_constraint(name) =>
            {
                validate_layout_domain_constraint(program, base_type, name, diagnostics, &owner);
            }
            // A declared encoding domain on a carrier (`[u8] in Utf8`; ch8). It
            // must reference a `domain ...::Name` declaration -- this rejects
            // typos (`in Utf8x`) and is what distinguishes a domain from a
            // structural property (`[copy]`, which stays an unvalidated `Named`).
            TypeConstraintNode::Domain(domain) => {
                if !domain_is_declared(program, base_type, domain) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `in {domain}`, but no `domain` named `{domain}` is declared \
                         for `{}` (domains are bound to their storage type)",
                        type_reference_label(program, base_type)
                    )));
                } else {
                    validate_indexed_domain_arguments(
                        program,
                        domain,
                        type_parameter_scope,
                        diagnostics,
                        &owner,
                    );
                }
            }
            TypeConstraintNode::Range { minimum, maximum } => {
                let Some(primitive_type) = primitive_type else {
                    continue;
                };

                if !primitive_type.accepts_range_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `range` on `{}`, but `range` is only valid on numeric types",
                        primitive_type.name()
                    )));
                    continue;
                }
                match (
                    crate::arithmetic_domains::literal_i64(program, *minimum),
                    crate::arithmetic_domains::literal_i64(program, *maximum),
                ) {
                    (Some(low), Some(high)) if low > high => {
                        // An inverted range `[10..=5]` is the empty set -- no value can
                        // satisfy it -- yet it silently disabled range checking (every
                        // value "passed" the malformed bound). Reject it at the declaration.
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} declares an inverted range `[{low}..={high}]`: the low bound \
                             exceeds the high bound, so no value can satisfy it",
                        )));
                    }
                    (Some(_), Some(_)) => {}
                    // An INTEGER range whose bound does not const-evaluate (a place
                    // read, a call): the range would silently behave UNBOUNDED --
                    // every store "passes" a constraint the declaration claims.
                    // Constant expressions (`[0 - 1..=40]`) fold above; anything
                    // else is rejected rather than lied about. FLOAT ranges keep
                    // their own literal path (the proof side reads them as
                    // FloatRange), so they are exempt here.
                    //
                    // EXCEPTION (R1a, dependent ranges): a STATE PARAMETER may
                    // carry a literal minimum with a `self.<field> [+/- k]`
                    // maximum (`i: u32 [0..=self.count]`) -- the caller-side
                    // proof plan mints a relational atom for it (checked at
                    // every transition into the state) and the index prover
                    // substitutes through the field's own enforced range, so
                    // the bound is DISCHARGED, never silently unbounded. The
                    // named field must exist on the machine's attached data and
                    // itself carry an enforced literal integer range -- checked
                    // here so an unranged/mistyped field name refuses at the
                    // declaration instead of unproving every call site.
                    _ if primitive_type.accepts_integer_literal() => {
                        if let Some(message) = dependent_state_parameter_range_error(
                            program, &owner, *minimum, *maximum,
                        ) {
                            diagnostics.push(Diagnostic::error(message));
                        }
                    }
                    _ => {}
                }
            }
            // Arithmetic policy domains (`Wrapping`/`Saturating`/`Trapping`).
            // On INTEGERS they are meaningful today (decision 17). On FLOATS
            // (float semantics, ch5 "Float Facts", 2026-07-18): `Wrapping` is
            // a hard error -- there is no modular reading of a float; the
            // other two are recognized real policies but do not lower yet
            // (float `Trapping`/`Saturating` = the F5 rung), so they refuse
            // LOUDLY here rather than silently no-opping. `accepts_finite_
            // constraint()` is the floats-only test (the same one `finite`
            // uses just above).
            TypeConstraintNode::ArithmeticDomain(domain) => {
                use psi_numerics::arithmetic::ArithmeticDomain;
                if primitive_type.is_some_and(|primitive| primitive.accepts_finite_constraint()) {
                    let primitive_name = primitive_type.expect("checked above").name();
                    match domain {
                        ArithmeticDomain::Wrapping => diagnostics.push(Diagnostic::error(format!(
                            "{owner} applies `Wrapping` to `{primitive_name}`, but there \
                                 is no modular reading of a float -- a wrapping policy is only \
                                 meaningful on integers"
                        ))),
                        // F5 LANDED (2026-07-16): Saturating clamps magnitude
                        // overflow to +-MAX_FINITE (div-by-zero/invalid keep
                        // their non-finites, per the brief); Trapping traps on
                        // invalid/overflow/div-by-zero. Lowered on the
                        // interpreter + aarch64; x86_64 passes through until
                        // its host session (the documented status-quo
                        // divergence, same as the float->int cast policies).
                        ArithmeticDomain::Saturating
                        | ArithmeticDomain::Trapping
                        | ArithmeticDomain::Exact => {}
                    }
                }
            }
        }
    }
}

fn validate_semantic_role_composition(
    constraints: &[TypeConstraintNode],
    diagnostics: &mut Vec<Diagnostic>,
    owner: &TypeReferenceOwner<'_>,
) {
    use psi_language_semantics::{DomainSemanticRole, SemanticDomainTable};

    for role in [
        DomainSemanticRole::DenotationDimension,
        DomainSemanticRole::ArithmeticPolicy,
    ] {
        let mut contributions = Vec::new();
        for constraint in constraints {
            let contribution = match constraint {
                TypeConstraintNode::Domain(domain) => domain
                    .semantic_roles
                    .contribution(role)
                    .map(|semantic_id| (semantic_id, domain.name.to_string())),
                TypeConstraintNode::ArithmeticDomain(domain)
                    if role == DomainSemanticRole::ArithmeticPolicy =>
                {
                    let semantic_id = match domain {
                        psi_numerics::arithmetic::ArithmeticDomain::Exact => continue,
                        psi_numerics::arithmetic::ArithmeticDomain::Wrapping => {
                            SemanticDomainTable::WRAPPING
                        }
                        psi_numerics::arithmetic::ArithmeticDomain::Saturating => {
                            SemanticDomainTable::SATURATING
                        }
                        psi_numerics::arithmetic::ArithmeticDomain::Trapping => {
                            SemanticDomainTable::TRAPPING
                        }
                    };
                    Some((semantic_id, domain.name().to_owned()))
                }
                _ => None,
            };
            let Some((semantic_id, label)) = contribution else {
                continue;
            };
            if contributions.iter().all(
                |(prior, _): &(psi_language_semantics::SemanticDomainId, String)| {
                    *prior != semantic_id
                },
            ) {
                contributions.push((semantic_id, label));
            }
        }
        if contributions.len() <= 1 {
            continue;
        }
        let rendered = contributions
            .iter()
            .map(|(_, label)| format!("`{label}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let rule = match role {
            DomainSemanticRole::ArithmeticPolicy => {
                "a domain chain may carry at most one policy domain"
            }
            DomainSemanticRole::DenotationDimension => {
                "a domain chain may carry at most one denotation/dimension domain"
            }
        };
        diagnostics.push(Diagnostic::error(format!(
            "{owner} declares conflicting `{}` semantic-role contributions {rendered}; {rule}",
            role.as_str(),
        )));
    }
}

/// Validate one `OmegaLayout` instance in constraint position (ch20 §7,
/// domain entry = MINTS ONLY). A layout domain is a fact about bytes that a
/// mint (validate) or an encoder makes true -- it rides BORROWED VIEWS
/// (`&[u8] in OmegaLayout<Save>`, the mint-result payload spelling). It is
/// NEVER declared on owned storage: a stored `[u8; N]` is zero bytes at ZII,
/// so a declared refinement would be a trivially-claimed membership -- false
/// until the first encode. (Contrast `[u8; N] in Utf8`, whose zero state
/// `{len: 0}` is genuinely valid and whose write ops preserve the invariant.)
fn validate_layout_domain_constraint(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    domain: &psi_typed_trees::types::DomainConstraint,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &TypeReferenceOwner<'_>,
) {
    let Some((schema_name, grammar)) =
        psi_typed_trees::wire::layout_domain_constraint_arguments(program, domain)
    else {
        return;
    };
    // Owned stored bytes: rejected outright (mints-only). Borrowed views
    // (`&[u8]`, `&[u8; N]`) are the legal carrier.
    match program.type_reference_table.type_reference(base_type) {
        TypeReferenceNode::FixedArray { .. } => {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} declares `in OmegaLayout<{schema_name}>` on owned stored bytes; a layout domain is a \
                 fact about bytes ESTABLISHED by validate/encode, never declared on storage \
                 (a zeroed buffer holds no valid encoding). Hold the value (`{schema_name}`) \
                 and encode at the edge, or carry the fact on a borrowed view \
                 (`&[u8] in OmegaLayout<{schema_name}>`)"
            )));
            return;
        }
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {}
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} uses `in OmegaLayout<{schema_name}>` on `{}`, but a layout domain lives on a borrowed \
                 byte view (`&[u8] in OmegaLayout<{schema_name}>`)",
                type_reference_label(program, base_type)
            )));
            return;
        }
    }
    if let Some(grammar) = grammar {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} names the `{grammar}` grammar explicitly, but `Derived` is the default \
             and the only implemented grammar of `OmegaLayout` (an explicit `Packed` \
             parameter is not implemented yet)"
        )));
        return;
    }
    if !program
        .wire_schemas()
        .iter()
        .any(|schema| schema.name.as_str() == schema_name.as_str())
    {
        let is_plain_data = program
            .data_definitions()
            .iter()
            .any(|data| data.name.as_str() == schema_name.as_str());
        if is_plain_data {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} uses `in OmegaLayout<{schema_name}>`, but data `{schema_name}` has \
                 no identity numbers; the packed grammar of an unnumbered schema is not \
                 implemented yet -- number the fields (`1: name: type;`) for the derived \
                 tagged grammar"
            )));
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} uses `in OmegaLayout<{schema_name}>`, but no data definition named \
                 `{schema_name}` exists"
            )));
        }
    }
}

/// R1a dependent-range gate for the non-constant-bound arm: `None` ACCEPTS
/// (the range is the admissible dependent class on a machine state
/// parameter, and its named field can actually discharge); `Some(message)`
/// keeps refusing with the sharpest description available. Kept in lockstep
/// with the proof plan's atom minting and the index prover's substitution
/// through the shared `psi_typed_trees::dependent_ranges` recognizer.
fn dependent_state_parameter_range_error(
    program: &TypedTrees,
    owner: &TypeReferenceOwner<'_>,
    minimum: psi_typed_trees::expression::ExpressionHandle,
    maximum: psi_typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    let generic = || {
        format!(
            "{owner} declares a range whose bound is not a constant integer \
             expression; a non-constant bound cannot be enforced (it would \
             silently behave unbounded). A dependent maximum \
             (`[0..=self.<field>]`) is supported on machine STATE PARAMETERS \
             whose named field carries an enforced literal integer range"
        )
    };
    if crate::arithmetic_domains::literal_i64(program, minimum).is_none() {
        return Some(generic());
    }
    // Sibling-length class (`[0..items.len]`): the named sibling must be a
    // slice/fixed-array parameter of the SAME state, and only offsets <= 0
    // are admissible (a `+k` bound would exceed the length).
    if let Some(sibling) = psi_typed_trees_sibling(program, maximum) {
        let TypeReferenceOwner::StateParameter {
            owner: StateSignatureOwner::Machine(machine_name),
            state: state_name,
            ..
        } = owner
        else {
            return Some(generic());
        };
        if sibling.offset > 0 {
            return Some(format!(
                "{owner} declares a sibling-length maximum with a positive offset \
                 (`{}.len + {}`); an index past the length cannot be satisfied",
                sibling.sibling, sibling.offset,
            ));
        }
        let sibling_is_sliceable = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == *machine_name)
            .and_then(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .find(|state| state.name.as_str() == *state_name)
                    .map(|state| (machine, state))
            })
            .is_some_and(|(_, state)| {
                program.state_parameters(state).iter().any(|parameter| {
                    parameter.name.as_str() == sibling.sibling.as_str()
                        && parameter.type_reference.is_valid()
                        && type_reference_is_sliceable(program, parameter.type_reference)
                })
            });
        if !sibling_is_sliceable {
            return Some(format!(
                "{owner} declares a sibling-length maximum naming `{}`, but no slice or \
                 fixed-array parameter of that name exists on the state",
                sibling.sibling,
            ));
        }
        return None;
    }
    let Some(symbolic) =
        psi_typed_trees::dependent_ranges::symbolic_max_bound(&program.expression_table, maximum)
    else {
        return Some(generic());
    };
    let TypeReferenceOwner::StateParameter {
        owner: StateSignatureOwner::Machine(machine_name),
        ..
    } = owner
    else {
        return Some(generic());
    };
    let field_is_dischargeable = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == *machine_name)
        .and_then(|machine| machine.attached_data.as_ref())
        .and_then(|attached| {
            program
                .data_definitions()
                .iter()
                .find(|data| data.name.as_str() == attached.as_str())
        })
        .and_then(|data| {
            program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    psi_typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == symbolic.field.as_str() =>
                    {
                        field
                            .type_reference
                            .is_valid()
                            .then_some(field.type_reference)
                    }
                    _ => None,
                })
        })
        .is_some_and(|field_type| {
            // The field must EXIST and be an integer primitive. A LITERAL
            // range on it is NOT required: the guard route discharges
            // rangeless fields at every call site (`arg < self.rows`); the
            // floor route and the callee substitution simply contribute
            // nothing for a rangeless field (both are None-safe), and the
            // R3 product rule supplies bounds through couplings instead.
            crate::places::unwrapped_type_reference(program, field_type)
                .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
                .is_some_and(|primitive| primitive.accepts_integer_literal())
        });
    if !field_is_dischargeable {
        return Some(format!(
            "{owner} declares a dependent maximum naming `self.{}`, but no integer field of \
             that name exists on the machine's attached data",
            symbolic.field,
        ));
    }
    None
}

fn psi_typed_trees_sibling(
    program: &TypedTrees,
    maximum: psi_typed_trees::expression::ExpressionHandle,
) -> Option<psi_typed_trees::dependent_ranges::SiblingLenBound> {
    psi_typed_trees::dependent_ranges::sibling_len_bound(&program.expression_table, maximum)
}

fn type_reference_is_sliceable(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { referee, .. } => {
            type_reference_is_sliceable(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_sliceable(program, *base_type)
        }
        TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. } => true,
        _ => false,
    }
}
