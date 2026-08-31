mod constraint_validation;
mod generic_arguments;
mod open_index_expressions;

use constraint_validation::validate_type_constraints_node;
pub use generic_arguments::validate_exact_const_value_encoding;
pub(crate) use generic_arguments::{
    const_integer_value_fits_primitive, validate_exact_const_identity,
    validate_exact_typed_structured_const_argument,
};
use generic_arguments::{
    machine_argument_name, validate_const_data_argument, validate_generic_argument_bounds,
    validate_machine_data_argument, validate_symbolic_array_length,
};
pub use open_index_expressions::normalize_open_index_expressions;
pub(crate) use open_index_expressions::validate_indexed_qualification_arguments;

use crate::StateSignatureOwner;
use crate::symbols::TopLevelSymbols;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{TypeParameter, TypeParameterKind};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};
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
    OperatorParameter {
        operator: &'program str,
        parameter: &'program str,
        generic_depth: usize,
    },
    OperatorReturn {
        operator: &'program str,
        generic_depth: usize,
    },
    TraitParent {
        trait_name: &'program str,
        parent: &'program str,
        generic_depth: usize,
    },
}

impl TypeReferenceOwner<'_> {
    fn allows_checked_write_only_parameter(&self) -> bool {
        matches!(
            self,
            Self::StateParameter {
                owner: StateSignatureOwner::Machine(_),
                generic_depth: 0,
                ..
            } | Self::StateLocalData {
                generic_depth: 0,
                ..
            }
        )
    }

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
            Self::OperatorParameter {
                operator,
                parameter,
                generic_depth,
            } => Self::OperatorParameter {
                operator,
                parameter,
                generic_depth: generic_depth + 1,
            },
            Self::OperatorReturn {
                operator,
                generic_depth,
            } => Self::OperatorReturn {
                operator,
                generic_depth: generic_depth + 1,
            },
            Self::TraitParent {
                trait_name,
                parent,
                generic_depth,
            } => Self::TraitParent {
                trait_name,
                parent,
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
            Self::OperatorParameter {
                operator,
                parameter,
                generic_depth,
            } => {
                write!(formatter, "operator `{operator}` parameter `{parameter}`")?;
                *generic_depth
            }
            Self::OperatorReturn {
                operator,
                generic_depth,
            } => {
                write!(formatter, "operator `{operator}` return type")?;
                *generic_depth
            }
            Self::TraitParent {
                trait_name,
                parent,
                generic_depth,
            } => {
                write!(
                    formatter,
                    "trait `{trait_name}` parent `{parent}` type argument"
                )?;
                *generic_depth
            }
        };

        for _ in 0..generic_depth {
            formatter.write_str(" generic argument")?;
        }

        Ok(())
    }
}

