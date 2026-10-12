//! Authored static applications retained independently of executable calls.
//!
//! Specialization consumes static operands and retargets execution to a concrete
//! state. That rewrite cannot consume the source edge checking must admit. This
//! carrier holds declaration and operand identities, not an admitted selection
//! or a trust receipt. Both expression and statement calls refer to these rows
//! in the shared expression payload arenas; their zero handle means no retained
//! application. Copies own fresh rows before lexical and type substitution.

mod arguments;
mod declaration_view;
mod identity;
mod interface_identity;
mod interfaces;

pub use arguments::{
    static_argument_from_type_reference, static_const_literal_from_type_reference,
};

pub use declaration_view::{
    DeclaredStaticLifetime, StaticApplicationDeclaration, StaticApplicationView,
};

pub use identity::{
    StaticApplicationIdentity, StaticApplicationRegionIdentity,
    requirement_interface_application_identity, static_application_identities_equal,
    static_application_identity, static_binding_identity,
};
pub use interface_identity::{
    requirement_call_interface_identity, requirement_provider_application_identity,
    trait_type_application_identity,
};
pub use interfaces::{boundary_interface_members, original_requirement_is_complete};

use crate::expression::StaticMachineArgument;
use crate::name::Identifier;
use crate::types::TypeReferenceHandle;
use arena::{Handle, HandleSpan};
use symbols::SymbolHandle;

pub type StaticMachineApplicationHandle = Handle<StaticMachineApplication>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticMachineApplication {
    /// The authored operation, possibly a trait requirement, never its private
    /// executable realization or instance.
    pub registration_operation: SymbolHandle,
    /// Exact authored interface through which this requirement was selected.
    /// It may own Calling while an inherited requirement belongs to a parent.
    /// Zero denotes an ordinary direct declaring interface, never an inferred
    /// descendant or permission to search for a policy owner.
    pub selected_interface: SymbolHandle,
    /// The declaration whose type/lifetime scope authored these operands.
    /// Folding its type root does not transfer that scope to another caller.
    pub lexical_owner: SymbolHandle,
    /// Original captured lexical declaration, when operands outlive Build
    /// roots. The handle belongs to the containing TypedTrees scope arena;
    /// zero retains an ordinary live declaration context.
    pub captured_lexical_scope:
        Handle<crate::typed_trees::product_applications::CapturedStaticLexicalScopeSlot>,
    /// Diagnostic text only; the symbol above owns declaration identity.
    pub target: Identifier,
    pub arguments: HandleSpan<StaticMachineArgument>,
    /// Exact original receiving binders when a producer derives outer operands
    /// from a receiver and retains only the member's authored static list.
    /// An empty span keeps the original declaration-order source telescope.
    /// These handles belong to the callee, never the copied lexical operands.
    pub argument_parameters: HandleSpan<SymbolHandle>,
    /// Actual outer-to-inner data applications on the supplying receiver place.
    /// This span belongs to the containing type arena; it is not a shared
    /// storage declaration's representative tuple or a provider selection.
    pub enclosing_data_applications: HandleSpan<TypeReferenceHandle>,
    /// Exact ordinary data formal on binding_declaration that issued the
    /// receiving graph. Zero denotes no unique declaration-owned data operand.
    /// This callee symbol survives caller copying without lexical remapping.
    pub receiving_data_parameter: SymbolHandle,
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
    /// The original declaration whose binders the derived rows substitute.
    /// A requirement call's authored operation may instead belong to a trait.
    pub binding_declaration: SymbolHandle,
    /// Result carrier after this original public tuple is substituted in its
    /// caller's scope. A symbolic tuple can determine a result without owning
    /// an executable header. Copies start absent and derive their own view.
    pub public_result_type: TypeReferenceHandle,
    /// Complete non-executing public contract view owned by this occurrence.
    /// Its source symbols remain declaration identities; copied payloads are
    /// unavailable after any tuple/capture mutation or source occurrence copy.
    pub(crate) public_contract:
        Handle<crate::typed_trees::callable_applications::AppliedCallableSignature>,
}

impl StaticMachineApplication {
    pub fn type_identity_owner<'owner>(
        &self,
        program: &'owner crate::TypedTrees,
    ) -> Option<crate::type_identity::AppliedTypeIdentityOwner<'owner>> {
        use crate::type_identity::AppliedTypeIdentityOwner;
        if self.captured_lexical_scope.is_valid() {
            let capture = program.captured_static_lexical_scope(self.captured_lexical_scope)?;
            (capture.symbol() == self.lexical_owner)
                .then_some(AppliedTypeIdentityOwner::Captured(capture))
        } else {
            Some(AppliedTypeIdentityOwner::Declaration(self.lexical_owner))
        }
    }
}

/// A supplied invocation region is either an original named lexical binder
/// or an actual anonymous reference region. Absence of a binding row remains
/// missing correspondence; Anonymous never names a declaration or a referent.
/// Exact loans and result/source paths belong to ordinary call checking.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum StaticMachineLifetimeArgument {
    Named(Identifier),
    #[default]
    Anonymous,
}

