//! Exact recognition of the toolchain-owned routed service carrier.
//!
//! A same-named package declaration is ordinary opaque data. Compiler
//! privilege requires the complete core source and declaration shape.
//! `Binding<R>` admits no authored qualification. Its nominal contract and its
//! closed occurrence are different judgments: a field declaration cannot supply
//! a caller's actual regions or enclosing data applications.
//!
//! A declaration's contract may refer to its own formal operands. Inspecting
//! that contract does not admit a closed service occurrence: selection and
//! execution still require `ExactServiceCarrier` and transitive closure. Both
//! judgments decode the same nominal shape, rather than recognizing separate
//! template and concrete spellings.

use crate::TypedTrees;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

pub const SERVICE_CORE_SOURCE: &str = "binding.omg";

/// Nominal contract inspection, without closed-application admission or an
/// executable identity. The supplying telescope belongs to the occurrence.
#[derive(Debug, Clone, Copy)]
pub struct ServiceCarrierContract {
    pub service_data: SymbolHandle,
    pub requirement: SymbolHandle,
    pub requirement_reference: TypeReferenceHandle,
}

impl ServiceCarrierContract {
    pub fn lifetimes(self, program: &TypedTrees) -> &[crate::name::Identifier] {
        match program
            .type_reference_table
            .type_reference(self.requirement_reference)
        {
            TypeReferenceNode::Generic {
                lifetime_arguments, ..
            } => lifetime_arguments,
            _ => &[],
        }
    }

    pub fn arguments(self, program: &TypedTrees) -> &[TypeReferenceHandle] {
        match program
            .type_reference_table
            .type_reference(self.requirement_reference)
        {
            TypeReferenceNode::Generic { arguments, .. } => program
                .type_reference_table
                .type_reference_handles(*arguments),
            _ => &[],
        }
    }

    /// Borrow the complete supplying environment. This constructs a logical
    /// view, not proof that its operands are closed or its provider admitted.
    pub fn application<'program>(
        self,
        program: &'program TypedTrees,
        lexical_owner: SymbolHandle,
        enclosing_data_applications: &[TypeReferenceHandle],
    ) -> Option<crate::trait_definition::AppliedTraitScope<'program>> {
        crate::trait_definition::AppliedTraitScope::new(
            program,
            program.trait_definition_by_symbol(self.requirement)?,
            crate::trait_definition::TraitApplicationArguments {
                lexical_owner,
                enclosing_data_applications,
                arguments: self.arguments(program),
                lifetimes: self.lifetimes(program),
            },
        )
    }

    /// Formal well-formedness preserves the supplying declaration's region
    /// telescope. It neither closes static operands nor selects a provider.
    pub fn require_declared_operands(
        self,
        program: &TypedTrees,
        lexical_owner: SymbolHandle,
    ) -> Result<(), &'static str> {
        program.require_declared_application_operands(self.arguments(program), lexical_owner)?;
        let scope = self
            .application(program, lexical_owner, &[])
            .ok_or("service declaration lost its complete application telescope")?;
        scope
            .application_identity(program)
            .ok_or("service declaration lost its supplying region telescope")?;
        Ok(())
    }
}

/// Nominal parameter presentation. Borrow access is retained independently of
/// the logical contract; this record grants no closed occurrence or erasure.
#[derive(Debug, Clone, Copy)]
pub struct ServiceParameterContract {
    pub carrier: ServiceCarrierContract,
    pub carrier_type: TypeReferenceHandle,
    pub access: Option<language_semantics::ReferenceAccess>,
}

pub fn inspect_service_parameter_contract(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Option<ServiceParameterContract>, String> {
    let (carrier_type, access) = match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference {
            referee, access, ..
        } if matches!(
            access,
            language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable
        ) =>
        {
            (*referee, Some(*access))
        }
        _ => (type_reference, None),
    };
    Ok(
        inspect_service_carrier_contract(program, carrier_type)?.map(|carrier| {
            ServiceParameterContract {
                carrier,
                carrier_type,
                access,
            }
        }),
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ServiceCarrierOccurrence<'scope> {
    pub type_reference: TypeReferenceHandle,
    pub lexical_owner: SymbolHandle,
    pub enclosing_data_applications: &'scope [TypeReferenceHandle],
}

/// Closed static operands issued under one exact supplying occurrence. The
/// local application key retains semantic scope independently of ABI erasure.
#[derive(Debug, Clone, Copy)]
pub struct ExactServiceCarrier {
    pub service_data: SymbolHandle,
    pub requirement: SymbolHandle,
    /// Original exact application, not the enclosing Binding or an ABI view.
    pub requirement_reference: TypeReferenceHandle,
    pub requirement_identity: crate::type_identity::TypeIdentityId,
}

impl PartialEq for ExactServiceCarrier {
    fn eq(&self, other: &Self) -> bool {
        self.service_data == other.service_data
            && self.requirement == other.requirement
            && self.requirement_identity == other.requirement_identity
    }
}

impl Eq for ExactServiceCarrier {}

impl ExactServiceCarrier {
    pub fn arguments<'program>(
        self,
        program: &'program TypedTrees,
    ) -> &'program [TypeReferenceHandle] {
        match program
            .type_reference_table
            .type_reference(self.requirement_reference)
        {
            TypeReferenceNode::Generic { arguments, .. } => program
                .type_reference_table
                .type_reference_handles(*arguments),
            _ => &[],
        }
    }

    /// Inspect the exact carrier beneath its supplying occurrence. Enclosing
    /// records contribute their simultaneous scopes; a bare carrier handle
    /// cannot invent that environment or transfer a selected provider's scope.
    pub fn application<'program>(
        self,
        program: &'program TypedTrees,
        lexical_owner: SymbolHandle,
        enclosing_data_applications: &[TypeReferenceHandle],
    ) -> Option<crate::trait_definition::AppliedTraitScope<'program>> {
        ServiceCarrierContract {
            service_data: self.service_data,
            requirement: self.requirement,
            requirement_reference: self.requirement_reference,
        }
        .application(program, lexical_owner, enclosing_data_applications)
        .filter(|scope| scope.application_identity(program) == Some(self.requirement_identity))
    }
}

