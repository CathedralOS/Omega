# Psi containment audit

This audit is step 5 of [Psi admission and interpreter preparation](psi_claim_admission.md#implementation-work).
It checks the interpreter, embedding and runtime host against the containment
rules in [Ownership and lifetime](../../spec/build/embedding.md#ownership-and-lifetime),
[Execution and interference](../../spec/build/embedding.md#execution-and-interference)
and [Closing and failure](../../spec/build/embedding.md#closing-and-failure). It
was made on `main` after #6468.

**Verdict: do not open untrusted execution yet.** Every route that runs Psi
still requires the host's trust in the exact artifact and producer, so the gaps
below are not reachable by an untrusted producer today. Execution of a trusted
artifact is contained for denied host access, forged handles, stale access,
work exhaustion and faults. The discard sequence for a faulted, stopped or
waiting guest does not exist yet. Until it does, an instance that has not
settled can only be kept or dropped. It cannot be safely abandoned.

## Routes into execution

| Route | Gate | Evidence |
| --- | --- | --- |
| `omega run <program.psi>` and saved programs | `--trusted-artifact <manifest-id>` names the host-accepted artifact. `open_trusted_program` decodes under `PsiLoadLimits` and authorizes. | `application/src/saved_program/input.rs`; #6458, #6462 |
| `omega psi inspect --artifact` | Inspection only. Decoding runs under the input cap and work budget and never executes. | `application/src/inspection/mod.rs`; `named_psi_inspection` tests |
| Embedding: `TerminalProgram::admit`, `retain_authorized`, `TerminalExecution::start_artifact` | Each takes `&AuthorizedTerminalArtifact`. `borrow` and `retain` refuse received code. Only `authorize_terminal_artifact` makes an executable copy. | `received_artifacts_execute_only_with_host_trust_in_their_exact_bytes`, `only_authorization_records_host_acceptance` |
| Runtime host: `HostedTerminalProgram::prepare`, component preparation | Refuses an unauthorized received artifact with `InvalidProvision`. | `interpreter/host/src/execution.rs`, `execution/components.rs` |
| Native lowering | Requires `AuthorizedTerminalArtifact`. | `psi-to-native` driver inputs |

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

## Gaps before untrusted execution

These are the parts of [Closing and failure](../../spec/build/embedding.md#closing-and-failure)
that have no implementation. Each needs one before untrusted execution opens.

1. **No discard operation.** `TerminalInstance::close` treats any invoked,
   unfinished execution as `Busy`, and a faulted execution counts as
   unfinished. The only other way out is dropping the instance, and a
   host-language destructor does not establish safe cancellation. A discard
   entry must take a faulted, stopped or waiting instance through the
   four-step sequence: prevent resumption, request cancellation, reclaim only
   disposable interpreter storage, retain provider-held storage.
2. **No cancellation request.** Neither `TerminalImportAdapter` nor component
   bindings have a cancellation operation, so outstanding provider operations
   (`AwaitingEffect`, `HostComputationPending`, pending component invocations)
   cannot be asked to stop under their contracts.
3. **Late completions have no settlement path.** A completion that arrives
   after discard must be consumed for settlement and cleanup without resuming
   the guest. Today the only consumer of a completion is resumption.
4. **Provider-held storage is not retained and charged after abandonment.**
   `NativeProviderProcess::drop` releases its home directory only after it reaps
   the provider, but the rest of the process state is still dropped, and nothing
   charges a still-pinned provider buffer to the responsible host context. The
   spec requires an uncooperative provider's storage to stay retained and
   accounted.
5. **No test of late completion after discard or of an uncooperative
   provider.** The acceptance list names both. They depend on gaps 1 to 4.

Gaps 1 to 4 are one protocol: a discard state on `TerminalInstance`, a
cancellation operation on adapters and component bindings, and a custody
ledger that outlives the guest until each outstanding obligation settles. No
spec change is needed. The spec already states the sequence; it does not yet
name the embedding API that carries it.
