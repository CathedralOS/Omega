//! Offline association checks; no compiler replay or source access.

use super::PackagePolicyRepresentation;
use crate::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
use std::cmp::Ordering;

impl PackagePolicyRepresentation {
    pub(crate) fn validate_canonical_structure(&self) -> Result<(), &'static str> {
        self.target.validate_canonical_structure()?;
        if self.declarations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err("representation declarations are unordered or repeated");
        }
        for declaration in &self.declarations {
            declaration.validate_resolved()?;
            if declaration.owner != PackageReviewNominalOwner::Package(self.package) {
                return Err("representation declaration has another package owner");
            }
        }
        self.validate_availability()?;
        self.validate_selections()?;
        self.validate_demands()
    }

    fn validate_owned_opaque(
        &self,
        opaque: &PackageReviewNominalIdentity,
    ) -> Result<(), &'static str> {
        opaque.validate_resolved()?;
        if opaque.owner == PackageReviewNominalOwner::Package(self.package)
            && self.declarations.binary_search(opaque).is_err()
        {
            return Err("representation policy omits its own opaque declaration");
        }
        Ok(())
    }

    fn validate_availability(&self) -> Result<(), &'static str> {
        if self
            .producer_availability
            .windows(2)
            .any(|pair| pair[0].conformance.identity >= pair[1].conformance.identity)
        {
            return Err("representation producer declarations are unordered or repeated");
        }
        for candidate in &self.producer_availability {
            self.validate_owned_opaque(&candidate.opaque)?;
            candidate.carrier.validate_resolved()?;
            let conformance = &candidate.conformance;
            crate::record::public_policy::validation::validate_conformance_shape(conformance)?;
            conformance.identity.validate_resolved()?;
            conformance.interface.trait_identity.validate_resolved()?;
            // `OpaqueRepresentation<Opaque, Representation>` names both data
            // types as ordinary arguments; the implementation has no subject.
            if conformance.identity.owner != PackageReviewNominalOwner::Package(self.package)
                || conformance.interface.arguments.len() != 2
                || !conformance.interface.lifetime_arguments.is_empty()
                || !conformance.interface.requirements.is_empty()
            {
                return Err(
                    "representation producer surface has inconsistent ownership or interface",
                );
            }
        }
        Ok(())
    }

    fn validate_selections(&self) -> Result<(), &'static str> {
        if self
            .selected_availability
            .windows(2)
            .any(|pair| pair[0].opaque >= pair[1].opaque)
        {
            return Err("representation selections repeat or reorder an opaque declaration");
        }
        for selection in &self.selected_availability {
            self.validate_owned_opaque(&selection.opaque)?;
            selection.carrier.validate_resolved()?;
            let application = &selection.application;
            application.declaration.validate_resolved()?;
            application.trait_identity.validate_resolved()?;
            if selection.selection_owner != PackageReviewNominalOwner::Package(self.package)
                || !application.lifetime_arguments.is_empty()
                || !application.trait_lifetime_arguments.is_empty()
                || application.trait_arguments.len() != 2
                || !application.rows.is_empty()
            {
                return Err(
                    "representation selection has an inconsistent owner or closed interface",
                );
            }
            if let Ok(index) = self.producer_availability.binary_search_by(|candidate| {
                candidate.conformance.identity.cmp(&application.declaration)
            }) {
                let candidate = &self.producer_availability[index];
                if candidate.opaque != selection.opaque
                    || candidate.carrier != selection.carrier
                    || candidate.conformance.interface.trait_identity != application.trait_identity
                    || candidate.conformance.interface.arguments != application.trait_arguments
                {
                    return Err(
                        "representation selection differs from its public producer surface",
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_demands(&self) -> Result<(), &'static str> {
        if self
            .demands
            .windows(2)
            .any(|pair| pair[0].compare_application(&pair[1]) != Ordering::Less)
        {
            return Err("representation demand applications are unordered or repeated");
        }
        let mut remaining = self.demands.as_slice();
        while let Some(first) = remaining.first() {
            let count = remaining
                .iter()
                .take_while(|demand| first.compare_calling_application(demand).is_eq())
                .count();
            let group = &remaining[..count];
            let calling = &first.calling;
            calling.validate_canonical_structure()?;
            if calling.target != self.target || group.len() != calling.opaque_uses.len() {
                return Err("representation demand target or complete opaque-use coverage differs");
            }
            for (demand, use_) in group.iter().zip(&calling.opaque_uses) {
                if demand.calling != *calling || demand.opaque != use_.opaque {
                    return Err(
                        "representation demands disagree on their shared calling application",
                    );
                }
                let index = self
                    .selected_availability
                    .binary_search_by(|selection| selection.opaque.cmp(&demand.opaque))
                    .map_err(|_| "representation demand has no selected availability")?;
                let selection = &self.selected_availability[index];
                if use_.carrier != selection.carrier
                    || use_.selection_owner != selection.selection_owner
                    || use_.application != selection.application
                    || use_.origin != selection.origin
                    || use_.lifecycle != selection.lifecycle
                    || use_.copy_disposition != selection.copy_disposition
                {
                    return Err(
                        "representation demand disagrees with its complete selected meaning",
                    );
                }
            }
            remaining = &remaining[count..];
        }
        Ok(())
    }
}
