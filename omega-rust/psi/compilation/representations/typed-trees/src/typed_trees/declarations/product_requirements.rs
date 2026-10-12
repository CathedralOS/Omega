//! Original Product requirement telescopes, independent of source admission.
//!
//! The selected interface owns the captured tuple while the original member
//! owns invocation binders. These views inspect retained declarations; they
//! do not select an implementation or admit any static operand.

use crate::TypedTrees;
use crate::expression::StaticMachineArgument;
use crate::name::Identifier;
use crate::static_applications::{
    StaticApplicationDeclaration, StaticApplicationDemand, StaticApplicationView,
    StaticMachineApplication, StaticMachineApplicationHandle, static_application_identity,
};
use crate::trait_definition::AppliedTraitScope;
use symbols::{JoinedPath, SymbolHandle};

pub struct ProductDeclarationTelescope<'program> {
    pub symbol: SymbolHandle,
    pub name: &'program Identifier,
    pub parameters: Vec<&'program crate::data::TypeParameter>,
    pub conformance_bounds: &'program [crate::machine::GenericConformanceBound],
    pub captured_lifetimes: &'program [Identifier],
    pub invocation_lifetimes: &'program [Identifier],
}

pub fn product_requirement_application_declaration<'program>(
    program: &'program TypedTrees,
    application: &StaticMachineApplication,
) -> Option<ProductDeclarationTelescope<'program>> {
    let declaration = StaticApplicationDeclaration::for_application(program, application).ok()?;
    product_requirement_schema(
        program,
        application.binding_declaration,
        application.selected_interface,
        declaration,
    )
}

pub fn product_requirement_schema<'program>(
    program: &'program TypedTrees,
    symbol: SymbolHandle,
    selected_interface: SymbolHandle,
    declaration: StaticApplicationDeclaration<'program>,
) -> Option<ProductDeclarationTelescope<'program>> {
    if let Some(machine) = program.machine(symbol) {
        if machine.supply_mode != language_semantics::MachineSupplyMode::TopLevelRequirement {
            return None;
        }
        return Some(ProductDeclarationTelescope {
            symbol,
            name: &machine.name,
            parameters: declaration.parameters().to_vec(),
            conformance_bounds: declaration.conformance_bounds(),
            captured_lifetimes: &[],
            invocation_lifetimes: &machine.lifetime_parameters,
        });
    }
    let (owner, signature) = program.trait_machine_signature(symbol)?;
    let selected = if selected_interface.is_valid() {
        program.trait_definition_by_symbol(selected_interface)?
    } else {
        owner
    };
    if !owner.is_boundary || !selected.is_boundary {
        return None;
    }
    Some(ProductDeclarationTelescope {
        symbol,
        name: &signature.name,
        parameters: declaration.parameters().to_vec(),
        conformance_bounds: declaration.conformance_bounds(),
        captured_lifetimes: &selected.lifetime_parameters,
        invocation_lifetimes: &signature.lifetime_parameters,
    })
}

