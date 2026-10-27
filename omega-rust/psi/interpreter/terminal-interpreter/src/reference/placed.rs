//! Placed interpretation of initialized RAM or retained provider storage.
//!
//! The allocation's declared layout models an aligned arena allocation, never
//! a host address. Captured subregions retain that allocation and its live frame
//! home; an opaque identity is used only to join storage, never as an address.
//! Native execution must separately discharge the concrete-address condition.
//! Resident formation instead captures the original extent's live custody and
//! provider allocation. Both backings use ordinary reference/loan transport;
//! event dispatch preserves their distinct geometry and authority contracts.

use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{
    StructuralRuntimePlace, StructuralScalarRuntimeField, TerminalStructuralValue,
};
use semantic_vocabulary::{MachineId, PlaceId};
use terminal_psi::{
    Operation, StructuralAccess, StructuralAffineDiscard, StructuralArgument,
    StructuralMultiplicity, StructuralTypeShape,
};

mod view;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedReference {
    storage: PlacedStorage,
    pub(crate) presentation: TerminalStructuralValue,
    pub(crate) access: StructuralAccess,
    pub(crate) selected_field: Option<u32>,
    /// Captured coordinates below the permission-bearing field, never a new
    /// backing or independent element permission. Runtime selectors are frozen
    /// by ordinary reference projection before entering this path.
    projection: Vec<terminal_psi::StructuralPathSegment>,
    /// The original source owner: initialized T for RAM, qualified Extent for
    /// resident backing. Presentation fields never replace its custody.
    pub(crate) source: TerminalStructuralValue,
}

/// Ordinary initialized objects and resident provider allocations have distinct
/// backing. In particular, an Extent's two descriptor fields are not T's bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum PlacedStorage {
    Ram {
        correspondence: terminal_semantics::PlacedRamCorrespondence,
        home: PlacedRamHome,
        byte_offset: u64,
    },
    Resident {
        carrier: semantic_vocabulary::StructuralTypeId,
        owner: StructuralRuntimePlace,
        binding: crate::provider_storage::ProviderStorageBinding,
    },
    External {
        carrier: semantic_vocabulary::StructuralTypeId,
        owner: StructuralRuntimePlace,
        binding: crate::provider_storage::ProviderStorageBinding,
        compatibility: access_plans::PlacementResourceCompatibility,
    },
}

