//! Type references: whether a written type is well formed where it appears,
//! and the declared ranges other stages read from a type.
//!
//! The check entrance is `validate_type_reference_handle`.
//! Declaration validators call it with a `TypeReferenceOwner` that names where
//! the type is written: data fields (`value_custody::data`), machine-owned
//! data (`machine_calls::machine_data`), state parameters and returns
//! (`declarations::state_signatures`), local data
//! (`program_validation::statements`), operator parameters and returns
//! (`declarations::operators`), trait parents
//! (`declarations::traits::requirements`) and domain targets
//! (`proof_contracts::domains`). `validate_static_type_argument` runs the same
//! check on a static call's type argument for the typed-to-checked stage's
//! monomorphization, and `validate_generic_instance_argument_bounds` checks a
//! generated data instance's generic arguments for `value_custody::data`.
//! The entrance first checks where a routed service carrier may appear and
//! rejects a range constraint under a non-Exact domain, then walks the type:
//! reference access and lifetimes, constraints (`constraint_validation`),
//! fixed-array lengths and elements, slices, generic applications and their
//! arguments (lengths and arguments through `generic_arguments`), dynamic
//! trait values, and named types. Generic constructor heads must select a
//! declaration: an ordinary type binder is not a higher-kinded constructor.
//! A named binder in a type position must denote a type, not its value carrier:
//! const and runtime value binders remain values even in an unused template.
//! Substituted operands retain their supplying region scope for the entire
//! nested type walk. A child's explicit supplier overrides the inherited one;
//! neither can acquire a same-spelled region from the receiving declaration.
//! `generic_arguments` also exports the exact const-argument checks that
//! checking, package review and the terminal artifact call.
//!
//! The other children are queries. `integer_ranges` and `float_ranges` answer
//! the closed range a declared type carries, for checking, lowering and proof.
//! `open_index_expressions` binds each open index operator in a type
//! constraint to the consuming machine's conformance bound
//! (`normalize_open_index_expressions`, which the typed-to-checked stage runs)
//! and checks indexed-domain arguments for `constraint_validation` and for
//! casts. `type_references_match` and `type_reference_label` compare and
//! display type references for other validators.

mod const_predicates;
mod constraint_validation;
mod generic_arguments;
mod open_index_expressions;

use constraint_validation::validate_type_constraints_node;
pub(crate) use generic_arguments::{
    AppliedValueCarrier, const_argument_matches_parameter, const_integer_value_fits_primitive,
    validate_const_binder_carriers, validate_exact_const_identity,
    validate_exact_typed_structured_const_argument,
};
use generic_arguments::{
    machine_argument_name, validate_const_data_argument, validate_generic_argument_bounds,
    validate_machine_data_argument, validate_symbolic_array_length,
};
pub use generic_arguments::{validate_closed_const_argument, validate_exact_const_value_encoding};
pub(crate) use generic_arguments::{
    validate_const_data_argument as validate_const_application_argument,
    validate_machine_data_argument as validate_machine_application_argument,
    validate_parameter_bounds,
};
pub use open_index_expressions::normalize_open_index_expressions;
mod refinement_predicates;
pub use refinement_predicates::{
    scalar_result_contract_is_represented, scalar_type_refinement_predicates,
    scalar_value_domain_predicate,
};
mod float_ranges;
pub use float_ranges::{closed_float_range_endpoint, ieee_float_range_ordered};
mod integer_ranges;
pub use integer_ranges::{
    closed_integer_range_bound, closed_integer_range_maximum, constrained_range_bounds,
    declared_integer_range, is_arithmetic_policy_only_integer, is_plain_scalar_storage,
    unrestricted_builtin_primitive,
};
pub(crate) use open_index_expressions::validate_indexed_qualification_arguments;

use crate::StateSignatureOwner;
use crate::declarations::symbols::TopLevelSymbols;
use diagnostics::Diagnostic;
use std::fmt;
use typed_trees::data::{TypeParameter, TypeParameterKind};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

