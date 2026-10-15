//! Placed interpretation of initialized storage in the interpreter's arena.
//!
//! The allocation's declared layout models an aligned arena allocation, never
//! a host address. Captured subregions retain that allocation and its live frame
//! home; an opaque identity is used only to join storage, never as an address.
//! Native execution must separately discharge the concrete-address condition.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlacedRamReference {
    pub(crate) correspondence: terminal_semantics::PlacedRamCorrespondence,
    pub(crate) presentation: TerminalStructuralValue,
    pub(crate) access: StructuralAccess,
    pub(crate) selected_field: Option<u32>,
    /// The original initialized schema, including its source qualifications.
    pub(crate) source: TerminalStructuralValue,
    /// A home in a pinned invocation frame, never a reconstructed source recipe.
    home: PlacedRamHome,
    pub(crate) byte_offset: u64,
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
    pub(crate) fn validate_placed_ram_home(
        &self,
        captured: &PlacedRamReference,
    ) -> Result<(), TerminalInterpretError> {
        match &captured.home {
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
            if recipe.terminal() != self.program.identity
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
                    )
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
            occurrence.ram = Some(PlacedRamReference {
                correspondence: recipe.correspondence().clone(),
                presentation,
                access: recipe.input().access,
                selected_field: None,
                source: allocation.clone(),
                home: PlacedRamHome::PrivateEntry {
                    recipe: recipe.clone(),
                    allocation: allocation.clone(),
                },
                byte_offset: 0,
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
                        .ok_or_else(malformed)?;
                    let bits = match value {
                        crate::TerminalScalarValue::Boolean(value) => u128::from(*value),
                        crate::TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Unsigned(value),
                            ..
                        } => *value,
                        crate::TerminalScalarValue::Integer {
                            value: semantic_vocabulary::IntegerValue::Signed(value),
                            ..
                        } => *value as u128,
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
    ) -> Result<PlacedRamReference, TerminalInterpretError> {
        // Prefer the actual enclosing allocation. A projected logical referee
        // cannot supply its parent's alignment or prove the available extent.
        let homes = self
            .call_stack
            .iter()
            .enumerate()
            .map(|(depth, frame)| (depth, frame.current_machine, &frame.structural_values))
            .chain(std::iter::once((
                self.call_stack.len(),
                self.current_machine,
                &self.structural_values,
            )));
        let mut candidates = Vec::new();
        for (depth, machine_id, values) in homes {
            let machine = self
                .program
                .machines()
                .get(&machine_id)
                .ok_or_else(malformed)?;
            for (place, allocation) in values {
                if allocation.opaque_identity != source.opaque_identity
                    || !source.path.starts_with(&allocation.path)
                {
                    continue;
                }
                let owned = machine
                    .structural_parameter(*place)
                    .is_some_and(|parameter| {
                        parameter.access == StructuralAccess::Owned
                            && parameter.structural_type == allocation.structural_type
                    })
                    || machine
                        .producing_operation(*place)
                        .is_some_and(|(operation, ty)| {
                            operation.result.structural().is_some_and(|result| {
                                result.structural_type == ty && ty == allocation.structural_type
                            })
                        });
                if !owned
                    || terminal_semantics::contains_placed_carrier(
                        &self.program.catalogs,
                        allocation.structural_type,
                    )
                {
                    continue;
                }
                candidates.push((depth, machine_id, *place, allocation.clone()));
            }
        }
        // Choose the original enclosing owner before considering geometry.
        // A child presentation must not hide its unaligned parent by offering
        // a stronger logical alignment. Competing original owners are invalid.
        candidates.sort_by_key(|(_, _, _, allocation)| allocation.path.len());
        let mut candidates = candidates.into_iter();
        let (depth, machine, place, allocation) =
            candidates
                .next()
                .ok_or(TerminalInterpretError::UnsupportedSemanticVariant(
                    "placed RAM source has no live supplying allocation",
                ))?;
        if candidates
            .next()
            .is_some_and(|(_, _, _, other)| other.path.len() == allocation.path.len())
        {
            return Err(malformed());
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
        let captured = PlacedRamReference {
            correspondence,
            presentation,
            access,
            selected_field: None,
            source: source.clone(),
            home: PlacedRamHome::Frame {
                depth,
                machine,
                place,
                allocation,
            },
            byte_offset: region.byte_offset,
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
            let scalar = field.field_type.scalar_type().ok_or(
                TerminalInterpretError::UnsupportedSemanticVariant(
                    "placed RAM source requires initialized scalar field coverage",
                ),
            )?;
            let value = self
                .structural_scalar_fields
                .get(&StructuralScalarRuntimeField::member(
                    StructuralRuntimePlace::from(&source),
                    field.id,
                ))
                .ok_or_else(malformed)?;
            if value.scalar_type() != scalar || !value.satisfies_field(&field.field_type) {
                return Err(malformed());
            }
        }
        self.validate_placed_ram_home(&captured)?;
        Ok(captured)
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
        let correspondence = terminal_semantics::placed_reference_correspondence(
            &self.program.catalogs,
            backing.structural_type,
            result.structural_type,
            source.access,
        )
        .map_err(|_| malformed())?;
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
        let captured = self.capture_placed_ram_home(backing.clone(), access, correspondence)?;
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
                placed_ram: Some(captured),
            },
        );
        self.structural_values.insert(result.place, carrier);
        self.live_affine_frontier.insert(StructuralAffineDiscard {
            place: result.place,
            path: Vec::new(),
            structural_type: result.structural_type,
        });
        Ok(())
    }
    pub(crate) fn project_placed_ram_presentation(
        &self,
        captured: &mut PlacedRamReference,
        prefix: &[terminal_psi::StructuralPathSegment],
        presentation: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        self.validate_placed_ram_home(captured)?;
        if !prefix.is_empty() {
            let [terminal_psi::StructuralPathSegment::Field(identity)] = prefix else {
                return Err(malformed());
            };
            if captured.selected_field.is_some() {
                return Err(malformed());
            }
            let plan = self
                .program
                .catalogs
                .placed_view_plans
                .iter()
                .find(|plan| plan.carrier == captured.correspondence.carrier)
                .ok_or_else(malformed)?;
            let field = plan
                .fields
                .iter()
                .find(|field| field.view_field == *identity)
                .ok_or_else(malformed)?;
            if presentation.structural_type != field.accessor
                || !captured
                    .correspondence
                    .fields
                    .iter()
                    .any(|source| source.access_slot == field.access_slot)
            {
                return Err(malformed());
            }
            captured.selected_field = Some(field.access_slot);
        }
        captured.presentation = presentation.clone();
        Ok(())
    }
}