impl From<Identifier> for StaticMachineLifetimeArgument {
    fn from(argument: Identifier) -> Self {
        Self::Named(argument)
    }
}

impl StaticMachineLifetimeArgument {
    pub fn named(&self) -> Option<&Identifier> {
        match self {
            Self::Named(argument) => Some(argument),
            Self::Anonymous => None,
        }
    }

    pub fn inferred(argument: Option<&Identifier>) -> Self {
        argument.cloned().map_or(Self::Anonymous, Self::Named)
    }

    /// A present None substitutes an actual anonymous region. No tuple entry
    /// still means missing correspondence, independently of this value class.
    pub fn scope_argument(&self) -> Option<String> {
        self.named().map(|argument| argument.as_str().to_owned())
    }
}

/// Shared read of authored named operands and retained invocation operands.
/// A source Identifier always remains named, including malformed spellings.
pub trait StaticLifetimeOperand {
    fn named(&self) -> Option<&Identifier>;
}

impl StaticLifetimeOperand for Identifier {
    fn named(&self) -> Option<&Identifier> {
        Some(self)
    }
}

impl StaticLifetimeOperand for StaticMachineLifetimeArgument {
    fn named(&self) -> Option<&Identifier> {
        self.named()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticMachineLifetimeBinding {
    pub parameter: Identifier,
    pub argument: StaticMachineLifetimeArgument,
}

/// One known substitution for a source declaration's binder. Missing bindings
/// are absent rows, not claims that an open tuple satisfies its requirements.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StaticMachineBinding {
    /// Always the original callee binder, even when a caller body is cloned.
    pub parameter: SymbolHandle,
    pub value: StaticMachineBindingValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticMachineBindingValue {
    Type(TypeReferenceHandle),
    Const(TypeReferenceHandle),
    /// Calls retain their exact runtime index subject beside its carrier.
    /// Shared instances have no caller subject (zero), only the carrier.
    RuntimeValue {
        type_reference: TypeReferenceHandle,
        subject: SymbolHandle,
    },
    Machine(StaticMachineArgument),
    Conformance(StaticMachineArgument),
}

impl Default for StaticMachineBindingValue {
    fn default() -> Self {
        Self::Type(TypeReferenceHandle::invalid())
    }
}

impl StaticMachineBinding {
    pub fn remap_symbols(&mut self, symbols: &[(SymbolHandle, SymbolHandle)]) {
        // The binder belongs to binding_declaration, not lexical_owner.
        // Remapping it would turn recursive calls into bindings for a private
        // caller's erased telescope. Actual operands do follow caller cloning.
        match &mut self.value {
            StaticMachineBindingValue::Machine(argument)
            | StaticMachineBindingValue::Conformance(argument) => argument.remap_symbols(symbols),
            StaticMachineBindingValue::RuntimeValue { subject, .. } => {
                if let Some((_, replacement)) =
                    symbols.iter().find(|(source, _)| *source == *subject)
                {
                    *subject = *replacement;
                }
            }
            StaticMachineBindingValue::Type(_) | StaticMachineBindingValue::Const(_) => {}
        }
    }
}

/// A non-executing demand whose payload handles retain their original owner.
/// Source preparation may instantiate this declaration; this record provides
/// neither bound admission nor implementation, provider or trust authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticApplicationDemand {
    pub declaration: SymbolHandle,
    pub owner: std::sync::Arc<crate::FrozenTypedTrees>,
    pub application: StaticMachineApplicationHandle,
}

/// One executed, non-executing Product relationship request. Preparation closes
/// the endpoints; Psi judges their public connection. This grants neither a
/// provider selection nor root/receiving consent to install the relationship.
#[derive(Debug, Clone)]
pub struct AssumedBindingContractDemand {
    pub required: StaticApplicationDemand,
    pub supplied: StaticApplicationDemand,
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}

impl PartialEq for AssumedBindingContractDemand {
    fn eq(&self, other: &Self) -> bool {
        let same_endpoint = |left: &StaticApplicationDemand, right: &StaticApplicationDemand| {
            left.declaration == right.declaration
                && left.application == right.application
                && std::sync::Arc::ptr_eq(&left.owner, &right.owner)
        };
        self.selecting_machine == other.selecting_machine
            && self.source_span == other.source_span
            && same_endpoint(&self.required, &other.required)
            && same_endpoint(&self.supplied, &other.supplied)
    }
}

impl Eq for AssumedBindingContractDemand {}

/// Arena-owned requested roster, retained unchanged through source admission.
/// The zero/dummy row is absent, never an executed pair.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AssumedBindingContractDemands {
    rows: arena::Arena<Option<AssumedBindingContractDemand>>,
}

impl AssumedBindingContractDemands {
    pub fn from_slice(demands: &[AssumedBindingContractDemand]) -> Self {
        let mut rows = arena::Arena::new();
        rows.insert_many(demands.iter().cloned().map(Some));
        Self { rows }
    }

    pub fn iter(&self) -> impl Iterator<Item = &AssumedBindingContractDemand> {
        self.rows.iter().filter_map(|(_, demand)| demand.as_ref())
    }
}

