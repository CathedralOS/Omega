use crate::StateSignatureOwner;
use crate::symbols::TopLevelSymbols;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::TypeParameter;
use omega_typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub(crate) enum TypeReferenceOwner<'program> {
    DomainTarget {
        domain: &'program str,
        generic_depth: usize,
    },
    DataField {
        data: &'program str,
        field: &'program str,
        generic_depth: usize,
    },
    MachineOwnedData {
        machine: &'program str,
        data: &'program str,
        generic_depth: usize,
    },
    StateLocalData {
        machine: &'program str,
        state: &'program str,
        local: &'program str,
        generic_depth: usize,
    },
    StateParameter {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        parameter: &'program str,
        generic_depth: usize,
    },
    StateReturn {
        owner: StateSignatureOwner<'program>,
        state: &'program str,
        generic_depth: usize,
    },
}

impl TypeReferenceOwner<'_> {
    /// `Self` is a legal type name inside TRAIT machine signatures
    /// (`machine equals(&self, other: &Self) -> bool;`): it denotes the
    /// conforming type, substituted per conformance.
    fn allows_self_type(&self) -> bool {
        matches!(
            self,
            Self::StateParameter {
                owner: StateSignatureOwner::Trait(_),
                ..
            } | Self::StateReturn {
                owner: StateSignatureOwner::Trait(_),
                ..
            }
        )
    }

    fn generic_argument(self) -> Self {
        match self {
            Self::DomainTarget {
                domain,
                generic_depth,
            } => Self::DomainTarget {
                domain,
                generic_depth: generic_depth + 1,
            },
            Self::DataField {
                data,
                field,
                generic_depth,
            } => Self::DataField {
                data,
                field,
                generic_depth: generic_depth + 1,
            },
            Self::MachineOwnedData {
                machine,
                data,
                generic_depth,
            } => Self::MachineOwnedData {
                machine,
                data,
                generic_depth: generic_depth + 1,
            },
            Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth,
            } => Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth: generic_depth + 1,
            },
            Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth,
            } => Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth: generic_depth + 1,
            },
            Self::StateReturn {
                owner,
                state,
                generic_depth,
            } => Self::StateReturn {
                owner,
                state,
                generic_depth: generic_depth + 1,
            },
        }
    }
}

impl fmt::Display for TypeReferenceOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generic_depth = match self {
            Self::DomainTarget {
                domain,
                generic_depth,
            } => {
                write!(formatter, "domain `{domain}` target type")?;
                *generic_depth
            }
            Self::DataField {
                data,
                field,
                generic_depth,
            } => {
                write!(formatter, "data `{data}` field `{field}`")?;
                *generic_depth
            }
            Self::MachineOwnedData {
                machine,
                data,
                generic_depth,
            } => {
                write!(formatter, "machine `{machine}` owned data `{data}`")?;
                *generic_depth
            }
            Self::StateLocalData {
                machine,
                state,
                local,
                generic_depth,
            } => {
                write!(
                    formatter,
                    "machine `{machine}` state `{state}` local data `{local}`"
                )?;
                *generic_depth
            }
            Self::StateParameter {
                owner,
                state,
                parameter,
                generic_depth,
            } => {
                write!(formatter, "{owner} state `{state}` parameter `{parameter}`")?;
                *generic_depth
            }
            Self::StateReturn {
                owner,
                state,
                generic_depth,
            } => {
                write!(formatter, "{owner} state `{state}` return type")?;
                *generic_depth
            }
        };

        for _ in 0..generic_depth {
            formatter.write_str(" generic argument")?;
        }

        Ok(())
    }
}

pub(crate) fn validate_type_reference_handle(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    // Q9 ruling: see the twin call in the with_type_parameters entry.
    crate::arithmetic_domains::check_range_under_non_exact_domain(
        program,
        type_reference,
        owner,
        diagnostics,
    );
    validate_type_reference_handle_with_context(
        program,
        type_reference,
        symbols,
        diagnostics,
        owner,
        false,
        TypeParameterScope::empty(),
    );
}