/// The declaration whose lexical binders own a static call's type arguments.
/// A call in a field type belongs to the data declaration, never to a machine
/// that happens to construct or receive that data.
#[derive(Debug, Clone, Copy)]
pub enum StaticTypeArgumentOwner<'program> {
    Machine(&'program typed_trees::machine::Machine),
    Data(&'program typed_trees::data::DataDefinition),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TypeReferenceOwner<'program> {
    StaticMachineArgument {
        owner: StaticTypeArgumentOwner<'program>,
        generic_depth: usize,
    },
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
        machine: &'program typed_trees::machine::Machine,
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
    /// The carrier of a `const N: T` or `u: T` binder; `owner` is the
    /// declaring data or machine's display label.
    GenericBinder {
        owner: &'program str,
        parameter: &'program str,
        generic_depth: usize,
    },
}

impl TypeReferenceOwner<'_> {
    fn allows_direct_routed_service_carrier(&self) -> bool {
        matches!(
            self,
            Self::DataField {
                generic_depth: 0,
                ..
            } | Self::StateParameter {
                owner: StateSignatureOwner::Machine(_),
                generic_depth: 0,
                ..
            }
        )
    }

    fn allows_borrowed_routed_service_carrier(&self) -> bool {
        matches!(
            self,
            Self::DataField {
                generic_depth: 0,
                ..
            } | Self::StateParameter {
                owner: StateSignatureOwner::Machine(_),
                generic_depth: 0,
                ..
            } | Self::StateLocalData {
                generic_depth: 0,
                ..
            } | Self::StateReturn {
                owner: StateSignatureOwner::Machine(_),
                generic_depth: 0,
                ..
            }
        )
    }

    fn generic_argument(self) -> Self {
        match self {
            Self::StaticMachineArgument {
                owner,
                generic_depth,
            } => Self::StaticMachineArgument {
                owner,
                generic_depth: generic_depth + 1,
            },
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
            Self::GenericBinder {
                owner,
                parameter,
                generic_depth,
            } => Self::GenericBinder {
                owner,
                parameter,
                generic_depth: generic_depth + 1,
            },
        }
    }
}

impl fmt::Display for TypeReferenceOwner<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let generic_depth = match self {
            Self::StaticMachineArgument {
                owner,
                generic_depth,
            } => {
                match owner {
                    StaticTypeArgumentOwner::Machine(machine) => {
                        write!(formatter, "machine `{}`", machine.name)?;
                    }
                    StaticTypeArgumentOwner::Data(data) => {
                        write!(formatter, "data `{}`", data.name)?;
                    }
                }
                formatter.write_str(" static type argument")?;
                *generic_depth
            }
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
                    "machine `{}` state `{state}` local data `{local}`",
                    machine.name
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
            Self::GenericBinder {
                owner,
                parameter,
                generic_depth,
            } => {
                write!(formatter, "{owner} parameter `{parameter}`")?;
                *generic_depth
            }
        };

        for _ in 0..generic_depth {
            formatter.write_str(" generic argument")?;
        }

        Ok(())
    }
}

/// Check a static type argument before specialization can erase its call site.
/// The callee need not use this type in its body: formation, qualification and
/// lifetime obligations belong to the caller's authored argument itself.
pub fn validate_static_type_argument(
    program: &dyn crate::CheckingInput,
    caller: StaticTypeArgumentOwner<'_>,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let (type_parameters, lifetime_parameters, lexical_owner) = match caller {
        StaticTypeArgumentOwner::Machine(machine) => (
            program.machine_type_parameters(machine),
            machine.lifetime_parameters.as_ref(),
            machine.symbol,
        ),
        StaticTypeArgumentOwner::Data(data) => (
            program.data_type_parameters(data),
            data.lifetime_parameters.as_ref(),
            data.symbol,
        ),
    };
    validate_type_reference_handle(
        program,
        type_reference,
        symbols,
        diagnostics,
        TypeReferenceOwner::StaticMachineArgument {
            owner: caller,
            generic_depth: 0,
        },
        TypeParameterScope {
            operand_lifetime_owner: symbols::SymbolHandle::invalid(),
            lexical_owner,
            type_parameters,
            lifetime_parameters,
        },
    );
}

