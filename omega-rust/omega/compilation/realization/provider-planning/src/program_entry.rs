//! Program-entry selection and validation of source and physical calling contracts.
//!
//! Entry slots select a contract owner before selecting declarations. Bundled
//! contracts use their exact source custody; accepted packages use their explicit
//! package/declaration binding. The physical requirement comes from that schema's
//! resolved composition graph; the semantic requirement retains core's identity.
//! Calling plans must join both exact requirement symbols to the selected schema.
//! Unrelated same-leaf traits cannot redirect selection, including when Build and
//! product retain separate package instances.

use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;

mod discovery;
mod extent_layout;
pub use discovery::discover_explicit_entry_binding;
mod root_bindings;
pub use root_bindings::collect_root_bindings;

/// An executed Build root choice with its exact lexical product declaration.
/// Spelling is diagnostic payload; the resolved symbol selects the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
    /// Exact product machine selected under the occurrence's lexical package scope.
    pub implementation_symbol: symbols::SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedProgramEntry<'config> {
    pub machine_name: &'config str,
    /// Exact machine selected under the binding occurrence's lexical package.
    pub machine_symbol: symbols::SymbolHandle,
    pub slot: target::ProgramEntrySlotDeclaration,
}

/// Product-entry query compatibility for one admitted Build activation. Query
/// compatibility is supplied to Psi through its role-specific interface:
/// target slot catalogs and source admission stay here, beside final
/// ProgramEntry settlement. The Build does not observe a realized target, so
/// a query may name any recognized target's slot; each is a row keyed by
/// that target.
///
/// Whether a data declaration's all-zero image is a valid value is a checking
/// judgment — the zero-admission proof replay is checking machinery, so the
/// judgment is supplied by the driving stage that owns the checking edge
/// rather than called across the realization boundary.
pub struct ProductEntryQueryCompatibility {
    requires_establishment:
        fn(&TypedTrees, &checked_trees::typed_trees::data::DataDefinition) -> bool,
}

impl ProductEntryQueryCompatibility {
    pub fn new(
        requires_establishment: fn(
            &TypedTrees,
            &checked_trees::typed_trees::data::DataDefinition,
        ) -> bool,
    ) -> Self {
        Self {
            requires_establishment,
        }
    }
}

impl checked_interpreter::ProductEntryCompatibility for ProductEntryQueryCompatibility {
    fn validate_entry(
        &self,
        typed: &TypedTrees,
        declaration: checked_interpreter::ProductEntryDeclaration<'_>,
        slot: &str,
    ) -> Result<(), String> {
        let (owner, name) = slot.rsplit_once("::").ok_or_else(|| {
            format!("root slot `{slot}` is not target-qualified; expected `target::ProgramEntry`")
        })?;
        let profile = target::TargetProfile::from_root_slot_owner(owner).map_err(|_| {
            format!("root slot `{slot}` belongs to unknown target profile `{owner}`")
        })?;
        let required = profile.required_root_slot(name).ok_or_else(|| {
            format!(
                "target profile `{}` declares no required root slot `{slot}`",
                profile.target_name()
            )
        })?;
        let slot = required.program_entry().ok_or_else(|| {
            format!("root slot `{slot}` does not have a supported ProgramEntry signature")
        })?;
        validate_program_entry_source_compatibility(
            typed,
            declaration,
            slot,
            self.requires_establishment,
        )
        .map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect::<Vec<_>>()
                .join("; ")
        })
    }
}

