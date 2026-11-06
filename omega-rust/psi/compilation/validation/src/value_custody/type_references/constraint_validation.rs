use super::open_index_expressions::validate_indexed_domain_arguments;
use super::{StateSignatureOwner, TypeParameterScope, TypeReferenceOwner, type_reference_label};
use crate::value_custody::places::collection_operations::type_reference_is_collection;
use diagnostics::Diagnostic;
use language_semantics::value_domain::ValueDomain;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

/// A domain constraint `T in Name` is satisfied by a declared `domain <T>::Name`:
/// the name component (after the final `::`) must match AND the domain's TARGET
/// carrier must match `base_type`. Domains are storage-bound (re-declared per
/// carrier), so `[u8] in Utf8` resolves to `domain [u8]::Utf8` and NOT to a
/// `domain String::Utf8` that happens to share the `Utf8` name.
fn domain_is_declared(
    program: &dyn crate::CheckingInput,
    base_type: TypeReferenceHandle,
    constraint: &typed_trees::types::DomainConstraint,
) -> bool {
    constraint.symbol.is_valid()
        && program.domain_definitions().iter().any(|domain| {
            domain.symbol == constraint.symbol
                && domain.classification == constraint.classification
                && domain.predicate_presence == constraint.predicate_presence
                && domain.semantic_roles.denotation_dimension.is_some()
                    == constraint.semantic_roles.denotation_dimension.is_some()
                && domain.semantic_roles.arithmetic_policy.is_some()
                    == constraint.semantic_roles.arithmetic_policy.is_some()
                && domain.establishment_routes == constraint.establishment_routes
                && constraint.arguments.len()
                    == typed_trees::domain::index_parameters(program, domain).len()
                && typed_trees::domain::domain_accepts_carrier(
                    program,
                    domain,
                    base_type,
                    &constraint.arguments,
                )
        })
}

pub(super) fn validate_type_constraints_node(
    program: &dyn crate::CheckingInput,
    base_type: TypeReferenceHandle,
    constraints: arena::HandleSpan<TypeConstraintNode>,
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
                if language_semantics::CarryPermission::from_name(name.as_str()).is_some() => {}
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
            TypeConstraintNode::Domain(name) if ValueDomain::from_name(name.as_str()).is_some() => {
                let value_domain = ValueDomain::from_name(name.as_str())
                    .expect("match guard recognized the core value domain");
                match value_domain {
                    ValueDomain::Finite
                        if !primitive_type
                            .is_some_and(|primitive| primitive.accepts_finite_constraint()) =>
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} uses `in {value_domain_name}` on `{carrier}`, but `{value_domain_name}` is only valid on floats",
                            value_domain_name = value_domain.name(),
                            carrier = type_reference_label(program, base_type),
                        )));
                    }
                    ValueDomain::Finite => {}
                }
            }
            // The OmegaLayout FAMILY (`[u8; N] in OmegaLayout<Save>`; ch20
            // "grammars are layout policies"): compiler-known, never declared.
            // The refinement records what the bytes hold -- the carrier stays a
            // plain byte array (see the layout builder's carrier exclusion).
            TypeConstraintNode::Domain(name)
                if typed_trees::wire::is_layout_domain_constraint(name) =>
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
                use numerics::arithmetic::ArithmeticDomain;
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
    use language_semantics::{DomainSemanticRole, SemanticDomainTable};

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
                        numerics::arithmetic::ArithmeticDomain::Exact => continue,
                        numerics::arithmetic::ArithmeticDomain::Wrapping => {
                            SemanticDomainTable::WRAPPING
                        }
                        numerics::arithmetic::ArithmeticDomain::Saturating => {
                            SemanticDomainTable::SATURATING
                        }
                        numerics::arithmetic::ArithmeticDomain::Trapping => {
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
                |(prior, _): &(language_semantics::SemanticDomainId, String)| *prior != semantic_id,
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
    program: &dyn crate::CheckingInput,
    base_type: TypeReferenceHandle,
    domain: &typed_trees::types::DomainConstraint,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &TypeReferenceOwner<'_>,
) {
    let Some((schema_name, grammar)) =
        typed_trees::wire::layout_domain_constraint_arguments(program, domain)
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