pub(crate) fn validate_type_reference_handle(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
    type_parameter_scope: TypeParameterScope<'_>,
) {
    validate_routed_service_carrier_placement(
        program,
        type_reference,
        owner,
        type_parameter_scope.lexical_owner,
        true,
        diagnostics,
    );
    // `range-constraints-require-exact-domain`: a range constraint under a
    // non-Exact domain is ill-formed
    // (checked once per declared handle; the accessors walk nested
    // Reference/Constrained spellings transitively).
    crate::proof_contracts::arithmetic_domains::check_range_under_non_exact_domain(
        program,
        type_reference,
        owner,
        diagnostics,
    );
    validate_type_reference_node(
        program,
        type_reference,
        symbols,
        diagnostics,
        owner,
        false,
        type_parameter_scope,
    );
}

/// Find routed Binding carriers recursively rather than relying on the outer
/// type shell. Concrete-machine parameters may own or borrow one exact
/// carrier. Borrowing preserves its identity; it does not establish another
/// binding. Borrowed locals and fields retain the same existing loan. Aggregate/generic
/// nesting, owned locals, owned returns,
/// traits, requirements, and operators retain their implementation fences.
fn validate_routed_service_carrier_placement(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    owner: TypeReferenceOwner<'_>,
    lexical_owner: symbols::SymbolHandle,
    direct: bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A declaration is checked under its own telescope, not under the closed
    // receiver of a future invocation. Concrete carrier consumers separately
    // require transitive closure; this check cannot issue an exact identity.
    match typed_trees::service::inspect_service_parameter_contract(program, type_reference) {
        Err(error) => {
            diagnostics.push(Diagnostic::error(format!(
                "{owner} uses an invalid routed service carrier: {error}"
            )));
            return;
        }
        Ok(Some(parameter)) => {
            if let Err(error) = parameter
                .carrier
                .require_declared_operands(program, lexical_owner)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses an invalid routed service carrier: {error}"
                )));
            }
            let allowed = if parameter.access.is_some() {
                owner.allows_borrowed_routed_service_carrier()
            } else {
                owner.allows_direct_routed_service_carrier()
            };
            if !direct || !allowed {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} places the routed `Binding<R>` carrier outside the first direct field/owned concrete-machine-parameter rung"
                )));
            }
            return;
        }
        Ok(None) => {}
    }

    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            validate_routed_service_carrier_placement(
                program,
                *referee,
                owner,
                lexical_owner,
                false,
                diagnostics,
            );
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            validate_routed_service_carrier_placement(
                program,
                *base_type,
                owner,
                lexical_owner,
                direct,
                diagnostics,
            );
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            validate_routed_service_carrier_placement(
                program,
                *element_type,
                owner,
                lexical_owner,
                false,
                diagnostics,
            );
        }
        TypeReferenceNode::Generic { arguments, .. } => {
            for argument in program
                .type_reference_table
                .type_reference_handles(*arguments)
            {
                validate_routed_service_carrier_placement(
                    program,
                    *argument,
                    owner.generic_argument(),
                    lexical_owner,
                    false,
                    diagnostics,
                );
            }
        }
        TypeReferenceNode::Named { symbol, name } => {
            // A boundary names a requirement, never the value or reference
            // that supplies it. Keep formal checking on the actual Binding
            // carrier instead of letting a bare-trait reference skip custody.
            if owner.allows_borrowed_routed_service_carrier()
                && program
                    .traits()
                    .iter()
                    .any(|definition| definition.symbol == *symbol && definition.is_boundary)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses boundary trait `{name}` as a value type; use `Binding<{name}>` or a borrow of that carrier"
                )));
                return;
            }
            if let Some(origin) =
                crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)
                    .and_then(|definition| definition.generic_instance)
            {
                validate_routed_service_carrier_placement(
                    program,
                    origin,
                    owner.generic_argument(),
                    lexical_owner,
                    false,
                    diagnostics,
                );
            }
        }
        TypeReferenceNode::DynamicTrait { .. }
        | TypeReferenceNode::ConstExpression(_)
        | TypeReferenceNode::Unit => {}
    }
}