pub(crate) fn validate_type_reference_handle_with_type_parameters(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
    type_parameters: &[TypeParameter],
) {
    // Q9 ruling: a range constraint under a non-Exact domain is ill-formed
    // (checked once per declared handle; the accessors walk nested
    // Reference/Constrained spellings transitively).
    crate::arithmetic_domains::check_range_under_non_exact_domain(
        program,
        type_reference,
        owner,
        diagnostics,
    );
    validate_type_reference_handle_with_context(
        program,
        type_reference,
        symbols,
        diagnostics,
        owner,
        false,
        TypeParameterScope { type_parameters },
    );
}

fn validate_type_reference_handle_with_context(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
    allow_bare_str: bool,
    type_parameter_scope: TypeParameterScope<'_>,
) {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            validate_type_reference_handle_with_context(
                program,
                *referee,
                symbols,
                diagnostics,
                owner,
                type_reference_is_named_str(program, *referee),
                type_parameter_scope,
            );
        }
        TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } => {
            validate_type_reference_handle_with_context(
                program,
                *base_type,
                symbols,
                diagnostics,
                owner,
                allow_bare_str,
                type_parameter_scope,
            );
            validate_type_constraints_node(program, *base_type, *constraints, diagnostics, owner);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            // Element spelling of the Q9 rule (`[u8 [0..=9] in Wrapping; 3]`).
            crate::arithmetic_domains::check_range_under_non_exact_domain(
                program,
                *element_type,
                owner,
                diagnostics,
            );
            validate_type_reference_handle_with_context(
                program,
                *element_type,
                symbols,
                diagnostics,
                owner,
                false,
                type_parameter_scope,
            );
        }
        TypeReferenceNode::Slice { element_type } => {
            validate_type_reference_handle_with_context(
                program,
                *element_type,
                symbols,
                diagnostics,
                owner,
                false,
                type_parameter_scope,
            );
        }
        TypeReferenceNode::Generic {
            base_name,
            arguments,
            ..
        } => {
            if !symbols.has_type(base_name) && !type_parameter_scope.contains(base_name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown generic type `{base_name}`"
                )));
            }

            validate_generic_argument_bounds(
                program,
                symbols,
                base_name.as_str(),
                *arguments,
                type_parameter_scope,
                diagnostics,
            );

            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                validate_type_reference_handle_with_context(
                    program,
                    *argument,
                    symbols,
                    diagnostics,
                    owner.generic_argument(),
                    false,
                    type_parameter_scope,
                );
            }
        }
        TypeReferenceNode::DynamicTrait { name, .. } => {
            if symbols.trait_definition(name.as_str()).is_none() {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown dynamic trait `{name}`"
                )));
            }
        }
        TypeReferenceNode::Named { name, .. } => {
            if name.as_str() == "string" && !allow_bare_str {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses unsized text view type `string` by value; use `&string`"
                )));
                return;
            }

            if name.as_str() == "Self" && owner.allows_self_type() {
                return;
            }

            if !symbols.has_type(name) && !type_parameter_scope.contains(name.as_str()) {
                // The builtin era-tagged container exists only for data types
                // that declare version blocks (frozen decision 14); an
                // unresolved `Versioned<X>` means `X` has no eras (or does
                // not exist).
                if let Some(payload) =
                    omega_core::versioning::versioned_container_payload(name.as_str())
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} references `{name}`, but `Versioned<T>` exists only for data types with `version vN {{ ... }}` blocks -- `{payload}` declares none"
                    )));
                    return;
                }
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
        TypeReferenceNode::Unit => {}
    }
}