/// Resolve the selected target's `ProgramEntry` binding. This is the first
/// implemented target-root slot; other root-slot kinds reject rather than
/// being accepted and then ignored. A build file may describe a target matrix:
/// well-formed slots owned by other known profiles are validated and left for
/// those profiles, while this selection consumes only the chosen profile's
/// exact row. With no root declarations there is no selected entry; native
/// production independently requires one before realization.
pub fn selected_program_entry_machine<'config>(
    root_bindings: &'config [RootBinding],
    selected_profile: Option<target::TargetProfile>,
) -> Result<Option<SelectedProgramEntry<'config>>, Vec<Diagnostic>> {
    let Some(selected_profile) = selected_profile else {
        return Ok(None);
    };
    if root_bindings.is_empty() {
        return Ok(None);
    }
    let mut diagnostics = Vec::new();
    let mut selected_bindings = Vec::new();
    for binding in root_bindings {
        let Some((profile, slot_name)) = binding.slot.rsplit_once("::") else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}` is not target-qualified; expected `target::ProgramEntry`",
                binding.slot
            )));
            continue;
        };
        let profile = match target::TargetProfile::from_root_slot_owner(profile) {
            Ok(profile) => profile,
            Err(_) => {
                diagnostics.push(Diagnostic::error(format!(
                    "root slot `{}` belongs to unknown target profile `{profile}`",
                    binding.slot
                )));
                continue;
            }
        };
        let Some(required_slot) = profile.required_root_slot(slot_name) else {
            diagnostics.push(Diagnostic::error(format!(
                "target profile `{}` declares no required root slot `{}`",
                profile.target_name(),
                binding.slot
            )));
            continue;
        };
        if profile != selected_profile {
            continue;
        }
        selected_bindings.push((required_slot, binding));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut program_entries = Vec::new();
    for required_slot in selected_profile.required_root_slots() {
        let matches = selected_bindings
            .iter()
            .filter(|(selected, _)| selected == &required_slot)
            .collect::<Vec<_>>();
        let binding = match matches.as_slice() {
            [(_, binding)] => *binding,
            [] => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has no bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
            _ => {
                diagnostics.push(Diagnostic::error(format!(
                    "selected target `{}` has more than one bound required root slot `{}::{}`",
                    selected_profile.target_name(),
                    required_slot.owner().root_slot_owner_name(),
                    required_slot.slot_name()
                )));
                continue;
            }
        };
        let Some(program_entry) = required_slot.program_entry() else {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{}::{}` uses a target-required schema that the ProgramEntry source-lowering path does not implement",
                required_slot.owner().root_slot_owner_name(),
                required_slot.slot_name()
            )));
            continue;
        };
        program_entries.push((program_entry, binding));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    match program_entries.as_slice() {
        [(slot, binding)] => Ok(Some(SelectedProgramEntry {
            machine_name: binding.implementation.as_str(),
            machine_symbol: binding.implementation_symbol,
            slot: *slot,
        })),
        [] => Err(vec![Diagnostic::error(format!(
            "selected target `{}` has no supported ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
        _ => Err(vec![Diagnostic::error(format!(
            "selected target `{}` declares more than one ProgramEntry root schema",
            selected_profile.target_name()
        ))]),
    }
}

/// Validate only the static source compatibility of `ProgramEntry`.
/// schema. Hosted targets expose no arrival parameters: the selected machine
/// is either free or has exactly one mutable context parameter for later bridge
/// provisioning. Freestanding parameters must exactly match the canonical
/// typed positions on the target-selected arrival requirement.
///
/// Both description issuance and final admission use this decision. It does
/// not require the provider or layout choices the build is still computing.
fn validate_program_entry_source_compatibility(
    typed: &TypedTrees,
    declaration: checked_interpreter::ProductEntryDeclaration<'_>,
    slot: target::ProgramEntrySlotDeclaration,
    requires_establishment: fn(
        &TypedTrees,
        &checked_trees::typed_trees::data::DataDefinition,
    ) -> bool,
) -> Result<(), Vec<Diagnostic>> {
    let machine_name = declaration.name;
    let Some(entry) = declaration.entry else {
        return Err(vec![Diagnostic::error(format!(
            "entry machine `{machine_name}` has no executable entry state"
        ))]);
    };

    let mut diagnostics = Vec::new();
    if !declaration.type_parameters.is_empty() {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` is generic; a root slot must bind one exact machine"
        )));
    }
    if entry.return_type.is_valid() {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` returns a value, but `ProgramEntry` has no result"
        )));
    }

    let parameters = entry.parameters;
    let context = provisioned_entry_context_from_parameters(typed, machine_name, parameters)
        .map_err(|diagnostic| vec![diagnostic])?;
    if let Some(context) = &context
        && requires_establishment(typed, context.definition)
    {
        diagnostics.push(Diagnostic::error(format!(
            "entry machine `{machine_name}` requests a provisioned `{}` context, but its all-zero image is not a valid value; use a free entry and construct the state explicitly",
            context.definition.name.as_str()
        )));
    }

    // A `&Placed<P, T>` / `&mut Placed<P, T>` entry parameter is a custody
    // input — the roster row demands an establishment the hosted entry
    // boundary lends for the invocation — not an arrival parameter the loader
    // hands in. Keeping it out of `visible` lets the slot's arrival-shape
    // checks count only real arrival parameters; the row itself still answers
    // native input admission and fails closed there while no bridge lends it.
    let visible = visible_entry_parameters(typed, parameters, &context);
    match slot.visible_parameters {
        target::ProgramEntryVisibleParameters::None if !visible.is_empty() => {
            diagnostics.push(Diagnostic::error(format!(
                "hosted `ProgramEntry` exposes no arrival parameters, but `{machine_name}` declares `{}`",
                visible
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<Vec<_>>()
                    .join("`, `")
            )));
        }
        target::ProgramEntryVisibleParameters::ImageAndInitialStorage if visible.len() != 2 => {
            diagnostics.push(Diagnostic::error(format!(
                "target schema `{:?}` exposes exactly image and initial-storage roots, but `{machine_name}` declares {} visible parameter{}",
                slot.schema,
                visible.len(),
                if visible.len() == 1 { "" } else { "s" },
            )));
        }
        _ => {}
    }

    if slot.visible_parameters == target::ProgramEntryVisibleParameters::ImageAndInitialStorage
        && visible.len() == 2
    {
        match core_arrival_requirement_contract(typed, slot.semantic_arrival_requirement) {
            Ok(required) if required.parameters.len() == visible.len() => {
                for (index, (actual, required)) in
                    visible.iter().zip(required.parameters.iter()).enumerate()
                {
                    if typed.normalized_type_identity(actual.type_reference) != required.identity
                        || actual.is_const != required.is_const
                        || actual.is_mutable != required.is_mutable
                    {
                        diagnostics.push(Diagnostic::error(format!(
                            "target root slot `{}::{}` requires visible parameter {index} ({}) to have exact type `{}`, but entry machine `{machine_name}` declares `{}`",
                            slot.owner.root_slot_owner_name(),
                            slot.slot_name,
                            ["image", "initial storage"][index],
                            required.display,
                            typed.display_type_reference_with_constraints(actual.type_reference),
                        )));
                    }
                }
            }
            Ok(required) => diagnostics.push(Diagnostic::error(format!(
                "target root slot `{}::{}` selects arrival requirement `{}` with {} visible parameters, but its target schema declares {}",
                slot.owner.root_slot_owner_name(),
                slot.slot_name,
                slot.semantic_arrival_requirement,
                required.parameters.len(),
                visible.len(),
            ))),
            Err(diagnostic) => diagnostics.push(diagnostic),
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(())
}

/// Borrowed source coordinates shared by entry selection and service discovery.
pub struct ProvisionedEntryContext<'program> {
    pub source_position: u32,
    pub parameter: &'program checked_trees::typed_trees::signature::StateParameter,
    pub definition: &'program checked_trees::typed_trees::data::DataDefinition,
}

/// The source occurrence provisioned by the entry bridge is not a physical
/// arrival argument and is unrelated to the machine's namespace qualifier.
/// Placed views keep their own custody-input role.
pub fn provisioned_entry_context<'program>(
    typed: &'program TypedTrees,
    machine: &checked_trees::typed_trees::machine::Machine,
    entry: &checked_trees::typed_trees::state::State,
) -> Result<Option<ProvisionedEntryContext<'program>>, Diagnostic> {
    provisioned_entry_context_from_parameters(
        typed,
        machine.name.as_str(),
        typed.state_parameters(entry),
    )
}