/// Rejoin an original non-executing endpoint to its preparation-produced public
/// declaration. Source and realization share this association; neither elects
/// a different interface or treats a generated symbol as an authored telescope.
pub fn prepared_product_requirement_symbol(
    program: &TypedTrees,
    demand: &StaticApplicationDemand,
) -> Result<SymbolHandle, diagnostics::Diagnostic> {
    let rejected = |reason| diagnostics::Diagnostic::error(reason);
    // Retained handles belong to this compilation's cloned declaration frontier;
    // matching source provenance alone cannot establish their arena namespace.
    if program.identity.get() != demand.owner.identity.get() {
        return Err(rejected(
            "assumed requirement application belongs to a different compilation lineage",
        ));
    }
    let symbol = demand.declaration;
    if !symbol.is_valid()
        || !demand
            .owner
            .expression_table
            .static_machine_application_is_valid(demand.application)
    {
        return Err(rejected(
            "assumed requirement lost its original application owner",
        ));
    }
    let application = demand
        .owner
        .expression_table
        .static_machine_application(demand.application);
    let product_schema = product_requirement_application_declaration(program, application)
        .ok_or_else(|| {
            rejected("assumed requirement application lost its checked Product declaration")
        })?;
    let original_schema = product_requirement_application_declaration(&demand.owner, application)
        .ok_or_else(|| {
        rejected("assumed requirement application lost its retained declaration owner")
    })?;
    if application.registration_operation != symbol
        || application.binding_declaration != symbol
        || program.symbols.symbol_provenance_source_span(symbol)
            != demand.owner.symbols.symbol_provenance_source_span(symbol)
        || program.symbols.symbol_package_identity(symbol)
            != demand.owner.symbols.symbol_package_identity(symbol)
        || program.symbols.symbol_source_origin(symbol)
            != demand.owner.symbols.symbol_source_origin(symbol)
        || product_schema
            .parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .ne(original_schema
                .parameters
                .iter()
                .map(|parameter| parameter.symbol))
        || product_schema.captured_lifetimes != original_schema.captured_lifetimes
        || product_schema.invocation_lifetimes != original_schema.invocation_lifetimes
    {
        return Err(rejected(
            "assumed requirement application does not correspond to its original checked Product declaration",
        ));
    }
    let authored_trait = program.trait_machine_signature(symbol);
    let selected_interface = if application.selected_interface.is_valid() {
        application.selected_interface
    } else {
        demand
            .owner
            .trait_machine_signature(symbol)
            .map_or(SymbolHandle::invalid(), |(owner, _)| owner.symbol)
    };
    let needs_instance = authored_trait
        .is_some_and(|(definition, _)| selected_interface != definition.symbol)
        || !product_schema.parameters.is_empty()
        || !product_schema.conformance_bounds.is_empty()
        || (authored_trait.is_some() && !product_schema.captured_lifetimes.is_empty());
    if !needs_instance {
        return Ok(symbol);
    }
    let identity = static_application_identity(&demand.owner, demand.application)?;
    let mut instances = program
        .instantiated_static_applications()
        .filter_map(|(_, row)| {
            (row.demand.declaration == symbol
                && std::sync::Arc::ptr_eq(&row.demand.owner, &demand.owner))
            .then_some(row)
        });
    let mut selected = None;
    for row in &mut instances {
        if static_application_identity(&row.demand.owner, row.demand.application)? != identity {
            continue;
        }
        if selected.is_some() {
            return Err(rejected(
                "assumed requirement application has multiple original demand associations",
            ));
        }
        selected = Some(row.instance);
    }
    let instance = selected.ok_or_else(|| {
        rejected("assumed requirement application needs its original source instance association")
    })?;
    if let Some((definition, _)) = authored_trait {
        let mut headers = program
            .instantiated_requirement_signatures
            .iter()
            .filter(|(_, row)| {
                row.declaring_trait == definition.symbol
                    && row.selected_interface == selected_interface
                    && row.requirement_template == symbol
                    && program.trait_machine_signatures.get(row.signature).symbol == instance
            });
        let (_, row) = headers.next().ok_or_else(|| {
            rejected("assumed requirement application lost its original owned public header")
        })?;
        if headers.next().is_some()
            || !row.binding_application.is_valid()
            || !row.declaring_binding_application.is_valid()
            || !program
                .trait_machine_signatures
                .get(row.signature)
                .type_parameters
                .is_empty()
        {
            return Err(rejected(
                "assumed requirement source header does not own one complete application",
            ));
        }
    } else {
        let mut receipts = program
            .machine_specializations
            .iter()
            .filter(|receipt| receipt.template == symbol && receipt.instance == instance);
        let receipt = receipts.next().ok_or_else(|| {
            rejected("assumed requirement application lost its original source specialization")
        })?;
        if receipts.next().is_some()
            || !receipt.binding_application.is_valid()
            || program
                .machine(instance)
                .is_none_or(|machine| machine.has_specialization_parameters())
        {
            return Err(rejected(
                "assumed requirement source instance does not own one complete specialization",
            ));
        }
    }
    Ok(instance)
}

/// Project a prepared callable to the original declaration retained by its
/// producer. The application owns the complete original telescope; a generated
/// symbol or equal runtime signature cannot stand in for that association.
/// Supplier selection still authenticates the selected row and full tuple.
pub fn original_prepared_requirement_symbol(
    program: &TypedTrees,
    endpoint: SymbolHandle,
) -> Result<SymbolHandle, diagnostics::Diagnostic> {
    let rejected = |reason| diagnostics::Diagnostic::error(reason);
    let mut machines = program
        .machine_specializations
        .iter()
        .filter(|receipt| receipt.instance == endpoint);
    let machine = machines.next();
    let public = program.public_trait_requirement_signature(endpoint);
    if machines.next().is_some() || (machine.is_some() && public.is_some()) {
        return Err(rejected(
            "prepared requirement has competing original producer associations",
        ));
    }
    if let Some(receipt) = machine {
        if program
            .machine(endpoint)
            .is_none_or(|machine| machine.has_specialization_parameters())
        {
            return Err(rejected(
                "prepared requirement lost its complete machine specialization",
            ));
        }
        original_prepared_application(program, receipt.binding_application, receipt.template)?;
        return Ok(receipt.template);
    }
    if let Some(public) = public {
        if !public.application.is_valid() {
            return Ok(public.declaration.symbol);
        }
        original_prepared_application(program, public.application, public.declaration.symbol)?;
        // A demanded inherited header retains both selected and declaring
        // applications. The latter cannot be reconstructed from a receiver or
        // borrowed from a selected provider row.
        if let Some((_, header)) =
            program
                .instantiated_requirement_signatures
                .iter()
                .find(|(_, header)| {
                    program
                        .trait_machine_signatures
                        .get(header.signature)
                        .symbol
                        == endpoint
                })
        {
            original_prepared_application(
                program,
                header.declaring_binding_application,
                public.declaration.symbol,
            )?;
        }
        return Ok(public.declaration.symbol);
    }
    // An ordinary declaration has no application producer to reverse. The
    // declaration owner must still exist; an absent or ambiguous public header
    // cannot silently become its own original source requirement.
    if program
        .instantiated_requirement_signatures
        .iter()
        .any(|(_, header)| {
            program
                .trait_machine_signatures
                .get(header.signature)
                .symbol
                == endpoint
        })
        || program
            .closed_requirement_applications()
            .any(|row| row.signature.symbol == endpoint)
    {
        return Err(rejected(
            "prepared requirement lost its unique original public declaration",
        ));
    }
    let declaration =
        StaticApplicationDeclaration::for_symbol(program, endpoint).map_err(rejected)?;
    Ok(declaration.symbol())
}

