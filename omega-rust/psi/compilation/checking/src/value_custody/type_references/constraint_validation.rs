use super::open_index_expressions::validate_indexed_domain_arguments;
use super::{
    StateSignatureOwner, TypeParameterScope, TypeReferenceOwner, type_reference_label,
    type_references_match,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

/// A domain constraint `T in Name` is satisfied by a declared `domain <T>::Name`:
/// the name component (after the final `::`) must match AND the domain's TARGET
/// carrier must match `base_type`. Domains are storage-bound (re-declared per
/// carrier), so `[u8] in Utf8` resolves to `domain [u8]::Utf8` and NOT to a
/// `domain String::Utf8` that happens to share the `Utf8` name.
fn domain_is_declared(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    constraint: &typed_trees::types::DomainConstraint,
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
                && if typed_trees::domain::has_generic_carrier(program, domain) {
                    constraint.arguments.len()
                        == typed_trees::domain::index_parameters(program, domain).len()
                } else if typed_trees::domain::index_parameters(program, domain).is_empty() {
                    type_references_match(program, base_type, domain.target_type)
                } else {
                    constraint.arguments.len()
                        == typed_trees::domain::index_parameters(program, domain).len()
                }
        })
}

pub(super) fn validate_type_constraints_node(
    program: &TypedTrees,
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
            TypeConstraintNode::Domain(name)
                if language_semantics::value_domain::ValueDomain::from_name(name.as_str())
                    .is_some() =>
            {
                let value_domain =
                    language_semantics::value_domain::ValueDomain::from_name(name.as_str())
                        .expect("match guard recognized the core value domain");
                match value_domain {
                    language_semantics::value_domain::ValueDomain::Finite
                        if !primitive_type
                            .is_some_and(|primitive| primitive.accepts_finite_constraint()) =>
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "{owner} uses `in {value_domain_name}` on `{carrier}`, but `{value_domain_name}` is only valid on floats",
                            value_domain_name = value_domain.name(),
                            carrier = type_reference_label(program, base_type),
                        )));
                    }
                    language_semantics::value_domain::ValueDomain::Finite => {}
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
            TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } => {
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
                    crate::closed_integer_range_bound(program, *minimum),
                    crate::closed_integer_range_maximum(program, *maximum, *end_inclusive),
                ) {
                    // Empty integer intervals are legal types, not value
                    // establishment. Delivery and representation readers retain
                    // bottom before bounded conversion and reject every value;
                    // a predecessor below i64::MIN must never become no range.
                    (Some(_), Some(_)) => {}
                    // An INTEGER range whose bound does not const-evaluate (a place
                    // read, a call): the range would silently behave UNBOUNDED --
                    // every store "passes" a constraint the declaration claims.
                    // Constant expressions (`[0 - 1..=40]`) fold above; anything
                    // else is rejected rather than lied about. FLOAT ranges use
                    // their own literal proof path, so integer endpoint
                    // evaluation does not decide their validity here.
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
                        // A still-authored endpoint call whose const
                        // evaluation deferred to selected execution is a
                        // PENDING CONSTANT, not a dependent or non-constant
                        // bound: the owning pre-check continuation lands the
                        // integer literal here before final checked lowering,
                        // the same position the `(Some, Some)` arm accepts
                        // for an already evaluated endpoint. Marks are set
                        // only by the build-time evaluation owner when its
                        // continuation defers, so an unmarked call still
                        // refuses below. The gate requires at least one real
                        // mark: a bound whose leaves happen to be integers
                        // but whose whole does not fold (`0..=5 / 2` keeps a
                        // fractional landing) is still the non-constant arm
                        // below, not a pending constant. This gate is
                        // deliberately owner-independent: the
                        // immediate-evaluation route folds the same call to
                        // a literal before any owner distinction applies.
                        let mut found_pending = false;
                        if [*minimum, *maximum].into_iter().all(|bound| {
                            let (closed, pending) =
                                range_bound_pending_evaluation_shape(program, bound);
                            found_pending |= pending;
                            closed
                        }) && found_pending
                        {
                            continue;
                        }
                        if matches!(
                            owner,
                            TypeReferenceOwner::StateReturn {
                                owner: StateSignatureOwner::Machine(_),
                                generic_depth: 0,
                                ..
                            } | TypeReferenceOwner::StateParameter {
                                owner: StateSignatureOwner::Machine(_),
                                generic_depth: 0,
                                ..
                            } | TypeReferenceOwner::StateLocalData {
                                generic_depth: 0,
                                ..
                            }
                        ) && [*minimum, *maximum].into_iter().all(|bound| {
                            crate::proof_contracts::contract_entailment::const_range_bound_is_supported(
                                program,
                                type_parameter_scope.type_parameters,
                                bound,
                            )
                        }) {
                            // Returns, stores and exact call delivery independently
                            // prove these bounds in their current const namespace.
                            continue;
                        }
                        if let Some(message) = dependent_state_parameter_range_error(
                            program,
                            &owner,
                            *minimum,
                            *maximum,
                            *end_inclusive,
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
    program: &TypedTrees,
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

/// The pending-evaluation shape of one range bound:
/// `(closed_or_pending, contains_pending_mark)`. A marked endpoint call admits
/// as the pending constant the owning continuation still folds; a closed
/// integer admits as already evaluated. The evaluator's pending scan descends
/// the same binary composition this walks, so a bound with any other unclosed
/// leaf (a place read, an unmarked call) refuses. `contains_pending_mark`
/// stays false for a bound that merely has integer leaves without a mark --
/// whole-expression folding, not leaf shape, decides those bounds, so
/// `0..=5 / 2` still reaches the non-constant arm below.
fn range_bound_pending_evaluation_shape(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> (bool, bool) {
    if program.pending_const_range_endpoints.contains(&expression) {
        return (true, true);
    }
    if crate::closed_integer_range_bound(program, expression).is_some() {
        return (true, false);
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let (left_closed, left_pending) =
                range_bound_pending_evaluation_shape(program, binary.left);
            let (right_closed, right_pending) =
                range_bound_pending_evaluation_shape(program, binary.right);
            (left_closed && right_closed, left_pending || right_pending)
        }
        _ => (false, false),
    }
}

/// R1a dependent-range gate for the non-constant-bound arm: `None` ACCEPTS
/// (the range is the admissible dependent class on a machine state
/// parameter, and its named field can actually discharge); `Some(message)`
/// keeps refusing with the sharpest description available. Kept in lockstep
/// with the proof plan's atom minting and the index prover's substitution
/// through the shared `typed_trees::dependent_ranges` recognizer.
fn dependent_state_parameter_range_error(
    program: &TypedTrees,
    owner: &TypeReferenceOwner<'_>,
    minimum: typed_trees::expression::ExpressionHandle,
    maximum: typed_trees::expression::ExpressionHandle,
    end_inclusive: bool,
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
    if crate::closed_integer_range_bound(program, minimum)
        .and_then(|value| value.to_i64())
        .is_none()
    {
        return Some(generic());
    }
    // Sibling-length class (`[0..items.len]`): the named sibling must be a
    // slice/fixed-array parameter of the SAME state, and only offsets <= 0
    // are admissible (a `+k` bound would exceed the length).
    if let Some(sibling) = typed_trees::dependent_ranges::sibling_range_maximum(
        &program.expression_table,
        maximum,
        end_inclusive,
    ) {
        let TypeReferenceOwner::StateParameter {
            owner: StateSignatureOwner::Machine(machine),
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
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == *state_name)
            .is_some_and(|state| {
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
    // Scope-value class (`[0..=param]`, `[0..=param - 1]`): the bound names
    // an integer scalar parameter of the SAME state signature, or -- on a
    // local declaration -- a parameter or an earlier local. The strict
    // arithmetic engine binds the named symbol as a proof atom with its
    // declared type envelope as the only premise, so the relation is
    // discharged at formation without versioning the name. This is also the
    // shape a specialized `Value` binder leaves behind: a runtime-bound
    // generic argument's result range `-> u64[0..=Bound]` names the
    // specialization's own realized parameter.
    if let Some(scoped) = typed_trees::dependent_ranges::scoped_range_maximum(
        &program.expression_table,
        maximum,
        end_inclusive,
    ) {
        return scoped_maximum_error(program, owner, scoped);
    }
    let Some(symbolic) = typed_trees::dependent_ranges::symbolic_range_maximum(
        &program.expression_table,
        maximum,
        end_inclusive,
    ) else {
        return Some(generic());
    };
    let TypeReferenceOwner::StateParameter {
        owner: StateSignatureOwner::Machine(machine),
        ..
    } = owner
    else {
        return Some(generic());
    };
    // The signature's owner is already resolved. Recover the field only
    // within that exact attached declaration; another same-named machine or
    // data declaration cannot establish this bound's integer eligibility.
    let field_is_dischargeable = crate::exact_attached_field(
        program,
        machine,
        symbols::SymbolHandle::invalid(),
        symbolic.field.as_str(),
    )
    .is_some_and(|field| {
        // The field must EXIST and be an integer primitive. A LITERAL
        // range on it is NOT required: the guard route discharges
        // rangeless fields at every call site (`arg < self.rows`); the
        // floor route and the callee substitution simply contribute
        // nothing for a rangeless field (both are None-safe), and the
        // R3 product rule supplies bounds through couplings instead.
        crate::value_custody::places::unwrapped_type_reference(program, field.type_reference)
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

/// Scoped-value admission for a recognized `name [+/- k]` maximum: `None`
/// accepts when the bound's named symbol is an integer scalar parameter of
/// the owning state signature, or -- on a `StateLocalData` declaration -- a
/// parameter or a local declared earlier in that state. Everything else keeps
/// refusing. Symbol identity (not spelling) decides membership, so a shadowed
/// or out-of-scope name cannot slip through on a matching identifier.
fn scoped_maximum_error(
    program: &TypedTrees,
    owner: &TypeReferenceOwner<'_>,
    scoped: typed_trees::dependent_ranges::ScopedNameBound,
) -> Option<String> {
    let ExpressionNode::Name(path) = program.expression_table.expression(scoped.name) else {
        return Some(format!(
            "{owner} declares a range maximum that is not a name"
        ));
    };
    let (machine, state_name, local_name, generic_depth) = match owner {
        TypeReferenceOwner::StateParameter {
            owner: StateSignatureOwner::Machine(machine),
            state: state_name,
            generic_depth,
            ..
        }
        | TypeReferenceOwner::StateReturn {
            owner: StateSignatureOwner::Machine(machine),
            state: state_name,
            generic_depth,
            ..
        } => (*machine, *state_name, None, *generic_depth),
        TypeReferenceOwner::StateLocalData {
            machine,
            state: state_name,
            local,
            generic_depth,
        } => (*machine, *state_name, Some(*local), *generic_depth),
        _ => {
            return Some(format!(
                "{owner} declares a range whose bound names a value; a scope-named maximum \
                 (`[0..=<param>]`) is supported on machine state parameters, returns, and locals"
            ));
        }
    };
    if generic_depth != 0 {
        return Some(format!(
            "{owner} declares a scope-named maximum inside a generic argument list; a value-bound \
             qualification is only supported at the outer constraint"
        ));
    }
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.name.as_str() == state_name)
    else {
        return Some(format!(
            "{owner} declares a scope-named maximum, but state `{state_name}` is not declared"
        ));
    };
    let scalar_value = |type_reference: TypeReferenceHandle| {
        crate::value_custody::places::unwrapped_type_reference(program, type_reference)
            .and_then(|unwrapped| program.primitive_type_reference(unwrapped))
            .is_some_and(|primitive| primitive.accepts_integer_literal())
    };
    let is_parameter = program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == path.symbol && scalar_value(parameter.type_reference));
    if is_parameter {
        return None;
    }
    if let Some(local_name) = local_name {
        let mut prior_is_integer = false;
        let mut found = false;
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::LocalData(local) = statement else {
                continue;
            };
            if local.name.as_str() == local_name {
                break;
            }
            if local.symbol == path.symbol {
                found = true;
                prior_is_integer = scalar_value(local.type_reference);
            }
        }
        if found && prior_is_integer {
            return None;
        }
    }
    Some(format!(
        "{owner} declares a scope-named maximum, but the bound names no integer scalar \
         parameter{} in scope",
        if local_name.is_some() {
            " or earlier local"
        } else {
            ""
        },
    ))
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