fn provisioned_entry_context_from_parameters<'program>(
    typed: &'program TypedTrees,
    machine_name: &str,
    parameters: &'program [checked_trees::typed_trees::signature::StateParameter],
) -> Result<Option<ProvisionedEntryContext<'program>>, Diagnostic> {
    use checked_trees::typed_trees::types::TypeReferenceNode;

    let mut candidates = parameters.iter().enumerate().filter(|(_, parameter)| {
        !parameter.relevance.is_erased()
            && typed
                .placed_view_plan_for_input_type_reference(parameter.type_reference)
                .is_none()
            && matches!(
                typed
                    .type_reference_table
                    .type_reference(parameter.type_reference),
                TypeReferenceNode::Reference { .. }
            )
    });
    let Some((source_position, parameter)) = candidates.next() else {
        return Ok(None);
    };
    if candidates.next().is_some() {
        return Err(Diagnostic::error(format!(
            "entry machine `{machine_name}` declares multiple context candidates; `ProgramEntry` provisions at most one ordinary mutable context parameter"
        )));
    }
    if parameter.is_const || parameter.relevance.is_erased() {
        return Err(Diagnostic::error(format!(
            "entry machine `{machine_name}` context must be an ordinary runtime parameter"
        )));
    }
    let context_reference = {
        let TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Mutable,
            referee,
            ..
        } = typed
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return Err(Diagnostic::error(format!(
                "entry machine `{machine_name}` context `{}` requires an exclusive mutable borrow",
                parameter.name
            )));
        };
        *referee
    };
    // Entry provisioning consumes the same retained closed carrier as other
    // nominal operations. A lifetime application changes semantic borrowing,
    // not the context's storage declaration; requiring a bare Named node here
    // rejected valid applications before the ordinary loan checks could run.
    // Keep the full parameter reference for signature and establishment custody.
    let definition = typed
        .closed_data_definition(context_reference)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "entry machine `{machine_name}` context `{}` has no exact data declaration",
                parameter.name
            ))
        })?;
    let source_position = u32::try_from(source_position)
        .map_err(|_| Diagnostic::error("program-entry context source position exceeds u32"))?;
    Ok(Some(ProvisionedEntryContext {
        source_position,
        parameter,
        definition,
    }))
}

/// Recheck the selected source signature and retain its final layout evidence.
/// An earlier product description neither settles providers nor authorizes
/// omission of this check against the completed product.
fn validate_selected_program_entry_shape(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
    requires_establishment: fn(
        &TypedTrees,
        &checked_trees::typed_trees::data::DataDefinition,
    ) -> bool,
) -> Result<SelectedEntryDeclaration, Vec<Diagnostic>> {
    let machine_name = selected.machine_name;
    // Description issuance only needs an interface. Final admission still
    // requires the exact selected executable machine and its real entry state.
    let machine = typed.machine(selected.machine_symbol)
        .ok_or_else(|| vec![Diagnostic::error(format!(
            "build root slot selected entry `{machine_name}` is not a declaration in the admitted program"
        ))])?;
    let entry = typed.machine_states(machine).first().ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "entry machine `{machine_name}` has no executable entry state"
        ))]
    })?;
    validate_program_entry_source_compatibility(
        typed,
        checked_interpreter::ProductEntryDeclaration::from_machine(typed, machine),
        selected.slot,
        requires_establishment,
    )?;
    let parameters = typed.state_parameters(entry);
    let context =
        provisioned_entry_context(typed, machine, entry).map_err(|diagnostic| vec![diagnostic])?;
    // Same exclusion as source compatibility: placed-view inputs are custody
    // rows, not arrival parameters, so they never occupy the signature's
    // visible-parameter roles.
    let visible = visible_entry_parameters(typed, parameters, &context);
    // The signature's identity table mints its indices beside the checked
    // program, before a Terminal module exists; native consumers rejoin them
    // through the text.
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let receiver = context.as_ref().map_or(
        program_entry_plan::ProgramEntryReceiverSignature::Free,
        |context| program_entry_plan::ProgramEntryReceiverSignature::ProvisionedMutable {
            source_position: context.source_position,
            normalized_type_identity: identities
                .intern(&typed.normalized_type_identity_text(context.parameter.type_reference)),
        },
    );
    let visible_parameters = visible
        .iter()
        .enumerate()
        .map(|(index, parameter)| -> Result<_, Diagnostic> {
            let role = match index {
                0 => program_entry_plan::ProgramStorageEntryRootRole::Image,
                1 => program_entry_plan::ProgramStorageEntryRootRole::InitialStorage,
                _ => unreachable!("selected source shape validation fixed visible arity"),
            };
            let extent_value_layout =
                extent_layout::capture(
                    typed,
                    selected.slot,
                    parameter.type_reference,
                )
            .map_err(|diagnostic| {
                Diagnostic::error(format!(
                    "selected entry machine `{machine_name}` visible parameter {index} has no exact Extent value layout: {diagnostic}"
                ))
            })?;
            let value_shape = extent_value_layout.shape();
            Ok(program_entry_plan::SelectedProgramEntrySignature::visible_parameter(
                role,
                index,
                identities
                    .intern(&typed.normalized_type_identity_text(parameter.type_reference)),
                value_shape,
                extent_value_layout,
                parameter.is_const,
                parameter.is_mutable,
            ))
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|diagnostic| vec![diagnostic])?;
    if !machine.symbol.is_valid()
        || !entry.symbol.is_valid()
        || context
            .as_ref()
            .is_some_and(|context| !context.parameter.symbol.is_valid())
    {
        return Err(vec![Diagnostic::error(
            "selected source signature contains an invalid declaration coordinate",
        )]);
    }
    // Only this declaration's owner participates in its nominal identity. Do
    // not hash the entire source set to close one entry's source coordinate.
    let declaration_source = typed
        .symbols
        .symbol_provenance_source_span(machine.symbol)
        .map(|span| span.source_id);
    let exact_sources = package_compilation::toolchain_source_identities(
        typed
            .symbols
            .source_files()
            .filter(|file| Some(file.source_id) == declaration_source),
    )?;
    let declaration = typed
        .exact_owner_nominal_identity(machine.symbol, &exact_sources)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "selected program entry has no exact declaration owner",
            )]
        })?;
    let declaration = typed.type_identity_text(declaration).unwrap_or_default();
    let signature = program_entry_plan::SelectedProgramEntrySignature::from_checked_declaration(
        selected.slot,
        declaration,
        machine.name.as_str().to_owned(),
        entry.name.as_str().to_owned(),
        identities.intern(
            &typed
                .machine_callable_identity_text(machine)
                .expect("selected entry has one checked executable state"),
        ),
        receiver,
        visible_parameters,
        identities.build(),
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])?;
    Ok(SelectedEntryDeclaration {
        machine: machine.symbol,
        state: entry.symbol,
        context_parameter: context
            .map_or_else(Default::default, |context| context.parameter.symbol),
        signature,
    })
}

