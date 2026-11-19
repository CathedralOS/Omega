// Close physical callback/handler plans and installation requirements before
// encoding the root or any private body. Optimized physical references decide
// membership; merely available callbacks do not become emission roots.

use super::callback_thunks::{PreparedCallbackBodies, prepare_installed_callbacks};
use super::physical_body::{self, BodyEmissionError};
use super::selected_physical_pipeline_failed;
use artifact_lowering::ValidatedNativeProgramEntrySettlement;
use diagnostics::Diagnostic;
use native_artifact::{
    FragmentObjectPublicationRequest, PublishedFragmentObject, publish_fragment_object,
};
use provider_admission::provider_admission::realization_error;
use published_object_verification::NativeStageReplay;

pub(crate) struct EmittedRealizationObject {
    pub stage_replay: NativeStageReplay,
    pub physical_originals: native_artifact::NativePhysicalOriginals,
    pub object: PublishedFragmentObject,
}

/// The root and the closed private roster advance together. The next stage
/// receives its input only after every body in the current stage succeeds.
struct BodyRoster<Root, Private> {
    root: Root,
    private: Vec<Private>,
}

impl<Root, Private> BodyRoster<Root, Private> {
    fn try_map<NextRoot, NextPrivate, Error>(
        self,
        root: impl FnOnce(Root) -> Result<NextRoot, Error>,
        private: impl FnMut(Private) -> Result<NextPrivate, Error>,
    ) -> Result<BodyRoster<NextRoot, NextPrivate>, Error> {
        Ok(BodyRoster {
            root: root(self.root)?,
            private: self
                .private
                .into_iter()
                .map(private)
                .collect::<Result<_, _>>()?,
        })
    }
}

