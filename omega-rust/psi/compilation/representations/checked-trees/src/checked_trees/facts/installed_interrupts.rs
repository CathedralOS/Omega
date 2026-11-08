//! Callback contracts owned by an explicit interrupt installation request.
//!
//! Checking establishes these selections and their entry resource receipts.
//! Native admission must still join the exact slot, current table, retained
//! backing and actual arrival implementation before publishing an interrupt.

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedInstalledInterruptRegistration {
    pub site: crate::NominalMachineUseSite,
    pub control_service: language_semantics::ServiceReachId,
    pub application: typed_trees::static_applications::StaticMachineApplicationHandle,
    pub target: symbols::SymbolHandle,
    pub registrar_owner: symbols::SymbolHandle,
    pub registrar_application_identity: String,
    pub registrar_declaring_application: String,
    pub registrar_arguments: Vec<String>,
    pub registrar_lifetimes: Vec<String>,
    pub timer_slot: typed_trees::expression::StaticMachineArgument,
    pub slot_application: typed_trees::typed_trees::ClosedConformanceApplication,
    pub callbacks: crate::NominalMachineUseFacts,
    pub specializations: crate::RequirementCallSpecializationFacts,
}
