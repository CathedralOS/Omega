//! Replacing a stored pointer does not identify its old target afterward.
//! Select the descriptor at the actual's captured cut, then keep the supplying
//! capture's own epoch and complete storage interpretation.

use super::ArgumentSources;
use checked_trees::{CheckedUnitStructuralPathSegment, FlowControlFacts};
use facts::PlaceSegment;
use validation::CanonicalPlace;

impl ArgumentSources<'_, '_> {
    pub(in crate::flow) fn captured_lender_subject(
        &self,
        control: &FlowControlFacts,
        position: usize,
        path: &[PlaceSegment],
    ) -> Result<(CanonicalPlace, Vec<CheckedUnitStructuralPathSegment>), &'static str> {
        let parameter = self
            .parameters
            .get(position)
            .ok_or("the lender path has no formal")?;
        if validation::reference_shell(self.program, parameter.type_reference)
            .is_none_or(|shell| shell.access != language_core::ReferenceAccess::Mutable)
        {
            return Err("the lender promise requires a mutable borrowed carrier");
        }
        if path.iter().any(|segment| {
            !matches!(
                segment,
                PlaceSegment::Field { .. } | PlaceSegment::FixedIndex { .. }
            )
        }) {
            return Err("the lender promise requires a static active inline path");
        }
        let referent = crate::checks::contracts::exclusive_referent_type(
            self.program,
            parameter.type_reference,
        );
        if !referent.is_valid() {
            return Err("the lender path is not below an exclusive borrowed carrier");
        }
        let mut selected = referent;
        let mut crossing = None;
        for index in 0..=path.len() {
            if let Some(shell) = validation::reference_shell(self.program, selected) {
                crossing = Some((index, shell.referee));
                break;
            }
            let Some(segment) = path.get(index) else {
                break;
            };
            selected = validation::project_type_reference_from_segments(
                self.program,
                selected,
                std::slice::from_ref(segment),
            )
            .ok_or("the lender field has no typed projection")?;
        }
        let (crossing, target) = crossing.ok_or("the lender path selects no stored reference")?;
        let (mut requested, _) =
            validation::structural_storage_path(self.program, referent, &path[..crossing])?;
        requested.push(CheckedUnitStructuralPathSegment::Referent);
        let (suffix, _) =
            validation::structural_storage_path(self.program, target, &path[crossing..])?;
        requested.extend(suffix);
        let capture = self
            .captures
            .iter()
            .find(|capture| capture.position == position && capture.destination.is_empty())
            .ok_or("the lender formal has no whole captured actual")?;
        if capture
            .later_writes
            .as_ref()
            .is_none_or(|writes| !writes.is_empty())
        {
            return Err("the captured lender needs an unavailable later-operand storage cut");
        }
        let captures = validation::completed_storage::stored_reference_sources_at_path(
            self.program,
            control,
            capture.handle,
            &path[..crossing],
            self.invocation_storage,
        )
        .captures()
        .filter(|captures| !captures.is_empty())
        .ok_or("the stored lender has an incoming or unavailable source alternative")?;
        let resolver = validation::CallFrameResolver::new(self.program)
            .ok_or("the lender capture has no call-frame owner")?;
        let state = self
            .program
            .state_by_symbol(self.state)
            .ok_or("the lender call has no state owner")?;
        let mut common = None;
        for stored in captures {
            if !control.operands.is_valid(stored.capture) {
                return Err("the stored lender capture is not recorded in this owner");
            }
            let installed = control.operands.get(stored.capture);
            let machine = self
                .program
                .machine_holding_state(installed.state_symbol)
                .ok_or("the stored lender installation has no machine owner")?;
            let leaves = resolver.completed_operand_reference_sources(
                machine,
                installed.state_symbol,
                installed.statement_index,
                installed.expression,
                stored.reference,
            )?;
            let projection = validation::corresponding_place_projection(
                self.program,
                stored.reference,
                &stored.projection,
            )
            .ok_or("the stored lender installation has no member correspondence")?;
            let sources = leaves
                .iter()
                .find(|leaf| leaf.path == projection)
                .ok_or("the stored lender installation has no selected reference leaf")?
                .sources
                .as_ref()
                .map_err(|reason| *reason)?;
            if sources.is_empty() {
                return Err("the stored lender installation has no source");
            }
            for source in sources {
                if source.invocation_output.is_some()
                    || source.invocation_result.is_some()
                    || source.is_image_storage
                    || source.collection_coarse
                    || source.representation.is_some()
                    || !source.recasts.is_empty()
                {
                    return Err("the lender source has opaque or represented storage");
                }
                let source = crate::flow::mutation::exact_source_place(
                    self.program,
                    state,
                    self.statement,
                    source,
                )
                .ok_or("the lender source has no exact structured coordinate")?;
                let subject = validation::completed_storage::project_captured_storage_place(
                    self.program,
                    self.state,
                    self.statement,
                    source,
                    &path[crossing..],
                )
                .ok_or("the lender subject has no complete storage projection")?;
                if common.as_ref().is_some_and(|prior| prior != &subject) {
                    return Err("the lender leaf has more than one physical subject");
                }
                common = Some(subject);
            }
        }
        Ok((
            common.ok_or("the lender leaf has no physical subject")?,
            requested,
        ))
    }
}
