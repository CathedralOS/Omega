//! A call can restore field guarantees after their storage was created.
//! Keep those facts separate from the initial declaration. Every read still
//! proves its current contents before a fresh value transports the guarantee.
use super::SourceCallCoordinate;
use semantic_vocabulary::{OperationId, PlaceId, StructuralDomainId};
use terminal_psi::StructuralPathQualification;

#[derive(Default)]
pub(super) struct RestoredCallMemberships {
    // Signatures precede body allocation, so global IDs have gaps within a
    // frame. Store only actual homes; calls on one home share its row.
    homes: Vec<RestoredHomeMemberships>,
}

struct RestoredHomeMemberships {
    place: PlaceId,
    calls: Vec<RestoredCallMembership>,
}

pub(crate) struct RestoredCallMembership {
    pub(crate) coordinate: SourceCallCoordinate,
    operation: OperationId,
    pub(crate) qualifications: Vec<StructuralDomainId>,
    pub(crate) projected: Vec<StructuralPathQualification>,
}

impl RestoredCallMemberships {
    pub(super) fn candidates(&self, place: PlaceId) -> &[RestoredCallMembership] {
        self.homes
            .binary_search_by_key(&place, |home| home.place)
            .map_or(&[], |index| self.homes[index].calls.as_slice())
    }

    pub(super) fn record(
        &mut self,
        place: PlaceId,
        coordinate: SourceCallCoordinate,
        operation: OperationId,
        qualifications: &[StructuralDomainId],
        projected: &[StructuralPathQualification],
    ) -> Result<(), &'static str> {
        let index = match self.homes.binary_search_by_key(&place, |home| home.place) {
            Ok(index) => index,
            Err(index) => {
                self.homes.insert(
                    index,
                    RestoredHomeMemberships {
                        place,
                        calls: Vec::new(),
                    },
                );
                index
            }
        };
        let rows = &mut self.homes[index].calls;
        let row = if let Some(index) = rows.iter().position(|row| row.operation == operation) {
            if rows[index].coordinate != coordinate {
                return Err("restored membership operation has different source coordinates");
            }
            &mut rows[index]
        } else {
            let index = rows.len();
            rows.push(RestoredCallMembership {
                coordinate,
                operation,
                qualifications: Vec::new(),
                projected: Vec::new(),
            });
            &mut rows[index]
        };
        row.qualifications.extend_from_slice(qualifications);
        row.qualifications.sort();
        row.qualifications.dedup();
        row.projected.extend_from_slice(projected);
        row.projected.sort();
        row.projected.dedup();
        Ok(())
    }
}