/// Instantiation-time property-bound check (frozen decision 13): every type
/// argument must carry each bound the matching parameter declares, e.g.
/// `Box<String>` is rejected when `Box` declares `data Box<T [copy]>`. Bounds
/// on in-scope type parameters count, so `data Outer<U [copy]>` may store a
/// `Box<U>`.
fn validate_generic_argument_bounds(
    program: &TypedTrees,
    symbols: &TopLevelSymbols<'_>,
    base_name: &str,
    arguments: omega_core::arena::HandleSpan<TypeReferenceHandle>,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(definition) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == base_name)
    else {
        return;
    };

    let parameters = program.data_type_parameters(definition);
    let argument_handles = program.type_reference_table.type_reference_handles(arguments);
    for (parameter, argument) in parameters.iter().zip(argument_handles) {
        let bound_names = crate::properties::declared_property_names(&parameter.bounds);
        for property in &bound_names {
            if crate::properties::type_satisfies_declared_property(
                program,
                symbols,
                type_parameter_scope.type_parameters,
                *argument,
                property,
            ) {
                continue;
            }
            diagnostics.push(Diagnostic::error(format!(
                "type parameter `{} [{}]` of `{base_name}` was instantiated with `{}`, which does not declare `[{property}]`",
                parameter.name,
                bound_names.join(", "),
                type_reference_label(program, *argument)
            )));
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TypeParameterScope<'program> {
    type_parameters: &'program [TypeParameter],
}

impl TypeParameterScope<'_> {
    fn empty() -> Self {
        Self {
            type_parameters: &[],
        }
    }

    fn contains(self, name: &str) -> bool {
        self.type_parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == name)
    }
}

/// A domain constraint `T in Name` is satisfied by a declared `domain <T>::Name`:
/// the name component (after the final `::`) must match AND the domain's TARGET
/// carrier must match `base_type`. Domains are storage-bound (re-declared per
/// carrier), so `[u8] in Utf8` resolves to `domain [u8]::Utf8` and NOT to a
/// `domain String::Utf8` that happens to share the `Utf8` name.
fn domain_is_declared(program: &TypedTrees, base_type: TypeReferenceHandle, wanted: &str) -> bool {
    program.domain_definitions().iter().any(|domain| {
        let full = domain.name.as_str();
        let name_matches = full.rsplit("::").next().unwrap_or(full) == wanted;
        name_matches && type_references_match(program, base_type, domain.target_type)
    })
}

fn type_reference_is_named_str(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "string"
    )
}

fn validate_type_constraints_node(
    program: &TypedTrees,
    base_type: TypeReferenceHandle,
    constraints: omega_core::arena::HandleSpan<TypeConstraintNode>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    let primitive_type = program.type_reference_table.primitive_type(base_type);

    for constraint in program.type_reference_table.constraints(constraints) {
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
            // The OmegaLayout FAMILY (`[u8; N] in OmegaLayout<Save>`; ch20
            // "grammars are layout policies"): compiler-known, never declared.
            // The refinement records what the bytes hold -- the carrier stays a
            // plain byte array (see the layout builder's carrier exclusion).
            TypeConstraintNode::Domain(name)
                if omega_typed_trees::wire::is_layout_domain_name(name.as_str()) =>
            {
                validate_layout_domain_constraint(
                    program,
                    base_type,
                    name.as_str(),
                    diagnostics,
                    &owner,
                );
            }
            // A declared encoding domain on a carrier (`[u8] in Utf8`; ch8). It
            // must reference a `domain ...::Name` declaration -- this rejects
            // typos (`in Utf8x`) and is what distinguishes a domain from a
            // structural property (`[copy]`, which stays an unvalidated `Named`).
            TypeConstraintNode::Domain(name) => {
                if !domain_is_declared(program, base_type, name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `in {name}`, but no `domain` named `{name}` is declared \
                         for `{}` (domains are bound to their storage type)",
                        type_reference_label(program, base_type)
                    )));
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
            // An arithmetic overflow domain (`Wrapping`/`Saturating`/`Trapping`)
            // is only meaningful on integer primitives. (Stricter integer-only
            // validation can be added when the domains gain distinct codegen.)
            TypeConstraintNode::ArithmeticDomain(_) => {}
        }
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
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
    owner: &TypeReferenceOwner<'_>,
) {
    let Some((schema_name, grammar)) = omega_typed_trees::wire::layout_domain_arguments(name)
    else {
        return;
    };

    // Owned stored bytes: rejected outright (mints-only). Borrowed views
    // (`&[u8]`, `&[u8; N]`) are the legal carrier.
    match program.type_reference_table.type_reference(base_type) {
        TypeReferenceNode::FixedArray { .. } => {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} declares `in {name}` on owned stored bytes; a layout domain is a \
                 fact about bytes ESTABLISHED by validate/encode, never declared on storage \
                 (a zeroed buffer holds no valid encoding). Hold the value (`{schema_name}`) \
                 and encode at the edge, or carry the fact on a borrowed view \
                 (`&[u8] in {name}`)"
            )));
            return;
        }
        TypeReferenceNode::Reference { .. } | TypeReferenceNode::Slice { .. } => {}
        _ => {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} uses `in {name}` on `{}`, but a layout domain lives on a borrowed \
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
        .any(|schema| schema.name.as_str() == schema_name)
    {
        let is_plain_data = program
            .data_definitions()
            .iter()
            .any(|data| data.name.as_str() == schema_name);
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

pub(crate) fn type_references_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }

    program.display_type_reference_with_constraints(actual)
        == program.display_type_reference_with_constraints(required)
}

