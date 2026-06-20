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

/// A domain constraint `in Name` is satisfied by any declared `domain ...::Name`.
/// Domain definitions store their name as `Target::Name` (e.g. `Slice<u8>::Utf8`),
/// so we match the component after the final `::` (or the whole name if none).
fn domain_is_declared(program: &TypedTrees, wanted: &str) -> bool {
    program.domain_definitions().iter().any(|domain| {
        let full = domain.name.as_str();
        full.rsplit("::").next().unwrap_or(full) == wanted
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
            // A declared encoding domain on a carrier (`[u8] in Utf8`; ch8). It
            // must reference a `domain ...::Name` declaration -- this rejects
            // typos (`in Utf8x`) and is what distinguishes a domain from a
            // structural property (`[copy]`, which stays an unvalidated `Named`).
            TypeConstraintNode::Domain(name) => {
                if !domain_is_declared(program, name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `in {name}`, but no `domain` named `{name}` is declared"
                    )));
                }
            }
            TypeConstraintNode::Range { .. } => {
                let Some(primitive_type) = primitive_type else {
                    continue;
                };

                if !primitive_type.accepts_range_constraint() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses `range` on `{}`, but `range` is only valid on numeric types",
                        primitive_type.name()
                    )));
                }
            }
            // An arithmetic overflow domain (`Wrapping`/`Saturating`/`Trapping`)
            // is only meaningful on integer primitives. (Stricter integer-only
            // validation can be added when the domains gain distinct codegen.)
            TypeConstraintNode::ArithmeticDomain(_) => {}
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