/// Original demanded application joined to its produced source instance.
/// The demand's handles stay in its owner; `instance` belongs to the program
/// that published this row. Checking still judges the resulting declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantiatedStaticApplication {
    pub demand: StaticApplicationDemand,
    pub instance: SymbolHandle,
}

/// A complete public header produced from an exact trait/member application.
/// The original trait remains its declaration owner; `signature` and the copied
/// binding application belong to this program. Captured trait regions precede
/// member invocation regions in the application roster, and remain distinct
/// from the invocation family retained by the public signature. This record
/// selects no implementation and supplies no conformance or trust authority.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstantiatedRequirementSignature {
    pub declaring_trait: SymbolHandle,
    pub selected_interface: SymbolHandle,
    pub requirement_template: SymbolHandle,
    pub signature: Handle<crate::signature::StateSignature>,
    /// Exact authored-to-applied contract fact custody for this public header.
    pub contract_fact_copies: HandleSpan<crate::signature::SignatureContractFactCopy>,
    /// Original explicitly selected interface tuple, including Calling custody.
    pub binding_application: StaticMachineApplicationHandle,
    /// The same application projected through its exact inherited declaring
    /// scope. Provider correspondence consumes the declaring binder/region
    /// roster, independently of the selected interface policy tuple.
    pub declaring_binding_application: StaticMachineApplicationHandle,
    /// Original reachable boundary interfaces that contain this member. An
    /// absent application marks missing original evidence correspondence; it
    /// cannot mean that the interface has no authored policy obligation.
    pub interface_applications: HandleSpan<RequirementInterfaceApplication>,
    pub captured_lifetime_count: u32,
}

/// One original interface and its induced, non-executing application. Every
/// child belongs to the same typed program as the public header. Zero retains
/// an honest missing independent-evidence producer, never an empty tuple.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RequirementInterfaceApplication {
    pub interface: SymbolHandle,
    pub binding_application: StaticMachineApplicationHandle,
}

/// Read the original selected interface of an exact public requirement. A
/// produced header owns its source interface; an authored direct declaration
/// owns itself. Equal signatures, another child's tuple or an ambient policy
/// cannot supply this coordinate.
pub fn requirement_selected_interface(
    program: &crate::TypedTrees,
    declaring_trait: SymbolHandle,
    requirement: SymbolHandle,
) -> Result<SymbolHandle, &'static str> {
    let mut headers = program
        .instantiated_requirement_signatures
        .iter()
        .filter(|(_, row)| {
            program.trait_machine_signatures.get(row.signature).symbol == requirement
        });
    if let Some((_, header)) = headers.next() {
        if headers.next().is_some()
            || header.declaring_trait != declaring_trait
            || !header.selected_interface.is_valid()
            || !header.binding_application.is_valid()
        {
            return Err("public requirement lost one original selected interface");
        }
        return Ok(header.selected_interface);
    }
    if program
        .trait_machine_signature(requirement)
        .is_some_and(|(declaration, _)| declaration.symbol == declaring_trait)
    {
        Ok(declaring_trait)
    } else {
        Err("public requirement has no original declaring interface")
    }
}

/// One exact source conformance instantiated for a demanded requirement tuple.
/// The source edge and both produced machines belong to the containing program.
/// Ordinary checking still establishes their complete conformance; these rows
/// neither elect a provider nor supply a different requirement implicitly.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstantiatedRequirementRealization {
    pub conformance: Handle<crate::machine::TraitConformance>,
    /// The same authored edge copied into the produced provider.
    pub instance_conformance: Handle<crate::machine::TraitConformance>,
    /// Named-map source custody, mutually exclusive with the machine-edge
    /// handles above. The map retains the exact original member and entry;
    /// specializing that body cannot replace the selecting declaration.
    pub named_conformance: Handle<crate::trait_definition::Conformance>,
    pub realization_state: SymbolHandle,
    pub requirement_template: SymbolHandle,
    pub requirement_instance: SymbolHandle,
    pub provider_template: SymbolHandle,
    pub provider_instance: SymbolHandle,
}

/// Default arena slots carry absence, never an invented original payload owner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstantiatedStaticApplicationSlot {
    application: Option<InstantiatedStaticApplication>,
}

impl crate::TypedTrees {
    pub fn retain_instantiated_static_application(
        &mut self,
        application: InstantiatedStaticApplication,
    ) -> Handle<InstantiatedStaticApplicationSlot> {
        self.instantiated_static_applications
            .append(InstantiatedStaticApplicationSlot {
                application: Some(application),
            })
    }

    pub fn instantiated_static_applications(
        &self,
    ) -> impl Iterator<
        Item = (
            Handle<InstantiatedStaticApplicationSlot>,
            &InstantiatedStaticApplication,
        ),
    > {
        self.instantiated_static_applications
            .iter()
            .filter_map(|(handle, slot)| {
                slot.application
                    .as_ref()
                    .map(|application| (handle, application))
            })
    }
}

pub(crate) use identity::static_application_identity_text;
