//! Read genuine prepared lifecycle occurrences through the same ordinary
//! boundary namespace and call lowering as other compiler-owned contracts.

use semantic_vocabulary::MachineId;
use terminal_check_tables::{Checked, Construct};
use typed_trees::references::StaticArgumentCall;

use super::{Program, Slot};
use crate::body::{CallTarget, Expression, ValueType};
use crate::{SourceOrigin, Unserved, refuse};

impl Program<'_> {
    pub(super) fn access_lifecycle_call(
        &mut self,
        id: MachineId,
        slots: &[Slot],
        call: StaticArgumentCall<'_>,
        source: SourceOrigin,
        expected: Option<ValueType>,
    ) -> Checked<Option<Expression>> {
        let typed = self.typed;
        let authored = match call {
            StaticArgumentCall::Expression(_, call) => call.target_symbol,
            StaticArgumentCall::Statement(call) => call.target_symbol,
        };
        let Some(machine) = self
            .typed
            .machine_by_symbol(authored)
            .or_else(|| typed.machine_holding_state(authored))
        else {
            return Ok(None);
        };
        let Some(execution) =
            typed_trees::AccessExecutionDeclaration::authenticate(typed, machine.symbol)
                .map_err(|diagnostic| super::services::unavailable(id, diagnostic.message))?
        else {
            return Ok(None);
        };
        let target = self.call_target_symbol(id, authored)?;
        let (selection, binding, arguments, unsupported) = match call {
            StaticArgumentCall::Expression(_, call) => (
                call.authored_call_selection,
                call.authored_call_binding,
                typed.expression_table.expression_handles(call.arguments),
                (call.receiver.is_valid() && !typed.call_has_no_runtime_receiver_for_target(call))
                    || !call.evidence_arguments.is_empty()
                    || call.requirement_application.is_valid()
                    || call.exact_coercion_target.is_valid()
                    || call.build_selection.is_valid()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.layout_query.is_some()
                    || call.native_code_layout.is_some()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block,
            ),
            StaticArgumentCall::Statement(call) => (
                call.authored_call_selection,
                None,
                typed.statement_table.expression_handles(call.arguments),
                call.receiver_root_symbol.is_valid()
                    || call.receiver_symbol.is_valid()
                    || !call.receiver.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.installed_interrupt_operation.is_some()
                    || call.reflected_field_origin.is_some()
                    || call.operational_acknowledgement.acknowledges_suspend
                    || call.operational_acknowledgement.acknowledges_block,
            ),
        };
        let unavailable = |detail| refuse(id, Construct::Machine, Unserved::Call(detail));
        self.visible_to(id, target)?;
        self.direct_selection(id, authored, selection, binding)?;
        if unsupported {
            return Err(unavailable(
                "placement lifecycle has another call feature or runtime receiver",
            ));
        }
        let application = call.application();
        let prepared = typed
            .bodyless_callable_signature(application, target)
            .map_err(unavailable)?
            .ok_or_else(|| unavailable("placement has no prepared closed source contract"))?;
        let occurrence = typed
            .expression_table
            .static_machine_application(application);
        if self.symbols.get(id).copied() != Some(occurrence.lexical_owner)
            || occurrence.binding_declaration != target
        {
            return Err(unavailable(
                "placement lifecycle belongs to another source call owner",
            ));
        }
        let authored_arguments = match call {
            StaticArgumentCall::Expression(_, call) => &call.machine_arguments,
            StaticArgumentCall::Statement(call) => &call.machine_arguments,
        };
        if !authored_arguments.is_empty()
            && authored_arguments.as_ref()
                != typed
                    .expression_table
                    .static_machine_application_arguments(application)
        {
            return Err(unavailable(
                "placement lifecycle changed its authored static arguments",
            ));
        }
        let mut plans = typed.access_lifecycle_plans.iter().filter(|(_, plan)| matches!(plan,
            typed_trees::AccessLifecyclePlan::Native { callable, application: actual, operation, .. }
                if *callable == target && *actual == application && *operation == execution.operation));
        let (Some((plan_handle, typed_trees::AccessLifecyclePlan::Native { schema, .. })), None) =
            (plans.next(), plans.next())
        else {
            return Err(unavailable(
                "placement lost its unique forward-evaluated policy request",
            ));
        };
        let element = self.value_type(id, *schema)?;
        let boundary = match self
            .boundaries
            .application(typed, application, target)
            .map_err(unavailable)?
        {
            Some(boundary) => boundary,
            None => {
                let (signature, _) = self.declared_requirement_signature(id, prepared, &[])?;
                let machine = typed
                    .machine_by_symbol(target)
                    .ok_or_else(|| unavailable("placement lifecycle lost its source machine"))?;
                let Some([entry]) = typed.machine_states(machine).as_array() else {
                    return Err(unavailable("placement lifecycle lost its exact entry"));
                };
                let declaration = crate::declare::boundary_source(
                    typed,
                    target,
                    target,
                    entry.symbol,
                    prepared,
                    typed
                        .machine_callable_identity_text(machine)
                        .ok_or_else(|| {
                            unavailable("placement lifecycle lost its template callable")
                        })?,
                )
                .map_err(unavailable)?;
                let envelope = self.signature_envelope(id, prepared)?;
                let boundary = self
                    .boundaries
                    .insert_application(
                        typed,
                        crate::boundaries::BoundarySource {
                            source: target,
                            owner: target,
                            kind: semantic_vocabulary::BoundaryRequirementKind::Machine,
                            signature,
                            envelope,
                            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                            owning_service: None,
                            declaration,
                            lifecycle: Some(crate::boundaries::AccessLifecycleBoundarySource {
                                application,
                                plan: plan_handle,
                                element,
                            }),
                            run: None,
                        },
                    )
                    .map_err(unavailable)?;
                self.admitted_declarations.include(target);
                boundary
            }
        };
        self.call_of(
            id,
            slots,
            (CallTarget::Boundary { boundary }, None),
            arguments,
            source,
            expected,
        )
        .map(Some)
    }
}