pub(crate) fn emit_realization_object(
    roster: super::plan_callables::PlannedProgramBodies<'_>,
    program_entry: Option<&ValidatedNativeProgramEntrySettlement>,
    runtime_provisions: Option<
        super::runtime_provisions::NativeRuntimePlan<
            super::runtime_provisions::NativeForeignAdapterRequest,
        >,
    >,
    observation: &crate::pipeline::observations::NativePipelineObservation,
) -> Result<EmittedRealizationObject, Vec<Diagnostic>> {
    let (target, prepared_callbacks) = PreparedCallbackBodies::from_roster(roster)?;
    let physical_root = program_entry
        .filter(|entry| entry.source().target_slot().owner == target::TargetProfile::UefiX64)
        .and_then(|entry| {
            entry.physical_adapter().map(|adapter| {
                installation_conditions::InstalledPhysicalStackRoot {
                    semantic_entry: entry.checked_entry().terminal_entry(),
                    adapter: adapter.machine(),
                }
            })
        });
    if physical_root.is_some_and(|entry| entry.adapter != target.execution_plan().plan().entry) {
        return Err(realization_error(
            "installed arrival stack",
            "physical adapter differs from the emitted root",
        ));
    }
    let requirements = prepare_installed_callbacks(&target, &prepared_callbacks, physical_root)?;
    let selections = target.selections().clone();
    let root_error = |error| match error {
        BodyEmissionError::Physical(error) => selected_physical_pipeline_failed(&selections, error),
        BodyEmissionError::Installation(reason) => {
            realization_error("installed handler stack", reason)
        }
        BodyEmissionError::Object(diagnostics) => diagnostics,
    };
    let physical = physical_body::prepare_physical_body(target, None, requirements, observation)
        .map_err(root_error)?;
    let private_bodies = prepared_callbacks.finish_physical()?;
    let hosted_entry = if entry_planning::hosted_program::needs_hosted_entry(
        &physical.stage.program().frame,
        &physical.stage.facts().semantic.program,
        program_entry.map(|entry| entry.source().target_slot().owner),
    ) {
        let demand = super::body_resources::plan(&physical, &private_bodies)?;
        let program = &physical.stage.program().frame.instructions;
        let receiver = program_entry
            .map(|entry| {
                verified_optimization_input::hosted_receiver::AdmittedHostedReceiver::admit(
                    program.target,
                    program.contracts.profile(),
                    &physical.stage.facts().semantic.admission,
                    physical
                        .stage
                        .facts()
                        .semantic
                        .program
                        .functions
                        .iter()
                        .find(|function| function.machine == program.entry),
                    program.contracts.function_abi(program.entry),
                    entry,
                )
            })
            .transpose()
            .map_err(|error| vec![error])?;
        Some(
            entry_planning::hosted_program::plan_hosted_program_entry(
                entry_planning::hosted_program::HostedProgramEntryRequest {
                    frame: &physical.stage.program().frame,
                    program: &physical.stage.facts().semantic.program,
                    receiver,
                    demand,
                },
            )
            .map_err(|error| vec![error])?,
        )
    } else {
        None
    };
    let physical_entry = program_entry
        .filter(|entry| entry.source().target_slot().owner == target::TargetProfile::UefiX64)
        .map(|entry| {
            let adapter = entry.physical_adapter().ok_or_else(|| {
                realization_error(
                    "physical program entry",
                    "semantic entry requires an explicitly selected checked physical adapter",
                )
            })?;
            if physical.stage.program().frame.instructions.entry != adapter.machine() {
                return Err(realization_error(
                    "physical program entry",
                    "physical root is not the selected physical adapter",
                ));
            }
            let calling = entry.physical_calling_plan().ok_or_else(|| {
                realization_error("physical program entry", "missing physical calling plan")
            })?;
            let contract = entry
                .storage_entry()
                .and_then(|storage| storage.physical_contract())
                .ok_or_else(|| {
                    realization_error(
                        "physical program entry",
                        "missing selected physical entry contract",
                    )
                })?;
            let private_entries = private_bodies
                .iter()
                .map(
                    |body| entry_planning::physical_entry::PrivatePhysicalEntry {
                        psi: body.callable.body.stage.program().frame.instructions.psi,
                        target: body.callable.body.stage.program().frame.instructions.target,
                        installations: body
                            .callable
                            .body
                            .stage
                            .installations()
                            .for_entry_planning(),
                        entry: &body.entry,
                    },
                )
                .collect::<Vec<_>>();
            entry_planning::physical_entry::plan_physical_entry(
                entry_planning::physical_entry::PhysicalEntryPlanningRequest {
                    frame: &physical.stage.program().frame,
                    selection: physical
                        .stage
                        .facts()
                        .semantic
                        .admission
                        .physical_entry(entry)
                        .map_err(|reason| realization_error("physical image entry", reason))?,
                    installations: physical.stage.installations().for_entry_planning(),
                    private_entries: &private_entries,
                    physical: calling.plan(),
                    contract,
                    demand: super::body_resources::plan(&physical, &private_bodies)?,
                },
            )
            .map_err(|error| vec![error])
        })
        .transpose()?;
    let handlers =
        super::callback_thunks::retained_handler_measurements(&physical, &private_bodies)?;
    let physical = physical.bind(&handlers, physical_root)?;
    let private_bodies = private_bodies
        .into_iter()
        .map(|body| body.bind(&handlers))
        .collect::<Result<Vec<_>, _>>()?;
    let hosted_entry = hosted_entry.map(installation_binding::entries::bind_hosted_entry);
    let physical_entry = physical_entry.map(installation_binding::entries::bind_physical_entry);
    let runtime_provisions = runtime_provisions.map(|runtime| {
        runtime.map_adapters(|request| {
            super::auxiliary_native::plan(request)
                .map(installation_binding::entries::bind_foreign_adapter)
        })
    });
    // Body membership, entry transport and installation resources are now closed.
    // Encoding consumes these plans and cannot discover or request another body.
    let runtime_provisions =
        runtime_provisions.map(|runtime| runtime.map_adapters(super::auxiliary_native::encode));
    let hosted_entry = hosted_entry
        .map(native_encoding::hosted_program::encode_hosted_program_entry)
        .transpose()
        .map_err(|error| vec![error])?;
    let physical_entry = physical_entry
        .map(native_encoding::physical_entry::encode_physical_entry)
        .transpose()
        .map_err(|error| vec![error])?;
    let encoded = BodyRoster {
        root: physical,
        private: private_bodies,
    }
    .try_map(
        |body| physical_body::encode_physical_body(body, observation).map_err(root_error),
        |body| body.encode(),
    )?;
    let runtime_provisions =
        runtime_provisions.map(|runtime| runtime.map_adapters(super::auxiliary_native::place));
    let hosted_entry = hosted_entry
        .map(placement::program_entry_layout::resolve_hosted_program)
        .transpose()
        .map_err(|error| vec![error])?
        .map(relaxation::retain_fixed_entry)
        .map(function_layout::place_fixed_entry);
    let physical_entry = physical_entry
        .map(placement::program_entry_layout::resolve_physical_program)
        .transpose()
        .map_err(|error| vec![error])?
        .map(relaxation::retain_fixed_entry)
        .map(function_layout::place_fixed_entry);
    let layouts = encoded.try_map(
        |body| physical_body::layout_encoded_body(body, observation).map_err(root_error),
        |body| body.layout(),
    )?;
    let runtime_provisions =
        runtime_provisions.map(|runtime| runtime.map_adapters(super::auxiliary_native::construct));
    let hosted_entry =
        hosted_entry.map(object_file::program_entry_object::HostedProgramObject::from_placement);
    let physical_entry = physical_entry
        .map(object_file::program_entry_object::PhysicalProgramObject::from_placement);
    let objects = layouts.try_map(
        |body| physical_body::construct_body_object(body).map_err(root_error),
        |body| body.construct_object(),
    )?;
    let mut private_functions = Vec::with_capacity(objects.private.len());
    let mut private_originals = Vec::with_capacity(objects.private.len());
    let mut private_replays = Vec::new();
    for body in objects.private {
        if let Some(inputs) = body.stage_originals {
            private_replays.push((body.object.identity, inputs));
        }
        private_originals.push((body.object.identity, body.physical_original));
        private_functions.push(body.object);
    }
    let physical_originals = native_artifact::NativePhysicalOriginals::new(private_originals)
        .map_err(|reason| realization_error("private physical originals", reason))?;
    let object = publish_fragment_object(
        objects.root.projection,
        FragmentObjectPublicationRequest {
            runtime: runtime_provisions,
            program_entry,
            physical_entry,
            hosted_entry,
            private_functions: &private_functions,
        },
    )?;
    let stage_replay = NativeStageReplay::from_bodies(objects.root.originals, private_replays)
        .map_err(|error| vec![error])?;
    Ok(EmittedRealizationObject {
        stage_replay,
        physical_originals,
        object,
    })
}

#[cfg(test)]
mod tests {
    use super::BodyRoster;
    use std::cell::RefCell;

    #[test]
    fn image_emission_waits_for_the_last_private_object() {
        let bodies = ["root", "callback", "installed handler"];
        for refused in [None, Some("installed handler")] {
            let events = RefCell::new(Vec::new());
            let construct = |body| {
                events.borrow_mut().push(body);
                if refused == Some(body) {
                    Err(body)
                } else {
                    Ok(body)
                }
            };
            let result = BodyRoster {
                root: bodies[0],
                private: bodies[1..].to_vec(),
            }
            .try_map(construct, construct)
            .inspect(|_| events.borrow_mut().push("image"));
            let mut expected = bodies.to_vec();
            match refused {
                Some(reason) => assert_eq!(result.err(), Some(reason)),
                None => {
                    let objects = result.unwrap();
                    assert_eq!(objects.root, bodies[0]);
                    assert_eq!(objects.private.as_slice(), &bodies[1..]);
                    expected.push("image");
                }
            }
            assert_eq!(events.into_inner(), expected);
        }
    }
}