pub(crate) fn validate_generic_instance_argument_bounds(
    program: &dyn crate::CheckingInput,
    instance: &typed_trees::data::DataDefinition,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(origin) = instance.generic_instance else {
        return;
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        base_name,
        arguments,
        ..
    } = program.type_reference_table.type_reference(origin)
    else {
        diagnostics.push(Diagnostic::error(format!(
            "generated data instance `{}` has a non-generic derivation origin",
            instance.name
        )));
        return;
    };
    validate_generic_argument_bounds(
        program,
        *base_symbol,
        base_name.as_str(),
        *arguments,
        TypeParameterScope {
            operand_lifetime_owner: symbols::SymbolHandle::invalid(),
            lexical_owner: instance.symbol,
            type_parameters: &[],
            lifetime_parameters: &instance.lifetime_parameters,
        },
        diagnostics,
    );
}

fn validate_type_reference_node(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    owner: TypeReferenceOwner<'_>,
    allow_bare_str: bool,
    mut type_parameter_scope: TypeParameterScope<'_>,
) {
    let supplying_owner = program
        .type_reference_table
        .operand_lifetime_owner(type_reference);
    if supplying_owner.is_valid() {
        // A substituted compound carries its supplier at the root. Children
        // inherit that region scope until a separately substituted operand
        // supplies its own. Keep this distinct from the receiver's type and
        // value telescope: no declaration, loan or access is created here.
        type_parameter_scope.operand_lifetime_owner = supplying_owner;
    }
    // Generic-argument sites below validate every spelled argument under the
    // application owner's custody.
    let validate_generic_argument =
        |argument: TypeReferenceHandle, diagnostics: &mut Vec<Diagnostic>| {
            validate_type_reference_node(
                program,
                argument,
                symbols,
                diagnostics,
                owner.generic_argument(),
                false,
                type_parameter_scope,
            );
        };
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, lifetime, ..
        } => {
            if let Some(lifetime) = lifetime
                && !type_parameter_scope.contains_lifetime(program, lifetime.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses undeclared lifetime `'{}'; declare it in the owner's generic parameter list",
                    lifetime.as_str()
                )));
            }
            validate_type_reference_node(
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
            validate_type_reference_node(
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
            crate::proof_contracts::arithmetic_domains::check_range_under_non_exact_domain(
                program,
                *element_type,
                owner,
                diagnostics,
            );
            validate_type_reference_node(
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
            validate_type_reference_node(
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
            base_symbol,
            base_name,
            lifetime_arguments,
            arguments,
            ..
        } => {
            // Resolution gives lexical binders priority over declarations.
            // Validate that selected identity, rather than accepting any
            // in-scope spelling as a constructor or recapturing a global.
            if type_parameter_scope
                .type_parameters
                .iter()
                .any(|parameter| {
                    matches!(parameter.kind, TypeParameterKind::Type)
                        && ((base_symbol.is_valid() && parameter.symbol == *base_symbol)
                            || (!program.symbols.has_source_metadata()
                                && parameter.name.as_str() == base_name.as_str()))
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} cannot apply generic parameter `{base_name}` as a generic constructor"
                )));
            }
            let base_resolves = symbols.has_type_symbol(*base_symbol)
                || PrimitiveType::from_name(base_name.as_str()).is_some()
                || (!program.symbols.has_source_metadata() && symbols.has_type(base_name));
            if !base_resolves && !type_parameter_scope.contains(base_name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown generic type `{base_name}`"
                )));
            }

            for lifetime in lifetime_arguments {
                if !type_parameter_scope.contains_lifetime(program, lifetime.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} uses undeclared lifetime argument `'{}' in `{base_name}`; declare it in the owner's generic parameter list",
                        lifetime.as_str()
                    )));
                }
            }

            let argument_handles = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let definition = program.data_definitions().iter().find(|definition| {
                definition.symbol == *base_symbol
                    || (!program.symbols.has_source_metadata()
                        && definition.name.as_str() == base_name.as_str())
            });
            if let Some(definition) = definition {
                validate_generic_argument_bounds(
                    program,
                    *base_symbol,
                    base_name.as_str(),
                    *arguments,
                    type_parameter_scope,
                    diagnostics,
                );
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
                        validate_generic_argument(*argument, diagnostics);
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
                        TypeParameterKind::Implementation => {
                            diagnostics.push(Diagnostic::error(format!(
                                "data `{base_name}` declares implementation binder `{}`; implementation parameters belong to trait or machine contracts, not stored data",
                                parameter.name
                            )));
                        }
                        TypeParameterKind::Const { type_reference }
                        | TypeParameterKind::Value { type_reference } => {
                            validate_const_data_argument(
                                program,
                                base_name,
                                parameter,
                                AppliedValueCarrier::applied(
                                    *type_reference,
                                    parameters,
                                    argument_handles,
                                ),
                                *argument,
                                type_parameter_scope,
                                diagnostics,
                            );
                        }
                        TypeParameterKind::Machine { contract } => {
                            validate_machine_data_argument(
                                program,
                                *base_symbol,
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
                            validate_generic_argument(*argument, diagnostics);
                        }
                    }
                }
            } else if let Some(definition) = program.trait_definition_by_symbol(*base_symbol) {
                if definition.lifetime_parameters.len() != lifetime_arguments.len() {
                    diagnostics.push(Diagnostic::error(format!(
                        "trait `{base_name}` expected {} lifetime arguments but got {}",
                        definition.lifetime_parameters.len(),
                        lifetime_arguments.len()
                    )));
                }
                crate::declarations::traits::validate_trait_application_obligations(
                    program,
                    definition,
                    argument_handles,
                    lifetime_arguments,
                    base_name.as_str(),
                    symbols,
                    type_parameter_scope,
                    diagnostics,
                );
            } else {
                for argument in argument_handles {
                    validate_generic_argument(*argument, diagnostics);
                }
            }
        }
        TypeReferenceNode::DynamicTrait {
            symbol,
            name,
            arguments,
        } => {
            let trait_definition = symbols.trait_definition_by_symbol(*symbol).or_else(|| {
                (!program.symbols.has_source_metadata())
                    .then(|| symbols.trait_definition(name.as_str()))
                    .flatten()
            });
            let Some(trait_definition) = trait_definition else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} references unknown dynamic trait `{name}`"
                )));
                return;
            };

            if trait_definition.is_boundary {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses boundary trait `{name}` as a local dynamic value; local dynamic selectors cannot cross a replaceable component boundary"
                )));
            }

            // A selector names one exact trait application: every trait
            // parameter is bound, so each requirement's call shape is concrete.
            let parameter_count = program.trait_type_parameters(trait_definition).len();
            let arguments = program
                .type_reference_table
                .type_reference_handles(*arguments);
            if arguments.len() != parameter_count {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses `dyn {name}` with {} argument(s); `{name}` declares {parameter_count} trait parameter(s)",
                    arguments.len()
                )));
            }
            crate::declarations::traits::validate_trait_application_obligations(
                program,
                trait_definition,
                arguments,
                &[],
                name.as_str(),
                symbols,
                type_parameter_scope,
                diagnostics,
            );
        }
        TypeReferenceNode::Named { symbol, name } => {
            // Name resolution binds the lexical telescope before globals.
            // Scope membership alone does not establish a type: these binders
            // supply values to index/extent positions, never stored types.
            if type_parameter_scope
                .type_parameters
                .iter()
                .any(|parameter| {
                    matches!(
                        parameter.kind,
                        TypeParameterKind::Const { .. } | TypeParameterKind::Value { .. }
                    ) && ((symbol.is_valid() && parameter.symbol == *symbol)
                        || (!program.symbols.has_source_metadata()
                            && parameter.name.as_str() == name.as_str()))
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses value parameter `{name}` as a type; const and runtime value parameters supply values, not types"
                )));
                return;
            }
            if type_parameter_scope
                .machine_parameter(*symbol, name.as_str())
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses machine parameter `{name}` as a stored/runtime type; static machine parameters may only index a generic proof-data family or specialize a direct call"
                )));
                return;
            }
            if symbol.is_valid()
                && matches!(
                    program.symbols.get(*symbol).kind,
                    symbols::SymbolKind::Conformance | symbols::SymbolKind::ConformanceParameter
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses implementation `{name}` as a stored/runtime type; implementations select code, not data"
                )));
                return;
            }
            if name.as_str() == "string" && !allow_bare_str {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} uses unsized text view type `string` by value; use `&string`"
                )));
                return;
            }

            let type_resolves = symbols.has_type_symbol(*symbol)
                || PrimitiveType::from_name(name.as_str()).is_some()
                || (!program.symbols.has_source_metadata() && symbols.has_type(name));
            if !type_resolves && !type_parameter_scope.contains(name.as_str()) {
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
pub(crate) struct TypeParameterScope<'program> {
    /// Nearest retained operand supplier; zero uses the authored telescope.
    pub(crate) operand_lifetime_owner: symbols::SymbolHandle,
    pub(crate) lexical_owner: symbols::SymbolHandle,
    pub(crate) type_parameters: &'program [TypeParameter],
    pub(crate) lifetime_parameters: &'program [typed_trees::name::Identifier],
}