/// Source-only coordinates stay beside the producer that can interpret them.
/// Native consumers receive `signature`, never this record.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SelectedEntryDeclaration {
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    context_parameter: symbols::SymbolHandle,
    signature: program_entry_plan::SelectedProgramEntrySignature,
}

/// Retain both authored applications: their source/signature commitments are
/// distinct from the validated ABI plans' own structural commitments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProgramEntryCallingPlans {
    pub physical_requirement: symbols::SymbolHandle,
    pub semantic_calling_application: calling_conventions::BoundaryCallingApplication,
    pub physical_calling_application: calling_conventions::BoundaryCallingApplication,
    pub storage_entry: program_entry_plan::SelectedProgramStorageEntryPlan,
}

/// Complete realization-owned settlement for one target-selected `ProgramEntry`.
///
/// The source signature and optional two-surface calling plans are validated
/// while typed declarations are still available, then travel together instead
/// of becoming independent driver couriers. A test-harness entry-name override
/// is deliberately absent: it cannot acquire source, calling-plan, or storage
/// authority through this carrier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedCompilerProgramEntry {
    declaration: SelectedEntryDeclaration,
    calling_plans: Option<SelectedProgramEntryCallingPlans>,
    physical_adapter: Option<symbols::SymbolHandle>,
    fused_service_establishments: Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
}

impl SelectedCompilerProgramEntry {
    fn new(
        declaration: SelectedEntryDeclaration,
        calling_plans: Option<SelectedProgramEntryCallingPlans>,
    ) -> Self {
        Self {
            declaration,
            calling_plans,
            physical_adapter: None,
            fused_service_establishments: Vec::new(),
        }
    }

    pub fn machine_name(&self) -> &str {
        self.declaration.signature.machine_name()
    }

    /// The explicitly selected checked implementation of physical arrival.
    /// This is separate from the semantic ProgramEntry: firmware cannot call
    /// the two-root continuation with its handle and System Table registers.
    pub const fn physical_adapter(&self) -> Option<symbols::SymbolHandle> {
        self.physical_adapter
    }

    pub fn bind_physical_adapter(
        &mut self,
        typed: &TypedTrees,
        providers: &[crate::SelectedProviderReviewProvenance],
    ) -> Result<(), Vec<Diagnostic>> {
        let Some(plans) = &self.calling_plans else {
            return Ok(());
        };
        let mut selected = None;
        for provider in providers {
            for (requirement, implementation) in provider
                .provider
                .row_requirements
                .iter()
                .zip(&provider.provider.row_realizations)
            {
                if *requirement != plans.physical_requirement {
                    continue;
                }
                if matches!(
                    provider.selected_by,
                    crate::ProviderSelectionProvenance::UniqueCoveringCandidate
                ) {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter requires an explicit provider selection",
                    )]);
                }
                let machine = typed
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == *implementation)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "selected physical entry adapter is absent",
                        )]
                    })?;
                if typed
                    .machine_states(machine)
                    .first()
                    .is_none_or(|entry| !entry.symbol.is_valid())
                    || !typed.machine_type_parameters(machine).is_empty()
                {
                    return Err(vec![Diagnostic::error(
                        "physical entry adapter must have a checked source body",
                    )]);
                }
                if selected.replace(*implementation).is_some() {
                    return Err(vec![Diagnostic::error(
                        "physical arrival has more than one selected adapter",
                    )]);
                }
            }
        }
        self.physical_adapter = selected;
        Ok(())
    }

    pub const fn source_signature(&self) -> &program_entry_plan::SelectedProgramEntrySignature {
        &self.declaration.signature
    }

    /// Replaces the source signature's identity table after late minting —
    /// bound fused service establishments intern their rows into it.
    pub fn set_source_identities(&mut self, identities: terminal_psi::TerminalIdentityTable) {
        self.declaration.signature.set_identities(identities);
    }

    pub const fn machine_symbol(&self) -> symbols::SymbolHandle {
        self.declaration.machine
    }

    pub const fn state_symbol(&self) -> symbols::SymbolHandle {
        self.declaration.state
    }

    pub const fn context_parameter(&self) -> symbols::SymbolHandle {
        self.declaration.context_parameter
    }

    pub const fn calling_plans(&self) -> Option<&SelectedProgramEntryCallingPlans> {
        self.calling_plans.as_ref()
    }

    pub fn fused_service_establishments(
        &self,
    ) -> &[program_entry_plan::ProgramEntryFusedServiceEstablishment] {
        &self.fused_service_establishments
    }

    pub fn bind_fused_service_establishments(
        &mut self,
        mut establishments: Vec<program_entry_plan::ProgramEntryFusedServiceEstablishment>,
    ) -> Result<(), &'static str> {
        if establishments.is_empty() {
            self.fused_service_establishments.clear();
            return Ok(());
        }
        let source_identity = self.source_signature().identity();
        let target_slot = self.source_signature().target_slot();
        let receiver_identity = self
            .source_signature()
            .receiver()
            .normalized_type_identity()
            .ok_or("a free ProgramEntry cannot establish receiver fields")?;
        // Key on the field ROUTE, not the leaf name. A service carrier
        // contributes one establishment at whatever depth the receiver's
        // record-field tree places it, which is how the producer both orders
        // and de-duplicates them, so two bindings that differ only in their
        // owning record -- `Main::console` and `Main::banner::console` --
        // share a leaf name and are not a repeat.
        let identities = self.source_signature().identities();
        establishments.sort_by_cached_key(|establishment| {
            establishment
                .field_path()
                .iter()
                .map(|segment| identities.text(*segment).unwrap_or_default().to_owned())
                .collect::<Vec<_>>()
        });
        if establishments
            .windows(2)
            .any(|pair| pair[0].field_path() == pair[1].field_path())
            || establishments.iter().any(|establishment| {
                establishment.source_signature_identity() != source_identity
                    || establishment.target_slot() != target_slot
                    || establishment.receiver_type_identity() != receiver_identity
            })
        {
            return Err("Fused root establishments drifted from their selected ProgramEntry");
        }
        self.fused_service_establishments = establishments;
        Ok(())
    }
}