impl PlacedReference {
    pub(crate) fn carrier(&self) -> semantic_vocabulary::StructuralTypeId {
        match &self.storage {
            PlacedStorage::Ram { correspondence, .. } => correspondence.carrier,
            PlacedStorage::Resident { carrier, .. } | PlacedStorage::External { carrier, .. } => {
                *carrier
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PlacedRamHome {
    Frame {
        depth: usize,
        machine: MachineId,
        place: PlaceId,
        allocation: TerminalStructuralValue,
    },
    PrivateEntry {
        recipe: terminal_installation::CheckedPrivateEntryRamRecipe,
        allocation: TerminalStructuralValue,
    },
}

pub(crate) struct PrivateEntryRamAllocation {
    pub(crate) recipe: terminal_installation::CheckedPrivateEntryRamRecipe,
    pub(crate) allocation: TerminalStructuralValue,
}

impl TerminalExecution<'_> {
    /// A callee may form a new interpretation of its captured ingress backing.
    /// Return checking already matched the physical source against the caller's
    /// public origin bound. Retain the actual live formation, not an association
    /// reconstructed from the result type. A previously captured interpretation
    /// must still match exactly; it cannot be replaced by a different view.
    pub(super) fn retain_returned_placed_interpretation(
        &self,
        projected: &mut super::ResolvedStructuralProjection,
        actual: &super::ReferenceReferent,
        referent: semantic_vocabulary::StructuralTypeId,
        access: StructuralAccess,
    ) -> Result<(), TerminalInterpretError> {
        let Some(captured) = &actual.placed else {
            return Ok(());
        };
        self.validate_placed_backing(captured)?;
        if projected.placed.is_some() {
            return Ok(());
        }
        if captured.selected_field.is_none()
            && captured.presentation.structural_type == referent
            && captured.carrier() == referent
            && captured.source == actual.view
            && projected.value == captured.source
            && projected.view_tip.is_none()
            && actual.aggregate_view.is_none()
            && super::access_covers(captured.access, access)
        {
            projected.placed = Some(captured.clone());
        }
        Ok(())
    }

    pub(crate) fn validate_placed_backing(
        &self,
        captured: &PlacedReference,
    ) -> Result<(), TerminalInterpretError> {
        let home = match &captured.storage {
            PlacedStorage::Ram { home, .. } => home,
            PlacedStorage::Resident { owner, binding, .. }
            | PlacedStorage::External { owner, binding, .. } => {
                return if self.provider_storage.descriptors.get(owner) == Some(binding)
                    && self.is_live_provider_descriptor(owner, binding)
                {
                    Ok(())
                } else {
                    Err(TerminalInterpretError::ContentTransformationMismatch(
                        "resident reference lost its original live owner or backing",
                    ))
                };
            }
        };
        match home {
            PlacedRamHome::Frame {
                depth,
                machine,
                place,
                allocation,
            } => {
                let (actual_machine, values) = if *depth == self.call_stack.len() {
                    (self.current_machine, &self.structural_values)
                } else {
                    let frame = self.call_stack.get(*depth).ok_or_else(malformed)?;
                    (frame.current_machine, &frame.structural_values)
                };
                if actual_machine != *machine || values.get(place) != Some(allocation) {
                    return Err(malformed());
                }
            }
            PlacedRamHome::PrivateEntry { recipe, allocation } => {
                if self.result.is_some()
                    || !self.private_entry_ram.iter().any(|original| {
                        original.recipe == *recipe && original.allocation == *allocation
                    })
                {
                    return Err(malformed());
                }
            }
        }
        Ok(())
    }

    /// Construct complete original typed storage in this invocation's stable
    /// scalar arena before lending any direct entry view into it.
    pub(crate) fn establish_private_entry_ram(
        &mut self,
        recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
        establishments: &[terminal_installation::TerminalPlacedViewEstablishment],
    ) -> Result<(), TerminalInterpretError> {
        for recipe in recipes {
            if recipe.terminal() != self.program.identity.semantic()
                || recipe.input().machine != self.current_machine
            {
                return Err(malformed());
            }
            let establishment = establishments
                .iter()
                .find(|loan| loan.input == *recipe.input())
                .ok_or_else(malformed)?;
            if establishment.referent.structural_type != recipe.schema()
                || !establishment.referent.path.is_empty()
                || !establishment.referent.qualifications.is_empty()
                || self.private_entry_ram.iter().any(|original| {
                    original.allocation.opaque_identity == establishment.referent.opaque_identity
                })
            {
                return Err(malformed());
            }
            let allocation = establishment.referent.clone();
            for field in recipe.fields() {
                let value = match field.value {
                    semantic_vocabulary::ScalarTerm::Boolean(value) => {
                        crate::TerminalScalarValue::Boolean(value)
                    }
                    semantic_vocabulary::ScalarTerm::Integer { scalar_type, value } => {
                        crate::TerminalScalarValue::Integer { scalar_type, value }
                    }
                    _ => return Err(malformed()),
                };
                if self
                    .structural_scalar_fields
                    .insert(
                        StructuralScalarRuntimeField::member(
                            StructuralRuntimePlace::from(&allocation),
                            field.field,
                        ),
                        value,
                    )?
                    .is_some()
                {
                    return Err(malformed());
                }
            }
            let occurrence = self
                .placed_view_occurrences
                .get_mut(recipe.input())
                .ok_or_else(malformed)?;
            let parameter = terminal_semantics::placed_view_input_parameter(
                &self.program.catalogs,
                recipe.input(),
            )
            .map_err(|_| malformed())?;
            let presentation = self
                .structural_values
                .get(&parameter.place)
                .ok_or_else(malformed)?
                .clone();
            occurrence.ram = Some(PlacedReference {
                storage: PlacedStorage::Ram {
                    correspondence: recipe.correspondence().clone(),
                    home: PlacedRamHome::PrivateEntry {
                        recipe: recipe.clone(),
                        allocation: allocation.clone(),
                    },
                    byte_offset: 0,
                },
                presentation,
                access: recipe.input().access,
                selected_field: None,
                projection: Vec::new(),
                source: allocation.clone(),
            });
            self.private_entry_ram.push(PrivateEntryRamAllocation {
                recipe: recipe.clone(),
                allocation,
            });
        }
        Ok(())
    }

    /// Only a completed Unit entry has retired these invocation-scoped loans.
    /// Read actual original scalar cells; a returned view or unfinished frame
    /// cannot be converted into an owner observation.
    pub(crate) fn observe_private_entry_ram(
        &self,
    ) -> Result<Vec<(u32, Vec<u8>)>, TerminalInterpretError> {
        if self.private_entry_ram.is_empty() {
            return Ok(Vec::new());
        }
        if self.result != Some(crate::TerminalExecutionResult::Unit) || !self.call_stack.is_empty()
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "private entry RAM observation requires normal Unit return and retired loans",
            ));
        }
        self.private_entry_ram
            .iter()
            .map(|original| {
                let mut bytes = vec![0; original.recipe.contents().len()];
                for field in original.recipe.fields() {
                    let value = self
                        .structural_scalar_fields
                        .get(&StructuralScalarRuntimeField::member(
                            StructuralRuntimePlace::from(&original.allocation),
                            field.field,
                        ))
                        .ok_or_else(malformed)?
                        .read()?;
                    let bits = match value {
                        crate::TerminalScalarValue::Boolean(value) => u128::from(value),
                        crate::TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(value),
                            ..
                        } => value,
                        crate::TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Signed(value),
                            ..
                        } => value as u128,
                        _ => return Err(malformed()),
                    };
                    let end = field
                        .byte_offset
                        .checked_add(field.byte_width)
                        .ok_or_else(malformed)?;
                    bytes
                        .get_mut(field.byte_offset..end)
                        .ok_or_else(malformed)?
                        .copy_from_slice(&bits.to_le_bytes()[..field.byte_width]);
                }
                Ok((original.recipe.input().position, bytes))
            })
            .collect()
    }

    fn capture_placed_ram_home(
        &self,
        source: TerminalStructuralValue,
        access: StructuralAccess,
        correspondence: terminal_semantics::PlacedRamCorrespondence,
    ) -> Result<PlacedReference, TerminalInterpretError> {
        let original = self.original_owned_allocation(&source)?;
        let depth = original.depth;
        let machine = original.machine;
        let place = original.place;
        let allocation = original.value;
        if terminal_semantics::contains_placed_carrier(
            &self.program.catalogs,
            allocation.structural_type,
        ) {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "placed RAM original allocation contains placed backing",
            ));
        }
        let path = &source.path[allocation.path.len()..];
        let types = self.program.structural_types();
        let (size, alignment) = terminal_semantics::reference_storage_layout(
            types.values(),
            allocation.structural_type,
        )
        .ok_or_else(malformed)?;
        let region = terminal_semantics::reference_storage_region(
            types.values(),
            allocation.structural_type,
            path,
        )
        .ok_or_else(malformed)?;
        let tip = terminal_semantics::runtime_structural_path_tip(
            types.values(),
            allocation.structural_type,
            path,
        )
        .ok_or_else(malformed)?;
        if tip.id != correspondence.schema
            || region.scalar.is_some()
            || region.byte_length != correspondence.source_byte_length
            || region
                .byte_offset
                .checked_add(region.byte_length)
                .is_none_or(|end| end > size)
            || alignment % correspondence.required_base.modulus() != 0
            || !correspondence.required_base.admits(region.byte_offset)
        {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "placed RAM source does not meet its captured allocation geometry",
            ));
        }
        let mut presentation = source.clone();
        presentation.structural_type = correspondence.carrier;
        presentation.qualifications.clear();
        presentation.projected_qualifications.clear();
        let captured = PlacedReference {
            storage: PlacedStorage::Ram {
                correspondence,
                home: PlacedRamHome::Frame {
                    depth,
                    machine,
                    place,
                    allocation,
                },
                byte_offset: region.byte_offset,
            },
            presentation,
            access,
            selected_field: None,
            projection: Vec::new(),
            source: source.clone(),
        };
        // Contents belong to that captured schema, not the accessor's opaque
        // presentation. An incomplete source cannot mint initialized custody.
        let StructuralTypeShape::Record { fields, .. } = &self
            .program
            .structural_types()
            .get(&source.structural_type)
            .ok_or_else(malformed)?
            .shape
        else {
            return Err(malformed());
        };
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            let Some(scalar) = field.field_type.scalar_type() else {
                let root = field.field_type.as_structural().ok_or_else(malformed)?;
                let (scalar, count) = terminal_semantics::scalar_array_leaf_shape(
                    self.program.structural_types().values(),
                    root,
                )
                .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "placed RAM initialization requires a supported scalar storage tree",
                ))?;
                let PlacedStorage::Ram {
                    home:
                        PlacedRamHome::Frame {
                            depth,
                            machine,
                            place,
                            allocation,
                        },
                    ..
                } = &captured.storage
                else {
                    return Err(malformed());
                };
                let home = crate::primitive_storage::PrimitiveStorageHome::captured_frame(
                    *depth,
                    *machine,
                    *place,
                    allocation.structural_type,
                );
                let mut path = source.path[allocation.path.len()..].to_vec();
                path.push(terminal_psi::StructuralPathSegment::Field(field.identity));
                let prefix_length = path.len();
                for ordinal in 0..count {
                    path.truncate(prefix_length);
                    path.extend(
                        terminal_semantics::scalar_array_leaf_path(
                            self.program.structural_types().values(),
                            root,
                            ordinal,
                        )
                        .ok_or_else(malformed)?,
                    );
                    // Observe the original live home, including a suspended
                    // caller's cells. Shape alone never establishes contents.
                    let value = self.read_primitive_home(home, &path, scalar)?;
                    if !crate::scalar_operations::terminal_scalar_belongs_to_type(value) {
                        return Err(malformed());
                    }
                }
                continue;
            };
            let value = self
                .structural_scalar_fields
                .get(&StructuralScalarRuntimeField::member(
                    StructuralRuntimePlace::from(&source),
                    field.id,
                ))
                .ok_or_else(malformed)?
                .read()?;
            if value.scalar_type() != scalar || !value.satisfies_field(&field.field_type) {
                return Err(malformed());
            }
        }
        self.validate_placed_backing(&captured)?;
        Ok(captured)
    }

    fn capture_resident_reference(
        &self,
        source: &TerminalStructuralValue,
        access: StructuralAccess,
        result_type: semantic_vocabulary::StructuralTypeId,
        roles: terminal_psi::ResidentReferenceSource,
    ) -> Result<PlacedReference, TerminalInterpretError> {
        let plan = terminal_semantics::resident_reference_plan(
            &self.program.catalogs,
            source.structural_type,
            result_type,
            access,
            roles,
        )
        .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
        let owner = StructuralRuntimePlace::from(source);
        let binding = self
            .provider_storage
            .descriptors
            .get(&owner)
            .copied()
            .ok_or(TerminalInterpretError::ContentTransformationMismatch(
                "resident reference has no actual provider backing",
            ))?;
        if binding.resident == arena::Handle::default()
            || !self.is_live_provider_descriptor(&owner, &binding)
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "resident reference has no live original resident claim",
            ));
        }
        let (initialized, custody, compatibility) = self.live_resident_custody(binding.resident)?;
        if source.structural_type != initialized.storage.carrier
            || !source.qualifications.contains(&roles.granted)
            || !source.qualifications.contains(&roles.resident)
            || initialized
                .storage
                .vacant
                .is_some_and(|vacant| source.qualifications.contains(&vacant))
            || initialized.storage.granted != roles.granted
            || initialized.storage.resident != Some(roles.resident)
            || initialized.storage.base != roles.base
            || initialized.storage.length != roles.length
            || initialized.schema != plan.schema
            || custody.value.structural_type != plan.schema
            || initialized.policy_identity != plan.policy_identity
            || initialized.policy_plan_machine_identity != plan.policy_plan_machine_identity
            || initialized.schema_identity != plan.schema_identity
            || initialized.placement != plan.placement
            || plan.placement.layout().size != Some(compatibility.byte_length())
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "resident reference changed its original owner, policy or schema",
            ));
        }
        let (_, _, base) = self.provider_storage.live_provider_allocation(&binding)?;
        if !self.provider_storage_geometry_matches(
            &owner,
            &initialized.storage,
            base,
            binding.length,
        )? {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "resident reference changed its original allocation geometry",
            ));
        }
        let mut presentation = source.clone();
        presentation.structural_type = plan.carrier;
        presentation.qualifications.clear();
        presentation.projected_qualifications.clear();
        // Formation may attenuate a mutable source to a shared reference.
        // Keep that result ceiling in the captured backing as well as in the
        // ordinary reference descriptor; a later projection cannot restore it.
        let (_, result_access) = self.reference_contract(result_type)?;
        Ok(PlacedReference {
            storage: PlacedStorage::Resident {
                carrier: plan.carrier,
                owner,
                binding,
            },
            presentation,
            access: result_access,
            selected_field: None,
            projection: Vec::new(),
            source: source.clone(),
        })
    }

    pub(crate) fn establish_placed_reference(
        &mut self,
        operation: &Operation,
        source: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        let result = operation.result.structural().ok_or_else(malformed)?;
        let backing = self
            .resolve_reference_call_arguments(std::slice::from_ref(source))?
            .pop()
            .ok_or_else(malformed)?;
        if result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || self.structural_values.contains_key(&result.place)
            || self.live_affine_discard_at(result.place).is_some()
        {
            return Err(malformed());
        }
        let (_, access) = self.reference_contract(result.structural_type)?;
        let terminal_psi::OperationKind::EstablishPlacedReference { resident, .. } = operation.kind
        else {
            return Err(malformed());
        };
        let captured = if let Some(resident) = resident {
            self.capture_resident_reference(
                &backing,
                source.access,
                result.structural_type,
                resident,
            )?
        } else {
            let correspondence = terminal_semantics::placed_reference_correspondence(
                &self.program.catalogs,
                backing.structural_type,
                result.structural_type,
                source.access,
            )
            .map_err(TerminalInterpretError::PlacedReference)?;
            self.capture_placed_ram_home(backing.clone(), access, correspondence)?
        };
        let carrier = TerminalStructuralValue::minted(
            self.local_structural_identities.allocate()?,
            result.structural_type,
            Vec::new(),
        );
        self.reference_referents.insert(
            StructuralRuntimePlace::from(&carrier),
            super::ReferenceReferent {
                view: backing,
                primitive_loan: None,
                collection_loan: None,
                byte_region: None,
                byte_owner: None,
                aggregate_view: None,
                placed: Some(captured),
            },
        )?;
        self.structural_values.insert(result.place, carrier)?;
        self.live_affine_frontier.insert(StructuralAffineDiscard {
            place: result.place,
            path: Vec::new(),
            structural_type: result.structural_type,
        });
        Ok(())
    }
    pub(crate) fn project_placed_presentation(
        &self,
        captured: &mut PlacedReference,
        prefix: &[terminal_psi::StructuralPathSegment],
        presentation: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        self.validate_placed_backing(captured)?;
        if !prefix.is_empty() {
            let plan = self
                .program
                .catalogs
                .placed_view_plans
                .iter()
                .find(|plan| plan.carrier == captured.carrier())
                .ok_or_else(malformed)?;
            let (field, relative) = if let Some(slot) = captured.selected_field {
                (
                    plan.fields
                        .iter()
                        .find(|field| field.access_slot == slot)
                        .ok_or_else(malformed)?,
                    prefix,
                )
            } else {
                let Some((terminal_psi::StructuralPathSegment::Field(identity), relative)) =
                    prefix.split_first()
                else {
                    return Err(malformed());
                };
                (
                    plan.fields
                        .iter()
                        .find(|field| field.view_field == *identity)
                        .ok_or_else(malformed)?,
                    relative,
                )
            };
            let mut projection = captured.projection.clone();
            projection.extend_from_slice(relative);
            let selected = terminal_semantics::runtime_structural_path_tip(
                self.program.structural_types().values(),
                field.accessor,
                &projection,
            )
            .ok_or_else(malformed)?;
            if presentation.structural_type != selected.id {
                return Err(malformed());
            }
            captured.selected_field = Some(field.access_slot);
            captured.projection = projection;
        }
        captured.presentation = presentation.clone();
        Ok(())
    }
}

