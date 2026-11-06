//! Forming a qualified loan can prove an explicitly requested value predicate.
//! Its old source duties remain a separate complete preservation obligation;
//! neither incidental observations nor the requested result prove themselves.
use super::*;

impl PayloadMembershipHistory<'_> {
    pub fn reference_formation_memberships(
        &mut self,
        identities: &terminal_psi::TerminalIdentityTable,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        source: &StructuralArgument,
        storage: Option<&terminal_psi::ReferenceStorageView>,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Result<Vec<terminal_psi::StructuralPathQualification>, &'static str> {
        if !result.qualifications.is_empty() {
            return Err("a reference introduction cannot qualify its pointer descriptor");
        }
        let mut preserved = self.reference_preservation_memberships(
            identities, analysis, position, source, storage, result,
        )?;
        let requested = result
            .projected_qualifications
            .iter()
            .filter(|row| !preserved.contains(row))
            .cloned()
            .collect::<Vec<_>>();
        if requested.is_empty() {
            return Ok(preserved);
        }
        let StructuralTypeShape::Reference { referent, .. } =
            terminal_psi::find_structural_type(self.types, result.structural_type)
                .ok_or("the formed reference has no carrier")?
                .shape
        else {
            return Err("the formed reference has no reference carrier");
        };
        // Preserve authority prerequisites only from the authenticated old
        // source roster. Observed predicate closure cannot invent an issuer or
        // a content receipt for a missing prerequisite.
        for row in &requested {
            let path = row
                .path
                .strip_prefix(&[StructuralPathSegment::Referent])
                .ok_or("reference predicate introduction is outside its referent")?;
            let carrier = qualification_path_carrier(self.types.iter(), referent, path)
                .ok_or("reference predicate introduction has no typed selected carrier")?;
            let domain = self
                .domains
                .iter()
                .find(|domain| domain.id == row.domain)
                .filter(|domain| domain.carrier == carrier)
                .ok_or("reference predicate introduction has no matching domain carrier")?;
            let supplied = preserved
                .iter()
                .filter(|old| old.path == row.path)
                .map(|old| old.domain)
                .collect::<Vec<_>>();
            payload_predicates::introduction_predicate(self.types, self.domains, domain, &supplied)
                .ok_or("reference introduction has an unproved authority, content or predicate prerequisite")?;
        }
        let root = self
            .sources
            .contract(source.place)
            .ok_or("reference predicate introduction has no independent source")?;
        let path =
            crate::reference_source_storage_path(self.types.iter(), root.carrier, &source.path);
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| descriptor.and_then(|machine| incoming_flow(&machine.flow)))
            .as_ref();
        let current = selected_payload_contract_reusing(
            self.types,
            self.domains,
            self.sources.identity(),
            descriptor,
            incoming,
            analysis,
            position,
            source.place,
            path,
            &|place| self.sources.contract(place),
            self.entry_snapshots.as_deref().unwrap_or(&[]),
            &mut self.predicate_observations,
            self.normal.as_mut(),
        )?
        .presented(self.types, self.domains, referent)
        .ok_or("reference predicate introduction changes its current source carrier")?;
        for row in requested {
            let path = row
                .path
                .strip_prefix(&[StructuralPathSegment::Referent])
                .ok_or("reference predicate introduction is outside its referent")?;
            let proved = if path.is_empty() {
                current.qualifications.contains(&row.domain)
            } else {
                current
                    .projected_qualifications
                    .iter()
                    .any(|candidate| candidate.path == path && candidate.domain == row.domain)
            };
            if !proved {
                return Err(
                    "reference predicate introduction is not proved by every current source alternative",
                );
            }
            preserved.push(row);
        }
        preserved.sort();
        preserved.dedup();
        Ok(preserved)
    }
}