impl Program<'_> {
    pub(crate) fn publish_access_lifecycle_plans(
        &self,
        diagnostic: MachineId,
        module: &mut terminal_psi::TerminalModule,
    ) -> Checked<()> {
        use language_core::core_vocabulary::{CoreVocabulary, ExtentDomain, RunDeclaration};
        use terminal_psi::{
            BoundaryMachineResult, StructuralTypeShape, TerminalAccessLifecycleExecution,
            TerminalAccessLifecycleOperation, TerminalAccessLifecyclePlan, TerminalAccessOwner,
            TerminalAccessPlacement, TerminalAccessProfileRequirement, TerminalAccessStorage,
        };
        if !self
            .boundaries
            .rows
            .iter()
            .any(|(_, row)| row.lifecycle.is_some())
        {
            return Ok(());
        }
        let unavailable = |detail| super::services::unavailable(diagnostic, detail);
        let sources =
            source::source_identity::toolchain_source_identities(self.typed.symbols.source_files())
                .map_err(|_| unavailable("placement publication lost its exact source owner"))?;
        let mut identities = std::mem::take(&mut module.identities).into_builder();
        let result = (|| {
            for (boundary_id, source) in self.boundaries.rows.iter() {
                let Some(lifecycle) = source.lifecycle else {
                    continue;
                };
                let typed_trees::AccessLifecyclePlan::Native {
                    callable,
                    application,
                    operation,
                    policy_selection,
                    schema: schema_type,
                    policy_machine,
                    normalized,
                } = self.typed.access_lifecycle_plans.get(lifecycle.plan)
                else {
                    return Err(unavailable(
                        "placement source lost its forward policy demand",
                    ));
                };
                if *callable != source.source || *application != lifecycle.application {
                    return Err(unavailable(
                        "placement source has another evaluated occurrence",
                    ));
                }
                let declaration =
                    typed_trees::AccessExecutionDeclaration::authenticate(self.typed, *callable)
                        .map_err(|diagnostic| unavailable(&diagnostic.message))?
                        .ok_or_else(|| unavailable("placement source lost its sealed operation"))?;
                if declaration.operation != *operation {
                    return Err(unavailable(
                        "placement source changed its evaluated operation",
                    ));
                }
                if declaration.content == typed_trees::AccessContentShape::Resident {
                    let bindings = self
                        .typed
                        .expression_table
                        .static_machine_application_bindings(*application);
                    let Some(typed_trees::static_applications::StaticMachineBinding {
                        value:
                            typed_trees::static_applications::StaticMachineBindingValue::Type(storage),
                        ..
                    }) = bindings.get(2)
                    else {
                        return Err(unavailable(
                            "resident placement lost its actual Storage application",
                        ));
                    };
                    let layout = self.typed.layout_storage_arguments(*storage)
                        .ok_or_else(|| unavailable("resident placement Storage is not an actual LayoutStorage application"))?;
                    if self.typed.normalized_type_identity(layout.semantic_type)
                        != self.typed.normalized_type_identity(*schema_type)
                    {
                        return Err(unavailable(
                            "resident placement Storage denotes another semantic type",
                        ));
                    }
                    // The explicit authored storage needs its evaluated layout
                    // correspondence. A default native demand would erase it.
                    return Err(unavailable(
                        "resident placement needs its selected LayoutStorage geometry",
                    ));
                }
                let operation = match operation {
                    typed_trees::AccessLifecycleOperation::Initialize => {
                        TerminalAccessLifecycleOperation::Initialize
                    }
                    typed_trees::AccessLifecycleOperation::MoveOut => {
                        TerminalAccessLifecycleOperation::MoveOut
                    }
                    typed_trees::AccessLifecycleOperation::View => {
                        TerminalAccessLifecycleOperation::View
                    }
                    typed_trees::AccessLifecycleOperation::Relocate => {
                        TerminalAccessLifecycleOperation::Relocate
                    }
                };
                let schema_identity = self
                    .typed
                    .type_identity_text(self.typed.normalized_type_identity(*schema_type))
                    .filter(|identity| !identity.is_empty())
                    .ok_or_else(|| {
                        unavailable("placement schema lost its exact applied identity")
                    })?;
                let schema_identity = identities.intern(&schema_identity);
                let scalar_schema = matches!(lifecycle.element, ValueType::Scalar(..));
                let schema = match lifecycle.element {
                    ValueType::Record(schema, ..) => schema,
                    ValueType::Scalar(scalar, _) => crate::lower::places::declare_scalar_type(
                        &mut module.structural_types,
                        schema_identity,
                        scalar,
                    )
                    .map_err(unavailable)?,
                    ValueType::Rigid(_) => {
                        return Err(unavailable("placement schema is still open"));
                    }
                };
                // Body/type IDs are fixed by the existing source reader. Resolve
                // the owner through the actual input or Ready payload only.
                let boundary = module
                    .boundary_machine(boundary_id)
                    .ok_or_else(|| unavailable("placement demand lost its boundary"))?;
                let owner = if operation == TerminalAccessLifecycleOperation::Initialize {
                    let BoundaryMachineResult::Structural(result) = &boundary.result else {
                        return Err(unavailable(
                            "placement initialization lost its owned outcome",
                        ));
                    };
                    let Some(StructuralTypeShape::Sum { cases }) = module
                        .structural_type(result.structural_type)
                        .map(|row| &row.shape)
                    else {
                        return Err(unavailable("placement initialization lost its Ready case"));
                    };
                    let [ready, _] = cases.as_slice() else {
                        return Err(unavailable(
                            "placement initialization changed its outcome roster",
                        ));
                    };
                    let [field] = ready.fields.as_slice() else {
                        return Err(unavailable("placement Ready changed its owned payload"));
                    };
                    field
                        .field_type
                        .as_structural()
                        .ok_or_else(|| unavailable("placement Ready lost its storage owner"))?
                } else {
                    boundary
                        .structural_parameters
                        .first()
                        .ok_or_else(|| unavailable("placement lost its original owner input"))?
                        .structural_type
                };
                let owner_type = module
                    .structural_type(owner)
                    .ok_or_else(|| unavailable("placement owner carrier is absent"))?;
                let (carrier, owner) = if declaration.content
                    == typed_trees::AccessContentShape::Run
                {
                    let sequence = owner_type.retained_sequence.as_ref().ok_or_else(|| {
                        unavailable("placement Run owner lost its retained cell roles")
                    })?;
                    let StructuralTypeShape::Record { fields, .. } = &owner_type.shape else {
                        return Err(unavailable("placement Run owner lost its exact descriptor"));
                    };
                    let carrier = fields
                        .iter()
                        .find(|field| field.id == sequence.storage)
                        .and_then(|field| field.field_type.as_structural())
                        .ok_or_else(|| unavailable("placement Run owner lost its backing field"))?;
                    let singleton = if operation == TerminalAccessLifecycleOperation::Initialize {
                        Some(self.lifecycle_domain(
                            diagnostic,
                            module,
                            owner_type.id,
                            CoreVocabulary::Run(RunDeclaration::Singleton),
                            &sources,
                            &mut identities,
                        )?)
                    } else {
                        None
                    };
                    (
                        carrier,
                        TerminalAccessOwner::Run {
                            carrier: owner_type.id,
                            storage: sequence.storage,
                            count: sequence.count,
                            singleton,
                        },
                    )
                } else {
                    (owner, TerminalAccessOwner::Extent)
                };
                let extent_source = self
                    .record_sources
                    .get(carrier)
                    .ok_or_else(|| unavailable("placement backing has no source declaration"))?;
                if self.typed.core_vocabulary(extent_source.declaration)
                    != Some(CoreVocabulary::Extent)
                {
                    return Err(unavailable(
                        "placement backing is not the exact core Extent",
                    ));
                }
                let Some(StructuralTypeShape::Record { fields, .. }) =
                    module.structural_type(carrier).map(|row| &row.shape)
                else {
                    return Err(unavailable(
                        "placement backing lost its exact descriptor fields",
                    ));
                };
                let [base, length] = fields.as_slice() else {
                    return Err(unavailable(
                        "placement backing changed its base/length roster",
                    ));
                };
                let (base, length) = (base.id, length.id);
                let granted = self.lifecycle_domain(
                    diagnostic,
                    module,
                    carrier,
                    CoreVocabulary::ExtentDomain(ExtentDomain::Granted),
                    &sources,
                    &mut identities,
                )?;
                let (vacant, resident) = if operation == TerminalAccessLifecycleOperation::View {
                    (None, None)
                } else {
                    (
                        Some(self.lifecycle_domain(
                            diagnostic,
                            module,
                            carrier,
                            CoreVocabulary::ExtentDomain(ExtentDomain::Vacant),
                            &sources,
                            &mut identities,
                        )?),
                        Some(self.lifecycle_domain(
                            diagnostic,
                            module,
                            match owner {
                                TerminalAccessOwner::Extent => carrier,
                                TerminalAccessOwner::Run { carrier, .. } => carrier,
                            },
                            if declaration.content == typed_trees::AccessContentShape::Run {
                                CoreVocabulary::Run(RunDeclaration::Live)
                            } else {
                                CoreVocabulary::ExtentDomain(ExtentDomain::Resident)
                            },
                            &sources,
                            &mut identities,
                        )?),
                    )
                };
                let boundary = module.boundary_machine(boundary_id).ok_or_else(|| {
                    unavailable("placement demand lost its boundary during publication")
                })?;
                let profile_type = boundary
                    .structural_parameters
                    .last()
                    .ok_or_else(|| unavailable("placement lost its original profile input"))?
                    .structural_type;
                let bindings = self
                    .typed
                    .expression_table
                    .static_machine_application_bindings(*application);
                let role_count = if operation == TerminalAccessLifecycleOperation::Relocate {
                    2
                } else {
                    1
                };
                let mut profile_requirements = Vec::with_capacity(role_count);
                let roles = bindings
                    .get(2..2 + role_count)
                    .ok_or_else(|| unavailable("placement lost its exact supply role telescope"))?;
                for role in roles {
                    let typed_trees::static_applications::StaticMachineBindingValue::Type(role) =
                        role.value
                    else {
                        return Err(unavailable(
                            "placement profile selector is not a closed type role",
                        ));
                    };
                    let role = self.typed.normalized_type_identity(role);
                    let mut matches = self.boundaries.rows.iter().filter(|(_, candidate)| {
                        self.typed
                            .closed_requirement_application(candidate.source)
                            .is_some_and(|closed| {
                                self.typed.core_vocabulary(closed.declaring_trait)
                                    == Some(CoreVocabulary::ResourceProfileSupply)
                                    && self.typed.core_vocabulary(closed.template)
                                        == Some(CoreVocabulary::ResourceProfileSupplyProfile)
                                    && closed.trait_arguments.len() == 1
                                    && self
                                        .typed
                                        .normalized_type_identity(closed.trait_arguments[0])
                                        == role
                            })
                    });
                    let (Some((id, _)), None) = (matches.next(), matches.next()) else {
                        return Err(unavailable(
                            "placement lost its unique original profile supply role",
                        ));
                    };
                    let supply = module
                        .boundary_machine(id)
                        .ok_or_else(|| unavailable("placement profile supply was not published"))?;
                    profile_requirements.push(TerminalAccessProfileRequirement {
                        boundary: id,
                        owner: supply.owner,
                        callable: supply.identity,
                    });
                }
                let policy = crate::declare::implementation_argument_identity(
                    self.typed,
                    self.typed
                        .static_machine_application(*application)
                        .lexical_owner,
                    policy_selection,
                )
                .map_err(unavailable)?;
                let placement = crate::qualifications::bind_lifecycle_access_plan(
                    diagnostic,
                    self,
                    schema,
                    *schema_type,
                    scalar_schema,
                    normalized,
                    &mut identities,
                )?;
                let policy_plan_machine_identity =
                    crate::declare::semantic_identity(self.typed, *policy_machine, &mut identities)
                        .map_err(unavailable)?;
                module
                    .access_lifecycle_plans
                    .push(TerminalAccessLifecyclePlan {
                        boundary: boundary_id,
                        target: None,
                        operation,
                        execution: TerminalAccessLifecycleExecution::CompilerPrimitive {
                            profile_type: Some(profile_type),
                            profile_requirements,
                        },
                        storage: TerminalAccessStorage {
                            carrier,
                            owner,
                            base,
                            length,
                            granted,
                            vacant,
                            resident,
                        },
                        schema,
                        image_fields: Vec::new(),
                        policy_identity: identities.intern(&policy),
                        policy_plan_machine_identity,
                        schema_identity,
                        placement: TerminalAccessPlacement::Native(placement),
                    });
            }
            Ok(())
        })();
        module.identities = identities.build();
        result
    }

    fn lifecycle_domain(
        &self,
        diagnostic: MachineId,
        module: &terminal_psi::TerminalModule,
        carrier: semantic_vocabulary::StructuralTypeId,
        role: language_core::core_vocabulary::CoreVocabulary,
        sources: &[(source::SourceId, [u8; 32])],
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    ) -> Checked<semantic_vocabulary::StructuralDomainId> {
        let unavailable = |detail| super::services::unavailable(diagnostic, detail);
        let mut candidates = self.declared_domains.iter().filter(|row| {
            matches!(row.carrier, ValueType::Record(actual, ..) if actual == carrier)
                && self.typed.core_vocabulary(row.instance.declaration) == Some(role)
        });
        let (Some(source), None) = (candidates.next(), candidates.next()) else {
            return Err(unavailable(
                "placement lost its exact applied core content/grant domain",
            ));
        };
        let identity = crate::qualifications::domain_identity(
            self.typed,
            sources,
            source.instance,
            identities,
        )
        .map_err(unavailable)?;
        module
            .structural_domains
            .iter()
            .find(|row| row.carrier == carrier && row.identity == identity)
            .map(|row| row.id)
            .ok_or_else(|| unavailable("placement domain has no canonical declaration"))
    }
}
