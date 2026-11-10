/*
Top-level Unit machine plan dispatcher (`lower_unit_plan_machine`), nominal and partial affine
cleanup machines (`unit_cleanup`), dynamic dispatch tables, crash interfaces, and runtime
requirements.
*/

pub(crate) use crate::attached_unit::inner as attached_unit;

pub(crate) use std::collections::{BTreeMap, BTreeSet};

pub(crate) use checked_trees::types::PrimitiveType;
pub(crate) use checked_trees::{
    CheckedBooleanExpression, CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedComposedUnitControlTerminatorPlan, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedTerminalSignatureEligibility,
    CheckedTrees, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEntryClaimPlan, CheckedUnitPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypeShape,
};
pub(crate) use language_semantics::{
    CarryPolicy, Multiplicity, PermissionClaimIdentity, SemanticDomainId, ServiceReachId,
    ServiceReachInterface, ServiceReachPlan, ServiceReachRowId, ServiceReachSummary,
};
pub(crate) use lowered_psi::LoweredPsi;
pub(crate) use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, DomainSemanticId, IntegerType, MachineId, ObligationId,
    PlaceId, Proposition, QualifiedScalarType, ScalarTerm, ScalarType, ServiceId, StructuralCaseId,
    StructuralDomainId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId, ValueId,
};
pub(crate) use terminal_psi::{
    Block, BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryStructuralResultDeclaration,
    ContractClause, EntryClaim, InstallationReachDependency, MachineContract, NominalAffineCleanup,
    Operation, OperationKind, OperationResult, ProgramLocalRootIntroductionSchema,
    ProviderCandidateConformance, ProviderParameterRefinement, ProviderRefinement,
    ProviderSignature, ProviderSignatureParameter, ServiceDeclaration, StructuralAccess,
    StructuralArgument, StructuralCaseDeclaration, StructuralCaseSuccessorEdge,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, SuccessorEdge, TerminalMachine,
    TerminalMachineResult, TerminalModule, TerminalRankedScc, Terminator, ValueDeclaration,
    VocabularyMarker, program_local_root_introduction_compatibility_report_identity,
};

pub(crate) use crate::emission::boolean_control::{
    boolean_decision_block_count, emit_reserved_boolean_tuple_stage_blocks,
    lower_boolean_value_decision,
};
pub(crate) use crate::emission::expression_validation::{
    contains_short_circuit, direct_expression_contains_short_circuit,
    validate_direct_parameter_types,
};
pub(crate) use crate::emission::operation_emission::{
    emit_boolean_expression, emit_direct_expression, emit_scalar_binding,
};
pub(crate) use crate::emission::scalar_types::{
    integer_landing_scalar_type, integer_scalar_type, integer_value, terminal_scalar_type,
};
pub(crate) use crate::proofs::content_conservation;
pub(crate) use crate::proofs::content_conservation::lower_boundary_content_guarantees;
pub(crate) use crate::proofs::crash_routes::{
    lower_boundary_crash_routes, lower_checked_crash_route_buckets,
    lower_structural_crash_route_buckets, substitute_structural_crash_route_roots,
};
pub(crate) use crate::retention::access_view_inputs::lower_access_view_input;
pub(crate) use crate::scalar_lowering::scalar_carriers;
pub(crate) use crate::shared::lowering_error::{LoweringError, unsupported};
pub(crate) use crate::shared::producer_result::{
    DebugPublication, LoweredSelectedMachine, LoweringCompletion,
};
pub(crate) use crate::shared::structural_types::{
    lower_mixed_cases, lower_mixed_fields, terminal_byte_sequence_carrier,
};
pub(crate) use crate::shared::terminal_identities::{
    TERMINAL_UNIT_CALL_OBLIGATION_BASE, allocate_dense, block_id, boundary_machine_id, claim_id,
    contract_id, dense_identity, edge_id, lookup_claim_id, lookup_domain_id, lookup_machine_id,
    lookup_service_id, lookup_type_id, machine_id, obligation_id, place_id, service_id,
    structural_domain_id, structural_field_id, structural_type_id, value_id,
};
pub(crate) use crate::unit::attached_unit::{lower_unit_closure, lower_unit_effect_closure};
pub(crate) use crate::unit::unit_cleanup::lower_nominal_affine_unit_cleanup_machine;

pub(crate) mod crash_interface {

    use std::collections::BTreeMap;

    use checked_trees::{
        CheckedBooleanExpression, CheckedScalarExpression, CheckedTrees, CrashInterface,
        CrashRouteBucket, CrashRouteGuard,
    };

    use crate::shared::lowering_error::LoweringError;

    pub(crate) fn effective_crash_routes(
        checked: &CheckedTrees,
        machine: symbols::SymbolHandle,
    ) -> Result<Vec<CrashRouteBucket>, LoweringError> {
        let Some(plan) = checked.facts.contract_plans.for_machine(machine) else {
            return Ok(Vec::new());
        };
        if plan.crash.interface() == CrashInterface::PublishedCeiling {
            return Ok(plan.crash.published().to_vec());
        }
        let mut grouped = BTreeMap::<checked_trees::CrashCause, Vec<CrashRouteGuard>>::new();
        for site in plan.crash.checked_sites() {
            grouped.insert(site.cause(), vec![CrashRouteGuard::Truth]);
        }
        if plan.crash.has_possible_trapping_site() {
            grouped.insert(
                checked_trees::CrashCause::Trap,
                vec![CrashRouteGuard::Truth],
            );
        }
        for call in plan.crash.checked_calls() {
            contribute(&mut grouped, call.surviving_buckets());
        }
        for operator in plan.crash.checked_operators() {
            contribute(&mut grouped, operator.surviving());
        }
        Ok(grouped
            .into_iter()
            .filter_map(|(cause, guards)| CrashRouteBucket::new(cause, guards))
            .collect())
    }

    fn contribute(
        grouped: &mut BTreeMap<checked_trees::CrashCause, Vec<CrashRouteGuard>>,
        buckets: &[CrashRouteBucket],
    ) {
        for bucket in buckets {
            let guards = grouped.entry(bucket.cause()).or_default();
            for guard in bucket.alternative_guards() {
                match guard {
                    CrashRouteGuard::Truth => {
                        *guards = vec![CrashRouteGuard::Truth];
                    }
                    CrashRouteGuard::Predicate(predicate) => match predicate.scalar_expression() {
                        Some(CheckedBooleanExpression::Constant(false)) => {}
                        Some(CheckedBooleanExpression::Constant(true)) => {
                            *guards = vec![CrashRouteGuard::Truth];
                        }
                        Some(expression) if contract_scoped(expression) => {
                            if *guards != [CrashRouteGuard::Truth] {
                                guards.push(guard.clone());
                            }
                        }
                        _ => {
                            *guards = vec![CrashRouteGuard::Truth];
                        }
                    },
                }
                if *guards == [CrashRouteGuard::Truth] {
                    break;
                }
            }
        }
    }

    fn contract_scoped(expression: &CheckedBooleanExpression) -> bool {
        match expression {
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::Local { .. } => false,
            CheckedBooleanExpression::ByteSequencePredicate { root, .. } => {
                root.parameter().is_some()
            }
            CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::StructuralIdentity { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => true,
            CheckedBooleanExpression::Not(operand) => contract_scoped(operand),
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                contract_scoped(left) && contract_scoped(right)
            }
            CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
                let validate = |operand: &CheckedScalarExpression| {
                    if contract_scoped_scalar(operand) {
                        Ok(())
                    } else {
                        Err(LoweringError::Unsupported(
                            "mathematical guard operand is outside the contract namespace",
                        ))
                    }
                };
                crate::proofs::contract_predicates::validate_math_namespace(left, &validate).is_ok()
                    && crate::proofs::contract_predicates::validate_math_namespace(right, &validate)
                        .is_ok()
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. }
            | CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
                contract_scoped_scalar(left) && contract_scoped_scalar(right)
            }
        }
    }

    fn contract_scoped_scalar(expression: &CheckedScalarExpression) -> bool {
        match expression {
            CheckedScalarExpression::StorageRead { .. }
            | CheckedScalarExpression::Local { .. }
            | CheckedScalarExpression::ImplementationSelection { .. } => false,
            CheckedScalarExpression::RepresentationView {
                primitive_type,
                operand,
            } => {
                primitive_type.is_fixed_integer()
                    && operand.primitive_type().is_some_and(|source| {
                        source.is_fixed_integer()
                            && source.scalar_byte_size() == primitive_type.scalar_byte_size()
                    })
                    && contract_scoped_scalar(operand)
            }
            CheckedScalarExpression::ConstantApplication { .. }
            | CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::ErasedParameter { .. }
            | CheckedScalarExpression::StructuralParameterField { .. }
            | CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. } => true,
            CheckedScalarExpression::StructuralSequenceLength { root, path } => {
                root.parameter().is_some()
                    && path.iter().all(|step| match step {
                        checked_trees::CheckedScalarReadProjection::Static(_) => true,
                        checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                            contract_scoped_scalar(&selector.expression)
                        }
                    })
            }
            CheckedScalarExpression::StructuralParameterIndexedRead {
                index,
                element_path,
                ..
            } => {
                contract_scoped_scalar(&index.expression)
                    && element_path.iter().all(|step| match step {
                        checked_trees::CheckedScalarReadProjection::Static(_) => true,
                        checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) => {
                            contract_scoped_scalar(&selector.expression)
                        }
                    })
            }
            CheckedScalarExpression::IntegerBinary { left, right, .. } => {
                contract_scoped_scalar(left) && contract_scoped_scalar(right)
            }
            checked_trees::integer_widen_triple_pattern!(operand, ..) => {
                contract_scoped_scalar(operand)
            }
            CheckedScalarExpression::Boolean(expression) => contract_scoped(expression),
        }
    }
}
pub(crate) mod dynamic_dispatch {

    use crate::LoweringError;
    use crate::emission::scalar_types::terminal_scalar_type;
    use crate::expression_preparation::bindings::structural_paths::ProjectionRoot;
    use crate::expression_preparation::bindings::structural_paths::terminal_access;
    use crate::shared::checked_facts::flow_facts;
    use crate::shared::terminal_identities::lookup_domain_id_for_carrier;
    use crate::shared::terminal_identities::lookup_type_id;
    use crate::shared::terminal_identities::structural_multiplicity;
    use checked_trees::{CheckedDynamicSlotPlan, CheckedTrees};
    use symbols::SymbolHandle;
    use terminal_psi::{
        TerminalDynamicClaimPath, TerminalDynamicDispatchCatalog, TerminalDynamicInterface,
        TerminalDynamicSlot, TerminalDynamicStructuralFormal, TerminalDynamicTable, TerminalModule,
    };

    #[derive(Debug, Clone, Default)]
    pub(crate) struct DynamicDispatchIndex {
        interfaces: Vec<typed_trees::type_identity::TypeIdentityId>,
        tables: Vec<(typed_trees::type_identity::TypeIdentityId, SymbolHandle)>,
        slots: Vec<Vec<(typed_trees::type_identity::CallableIdentityId, Vec<String>)>>,
    }

    impl DynamicDispatchIndex {
        pub(crate) fn new(checked: &CheckedTrees) -> Self {
            let selections = &checked.facts.dynamic_conformances.selections;
            let slot_plans = &checked.facts.flow.terminal_unit_effects.dynamic_slots;
            let mut interfaces = selections
                .iter()
                .map(|selection| selection.interface_identity)
                .chain(slot_plans.iter().map(|plan| plan.interface_identity))
                .collect::<Vec<_>>();
            interfaces.sort();
            interfaces.dedup();
            let mut tables = selections
                .iter()
                .map(|selection| (selection.interface_identity, selection.implementation))
                .collect::<Vec<_>>();
            tables.sort_by(|(left, left_symbol), (right, right_symbol)| {
                left.cmp(right)
                    .then(left_symbol.arena_index().cmp(&right_symbol.arena_index()))
            });
            tables.dedup();
            let slots = interfaces
                .iter()
                .map(|interface| {
                    match selections
                        .iter()
                        .find(|selection| &selection.interface_identity == interface)
                    {
                        Some(selection) => selection
                            .rows
                            .iter()
                            .map(|row| {
                                (row.requirement_identity, row.family_tuple.as_ref().to_vec())
                            })
                            .collect(),
                        None => {
                            let mut called = slot_plans
                                .iter()
                                .filter(|plan| &plan.interface_identity == interface)
                                .map(|plan| (plan.requirement_identity, plan.family_tuple.clone()))
                                .collect::<Vec<_>>();
                            called.sort();
                            called.dedup();
                            let concrete = called
                                .iter()
                                .filter(|(_, tuple)| !tuple.is_empty())
                                .map(|(identity, _)| *identity)
                                .collect::<Vec<_>>();
                            called.retain(|(identity, tuple)| {
                                !tuple.is_empty() || !concrete.contains(identity)
                            });
                            called
                        }
                    }
                })
                .collect();
            Self {
                interfaces,
                tables,
                slots,
            }
        }

        pub(crate) fn interface(
            &self,
            identity: &typed_trees::type_identity::TypeIdentityId,
        ) -> Option<u32> {
            let position = self.interfaces.binary_search(identity).ok()?;
            u32::try_from(position).ok()
        }

        pub(crate) fn table(
            &self,
            interface: &typed_trees::type_identity::TypeIdentityId,
            implementation: SymbolHandle,
        ) -> Option<u32> {
            let position = self
                .tables
                .binary_search_by(|(candidate, symbol)| {
                    candidate
                        .cmp(interface)
                        .then(symbol.arena_index().cmp(&implementation.arena_index()))
                })
                .ok()?;
            u32::try_from(position).ok()
        }

        pub(crate) fn interface_slots(
            &self,
            interface: u32,
        ) -> Option<&[(typed_trees::type_identity::CallableIdentityId, Vec<String>)]> {
            self.slots
                .get(usize::try_from(interface).ok()?)
                .map(Vec::as_slice)
        }

        pub(crate) fn slot(
            &self,
            interface: u32,
            requirement_identity: &typed_trees::type_identity::CallableIdentityId,
        ) -> Option<u32> {
            let slots = self.slots.get(usize::try_from(interface).ok()?)?;
            let position = slots
                .iter()
                .position(|(candidate, _)| candidate == requirement_identity)?;
            u32::try_from(position).ok()
        }

        pub(crate) fn resolve_slot(
            &self,
            interface_identity: &typed_trees::type_identity::TypeIdentityId,
            requirement_identity: &typed_trees::type_identity::CallableIdentityId,
        ) -> Result<(u32, u32), LoweringError> {
            let interface =
                self.interface(interface_identity)
                    .ok_or(LoweringError::Unsupported(
                        "selector call names an interface no implementation is selected for",
                    ))?;
            let slot =
                self.slot(interface, requirement_identity)
                    .ok_or(LoweringError::Unsupported(
                        "selector call names a requirement absent from its interface",
                    ))?;
            Ok((interface, slot))
        }

