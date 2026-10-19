//! Source selection for compiler-supplied placed operations. The public generic
//! declaration names an operation, never a blanket trait implementation. Its
//! closed application selects an already-installed field target; no second
//! provider demand or memory-access implementation is manufactured here.

use diagnostics::Diagnostic;
use language_core::ReferenceAccess;
use language_semantics::MachineSupplyMode;
use symbols::SymbolHandle;

use crate::data::TypeParameterKind;
use crate::typed_trees::{PlacedAccessorTarget, PlacedScalarAccessorDeclaration};
use crate::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedAccessPrimitive {
    Read,
    Write,
}

impl PlacedAccessPrimitive {
    pub const fn operation(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
        }
    }

    /// Classification requires the sealed toolchain file and its exact generic
    /// signature. The spelling alone never gives an authored machine authority.
    pub fn from_declaration(
        program: &crate::TypedTrees,
        symbol: SymbolHandle,
    ) -> Result<Option<Self>, Diagnostic> {
        let Some(machine) = program.machine(symbol) else {
            return Ok(None);
        };
        let operation = match machine.name.as_str().rsplit("::").next() {
            Some("placed_read") => Self::Read,
            Some("placed_write") => Self::Write,
            _ => return Ok(None),
        };
        let source = program
            .symbols
            .symbol_source_span(machine.symbol)
            .and_then(|span| program.symbols.source_file(span));
        if source.is_none_or(|file| {
            file.origin != source::SourceOrigin::Toolchain
                || file.path.strip_prefix(&file.package_root).ok()
                    != Some(std::path::Path::new("layout.omg"))
        }) {
            return Ok(None);
        }
        let invalid = || {
            Diagnostic::error(format!(
                "sealed placed {} primitive has an invalid declaration contract",
                operation.operation(),
            ))
        };
        let [accessor, value] = program.machine_type_parameters(machine) else {
            return Err(invalid());
        };
        let [state] = program.machine_states(machine) else {
            return Err(invalid());
        };
        if machine.body_is_present
            || machine.supply_mode != MachineSupplyMode::Boundary
            || !machine.lifetime_parameters.is_empty()
            || !machine.conformance_bounds.is_empty()
            || machine.suspends
            || machine.blocks
            || machine.service_reach_is_installation_bound
            || !machine.contracts.is_empty()
            || !state.contracts.is_empty()
            || !machine.invokes.is_empty()
            || !machine.owned_data.is_empty()
            || machine.admitted_same_stack_bytes != 0
            || machine.target.is_some()
            || machine.authored_target.is_some()
            || machine.attached_data_symbol.is_valid()
            || machine.spelling.is_some()
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty()
            || accessor.kind != TypeParameterKind::Type
            || value.kind != TypeParameterKind::Type
            || accessor.bounds != Default::default()
            || value.bounds != Default::default()
        {
            return Err(invalid());
        }
        let parameters = program.state_parameters(state);
        let Some(field) = parameters.first() else {
            return Err(invalid());
        };
        let TypeReferenceNode::Reference {
            referee,
            access,
            lifetime,
        } = program
            .type_reference_table
            .type_reference(field.type_reference)
        else {
            return Err(invalid());
        };
        let expected_access = if operation == Self::Read {
            ReferenceAccess::Shared
        } else {
            ReferenceAccess::Mutable
        };
        let names_parameter = |reference, parameter| {
            matches!(program.type_reference_table.type_reference(reference),
                TypeReferenceNode::Named { symbol, .. } if *symbol == parameter)
        };
        if !names_parameter(*referee, accessor.symbol)
            || *access != expected_access
            || lifetime.is_some()
            || field.is_mutable != expected_access.is_exclusive()
            || parameters.iter().any(|parameter| {
                parameter.is_const
                    || parameter.relevance != language_core::BindingRelevance::Relevant
            })
        {
            return Err(invalid());
        }
        let signature_matches = match operation {
            Self::Read => parameters.len() == 1 && names_parameter(state.return_type, value.symbol),
            Self::Write => {
                parameters.len() == 2
                    && !parameters[1].is_mutable
                    && names_parameter(parameters[1].type_reference, value.symbol)
                    && matches!(
                        program
                            .type_reference_table
                            .type_reference(state.return_type),
                        TypeReferenceNode::Unit
                    )
            }
        };
        if !signature_matches {
            return Err(invalid());
        }
        let [conformance] = program.machine_trait_conformances(machine) else {
            return Err(invalid());
        };
        let expected_trait = match operation {
            Self::Read => "Readable",
            Self::Write => "Writable",
        };
        let trait_source = program
            .symbols
            .symbol_source_span(conformance.symbol)
            .and_then(|span| program.symbols.source_file(span));
        let arguments = program
            .type_reference_table
            .type_reference_handles(conformance.arguments);
        if program
            .trait_definition_by_symbol(conformance.symbol)
            .is_none()
            || trait_source != source
            || conformance.name.as_str().rsplit("::").next() != Some(expected_trait)
            || conformance.requirement.as_ref().map(|name| name.as_str())
                != Some(operation.operation())
            || conformance.alias.is_some()
            || conformance.external_binding.is_some()
            || conformance.via_expression.is_valid()
            || !matches!(arguments, [first, second]
                if names_parameter(*first, accessor.symbol) && names_parameter(*second, value.symbol))
        {
            return Err(invalid());
        }
        Ok(Some(operation))
    }

    /// Resolve only through the producer-retained accessor identity. This
    /// establishes callable selection, not occurrence, loan or range authority;
    /// those remain obligations on the ordinary selected field operation.
    pub fn select_target(
        self,
        program: &crate::TypedTrees,
        accessor: TypeReferenceHandle,
        value: TypeReferenceHandle,
    ) -> Result<PlacedAccessorTarget, Diagnostic> {
        let invalid = |reason: &str| {
            Diagnostic::error(format!("placed {} application: {reason}", self.operation()))
        };
        let accessor_symbol = program.type_reference_table.type_symbol(accessor);
        let mut fields = program.placed_view_plans.iter().flat_map(|view| {
            view.fields
                .iter()
                .filter(|field| field.accessor_data_symbol == accessor_symbol)
        });
        let Some(field) = fields.next() else {
            return Err(invalid(
                "requires an exact compiler-issued placed field accessor; an ordinary or unconstrained generic type grants no operation",
            ));
        };
        if fields.next().is_some() {
            return Err(invalid("accessor identity belongs to more than one field"));
        }
        // A wrapper containing an accessor is not that accessor. Nominal-head
        // lookup above locates the plan, but cannot establish the operand type.
        if program.normalized_type_identity(field.accessor_type)
            != program.normalized_type_identity(accessor)
        {
            return Err(invalid(
                "requires the exact accessor type, not a reference or collection of accessors",
            ));
        }
        if program.normalized_type_identity(field.value_type)
            != program.normalized_type_identity(value)
        {
            return Err(invalid(
                "value type does not match the placed field's exact value type",
            ));
        }
        let target = field
            .accessor_targets
            .iter()
            .find(|target| target.operation == self.operation())
            .ok_or_else(|| invalid("the placement plan does not permit this operation"))?;
        if PlacedScalarAccessorDeclaration::authenticate(program, target.state_symbol)?.is_none() {
            return Err(invalid(
                "the selected field has no admitted scalar operation declaration",
            ));
        }
        Ok(target.clone())
    }
}