/// Shared checking for bound rows the request does not realize. One Build
/// produces checked rows for every target it names, so an authored binding
/// carries the same source obligations whether or not its target is selected:
/// the bound machine must exist, own an executable entry state, and satisfy
/// its slot's source compatibility. What stays per target is absence, not
/// malformation — a family with no body for an unselected target and an
/// unbound required slot wait for that target's own selection.
fn check_unrequested_program_entry_rows(
    typed: &TypedTrees,
    root_bindings: &[RootBinding],
    selected_profile: Option<target::TargetProfile>,
    requires_establishment: fn(
        &TypedTrees,
        &checked_trees::typed_trees::data::DataDefinition,
    ) -> bool,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for binding in root_bindings {
        let Some((owner, slot_name)) = binding.slot.rsplit_once("::") else {
            continue;
        };
        let Ok(profile) = target::TargetProfile::from_root_slot_owner(owner) else {
            continue;
        };
        if Some(profile) == selected_profile {
            continue;
        }
        let Some(required_slot) = profile.required_root_slot(slot_name) else {
            continue;
        };
        let Some(slot) = required_slot.program_entry() else {
            continue;
        };
        // Mirror the selected path's family resolution: the binding names the
        // family, and only the body authored for its own target answers the
        // source-shape check.
        let mut machine_symbol = binding.implementation_symbol;
        if let Some(family) = typed.target_machine_families.iter().find(|family| {
            family.canonical == machine_symbol
                || family
                    .members
                    .iter()
                    .any(|member| member.machine == machine_symbol)
        }) {
            let Some(member) = family
                .members
                .iter()
                .find(|member| member.target.as_str() == profile.target_name())
            else {
                continue;
            };
            machine_symbol = member.machine;
        }
        if let Err(mut rejected) = validate_selected_program_entry_shape(
            typed,
            SelectedProgramEntry {
                machine_name: binding.implementation.as_str(),
                machine_symbol,
                slot,
            },
            requires_establishment,
        ) {
            diagnostics.append(&mut rejected);
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Resolve and validate the complete realization-owned `ProgramEntry` input.
/// Diagnostic order is intentional: exact target-slot selection precedes
/// source-shape validation, which precedes optional physical/semantic calling-
/// plan validation.
pub fn select_compiler_program_entry(
    typed: &TypedTrees,
    root_bindings: &[RootBinding],
    selected_profile: Option<target::TargetProfile>,
    realizations: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    package_inputs: Option<&package_compilation::PackageCompilationInputs>,
    requires_establishment: fn(
        &TypedTrees,
        &checked_trees::typed_trees::data::DataDefinition,
    ) -> bool,
) -> Result<Option<SelectedCompilerProgramEntry>, Vec<Diagnostic>> {
    let selected = selected_program_entry_machine(root_bindings, selected_profile)?;
    check_unrequested_program_entry_rows(
        typed,
        root_bindings,
        selected_profile,
        requires_establishment,
    )?;
    let Some(mut selected) = selected else {
        return Ok(None);
    };
    // A root binding names the family, not the body that happens to own its
    // canonical spelling. Select the exact entry handle without changing the
    // source graph; every body still undergoes the ordinary Psi checks.
    if let Some(family) = typed.target_machine_families.iter().find(|family| {
        family.canonical == selected.machine_symbol
            || family
                .members
                .iter()
                .any(|member| member.machine == selected.machine_symbol)
    }) {
        let profile = selected.slot.owner;
        let member = family
            .members
            .iter()
            .find(|member| member.target.as_str() == profile.target_name())
            .ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "entry family `{}` has no body for target `{}`",
                    family.name,
                    profile.target_name(),
                ))]
            })?;
        selected.machine_symbol = member.machine;
    }
    let source_signature =
        validate_selected_program_entry_shape(typed, selected, requires_establishment)?;
    // Only an exact selected target activates an entry. Resolve its physical
    // contract's accepted package role here, never from the host or a caller-
    // chosen binding for another target. Calling-plan validation still rejoins
    // the accepted nominal and schema independently.
    let accepted_entry_binding = selected
        .slot
        .physical_contract_package
        .map(package_compilation::program_entry_semantic_binding_role)
        .and_then(|role| package_inputs.and_then(|inputs| inputs.accepted_semantic_binding(role)));
    let calling_plans = validate_selected_program_entry_calling_plan(
        typed,
        selected,
        realizations,
        accepted_entry_binding,
    )?;
    Ok(Some(SelectedCompilerProgramEntry::new(
        source_signature,
        calling_plans,
    )))
}

