//! Retain the selected handler ABI independently of emitted entry metadata.
//! Both foreign registrations and installed arrivals use the same exact body,
//! Calling selection and nominal-formal join before receiving this receipt.
use super::{AdmittedCallbackEntry, NativeRealizationInput, ProviderAdmissionRequest};
use backend_plan::TerminalCallbackOccurrenceProposal;
use calling_conventions::{BoundaryCallingApplication, BoundaryNativeParameterOrigin};
use semantic_vocabulary::{BoundaryMachineId, IdTable, MachineId};
use std::collections::BTreeMap;
use terminal_psi::{TerminalCallbackApplication, TerminalMachine};

pub(super) struct PreparedEntries<'a> {
    input: &'a NativeRealizationInput<'a>,
    request: &'a ProviderAdmissionRequest<'a, 'a>,
    bodies: IdTable<MachineId, Option<&'a TerminalMachine>>,
    occurrences: Vec<(usize, &'a TerminalCallbackOccurrenceProposal)>,
    calling: BTreeMap<
        (&'a semantic_vocabulary::CallingAbiIdentity, Option<&'a str>),
        &'a BoundaryCallingApplication,
    >,
}
impl<'a> PreparedEntries<'a> {
    pub(super) fn new(
        input: &'a NativeRealizationInput<'a>,
        request: &'a ProviderAdmissionRequest<'a, 'a>,
    ) -> Result<Self, String> {
        if request.callback_occurrences.is_empty() {
            return Ok(Self {
                input,
                request,
                bodies: IdTable::default(),
                occurrences: Vec::new(),
                calling: BTreeMap::new(),
            });
        }
        let mut bodies = Vec::new();
        let mut machines: Vec<_> = input.module().machines.iter().collect();
        machines.sort_unstable_by_key(|body| body.id);
        let first = machines.first().map(|body| body.id.get());
        if let Some(first) = first {
            let last = machines.last().expect("nonempty machine roster").id.get();
            let length = last
                .checked_sub(first)
                .and_then(|n| n.checked_add(1))
                .and_then(|n| usize::try_from(n).ok())
                .ok_or("callback body identity span overflows")?;
            bodies
                .try_reserve_exact(length)
                .map_err(|_| "callback body identity span cannot be allocated")?;
            bodies.resize(length, None);
            for body in machines {
                let index = usize::try_from(body.id.get() - first)
                    .map_err(|_| "callback body identity exceeds index")?;
                if bodies[index].replace(body).is_some() {
                    return Err("canonical callback body identity is duplicated".into());
                }
            }
        }
        let bodies = IdTable::from_rows(bodies.into_iter().enumerate().map(|(index, body)| {
            (
                MachineId::new(first.expect("nonempty indexed roster") + index as u64)
                    .expect("validated nonzero machine identity"),
                body,
            )
        }))
        .map_err(|_| "callback body identity span is inconsistent")?;
        let mut occurrences: Vec<_> = request
            .callback_occurrences
            .iter()
            .map(|row| (row.placement_index(), row))
            .collect();
        occurrences.sort_unstable_by_key(|row| row.0);
        if occurrences.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("retained callback placement is duplicated".into());
        }
        let mut applications: Vec<_> = occurrences
            .iter()
            .map(|(_, row)| row.application_index())
            .collect();
        applications.sort_unstable();
        if applications.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err("retained callback application is duplicated".into());
        }
        let mut calling = BTreeMap::new();
        for row in request.installed_calling {
            if calling
                .insert((&row.abi, row.calling_owner_application.as_deref()), row)
                .is_some()
            {
                return Err("selected Calling application is duplicated".into());
            }
        }
        Ok(Self {
            input,
            request,
            bodies,
            occurrences,
            calling,
        })
    }
    pub(super) fn occurrence(
        &self,
        placement: usize,
    ) -> Result<&'a TerminalCallbackOccurrenceProposal, String> {
        self.occurrences
            .binary_search_by_key(&placement, |row| row.0)
            .ok()
            .map(|i| self.occurrences[i].1)
            .ok_or("entry has no independently retained callback occurrence".into())
    }
    pub(super) fn selected(
        &self,
        boundary: BoundaryMachineId,
        owner: &str,
    ) -> Result<&'a BoundaryCallingApplication, String> {
        let abi = self.input.selected_calling_abi_identity(boundary)?;
        let row = *self
            .calling
            .get(&(&abi, Some(owner)))
            .ok_or("entry has no independently selected Calling application")?;
        if row.target != self.request.target() || row.recomputed_commitment()? != row.commitment {
            return Err(
                "selected Calling application changed its target or complete payload".into(),
            );
        }
        Ok(row)
    }
    pub(super) fn admit(
        &self,
        callback: &TerminalCallbackApplication,
        occurrence: &TerminalCallbackOccurrenceProposal,
    ) -> Result<AdmittedCallbackEntry, String> {
        let module = self.input.module();
        let thunk = occurrence.callback_thunk_artifact();
        if !callback.entry_admitted
            || callback.application_index != occurrence.application_index()
            || callback.registrar != occurrence.terminal_operation()
            || occurrence.callback_thunk_identity().caller() != self.input.plan().psi
            || occurrence
                .callback_thunk_identity()
                .callback_thunk_placement_index()
                != Some(occurrence.placement_index())
            || thunk.artifact().view().identity() != self.input.plan().psi
        {
            return Err("entry changed its canonical program or callback application".into());
        }
        let body = self
            .bodies
            .get(thunk.entry())
            .and_then(|body| *body)
            .ok_or("entry lost its exact canonical body")?;
        let declaration = body
            .declaration
            .as_ref()
            .ok_or("entry body has no checked declaration")?;
        let formals = body.runtime_parameter_formals()?;
        if !declaration.same_identity_as(&module.identities, &callback.body, &module.identities)
            || formals.as_slice() != thunk.parameter_formals()
        {
            return Err("entry changed its canonical body declaration or formal order".into());
        }
        let boundary = callback.boundary.ok_or("entry lost its inbound boundary")?;
        let inbound = self.selected(boundary, &callback.calling_owner_application)?;
        if inbound.plan.plan() != occurrence.boundary_entry_plan() {
            return Err("entry changed its independently selected inbound plan".into());
        }
        let boundary = super::super::selected_calling_boundary(
            module,
            self.input.target_machine_selections(),
            boundary,
        )?;
        let parameters = calling_conventions::TerminalCallingTelescope::for_application(
            module, boundary, inbound,
        )?
        .parameters;
        if inbound.dispatch_only_parameter_count != 0
            || parameters.len() != occurrence.entry_parameters().len()
            || parameters.len() != formals.len()
        {
            return Err("entry changed its complete inbound parameter roster".into());
        }
        let projection = inbound.call_projection()?;
        let mut formal_ordinals = Vec::with_capacity(parameters.len());
        for (expected, actual) in parameters.iter().zip(occurrence.entry_parameters()) {
            let ordinal = projection
                .parameter_ordinal(expected.native_ordinal)?
                .ok_or("entry contains a dispatch-only parameter")?;
            let placement = inbound
                .plan
                .plan()
                .call
                .parameters
                .get(ordinal)
                .ok_or("entry parameter has no inbound placement")?;
            let BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } = actual.origin
            else {
                return Err("entry contains a non-formal inbound parameter".into());
            };
            if actual.origin != expected.origin
                || actual.application.parameter != expected.identity
                || usize::try_from(actual.application.native_ordinal).ok() != Some(ordinal)
                || actual.application.placement != *placement
                || actual.application.shape != placement.shape
            {
                return Err(
                    "entry parameter changed its declared origin or inbound placement".into(),
                );
            }
            formal_ordinals.push((formal_ordinal, actual.application.native_ordinal));
        }
        formal_ordinals.sort_unstable();
        if formal_ordinals
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0)
        {
            return Err("entry repeats an inbound semantic formal".into());
        }
        let inbound_ordinals = formals
            .iter()
            .map(|formal| {
                formal_ordinals
                    .binary_search_by_key(formal, |row| row.0)
                    .ok()
                    .map(|i| formal_ordinals[i].1)
                    .ok_or("entry lost a canonical body's semantic formal".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AdmittedCallbackEntry {
            psi: self.input.plan().psi,
            target: self.request.target(),
            machine: body.id,
            inbound: inbound.plan.plan().clone(),
            inbound_ordinals,
        })
    }
}