impl<'program> TypeParameterScope<'program> {
    fn contains(self, name: &str) -> bool {
        self.type_parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == name)
    }

    fn contains_lifetime(self, program: &dyn crate::CheckingInput, name: &str) -> bool {
        let supplying_owner = self.operand_lifetime_owner;
        if supplying_owner.is_valid() {
            // Substituted operands retain their issuing declaration. Looking
            // for the spelling in the receiver's telescope would either reject
            // a valid captured region or recapture a same-spelled fresh binder.
            // This establishes declaration well-formedness, not application
            // agreement or a loan; those consume the full retained scopes.
            return typed_trees::type_identity::lifetime_parameter_identity(
                program.typed_program(),
                supplying_owner,
                name,
            )
            .is_some();
        }
        self.lifetime_parameters
            .iter()
            .any(|parameter| parameter.as_str() == name)
    }

    fn machine_parameter(
        self,
        symbol: symbols::SymbolHandle,
        name: &str,
    ) -> Option<&'program TypeParameter> {
        self.type_parameters.iter().find(|parameter| {
            matches!(parameter.kind, TypeParameterKind::Machine { .. })
                && ((symbol.is_valid() && parameter.symbol == symbol)
                    || parameter.name.as_str() == name)
        })
    }
}

fn type_reference_is_named_str(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Named { name, .. } if name.as_str() == "string"
    )
}

pub(crate) fn type_references_match(
    program: &dyn crate::CheckingInput,
    actual: TypeReferenceHandle,
    required: TypeReferenceHandle,
) -> bool {
    if !actual.is_valid() || !required.is_valid() {
        return actual.is_valid() == required.is_valid();
    }
    // One handle renders one identity.
    if actual == required {
        return true;
    }

    program.normalized_type_identity(actual) == program.normalized_type_identity(required)
}

pub(crate) fn type_reference_label(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> String {
    if type_reference.is_valid() {
        program.display_type_reference_with_constraints(type_reference)
    } else {
        "()".to_owned()
    }
}