impl TerminalExecution<'_> {
    /// The sealed accessor consumes a live placed interpretation. This is
    /// an ordered boundary operation over the retained encoding, never an
    /// ordinary structural field read/store on the opaque accessor carrier.
    pub(crate) fn execute_placed_boundary(
        &mut self,
        operation: &Operation,
        declaration: &terminal_psi::BoundaryMachineDeclaration,
        arguments: &[crate::values::TerminalScalarValue],
        structural_arguments: &[StructuralArgument],
    ) -> Result<bool, TerminalInterpretError> {
        let program = std::sync::Arc::clone(&self.program);
        let mut operations = program.catalogs.placed_view_plans.iter().flat_map(|plan| {
            plan.fields.iter().flat_map(move |field| {
                field
                    .operations
                    .iter()
                    .filter(move |accessor| accessor.boundary == declaration.id)
                    .map(move |accessor| (plan, field, accessor.kind))
            })
        });
        let Some((plan, field, kind)) = operations.next() else {
            return Ok(false);
        };
        if operations.next().is_some() {
            return Err(malformed());
        }
        let [argument] = structural_arguments else {
            return Err(malformed());
        };
        let (presentation, Some(ram)) = self.resolve_live_placed_projection(argument)? else {
            // A host-established occurrence still uses its admitted adapter.
            return Ok(false);
        };
        self.validate_placed_backing(&ram)?;
        let terminal_psi::OperationKind::BoundaryCall {
            completion_receipts,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        if !completion_receipts.is_empty() {
            return Err(malformed());
        }
        let required = match kind {
            terminal_psi::TerminalPlacedAccessorKind::Read => StructuralAccess::SharedBorrow,
            terminal_psi::TerminalPlacedAccessorKind::Write => StructuralAccess::MutableBorrow,
            terminal_psi::TerminalPlacedAccessorKind::Take => {
                return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                    "ordinary RAM does not establish destructive External supply",
                ));
            }
        };
        if argument.access != required
            || !super::access_covers(ram.access, required)
            || ram.carrier() != plan.carrier
            || ram.selected_field != Some(field.access_slot)
            || declaration
                .structural_parameters
                .first()
                .is_none_or(|parameter| presentation.structural_type != parameter.structural_type)
        {
            return Err(malformed());
        }
        crate::custody::bind_structural_arguments(
            &declaration.structural_parameters,
            std::slice::from_ref(&presentation),
            self.account.as_ref(),
        )?;
        crate::custody::validate_boundary_requirements(declaration, |index| {
            (index == 0).then_some(&presentation)
        })?;
        self.preflight_boundary_result(declaration, &operation.result)?;
        if kind == terminal_psi::TerminalPlacedAccessorKind::Write
            && (declaration.result != terminal_psi::BoundaryMachineResult::Unit
                || operation.result != terminal_psi::OperationResult::Unit)
        {
            return Err(malformed());
        }
        let returned = match &ram.storage {
            PlacedStorage::Resident { binding, .. } => self.execute_provider_field(
                *binding,
                crate::provider_storage::field_access::ProviderFieldContent::Resident,
                plan,
                field,
                &ram.projection,
                kind,
                ram.access,
                arguments,
            )?,
            PlacedStorage::External {
                binding,
                compatibility,
                ..
            } => self.execute_provider_field(
                *binding,
                crate::provider_storage::field_access::ProviderFieldContent::Borrowed(
                    compatibility,
                ),
                plan,
                field,
                &ram.projection,
                kind,
                ram.access,
                arguments,
            )?,
            PlacedStorage::Ram {
                correspondence,
                byte_offset,
                home,
            } => {
                let source = correspondence
                    .fields
                    .iter()
                    .find(|source| source.access_slot == field.access_slot)
                    .ok_or_else(malformed)?;
                let selected_projection = if ram.projection.is_empty() {
                    terminal_semantics::ReferenceStorageProjection {
                        byte_offset: source.byte_offset,
                        logical_scalar: source.logical_scalar,
                        stored_scalar: source.stored_scalar,
                    }
                } else {
                    terminal_semantics::placed_region_projection(
                        &program.catalogs.structural_types,
                        plan,
                        field,
                        &ram.projection,
                    )
                    .map_err(TerminalInterpretError::PlacedReference)?
                };
                let width = u64::from(source.transfer_width_bits)
                    .checked_div(8)
                    .ok_or_else(malformed)?;
                if width == 0
                    || source.transfer_width_bits % 8 != 0
                    || selected_projection
                        .byte_offset
                        .checked_add(width)
                        .is_none_or(|end| end > correspondence.source_byte_length)
                    || byte_offset
                        .checked_add(selected_projection.byte_offset)
                        .is_none()
                {
                    return Err(malformed());
                }
                let StructuralTypeShape::Record { fields, .. } = &program
                    .structural_types()
                    .get(&ram.source.structural_type)
                    .ok_or_else(malformed)?
                    .shape
                else {
                    return Err(malformed());
                };
                let source_field = fields
                    .iter()
                    .find(|field| field.id == source.source_field)
                    .ok_or_else(malformed)?;
                let storage = if !ram.projection.is_empty() {
                    let PlacedRamHome::Frame {
                        depth,
                        machine,
                        place,
                        allocation,
                    } = home
                    else {
                        return Err(malformed());
                    };
                    let mut path = ram.source.path[allocation.path.len()..].to_vec();
                    path.push(terminal_psi::StructuralPathSegment::Field(
                        source_field.identity,
                    ));
                    path.extend_from_slice(&ram.projection);
                    let (storage, scalar, _) = self.primitive_home_access(
                        crate::primitive_storage::PrimitiveStorageHome::captured_frame(
                            *depth,
                            *machine,
                            *place,
                            allocation.structural_type,
                        ),
                        kind == terminal_psi::TerminalPlacedAccessorKind::Write,
                        &path,
                    )?;
                    if scalar != selected_projection.logical_scalar {
                        return Err(malformed());
                    }
                    storage
                } else {
                    // Scalar record fields retain their admitted storage
                    // codec. An ordinary primitive reference is deliberately
                    // unavailable for several such field representations.
                    let cell = StructuralScalarRuntimeField::member(
                        StructuralRuntimePlace::from(&ram.source),
                        source.source_field,
                    );
                    crate::primitive_storage::PrimitiveStorage {
                        endpoint: crate::primitive_storage::PrimitiveStorageEndpoint::ScalarField(
                            cell,
                        ),
                        encoding: Some(crate::primitive_storage::PrimitiveStorageEncoding {
                            source: source.stored_scalar,
                            view: source.stored_scalar,
                        }),
                    }
                };
                let value_type = if ram.projection.is_empty() {
                    source_field.field_type.clone()
                } else {
                    terminal_psi::StructuralFieldType::Scalar(selected_projection.logical_scalar)
                };
                let write = match (kind, arguments) {
                    (terminal_psi::TerminalPlacedAccessorKind::Read, []) => None,
                    (terminal_psi::TerminalPlacedAccessorKind::Write, [value])
                        if value.scalar_type() == source.logical_scalar
                            && value.satisfies_field(&value_type) =>
                    {
                        Some(*value)
                    }
                    _ => return Err(malformed()),
                };
                if write.is_some()
                    && (!ram.source.qualifications.is_empty()
                        || !ram.source.projected_qualifications.is_empty())
                {
                    return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                        "qualified RAM writes require source frame correspondence",
                    ));
                }
                if let Some(value) = write {
                    self.store_primitive_storage(
                        argument.place,
                        storage,
                        source.logical_scalar,
                        value,
                    )?;
                    crate::effects::results::TerminalEffectResult::Unit
                } else {
                    let value = self.read_primitive_storage(
                        argument.place,
                        storage,
                        source.logical_scalar,
                        source.logical_scalar,
                        false,
                    )?;
                    if !value.satisfies_field(&value_type) {
                        return Err(malformed());
                    }
                    crate::effects::results::TerminalEffectResult::Scalar(value)
                }
            }
        };
        self.commit_boundary_result(&operation.result, &declaration.result, returned)?;
        self.effects
            .push(crate::effects::TerminalEffect::BoundaryCall {
                operation: operation.id,
                boundary: declaration.id,
                arguments: arguments.to_vec(),
                structural_arguments: vec![presentation],
                byte_sequence_arguments: vec![None],
                completion_receipts: Vec::new(),
                result: declaration.result.clone(),
            });
        Ok(true)
    }
}
