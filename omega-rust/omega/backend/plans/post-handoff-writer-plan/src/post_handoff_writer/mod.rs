//! Post-handoff writer plans: the reusable fragment ABI, generated fragment
//! plans, invocation plans, and the step/plan carriers a provider replays
//! after control leaves the loader.

use layout_plans::{
    ByteOrder, MaterializationAction, MaterializationDiagnostic, MaterializationWrite,
    PlacementConstraints, PlacementSite, RelocationTarget, StoredIntegerFit,
    SymbolicMaterializationPlan, validate_fragment, validate_stored_integer_fit,
    validate_stored_integer_fit_shape, validate_write, validate_write_source_value,
};

#[cfg(test)]
mod tests;

/// Derive the consumer writer program from a portable symbolic materialization
/// plan. Resolved writes retain their compiler-issued values; runtime writes
/// become resolver-bound sources; loader-native relocations are not writer
/// instructions and reject here rather than reaching a provider.
pub fn derive_post_handoff_writer(
    plan: &SymbolicMaterializationPlan,
) -> Result<PostHandoffWriterPlan, MaterializationDiagnostic> {
    if plan.actions.is_empty() {
        return Err(MaterializationDiagnostic(
            "post-handoff writer requires at least one fragment".into(),
        ));
    }
    let mut steps = Vec::with_capacity(plan.actions.len());
    for action in &plan.actions {
        let step = match action {
            MaterializationAction::ResolvedWrite {
                write,
                source_value,
            } => PostHandoffWriterStep {
                write: write.clone(),
                source: PostHandoffWriterSource::Resolved(*source_value),
            },
            MaterializationAction::RuntimeWriter(write) => PostHandoffWriterStep {
                write: write.clone(),
                source: PostHandoffWriterSource::Resolve(write.target),
            },
            MaterializationAction::NativePointerRelocation { .. } => {
                return Err(MaterializationDiagnostic(
                    "loader-native relocation cannot enter a post-handoff writer program".into(),
                ));
            }
        };
        steps.push(step);
    }
    Ok(PostHandoffWriterPlan {
        byte_len: plan.byte_len,
        byte_order: plan.byte_order,
        placement: plan.placement,
        steps,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostHandoffWriterSource {
    Resolved(u64),
    Resolve(RelocationTarget),
}

/// `PHWRITR1`: target-neutral packed input ABI for a reusable post-handoff
/// fragment. Word zero is the destination address. The remaining words are
/// dense source slots assigned by the fragment plan. Numeric words stay inside
/// provider-owned invocation evidence; source code sees neither this context
/// nor an address-valued writer operation.
pub const POST_HANDOFF_WRITER_CONTEXT_ABI_V1: u64 = 0x5048_5752_4954_5231;
pub const POST_HANDOFF_WRITER_DESTINATION_OFFSET: usize = 0;
pub const POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET: usize = 8;
pub const POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH: usize = 8;

pub fn post_handoff_writer_context_byte_len(source_slot_count: usize) -> Option<usize> {
    source_slot_count
        .checked_mul(POST_HANDOFF_WRITER_SOURCE_SLOT_WIDTH)?
        .checked_add(POST_HANDOFF_WRITER_SOURCE_SLOTS_OFFSET)
}

/// One address-free fragment in a reusable generated writer. `source_slot`
/// indexes the provider-private invocation context. Field names, symbolic
/// identities, resolved values, and concrete placement are deliberately
/// absent: none changes the emitted transfer geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeneratedPostHandoffWriterStep {
    pub container_byte_offset: u64,
    pub container_width_bits: u16,
    pub destination_lsb: u16,
    pub source_lsb: u16,
    pub width: u16,
    pub source_slot: usize,
}

/// Static normalized plan for one reusable post-handoff fragment.
///
/// Its report fingerprint covers only facts that can change emitted code. Exact
/// relocation targets, resolved content, placement, resolver authority, and
/// roots belong to `PostHandoffWriterInvocationPlan` instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPostHandoffWriterFragmentPlan {
    pub(crate) context_abi: u64,
    byte_len: usize,
    byte_order: ByteOrder,
    source_slot_count: usize,
    pub(crate) steps: Vec<GeneratedPostHandoffWriterStep>,
    pub(crate) report_fingerprint: u64,
}

impl GeneratedPostHandoffWriterFragmentPlan {
    pub const fn context_abi(&self) -> u64 {
        self.context_abi
    }

    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    pub const fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    pub const fn source_slot_count(&self) -> usize {
        self.source_slot_count
    }

    pub fn steps(&self) -> &[GeneratedPostHandoffWriterStep] {
        &self.steps
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.report_fingerprint
    }
}

/// Exact value bound to one private source slot for one invocation. The target
/// remains present even for a pre-resolved value so fragmented writes group by
/// compiler-issued source identity rather than accidentally by equal numeric
/// content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PostHandoffWriterSourceSlot {
    pub target: RelocationTarget,
    pub source: PostHandoffWriterSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterFitConstraint {
    pub source_slot: usize,
    pub field: String,
    pub fit: StoredIntegerFit,
}

/// Invocation-sensitive half of generated writer lowering. This evidence is
/// intentionally separate from the reusable fragment identity. Only validated
/// writer lowering constructs it; consumers may inspect but cannot substitute
/// targets, placement, or fit evidence.
///
/// ```compile_fail
/// use post_handoff_writer_plan::PostHandoffWriterInvocationPlan;
///
/// fn discard_fit_evidence(invocation: &mut PostHandoffWriterInvocationPlan) {
///     invocation.fit_constraints.clear();
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterInvocationPlan {
    pub(crate) fragment: GeneratedPostHandoffWriterFragmentPlan,
    pub(crate) placement: PlacementConstraints,
    pub(crate) sources: Vec<PostHandoffWriterSourceSlot>,
    pub(crate) fit_constraints: Vec<PostHandoffWriterFitConstraint>,
}

impl PostHandoffWriterInvocationPlan {
    pub const fn fragment(&self) -> &GeneratedPostHandoffWriterFragmentPlan {
        &self.fragment
    }

    pub const fn placement(&self) -> PlacementConstraints {
        self.placement
    }

    pub const fn source_slot_count(&self) -> usize {
        self.sources.len()
    }

    pub fn sources(&self) -> &[PostHandoffWriterSourceSlot] {
        &self.sources
    }

    pub fn fit_constraints(&self) -> &[PostHandoffWriterFitConstraint] {
        &self.fit_constraints
    }

    /// Independently replay the sealed invocation before a consumer accepts
    /// provider-supplied words. Rejection only borrows this carrier, so the
    /// exact invocation remains available for inspection or corrected retry.
    pub fn validate_structure(&self) -> Result<(), MaterializationDiagnostic> {
        let fragment = &self.fragment;
        if fragment.context_abi != POST_HANDOFF_WRITER_CONTEXT_ABI_V1 {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation uses an unsupported context ABI".into(),
            ));
        }
        if fragment.byte_len == 0 || fragment.steps.is_empty() || self.sources.is_empty() {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation requires nonempty bytes, sources, and fragments"
                    .into(),
            ));
        }
        if self.placement.alignment() == 0 {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation placement alignment must be nonzero".into(),
            ));
        }
        if fragment.source_slot_count != self.sources.len()
            || post_handoff_writer_context_byte_len(self.sources.len()).is_none()
        {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation source-slot geometry is inconsistent".into(),
            ));
        }

        let mut distinct_targets = std::collections::BTreeSet::new();
        for slot in &self.sources {
            if !distinct_targets.insert(slot.target) {
                return Err(MaterializationDiagnostic(
                    "post-handoff writer invocation repeats one relocation target in multiple source slots"
                        .into(),
                ));
            }
            if let PostHandoffWriterSource::Resolve(target) = slot.source
                && target != slot.target
            {
                return Err(MaterializationDiagnostic(
                    "post-handoff writer invocation resolver source does not match its source-slot target"
                        .into(),
                ));
            }
        }

        let mut used_slots = vec![false; self.sources.len()];
        let mut next_first_slot = 0;
        for step in &fragment.steps {
            validate_fragment(
                fragment.byte_len,
                "generated post-handoff writer fragment",
                step.container_byte_offset,
                step.container_width_bits,
                step.destination_lsb,
                step.source_lsb,
                step.width,
            )?;
            let used = used_slots.get_mut(step.source_slot).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "post-handoff writer fragment names missing source slot {}",
                    step.source_slot
                ))
            })?;
            if !*used {
                if step.source_slot != next_first_slot {
                    return Err(MaterializationDiagnostic(
                        "post-handoff writer fragment source slots are not in canonical first-occurrence order"
                            .into(),
                    ));
                }
                *used = true;
                next_first_slot += 1;
            }
        }
        if used_slots.iter().any(|used| !used) {
            return Err(MaterializationDiagnostic(
                "post-handoff writer invocation retains an unused source slot".into(),
            ));
        }

        for constraint in &self.fit_constraints {
            validate_stored_integer_fit_shape(
                &constraint.field,
                constraint.fit,
                "post-handoff invocation",
            )?;
            if constraint.source_slot >= self.sources.len()
                || !fragment.steps.iter().any(|step| {
                    step.source_slot == constraint.source_slot
                        && step.container_width_bits == constraint.fit.stored_width_bits
                        && step.width == constraint.fit.stored_width_bits
                        && step.destination_lsb == 0
                        && step.source_lsb == 0
                })
            {
                return Err(MaterializationDiagnostic(format!(
                    "post-handoff stored-integer constraint for `{}` does not bind one exact generated fragment",
                    constraint.field
                )));
            }
        }

        let expected_report_fingerprint = generated_post_handoff_writer_report_fingerprint(
            fragment.byte_len,
            fragment.byte_order,
            fragment.source_slot_count,
            &fragment.steps,
        );
        if fragment.report_fingerprint != expected_report_fingerprint {
            return Err(MaterializationDiagnostic(
                "post-handoff writer fragment fingerprint does not match its exact geometry".into(),
            ));
        }
        Ok(())
    }

    pub fn validate_source_values(
        &self,
        source_values: &[u64],
    ) -> Result<(), MaterializationDiagnostic> {
        self.validate_structure()?;
        if source_values.len() != self.sources.len() {
            return Err(MaterializationDiagnostic(format!(
                "post-handoff writer has {} source values for {} source slots",
                source_values.len(),
                self.sources.len()
            )));
        }
        for (slot_index, (slot, supplied)) in self.sources.iter().zip(source_values).enumerate() {
            if let PostHandoffWriterSource::Resolved(expected) = slot.source
                && *supplied != expected
            {
                return Err(MaterializationDiagnostic(format!(
                    "post-handoff writer source slot {slot_index} for {:?} supplied {supplied:#x}, but its invocation evidence retains {expected:#x}",
                    slot.target
                )));
            }
        }
        for constraint in &self.fit_constraints {
            let value = source_values.get(constraint.source_slot).ok_or_else(|| {
                MaterializationDiagnostic(format!(
                    "post-handoff stored-integer constraint for `{}` names missing source slot {}",
                    constraint.field, constraint.source_slot
                ))
            })?;
            validate_stored_integer_fit(
                &constraint.field,
                constraint.fit,
                *value,
                "resolved symbolic",
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterStep {
    pub write: MaterializationWrite,
    pub source: PostHandoffWriterSource,
}

/// Provider-consumable writer program derived from symbolic materialization
/// actions. It contains no source-callable address operation: only the
/// provider resolver may turn a sealed relocation target into address bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostHandoffWriterPlan {
    pub byte_len: usize,
    pub byte_order: ByteOrder,
    pub placement: PlacementConstraints,
    pub steps: Vec<PostHandoffWriterStep>,
}

impl PostHandoffWriterPlan {
    /// Derive one reusable address-free fragment plus exact invocation evidence.
    /// Source slots follow first target occurrence and are dense. Repeated
    /// fragments of one symbolic target therefore consume one once-resolved
    /// word even when the concrete address or artifact realization changes.
    pub fn lower_reusable_fragment(
        &self,
    ) -> Result<PostHandoffWriterInvocationPlan, MaterializationDiagnostic> {
        validate_post_handoff_writer_nonempty(&self.steps)?;

        let mut sources = Vec::<PostHandoffWriterSourceSlot>::new();
        let mut fit_constraints = Vec::new();
        let mut steps = Vec::with_capacity(self.steps.len());
        for step in &self.steps {
            validate_post_handoff_writer_step(self.byte_len, step)?;

            let source_slot = if let Some(index) = sources
                .iter()
                .position(|source| source.target == step.write.target)
            {
                if sources[index].source != step.source {
                    return Err(MaterializationDiagnostic(format!(
                        "post-handoff writer target {:?} has inconsistent invocation values across fragments",
                        step.write.target
                    )));
                }
                index
            } else {
                let index = sources.len();
                sources.push(PostHandoffWriterSourceSlot {
                    target: step.write.target,
                    source: step.source,
                });
                index
            };
            if let Some(fit) = step.write.stored_integer_fit {
                fit_constraints.push(PostHandoffWriterFitConstraint {
                    source_slot,
                    field: step.write.field.clone(),
                    fit,
                });
            }
            steps.push(GeneratedPostHandoffWriterStep {
                container_byte_offset: step.write.container_byte_offset,
                container_width_bits: step.write.container_width_bits,
                destination_lsb: step.write.destination_lsb,
                source_lsb: step.write.source_lsb,
                width: step.write.width,
                source_slot,
            });
        }

        let fragment = GeneratedPostHandoffWriterFragmentPlan {
            context_abi: POST_HANDOFF_WRITER_CONTEXT_ABI_V1,
            byte_len: self.byte_len,
            byte_order: self.byte_order,
            source_slot_count: sources.len(),
            report_fingerprint: generated_post_handoff_writer_report_fingerprint(
                self.byte_len,
                self.byte_order,
                sources.len(),
                &steps,
            ),
            steps,
        };
        Ok(PostHandoffWriterInvocationPlan {
            fragment,
            placement: self.placement,
            sources,
            fit_constraints,
        })
    }

    /// Validate the complete direct-destination program without resolving a
    /// symbolic address or mutating the destination. Provider preparation
    /// uses this pass to bind one exact checked writer before any numeric
    /// entry address exists outside the sealed resolver.
    pub fn validate(
        &self,
        destination_len: usize,
        site: PlacementSite,
    ) -> Result<(), MaterializationDiagnostic> {
        validate_post_handoff_writer_nonempty(&self.steps)?;
        self.placement.validate_site(self.byte_len, site)?;
        if destination_len < self.byte_len {
            return Err(MaterializationDiagnostic(format!(
                "post-handoff writer needs {} bytes, destination has {}",
                self.byte_len, destination_len
            )));
        }
        let mut sources = Vec::<PostHandoffWriterSourceSlot>::new();
        for step in &self.steps {
            validate_post_handoff_writer_step(self.byte_len, step)?;
            if let Some(source) = sources
                .iter()
                .find(|source| source.target == step.write.target)
            {
                if source.source != step.source {
                    return Err(MaterializationDiagnostic(format!(
                        "post-handoff writer target {:?} has inconsistent invocation values across fragments",
                        step.write.target
                    )));
                }
            } else {
                sources.push(PostHandoffWriterSourceSlot {
                    target: step.write.target,
                    source: step.source,
                });
            }
        }
        Ok(())
    }
}

fn validate_post_handoff_writer_nonempty(
    steps: &[PostHandoffWriterStep],
) -> Result<(), MaterializationDiagnostic> {
    if steps.is_empty() {
        return Err(MaterializationDiagnostic(
            "post-handoff writer requires at least one fragment".into(),
        ));
    }
    Ok(())
}

fn validate_post_handoff_writer_step(
    byte_len: usize,
    step: &PostHandoffWriterStep,
) -> Result<(), MaterializationDiagnostic> {
    if let PostHandoffWriterSource::Resolve(target) = step.source
        && target != step.write.target
    {
        return Err(MaterializationDiagnostic(format!(
            "post-handoff writer source {target:?} does not match write target {:?}",
            step.write.target
        )));
    }
    validate_write(byte_len, &step.write)?;
    if let PostHandoffWriterSource::Resolved(source_value) = step.source {
        validate_write_source_value(&step.write, source_value, "pre-resolved symbolic")?;
    }
    Ok(())
}

fn generated_post_handoff_writer_report_fingerprint(
    byte_len: usize,
    byte_order: ByteOrder,
    source_slot_count: usize,
    steps: &[GeneratedPostHandoffWriterStep],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    hash_fingerprint_bytes(&mut hash, b"omega.post-handoff-writer.v1");
    hash_fingerprint_u64(&mut hash, POST_HANDOFF_WRITER_CONTEXT_ABI_V1);
    hash_fingerprint_u64(&mut hash, byte_len as u64);
    hash_fingerprint_byte(
        &mut hash,
        match byte_order {
            ByteOrder::LittleEndian => 0,
            ByteOrder::BigEndian => 1,
        },
    );
    hash_fingerprint_u64(&mut hash, source_slot_count as u64);
    hash_fingerprint_u64(&mut hash, steps.len() as u64);
    for step in steps {
        for value in [
            step.container_byte_offset,
            u64::from(step.container_width_bits),
            u64::from(step.destination_lsb),
            u64::from(step.source_lsb),
            u64::from(step.width),
            step.source_slot as u64,
        ] {
            hash_fingerprint_u64(&mut hash, value);
        }
    }
    if hash == 0 { 1 } else { hash }
}

fn hash_fingerprint_u64(hash: &mut u64, value: u64) {
    hash_fingerprint_bytes(hash, &value.to_le_bytes());
}

fn hash_fingerprint_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        hash_fingerprint_byte(hash, *byte);
    }
}

fn hash_fingerprint_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(0x100000001b3);
}