pub fn validate_selected_program_entry_calling_plan(
    typed: &TypedTrees,
    selected: SelectedProgramEntry<'_>,
    realizations: &[crate::calling_policy_plans::BoundaryCallingPlanRealization],
    accepted_entry_binding: Option<&package_compilation::AcceptedSemanticBinding>,
) -> Result<Option<SelectedProgramEntryCallingPlans>, Vec<Diagnostic>> {
    let (
        Some(schema_name),
        Some(physical_requirement),
        Some(physical_convention),
        Some(semantic_convention),
    ) = (
        selected.slot.boundary_schema,
        selected.slot.physical_arrival_requirement,
        selected.slot.physical_calling_convention,
        selected.slot.semantic_calling_convention,
    )
    else {
        if selected.slot.boundary_schema.is_some()
            || selected.slot.physical_arrival_requirement.is_some()
            || selected.slot.physical_calling_convention.is_some()
            || selected.slot.semantic_calling_convention.is_some()
        {
            return Err(vec![Diagnostic::error(format!(
                "target root slot `{}::{}` has an incomplete two-surface entry declaration",
                selected.slot.owner.root_slot_owner_name(),
                selected.slot.slot_name,
            ))]);
        }
        return Ok(None);
    };
    let schema = selected_entry_schema(typed, selected.slot, schema_name, accepted_entry_binding)
        .map_err(|diagnostic| vec![diagnostic])?;
    let semantic =
        core_arrival_requirement_contract(typed, selected.slot.semantic_arrival_requirement)
            .map_err(|diagnostic| vec![diagnostic])?;
    let physical = arrival_requirement_contract(typed, schema, physical_requirement)
        .map_err(|diagnostic| vec![diagnostic])?;
    if semantic.signature == physical.signature
        || semantic.requirement_identity == physical.requirement_identity
    {
        return Err(vec![Diagnostic::error(format!(
            "target root slot `{}::{}` conflates physical requirement `{physical_requirement}` with semantic requirement `{}`",
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            selected.slot.semantic_arrival_requirement,
        ))]);
    }
    let semantic_matching = realizations
        .iter()
        .filter(|realization| {
            realization.boundary_trait == schema.symbol
                && realization.requirement_machine == semantic.signature
        })
        .collect::<Vec<_>>();
    let [semantic_realization] = semantic_matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` retains {} evaluated calling plans for semantic requirement `{}` instead of exactly one",
            semantic_matching.len(),
            selected.slot.semantic_arrival_requirement,
        ))]);
    };
    let physical_matching = realizations
        .iter()
        .filter(|realization| {
            realization.boundary_trait == schema.symbol
                && realization.requirement_machine == physical.signature
        })
        .collect::<Vec<_>>();
    let [physical_realization] = physical_matching.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` retains {} evaluated calling plans for physical requirement `{physical_requirement}` instead of exactly one",
            physical_matching.len(),
        ))]);
    };
    for (role, realization) in [
        ("semantic", *semantic_realization),
        ("physical", *physical_realization),
    ] {
        let (validated, application_report_fingerprint, application_commitment) = realization
            .replayed_validated_application()
            .map_err(|error| {
            vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` retains an invalid {role} calling plan: {error}"
            ))]
        })?;
        if realization.exact_boundary_entry_plan() != validated.plan()
            || realization.report_fingerprint != application_report_fingerprint
            || realization.commitment != application_commitment
        {
            return Err(vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` does not retain exact {role} calling-plan identity and custody"
            ))]);
        }
    }
    let (validated_physical_plan, _, _) = physical_realization
        .replayed_validated_application()
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "target boundary schema `{schema_name}` lost its validated physical calling plan: {error}"
            ))]
        })?;
    let expected_physical =
        calling_conventions::CallingPolicy::for_program_entry_convention(physical_convention);
    if physical_realization.boundary_entry_plan.call.policy != expected_physical {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` evaluates physical requirement `{physical_requirement}` with {:?}, but `{}::{}` requires {:?}",
            physical_realization.boundary_entry_plan.call.policy,
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            expected_physical,
        ))]);
    }
    let expected_semantic =
        calling_conventions::CallingPolicy::for_program_entry_convention(semantic_convention);
    if semantic_realization.boundary_entry_plan.call.policy != expected_semantic {
        return Err(vec![Diagnostic::error(format!(
            "target boundary schema `{schema_name}` evaluates semantic requirement `{}` with {:?}, but `{}::{}` requires {:?}",
            selected.slot.semantic_arrival_requirement,
            semantic_realization.boundary_entry_plan.call.policy,
            selected.slot.owner.root_slot_owner_name(),
            selected.slot.slot_name,
            expected_semantic,
        ))]);
    }
    let service_schema = crate::service_schema::from_typed(
        crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product),
        realizations,
        schema,
    )
    .ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "target entry schema `{schema_name}` is not a boundary service schema"
        ))]
    })?;
    let physical_source = target_owned_physical_contract_source(
        typed,
        selected.slot,
        schema.symbol,
        &service_schema,
        &physical,
        accepted_entry_binding,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let storage_entry = program_entry_plan::SelectedProgramStorageEntryPlan::from_target_slot(
        selected.slot,
        service_schema,
        semantic.requirement_identity,
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    let result_type_identity = physical.result_type_identity.ok_or_else(|| {
        vec![Diagnostic::error(format!(
            "physical entry requirement `{physical_requirement}` has no result"
        ))]
    })?;
    let physical_contract = program_entry_plan::ProgramEntryPhysicalContractPlan::new(
        selected.slot,
        physical.requirement_identity,
        physical_source.package,
        physical_source.package_source_digest,
        physical_source.non_authoritative_package_source_report_fingerprint,
        physical
            .parameters
            .into_iter()
            .map(|parameter| {
                typed
                    .type_identity_text(parameter.identity)
                    .unwrap_or_default()
            })
            .collect(),
        typed
            .type_identity_text(result_type_identity)
            .unwrap_or_default(),
        validated_physical_plan.contract_report_fingerprint(),
        physical_realization.boundary_entry_plan.clone(),
    )
    .map_err(|diagnostic| vec![Diagnostic::error(diagnostic)])?;
    let storage_entry = storage_entry
        .with_physical_contract(physical_contract.clone())
        .map_err(|diagnostic| vec![Diagnostic::error(diagnostic.to_string())])?;
    Ok(Some(SelectedProgramEntryCallingPlans {
        physical_requirement: physical.signature,
        semantic_calling_application: semantic_realization
            .native_application()
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        physical_calling_application: physical_realization
            .native_application()
            .map_err(|reason| vec![Diagnostic::error(reason)])?,
        storage_entry,
    }))
}

/// Resolve only inside the slot's contract owner. Source custody identifies a
/// bundled source unit; its leaf name selects a declaration inside that unit,
/// never a candidate in the whole program. Package bindings already retain the
/// complete declaration path and must not fall back to the bundled contract.
fn selected_entry_schema<'program>(
    typed: &'program TypedTrees,
    slot: target::ProgramEntrySlotDeclaration,
    schema_name: &str,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
) -> Result<&'program checked_trees::typed_trees::trait_definition::TraitDefinition, Diagnostic> {
    let package = slot.physical_contract_package.ok_or_else(|| {
        Diagnostic::error("target entry slot has no physical contract package identity")
    })?;
    let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
    let mut selected_symbols = Vec::new();
    if let Some(binding) = accepted_binding {
        if binding.role() == program_entry_semantic_binding_role(package)
            && binding.selected_provider_plan_digest().is_none()
        {
            selected_symbols.extend(typed.traits().iter().filter_map(|definition| {
                (context.contains(definition.symbol)
                    && typed.symbols.symbol_package_identity(definition.symbol)
                        == Some(binding.package())
                    && typed.symbols.display_path(definition.symbol, "::")
                        == binding.declaration_path())
                .then_some(definition.symbol)
            }));
        }
    } else {
        let expected_digest = exact_bundled_physical_contract_package_source_digest(package);
        for source_file in typed.symbols.source_files().filter(|source_file| {
            source_file.dependency_scope == source::DependencyScope::Product
                && source_file.package_identity.is_none()
                && source_file.path.strip_prefix(&source_file.package_root).ok()
                    == Some(std::path::Path::new(package.package_relative_source()))
                && program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
                    package,
                    source_file.source.as_bytes(),
                ) == expected_digest
        }) {
            selected_symbols.extend(typed.traits().iter().filter_map(|definition| {
                (typed.symbols.symbol_source_span(definition.symbol)
                    .is_some_and(|span| span.source_id == source_file.source_id)
                    && definition.name.as_str() == schema_name)
                    .then_some(definition.symbol)
            }));
        }
    }
    // An unlocked compile has no accepted binding, so a dependency's copy of
    // the contract (std's, usually) is loaded but not selected. Name the
    // review that supplies the binding instead of an empty candidate count.
    let package_copy_loaded = || {
        typed.symbols.source_files().any(|source_file| {
            source_file.dependency_scope == source::DependencyScope::Product
                && source_file.package_identity.is_some()
                && source_file
                    .path
                    .strip_prefix(&source_file.package_root)
                    .ok()
                    == Some(std::path::Path::new(package.package_relative_source()))
        })
    };
    if selected_symbols.is_empty() && accepted_binding.is_none() && package_copy_loaded() {
        return Err(Diagnostic::error(format!(
            "target root slot `{}::{}` takes boundary schema `{schema_name}` from a dependency's `{}`, which needs an accepted program-entry binding; run `omega update` to review and accept it",
            slot.owner.root_slot_owner_name(),
            slot.slot_name,
            package.package_relative_source(),
        )));
    }
    let [symbol] = selected_symbols.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target root slot `{}::{}` requires one exact contract-owned boundary schema `{schema_name}`, but found {}",
            slot.owner.root_slot_owner_name(),
            slot.slot_name,
            selected_symbols.len(),
        )));
    };
    typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == *symbol && definition.is_boundary)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "target entry contract declaration `{schema_name}` is not a boundary schema"
            ))
        })
}

struct TargetOwnedPhysicalContractSource {
    package: target::ProgramEntryPhysicalContractPackage,
    package_source_digest: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
    non_authoritative_package_source_report_fingerprint: u64,
}

fn target_owned_physical_contract_source(
    typed: &TypedTrees,
    slot: target::ProgramEntrySlotDeclaration,
    schema: symbols::SymbolHandle,
    service_schema: &effects::provider_plan::ServiceSchema,
    contract: &ArrivalRequirementContract,
    accepted_binding: Option<&package_compilation::AcceptedSemanticBinding>,
) -> Result<TargetOwnedPhysicalContractSource, Diagnostic> {
    let expected_package = slot.physical_contract_package.ok_or_else(|| {
        Diagnostic::error("target physical entry requirement has no owning package identity")
    })?;
    let source_span = typed
        .symbols
        .symbol_source_span(contract.signature)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "target physical entry requirement `{}` has no authored source provenance",
                contract.requirement_identity
            ))
        })?;
    let source_file = typed.symbols.source_file(source_span).ok_or_else(|| {
        Diagnostic::error("target physical entry requirement lost its source-file provenance")
    })?;
    let schema_source_span = typed.symbols.symbol_source_span(schema).ok_or_else(|| {
        Diagnostic::error("target physical entry schema has no authored source provenance")
    })?;
    let schema_source_file = typed
        .symbols
        .source_file(schema_source_span)
        .ok_or_else(|| {
            Diagnostic::error("target physical entry schema lost its source-file provenance")
        })?;
    let package_relative_source = source_file
        .path
        .strip_prefix(&source_file.package_root)
        .ok();
    let package_source_digest =
        program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            expected_package,
            source_file.source.as_bytes(),
        );
    if schema_source_file.source_id != source_file.source_id {
        return Err(Diagnostic::error(format!(
            "target physical entry requirement `{}` and its selected schema must come from one exact source unit",
            contract.requirement_identity,
        )));
    }
    let expected_role = program_entry_semantic_binding_role(expected_package);
    match accepted_binding {
        Some(binding) => {
            let exact_package_binding = binding.role() == expected_role
                && binding.selected_provider_plan_digest().is_none()
                && source_file.package_identity == Some(binding.package())
                && schema_source_file.package_identity == Some(binding.package())
                && typed.symbols.symbol_package_identity(schema) == Some(binding.package())
                && typed.symbols.display_path(schema, "::") == binding.declaration_path()
                && service_schema.trait_package_identity == Some(binding.package())
                && package_compilation::accepted_service_schema_digest(
                    binding.role(),
                    service_schema,
                ) == binding.normalized_schema_digest();
            if !exact_package_binding {
                return Err(Diagnostic::error(format!(
                    "target physical entry schema `{}` does not match the exact accepted {expected_role:?} package binding",
                    typed.symbols.display_path(schema, "::"),
                )));
            }
        }
        None => {
            let exact_bundled_source = source_file.package_identity.is_none()
                && schema_source_file.package_identity.is_none()
                && package_relative_source
                    == Some(std::path::Path::new(
                        expected_package.package_relative_source(),
                    ))
                && package_source_digest
                    == exact_bundled_physical_contract_package_source_digest(expected_package);
            if !exact_bundled_source {
                return Err(Diagnostic::error(format!(
                    "target physical entry requirement and schema `{}` require either the exact bundled {} contract or one accepted package-owned {} binding, not `{}`",
                    contract.requirement_identity,
                    expected_package.contract_name(),
                    expected_package.contract_name(),
                    source_file.path.display(),
                )));
            }
        }
    }
    let package_source_report_fingerprint = physical_contract_package_source_report_fingerprint(
        expected_package.manifest_identity().as_bytes(),
        source_file.source.as_bytes(),
    );
    Ok(TargetOwnedPhysicalContractSource {
        package: expected_package,
        package_source_digest,
        non_authoritative_package_source_report_fingerprint: package_source_report_fingerprint,
    })
}

pub use package_compilation::program_entry_semantic_binding_role;

/// Bundled-toolchain digest expected for one closed contract package.
fn exact_bundled_physical_contract_package_source_digest(
    package: target::ProgramEntryPhysicalContractPackage,
) -> program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest {
    match package {
        target::ProgramEntryPhysicalContractPackage::UefiX64 => {
            program_entry_plan::exact_uefi_x64_physical_contract_package_source_digest()
        }
        target::ProgramEntryPhysicalContractPackage::MacosArm64 => {
            program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest()
        }
        target::ProgramEntryPhysicalContractPackage::MacosX64 => {
            program_entry_plan::exact_macos_x86_64_physical_contract_package_source_digest()
        }
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64 => {
            program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest()
        }
        target::ProgramEntryPhysicalContractPackage::LinuxArm64 => {
            program_entry_plan::exact_linux_arm64_physical_contract_package_source_digest()
        }
        target::ProgramEntryPhysicalContractPackage::WindowsX64 => {
            program_entry_plan::exact_windows_x86_64_physical_contract_package_source_digest()
        }
    }
}

fn physical_contract_package_source_report_fingerprint(identity: &[u8], source: &[u8]) -> u64 {
    let mut hash = crate::fnv1a::Fnv1a::new();
    for bytes in [
        b"omega.program-entry-physical-package.v1".as_slice(),
        identity,
        source,
    ] {
        hash.bytes(bytes);
    }
    hash.finish()
}

struct ArrivalRequirementParameterType {
    identity: checked_trees::typed_trees::type_identity::NormalizedTypeIdentity,
    display: String,
    is_const: bool,
    is_mutable: bool,
}

struct ArrivalRequirementContract {
    signature: symbols::SymbolHandle,
    requirement_identity: String,
    parameters: Vec<ArrivalRequirementParameterType>,
    result_type_identity: Option<checked_trees::typed_trees::type_identity::NormalizedTypeIdentity>,
}

/// Source-shape queries run before package calling-plan selection. Their
/// visible storage parameters belong to core's semantic arrival, independently
/// of the eventual physical contract package and any local same-named trait.
fn core_arrival_requirement_contract(
    typed: &TypedTrees,
    requirement: &str,
) -> Result<ArrivalRequirementContract, Diagnostic> {
    let Some((owner, _)) = requirement.split_once("::") else {
        return Err(Diagnostic::error(
            "target semantic arrival has no requirement owner",
        ));
    };
    let mut declarations = typed
        .symbols
        .source_files()
        .filter(|file| {
            file.origin == source::SourceOrigin::Toolchain
                && file.dependency_scope == source::DependencyScope::Product
                && file.path.strip_prefix(&file.package_root).ok()
                    == Some(std::path::Path::new("extent.omg"))
        })
        .flat_map(|file| {
            typed.traits().iter().filter(move |definition| {
                typed
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .is_some_and(|span| span.source_id == file.source_id)
                    && definition.name.as_str() == owner
            })
        });
    let Some(definition) = declarations.next() else {
        return Err(Diagnostic::error(format!(
            "target semantic arrival `{requirement}` has no exact core declaration"
        )));
    };
    if declarations.next().is_some() {
        return Err(Diagnostic::error(format!(
            "target semantic arrival `{requirement}` has multiple core declaration instances"
        )));
    }
    arrival_requirement_contract(typed, definition, requirement)
}

/// Resolve the target declaration through the selected schema's composition.
/// The result is deliberately taken from Psi's normalized identities rather
/// than reconstructed from display strings in the Omega orchestrator.
fn arrival_requirement_contract(
    typed: &TypedTrees,
    schema: &checked_trees::typed_trees::trait_definition::TraitDefinition,
    requirement: &str,
) -> Result<ArrivalRequirementContract, Diagnostic> {
    let Some((owner, method)) = requirement.split_once("::") else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` is not an exact `Trait::machine` identity"
        )));
    };
    let mut inherited = vec![schema];
    let mut position = 0;
    while let Some(definition) = inherited.get(position).copied() {
        for parent in typed.trait_requirements(definition) {
            if inherited
                .iter()
                .any(|candidate| candidate.symbol == parent.symbol)
            {
                continue;
            }
            if let Some(parent) = typed.trait_definition_by_symbol(parent.symbol) {
                inherited.push(parent);
            }
        }
        position += 1;
    }
    let definitions = inherited
        .into_iter()
        .filter(|definition| definition.is_boundary && definition.name.as_str() == owner)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` resolves to {} boundary trait declarations in the selected schema instead of exactly one",
            definitions.len()
        )));
    };
    let signatures = typed
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| signature.name.as_str() == method)
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "target entry arrival requirement `{requirement}` resolves to {} machine declarations instead of exactly one",
            signatures.len()
        )));
    };
    Ok(ArrivalRequirementContract {
        signature: signature.symbol,
        requirement_identity: typed.trait_requirement_callable_identity_text(definition, signature),
        parameters: typed
            .state_signature_parameters(signature)
            .iter()
            .map(|parameter| ArrivalRequirementParameterType {
                identity: typed.normalized_type_identity(parameter.type_reference),
                display: typed.display_type_reference_with_constraints(parameter.type_reference),
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
            })
            .collect(),
        result_type_identity: signature
            .return_type
            .is_valid()
            .then(|| typed.normalized_type_identity(signature.return_type)),
    })
}

/// The entry's arrival-visible parameters: the roster minus the provisioned
/// context parameter and any placed-view custody rows, which lend custody
/// rather than arrive as parameters.
fn visible_entry_parameters<'program>(
    typed: &TypedTrees,
    parameters: &'program [checked_trees::typed_trees::signature::StateParameter],
    context: &Option<ProvisionedEntryContext<'program>>,
) -> Vec<&'program checked_trees::typed_trees::signature::StateParameter> {
    parameters
        .iter()
        .filter(|parameter| {
            Some(parameter.symbol) != context.as_ref().map(|context| context.parameter.symbol)
        })
        .filter(|parameter| {
            typed
                .placed_view_plan_for_input_type_reference(parameter.type_reference)
                .is_none()
        })
        .collect::<Vec<_>>()
}