pub(crate) fn type_reference_label(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> String {
    if type_reference.is_valid() {
        program.display_type_reference_with_constraints(type_reference)
    } else {
        "()".to_owned()
    }
}

/// R1a dependent-range gate for the non-constant-bound arm: `None` ACCEPTS
/// (the range is the admissible dependent class on a machine state
/// parameter, and its named field can actually discharge); `Some(message)`
/// keeps refusing with the sharpest description available. Kept in lockstep
/// with the proof plan's atom minting and the index prover's substitution
/// through the shared `omega_typed_trees::dependent_ranges` recognizer.
fn dependent_state_parameter_range_error(
    program: &TypedTrees,
    owner: &TypeReferenceOwner<'_>,
    minimum: omega_typed_trees::expression::ExpressionHandle,
    maximum: omega_typed_trees::expression::ExpressionHandle,
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
    if let Some(sibling) = omega_typed_rees_sibling(program, maximum) {
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
    let Some(symbolic) = omega_typed_trees::dependent_ranges::symbolic_max_bound(
        &program.expression_table,
        maximum,
    ) else {
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
            program.data_members(data).iter().find_map(|member| {
                match member {
                    omega_typed_trees::data::DataMember::Field(field)
                        if field.name.as_str() == symbolic.field.as_str() =>
                    {
                        field.type_reference.is_valid().then_some(field.type_reference)
                    }
                    _ => None,
                }
            })
        })
        .and_then(|field_type| crate::arithmetic_domains::range_constraint_interval(program, field_type))
        .is_some();
    if !field_is_dischargeable {
        return Some(format!(
            "{owner} declares a dependent maximum naming `self.{}`, but that field does not \
             carry an enforced literal integer range on the machine's attached data -- the \
             bound could not be discharged at any call site. Range the field (`{}: u32 \
             [0..=N]`, Exact) or spell a literal maximum",
            symbolic.field, symbolic.field,
        ));
    }
    None
}

fn omega_typed_rees_sibling(
    program: &TypedTrees,
    maximum: omega_typed_trees::expression::ExpressionHandle,
) -> Option<omega_typed_trees::dependent_ranges::SiblingLenBound> {
    omega_typed_trees::dependent_ranges::sibling_len_bound(&program.expression_table, maximum)
}

fn type_reference_is_sliceable(
    program: &TypedTrees,
    handle: TypeReferenceHandle,
) -> bool {
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
