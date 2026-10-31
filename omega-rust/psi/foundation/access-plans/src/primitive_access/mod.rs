//! One primitive access: projecting a access field, the sealed request,
//! its specialization for stable, external and atomic lowering, and the
//! corresponded forms that carry device correspondence.

/// The correspondence-required custody every specialized access carries:
/// a request joined to the originating view's exact correspondence, its
/// replay-for-lowering gate and rejection carrier, the
/// `into_corresponded_*` staging entrypoint, and the validator that
/// requires admitted correspondence. The per-model spec supplies the
/// names and the wording; the machinery is emitted once here.
macro_rules! corresponded_primitive_access {
    (
        $request:ident,
        $rejection:ident,
        $access:ident,
        $accessor:ident,
        $into_access:ident,
        $require:ident,
        $validator:ident,
        must_use = $must_use:literal,
        specialization = $specialization:literal,
        placed_doc = $placed_doc:literal,
        require_doc = $require_doc:literal,
        storage_doc = $storage_doc:literal,
        rejection_event = $rejection_event:literal,
        required_message = $required_message:literal,
        retained_message = $retained_message:literal $(,)?
    ) => {
        #[doc = concat!("One exact ", $specialization, " specialization joined to the physical correspondence retained by its originating access view.")]
        #[derive(Debug)]
        #[must_use = $must_use]
        pub struct $request<'view, 'extent> {
            access: $access<'view, 'extent>,
            correspondence: &'view AdmittedSchemaDeviceCorrespondence,
        }

        impl<'view, 'extent> $request<'view, 'extent> {
            #[doc = concat!("The exact ", $specialization, " specialization retained by this custody join.")]
            pub const fn $accessor(&self) -> &$access<'view, 'extent> {
                &self.access
            }

            /// The exact lifetime-bound, non-Clone physical correspondence
            /// retained by the originating access view.
            pub const fn correspondence(&self) -> &'view AdmittedSchemaDeviceCorrespondence {
                self.correspondence
            }

            #[doc = concat!("Replay both the complete ", $specialization, " specialization and its exact correspondence identity before a provider/device consumer proceeds. Rejection only borrows this carrier, so no ", $rejection_event, " occurs and the complete input remains available for repair and retry.")]
            pub fn validate_for_provider_lowering(&self) -> Result<(), AccessPlanDiagnostic> {
                let replayed = $validator(&self.access)?;
                if !std::ptr::eq(replayed, self.correspondence) {
                    return Err(AccessPlanDiagnostic($retained_message.into()));
                }
                Ok(())
            }

            #[doc = concat!("Remove only this correspondence-required staging layer. The original ", $placed_doc)]
            pub fn $into_access(self) -> $access<'view, 'extent> {
                self.access
            }

            #[cfg(test)]
            pub(crate) fn replace_request_plan_for_test(
                &mut self,
                placement: AccessPlanId,
            ) -> AccessPlanId {
                std::mem::replace(&mut self.access.request.plan, placement)
            }

            #[cfg(test)]
            pub(crate) fn replace_correspondence_for_test(
                &mut self,
                correspondence: &'view AdmittedSchemaDeviceCorrespondence,
            ) -> &'view AdmittedSchemaDeviceCorrespondence {
                std::mem::replace(&mut self.correspondence, correspondence)
            }
        }

        #[doc = concat!("Failed correspondence-required staging returns the exact already- specialized ", $specialization, " request. No provider/device operation is selected or attempted.")]
        #[derive(Debug)]
        pub struct $rejection<'view, 'extent> {
            access: $access<'view, 'extent>,
            diagnostic: AccessPlanDiagnostic,
        }

        impl<'view, 'extent> $rejection<'view, 'extent> {
            diagnostic_accessor!();

            pub fn into_parts(
                self,
            ) -> ($access<'view, 'extent>, AccessPlanDiagnostic) {
                (self.access, self.diagnostic)
            }
        }

        impl<'view, 'extent> $access<'view, 'extent> {
            #[doc = concat!("Require the exact schema/device correspondence retained by this ", $require_doc)]
            #[doc = concat!("Ordinary correspondence-free ", $storage_doc)]
            pub fn $require(self) -> Result<$request<'view, 'extent>, $rejection<'view, 'extent>> {
                let correspondence = match $validator(&self) {
                    Ok(correspondence) => correspondence,
                    Err(diagnostic) => {
                        return Err($rejection {
                            access: self,
                            diagnostic,
                        });
                    }
                };
                Ok($request {
                    access: self,
                    correspondence,
                })
            }
        }

        fn $validator<'access, 'view, 'extent>(
            access: &'access $access<'view, 'extent>,
        ) -> Result<&'view AdmittedSchemaDeviceCorrespondence, AccessPlanDiagnostic> {
            access.validate_for_lowering()?;
            access
                .request
                ._authority
                .correspondence()
                .ok_or_else(|| AccessPlanDiagnostic($required_message.into()))
        }
    };
}

/// The placed-access authority every access keeps: its resident claim,
/// the placed occurrence it projects, and the retained correspondence.
macro_rules! authority_accessors {
    () => {
        pub const fn resident_claim(&self) -> Option<ResidentClaimId> {
            self.resident_claim
        }

        pub const fn access_occurrence(&self) -> Option<AccessOccurrenceId> {
            self.access_occurrence
        }

        pub const fn correspondence(&self) -> Option<&AdmittedSchemaDeviceCorrespondence> {
            self._authority.correspondence()
        }
    };
}

pub(crate) mod corresponded_atomic;
pub(crate) mod corresponded_external;
pub(crate) mod corresponded_stable;
pub(crate) mod corresponded_stable_compound;
pub(crate) mod field_projection;
pub(crate) mod primitive_request;
pub(crate) mod primitive_specialization;