/// A parameter's exact underlying binding and its separately retained borrow
/// presentation. Unwrapping this shell is valid for parameter planning, not
/// for deciding whether a value may be moved out of borrowed storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactServiceParameterCarrier {
    pub carrier: ExactServiceCarrier,
    pub carrier_type: TypeReferenceHandle,
    pub access: Option<language_semantics::ReferenceAccess>,
}

pub fn classify_exact_service_parameter_carrier(
    program: &TypedTrees,
    occurrence: ServiceCarrierOccurrence<'_>,
) -> Result<Option<ExactServiceParameterCarrier>, String> {
    let Some(contract) = inspect_service_parameter_contract(program, occurrence.type_reference)?
    else {
        return Ok(None);
    };
    let carrier = close_service_carrier(program, contract.carrier, occurrence)?;
    Ok(Some(ExactServiceParameterCarrier {
        carrier,
        carrier_type: contract.carrier_type,
        access: contract.access,
    }))
}

/// Resolve the boundary requirement carried by one original parameter type.
///
/// Routed bindings retain the requirement in their exact core-owned application;
/// their outer data symbol is not the invoked service. Direct boundary parameters
/// retain their own declaration symbol. This projection grants no provider,
/// invocation ceiling, or installation authority.
pub fn exact_parameter_boundary_requirement(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Option<SymbolHandle>, String> {
    if !type_reference.is_valid() {
        return Ok(None);
    }
    if let Some(parameter) = inspect_service_parameter_contract(program, type_reference)? {
        return Ok(Some(parameter.carrier.requirement));
    }
    let symbol = program
        .type_reference_table
        .type_reference(type_reference)
        .type_symbol(&program.type_reference_table);
    Ok(program
        .traits()
        .iter()
        .any(|definition| definition.is_boundary && definition.symbol == symbol)
        .then_some(symbol))
}

/// Classify one type shell. `Ok(None)` means it is not the exact core
/// `Service` carrier; `Err` means it does name that carrier but violates the
/// deliberately narrow first-rung contract. The carrier is closed: an authored
/// `Constrained` shell over `Binding<R>` is itself a violation, whatever
/// domain or membership the constraint names.
pub fn classify_exact_bound_service_carrier(
    program: &TypedTrees,
    occurrence: ServiceCarrierOccurrence<'_>,
) -> Result<Option<ExactServiceCarrier>, String> {
    let Some(contract) = inspect_service_carrier_contract(program, occurrence.type_reference)?
    else {
        return Ok(None);
    };
    close_service_carrier(program, contract, occurrence).map(Some)
}

fn close_service_carrier(
    program: &TypedTrees,
    contract: ServiceCarrierContract,
    occurrence: ServiceCarrierOccurrence<'_>,
) -> Result<ExactServiceCarrier, String> {
    let scope = contract
        .application(
            program,
            occurrence.lexical_owner,
            occurrence.enclosing_data_applications,
        )
        .ok_or("service occurrence lost its complete supplying telescope")?;
    program
        .require_receiving_trait_operands(&scope)
        .map_err(str::to_owned)?;
    // Closure retains actual regions, not their ABI erasure. Provider selection
    // and receiving installation separately consume the checked correspondence;
    // identifying this occurrence cannot grant either judgment.
    scope
        .portable_application_identity(program)
        .ok_or("closed requirement needs a retained exact region application")?;
    let requirement_identity = scope
        .application_identity(program)
        .ok_or("service occurrence lost its exact application identity")?;
    Ok(ExactServiceCarrier {
        service_data: contract.service_data,
        requirement: contract.requirement,
        requirement_reference: contract.requirement_reference,
        requirement_identity,
    })
}

/// Decode the toolchain-owned contract once for formal checking and concrete
/// admission. Ordinary declaration checking must also establish that every
/// free operand belongs to its lexical telescope. This inspection cannot grant
/// provider/installation correspondence for a region-bearing occurrence.
pub fn inspect_service_carrier_contract(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Option<ServiceCarrierContract>, String> {
    let mut current = type_reference;
    let mut qualified = false;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(current)
    {
        qualified = true;
        current = *base_type;
    }

    let generic_origin = match program.type_reference_table.type_reference(current) {
        TypeReferenceNode::Generic { .. } => current,
        TypeReferenceNode::Named { symbol, .. } => {
            let Some(origin) = program
                .data_definition_by_symbol(*symbol)
                .and_then(|definition| definition.generic_instance)
            else {
                return Ok(None);
            };
            origin
        }
        _ => return Ok(None),
    };
    let TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(generic_origin)
    else {
        return Ok(None);
    };
    if !is_exact_service_data_symbol(program, *base_symbol) {
        return Ok(None);
    }
    if qualified {
        return Err(
            "the core `Binding` carrier is closed; it admits no authored qualification".to_owned(),
        );
    }
    if !lifetime_arguments.is_empty() {
        return Err("the core `Service` carrier takes no lifetime arguments".to_owned());
    }
    let arguments = program
        .type_reference_table
        .type_reference_handles(*arguments);
    let [requirement] = arguments else {
        return Err(format!(
            "the core `Service` carrier requires exactly one closed boundary requirement, but {} arguments were supplied",
            arguments.len()
        ));
    };
    let requirement_reference = *requirement;
    let (requirement, name, arguments, lifetimes) = match program.type_reference_table.type_reference(requirement_reference) {
        TypeReferenceNode::Named { symbol, name } => (*symbol, name, &[][..], &[][..]),
        TypeReferenceNode::Generic { base_symbol, base_name, lifetime_arguments, arguments, .. }
            => (*base_symbol, base_name,
                program.type_reference_table.type_reference_handles(*arguments), lifetime_arguments.as_slice()),
        _ => return Err("the core `Binding` carrier requires one complete closed boundary-trait application with retained exact regions".to_owned()),
    };
    let Some(requirement_definition) = program.trait_definition_by_symbol(requirement) else {
        return Err(format!(
            "`Binding<{name}>` does not name an exact boundary-trait requirement"
        ));
    };
    if !requirement_definition.is_public {
        return Err(format!(
            "`Binding<{name}>` requires a public boundary trait as its stable slot contract"
        ));
    }
    if !requirement_definition.is_boundary {
        return Err(format!(
            "`Binding<{name}>` requires a boundary trait; ordinary traits are local interfaces"
        ));
    }
    if requirement_definition.lifetime_parameters.len() != lifetimes.len()
        || program.trait_type_parameters(requirement_definition).len() != arguments.len()
    {
        return Err(format!(
            "the core `Binding` carrier requires a complete closed boundary application with retained exact regions; `{name}` remains open"
        ));
    }
    Ok(Some(ServiceCarrierContract {
        service_data: *base_symbol,
        requirement,
        requirement_reference,
    }))
}

/// The declared logical requirement, not a closed receiving application.
pub fn service_requirement_declaration(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    inspect_service_carrier_contract(program, type_reference)
        .ok()
        .flatten()
        .map(|carrier| carrier.requirement)
}

pub fn is_exact_service_data_symbol(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    let Some(definition) = program.data_definition_by_symbol(symbol) else {
        return false;
    };
    // One carrier, one source. `ForeignBinding` was listed here as a second
    // row, which is an artifact of the rename that turned `Service<R>` into
    // `Binding<R>`: the name it took over had belonged to a routed-service
    // carrier, but `ForeignBinding` is the compile-time foreign LOCATOR in
    // `core/external_binding.omg` and is not `boundary data` at all, so it
    // could never satisfy the shape checked below.
    let carrier_source = match definition.name.as_str() {
        "Binding" => SERVICE_CORE_SOURCE,
        _ => return false,
    };
    if !exact_toolchain_source(program, symbol, carrier_source) {
        return false;
    }
    let parameters = program.data_type_parameters(definition);
    definition.is_public
        && definition.supply_mode == language_semantics::DataSupplyMode::BoundaryOpaque
        && definition.properties.multiplicity == language_semantics::Multiplicity::Affine
        && definition.properties.carry.is_none()
        && definition.lifetime_parameters.is_empty()
        && matches!(parameters, [parameter] if matches!(parameter.kind, crate::data::TypeParameterKind::Type))
        && program.data_members(definition).is_empty()
}

fn exact_toolchain_source(program: &TypedTrees, symbol: SymbolHandle, relative: &str) -> bool {
    program
        .symbol_toolchain_relative_path(symbol)
        .is_some_and(|path| path == std::path::Path::new(relative))
}
