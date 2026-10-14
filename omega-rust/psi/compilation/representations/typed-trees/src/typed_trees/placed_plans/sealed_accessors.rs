//! Exact declaration role of a sealed generated scalar accessor.
//! This establishes nominal/callable identity, never storage or loan authority.
use crate::types::TypeReferenceNode;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;

/// A declaration-authenticated catalog entrance. Its fields are private so
/// schema projection cannot substitute an ordinary authored boundary machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedScalarAccessorDeclaration {
    machine: SymbolHandle,
    state: SymbolHandle,
    view: SymbolHandle,
    schema: SymbolHandle,
    policy: SymbolHandle,
    template: SymbolHandle,
    accessor: SymbolHandle,
    field: SymbolHandle,
    access_slot: u32,
    placement_commitment: [u8; 32],
    operation: PlacedScalarAccessorOperation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlacedScalarAccessorOperation {
    Read,
    Take,
    Write,
}

impl PlacedScalarAccessorDeclaration {
    pub const fn machine(&self) -> SymbolHandle {
        self.machine
    }
    pub const fn state(&self) -> SymbolHandle {
        self.state
    }
    pub const fn view(&self) -> SymbolHandle {
        self.view
    }
    pub const fn schema(&self) -> SymbolHandle {
        self.schema
    }
    pub const fn policy(&self) -> SymbolHandle {
        self.policy
    }
    pub const fn template(&self) -> SymbolHandle {
        self.template
    }
    pub const fn accessor(&self) -> SymbolHandle {
        self.accessor
    }
    pub const fn field(&self) -> SymbolHandle {
        self.field
    }
    pub const fn access_slot(&self) -> u32 {
        self.access_slot
    }
    pub const fn placement_commitment(&self) -> [u8; 32] {
        self.placement_commitment
    }
    pub const fn operation(&self) -> PlacedScalarAccessorOperation {
        self.operation
    }
    /// Rejoin the checked producer's exact generated target and closed P/T
    /// application. Equal names, equal carriers and a target tag are insufficient.
    pub fn authenticate(
        typed: &crate::TypedTrees,
        symbol: SymbolHandle,
    ) -> Result<Option<Self>, Diagnostic> {
        let mut matches = typed.placed_view_plans.iter().flat_map(|view| {
            view.fields.iter().flat_map(move |field| {
                field.accessor_targets.iter().filter_map(move |target| {
                    (target.machine_symbol == symbol || target.state_symbol == symbol)
                        .then_some((view, field, target))
                })
            })
        });
        let Some((view, field, target)) = matches.next() else {
            return Ok(None);
        };
        let invalid = |reason: &str| {
            Diagnostic::error(format!(
                "sealed placed accessor declaration {:?}: {reason}",
                symbol,
            ))
        };
        if matches.next().is_some() {
            return Err(invalid("target is not unique"));
        }
        if !view.template_symbol.is_valid()
            || typed.symbols.symbol_source_origin(view.template_symbol)
                != Some(source::SourceOrigin::Toolchain)
        {
            return Err(invalid(
                "closed view has no exact toolchain template custody",
            ));
        }
        let template_source = typed
            .symbols
            .symbol_source_span(view.template_symbol)
            .and_then(|span| typed.symbols.source_file(span));
        if template_source.is_none_or(|file| {
            file.origin != source::SourceOrigin::Toolchain
                || file.path.strip_prefix(&file.package_root).ok()
                    != Some(std::path::Path::new("layout.omg"))
        }) {
            return Err(invalid(
                "template does not belong to the sealed layout interface",
            ));
        }
        let entry = view
            .placement
            .access()
            .plan()
            .entries()
            .get(field.access_key.slot() as usize)
            .ok_or_else(|| invalid("field access slot is absent"))?;
        if entry.key() != field.access_key || entry.access() != &field.access {
            return Err(invalid(
                "field policy does not rejoin the normalized placement",
            ));
        }
        if typed.primitive_type_reference(field.value_type).is_none() {
            // Another provider catalog owns structural carriers; absence here
            // must not reject a valid declaration that this entry never calls.
            return Ok(None);
        }
        let data = typed
            .data_definition_by_symbol(view.data_symbol)
            .ok_or_else(|| invalid("view definition is absent"))?;
        let origin = data
            .generic_instance
            .ok_or_else(|| invalid("closed P/T application is absent"))?;
        let TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            lifetime_arguments,
            ..
        } = typed.type_reference_table.type_reference(origin)
        else {
            return Err(invalid("view origin is not an application"));
        };
        let operands = typed
            .type_reference_table
            .type_reference_handles(*arguments);
        if *base_symbol != view.template_symbol
            || !lifetime_arguments.is_empty()
            || !matches!(operands, [policy, schema]
                if typed.type_reference_table.type_symbol(*policy) == view.policy_symbol
                    && typed.type_reference_table.type_symbol(*schema) == view.schema_symbol)
        {
            return Err(invalid(
                "view application substituted template, policy or schema",
            ));
        }
        let machine = typed
            .machine(target.machine_symbol)
            .ok_or_else(|| invalid("generated machine is absent"))?;
        let [state] = typed.machine_states(machine) else {
            return Err(invalid("generated machine has no unique entry"));
        };
        if state.symbol != target.state_symbol
            || machine.body_is_present
            || machine.supply_mode != language_semantics::MachineSupplyMode::Boundary
            || !typed.machine_type_parameters(machine).is_empty()
            || !machine.lifetime_parameters.is_empty()
            || machine.suspends
            || machine.blocks
            || machine.service_reach_is_installation_bound
        {
            return Err(invalid(
                "generated boundary declaration does not rejoin its target",
            ));
        }
        let operation = match target.operation.as_str() {
            "read" => PlacedScalarAccessorOperation::Read,
            "take" => PlacedScalarAccessorOperation::Take,
            "write" => PlacedScalarAccessorOperation::Write,
            _ => return Ok(None),
        };
        if !field
            .access
            .operation_names()
            .contains(&target.operation.as_str())
        {
            return Err(invalid(
                "normalized field policy does not admit the operation",
            ));
        }
        let parameters = typed.state_parameters(state);
        let Some(first) = parameters.first() else {
            return Err(invalid("accessor argument is absent"));
        };
        let TypeReferenceNode::Reference {
            referee, access, ..
        } = typed
            .type_reference_table
            .type_reference(first.type_reference)
        else {
            return Err(invalid("accessor argument is not borrowed"));
        };
        if typed.type_reference_table.type_symbol(*referee) != field.accessor_data_symbol
            || *access
                != if operation == PlacedScalarAccessorOperation::Read {
                    language_core::ReferenceAccess::Shared
                } else {
                    language_core::ReferenceAccess::Mutable
                }
        {
            return Err(invalid(
                "accessor carrier or borrow polarity was substituted",
            ));
        }
        if first.is_mutable != access.is_exclusive()
            || parameters.iter().any(|parameter| {
                parameter.is_const
                    || parameter.relevance != language_core::BindingRelevance::Relevant
            })
            || parameters
                .iter()
                .skip(1)
                .any(|parameter| parameter.is_mutable)
        {
            return Err(invalid(
                "scalar parameter modifiers or runtime relevance were substituted",
            ));
        }
        let value_identity = typed.normalized_type_identity(field.value_type);
        let signature_matches = match operation {
            PlacedScalarAccessorOperation::Read | PlacedScalarAccessorOperation::Take => {
                parameters.len() == 1
                    && typed.normalized_type_identity(state.return_type) == value_identity
            }
            PlacedScalarAccessorOperation::Write => {
                parameters.len() == 2
                    && typed.normalized_type_identity(parameters[1].type_reference)
                        == value_identity
                    && !state.return_type.is_valid()
            }
        };
        if !signature_matches {
            return Err(invalid("scalar callable signature was substituted"));
        }
        Ok(Some(Self {
            machine: machine.symbol,
            state: state.symbol,
            view: view.data_symbol,
            schema: view.schema_symbol,
            policy: view.policy_symbol,
            template: view.template_symbol,
            accessor: field.accessor_data_symbol,
            field: field.field_symbol,
            access_slot: field.access_key.slot(),
            placement_commitment: view.placement.content_interpretation().commitment(),
            operation,
        }))
    }
}