pub(crate) fn validate_type_reference_handle_with_type_parameters(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
    type_parameters: &[TypeParameter],
    lifetime_parameters: &[psi_typed_trees::name::Identifier],
) {
    // `range-constraints-require-exact-domain`: a range constraint under a
    // non-Exact domain is ill-formed
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
        TypeParameterScope {
            type_parameters,
            lifetime_parameters,
        },
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
        TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } => {
            if *access == psi_language_semantics::ReferenceAccess::WriteOnly
                && !owner.allows_checked_write_only_parameter()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses `&write` outside the checked whole-scalar parameter slice; only an explicit direct-reborrow local is additionally admitted, while write-only fields, returns, operators, traits, and nested generic arguments are not implemented yet"
                )));
            }
            if let Some(lifetime) = lifetime
                && !type_parameter_scope.contains_lifetime(lifetime.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses undeclared lifetime `'{}'; declare it in the owner's generic parameter list",
                    lifetime.as_str()
                )));
            }
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
            validate_type_constraints_node(
                program,
                *base_type,
                *constraints,
                diagnostics,
                owner,
                type_parameter_scope,
            );
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length,
        } => {
            validate_symbolic_array_length(
                program,
                length,
                type_parameter_scope,
                diagnostics,
                owner,
            );
            // Element spelling of `range-constraints-require-exact-domain`
            // (`[u8 [0..=9] in Wrapping; 3]`).
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
            lifetime_arguments,
            arguments,
            ..
        } => {
            if !symbols.has_type(base_name) && !type_parameter_scope.contains(base_name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown generic type `{base_name}`"
                )));
            }

            for lifetime in lifetime_arguments {
                if !type_parameter_scope.contains_lifetime(lifetime.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses undeclared lifetime argument `'{}' in `{base_name}`; declare it in the owner's generic parameter list",
                        lifetime.as_str()
                    )));
                }
            }

            validate_generic_argument_bounds(
                program,
                symbols,
                base_name.as_str(),
                *arguments,
                type_parameter_scope,
                diagnostics,
            );

            let argument_handles = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == base_name.as_str());
            if let Some(definition) = definition {
                if definition.lifetime_parameters.len() != lifetime_arguments.len() {
                    diagnostics.push(Diagnostic::error(format!(
                        "data `{base_name}` expected {} lifetime arguments but got {}",
                        definition.lifetime_parameters.len(),
                        lifetime_arguments.len()
                    )));
                }
                let parameters = program.data_type_parameters(definition);
                // A zero-parameter data declaration can still appear in the
                // `Policy<Schema>` layout-application form. Its argument is a
                // schema, not a data-generic argument.
                if parameters.is_empty() {
                    for argument in argument_handles {
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
                    return;
                }
                if parameters.len() != argument_handles.len() {
                    diagnostics.push(Diagnostic::error(format!(
                        "generic data `{base_name}` expected {} arguments but got {}",
                        parameters.len(),
                        argument_handles.len()
                    )));
                }
                for (parameter, argument) in parameters.iter().zip(argument_handles) {
                    match &parameter.kind {
                        TypeParameterKind::Const { type_reference } => {
                            validate_const_data_argument(
                                program,
                                base_name,
                                parameter,
                                *type_reference,
                                *argument,
                                type_parameter_scope,
                                diagnostics,
                            );
                        }
                        TypeParameterKind::Machine { contract } => {
                            validate_machine_data_argument(
                                program,
                                base_name,
                                parameter,
                                contract,
                                *argument,
                                type_parameter_scope,
                                diagnostics,
                            );
                        }
                        TypeParameterKind::Proposition { .. } => {
                            diagnostics.push(Diagnostic::error(format!(
                                "proposition parameter `{}` of `{base_name}` requires a proposition-family argument, not a type-reference argument",
                                parameter.name
                            )));
                        }
                        TypeParameterKind::Type => {
                            if machine_argument_name(program, *argument, type_parameter_scope)
                                .is_some()
                            {
                                diagnostics.push(Diagnostic::error(format!(
                                    "type parameter `{}` of `{base_name}` was instantiated with a machine symbol; declare it as `<machine {}>` with a `where machine` contract",
                                    parameter.name, parameter.name
                                )));
                                continue;
                            }
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
                }
            } else {
                for argument in argument_handles {
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
        }
        TypeReferenceNode::DynamicTrait {
            name,
            conformance,
            conformance_carrier,
            conformance_name,
            ..
        } => {
            let Some(trait_definition) = symbols.trait_definition(name.as_str()) else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown dynamic trait `{name}`"
                )));
                return;
            };

            if trait_definition.is_boundary {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses boundary trait `{name}` as a local dynamic value; local dynamic descriptors cannot cross a replaceable component boundary"
                )));
            }

            let generic_parameter_count = program.trait_type_parameters(trait_definition).len();
            if generic_parameter_count != 0 {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses generic trait `{name}` as an unbound dynamic value; bind its {generic_parameter_count} generic parameter(s) before deriving a dynamic surface"
                )));
            }

            if conformance_name.is_some() && conformance.is_none() {
                let selection = conformance_carrier
                    .as_ref()
                    .zip(conformance_name.as_ref())
                    .map(|(carrier, conformance)| format!("{carrier}::{conformance}"))
                    .unwrap_or_else(|| "<invalid named conformance>".to_owned());
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} selects unresolved named conformance `{selection}` for dynamic trait `{name}`"
                )));
            }
        }
        TypeReferenceNode::Named { symbol, name } => {
            if type_parameter_scope
                .machine_parameter(*symbol, name.as_str())
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses machine parameter `{name}` as a stored/runtime type; static machine parameters may only index a generic proof-data family or specialize a direct call"
                )));
                return;
            }
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
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown data type `{name}`"
                )));
            }
        }
        TypeReferenceNode::ConstExpression(expression) => {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} uses proof-static index expression `{}` as a runtime type; computed const expressions are valid only inside an indexed-domain argument pack",
                program.expression_table.display_name(*expression)
            )));
        }
        TypeReferenceNode::Unit => {}
    }
}

#[derive(Debug, Clone, Copy)]
struct TypeParameterScope<'program> {
    type_parameters: &'program [TypeParameter],
    lifetime_parameters: &'program [psi_typed_trees::name::Identifier],
}

impl<'program> TypeParameterScope<'program> {
    fn contains(self, name: &str) -> bool {
        self.type_parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == name)
    }

    fn contains_lifetime(self, name: &str) -> bool {
        self.lifetime_parameters
            .iter()
            .any(|parameter| parameter.as_str() == name)
    }

    fn machine_parameter(
        self,
        symbol: psi_symbols::SymbolHandle,
        name: &str,
    ) -> Option<&'program TypeParameter> {
        self.type_parameters.iter().find(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Machine { .. })
                && ((symbol.is_valid() && parameter.symbol == symbol)
                    || parameter.name.as_str() == name)
        })
    }
}

fn type_reference_is_named_str(program: &TypedTrees, type_reference: TypeReferenceHandle) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "string"
    )
}

pub(crate) fn type_references_match(
    program: &TypedTrees,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }

    program.normalized_type_identity(actual) == program.normalized_type_identity(required)
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
