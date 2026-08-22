mod constraint_validation;
mod open_index_expressions;

use constraint_validation::validate_type_constraints_node;
pub use open_index_expressions::normalize_open_index_expressions;
pub(crate) use open_index_expressions::validate_indexed_qualification_arguments;

use crate::StateSignatureOwner;
use crate::symbols::TopLevelSymbols;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::const_value::CanonicalConstValue;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::{
    DataMember, MachineParameterContract, TypeParameter, TypeParameterKind,
};
use psi_typed_trees::types::{
    FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode,
};
use std::collections::HashSet;
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
            referee, lifetime, ..
        } => {
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

fn validate_machine_data_argument(
    program: &TypedTrees,
    base_name: &str,
    parameter: &TypeParameter,
    contract: &MachineParameterContract,
    argument: TypeReferenceHandle,
    _type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "machine parameter `{}` of proof data `{base_name}` requires a static machine symbol argument",
            parameter.name
        )));
        return;
    };

    // Recursive references inside the family (`CauchySeq<S>`) forward the
    // family parameter governed by this exact declaration-site contract.
    if *symbol == parameter.symbol {
        return;
    }

    let generic_types = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == base_name)
        .map(|definition| {
            program
                .data_type_parameters(definition)
                .iter()
                .filter(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // A distinct in-scope machine parameter is admissible only through the
    // same recursive refinement judgment as a concrete symbol. This is the
    // higher-order N7 path (`Family<Inner>` inside a schema parameter): its
    // authored contract, never its name alone, proves compatibility.
    crate::machine_parameters::validate_data_machine_selection(
        program,
        base_name,
        parameter,
        contract,
        *symbol,
        name.as_str(),
        &generic_types,
        diagnostics,
    );
}

fn machine_argument_name<'program>(
    program: &'program TypedTrees,
    argument: TypeReferenceHandle,
    type_parameter_scope: TypeParameterScope<'program>,
) -> Option<&'program str> {
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        return None;
    };
    if type_parameter_scope
        .machine_parameter(*symbol, name.as_str())
        .is_some()
        || program
            .machines()
            .iter()
            .any(|machine| machine.symbol == *symbol || machine.name.as_str() == name.as_str())
    {
        Some(name.as_str())
    } else {
        None
    }
}

fn validate_symbolic_array_length(
    program: &TypedTrees,
    length: &FixedArrayLength,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
) {
    let FixedArrayLength::ConstParameter { symbol, name } = length else {
        return;
    };
    let parameter = type_parameter_scope
        .type_parameters
        .iter()
        .find(|parameter| {
            (symbol.is_valid() && parameter.symbol == *symbol)
                || parameter.name.as_str() == name.as_str()
        });
    let Some(parameter) = parameter else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses unresolved fixed-array length `{name}`; a symbolic array length \
             must name an in-scope `const` parameter"
        )));
        return;
    };
    let TypeParameterKind::Const { type_reference } = parameter.kind else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses type parameter `{name}` as a fixed-array length; array lengths \
             must name a `const` parameter"
        )));
        return;
    };
    let Some(primitive) = program.type_reference_table.primitive_type(type_reference) else {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses const parameter `{name}` with a non-primitive type as a \
             fixed-array length; array lengths require an integer primitive"
        )));
        return;
    };
    if !primitive.accepts_integer_literal() {
        diagnostics.push(Diagnostic::error(format!(
            "{owner} uses const parameter `{name}` of non-integer type `{}` as a \
             fixed-array length",
            primitive.name(),
        )));
    }
}