impl TerminalExecution<'_> {
    /// The sealed accessor consumes a live local RAM interpretation. This is
    /// an ordered boundary operation over the retained encoding, never an
    /// ordinary structural field read/store on the opaque accessor carrier.
    pub(crate) fn execute_placed_ram_boundary(
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
        self.validate_placed_ram_home(&ram)?;
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
            || ram.correspondence.carrier != plan.carrier
            || ram.selected_field != Some(field.access_slot)
            || presentation.structural_type != field.accessor
        {
            return Err(malformed());
        }
        let source = ram
            .correspondence
            .fields
            .iter()
            .find(|source| source.access_slot == field.access_slot)
            .ok_or_else(malformed)?;
        let width = u64::from(source.transfer_width_bits)
            .checked_div(8)
            .ok_or_else(malformed)?;
        if width == 0
            || source.transfer_width_bits % 8 != 0
            || source
                .byte_offset
                .checked_add(width)
                .is_none_or(|end| end > ram.correspondence.source_byte_length)
            || ram.byte_offset.checked_add(source.byte_offset).is_none()
        {
            return Err(malformed());
        }
        crate::custody::bind_structural_arguments(
            &declaration.structural_parameters,
            std::slice::from_ref(&presentation),
        )?;
        crate::custody::validate_boundary_requirements(declaration, |index| {
            (index == 0).then_some(&presentation)
        })?;
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
        let cell = StructuralScalarRuntimeField::member(
            StructuralRuntimePlace::from(&ram.source),
            source.source_field,
        );
        let storage = crate::primitive_storage::PrimitiveStorage {
            endpoint: crate::primitive_storage::PrimitiveStorageEndpoint::ScalarField(cell),
            encoding: Some(crate::primitive_storage::PrimitiveStorageEncoding {
                source: source.stored_scalar,
                view: source.stored_scalar,
            }),
        };
        let write = match (kind, arguments) {
            (terminal_psi::TerminalPlacedAccessorKind::Read, []) => None,
            (terminal_psi::TerminalPlacedAccessorKind::Write, [value])
                if value.scalar_type() == source.logical_scalar
                    && value.satisfies_field(&source_field.field_type) =>
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
        self.preflight_boundary_result(declaration, &operation.result)?;
        let returned = if let Some(value) = write {
            if declaration.result != terminal_psi::BoundaryMachineResult::Unit
                || operation.result != terminal_psi::OperationResult::Unit
            {
                return Err(malformed());
            }
            self.store_primitive_storage(argument.place, storage, source.logical_scalar, value)?;
            crate::effects::results::TerminalEffectResult::Unit
        } else {
            let value = self.read_primitive_storage(
                argument.place,
                storage,
                source.logical_scalar,
                source.logical_scalar,
                false,
            )?;
            if !value.satisfies_field(&source_field.field_type) {
                return Err(malformed());
            }
            crate::effects::results::TerminalEffectResult::Scalar(value)
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
