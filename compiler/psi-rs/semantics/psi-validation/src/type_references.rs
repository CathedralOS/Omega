mod open_index_expressions;

pub use open_index_expressions::normalize_open_index_expressions;
use open_index_expressions::validate_indexed_domain_arguments;
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
    FixedArrayLength, PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
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
                && domain.predicate_body == constraint.predicate_body
                && domain.semantic_roles.denotation_dimension.is_some()
                    == constraint.semantic_roles.denotation_dimension.is_some()
                && domain.semantic_roles.arithmetic_policy.is_some()
                    == constraint.semantic_roles.arithmetic_policy.is_some()
                && domain.establishment_routes == constraint.establishment_routes
                && if program.domain_type_parameters(domain).is_empty() {
                    type_references_match(program, base_type, domain.target_type)
                } else {
                    constraint.arguments.len()
                        == program
                            .domain_type_parameters(domain)
                            .len()
                            .saturating_sub(1)
                }
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