        pub(crate) fn resolve_tuple(
            &self,
            interface_identity: &typed_trees::type_identity::TypeIdentityId,
            requirement_identity: &typed_trees::type_identity::CallableIdentityId,
            tuple: &[String],
        ) -> Result<(u32, u32), LoweringError> {
            let interface =
                self.interface(interface_identity)
                    .ok_or(LoweringError::Unsupported(
                        "static family call names no interface",
                    ))?;
            let position = self
                .interface_slots(interface)
                .and_then(|slots| {
                    slots.iter().position(|(requirement, candidate)| {
                        requirement == requirement_identity && candidate == tuple
                    })
                })
                .ok_or(LoweringError::Unsupported(
                    "static family call names no exact tuple",
                ))?;
            Ok((
                interface,
                u32::try_from(position)
                    .map_err(|_| LoweringError::Unsupported("static family slot overflows"))?,
            ))
        }

        pub(crate) fn family_roster(
            &self,
            interface: u32,
            slot: u32,
            requirement_identity: &typed_trees::type_identity::CallableIdentityId,
        ) -> Result<Vec<Vec<String>>, LoweringError> {
            let slots = self
                .interface_slots(interface)
                .ok_or(LoweringError::Unsupported(
                    "selector call names an interface with no slot roster",
                ))?;
            let base = usize::try_from(slot)
                .map_err(|_| LoweringError::Unsupported("dynamic family slot overflows"))?;
            Ok(slots
                .iter()
                .skip(base)
                .take_while(|(requirement, tuple)| {
                    requirement == requirement_identity && !tuple.is_empty()
                })
                .map(|(_, tuple)| tuple.clone())
                .collect())
        }
    }

