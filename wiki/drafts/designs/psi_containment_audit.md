# Psi containment audit

This audit is step 5 of [Psi admission and interpreter preparation](psi_claim_admission.md#implementation-work).
It checks the interpreter, embedding and runtime host against the containment
rules in [Ownership and lifetime](../../spec/build/embedding.md#ownership-and-lifetime),
[Execution and interference](../../spec/build/embedding.md#execution-and-interference)
and [Closing and failure](../../spec/build/embedding.md#closing-and-failure). It
was made on `main` after #6468.

**Audited implementation.** Untrusted interpretation is open through the embedding. A host
can admit an artifact without producer trust and run it on the interpreter
under bindings that enforce what they rely on (see
[Contained interpretation](#contained-interpretation)). Every other route still
requires the host's trust in the exact artifact and producer, and native
lowering never accepts the contained admission. A faulted, stopped or waiting
guest can be discarded under the four-step sequence (see [Discard](#discard)).
What is still open is listed under [Remaining](#remaining).

## Routes into execution

| Route | Gate | Evidence |
| --- | --- | --- |
| `omega run <program.psi>` | Without `--trusted-artifact`, `open_run_program` decodes under `PsiLoadLimits` and admits the artifact for contained interpretation. Runtime admission selects its host realizations, and the interpreter runs it under `TerminalRuntimeContainment`. See [Contained runtime admission](#contained-runtime-admission). | `application/src/saved_program/input.rs`, `application/src/execution/contained.rs`; `saved_psi_without_producer_trust_runs_contained` |
| `omega run --trusted-artifact <manifest-id> <program.psi>` and native compilation of saved Psi | `--trusted-artifact` names the host-accepted artifact. `open_trusted_program` decodes under `PsiLoadLimits` and authorizes. Compiling saved Psi to native code always requires it. | `application/src/saved_program/input.rs`; #6458, #6462 |
| `omega psi inspect --artifact` | Inspection only. Decoding runs under the input cap and work budget and never executes. | `application/src/inspection/mod.rs`; `named_psi_inspection` tests |
| Embedding: `TerminalProgram::admit`, `retain_authorized`, `TerminalExecution::start_artifact` | Each takes `&AuthorizedTerminalArtifact`. `borrow` and `retain` refuse received code. Only `authorize_terminal_artifact` makes an executable copy. | `received_artifacts_execute_only_with_host_trust_in_their_exact_bytes`, `only_authorization_records_host_acceptance` |
| Runtime host: `HostedTerminalProgram::prepare`, component preparation | Refuses an unauthorized received artifact with `InvalidProvision`. | `interpreter/host/src/execution.rs`, `execution/components.rs` |
| Native lowering | Image emission, component emission, foreign-adapter compilation and the optimizer require `AuthorizedTerminalArtifact`. Runtime admission takes `AdmittedTerminalArtifact`, which can also hold a contained artifact. | `psi-to-native` driver; `contained_saved_psi_is_admitted_for_the_interpreter_and_refused_native_code` |
| Contained interpretation: `TerminalProgram::admit_contained`, `TerminalRuntimeContainment` | Takes `&ContainedTerminalArtifact` from `admit_contained_interpretation`, without producer trust. See [Contained interpretation](#contained-interpretation). | `a_contained_guest_needs_an_account_and_contained_adapters`, `an_untrusted_artifact_runs_only_with_containable_grants` |

Authorization replays the checker's execution-only judgments
(`prepare_received_execution`) and charges them to the same load budget, so a
received module that the checker would refuse never executes (#6460, #6462).

## Contained today

| Property | How it holds | Evidence |
| --- | --- | --- |
| Denied host access | An instance refuses to run with any unbound required import or external completion (`TerminalInstanceError::UnboundImports`, `UnboundExternalCompletions`). A boundary call to an unbound slot is an effect rejection, not a host call. Filesystem and clock bindings admit only the services the supply names. | `embedding/program.rs` `invoke`, `ImportDispatch::adapter_at`; `a_filesystem_name_without_its_core_schema_supplies_no_authority`, `modeled_stream_supply_does_not_authorize_native_descriptor_namespace`, `authorized_native_route_translates_only_guest_namespace_descriptors` |
| Forged handles | Guest file descriptors are indexes into the instance's private table and fail with `EBADF` outside it. Native loans name an original home by era and ordinal and are refused for another incarnation. A forged provider frontier is refused before the handler runs. | `host/native-worker/src/private_filesystem.rs`; `pointer_links_use_worker_homes_and_reject_foreign_incarnations`; `ordinary_dispatch_refuses_missing_and_forged_provider_frontiers_before_the_handler` |
| Forged facts and qualifications | Received facts are not trusted: authorization replays structural memberships, external-completion providers and dynamic view results. Host-supplied access-view referents are admitted per instantiation. Actual values are still checked against their restrictions on every store and read. | #6460; `validate_access_view_referent`; `resident_primitive_and_packed_array_recheck_restrictions_on_every_read` |
| Stale and cross-instance access | Provider storage crosses an instance boundary as copied bytes with an extent and profile, charged to the receiving account, not as a handle. Stale resident reads are refused. Each instance owns its execution state. | `memory/providers.rs` `bind_provider_storage`; `resident_reference_reads_live_contents_and_refuses_shared_writes_and_stale_reads`, `live_reference_owner_keeps_stale_resident_reads_from_reaching_the_storage_handler`; `a_paused_instance_keeps_its_activation_until_it_settles` |
| Provider permissions | Region supplies must cover exact bytes and operations, and structural permissions do not relax scalar transfer limits. | `interpreter/tests/storage_access_policy.rs` |
| Quotas | Provider storage reserves from the invocation's `EvaluationInvocationAccount` when the host supplies one. Hosted output is capped by `TerminalOutputLimits`. A host's refusal for exhaustion is reported through `TerminalInstanceError::sponsor_exhaustion`. | `bind_provider_storage`; `registered_output_ceiling_stops_only_the_exhausted_invocation` |
| Work exhaustion | A work-budget pause returns `SponsorExhausted` with the activation intact. `close` refuses `Busy`, and resuming continues the same activation. Completed effects are not repeated. | `a_paused_instance_keeps_its_activation_until_it_settles`; `entry_results_capture_successful_operation_once_across_work_pauses`; `an_exhausted_work_budget_reports_an_unfinished_execution` |
| Faults | An interpreter error is recorded as the execution's failure. Every later resume returns that failure without running guest code. | `TerminalExecution::resume_with_components` (`self.failure`) |
| Closing a settled instance | `close` ends entry, observation, provider-storage return and binding. Closing a paused or waiting instance is `Busy` and keeps custody. | `a_paused_instance_keeps_its_activation_until_it_settles` |
| Loading | One host budget covers parsing, validation and execution preparation, and exhaustion has its own reason. | #6462 |

## Discard

The sequence in [Closing and failure](../../spec/build/embedding.md#closing-and-failure)
is `TerminalInstance::discard`.

| Step | How it holds | Evidence |
| --- | --- | --- |
| Prevent resumption and new guest-originated host calls | A discarded instance refuses every `invoke` with `TerminalInstanceError::Discarded` and refuses new bindings. Adapters are reached only to cancel and settle. | `a_late_completion_after_discard_settles_without_resuming_the_guest`, `a_discarded_paused_instance_never_resumes` |
| Request cancellation | The waiting call's adapter gets `TerminalEffectHandler::request_effect_cancellation`. A request releases nothing unless the provider confirms it. A pending component invocation discards its provider instance the same way. | `a_confirmed_cancellation_settles_the_discard_at_once`, `discarding_a_consumer_mid_call_settles_its_component_providers_host_operation` |
| Reclaim only disposable interpreter storage | The execution is dropped except for the waiting call, which keeps its argument buffers and any provider homes they name. | `an_uncooperative_provider_keeps_its_storage_retained_and_charged` (the rest of the guest's charge is released) |
| Retain provider-held storage; consume late completions | Each later `discard` calls `settle_abandoned_effect`. Settlement never handles the effect or resumes the guest. Storage the provider reports retained is charged to the invocation account until it settles, and `close` is `Busy` until then. A provider refusal settles nothing. | `a_late_completion_after_discard_settles_without_resuming_the_guest`, `an_uncooperative_provider_keeps_its_storage_retained_and_charged`, `a_faulted_guest_keeps_its_provider_operation_until_it_settles` |
| Unreaped native provider | Dropping a `NativeProviderProcess` that cannot be reaped moves its process, session and homes to its thread's quarantine instead of releasing them. `native_provider_quarantine` reports the retained bytes, and `settle_native_provider_quarantine` releases an incarnation only after reaping it. The runtime host settles it before starting a new provider. | `an_unreaped_provider_keeps_its_homes_retained_until_it_is_reaped` |

The component control publishes a caller and a separate provider from Omega
source. The provider parks on its own host import. A host wakeup lets the
caller return normally. If the caller is discarded first, the host receives
one cancellation request and its retained storage stays charged until it
settles. The wait has an external wakeup and supplies no evidence of an
internal synchronization cycle. See
[`discarding_a_caller_settles_its_parked_component_without_resuming_either_guest`](../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src/tests/operational_effects/guest_discard/component.rs).

The settlement defaults suit a polling adapter: with no contract of its own, an
adapter has settled when it reports the readiness and completion that
resumption would have consumed.

## Contained interpretation

| Rule | How it holds | Evidence |
| --- | --- | --- |
| Admission without producer trust | `psi_authorization::admit_contained_interpretation` replays the same execution-only judgments under the artifact's load budget and returns a `ContainedTerminalArtifact`. Its inner artifact still reports `requires_host_trust`, so `TerminalProgram::borrow`, `retain` and native verification refuse it. `TerminalProgram::borrow_admitted` accepts it only as a contained program. Only the authorization crate may call the codec's hook. | `contained_admission_grants_interpretation_without_producer_trust`, `only_authorization_records_host_acceptance` |
| No native lowering or fact-driven optimization | Native lowering and optimization take `AuthorizedTerminalArtifact`, which contained admission never makes. Runtime admission stops before both (see [Contained runtime admission](#contained-runtime-admission)). | `contained_saved_psi_is_admitted_for_the_interpreter_and_refused_native_code` |
| Bounded storage | A contained program instantiates only with an `EvaluationInvocationAccount`. | `a_contained_guest_needs_an_account_and_contained_adapters` |
| Bindings enforce dynamically or are unavailable | `TerminalInstance::bind_import` refuses an adapter whose `serves_contained_execution` is false (`UncontainedAdapter`), and component bindings are refused. | `a_contained_guest_needs_an_account_and_contained_adapters` |
| Runtime host grants | `TerminalRuntimeContainment` runs the named contained artifact under its account. It needs output limits and refuses native providers, native services and filesystem delegation other than standard streams alone. | `an_untrusted_artifact_runs_only_with_containable_grants` |

The runtime host's bindings, reviewed one by one:

| Binding | Contained | Why |
| --- | --- | --- |
| Numeric, absolute value, affine transform, integer division, integer parsing, network address, descriptor mode | Yes | Safe Rust over copied values and invocation-private state. Nothing outlives the call or reaches the host. |
| Buffered output | With output limits | Output reaches only the invocation's capped buffers. |
| Descriptor I/O | Without filesystem delegation or with standard streams alone, with output limits | Finite replay input and capped private output. A standard-streams namespace answers every other descriptor with `EBADF`. Delegated descriptors reach the host kernel. |
| Clock | Snapshot, or live time under a granted wait ceiling | Snapshot time is private and modeled. `TerminalClockEnvironment::HostWithin` refuses any wait past what remains of its ceiling before it sleeps; unbounded `Host` time does not serve. `a_live_wait_ceiling_refuses_waits_past_what_remains` |
| Allocation emulation | Yes, charged to the invocation account | Each `calloc` reserves one cell and its bytes on the contained guest's account, and returns null when the account cannot cover it. `free` returns the bytes; the cell stays because addresses never recycle. `a_charged_ledger_bounds_live_allocation_by_its_account` |
| Linux filesystem, filesystem delegation | No | Kernel calls on the host's namespace. |
| Native provider | No | Runs foreign native code. |

## Contained runtime admission

Runtime admission runs over a contained artifact, so the interpreter gets the
host's reviewed realizations for saved Psi it does not trust. Jarod chose this
over a separate contained settlement planner, which would duplicate provider
selection and target binding. The stages that runtime admission shares with
native compilation take either admission. Everything after it requires
producer trust.

| Rule | How it holds | Evidence |
| --- | --- | --- |
| One admission type for the stages runtime admission shares with native compilation | Stages 00 to 02, realization admission and runtime provision planning take `psi_authorization::AdmittedTerminalArtifact`, either authorized or contained. Target folding re-admits a folded contained artifact as contained. | `psi-to-native/driver/inputs/published_application.rs`, stage 01 `published_application.rs` |
| No optimizer | Stage 04 `admit_abstract_body` refuses a contained artifact with `ProducerTrustRequired`. | `abstract_input.rs` |
| No image, components or foreign adapters | `produce` refuses exports and suppliers for a contained program, gives it no native foreign adapter, and requires the authorized form before image planning. `compiler::CompileRequest::Native` refuses `Program::ContainedPsi`. | `contained_saved_psi_is_admitted_for_the_interpreter_and_refused_native_code` |
| Compile-time evaluation stays contained | Calling policies and scalar conditions evaluated during runtime admission use `TerminalProgram::borrow_admitted`, so a contained artifact's own code runs as a contained program under an invocation account. | `a_contained_guest_needs_an_account_and_contained_adapters` |
| CLI grants | `omega run app.psi` runs with the host's reviewed package bindings, standard streams only, no native provider or services, a live-wait ceiling and output ceilings. Explicit run directory mappings are refused until a confined filesystem binding is audited. | `saved_psi_without_producer_trust_runs_contained` |

## Remaining

1. Apply the remaining
   [standalone Product grants](../../spec/build/command_line.md#runtime-directory-mappings)
   to bare `omega run app.psi`: the cwd read/write mount at `/` by default,
   replaced by explicit mappings. Live networking and program launch are
   already unavailable on the contained route.
2. Audit a confined filesystem binding for contained execution. Exercise the
   read/write cwd mount, replacement by explicit mappings, symlink confinement
   and ordinary filesystem failures. The audited route above refuses filesystem
   delegation and native providers.
