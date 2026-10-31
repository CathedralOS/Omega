//! Authored static applications retained independently of executable calls.
//!
//! Specialization consumes static operands and retargets execution to a concrete
//! state. That rewrite cannot consume the source edge checking must admit. This
//! carrier holds declaration and operand identities, not an admitted selection
//! or a trust receipt. Both expression and statement calls refer to these rows
//! in the shared expression payload arenas; their zero handle means no retained
//! application. Copies own fresh rows before lexical and type substitution.

use crate::expression::{ExpressionHandle, StaticMachineArgument};
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

pub type StaticMachineApplicationHandle = Handle<StaticMachineApplication>;

/// Derived selection at one authored occurrence. The ordinal rejoins the
/// explicit operand; this is neither callable admission nor a trust receipt.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct StaticConformanceSelection {
    pub operand_ordinal: u32,
    pub application: Option<Box<crate::typed_trees::ClosedConformanceApplication>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct StaticMachineApplication {
    /// The authored operation, possibly a trait requirement, never its private
    /// executable realization or instance.
    pub registration_operation: SymbolHandle,
    /// The declaration whose type/lifetime scope authored these operands.
    /// Folding its type root does not transfer that scope to another caller.
    pub lexical_owner: SymbolHandle,
    /// Diagnostic text only; the symbol above owns declaration identity.
    pub target: Identifier,
    pub arguments: HandleSpan<StaticMachineArgument>,
    /// Original named input coordinates produced while materializing source recipes.
    pub input_selections: HandleSpan<crate::product_operand::ProductInputSelection>,
    /// Occurrence-local machine operands after contextual specialization, in
    /// machine-binder order. Authored operands remain above; closed entries
    /// retain complete tuples through the ordinary specialization roster.
    /// This is derived selection data, not callable admission.
    pub selected_machine_arguments: HandleSpan<StaticMachineArgument>,
    /// Exact inferred operands, including calls with no spelled static list.
    /// These are derivation data; their source bounds still owe admission.
    pub bindings: HandleSpan<StaticMachineBinding>,
    /// Ordinary parameter-position borrow mapping, retained before executable
    /// retargeting. Declaration names on the left belong to the callee;
    /// arguments on the right belong to `lexical_owner`.
    pub lifetime_bindings: HandleSpan<StaticMachineLifetimeBinding>,
    /// Exact receiving declaration of the region mapping above. A public
    /// requirement's regions remain public when executable specialization
    /// independently binds Type/Const/evidence operands to a private realizer.
    /// Nonempty mappings without this owner have no declaration custody.
    pub lifetime_binding_declaration: SymbolHandle,
    /// The original declaration whose binders the derived rows substitute.
    /// A requirement call's authored operation may instead belong to a trait.
    pub binding_declaration: SymbolHandle,
    pub conformance_selection: Option<Box<StaticConformanceSelection>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct StaticMachineLifetimeBinding {
    pub parameter: Identifier,
    pub argument: Identifier,
}

/// One known substitution for a source declaration's binder. Missing bindings
/// are absent rows, not claims that an open tuple satisfies its requirements.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct StaticMachineBinding {
    /// Always the original callee binder, even when a caller body is cloned.
    pub parameter: SymbolHandle,
    pub value: StaticMachineBindingValue,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum StaticMachineBindingValue {
    Type(TypeReferenceHandle),
    Const(TypeReferenceHandle),
    /// Calls retain their exact runtime index subject beside its carrier.
    /// A name subject uses `subject`. A field read or call result uses
    /// `expression` and leaves `subject` zero. Shared instances have neither:
    /// only the carrier is common to every caller.
    RuntimeValue {
        type_reference: TypeReferenceHandle,
        subject: SymbolHandle,
        expression: ExpressionHandle,
    },
    Machine(StaticMachineArgument),
    Conformance(StaticMachineArgument),
}

impl Default for StaticMachineBindingValue {
    fn default() -> Self {
        Self::Type(TypeReferenceHandle::invalid())
    }
}

/// Read one source type leaf under a simultaneous static application. An actual
/// reference belongs to its supplying scope and is returned without applying
/// the receiving substitutions again. Graph readers compose this operation at
/// source leaves; it neither copies a graph nor strips its qualifications.
pub fn project_type_operand(
    program: &crate::TypedTrees,
    reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> TypeReferenceHandle {
    if let crate::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
        && let Some((_, actual)) = substitutions.iter().find(|(formal, _)| formal == symbol)
    {
        return *actual;
    }
    reference
}

impl crate::TypedTrees {
    /// Borrow an occurrence's original region group, or project its own formal
    /// coordinates through the exact requirement/owned-realizer association.
    /// This is tuple correspondence, not conformance or invocation admission.
    /// Captured outer regions remain in the original supplying lexical scope.
    pub fn static_application_lifetimes_for_declaration(
        &self,
        application: StaticMachineApplicationHandle,
        receiving: SymbolHandle,
    ) -> Result<std::borrow::Cow<'_, [StaticMachineLifetimeBinding]>, &'static str> {
        let source = self
            .expression_table
            .static_machine_application(application);
        let bindings = self
            .expression_table
            .static_machine_application_lifetimes(application);
        if bindings.is_empty() {
            return Ok(std::borrow::Cow::Borrowed(bindings));
        }
        let original = self.lifetime_declaration_owner(source.lifetime_binding_declaration);
        let receiving = self.lifetime_declaration_owner(receiving);
        let registered = self.lifetime_declaration_owner(source.registration_operation);
        let registered = if crate::type_identity::declaration_lifetime_parameters(self, registered)
            .is_some()
        {
            registered
        } else if let Some((_, signature)) =
            self.machine_parameter_signature(source.registration_operation)
        {
            self.lifetime_declaration_owner(signature.symbol)
        } else if let Some(machine) = self.machine_holding_state(source.registration_operation) {
            self.lifetime_declaration_owner(machine.symbol)
        } else {
            return Err("static region group lost its original registered callable telescope");
        };
        if original != registered {
            return Err(
                "static region group does not belong to its exact registered callable telescope",
            );
        }
        let original_parameters =
            crate::type_identity::declaration_lifetime_parameters(self, original)
                .ok_or("static region group lost its original receiving declaration")?;
        let receiving_parameters =
            crate::type_identity::declaration_lifetime_parameters(self, receiving)
                .ok_or("static region group lost its requested receiving declaration")?;
        let same_declaration = original == receiving;
        if !same_declaration {
            let corresponds = |implementation, requirement| {
                self.trait_machine_signature(requirement)
                    .is_some_and(|(owner, signature)| {
                        self.owned_conformance_requirement_scope(
                            implementation,
                            owner.symbol,
                            signature.symbol,
                        )
                        .is_some()
                    })
            };
            if !corresponds(receiving, original) && !corresponds(original, receiving) {
                return Err(
                    "static region group has no exact requirement/owned-realizer correspondence",
                );
            }
            if original_parameters.len() != receiving_parameters.len() {
                return Err("requirement and owned realizer have different own region telescopes");
            }
        }
        let mut projected = Vec::new();
        for (ordinal, binding) in bindings.iter().enumerate() {
            let parameter = crate::type_identity::lifetime_parameter_identity(
                self,
                original,
                binding.parameter.as_str(),
            )
            .filter(|parameter| parameter.declaration == original)
            .ok_or("static region binding does not name its original declaration's own formal")?;
            if bindings[..ordinal]
                .iter()
                .any(|previous| previous.parameter == binding.parameter)
            {
                return Err("static region group repeats an original receiving formal");
            }
            crate::type_identity::lifetime_parameter_identity(
                self,
                source.lexical_owner,
                binding.argument.as_str(),
            )
            .ok_or("static region argument has no identity in its original supplying scope")?;
            if !same_declaration {
                projected.push(StaticMachineLifetimeBinding {
                    parameter: receiving_parameters
                        .get(parameter.ordinal)
                        .ok_or("static region formal ordinal exceeds its exact realizer telescope")?
                        .clone(),
                    argument: binding.argument.clone(),
                });
            }
        }
        if same_declaration {
            Ok(std::borrow::Cow::Borrowed(bindings))
        } else {
            Ok(std::borrow::Cow::Owned(projected))
        }
    }
}