fn original_prepared_application(
    program: &TypedTrees,
    application: StaticMachineApplicationHandle,
    declaration: SymbolHandle,
) -> Result<(), diagnostics::Diagnostic> {
    let rejected = |reason| diagnostics::Diagnostic::error(reason);
    if !program
        .expression_table
        .static_machine_application_is_valid(application)
    {
        return Err(rejected(
            "prepared requirement lost its original application payload",
        ));
    }
    let view = StaticApplicationView::new(program, application).map_err(rejected)?;
    if view.declaration().symbol() != declaration
        || view.application().binding_declaration != declaration
        || view.application().registration_operation != declaration
    {
        return Err(rejected(
            "prepared requirement application names a different original declaration",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ProductRequirementSelection {
    pub declaration: SymbolHandle,
    pub selected_interface: SymbolHandle,
}

/// Resolve the authored source selection without forming or admitting its
/// arguments. Inherited members retain the explicit selected interface;
/// visibility and overload refusal are the same for source and Build harvest.
pub fn resolve_product_requirement(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
) -> Option<ProductRequirementSelection> {
    if argument.type_reference.is_valid()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
        || argument.path.is_empty()
    {
        return None;
    }
    let program = program.product_declarations();
    let path = argument.path.joined_path();
    let machine = program.symbols.find_build_subject_declaration_from_source(
        &path,
        occurrence,
        program
            .machines()
            .iter()
            .filter(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
            .map(|machine| (machine.symbol, machine.is_public)),
        &[],
    );
    let mut ambiguous_member = false;
    let member = if argument.path.len() > 1 {
        let interface_path = argument.path[..argument.path.len() - 1].joined_path();
        let interface = program.symbols.find_build_subject_declaration_from_source(
            &interface_path,
            occurrence,
            program
                .traits()
                .iter()
                .filter(|definition| definition.is_boundary)
                .map(|definition| (definition.symbol, definition.is_public)),
            &[],
        );
        interface.and_then(|interface| {
            let definition = program.trait_definition_by_symbol(interface)?;
            let member = argument.path.last()?;
            let scope = AppliedTraitScope::abstract_scope(definition);
            let mut selected = None;
            let mut ambiguous = false;
            scope.visit(program, &mut |scope| {
                if !scope.declaration.is_boundary {
                    return;
                }
                for signature in program
                    .trait_machine_signatures(scope.declaration)
                    .iter()
                    .filter(|signature| signature.name == *member)
                {
                    if selected.is_some_and(|previous| previous != signature.symbol) {
                        ambiguous = true;
                    } else {
                        selected = Some(signature.symbol);
                    }
                }
            });
            if ambiguous {
                ambiguous_member = true;
                None
            } else {
                selected.map(|declaration| ProductRequirementSelection {
                    declaration,
                    selected_interface: interface,
                })
            }
        })
    } else {
        // A short unqualified declaration has its actual declaring interface;
        // there is no selected descendant hidden in this syntax.
        let declaration = program.symbols.find_build_subject_declaration_from_source(
            &path,
            occurrence,
            program
                .traits()
                .iter()
                .filter(|definition| definition.is_boundary)
                .flat_map(|definition| {
                    program
                        .trait_machine_signatures(definition)
                        .iter()
                        .map(move |signature| (signature.symbol, definition.is_public))
                }),
            &[],
        );
        declaration.and_then(|declaration| {
            let (definition, _) = program.trait_machine_signature(declaration)?;
            Some(ProductRequirementSelection {
                declaration,
                selected_interface: definition.symbol,
            })
        })
    };
    if ambiguous_member {
        return None;
    }
    match (machine, member) {
        (Some(declaration), None) => Some(ProductRequirementSelection {
            declaration,
            selected_interface: SymbolHandle::invalid(),
        }),
        (None, member) => member,
        _ => None,
    }
}