fn validate_const_data_argument(
    program: &TypedTrees,
    base_name: &str,
    parameter: &TypeParameter,
    parameter_type: TypeReferenceHandle,
    argument: TypeReferenceHandle,
    type_parameter_scope: TypeParameterScope<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let primitive = program.type_reference_table.primitive_type(parameter_type);
    let is_integer_parameter = primitive.is_some_and(PrimitiveType::accepts_integer_literal);
    let TypeReferenceNode::Named { symbol, name } =
        program.type_reference_table.type_reference(argument)
    else {
        diagnostics.push(Diagnostic::error(if is_integer_parameter {
            format!(
                "const parameter `{}` of `{base_name}` requires an integer literal argument",
                parameter.name
            )
        } else {
            format!(
                "const parameter `{}` of `{base_name}` requires a canonical const value",
                parameter.name
            )
        }));
        return;
    };

    if let Some(value) = CanonicalConstValue::from_atom(name.as_str()) {
        if let Err(reason) =
            validate_typed_const_index_type(program, parameter_type, &mut HashSet::new())
        {
            diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has an ineligible index type: {reason}",
                parameter.name
            )));
            return;
        }
        let required_type = type_reference_label(program, parameter_type);
        if value.type_name != required_type {
            diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` has type `{required_type}`, but its canonical value has type `{}`",
                parameter.name, value.type_name
            )));
        }
        return;
    }

    if let Ok(value) = name.as_str().parse::<i128>() {
        let Some(primitive) = primitive.filter(|primitive| primitive.accepts_integer_literal())
        else {
            diagnostics.push(Diagnostic::error(format!(
                "const parameter `{}` of `{base_name}` requires a `{}` value, not integer `{value}`",
                parameter.name,
                type_reference_label(program, parameter_type),
            )));
            return;
        };
        validate_const_integer_range(base_name, parameter, primitive, value, diagnostics);
        return;
    }

    let forwarded = type_parameter_scope
        .type_parameters
        .iter()
        .find(|candidate| {
            if symbol.is_valid() {
                candidate.symbol == *symbol
            } else {
                candidate.name.as_str() == name.as_str()
            }
        });
    let Some(forwarded) = forwarded else {
        diagnostics.push(Diagnostic::error(if is_integer_parameter {
            format!(
                "const parameter `{}` of `{base_name}` requires an integer literal argument or an in-scope const parameter, got `{name}`",
                parameter.name,
            )
        } else {
            format!(
                "const parameter `{}` of `{base_name}` requires a canonical const value or an in-scope const parameter, got `{name}`",
                parameter.name,
            )
        }));
        return;
    };
    let TypeParameterKind::Const {
        type_reference: forwarded_type,
    } = forwarded.kind
    else {
        diagnostics.push(Diagnostic::error(format!(
            "const parameter `{}` of `{base_name}` requires a value, not a type",
            parameter.name
        )));
        return;
    };
    if !type_references_match(program, forwarded_type, parameter_type) {
        diagnostics.push(Diagnostic::error(format!(
            "const parameter `{}` of `{base_name}` has type `{}`, but forwarded const parameter `{}` has type `{}`",
            parameter.name,
            type_reference_label(program, parameter_type),
            forwarded.name,
            type_reference_label(program, forwarded_type),
        )));
    }
}

fn validate_typed_const_index_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
    visiting: &mut HashSet<String>,
) -> Result<(), String> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => {
            if matches!(
                name.as_str(),
                "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64"
                    | "addr"
            ) {
                return Ok(());
            }
            if matches!(name.as_str(), "f32" | "f64" | "string") {
                return Err(format!(
                    "`{name}` does not have canonical structural index identity"
                ));
            }
            if !visiting.insert(name.as_str().to_owned()) {
                return Ok(());
            }
            let definition = program
                .data_definitions()
                .iter()
                .find(|definition| definition.name.as_str() == name.as_str())
                .ok_or_else(|| format!("`{name}` is not declared data"))?;
            if definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque {
                return Err(format!("boundary-opaque data `{name}` is not structural"));
            }
            if definition.quotient.is_some() {
                return Err(format!(
                    "quotient data `{name}` has no compiler-verified canonical representative"
                ));
            }
            if !definition.where_facts.is_empty() {
                return Err(format!(
                    "data `{name}` has default-domain facts whose index-site proof is not implemented"
                ));
            }
            for member in program.data_members(definition) {
                match member {
                    DataMember::Field(field) => validate_typed_const_index_type(
                        program,
                        field.type_reference,
                        visiting,
                    )?,
                    DataMember::Variant(variant) => {
                        for field in program.data_payload_fields(variant) {
                            validate_typed_const_index_type(
                                program,
                                field.type_reference,
                                visiting,
                            )?;
                        }
                    }
                }
            }
            visiting.remove(name.as_str());
            Ok(())
        }
        TypeReferenceNode::FixedArray {
            element_type,
            length: FixedArrayLength::Literal(_),
        } => validate_typed_const_index_type(program, *element_type, visiting),
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_typed_const_index_type(program, *base_type, visiting)
        }
        TypeReferenceNode::Unit => Ok(()),
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::FixedArray { .. } => Err(
            "indices require finite structural values with decidable equality and one canonical form"
                .to_owned(),
        ),
    }
}

fn validate_const_integer_range(
    base_name: &str,
    parameter: &TypeParameter,
    primitive: PrimitiveType,
    value: i128,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (minimum, maximum) = match primitive {
        PrimitiveType::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
        PrimitiveType::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
        PrimitiveType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        PrimitiveType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        PrimitiveType::U8 => (0, i128::from(u8::MAX)),
        PrimitiveType::U16 => (0, i128::from(u16::MAX)),
        PrimitiveType::U32 => (0, i128::from(u32::MAX)),
        PrimitiveType::U64 | PrimitiveType::Addr => (0, i128::from(u64::MAX)),
        _ => return,
    };
    if value < minimum || value > maximum {
        diagnostics.push(Diagnostic::error(format!(
            "const argument `{value}` for `{base_name}::{}` does not fit `{}`",
            parameter.name,
            primitive.name()
        )));
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
    arguments: psi_arena::HandleSpan<TypeReferenceHandle>,
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
    let argument_handles = program
        .type_reference_table
        .type_reference_handles(arguments);
    for (parameter, argument) in parameters.iter().zip(argument_handles) {
        let bounds = crate::properties::declared_property_requirements(&parameter.bounds);
        let bound_labels = bounds.iter().map(ToString::to_string).collect::<Vec<_>>();
        for property in bounds {
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
                "type parameter `{} [{}]` of `{base_name}` was instantiated with `{}`, which does not satisfy `[{property}]`",
                parameter.name,
                bound_labels.join(", "),
                type_reference_label(program, *argument)
            )));
        }
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
