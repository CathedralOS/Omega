//! Declaration identity for the private whole-value placement primitives.
//!
//! The checked provider performs the ordinary ResourceProfileSupply call. These
//! leaves own encoding/extraction, not profile selection. Recognizing one only
//! authorizes that operation's static contract: execution must still bind the
//! completed profile call, exact live claim and actual provider backing before
//! it can publish Resident or Vacant. A same-named source machine is not a leaf.

use super::PlacementLifecycleOperation;
use diagnostics::Diagnostic;
use language_semantics::MachineSupplyMode;

const LAYOUT_SOURCE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../source/library/core/layout.omg"
));

/// Source-authenticated operation. Instantiation retains its exact template
/// edge; no consumer infers the template by trimming a specialized name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlacementExecutionDeclaration {
    operation: PlacementLifecycleOperation,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    template: symbols::SymbolHandle,
}

impl PlacementExecutionDeclaration {
    pub const fn operation(&self) -> PlacementLifecycleOperation {
        self.operation
    }

    pub const fn machine(&self) -> symbols::SymbolHandle {
        self.machine
    }

    pub const fn state(&self) -> symbols::SymbolHandle {
        self.state
    }

    pub const fn template(&self) -> symbols::SymbolHandle {
        self.template
    }

    /// The P/T of this invocation, not the representative caller cached by a
    /// shared specialization. Declaration correspondence does not transfer the
    /// representative caller's policy-selection authority.
    pub fn invocation_operands(
        &self,
        program: &crate::TypedTrees,
        owner: symbols::SymbolHandle,
        application: crate::static_applications::StaticMachineApplicationHandle,
    ) -> Result<
        (
            crate::types::TypeReferenceHandle,
            crate::types::TypeReferenceHandle,
        ),
        Diagnostic,
    > {
        if !application.is_valid() {
            return Err(Diagnostic::error(
                "placement execution lost its retained invocation application",
            ));
        }
        let source = program
            .expression_table
            .static_machine_application(application);
        if source.lexical_owner != owner || source.binding_declaration != self.template {
            return Err(Diagnostic::error(
                "placement execution application does not belong to this caller and declaration",
            ));
        }
        let specialization = program
            .machine_specialization(self.machine)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "placement execution `{}` in caller `{}` lost its closed machine application",
                    program.symbols.name(self.machine),
                    program.symbols.name(owner),
                ))
            })?;
        let [policy, schema] = program
            .data_type_parameters
            .span_or_empty(specialization.template_parameters)
        else {
            return Err(Diagnostic::error(
                "placement execution lost its original policy/schema binders",
            ));
        };
        let bindings = program
            .expression_table
            .static_machine_application_bindings(application);
        let operand = |parameter| {
            let mut matching = bindings
                .iter()
                .filter(|binding| binding.parameter == parameter);
            match (matching.next(), matching.next()) {
                (
                    Some(crate::static_applications::StaticMachineBinding {
                        value:
                            crate::static_applications::StaticMachineBindingValue::Type(reference),
                        ..
                    }),
                    None,
                ) => Ok(*reference),
                _ => Err(Diagnostic::error(
                    "placement execution lost an exact invocation-owned type operand",
                )),
            }
        };
        Ok((operand(policy.symbol)?, operand(schema.symbol)?))
    }

    pub fn authenticate(
        program: &crate::TypedTrees,
        symbol: symbols::SymbolHandle,
    ) -> Result<Option<Self>, Diagnostic> {
        // Attached calls name the entry state; free calls name the machine.
        // Both identify one declaration, but the caller retains its actual
        // target as the coordinate of invocation evidence.
        let Some(machine) = program
            .machine(symbol)
            .or_else(|| program.machine_holding_state(symbol))
        else {
            return Ok(None);
        };
        let declaration = program
            .machine_specialization(machine.symbol)
            .map_or(machine.symbol, |application| application.template);
        let Some(source) = program
            .symbols
            .symbol_source_span(declaration)
            .and_then(|span| program.symbols.source_file(span))
        else {
            return Ok(None);
        };
        if source.origin != source::SourceOrigin::Toolchain
            || source.package_identity.is_some()
            || source.path.strip_prefix(&source.package_root).ok()
                != Some(std::path::Path::new("layout.omg"))
            || source.source.as_bytes() != LAYOUT_SOURCE
        {
            return Ok(None);
        }
        let operation = match program
            .normalized_hermetic_symbol_identity(declaration)
            .ok()
            .as_deref()
        {
            Some("toolchain::Placed::initialize_from_profile") => {
                PlacementLifecycleOperation::Initialize
            }
            Some("toolchain::Placed::extract_resident") => PlacementLifecycleOperation::MoveOut,
            Some(
                "toolchain::Placed::view_from_profile" | "toolchain::Placed::view_mut_from_profile",
            ) => PlacementLifecycleOperation::View,
            _ => return Ok(None),
        };
        let malformed = |reason| {
            Diagnostic::error(format!(
                "sealed placement execution `{}` {reason}",
                machine.name
            ))
        };
        if machine.body_is_present
            || machine.supply_mode != MachineSupplyMode::Boundary
            || machine.lifetime_parameters.len()
                != usize::from(operation == PlacementLifecycleOperation::View)
            || machine.spelling.is_some()
            || machine.suspends
            || machine.blocks
            || machine.target.is_some()
            || machine.authored_target.is_some()
            || machine.target_family_interface
            || !program.machine_trait_conformances(machine).is_empty()
            || !machine.invokes.is_empty()
            || machine.service_reach_is_installation_bound
        {
            return Err(malformed("lost its canonical declaration envelope"));
        }
        let [state] = program.machine_states(machine) else {
            return Err(malformed("must retain one entry state"));
        };
        if symbol != machine.symbol && symbol != state.symbol {
            return Err(malformed("call target is not its entry state"));
        }
        let parameters = program.state_parameters(state);
        let expected = match operation {
            PlacementLifecycleOperation::Initialize => 3,
            PlacementLifecycleOperation::MoveOut | PlacementLifecycleOperation::View => 2,
        };
        if parameters.len() != expected
            || parameters.iter().any(|parameter| {
                // Mutable-reference parameters are marked mutable by the
                // parser. Their access is part of the authenticated signature,
                // not a mutable owned operand smuggled into the leaf.
                parameter.is_const || parameter.relevance.is_erased()
            })
        {
            return Err(malformed("lost its complete operation parameter roster"));
        }
        Ok(Some(Self {
            operation,
            machine: machine.symbol,
            state: state.symbol,
            template: declaration,
        }))
    }
}
