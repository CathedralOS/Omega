//! The retired routed-service carrier `Binding<R>`, kept as an explicit
//! negative judgment while its remaining consumers are removed.
//!
//! [Component publication](../../../../../../../../wiki/spec/build/component_publication.md)
//! retires source-visible `Binding<R>` values: "A boundary trait is not a data
//! carrier, provider object, capability, or source-visible vtable, and no hidden
//! binding argument is inserted into requirement signatures." An exact closed
//! boundary-trait application is itself the static provider slot, and source
//! calls its requirements directly (`Console::write(bytes)`). The carrier's
//! declaring file `source/library/core/binding.omg` was deleted with
//! `tests/omega/fail/calls/retired_core_binding_*` asserting the bare name is
//! unknown, so no program can reach the recognition this module used to perform:
//! recognition required the exact toolchain source `core/binding.omg`, which no
//! longer exists. `STATIC-MACHINE-MIGRATION` (TASKS.md) owns deleting what
//! remains: the call sites whose carrier branch can no longer fire
//! (`declarations/trait_definition/applications/source_fields.rs`,
//! `declarations/invocations.rs`,
//! `04_typed-trees-to-checked-trees/.../shape_collector.rs`,
//! `checking/.../boundary_calls.rs`, `checking/.../value_custody/type_references.rs`)
//! and the selected-dispatch
//! `service_custody` traversal with `program_entry_service_requirements`, whose
//! only constructor of `ExactServiceCarrier` was this recognition.

use crate::TypedTrees;
use crate::types::{TypeReferenceHandle, TypeReferenceNode};
use symbols::SymbolHandle;

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
    // The carrier is retired, so no parameter can name it; see the module header.
    let _ = (program, type_reference);
    Ok(None)
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

/// Classify one parameter shell. `Ok(None)` always: the carrier is retired
/// (module header), so no parameter names it. Kept while
/// `STATIC-MACHINE-MIGRATION` removes the callers.
pub fn classify_exact_service_parameter_carrier(
    program: &TypedTrees,
    occurrence: ServiceCarrierOccurrence<'_>,
) -> Result<Option<ExactServiceParameterCarrier>, String> {
    let _ = (program, occurrence);
    Ok(None)
}

/// Classify one type shell. `Ok(None)` always: the exact core carrier this
/// used to decode (`Binding<R>` in `core/binding.omg`) is retired and its
/// declaring file no longer exists (module header). Kept while
/// `STATIC-MACHINE-MIGRATION` removes the callers.
pub fn classify_exact_bound_service_carrier(
    program: &TypedTrees,
    occurrence: ServiceCarrierOccurrence<'_>,
) -> Result<Option<ExactServiceCarrier>, String> {
    let _ = (program, occurrence);
    Ok(None)
}

/// Decode the toolchain-owned contract. `Ok(None)` always: the carrier is
/// retired, so nothing names it. Resolution, checking and provider planning
/// see the same negative answer the deleted `core/binding.omg` produced.
pub fn inspect_service_carrier_contract(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Result<Option<ServiceCarrierContract>, String> {
    let _ = (program, type_reference);
    Ok(None)
}

/// The declared logical requirement, not a closed receiving application.
/// `None` always, for the reason above.
pub fn service_requirement_declaration(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let _ = (program, type_reference);
    None
}