    pub(crate) fn lower_dynamic_dispatch_catalog(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        source_machines: &[SymbolHandle],
        module: &mut TerminalModule,
    ) -> Result<(), LoweringError> {
        let index = DynamicDispatchIndex::new(checked);
        if index.interfaces.is_empty() {
            module.dynamic_dispatch = TerminalDynamicDispatchCatalog::default();
            return Ok(());
        }
        if source_machines.len() != module.machines.len() {
            return unsupported("dynamic dispatch rows need the closure's source owners");
        }
        let owners = source_machines
            .iter()
            .zip(&module.machines)
            .map(|(source, machine)| (*source, machine.id))
            .collect::<Vec<_>>();
        let type_ids = module
            .structural_types
            .iter()
            .map(|declaration| {
                (
                    checked.type_identity_by_text(
                        module
                            .identities
                            .text(declaration.identity)
                            .unwrap_or_default(),
                    ),
                    declaration.id,
                )
            })
            .collect::<Vec<_>>();
        let domain_ids = module
            .structural_domains
            .iter()
            .map(|domain| {
                let semantic_domain = flow_facts(checked)
                    .terminal_unit_effects
                    .structural_domains
                    .iter()
                    .find(|plan| {
                        u64::from(plan.domain.0) == domain.semantic_domain.get()
                            && type_ids.iter().any(|(identity, carrier)| {
                                *identity == plan.carrier_type_identity
                                    && terminal_semantics::borrowed_referent_presentation(
                                        module.structural_types.iter(),
                                        *carrier,
                                        domain.carrier,
                                    ) == Some(
                                        terminal_semantics::BorrowedReferentPresentation::Unchanged,
                                    )
                            })
                    })
                    .map(|plan| plan.domain)
                    .ok_or(LoweringError::Unsupported(
                        "dynamic domain lost its exact source carrier",
                    ))?;
                Ok(
                    crate::shared::terminal_identities::StructuralDomainBinding {
                        byte_predicate: if domain.content_projection.is_none()
                            && domain.establishment_routes.is_empty()
                        {
                            domain.byte_predicate
                        } else {
                            None
                        },
                        semantic_domain,
                        carrier: domain.carrier,
                        domain: domain.id,
                    },
                )
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let slot_plans = &checked.facts.flow.terminal_unit_effects.dynamic_slots;
        let mut catalog = TerminalDynamicDispatchCatalog::default();
        for (interface_id, interface_identity) in index.interfaces.iter().enumerate() {
            let interface_id = u32::try_from(interface_id)
                .map_err(|_| LoweringError::Unsupported("dynamic interface id overflows"))?;
            let mut slots = Vec::new();
            for (slot, (requirement_identity, family_tuple)) in
                index.slots[interface_id as usize].iter().enumerate()
            {
                let matching = slot_plans.iter().filter(|plan| {
                    plan.interface_identity == *interface_identity
                        && plan.requirement_identity == *requirement_identity
                        && plan.family_tuple == *family_tuple
                });
                let mut published_contract: Option<(&CheckedDynamicSlotPlan, TerminalDynamicSlot)> =
                    None;
                for plan in matching {
                    let lowered = lower_slot(
                        identities,
                        checked,
                        plan,
                        u32::try_from(slot)
                            .map_err(|_| LoweringError::Unsupported("dynamic slot id overflows"))?,
                        family_tuple,
                        &type_ids,
                        &module.structural_types,
                        &domain_ids,
                        &module.scalar_qualifications,
                    )?;
                    if let Some((previous_plan, previous_slot)) = &published_contract {
                        if !same_checked_slot_contract(previous_plan, plan)
                            || previous_slot != &lowered
                        {
                            return unsupported(
                                "dynamic family tuple has conflicting public call contracts",
                            );
                        }
                    } else {
                        published_contract = Some((plan, lowered));
                    }
                }
                slots.push(published_contract.map(|(_, slot)| slot).ok_or(
                    LoweringError::Unsupported(
                        "dynamic family tuple has no checked public call shape",
                    ),
                )?);
            }
            catalog.interfaces.push(TerminalDynamicInterface {
                id: interface_id,
                identity: identities.intern(
                    &checked
                        .type_identity_text(*interface_identity)
                        .unwrap_or_default(),
                ),
                slots,
            });
        }
        for (table_id, (interface_identity, implementation)) in index.tables.iter().enumerate() {
            let table_id = u32::try_from(table_id)
                .map_err(|_| LoweringError::Unsupported("dynamic table id overflows"))?;
            let interface =
                index
                    .interface(interface_identity)
                    .ok_or(LoweringError::Unsupported(
                        "dynamic table names no interface",
                    ))?;
            let selections = checked
                .facts
                .dynamic_conformances
                .selections
                .iter()
                .filter(|selection| {
                    &selection.interface_identity == interface_identity
                        && selection.implementation == *implementation
                })
                .collect::<Vec<_>>();
            let Some(selection) = selections.first() else {
                return unsupported("dynamic table lost its selection");
            };
            let selected_here = selections
                .iter()
                .any(|selection| source_machines.contains(&selection.machine));
            let slots = &catalog.interfaces[interface as usize].slots;
            if selection.rows.len() != slots.len() {
                return unsupported("implementation rows disagree with their interface slots");
            }
            let mut rows = Vec::with_capacity(slots.len());
            for (row, slot) in selection.rows.iter().zip(slots) {
                if identities.intern(
                    &checked
                        .typed
                        .callable_identity_text(row.requirement_identity),
                ) != slot.requirement_identity
                {
                    return unsupported("implementation row is out of its interface's slot order");
                }
                if !selected_here {
                    rows.push(None);
                    continue;
                }
                let id = crate::shared::terminal_identities::lookup_machine_id(
                    &owners,
                    row.realization_machine,
                )?;
                let realization = module.machine(id).ok_or(LoweringError::Unsupported(
                    "dynamic table row names a machine outside the lowered closure",
                ))?;
                if !realizes_slot(realization, slot, &module.structural_types) {
                    return unsupported(
                        "implementation row does not realize its slot's call shape",
                    );
                }
                rows.push(Some(id));
            }
            catalog.tables.push(TerminalDynamicTable {
                id: table_id,
                interface,
                implementation_identity: identities
                    .intern(&checked.symbols.display_path(*implementation, "::")),
                rows,
            });
        }
        module.dynamic_dispatch = catalog;
        Ok(())
    }

    fn same_checked_slot_contract(
        left: &CheckedDynamicSlotPlan,
        right: &CheckedDynamicSlotPlan,
    ) -> bool {
        let CheckedDynamicSlotPlan {
            slot: _,
            interface_identity,
            declaring_trait,
            requirement,
            requirement_identity,
            family_tuple,
            structural_parameters,
            entry_claim_paths,
            scalar_parameters,
            result,
            result_claim_paths,
            result_reference_sources,
        } = left;
        (
            interface_identity,
            declaring_trait,
            requirement,
            requirement_identity,
            family_tuple,
            structural_parameters,
            entry_claim_paths,
            scalar_parameters,
            result,
            result_claim_paths,
            result_reference_sources,
        ) == (
            &right.interface_identity,
            &right.declaring_trait,
            &right.requirement,
            &right.requirement_identity,
            &right.family_tuple,
            &right.structural_parameters,
            &right.entry_claim_paths,
            &right.scalar_parameters,
            &right.result,
            &right.result_claim_paths,
            &right.result_reference_sources,
        )
    }

    fn lower_slot(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        plan: &CheckedDynamicSlotPlan,
        slot: u32,
        family_tuple: &[String],
        type_ids: &[(
            typed_trees::type_identity::TypeIdentityId,
            semantic_vocabulary::StructuralTypeId,
        )],
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
        domain_ids: &[crate::shared::terminal_identities::StructuralDomainBinding],
        scalar_qualifications: &terminal_psi::ScalarQualificationCatalog,
    ) -> Result<TerminalDynamicSlot, LoweringError> {
        let Some((selector, scalars)) = plan.scalar_parameters.split_first() else {
            return unsupported("dynamic slot lost its selector operand");
        };
        if selector.source_position != 0 {
            return unsupported("dynamic slot selector is not its first operand");
        }
        let scalar_parameters = scalars
            .iter()
            .map(|parameter| terminal_scalar_type(parameter.primitive_type))
            .collect::<Result<Vec<_>, _>>()?;
        let structural_parameters =
            plan.structural_parameters
                .iter()
                .enumerate()
                .map(|(position, parameter)| {
                    if parameter.position == 0 {
                        return unsupported("dynamic slot structural formal precedes its selector");
                    }
                    let structural_type = lookup_type_id(type_ids, &parameter.type_identity)?;
                    let qualifications = parameter
                        .qualifications
                        .iter()
                        .map(|domain| {
                            lookup_domain_id_for_carrier(domain_ids, *domain, structural_type)
                                .or_else(|_| {
                                    crate::shared::terminal_identities::lookup_domain_id(
                                        domain_ids, *domain,
                                    )
                                })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(TerminalDynamicStructuralFormal {
                        position: u32::try_from(position).map_err(|_| {
                            LoweringError::Unsupported(
                                "dynamic slot structural parameter count exceeds u32",
                            )
                        })?,
                        structural_type,
                        access: terminal_access(parameter.access),
                        multiplicity: structural_multiplicity(parameter.multiplicity),
                        qualifications,
                    })
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
        let mut entry_claim_paths = plan.entry_claim_paths.iter().map(|claim| {
            let formal = structural_parameters.get(claim.argument_index as usize).ok_or(
                LoweringError::Unsupported("dynamic slot claim has no structural formal"),
            )?;
            Ok(TerminalDynamicClaimPath {
                argument_index: claim.argument_index,
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(identities, &claim.path, formal.structural_type, structural_types)?,
            })
        }).collect::<Result<Vec<_>, LoweringError>>()?;
        entry_claim_paths.sort();
        let result = super::attached_unit::lower_boundary_result(
            identities,
            checked,
            plan.slot,
            scalar_qualifications,
            &plan.result,
            type_ids,
            structural_types,
            domain_ids,
        )?;
        let mut result_claim_paths = match &result {
            terminal_psi::BoundaryMachineResult::Structural(result) => plan.result_claim_paths.iter()
                .map(|path| crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities, path, result.structural_type, structural_types,
                )).collect::<Result<Vec<_>, _>>()?,
            _ if plan.result_claim_paths.is_empty() => Vec::new(),
            _ => return unsupported("nonstructural dynamic result carries claims"),
        };
        result_claim_paths.sort();
        let result_reference_regions = lower_result_reference_regions(
            identities,
            checked,
            &plan.result_reference_sources,
            &result,
            &structural_parameters
                .iter()
                .map(|p| (p.structural_type, p.access))
                .collect::<Vec<_>>(),
            type_ids,
            structural_types,
        )?;
        Ok(TerminalDynamicSlot {
            slot,
            declaring_trait_identity: identities
                .intern(&checked.symbols.display_path(plan.declaring_trait, "::")),
            requirement_identity: identities.intern(
                &checked
                    .typed
                    .callable_identity_text(plan.requirement_identity),
            ),
            family_tuple: family_tuple.to_vec(),
            scalar_parameters,
            structural_parameters,
            entry_claim_paths,
            result,
            result_claim_paths,
            result_reference_regions,
        })
    }

    fn realizes_slot(
        machine: &terminal_psi::TerminalMachine,
        slot: &TerminalDynamicSlot,
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> bool {
        let scalars_match = machine.parameters.len() == slot.scalar_parameters.len()
            && machine
                .parameters
                .iter()
                .zip(&slot.scalar_parameters)
                .all(|(parameter, expected)| parameter.scalar_type == *expected);
        let structurals_match = machine.structural_parameters.len()
            == slot.structural_parameters.len()
            && machine
                .structural_parameters
                .iter()
                .zip(&slot.structural_parameters)
                .all(|(parameter, expected)| {
                    parameter.position == expected.position
                        && parameter.structural_type == expected.structural_type
                        && parameter.access == expected.access
                        && parameter.multiplicity == expected.multiplicity
                });
        let result_matches = slot.result.matches_machine_result(&machine.result)
            && machine.result.structural().is_none_or(|result| {
                terminal_semantics::reference_result_refines_regions(
                    structural_types.iter(),
                    &machine.structural_parameters,
                    result,
                    &slot.result_reference_regions,
                )
            })
            && terminal_semantics::result_claim_paths_match_returns(
                &slot.result_claim_paths,
                machine
                    .blocks
                    .iter()
                    .filter_map(|block| match &block.terminator {
                        terminal_psi::Terminator::ReturnStructural {
                            returned_claims, ..
                        } => Some(returned_claims.as_slice()),
                        _ => None,
                    }),
            );
        let Some(mut entry_claim_paths) = machine
            .entry_claims
            .iter()
            .map(|claim| {
                let argument_index = machine
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.place == claim.input)?;
                Some(TerminalDynamicClaimPath {
                    argument_index: u32::try_from(argument_index).ok()?,
                    path: claim.path.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        entry_claim_paths.sort();
        scalars_match
            && structurals_match
            && result_matches
            && entry_claim_paths == slot.entry_claim_paths
    }

    fn unsupported<T>(reason: &'static str) -> Result<T, LoweringError> {
        Err(LoweringError::Unsupported(reason))
    }

    pub(crate) fn lower_result_reference_regions(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        sources: &[checked_trees::CheckedReturnedReferencePlan],
        result: &terminal_psi::BoundaryMachineResult,
        parameters: &[(
            semantic_vocabulary::StructuralTypeId,
            terminal_psi::StructuralAccess,
        )],
        type_ids: &[(
            typed_trees::type_identity::TypeIdentityId,
            semantic_vocabulary::StructuralTypeId,
        )],
        structural_types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<Vec<terminal_psi::TerminalDynamicReferenceRegion>, LoweringError> {
        let mut result_reference_regions = Vec::new();
        for reference in sources {
            if matches!(
                reference.source.root,
                checked_trees::CheckedReferenceOriginRoot::Parameter { .. }
            ) && reference.source.extent
                != semantic_vocabulary::ReferenceOriginExtent::WithinRegion
            {
                return unsupported("public result contract contains a private precise origin");
            }
            let terminal_psi::BoundaryMachineResult::Structural(result) = result else {
                return unsupported("nonstructural public result carries reference origins");
            };
            let (origin, root_type) = match reference.source.root {
                checked_trees::CheckedReferenceOriginRoot::Parameter { index } => {
                    let formal =
                        parameters
                            .get(index as usize)
                            .ok_or(LoweringError::Unsupported(
                                "public result region has no structural input",
                            ))?;
                    (
                        terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                            argument_index: index,
                        },
                        if reference.source.entry_reference_hops == 0 {
                            ProjectionRoot::Published(formal.0)
                        } else {
                            ProjectionRoot::BorrowedEntry {
                                published: formal.0,
                                access: formal.1,
                                reference_hops: reference.source.entry_reference_hops,
                            }
                        },
                    )
                }
                checked_trees::CheckedReferenceOriginRoot::Invocation { carrier_type } => {
                    let carrier_type = lookup_type_id(type_ids, &carrier_type)?;
                    (
                        terminal_psi::TerminalDynamicReferenceOrigin::Invocation { carrier_type },
                        {
                            if reference.source.entry_reference_hops != 0 {
                                return unsupported(
                                    "public invocation origin cannot erase a formal entry",
                                );
                            }
                            ProjectionRoot::Published(carrier_type)
                        },
                    )
                }
            };
            let source_path =
                crate::expression_preparation::bindings::structural_paths::lower_reference_origin_path(
                    identities,
                    checked,
                    &reference.source.path,
                    root_type,
                    terminal_access(reference.source.access),
                    structural_types,
                )?
                .into_iter()
                .map(|projection| match projection {
                    terminal_psi::ReferenceOriginProjection::Exact(segment) => Ok(segment),
                    terminal_psi::ReferenceOriginProjection::Element => {
                        unsupported("public result input region requires an exact supplying descriptor")
                    }
                })
                .collect::<Result<Vec<_>, LoweringError>>()?;
            result_reference_regions.push(terminal_psi::TerminalDynamicReferenceRegion {
                path: crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                    identities, &reference.path, result.structural_type, structural_types,
                )?,
                origin,
                source_path,
                access: terminal_access(reference.source.access),
            });
        }
        result_reference_regions.sort();
        result_reference_regions.dedup();
        Ok(result_reference_regions)
    }
}
pub(crate) use crash_interface::effective_crash_routes;
pub(crate) mod runtime_requirements {
    use super::{
        BTreeMap, CheckedBooleanExpression, CheckedScalarExpression, LoweringError, PrimitiveType,
        Proposition, ScalarTerm, ScalarType, StructuralParameterDeclaration,
        StructuralTypeDeclaration, ValueDeclaration, ValueId, integer_landing_scalar_type,
        integer_scalar_type, integer_value, terminal_scalar_type, unsupported,
    };
    use crate::proofs::contract_predicates::PredicateTerms;
    use crate::proofs::contract_predicates::validate_math_namespace;
    use crate::proofs::crash_routes::{
        checked_boolean_scalar_term, checked_scalar_term, lower_sequence_extent_projection_term,
        lower_structural_member_term,
    };
    use crate::scalar_lowering::contracts::validate_indexed_operands;

    pub(crate) mod source {
        use super::super::CheckedTrees;
        use super::{CheckedBooleanExpression, LoweringError, unsupported};
        use checked_trees::domain::ProofFact;
        use checked_trees::signature::SignatureContractKind;

        pub(crate) fn validate_runtime_memberships(
            checked: &CheckedTrees,
            machine_symbol: symbols::SymbolHandle,
        ) -> Result<(), LoweringError> {
            let machine = checked
                .machine(machine_symbol)
                .ok_or(LoweringError::Unsupported(
                    "runtime membership has no source machine",
                ))?;
            let state =
                checked
                    .machine_states(machine)
                    .first()
                    .ok_or(LoweringError::Unsupported(
                        "runtime membership has no source entry",
                    ))?;
            let crash = &checked
                .facts
                .contract_plans
                .for_machine(machine_symbol)
                .ok_or(LoweringError::Unsupported(
                    "runtime membership has no contract plan",
                ))?
                .crash;
            let mut retained = crash
                .structural_runtime_requirements()
                .ok_or(LoweringError::Unsupported(
                    "runtime membership has no complete requirement package",
                ))?
                .iter();
            let mut subjects = crash.runtime_membership_subjects().iter();
            let facts = checked
                .machine_contracts(machine)
                .iter()
                .map(|contract| (true, contract))
                .chain(
                    checked
                        .state_contracts(state)
                        .iter()
                        .map(|contract| (false, contract)),
                )
                .filter(|(_, contract)| contract.kind == SignatureContractKind::Requires)
                .flat_map(|(machine_owned, contract)| {
                    contract
                        .facts
                        .handles()
                        .map(move |fact| (machine_owned, contract, fact))
                });
            for (ordinal, (machine_owned, contract, fact_handle)) in facts.enumerate() {
                let fact = checked.proof_facts.get(fact_handle);
                if contract.binding.is_some() {
                    if !machine_owned
                        || fact.as_proposition().is_none()
                        || checked
                            .facts
                            .proof
                            .machine_requires_fact(&checked.typed, machine_symbol, fact_handle)
                            .is_none_or(|row| row.evidence_term.is_none())
                    {
                        return unsupported(
                            "runtime entry requirement lost its exact machine evidence lane",
                        );
                    }
                    continue;
                }
                if validation::restates_structural_parameter_qualification(
                    &checked.typed,
                    state,
                    fact,
                ) {
                    continue;
                }
                match fact {
                    ProofFact::Expression(_) => {
                        retained.next().ok_or(LoweringError::Unsupported(
                            "runtime requirement lost an authored expression",
                        ))?;
                    }
                    ProofFact::Membership(membership) => validate_membership_source(
                        checked,
                        membership,
                        u32::try_from(ordinal).map_err(|_| {
                            LoweringError::Unsupported("runtime requirement fact ordinal overflow")
                        })?,
                        &mut subjects,
                        &mut retained,
                    )?,
                    ProofFact::Proposition(_) => {
                        return unsupported(
                            "runtime membership predicate needs its evidence owner",
                        );
                    }
                }
            }
            if subjects.next().is_some() {
                return unsupported("runtime requirements retain an unauthored membership subject");
            }
            Ok(())
        }

        fn validate_membership_source<'clause>(
            checked: &CheckedTrees,
            membership: &checked_trees::domain::ProofMembershipFact,
            ordinal: u32,
            membership_subjects: &mut std::slice::Iter<
                'clause,
                checked_trees::CheckedRuntimeMembershipSubject,
            >,
            retained: &mut std::slice::Iter<'clause, CheckedBooleanExpression>,
        ) -> Result<(), LoweringError> {
            let witness = membership_subjects
                .next()
                .filter(|row| row.fact_ordinal == ordinal && row.membership == *membership)
                .ok_or(LoweringError::Unsupported(
                    "scalar entry membership lost its exact subject row",
                ))?;
            let _ = validation::scalar_membership_carrier(&checked.typed, membership).ok_or(
                LoweringError::Unsupported(
                    "scalar entry membership needs its declared theory evidence owner",
                ),
            )?;
            for _ in 0..witness.predicate_count {
                let actual = retained.next().ok_or(LoweringError::Unsupported(
                    "scalar entry membership lost a declared predicate",
                ))?;
                super::validate_namespace(actual)?;
            }
            Ok(())
        }
    }
    pub(crate) use source::validate_runtime_memberships;

    pub(crate) fn lower_structural_runtime_requirement(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        expression: &CheckedBooleanExpression,
        scalar_parameters: &[ValueDeclaration],
        erased: &[ValueDeclaration],
        parameters: &[StructuralParameterDeclaration],
        structural_types: &[StructuralTypeDeclaration],
    ) -> Result<Proposition, LoweringError> {
        validate_namespace(expression)?;
        crate::proofs::contract_predicates::proposition(
            identities,
            expression,
            &RuntimeRequirementTerms {
                scalar_parameters,
                erased,
                parameters,
                structural_types,
            },
        )
    }

    fn validate_namespace(expression: &CheckedBooleanExpression) -> Result<(), LoweringError> {
        match expression {
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::StructuralIdentity { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => Ok(()),
            CheckedBooleanExpression::Not(operand) => validate_namespace(operand),
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                validate_namespace(left)?;
                validate_namespace(right)
            }
            CheckedBooleanExpression::IntegerMathComparison { left, right, .. } => {
                validate_math_namespace(left, &validate_integer_namespace)?;
                validate_math_namespace(right, &validate_integer_namespace)
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                validate_integer_namespace(left)?;
                validate_integer_namespace(right)
            }
            CheckedBooleanExpression::ScalarIeeeFloatComparison { left, right, .. } => {
                validate_float_namespace(left)?;
                validate_float_namespace(right)
            }
            _ => unsupported(
                "runtime requirement is outside the immutable entry parameter namespace",
            ),
        }
    }

    fn validate_float_namespace(expression: &CheckedScalarExpression) -> Result<(), LoweringError> {
        match expression {
            CheckedScalarExpression::StructuralParameterIndexedRead {
                primitive_type: PrimitiveType::F32 | PrimitiveType::F64,
                ..
            } => validate_indexed_operands(expression, &validate_integer_namespace),

            CheckedScalarExpression::Parameter { primitive_type, .. }
            | CheckedScalarExpression::ErasedParameter { primitive_type, .. }
            | CheckedScalarExpression::StructuralParameterField { primitive_type, .. }
                if matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) =>
            {
                Ok(())
            }
            CheckedScalarExpression::IeeeFloatLiteral { .. } => Ok(()),
            _ => unsupported(
                "runtime IEEE requirement needs exact float entry parameters or literals",
            ),
        }
    }

    fn validate_integer_namespace(
        expression: &CheckedScalarExpression,
    ) -> Result<(), LoweringError> {
        match expression {
            CheckedScalarExpression::StructuralParameterIndexedRead { .. } => {
                validate_indexed_operands(expression, &validate_integer_namespace)
            }

            CheckedScalarExpression::Parameter { primitive_type, .. }
            | CheckedScalarExpression::ErasedParameter { primitive_type, .. }
            | CheckedScalarExpression::StructuralParameterField { primitive_type, .. } => {
                integer_scalar_type(*primitive_type).map(|_| ())
            }
            CheckedScalarExpression::IntegerLiteral { .. }
            | CheckedScalarExpression::IeeeFloatLiteral { .. } => Ok(()),
            CheckedScalarExpression::IntegerBinary {
                kind,
                primitive_type,
                left,
                right,
                ..
            } if matches!(
                kind,
                checked_trees::CheckedIntegerBinaryKind::WrappingAdd
                    | checked_trees::CheckedIntegerBinaryKind::WrappingSubtract
                    | checked_trees::CheckedIntegerBinaryKind::WrappingMultiply
                    | checked_trees::CheckedIntegerBinaryKind::SaturatingAdd
                    | checked_trees::CheckedIntegerBinaryKind::SaturatingSubtract
                    | checked_trees::CheckedIntegerBinaryKind::SaturatingMultiply
                    | checked_trees::CheckedIntegerBinaryKind::BitwiseAnd
                    | checked_trees::CheckedIntegerBinaryKind::BitwiseOr
                    | checked_trees::CheckedIntegerBinaryKind::BitwiseXor
                    | checked_trees::CheckedIntegerBinaryKind::WrappingShiftLeft
                    | checked_trees::CheckedIntegerBinaryKind::WrappingShiftRight
            ) && *primitive_type != PrimitiveType::Addr =>
            {
                integer_scalar_type(*primitive_type)?;
                validate_integer_namespace(left)?;
                validate_integer_namespace(right)
            }
            CheckedScalarExpression::IntegerBinary {
                primitive_type,
                left,
                right,
                ..
            } if *primitive_type != PrimitiveType::Addr => {
                integer_scalar_type(*primitive_type)?;
                validate_integer_namespace(left)?;
                validate_integer_namespace(right)
            }
            checked_trees::integer_widen_triple_pattern!(primitive_type, operand, ..)
                if *primitive_type != PrimitiveType::Addr =>
            {
                integer_scalar_type(*primitive_type)?;
                validate_integer_namespace(operand)
            }
            CheckedScalarExpression::Boolean(expression) => validate_namespace(expression),
            CheckedScalarExpression::StructuralSequenceLength { root, path } => {
                root.parameter().ok_or(LoweringError::Unsupported(
                    "runtime sequence extent requires a structural formal",
                ))?;
                for step in path {
                    if let checked_trees::CheckedScalarReadProjection::RuntimeIndex(selector) = step
                    {
                        validate_integer_namespace(&selector.expression)?;
                    }
                }
                Ok(())
            }
            _ => unsupported(
                "runtime requirements admit only fixed integer parameters, members, and literals",
            ),
        }
    }

    struct RuntimeRequirementTerms<'parameters> {
        scalar_parameters: &'parameters [ValueDeclaration],
        erased: &'parameters [ValueDeclaration],
        parameters: &'parameters [StructuralParameterDeclaration],
        structural_types: &'parameters [StructuralTypeDeclaration],
    }

    impl RuntimeRequirementTerms<'_> {
        fn scalar_leaf(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
        ) -> Result<ScalarTerm, LoweringError> {
            match expression {
                CheckedScalarExpression::StructuralParameterIndexedRead { .. } => {
                    crate::proofs::crash_routes::lower_structural_indexed_term(
                        identities,
                        expression,
                        self.parameters,
                        self.structural_types,
                        &mut |identities, selector| self.scalar(identities, selector),
                    )
                }

                CheckedScalarExpression::Parameter { primitive_type, .. }
                | CheckedScalarExpression::ErasedParameter { primitive_type, .. } => {
                    if !matches!(
                        primitive_type,
                        typed_trees::types::PrimitiveType::F32
                            | typed_trees::types::PrimitiveType::F64
                    ) {
                        integer_scalar_type(*primitive_type)?;
                    }
                    checked_scalar_term(identities, expression, self.scalar_parameters, self.erased)
                }
                CheckedScalarExpression::IeeeFloatLiteral { value } => {
                    Ok(ScalarTerm::IeeeFloatLiteral(*value))
                }
                CheckedScalarExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    primitive_type,
                    ..
                } => lower_structural_member_term(
                    identities,
                    *parameter_position,
                    path,
                    terminal_scalar_type(*primitive_type)?,
                    self.parameters,
                    self.structural_types,
                ),
                CheckedScalarExpression::StructuralSequenceLength { root, path } => {
                    let position = root.parameter().ok_or(LoweringError::Unsupported(
                        "runtime sequence extent requires a structural formal",
                    ))?;
                    lower_sequence_extent_projection_term(
                        identities,
                        position,
                        path,
                        self.parameters,
                        self.structural_types,
                        &mut |identities, selector| self.scalar(identities, selector),
                    )
                }
                CheckedScalarExpression::IntegerLiteral { literal } => {
                    let scalar_type = integer_landing_scalar_type(literal)?;
                    let Some(integer_type) = scalar_type.as_integer() else {
                        return unsupported("runtime requirement literal is not an integer");
                    };
                    ScalarTerm::integer(integer_type, integer_value(literal, scalar_type)?)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                _ => unsupported(
                    "runtime requirement integer operand is outside the entry namespace",
                ),
            }
        }
    }

    impl PredicateTerms for RuntimeRequirementTerms<'_> {
        fn structural_identity(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            left: &checked_trees::CheckedStructuralParameterField,
            right: &checked_trees::CheckedStructuralParameterField,
        ) -> Result<Proposition, LoweringError> {
            crate::proofs::crash_routes::lower_structural_identity(
                identities,
                left,
                right,
                self.parameters,
                self.structural_types,
            )
        }

        fn case_membership(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
        ) -> Result<Proposition, LoweringError> {
            let CheckedBooleanExpression::StructuralCaseMembership { subject, case } = expression
            else {
                return unsupported("runtime case requirement has no case subject");
            };
            crate::proofs::crash_routes::lower_structural_case_membership(
                identities,
                subject,
                case,
                self.parameters,
                None,
                self.structural_types,
            )
        }
        fn scalar(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedScalarExpression,
        ) -> Result<ScalarTerm, LoweringError> {
            crate::proofs::integer_predicates::lower(expression, &[], &mut |leaf| {
                self.scalar_leaf(identities, leaf)
            })
        }

        fn boolean(
            &self,
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            expression: &CheckedBooleanExpression,
        ) -> Result<ScalarTerm, LoweringError> {
            match expression {
                CheckedBooleanExpression::Not(operand) => {
                    ScalarTerm::boolean_not(self.boolean(identities, operand)?)
                        .map_err(LoweringError::InvalidCrashPredicate)
                }
                CheckedBooleanExpression::Equal { left, right } => ScalarTerm::boolean_equal(
                    self.boolean(identities, left)?,
                    self.boolean(identities, right)?,
                )
                .map_err(LoweringError::InvalidCrashPredicate),
                CheckedBooleanExpression::Constant(_)
                | CheckedBooleanExpression::Parameter { .. }
                | CheckedBooleanExpression::ErasedParameter { .. } => checked_boolean_scalar_term(
                    identities,
                    expression,
                    self.scalar_parameters,
                    self.erased,
                ),
                CheckedBooleanExpression::StructuralParameterField {
                    parameter_position,
                    path,
                    ..
                } => lower_structural_member_term(
                    identities,
                    *parameter_position,
                    path,
                    ScalarType::Boolean,
                    self.parameters,
                    self.structural_types,
                ),
                _ => unsupported(
                    "runtime requirement Boolean operand is outside the entry namespace",
                ),
            }
        }
    }

    pub(crate) fn substitute_runtime_requirement_scalar_values(
        proposition: &mut Proposition,
        substitutions: &BTreeMap<ValueId, ScalarTerm>,
    ) -> Result<(), LoweringError> {
        fn substitute_term(
            term: &ScalarTerm,
            substitutions: &BTreeMap<ValueId, ScalarTerm>,
        ) -> Result<ScalarTerm, LoweringError> {
            fn rebind(
                term: &mut ScalarTerm,
                substitutions: &BTreeMap<ValueId, ScalarTerm>,
            ) -> Result<(), LoweringError> {
                match term {
                    ScalarTerm::Application { .. } => Ok(()),
                    ScalarTerm::StructuralRead { path, .. } => {
                        for projection in path {
                            if let semantic_vocabulary::ScalarReadProjection::Index(selector) =
                                projection
                            {
                                rebind(selector, substitutions)?;
                            }
                        }
                        Ok(())
                    }

                    semantic_vocabulary::scalar_term_unary_operand!(operand) => {
                        rebind(operand, substitutions)
                    }
                    semantic_vocabulary::scalar_term_binary_operands!(left, right) => {
                        rebind(left, substitutions)?;
                        rebind(right, substitutions)
                    }
                    ScalarTerm::Value { id, scalar_type } => {
                        match scalar_type {
                            ScalarType::Boolean | ScalarType::IeeeFloat(_) => {}
                            ScalarType::Integer(_) => {}
                        }
                        let actual = substitutions.get(id).ok_or(LoweringError::Unsupported(
                            "runtime requirement scalar formal has no exact actual argument",
                        ))?;
                        if actual.scalar_type() != *scalar_type {
                            return unsupported(
                                "runtime requirement scalar argument changes its formal type",
                            );
                        }
                        *term = actual.clone();
                        Ok(())
                    }
                    ScalarTerm::Boolean(_)
                    | ScalarTerm::Integer { .. }
                    | ScalarTerm::IeeeFloatLiteral(_)
                    | semantic_vocabulary::scalar_term_predicate_quint_pattern!(..) => Ok(()),
                }
            }
            term.validate()
                .map_err(LoweringError::InvalidCrashPredicate)?;
            let mut rebound = term.clone();
            rebind(&mut rebound, substitutions)?;
            rebound
                .validate()
                .map_err(LoweringError::InvalidCrashPredicate)?;
            Ok(rebound)
        }

        match proposition {
            Proposition::Truth | Proposition::Falsehood => Ok(()),
            semantic_vocabulary::integer_math_comparison_operands!(left, right) => {
                let mut pending = vec![left, right];
                while let Some(term) = pending.pop() {
                    match term {
                        semantic_vocabulary::IntegerMathTerm::MathValue { source_type, value } => {
                            let scalar = substitute_term(
                                &ScalarTerm::value(*value, ScalarType::Integer(*source_type)),
                                substitutions,
                            )?;
                            *term = semantic_vocabulary::IntegerMathTerm::embed(scalar).ok_or(
                                LoweringError::Unsupported(
                                    "mathematical requirement lost its integer carrier",
                                ),
                            )?;
                        }
                        semantic_vocabulary::IntegerMathTerm::Embed(scalar) => {
                            **scalar = substitute_term(scalar, substitutions)?;
                        }
                        term => pending.extend(term.operands_mut()),
                    }
                }
                Ok(())
            }
            semantic_vocabulary::ordered_comparison_operands!(left, right) => {
                let rebound_left = substitute_term(left, substitutions)?;
                let rebound_right = substitute_term(right, substitutions)?;
                *left = rebound_left;
                *right = rebound_right;
                Ok(())
            }
            Proposition::ScalarIeeeFloatComparison {
                kind, left, right, ..
            } => {
                *proposition = crate::proofs::contract_predicates::ieee_float_proposition(
                    *kind,
                    substitute_term(left, substitutions)?,
                    substitute_term(right, substitutions)?,
                )?;
                Ok(())
            }
            Proposition::StructuralCaseMembership { .. }
            | Proposition::StructuralIdentity { .. } => Ok(()),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                substitute_runtime_requirement_scalar_values(premise, substitutions)?;
                substitute_runtime_requirement_scalar_values(conclusion, substitutions)
            }
            Proposition::Conjunction(children) | Proposition::Disjunction(children) => {
                for child in children {
                    substitute_runtime_requirement_scalar_values(child, substitutions)?;
                }
                Ok(())
            }
            _ => unsupported(
                "runtime requirement scalar substitution encountered an unsupported proposition",
            ),
        }
    }
}
pub(crate) mod unit_cleanup {

    use super::{
        CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, CheckedUnitEffectOperationPlan,
        LoweringError, MachineId, NominalAffineCleanup, PlaceId, Proposition, ScalarTerm,
        StructuralTypeId, StructuralTypeShape, TerminalMachine, lookup_machine_id, lookup_type_id,
        lower_unit_closure, obligation_id, unsupported,
    };
    use crate::expression_preparation::computation_graph::reachable_nodes;
    use crate::machine_lowering::nominal_cleanup_subject;
    use crate::shared::checked_facts::value_facts;
    use checked_trees::CheckedUnitNominalAffineCleanupPlan;
    use symbols::SymbolHandle;
    pub(crate) mod contexts {

        use super::{
            CheckedTrees, LoweringError, PlaceId, Proposition, ScalarTerm, StructuralTypeShape,
            SymbolHandle, unsupported,
        };
        use crate::shared::checked_facts::flow_facts;
        use crate::shared::terminal_identities::place_id;
        use terminal_psi::{
            StructuralParameterDeclaration, StructuralTypeDeclaration, find_structural_type,
        };

        pub(crate) struct SelectedCleanupSubjects<'checked> {
            remaining: Box<
                dyn Iterator<Item = &'checked checked_trees::CheckedUnitNominalAffineCleanupPlan>
                    + 'checked,
            >,
            first: symbols::SymbolKeyMap<
                SymbolHandle,
                &'checked checked_trees::CheckedUnitNominalAffineCleanupPlan,
            >,
        }

        impl<'checked> SelectedCleanupSubjects<'checked> {
            pub(crate) fn new(checked: &'checked CheckedTrees) -> Self {
                Self::from_rows(super::selected_cleanups(checked))
            }

            fn from_rows(
                rows: impl Iterator<Item = &'checked checked_trees::CheckedUnitNominalAffineCleanupPlan>
                + 'checked,
            ) -> Self {
                Self {
                    remaining: Box::new(rows),
                    first: Default::default(),
                }
            }

            fn first(
                &mut self,
                source: SymbolHandle,
            ) -> Option<&'checked checked_trees::CheckedUnitNominalAffineCleanupPlan> {
                if let Some(first) = self.first.get(&source) {
                    return Some(*first);
                }
                for cleanup in self.remaining.by_ref() {
                    self.first.entry(cleanup.cleanup_machine).or_insert(cleanup);
                    if cleanup.cleanup_machine == source {
                        return Some(cleanup);
                    }
                }
                None
            }
        }

        pub(crate) struct SignatureContext {
            pub(crate) receiver: Option<PlaceId>,
            dispatcher_requires: Option<Vec<Proposition>>,
        }

        impl SignatureContext {
            pub(crate) fn validate_contract(
                &self,
                requires: &[Proposition],
            ) -> Result<(), LoweringError> {
                if self
                    .dispatcher_requires
                    .as_deref()
                    .is_some_and(|expected| expected != requires)
                {
                    return unsupported(
                        "nominal cleanup requirements differ from the authored callable contract",
                    );
                }
                Ok(())
            }
        }

        pub(crate) fn validate_unconditional_contextual_requirements(
            checked: &CheckedTrees,
            source: SymbolHandle,
        ) -> Result<(), LoweringError> {
            if checked
                .facts
                .contextual_cleanup
                .entry_requirements
                .rows
                .iter()
                .any(|(_, row)| {
                    row.machine == source
                        && row.antecedent != checked_trees::CheckedBooleanExpression::Constant(true)
                })
            {
                return unsupported(
                    "guarded contextual cleanup requires an exact edge cleanup plan",
                );
            }
            Ok(())
        }

        pub(crate) fn field_clause(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            types: &[StructuralTypeDeclaration],
            parameter: &StructuralParameterDeclaration,
            identity: &checked_trees::CheckedMemberIdentity,
            expected: bool,
        ) -> Result<Proposition, LoweringError> {
            let field = find_structural_type(types, parameter.structural_type)
                .and_then(|declaration| match &declaration.shape {
                    StructuralTypeShape::Record { fields, .. } => terminal_psi::field_by_identity(
                        fields,
                        identities.intern(&identity.spelling()),
                    ),
                    _ => None,
                })
                .ok_or(LoweringError::Unsupported(
                    "contextual nominal cleanup requirement names an unlowered field",
                ))?;
            Ok(Proposition::Equal(
                ScalarTerm::boolean(expected),
                ScalarTerm::boolean_field(parameter.place, field.id),
            ))
        }

        pub(crate) fn prepare_signature(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            selected_subjects: &mut SelectedCleanupSubjects<'_>,
            source: SymbolHandle,
            predicate_parameters: &mut Vec<StructuralParameterDeclaration>,
            types: &[StructuralTypeDeclaration],
            next_place: &mut u64,
        ) -> Result<Option<SignatureContext>, LoweringError> {
            let plans = &flow_facts(checked)
                .terminal_nominal_affine_unit_cleanups
                .machines;
            let mut owners = plans.iter().filter(|nominal| nominal.machine == source);
            let nominal = owners.next();
            if owners.next().is_some() {
                return unsupported("nominal cleanup callable has duplicate checked plans");
            }
            let mut requires = Vec::new();
            if let Some(nominal) = nominal {
                validate_unconditional_contextual_requirements(checked, source)?;
                for requirement in &nominal.caller_requirements {
                    let parameter = predicate_parameters
                        .iter()
                        .find(|parameter| parameter.position == requirement.source_parameter_index)
                        .ok_or(LoweringError::Unsupported(
                            "nominal cleanup caller predicate parameter is absent",
                        ))?;
                    requires.push(field_clause(
                        identities,
                        types,
                        parameter,
                        &requirement.field_identity,
                        requirement.expected,
                    )?);
                }
            }
            let hook = selected_subjects.first(source);
            let mut receiver = None;
            if let Some(hook) = hook {
                if nominal.is_some() {
                    return unsupported("nominal cleanup dispatcher is its own hook");
                }
                let contract = checked.facts.contract_plans.for_machine(source).ok_or(
                    LoweringError::Unsupported("nominal cleanup hook contract is absent"),
                )?;
                if contract
                    .closed_scalar_values
                    .authored_requires()
                    .iter()
                    .any(Option::is_none)
                {
                    return unsupported(
                        "nominal cleanup hook lacks a complete callable prerequisite roster",
                    );
                }
                let structural_type =
                    crate::shared::terminal_identities::structural_declaration_by_identity(
                        types,
                        identities,
                        checked,
                        hook.type_identity,
                        "nominal cleanup hook owner type is absent",
                    )?
                    .id;
                if predicate_parameters.is_empty() {
                    predicate_parameters.push(StructuralParameterDeclaration {
                        place: place_id(crate::shared::terminal_identities::allocate_dense(
                            next_place,
                        )?),
                        position: 0,
                        structural_type,
                        multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        additional_input_qualifications: Vec::new(),
                    });
                }
                if let Some(parameter) = predicate_parameters.first() {
                    receiver = Some(parameter.place);
                }
            }
            if nominal.is_none() && hook.is_none() {
                return Ok(None);
            }
            crate::scalar_lowering::contracts::canonicalize_requires(&mut requires)?;
            Ok(Some(SignatureContext {
                receiver,
                dispatcher_requires: nominal.map(|_| requires),
            }))
        }
        #[cfg(test)]
        mod selected_subject_tests {

            use super::SelectedCleanupSubjects;
            use checked_trees::{
                CheckedNormalCleanupAction, CheckedTrees, CheckedUnitNominalAffineCleanupPlan,
                CheckedUnitStructuralArgumentSourcePlan,
            };
            use std::cell::Cell;
            use std::panic::{AssertUnwindSafe, catch_unwind};
            use symbols::SymbolHandle;

            fn cleanup(machine: SymbolHandle, marker: u64) -> CheckedUnitNominalAffineCleanupPlan {
                CheckedUnitNominalAffineCleanupPlan {
                    partition_leaf: Default::default(),
                    consumed_claims: Vec::new(),
                    source: CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0,
                    },
                    path: Vec::new(),
                    type_identity: Default::default(),
                    cleanup_machine: machine,
                    cleanup_state: SymbolHandle::from_parts(42, 1),
                    cleanup_contract_report_fingerprint: marker,
                    case_conditions: Default::default(),
                }
            }

            #[test]
            fn first_subjects_match_original_search_without_visiting_an_unused_tail() {
                let first = SymbolHandle::from_parts(7, 1);
                let next_generation = SymbolHandle::from_parts(7, 2);
                let other = SymbolHandle::from_parts(8, 1);
                let absent = SymbolHandle::from_parts(9, 1);
                let mut checked = CheckedTrees::default();
                checked
                    .facts
                    .values
                    .scalar_computations
                    .temporary_cleanup_actions
                    .insert_many(
                        [
                            cleanup(first, 11),
                            cleanup(next_generation, 22),
                            cleanup(first, 33),
                            cleanup(other, 44),
                        ]
                        .into_iter()
                        .map(CheckedNormalCleanupAction::InvokeNominal),
                    );
                let mut subjects = SelectedCleanupSubjects::new(&checked);
                for source in [other, next_generation, first, absent, first] {
                    let expected = super::super::selected_cleanups(&checked)
                        .find(|row| row.cleanup_machine == source);
                    assert_eq!(
                        subjects.first(source).map(std::ptr::from_ref),
                        expected.map(std::ptr::from_ref),
                    );
                }
                assert_eq!(
                    subjects
                        .first(first)
                        .unwrap()
                        .cleanup_contract_report_fingerprint,
                    11
                );
                assert_eq!(
                    subjects
                        .first(next_generation)
                        .unwrap()
                        .cleanup_contract_report_fingerprint,
                    22
                );
                assert!(
                    SelectedCleanupSubjects::new(&CheckedTrees::default())
                        .first(first)
                        .is_none()
                );

                let row = super::super::selected_cleanups(&checked).next().unwrap();
                let visits = Cell::new(0);
                let rows = std::iter::once(row)
                    .inspect(|_| visits.set(visits.get() + 1))
                    .chain(std::iter::once_with(|| panic!("unvisited cleanup tail")));
                let mut prefix = SelectedCleanupSubjects::from_rows(rows);
                assert!(std::ptr::eq(prefix.first(first).unwrap(), row));
                assert!(std::ptr::eq(prefix.first(first).unwrap(), row));
                assert_eq!(visits.get(), 1);
                assert!(catch_unwind(AssertUnwindSafe(|| prefix.first(absent))).is_err());
                assert!(
                    catch_unwind(|| {
                        std::iter::once(row)
                            .chain(std::iter::once_with(|| panic!("unvisited cleanup tail")))
                            .find(|candidate| candidate.cleanup_machine == absent)
                    })
                    .is_err()
                );
            }
        }
    }
    pub(crate) mod dispositions {

        use checked_trees::{
            CheckedNormalCleanupAction, CheckedStoreLeaf, CheckedStoreRoot,
            CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
            CheckedUnitStructuralParameterPlan,
        };
        use symbols::SymbolHandle;

        struct CurrentHomes {
            locals: Vec<(SymbolHandle, Option<u32>)>,
        }

        impl CurrentHomes {
            fn new(operations: &[CheckedUnitEffectOperationPlan]) -> Self {
                let mut locals = operations
                    .iter()
                    .filter_map(|operation| {
                        let result = operation.structural_result_binding()?;
                        result
                            .local
                            .is_named()
                            .then_some((result.local.symbol, None))
                    })
                    .collect::<Vec<_>>();
                locals.sort_unstable_by_key(|(symbol, _)| {
                    (symbol.arena_index(), symbol.generation())
                });
                locals.dedup_by_key(|(symbol, _)| *symbol);
                Self { locals }
            }

            fn local(&self, symbol: SymbolHandle) -> Option<usize> {
                self.locals
                    .binary_search_by_key(
                        &(symbol.arena_index(), symbol.generation()),
                        |(symbol, _)| (symbol.arena_index(), symbol.generation()),
                    )
                    .ok()
            }

            fn source_local(&self, source: &SourcePlan) -> Option<usize> {
                let SourcePlan::StructuralLocal { symbol } = source else {
                    return None;
                };
                self.local(*symbol)
            }

            fn binding(&self, local: Option<usize>) -> Option<u32> {
                self.locals.get(local?).and_then(|(_, binding)| *binding)
            }

            fn observe(&mut self, operation: &CheckedUnitEffectOperationPlan) {
                if let Some(result) = operation.structural_result_binding()
                    && result.local.is_named()
                    && let Some(local) = self.local(result.local.symbol)
                {
                    self.locals[local].1 = Some(result.binding_ordinal);
                }
                if let CheckedUnitEffectOperationPlan::Store(store) = operation
                    && matches!(store.destination.leaf, CheckedStoreLeaf::OwnedRebind { .. })
                    && let CheckedStoreRoot::Local { symbol } = store.destination.root
                    && let Some(local) = self.local(symbol)
                {
                    self.locals[local].1 = None;
                }
            }

            fn same_storage(
                &self,
                left: &SourcePlan,
                left_local: Option<usize>,
                right: &SourcePlan,
                right_local: Option<usize>,
            ) -> bool {
                if left == right {
                    return true;
                }
                match (left, right) {
                    (
                        SourcePlan::StructuralResult { binding_ordinal },
                        SourcePlan::StructuralLocal { .. },
                    ) => self.binding(right_local) == Some(*binding_ordinal),
                    (
                        SourcePlan::StructuralLocal { .. },
                        SourcePlan::StructuralResult { binding_ordinal },
                    ) => self.binding(left_local) == Some(*binding_ordinal),
                    _ => false,
                }
            }
        }

        pub(crate) fn outstanding_actions<'a>(
            actions: &'a [CheckedNormalCleanupAction],
            operations: &[CheckedUnitEffectOperationPlan],
            parameters: &[CheckedUnitStructuralParameterPlan],
            control: &checked_trees::CheckedCleanupControlPlans,
        ) -> Vec<&'a CheckedNormalCleanupAction> {
            if actions.is_empty() {
                return Vec::new();
            }
            let mut homes = CurrentHomes::new(operations);
            let action_locals = actions
                .iter()
                .map(|action| homes.source_local(action.source()))
                .collect::<Vec<_>>();
            let mut outstanding = vec![true; actions.len()];
            for operation in operations {
                homes.observe(operation);
                let destination = operation
                    .store()
                    .and_then(|store| store.window_place(parameters));
                let destination_local = destination
                    .as_ref()
                    .and_then(|destination| homes.source_local(&destination.source));
                let release_local = match operation {
                    CheckedUnitEffectOperationPlan::ReleaseReference { source, .. } => {
                        homes.source_local(source)
                    }
                    _ => None,
                };
                for ((action, local), retained) in
                    actions.iter().zip(&action_locals).zip(&mut outstanding)
                {
                    match operation {
                        CheckedUnitEffectOperationPlan::ContinuationCleanup {
                            cleanup_actions,
                            ..
                        } => {
                            if cleanup_actions
                                .iter()
                                .any(|candidate| candidate.same_effect(action))
                            {
                                *retained = false;
                            }
                        }
                        CheckedUnitEffectOperationPlan::ReleaseReference {
                            owner_cleanups, ..
                        } if control
                            .reference_owner_cleanups
                            .span_or_empty(*owner_cleanups)
                            .iter()
                            .any(|owner| {
                                control
                                    .actions
                                    .span_or_empty(owner.actions)
                                    .iter()
                                    .any(|candidate| candidate.same_effect(action))
                            }) =>
                        {
                            *retained = false;
                        }
                        CheckedUnitEffectOperationPlan::Store(_) => {
                            if destination.as_ref().is_some_and(|destination| {
                                homes.same_storage(
                                    action.source(),
                                    *local,
                                    &destination.source,
                                    destination_local,
                                ) && action.path().starts_with(&destination.path)
                            }) {
                                *retained = true;
                            }
                        }
                        CheckedUnitEffectOperationPlan::ReleaseReference {
                            source,
                            path,
                            case_conditions,
                            select_evaluated_storage,
                            defer_to_exit: false,
                            ..
                        } if path.is_empty()
                            && case_conditions.is_empty()
                            && !select_evaluated_storage
                            && homes.same_storage(
                                action.source(),
                                *local,
                                source,
                                release_local,
                            )
                            && !matches!(action, CheckedNormalCleanupAction::InvokeNominal(_)) =>
                        {
                            *retained = false;
                        }
                        _ => {}
                    }
                }
            }
            actions
                .iter()
                .zip(outstanding)
                .filter_map(|(action, retained)| retained.then_some(action))
                .collect()
        }
    }
    pub(crate) use dispositions::outstanding_actions;
    pub(crate) mod expression_owners {

        use super::{CleanupRegion, LoweringError, unsupported};
        use crate::expression_preparation::bindings::{
            StructuralLocalBinding, structural_local_by_root,
        };
        use crate::shared::terminal_identities::structural_declaration_by_identity;
        use checked_trees::CheckedUnitStructuralArgumentSourcePlan;

        pub(crate) fn expression_cleanup_regions<'action>(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            actions: impl IntoIterator<Item = &'action checked_trees::CheckedNormalCleanupAction>,
            partition_refs: &[checked_trees::CheckedCleanupPartitionRef],
            completed_homes: &[StructuralLocalBinding],
            carrier_multiplicity: impl FnMut(
                semantic_vocabulary::PlaceId,
            ) -> Result<
                terminal_psi::StructuralMultiplicity,
                LoweringError,
            >,
            calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
        ) -> Result<Vec<CleanupRegion>, LoweringError> {
            let types = calls.structural_types;
            let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
            let actions = super::prepare_checked_cleanup_regions(
                identities,
                checked,
                actions,
                partition_refs,
                &[],
                control,
                types,
                |identities, identity| {
                    Ok(structural_declaration_by_identity(
                        types,
                        identities,
                        checked,
                        identity,
                        "value completion has no structural type",
                    )?
                    .id)
                },
                |identities, source, path| {
                    let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                        expression,
                        ..
                    } = source
                    else {
                        return unsupported("value completion lost its expression owner");
                    };
                    let binding = structural_local_by_root(
                        completed_homes,
                        facts::PlaceRoot::Expression(*expression),
                    )
                    .ok_or(LoweringError::Unsupported(
                        "value completion lost its completed expression owner",
                    ))?;
                    if !binding.argument.path.is_empty() {
                        return unsupported(
                            "value completion names an alias instead of its owned home",
                        );
                    }
                    let path = crate::expression_preparation::bindings::structural_paths::lower_typed_argument_path(
                        identities, path, binding.structural_type, types,
                        None, &mut std::iter::empty(), None,
                    )?;
                    Ok((binding.argument.place, path))
                },
                carrier_multiplicity,
                |cleanup, _, _| Ok(cleanup.consumed_claims.clone()),
            )?;
            super::emit_prepared_cleanup_regions(&actions, calls)
        }
    }
    pub(crate) mod ordered {

        use super::{
            CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, LoweringError,
            lower_unit_closure,
        };
        use crate::shared::producer_result::SourceMappedLowered;
        use crate::unit::attached_unit::UnitClosureRequest;
        use crate::unit::attached_unit::bodies::UnitPlans;

        pub(crate) fn lower_ordered_nominal_affine_unit_cleanup_machine(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &CheckedTrees,
            nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
            program_roots: crate::TerminalProgramRoots<'_>,
            exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
        ) -> Result<SourceMappedLowered, LoweringError> {
            let plans = UnitPlans::published(&checked.facts.flow);
            lower_unit_closure(
                identities,
                checked,
                &UnitClosureRequest {
                    plans,
                    entry: nominal.machine,
                    unit_roots: &[nominal.machine],
                    program_roots,
                    exact_toolchain_sources,
                    scalar_entry: false,
                },
            )
        }
    }
    pub(crate) mod reference_retirement {

        use super::{CleanupRegion, LoweringError};
        use crate::emission::operation_emission::buffer::OperationBuffer;
        use crate::shared::checked_facts::flow_facts;
        use crate::unit::attached_unit::argument_evaluation::{Evaluation, StructuralValueStorage};
        use checked_trees::CheckedUnitEffectOperationPlan;
        use terminal_psi::{StructuralParameterDeclaration, TerminalAffineCleanupAction};

        pub(crate) fn lower_reference_completion_regions(
            identities: &mut terminal_psi::TerminalIdentityTableBuilder,
            checked: &checked_trees::CheckedTrees,
            planned_operations: &[CheckedUnitEffectOperationPlan],
            mut authored_regions: Vec<CleanupRegion>,
            parameters: &[StructuralParameterDeclaration],
            evaluation: &Evaluation,
            operations: &OperationBuffer,
            mut owner_regions: impl FnMut(
                &mut terminal_psi::TerminalIdentityTableBuilder,
                &checked_trees::CheckedReferenceOwnerCleanup,
            ) -> Result<Vec<CleanupRegion>, LoweringError>,
            mut result_storage: impl FnMut(u32) -> Result<StructuralValueStorage, LoweringError>,
        ) -> Result<Vec<CleanupRegion>, LoweringError> {
            let mut carriers = Vec::new();
            for operation in planned_operations {
                let CheckedUnitEffectOperationPlan::ReleaseReference {
                    source,
                    path,
                    case_conditions,
                    defer_to_exit: true,
                    owner_cleanups,
                    ..
                } = operation
                else {
                    continue;
                };
                let storage =
                    evaluation.cleanup_value_storage(source, parameters, &mut result_storage)?;
                let suffix =
                    crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                        identities,
                        path,
                        storage.selected_type,
                        &evaluation.structural_types,
                    )?;
                let conditions = super::lower_cleanup_conditions(
                    identities,
                    checked,
                    &checked.facts.flow.terminal_unit_effects.cleanup_control,
                    *case_conditions,
                    &evaluation.structural_types,
                    |identities, selected_path| {
                        let mut selected = storage.path.clone();
                        selected.extend(crate::expression_preparation::bindings::structural_paths::lower_static_argument_path(
                            identities, selected_path, storage.selected_type, &evaluation.structural_types,
                        )?);
                        Ok((storage.place, selected))
                    },
                )?;
                let mut path = storage.path;
                path.extend(suffix.clone());
                let place = storage.place;
                let action = if path.is_empty() {
                    evaluation.descriptor_retirement(place, operations)?
                } else {
                    let leaf = terminal_semantics::runtime_structural_path_tip(
                        evaluation.structural_types.iter(),
                        storage.selected_type,
                        &suffix,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "deferred release lost its carrier path",
                    ))?;
                    TerminalAffineCleanupAction::DiscardResidual(
                        terminal_psi::StructuralAffineDiscard {
                            place,
                            path: path.clone(),
                            structural_type: leaf.id,
                        },
                    )
                };
                if !carriers
                    .iter()
                    .any(|(region, _): &(CleanupRegion, Vec<CleanupRegion>)| {
                        region.action == action
                    })
                {
                    let mut owners = Vec::new();
                    for owner in flow_facts(checked)
                        .terminal_unit_effects
                        .cleanup_control
                        .reference_owner_cleanups
                        .span_or_empty(*owner_cleanups)
                    {
                        owners.extend(owner_regions(identities, owner)?);
                    }
                    carriers.push((
                        CleanupRegion {
                            partition_leaf: None,
                            declaration_order: None,
                            selected_region: Some({
                                terminal_psi::StructuralArgument {
                                    place,
                                    path,
                                    access: terminal_psi::StructuralAccess::Owned,
                                }
                            }),
                            conditions,
                            action: crate::unit::unit_cleanup::CleanupRegionAction::Run(action),
                        },
                        owners,
                    ));
                }
            }
            for (carrier, _) in &mut carriers {
                if let Some(position) = authored_regions.iter().position(|region| {
                    region.action.physical().is_some()
                        && region.action.place() == carrier.action.place()
                        && region.action.path() == carrier.action.path()
                }) {
                    *carrier = authored_regions.remove(position);
                }
            }
            let mut regions = Vec::new();
            for (carrier, owners) in carriers {
                regions.push(carrier);
                regions.extend(owners);
            }
            regions.extend(authored_regions);
            Ok(regions)
        }
    }
    pub(crate) use expression_owners::expression_cleanup_regions;
    use ordered::lower_ordered_nominal_affine_unit_cleanup_machine;
    pub(crate) use reference_retirement::lower_reference_completion_regions;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CleanupCaseCondition {
        pub(crate) place: PlaceId,
        pub(crate) path: Vec<terminal_psi::StructuralPathSegment>,
        pub(crate) structural_type: StructuralTypeId,
        pub(crate) case: semantic_vocabulary::StructuralCaseId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CleanupPartitionLeaf {
        pub(crate) owner: arena::Handle<checked_trees::CheckedCleanupPartitionPlan>,
        pub(crate) ordinal: u32,
        pub(crate) root: std::sync::Arc<CleanupPartitionNode>,
        pub(crate) leaf_count: usize,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum CleanupPartitionNode {
        Scope {
            leaf_span: std::ops::Range<usize>,
            place: PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            structural_type: StructuralTypeId,
            conditions: Vec<CleanupCaseCondition>,
            steps: Vec<CleanupPartitionNode>,
        },
        Leaf(u32),
        Cases {
            leaf_span: std::ops::Range<usize>,
            place: PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            structural_type: StructuralTypeId,
            arms: Vec<CleanupPartitionArm>,
            common: Vec<CleanupPartitionNode>,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CleanupPartitionArm {
        pub(crate) case: semantic_vocabulary::StructuralCaseId,
        pub(crate) reachability: checked_trees::CheckedCleanupCaseReachability,
        pub(crate) steps: Vec<CleanupPartitionNode>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum CleanupRegionAction {
        Run(terminal_psi::TerminalAffineCleanupAction),
        CheckedPartition {
            place: PlaceId,
            path: Vec<terminal_psi::StructuralPathSegment>,
        },
    }

    impl CleanupRegionAction {
        pub(crate) fn place(&self) -> PlaceId {
            match self {
                Self::Run(action) => action.place(),
                Self::CheckedPartition { place, .. } => *place,
            }
        }
        pub(crate) fn path(&self) -> &[terminal_psi::StructuralPathSegment] {
            match self {
                Self::Run(action) => action.path(),
                Self::CheckedPartition { path, .. } => path,
            }
        }
        pub(crate) fn into_physical(self) -> Option<terminal_psi::TerminalAffineCleanupAction> {
            match self {
                Self::Run(action) => Some(action),
                Self::CheckedPartition { .. } => None,
            }
        }
        pub(crate) fn physical(&self) -> Option<&terminal_psi::TerminalAffineCleanupAction> {
            match self {
                Self::Run(action) => Some(action),
                Self::CheckedPartition { .. } => None,
            }
        }
    }

    impl PartialEq<terminal_psi::TerminalAffineCleanupAction> for CleanupRegionAction {
        fn eq(&self, other: &terminal_psi::TerminalAffineCleanupAction) -> bool {
            self.physical() == Some(other)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CleanupRegion {
        pub(crate) partition_leaf: Option<CleanupPartitionLeaf>,
        pub(crate) declaration_order: Option<(u8, u32)>,
        pub(crate) selected_region: Option<terminal_psi::StructuralArgument>,
        pub(crate) conditions: Vec<CleanupCaseCondition>,
        pub(crate) action: CleanupRegionAction,
    }

    impl CleanupRegion {
        pub(crate) fn unconditional(action: terminal_psi::TerminalAffineCleanupAction) -> Self {
            Self {
                partition_leaf: None,
                declaration_order: None,
                selected_region: None,
                conditions: Vec::new(),
                action: crate::unit::unit_cleanup::CleanupRegionAction::Run(action),
            }
        }
    }

    pub(crate) fn lower_cleanup_conditions(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        control: &checked_trees::CheckedCleanupControlPlans,
        selected: arena::HandleSpan<checked_trees::CheckedCleanupCaseCondition>,
        types: &[terminal_psi::StructuralTypeDeclaration],
        mut storage: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<
            (PlaceId, Vec<terminal_psi::StructuralPathSegment>),
            LoweringError,
        >,
    ) -> Result<Vec<CleanupCaseCondition>, LoweringError> {
        if selected.is_empty() {
            return Ok(Vec::new());
        }
        let conditions = control
            .conditions
            .span(selected)
            .ok_or(LoweringError::Unsupported(
                "cleanup conditions lost their owning catalog",
            ))?;
        conditions
            .iter()
            .map(|condition| {
                let path = if condition.path.is_empty() {
                    &[][..]
                } else {
                    control
                        .paths
                        .span(condition.path)
                        .ok_or(LoweringError::Unsupported(
                            "cleanup condition lost its retained path",
                        ))?
                };
                let (place, path) = storage(identities, path)?;
                let structural_type =
                    crate::shared::terminal_identities::structural_declaration_by_identity(
                        types,
                        identities,
                        checked,
                        condition.type_identity,
                        "cleanup condition has no structural type",
                    )?
                    .id;
                let (_, case) =
                    crate::expression_preparation::bindings::structural_cases::resolve_at(
                        identities,
                        types,
                        structural_type,
                        &[],
                        &condition.case_identity,
                    )?;
                Ok(CleanupCaseCondition {
                    place,
                    path,
                    structural_type,
                    case,
                })
            })
            .collect()
    }

    pub(crate) fn lower_nominal_affine_unit_cleanup_machine(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
        program_roots: crate::TerminalProgramRoots<'_>,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<crate::shared::producer_result::SourceMappedLowered, LoweringError> {
        lower_ordered_nominal_affine_unit_cleanup_machine(
            identities,
            checked,
            nominal,
            program_roots,
            exact_toolchain_sources,
        )
    }

    pub(crate) fn continuation_cleanups<'a>(
        control: &'a checked_trees::CheckedCleanupControlPlans,
        operations: impl IntoIterator<Item = &'a CheckedUnitEffectOperationPlan>,
    ) -> impl Iterator<Item = &'a CheckedUnitNominalAffineCleanupPlan> {
        operations
            .into_iter()
            .flat_map(move |operation| control.operation_actions(operation))
            .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
    }

    pub(crate) struct SelectedCleanupSequence<'action> {
        pub(crate) actions:
            Vec<std::borrow::Cow<'action, checked_trees::CheckedNormalCleanupAction>>,
        pub(crate) partitions: Vec<checked_trees::CheckedCleanupPartitionRef>,
    }

    /// A later completion can select only part of an earlier cleanup schedule.
    /// Detach that partial schedule explicitly; it cannot close the full carrier.
    pub(crate) fn select_cleanup_partition_sequence<'action>(
        control: &checked_trees::CheckedCleanupControlPlans,
        roots: &[checked_trees::CheckedCleanupPartitionRef],
        actions: &[&'action checked_trees::CheckedNormalCleanupAction],
    ) -> Result<SelectedCleanupSequence<'action>, LoweringError> {
        let key = |owner: arena::Handle<checked_trees::CheckedCleanupPartitionPlan>| {
            (owner.arena_index(), owner.generation())
        };
        let mut owners = roots.iter().map(|root| root.partition).collect::<Vec<_>>();
        owners.sort_by_key(|owner| key(*owner));
        if owners.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LoweringError::InvalidCleanupPartition {
                reason: "cleanup sequence repeats a partition owner",
            });
        }
        if actions.iter().any(|action| {
            let owner = action.partition_leaf().partition;
            owner.is_valid()
                && owners
                    .binary_search_by_key(&key(owner), |owner| key(*owner))
                    .is_err()
        }) {
            return Err(LoweringError::InvalidCleanupPartition {
                reason: "annotated cleanup action has no sequence partition owner",
            });
        }
        let mut ordered = actions.iter().copied().enumerate().collect::<Vec<_>>();
        ordered
            .sort_by_key(|(position, action)| (key(action.partition_leaf().partition), *position));
        for owner in &owners {
            if !control.partitions.is_valid(*owner) {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "selected cleanup sequence names a missing partition owner",
                });
            }
            let plan = control.partitions.get(*owner);
            let authorized = control.actions.span(plan.actions).ok_or(
                LoweringError::InvalidCleanupPartition {
                    reason: "selected cleanup partition has an invalid action span",
                },
            )?;
            let mut previous = None;
            let start = ordered.partition_point(|(_, action)| {
                key(action.partition_leaf().partition) < key(*owner)
            });
            for (_, action) in ordered[start..]
                .iter()
                .take_while(|(_, action)| action.partition_leaf().partition == *owner)
            {
                let action = *action;
                let ordinal = action.partition_leaf().ordinal as usize;
                if previous.is_some_and(|previous| ordinal <= previous)
                    || authorized.get(ordinal) != Some(action)
                    || action.source() != &plan.source
                {
                    return Err(LoweringError::InvalidCleanupPartition {
                        reason: "partial cleanup sequence is not an ordered unique subset of its exact partition",
                    });
                }
                previous = Some(ordinal);
            }
        }
        let partitions = roots
            .iter()
            .copied()
            .filter(|root| {
                let owner = key(root.partition);
                let start = ordered
                    .partition_point(|(_, action)| key(action.partition_leaf().partition) < owner);
                let mut selected = ordered[start..]
                    .iter()
                    .take_while(|(_, action)| action.partition_leaf().partition == root.partition)
                    .map(|(_, action)| *action);
                let plan = control.partitions.get(root.partition);
                let complete = control
                    .actions
                    .span_or_empty(plan.actions)
                    .iter()
                    .all(|expected| selected.next() == Some(expected));
                complete && selected.next().is_none()
            })
            .collect::<Vec<_>>();
        let mut complete = partitions
            .iter()
            .map(|root| root.partition)
            .collect::<Vec<_>>();
        complete.sort_by_key(|owner| key(*owner));
        let actions = actions
            .iter()
            .map(|action| {
                let mut selected = std::borrow::Cow::Borrowed(*action);
                let owner = action.partition_leaf().partition;
                if owner.is_valid()
                    && complete
                        .binary_search_by_key(&key(owner), |owner| key(*owner))
                        .is_err()
                {
                    match selected.to_mut() {
                        checked_trees::CheckedNormalCleanupAction::Discard(row) => {
                            row.partition_leaf = Default::default()
                        }
                        checked_trees::CheckedNormalCleanupAction::InvokeNominal(row) => {
                            row.partition_leaf = Default::default()
                        }
                        checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => {}
                    }
                }
                selected
            })
            .collect();
        Ok(SelectedCleanupSequence {
            actions,
            partitions,
        })
    }

    type CleanupStorage = (PlaceId, Vec<terminal_psi::StructuralPathSegment>);

    struct PreparedCleanupPartition {
        owner: arena::Handle<checked_trees::CheckedCleanupPartitionPlan>,
        root: std::sync::Arc<CleanupPartitionNode>,
        leaf_count: usize,
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_cleanup_partitions(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        scheduled: &[&checked_trees::CheckedNormalCleanupAction],
        partition_refs: &[checked_trees::CheckedCleanupPartitionRef],
        control: &checked_trees::CheckedCleanupControlPlans,
        types: &[terminal_psi::StructuralTypeDeclaration],
        resolve_type: &mut impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            typed_trees::type_identity::TypeIdentityId,
        ) -> Result<StructuralTypeId, LoweringError>,
        storage: &mut impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<CleanupStorage, LoweringError>,
    ) -> Result<Vec<PreparedCleanupPartition>, LoweringError> {
        let mut owners = partition_refs
            .iter()
            .map(|row| row.partition)
            .collect::<Vec<_>>();
        owners.sort_by_key(|owner| (owner.arena_index(), owner.generation()));
        if owners.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(LoweringError::InvalidCleanupPartition {
                reason: "cleanup sequence repeats a partition owner",
            });
        }
        let mut selected = vec![Vec::new(); owners.len()];
        for action in scheduled {
            let owner = action.partition_leaf().partition;
            if let Ok(position) = owners
                .binary_search_by_key(&(owner.arena_index(), owner.generation()), |owner| {
                    (owner.arena_index(), owner.generation())
                })
            {
                selected[position].push(*action);
            } else if owner.is_valid() {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "annotated cleanup action has no sequence partition owner",
                });
            }
        }
        let mut retained = Vec::new();
        for (owner, selected) in owners.into_iter().zip(selected) {
            if !control.partitions.is_valid(owner) {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "cleanup sequence names a missing partition owner",
                });
            }
            let plan = control.partitions.get(owner);
            if !checked
                .facts
                .contextual_cleanup
                .dispositions
                .is_valid(plan.disposition)
            {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "cleanup partition has no checked disposition",
                });
            }
            let disposition = checked
                .facts
                .contextual_cleanup
                .dispositions
                .get(plan.disposition);
            if !disposition.root.is_valid() {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "cleanup partition has no exact source root",
                });
            }
            let authorized = control.actions.span(plan.actions).ok_or(
                LoweringError::InvalidCleanupPartition {
                    reason: "cleanup partition has an invalid authorized action span",
                },
            )?;
            if selected.len() != authorized.len()
                || selected.iter().zip(authorized).enumerate().any(
                    |(ordinal, (selected, authorized))| {
                        selected.partition_leaf().ordinal as usize != ordinal
                            || *selected != authorized
                            || selected.source() != &plan.source
                    },
                )
            {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "cleanup partition does not match its complete ordered action span",
                });
            }
            let expected = storage(
                identities,
                &plan.source,
                control
                    .paths
                    .span(plan.path)
                    .ok_or(LoweringError::InvalidCleanupPartition {
                        reason: "cleanup partition has an invalid carrier path",
                    })?,
            )?;
            let expected_type = resolve_type(identities, plan.type_identity)?;
            let mut leaves = Vec::new();
            let mut active = Vec::new();
            let root = prepare_cleanup_partition_node(
                identities,
                checked,
                control,
                types,
                &plan.source,
                plan.root,
                resolve_type,
                storage,
                &mut active,
                &mut leaves,
            )?;
            validate_cleanup_partition(&root, &expected, expected_type, selected.len(), types)?;
            if leaves
                != (0..authorized.len())
                    .map(|ordinal| ordinal as u32)
                    .collect::<Vec<_>>()
            {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "cleanup partition omits, repeats or reorders an action leaf",
                });
            }
            retained.push(PreparedCleanupPartition {
                owner,
                root: std::sync::Arc::new(root),
                leaf_count: authorized.len(),
                source: plan.source.clone(),
            });
        }
        Ok(retained)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_cleanup_partition_node(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        control: &checked_trees::CheckedCleanupControlPlans,
        types: &[terminal_psi::StructuralTypeDeclaration],
        source: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        handle: arena::Handle<checked_trees::CheckedCleanupPartitionNode>,
        resolve_type: &mut impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            typed_trees::type_identity::TypeIdentityId,
        ) -> Result<StructuralTypeId, LoweringError>,
        storage: &mut impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<CleanupStorage, LoweringError>,
        active: &mut Vec<arena::Handle<checked_trees::CheckedCleanupPartitionNode>>,
        leaves: &mut Vec<u32>,
    ) -> Result<CleanupPartitionNode, LoweringError> {
        use checked_trees::CheckedCleanupPartitionNode as Node;
        if !control.partition_nodes.is_valid(handle) || active.contains(&handle) {
            return Err(LoweringError::InvalidCleanupPartition {
                reason: "cleanup partition has a missing or recursive node",
            });
        }
        active.push(handle);
        let first_leaf = leaves.len();
        let node = match control.partition_nodes.get(handle) {
            Node::Leaf { ordinal } => {
                leaves.push(*ordinal);
                CleanupPartitionNode::Leaf(*ordinal)
            }
            Node::Scope {
                path,
                type_identity,
                conditions,
                steps,
            } => {
                let (place, path) = storage(
                    identities,
                    source,
                    control
                        .paths
                        .span(*path)
                        .ok_or(LoweringError::InvalidCleanupPartition {
                            reason: "cleanup node has an invalid storage path",
                        })?,
                )?;
                let structural_type = resolve_type(identities, *type_identity)?;
                control.conditions.span(*conditions).ok_or(
                    LoweringError::InvalidCleanupPartition {
                        reason: "cleanup scope has an invalid condition span",
                    },
                )?;
                let conditions = lower_cleanup_conditions(
                    identities,
                    checked,
                    control,
                    *conditions,
                    types,
                    |builder, path| storage(builder, source, path),
                )?;
                let mut children = Vec::new();
                for step in control.partition_steps.span(*steps).ok_or(
                    LoweringError::InvalidCleanupPartition {
                        reason: "cleanup scope has an invalid child span",
                    },
                )? {
                    children.push(prepare_cleanup_partition_node(
                        identities,
                        checked,
                        control,
                        types,
                        source,
                        step.node,
                        resolve_type,
                        storage,
                        active,
                        leaves,
                    )?);
                }
                CleanupPartitionNode::Scope {
                    leaf_span: first_leaf..leaves.len(),
                    place,
                    path,
                    structural_type,
                    conditions,
                    steps: children,
                }
            }
            Node::Cases {
                path,
                type_identity,
                arms,
                common,
            } => {
                let (place, path) = storage(
                    identities,
                    source,
                    control
                        .paths
                        .span(*path)
                        .ok_or(LoweringError::InvalidCleanupPartition {
                            reason: "cleanup node has an invalid storage path",
                        })?,
                )?;
                let structural_type = resolve_type(identities, *type_identity)?;
                let mut retained_arms = Vec::new();
                for arm in control.partition_arms.span(*arms).ok_or(
                    LoweringError::InvalidCleanupPartition {
                        reason: "cleanup carrier has an invalid alternative span",
                    },
                )? {
                    let (_, case) =
                        crate::expression_preparation::bindings::structural_cases::resolve_at(
                            identities,
                            types,
                            structural_type,
                            &[],
                            &arm.case_identity,
                        )?;
                    let mut steps = Vec::new();
                    for step in control.partition_steps.span(arm.steps).ok_or(
                        LoweringError::InvalidCleanupPartition {
                            reason: "cleanup alternative has an invalid child span",
                        },
                    )? {
                        steps.push(prepare_cleanup_partition_node(
                            identities,
                            checked,
                            control,
                            types,
                            source,
                            step.node,
                            resolve_type,
                            storage,
                            active,
                            leaves,
                        )?);
                    }
                    if arm.reachability == checked_trees::CheckedCleanupCaseReachability::Inactive
                        && !steps.is_empty()
                    {
                        return Err(LoweringError::InvalidCleanupPartition {
                            reason: "inactive cleanup alternative has executable actions",
                        });
                    }
                    retained_arms.push(CleanupPartitionArm {
                        case,
                        reachability: arm.reachability,
                        steps,
                    });
                }
                let mut retained_common = Vec::new();
                for step in control.partition_steps.span(*common).ok_or(
                    LoweringError::InvalidCleanupPartition {
                        reason: "cleanup shared fields have an invalid child span",
                    },
                )? {
                    retained_common.push(prepare_cleanup_partition_node(
                        identities,
                        checked,
                        control,
                        types,
                        source,
                        step.node,
                        resolve_type,
                        storage,
                        active,
                        leaves,
                    )?);
                }
                CleanupPartitionNode::Cases {
                    leaf_span: first_leaf..leaves.len(),
                    place,
                    path,
                    structural_type,
                    arms: retained_arms,
                    common: retained_common,
                }
            }
        };
        active.pop();
        Ok(node)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_checked_cleanup_regions<'action>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        actions: impl IntoIterator<Item = &'action checked_trees::CheckedNormalCleanupAction>,
        partition_refs: &[checked_trees::CheckedCleanupPartitionRef],
        selected_regions: &[checked_trees::CheckedUnitStructuralArgumentPlan],
        control: &checked_trees::CheckedCleanupControlPlans,
        types: &[terminal_psi::StructuralTypeDeclaration],
        mut resolve_type: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            typed_trees::type_identity::TypeIdentityId,
        ) -> Result<StructuralTypeId, LoweringError>,
        mut storage: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<
            (PlaceId, Vec<terminal_psi::StructuralPathSegment>),
            LoweringError,
        >,
        mut carrier_multiplicity: impl FnMut(
            PlaceId,
        ) -> Result<
            terminal_psi::StructuralMultiplicity,
            LoweringError,
        >,
        mut consumed_claims: impl FnMut(
            &CheckedUnitNominalAffineCleanupPlan,
            PlaceId,
            &[terminal_psi::StructuralPathSegment],
        ) -> Result<
            Vec<language_semantics::PermissionClaimIdentity>,
            LoweringError,
        >,
    ) -> Result<Vec<PreparedNormalCleanupAction>, LoweringError> {
        let mut nominal_scope: Option<(
            checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        )> = None;
        let actions = actions.into_iter().collect::<Vec<_>>();
        let partitions = prepare_cleanup_partitions(
            identities,
            checked,
            &actions,
            partition_refs,
            control,
            types,
            &mut resolve_type,
            &mut storage,
        )?;
        let mut prepared = actions
            .iter()
            .copied()
            .map(|action| {
                let leaf = action.partition_leaf();
                let partition_leaf = if leaf.partition.is_valid() {
                    let position = partitions
                        .binary_search_by_key(
                            &(leaf.partition.arena_index(), leaf.partition.generation()),
                            |partition| {
                                (partition.owner.arena_index(), partition.owner.generation())
                            },
                        )
                        .map_err(|_| LoweringError::InvalidCleanupPartition {
                            reason: "cleanup action lost its prepared partition owner",
                        })?;
                    let partition = &partitions[position];
                    Some(CleanupPartitionLeaf {
                        owner: partition.owner,
                        ordinal: leaf.ordinal,
                        root: partition.root.clone(),
                        leaf_count: partition.leaf_count,
                    })
                } else {
                    None
                };
                let (place, path) = storage(identities, action.source(), action.path())?;
                let retires_home = matches!(
                    action,
                    checked_trees::CheckedNormalCleanupAction::RetireHome { .. }
                );
                let selected = selected_regions
                    .iter()
                    .filter(|region| {
                        region.source == *action.source()
                            && (retires_home || action.path().starts_with(&region.path))
                    })
                    .max_by_key(|region| region.path.len());
                if nominal_scope.as_ref().is_some_and(|(source, prefix)| {
                    source != action.source() || !action.path().starts_with(prefix)
                }) || retires_home
                {
                    nominal_scope = None;
                }
                let selected_region = if selected.is_some() {
                    if nominal_scope.is_none()
                        && matches!(
                            action,
                            checked_trees::CheckedNormalCleanupAction::InvokeNominal(_)
                        )
                    {
                        nominal_scope = Some((action.source().clone(), action.path().to_vec()));
                    }
                    let scope = if retires_home {
                        &[][..]
                    } else {
                        nominal_scope
                            .as_ref()
                            .map_or(action.path(), |(_, path)| path)
                    };
                    let (place, path) = storage(identities, action.source(), scope)?;
                    Some(terminal_psi::StructuralArgument {
                        place,
                        path,
                        access: terminal_psi::StructuralAccess::Owned,
                    })
                } else {
                    nominal_scope = None;
                    None
                };
                let conditions = lower_cleanup_conditions(
                    identities,
                    checked,
                    control,
                    action.case_conditions(),
                    types,
                    |builder, path| storage(builder, action.source(), path),
                )?;
                let lowered = match action {
                    checked_trees::CheckedNormalCleanupAction::RetireHome { .. } => {
                        terminal_psi::TerminalAffineCleanupAction::RetireHome(place)
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_)
                        if path.is_empty() && conditions.is_empty() =>
                    {
                        if carrier_multiplicity(place)?
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                        {
                            terminal_psi::TerminalAffineCleanupAction::RetireHome(place)
                        } else {
                            terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place)
                        }
                    }
                    checked_trees::CheckedNormalCleanupAction::Discard(_) => {
                        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(
                            terminal_psi::StructuralAffineDiscard {
                                place,
                                path,
                                structural_type: resolve_type(identities, action.type_identity())?,
                            },
                        )
                    }
                    checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => {
                        let claims = consumed_claims(cleanup, place, &path)?;
                        return Ok(PreparedNormalCleanupAction::InvokeNominal {
                            partition_leaf,
                            selected_region,
                            conditions,
                            cleanup: cleanup.clone(),
                            place,
                            structural_type: resolve_type(identities, action.type_identity())?,
                            path,
                            consumed_claims: claims,
                        });
                    }
                };
                Ok(PreparedNormalCleanupAction::Resolved(CleanupRegion {
                    partition_leaf,
                    declaration_order: None,
                    selected_region,
                    conditions,
                    action: crate::unit::unit_cleanup::CleanupRegionAction::Run(lowered),
                }))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        for partition in partitions
            .iter()
            .rev()
            .filter(|partition| partition.leaf_count == 0)
        {
            let marker = PreparedNormalCleanupAction::PartitionOnly {
                owner: partition.owner,
                root: partition.root.clone(),
            };
            let position = actions
                .iter()
                .position(|action| action.source() == &partition.source)
                .unwrap_or(prepared.len());
            prepared.insert(position.min(prepared.len()), marker);
        }
        Ok(prepared)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_checked_cleanup_regions<'action>(
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        checked: &CheckedTrees,
        actions: impl IntoIterator<Item = &'action checked_trees::CheckedNormalCleanupAction>,
        partition_refs: &[checked_trees::CheckedCleanupPartitionRef],
        selected_regions: &[checked_trees::CheckedUnitStructuralArgumentPlan],
        control: &checked_trees::CheckedCleanupControlPlans,
        types: &[terminal_psi::StructuralTypeDeclaration],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        storage: impl FnMut(
            &mut terminal_psi::TerminalIdentityTableBuilder,
            &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
            &[checked_trees::CheckedUnitStructuralPathSegment],
        ) -> Result<
            (PlaceId, Vec<terminal_psi::StructuralPathSegment>),
            LoweringError,
        >,
        carrier_multiplicity: impl FnMut(
            PlaceId,
        )
            -> Result<terminal_psi::StructuralMultiplicity, LoweringError>,
        mut nominal: impl FnMut(
            &CheckedUnitNominalAffineCleanupPlan,
            PlaceId,
            StructuralTypeId,
            Vec<terminal_psi::StructuralPathSegment>,
        ) -> Result<NominalAffineCleanup, LoweringError>,
    ) -> Result<Vec<CleanupRegion>, LoweringError> {
        let prepared = prepare_checked_cleanup_regions(
            identities,
            checked,
            actions,
            partition_refs,
            selected_regions,
            control,
            types,
            |_, identity| lookup_type_id(type_ids, &identity),
            storage,
            carrier_multiplicity,
            |cleanup, _, _| Ok(cleanup.consumed_claims.clone()),
        )?;
        materialize_prepared_cleanup_regions(
            &prepared,
            |cleanup, place, structural_type, path, _| {
                nominal(cleanup, place, structural_type, path)
            },
        )
    }

    fn retain_cleanup_descendants(regions: &mut [CleanupRegion]) {
        for position in 0..regions.len().saturating_sub(1) {
            let (prefix, suffix) = regions.split_at_mut(position + 1);
            let CleanupRegionAction::Run(terminal_psi::TerminalAffineCleanupAction::InvokeNominal(
                cleanup,
            )) = &mut prefix[position].action
            else {
                continue;
            };
            let Some(next) = suffix.iter().find_map(|region| region.action.physical()) else {
                continue;
            };
            cleanup.retains_descendants = next.place() == cleanup.place
                && next.path().starts_with(&cleanup.path)
                && (next.path().len() > cleanup.path.len()
                    || matches!(
                        next,
                        terminal_psi::TerminalAffineCleanupAction::DiscardResidual(_)
                    ));
        }
    }

    pub(crate) fn computation_cleanups(
        checked: &CheckedTrees,
        machine: SymbolHandle,
    ) -> Result<Vec<&CheckedUnitNominalAffineCleanupPlan>, LoweringError> {
        let plans = &checked.facts.values.scalar_computations;
        let roots = plans.roots_for_machine(machine);
        let mut cleanups = Vec::new();
        for identity in reachable_nodes(checked, &roots)? {
            let retained = match plans.nodes.get(identity).kind {
                checked_trees::CheckedScalarComputationKind::Call {
                    temporary_cleanups, ..
                } => temporary_cleanups,
                _ => continue,
            };
            for row in plans.temporary_cleanups.span_or_empty(retained) {
                cleanups.extend(
                    plans
                        .temporary_cleanup_actions
                        .span_or_empty(row.actions)
                        .iter()
                        .filter_map(checked_trees::CheckedNormalCleanupAction::nominal),
                );
            }
        }
        cleanups.extend(retained_call_cleanups(checked, Some(machine)));
        let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
        cleanups.extend(
            control
                .value_completions
                .iter()
                .filter(|(_, row)| row.machine == machine)
                .flat_map(|(_, row)| control.actions.span_or_empty(row.actions))
                .filter_map(checked_trees::CheckedNormalCleanupAction::nominal),
        );
        Ok(cleanups)
    }

    pub(crate) fn selected_cleanups(
        checked: &CheckedTrees,
    ) -> impl Iterator<Item = &CheckedUnitNominalAffineCleanupPlan> {
        let plans = &checked.facts.flow.terminal_unit_effects;
        let composed_states = || {
            plans
                .composed_machines
                .iter()
                .flat_map(|machine| &machine.states)
        };
        value_facts(checked)
            .scalar_computations
            .temporary_cleanup_actions
            .iter()
            .filter_map(|(_, action)| action.nominal())
            .chain(plans.machines.iter().flat_map(|machine| {
                continuation_cleanups(&plans.cleanup_control, &machine.operations)
            }))
            .chain(
                composed_states().flat_map(|state| {
                    continuation_cleanups(&plans.cleanup_control, &state.operations)
                }),
            )
            .chain(retained_call_cleanups(checked, None))
            .chain(
                plans
                    .cleanup_control
                    .value_completions
                    .iter()
                    .flat_map(|(_, row)| plans.cleanup_control.actions.span_or_empty(row.actions))
                    .filter_map(checked_trees::CheckedNormalCleanupAction::nominal),
            )
            .chain(
                plans
                    .machines
                    .iter()
                    .map(|machine| machine.normal_exit_cleanups)
                    .chain(composed_states().map(|state| state.normal_exit_cleanups))
                    .flat_map(move |rows| plans.cleanup_control.normal_actions(rows))
                    .filter_map(checked_trees::CheckedNormalCleanupAction::nominal),
            )
    }

    pub(crate) fn retained_call_cleanups(
        checked: &CheckedTrees,
        machine: Option<SymbolHandle>,
    ) -> impl Iterator<Item = &CheckedUnitNominalAffineCleanupPlan> {
        let control = &checked.facts.flow.terminal_unit_effects.cleanup_control;
        value_facts(checked)
            .structural_values
            .scheduled_calls()
            .filter(move |(_, call)| {
                machine.is_none_or(|machine| {
                    value_facts(checked)
                        .structural_values
                        .nodes
                        .get(call.value)
                        .machine
                        == machine
                })
            })
            .flat_map(move |(_, call)| {
                continuation_cleanups(
                    control,
                    call.operands().iter().chain(
                        call.completions()
                            .into_iter()
                            .flat_map(|sequence| sequence.iter()),
                    ),
                )
            })
    }

    pub(crate) fn lower_nominal_cleanup_action(
        cleanup: &CheckedUnitNominalAffineCleanupPlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        machine_ids: &[(SymbolHandle, MachineId)],
        receivers: &[(SymbolHandle, PlaceId)],
        requirement_counts: &[(SymbolHandle, usize)],
        next_obligation: &mut u64,
    ) -> Result<NominalAffineCleanup, LoweringError> {
        materialize_nominal_cleanup_action(
            cleanup,
            place,
            structural_type,
            path,
            machine_ids,
            receivers.iter().find_map(|(source, receiver)| {
                (*source == cleanup.cleanup_machine).then_some(*receiver)
            }),
            requirement_count(requirement_counts, cleanup.cleanup_machine)?,
            &mut || {
                crate::shared::terminal_identities::allocate_dense(next_obligation)
                    .map(obligation_id)
            },
        )
    }

    pub(crate) fn bind_consumed_claims(
        action: &mut NominalAffineCleanup,
        consumed_claims: &[language_semantics::PermissionClaimIdentity],
        bindings: &[(
            language_semantics::PermissionClaimIdentity,
            semantic_vocabulary::ClaimId,
        )],
    ) -> Result<(), LoweringError> {
        action.consumed_claims = consumed_claims
            .iter()
            .map(|identity| {
                crate::shared::terminal_identities::lookup_claim_id(bindings, *identity)
            })
            .collect::<Result<Vec<_>, _>>()?;
        action.consumed_claims.sort_unstable();
        if action
            .consumed_claims
            .windows(2)
            .any(|claims| claims[0] == claims[1])
        {
            return Err(LoweringError::Unsupported(
                "nominal cleanup repeats its consumed claim",
            ));
        }
        Ok(())
    }

    fn materialize_nominal_cleanup_action(
        cleanup: &CheckedUnitNominalAffineCleanupPlan,
        place: PlaceId,
        structural_type: StructuralTypeId,
        path: Vec<terminal_psi::StructuralPathSegment>,
        machine_ids: &[(SymbolHandle, MachineId)],
        cleanup_receiver: Option<PlaceId>,
        requirement_count: usize,
        allocate_requirement: &mut impl FnMut() -> Result<
            semantic_vocabulary::ObligationId,
            LoweringError,
        >,
    ) -> Result<NominalAffineCleanup, LoweringError> {
        Ok(NominalAffineCleanup {
            retains_descendants: false,
            place,
            path,
            structural_type,
            cleanup_machine: lookup_machine_id(machine_ids, cleanup.cleanup_machine)?,
            consumed_claims: Vec::new(),
            cleanup_receiver,
            requirement_obligations: (0..requirement_count)
                .map(|_| allocate_requirement())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    fn requirement_count(
        counts: &[(SymbolHandle, usize)],
        machine: SymbolHandle,
    ) -> Result<usize, LoweringError> {
        counts
            .iter()
            .find_map(|(source, count)| (*source == machine).then_some(*count))
            .ok_or(LoweringError::Unsupported(
                "nominal cleanup target prerequisite signature is absent",
            ))
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum PreparedNormalCleanupAction {
        Resolved(CleanupRegion),
        PartitionOnly {
            owner: arena::Handle<checked_trees::CheckedCleanupPartitionPlan>,
            root: std::sync::Arc<CleanupPartitionNode>,
        },
        InvokeNominal {
            partition_leaf: Option<CleanupPartitionLeaf>,
            selected_region: Option<terminal_psi::StructuralArgument>,
            conditions: Vec<CleanupCaseCondition>,
            cleanup: CheckedUnitNominalAffineCleanupPlan,
            place: PlaceId,
            structural_type: StructuralTypeId,
            path: Vec<terminal_psi::StructuralPathSegment>,
            consumed_claims: Vec<language_semantics::PermissionClaimIdentity>,
        },
    }

    impl PreparedNormalCleanupAction {
        pub(crate) fn discard_root(place: PlaceId) -> Self {
            Self::Resolved(CleanupRegion::unconditional(
                terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place),
            ))
        }
    }

    impl From<&CleanupRegion> for PreparedNormalCleanupAction {
        fn from(region: &CleanupRegion) -> Self {
            Self::Resolved(region.clone())
        }
    }

    pub(crate) fn emit_prepared_cleanup_regions(
        actions: &[PreparedNormalCleanupAction],
        calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
    ) -> Result<Vec<CleanupRegion>, LoweringError> {
        materialize_prepared_cleanup_regions(
            actions,
            |cleanup, place, structural_type, path, consumed_claims| {
                let machine_ids = calls.machine_ids;
                let receiver = calls
                    .cleanup_receivers
                    .iter()
                    .find_map(|(source, receiver)| {
                        (*source == cleanup.cleanup_machine).then_some(*receiver)
                    });
                let mut action = materialize_nominal_cleanup_action(
                    cleanup,
                    place,
                    structural_type,
                    path,
                    machine_ids,
                    receiver,
                    requirement_count(calls.requirement_counts, cleanup.cleanup_machine)?,
                    &mut || calls.allocate_requirement(),
                )?;
                bind_consumed_claims(&mut action, consumed_claims, calls.claims.bindings())?;
                Ok(action)
            },
        )
    }

    fn validate_cleanup_partition(
        root: &CleanupPartitionNode,
        expected: &CleanupStorage,
        expected_type: StructuralTypeId,
        leaf_count: usize,
        types: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(), LoweringError> {
        let CleanupPartitionNode::Scope {
            place,
            path,
            structural_type,
            ..
        } = root
        else {
            return Err(LoweringError::InvalidCleanupPartition {
                reason: "cleanup partition has no carrier root",
            });
        };
        if *place != expected.0 || path != &expected.1 || *structural_type != expected_type {
            return Err(LoweringError::InvalidCleanupPartition {
                reason: "cleanup partition names another carrier or storage version",
            });
        }
        fn visit(
            node: &CleanupPartitionNode,
            expected: &CleanupStorage,
            expected_type: StructuralTypeId,
            types: &[terminal_psi::StructuralTypeDeclaration],
            leaves: &mut Vec<u32>,
        ) -> Result<(), LoweringError> {
            let (place, path, structural_type) = match node {
                CleanupPartitionNode::Leaf(ordinal) => {
                    leaves.push(*ordinal);
                    return Ok(());
                }
                CleanupPartitionNode::Scope {
                    place,
                    path,
                    structural_type,
                    ..
                }
                | CleanupPartitionNode::Cases {
                    place,
                    path,
                    structural_type,
                    ..
                } => (*place, path, *structural_type),
            };
            let relative = path.strip_prefix(expected.1.as_slice()).ok_or(
                LoweringError::InvalidCleanupPartition {
                    reason: "cleanup partition scope is outside its carrier",
                },
            )?;
            if place != expected.0
                || terminal_semantics::runtime_structural_path_tip(
                    types.iter(),
                    expected_type,
                    relative,
                )
                .is_none_or(|declaration| declaration.id != structural_type)
            {
                return Err(LoweringError::InvalidCleanupPartition {
                    reason: "cleanup partition scope has another root or type",
                });
            }
            match node {
                CleanupPartitionNode::Scope { steps, .. } => {
                    for step in steps {
                        visit(step, expected, expected_type, types, leaves)?;
                    }
                }
                CleanupPartitionNode::Cases { arms, common, .. } => {
                    let declaration = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        expected_type,
                        relative,
                    )
                    .ok_or(LoweringError::InvalidCleanupPartition {
                        reason: "cleanup alternatives lost their declared carrier",
                    })?;
                    let cases = match &declaration.shape {
                        terminal_psi::StructuralTypeShape::Sum { cases }
                        | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
                        _ => {
                            return Err(LoweringError::InvalidCleanupPartition {
                                reason: "cleanup alternatives require a sum carrier",
                            });
                        }
                    };
                    if arms.len() != cases.len()
                        || arms
                            .iter()
                            .enumerate()
                            .any(|(position, arm)| arm.case != cases[position].id)
                    {
                        return Err(LoweringError::InvalidCleanupPartition {
                            reason: "cleanup partition omits or reorders a declared alternative",
                        });
                    }
                    let reachable = arms
                        .iter()
                        .filter(|arm| {
                            arm.reachability
                                != checked_trees::CheckedCleanupCaseReachability::Inactive
                        })
                        .count();
                    if arms.iter().any(|arm| {
                        arm.reachability == checked_trees::CheckedCleanupCaseReachability::Selected
                    }) && reachable != 1
                    {
                        return Err(LoweringError::InvalidCleanupPartition {
                            reason: "selected cleanup alternative has competing reachable cases",
                        });
                    }
                    for arm in arms {
                        if arm.reachability
                            == checked_trees::CheckedCleanupCaseReachability::Inactive
                            && !arm.steps.is_empty()
                        {
                            return Err(LoweringError::InvalidCleanupPartition {
                                reason: "unreachable cleanup alternative has actions",
                            });
                        }
                        for step in &arm.steps {
                            visit(step, expected, expected_type, types, leaves)?;
                        }
                    }
                    for step in common {
                        visit(step, expected, expected_type, types, leaves)?;
                    }
                }
                CleanupPartitionNode::Leaf(_) => {}
            }
            Ok(())
        }
        let mut leaves = Vec::new();
        visit(root, expected, expected_type, types, &mut leaves)?;
        if leaves
            .iter()
            .copied()
            .map(|ordinal| ordinal as usize)
            .ne(0..leaf_count)
        {
            return Err(LoweringError::InvalidCleanupPartition {
                reason: "cleanup partition omits, repeats or reorders an action leaf",
            });
        }
        Ok(())
    }

    fn materialize_prepared_cleanup_regions(
        actions: &[PreparedNormalCleanupAction],
        mut nominal: impl FnMut(
            &CheckedUnitNominalAffineCleanupPlan,
            PlaceId,
            StructuralTypeId,
            Vec<terminal_psi::StructuralPathSegment>,
            &[language_semantics::PermissionClaimIdentity],
        ) -> Result<NominalAffineCleanup, LoweringError>,
    ) -> Result<Vec<CleanupRegion>, LoweringError> {
        let mut regions = actions
            .iter()
            .map(|action| {
                Ok(match action {
                    PreparedNormalCleanupAction::Resolved(action) => action.clone(),
                    PreparedNormalCleanupAction::PartitionOnly { owner, root } => {
                        let CleanupPartitionNode::Scope { place, path, .. } = root.as_ref() else {
                            return Err(LoweringError::InvalidCleanupPartition {
                                reason: "zero-action cleanup partition has no carrier scope",
                            });
                        };
                        CleanupRegion {
                            partition_leaf: Some(CleanupPartitionLeaf {
                                owner: *owner,
                                ordinal: 0,
                                root: root.clone(),
                                leaf_count: 0,
                            }),
                            declaration_order: None,
                            selected_region: None,
                            conditions: Vec::new(),
                            action: CleanupRegionAction::CheckedPartition {
                                place: *place,
                                path: path.clone(),
                            },
                        }
                    }
                    PreparedNormalCleanupAction::InvokeNominal {
                        partition_leaf,
                        selected_region,
                        conditions,
                        cleanup,
                        place,
                        structural_type,
                        path,
                        consumed_claims,
                    } => {
                        let action = nominal(
                            cleanup,
                            *place,
                            *structural_type,
                            path.clone(),
                            consumed_claims,
                        )?;
                        CleanupRegion {
                            partition_leaf: partition_leaf.clone(),
                            declaration_order: None,
                            selected_region: selected_region.clone(),
                            conditions: conditions.clone(),
                            action: crate::unit::unit_cleanup::CleanupRegionAction::Run(
                                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(action),
                            ),
                        }
                    }
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        retain_cleanup_descendants(&mut regions);
        Ok(regions)
    }

    pub(crate) fn bind_nominal_cleanup_subjects(
        checked: &CheckedTrees,
        machines: &mut [TerminalMachine],
        type_ids: &[(typed_trees::type_identity::TypeIdentityId, StructuralTypeId)],
        machine_ids: &[(SymbolHandle, MachineId)],
    ) -> Result<(), LoweringError> {
        for cleanup in selected_cleanups(checked) {
            let Ok(target_id) = lookup_machine_id(machine_ids, cleanup.cleanup_machine) else {
                continue;
            };
            let owner = lookup_type_id(type_ids, &cleanup.type_identity)?;
            let emitted = machines
                .iter_mut()
                .find(|machine| machine.id == target_id)
                .ok_or(LoweringError::Unsupported(
                    "nominal cleanup target was not emitted",
                ))?;
            emitted.attachment = Some(owner);
            emitted.nominal_cleanup_subject =
                nominal_cleanup_subject(checked, cleanup.cleanup_machine, type_ids)?;
        }
        Ok(())
    }
}

pub(crate) fn lower_unit_plan_machine(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    plan: CheckedUnitPlan<'_>,
    program_roots: crate::TerminalProgramRoots<'_>,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
) -> Result<LoweredSelectedMachine, LoweringError> {
    Ok(match plan {
        CheckedUnitPlan::NominalAffineCleanup(plan) => LoweredSelectedMachine::source_mapped(
            lower_nominal_affine_unit_cleanup_machine(
                identities,
                checked,
                plan,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        ),
        CheckedUnitPlan::PartialAffineCleanup(plan) => LoweredSelectedMachine::source_mapped(
            lower_unit_effect_closure(
                identities,
                checked,
                plan.machine.machine,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        ),
        CheckedUnitPlan::ComposedControl(plan) => LoweredSelectedMachine::source_mapped(
            lower_unit_effect_closure(
                identities,
                checked,
                plan.machine,
                program_roots,
                exact_toolchain_sources,
            )?,
            LoweringCompletion {
                debug: DebugPublication::Omit,
            },
        ),
    })
}
