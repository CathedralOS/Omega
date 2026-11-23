# Record: unit_064 (area 04-source-stages-06-07)

Base path `P06` = `source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/src`.
All files are in the stage 06 crate (the lowered-form checker). Everything below was read, not run. Counts come from the
Grep tool (content or count mode) over `P06/src` unless stated. "Not grepped" means I did not count callers.

Two files that `places.rs` depends on are outside this unit: `places/log.rs` (the write log: `stores`, `log_is_stated`,
`index_reaching_writes`, `reaching_writes`, `Logged`, `Stored`) and `places/loans.rs` (loan judgment, `borrowed_resource`,
`reference_graph*`, `provider_referent`). I cite their names only as callees; I did not read them.

---

## P06/lib.rs (2,750 lines, read in full)

1. Purpose. The crate root: module list, result types (`Evidence`, `Discharged`, `MachineDischarge`, `Discharge`), the
   per-machine driver (`discharge_inner`, `machine`, `check_prepared_machine`), the knowledge filler (`fill_knowledge`), the
   goal statement per obligation kind (`stated_goal`), the step that asks the prover for a certificate and the kernel to accept
   it (`decide`, `search_certificate`, `verify_certificate`), and clause substitution helpers.
2. Shape.
   - `Evidence` (`lib.rs:152`), `Discharged` (`lib.rs:175`: `Certificate`, `TargetCondition`, `EveryExit`,
     `PendingNormalGuarantee`, `Window`, `Preserved`, `Lent`, `FiniteIntegerRanking`), `WindowUse` (`:251`), `StorageUse`
     (`:271`), `MachineDischarge` (`:282`), `ImpossibleArm` (`:313`), `Discharge` (`:329`).
   - Functions: `discharge` (`:409`), `discharge_inner` (`:435`), `machine` (`:683`), `check_prepared_machine` (`:733`),
     `fill_knowledge` (`:1145`), `with_prepared_machine` (`:1451`), `published` (`:1564`), `result_equation` (`:1653`),
     `substitute_goal` (`:1803`), `erased_actuals` (`:1876`), `ErasedTerms` (`:1917`), `ScalarActuals` (`:2017`), `stated_goal`
     (`:2153`), `concrete` (`:2129`), `decide` (`:2304`), `connected` (`:2360`), `hypotheses` (`:2515`), `verify` (`:2566`),
     `derive_contract_obligation` (`:2694`), `Unreached` (`:604`), `Unbound` (`:1724`).
3. Inputs and outputs. `discharge_inner` takes `&mut Elaboration`, carried proofs and optional verified borrow ends. It
   appends knowledge rows (`push_knowledge`, `lib.rs:1606`) and returns one `MachineDischarge` per machine.
4. How it decides.
   - Per obligation, keyed on `ObligationKind` (`lib.rs:828-916`, `2168-2298`). Loans, windows, rankings, published guarantees
     are structural verdicts; the rest go to `decide`, which asks `prove::Prover` for a certificate and the kernel
     (`verify_obligation`, `lib.rs:2603`) to accept it.
   - Which knowledge an obligation sees: rows whose `available_from` dominates the obligation's point (`hypotheses`,
     `lib.rs:2526-2528`), split into axioms and assumptions by `Justification` (`:2539-2553`). Keyed on graph dominance.
   - `connected` (`:2360`) narrows to hypotheses sharing a `ValueId` with the goal (union-find); on failure the full set is
     retried (`:2334`).
   - A DataDomain row that is `Unproved` and whose operation stores or zero-establishes becomes `Window` or `Preserved`
     instead of a refusal (`lib.rs:1004-1022`); "untouched" is a place-overlap test (`Storage::untouched`, `places.rs:1836`).
   - A bodyless requirement: every obligation is `Err(Unsupported)` (`lib.rs:549-565`).
   - `published` (`:1564`) decides a published ensures clause by finding, per `Return` edge, the `NormalGuarantee` row by
     equality of its `ObligationKind` (`:1584-1591`).
5. Cases by shape. Header: "Kinds outside those five are refused as `Unsupported`" (`lib.rs:50`). `stated_goal` refuses `Claim`,
   `Loan`, `RestorationWindow`, `DomainDebt`, `EstablishmentDebt`, `WellFoundedness` as "obligation kind outside this route"
   (`:2287-2297`). `erased_actuals` refuses any callee with an erased proof formal; erased scalar actuals only
   `Value`/`Integer`/`Boolean` (`:1883-1908`). `ScalarActuals::field` and `case` refuse as "outside the scalar subset" (`:2066-2087`).
6. Same job elsewhere. `substitute_goal(` has 5 hits in `P06/src` (3 in `lib.rs`, 2 in `joins.rs`; Grep count mode).
   `derive_contract_obligation` is called at `external_providers.rs:188`, `refinement.rs:183`, `static_contracts.rs:768`
   (3 production sites) plus 1 test (`tests/callable_contracts.rs:331`).
7. Unused / one-caller. `Unreached` is used by `lib.rs`, `entry_inputs.rs`, `entry_inputs/recursive.rs`, `formation.rs`,
   `result_summaries.rs`, `prove/reach.rs`, `joins.rs` (comment) (grep above). `discharge_carrying` and `prepare` are
   test-support only (`lib.rs:426`, `:1409`). `ImpossibleArm` constructor sites: not grepped.
8. Stated limits. "Kinds outside those five are refused as `Unsupported`" (`:50`). "no proof rule is yet marked as holding at
   every type" (`:2125`), so any goal or hypothesis naming a type-parameter value is refused (`concrete`, `:2129-2150`).
9. Fail-open risks.
   - `entry_snapshot_conjuncts`/`storage_conjuncts` (`lib.rs:1761-1789`): a premise part that is `Unbound::Unproved` is kept
     as `Some(unread.clone())` (entry closure `:1765`), i.e. left unsubstituted. Input: a `requires` part naming a field with no
     value identity. Comment `:1758-1760` explains ("An unread field still names its original entry snapshot"). "stated".
     Other callers of `storage_conjuncts` not grepped.
   - `substitute_goal` / `goal_from_observed_parts` (`:1803-1861`) drop disjuncts it cannot state as false; "stated"
     (`:1794-1802`). This only strengthens a goal.
   - `returned_for_result` (`:2644`) leaves other proposition forms unchanged: "never wrongly accepted" (`:2640-2643`). "stated".
   - `published` accepts with an empty exit list when there is no return edge (`:1603`); "stated" vacuously
     (`:1559-1563`).
   - `TargetCondition` verdicts carry no certificate (`:855-876`); acceptance relies on the later target closure
     (`lib.rs:5-7`). "stated".
   - `Preserved` accepts a store with no certificate when `untouched` finds no overlap (`:1009`). "traced" to
     `Storage::untouched` (`places.rs:1836-1880`): an `Unsupported` clause returns `false` (`places.rs:1857`), so unknown
     clauses are not preserved.

## P06/machine_batch.rs (127 lines, read in full)

1. Purpose. Runs one machine check per machine, in parallel with rayon when more than one worker exists.
2. Shape. `check<T>` (`machine_batch.rs:11`); one test (`:78`).
3. I/O. `&mut IdTable<MachineId, MachineTables>` and a closure in; `IdTable<MachineId, T>` out.
4. Decision. Jobs with `id > first_failure` are skipped (`:38`); the returned refusal is the lowest failing id (`:55-65`).
5. Cases. Serial when `current_workers() == 1 || tables.len() < 2` (`:22`).
6. Same job elsewhere. None seen.
7. Callers. `machine_batch::check`: 1 production site (`lib.rs:544`).
8. Limits. None.
9. Fail-open. A skipped job is turned into an `Unsupported` refusal (`:56-65`). None seen.

## P06/machine_inputs.rs (198 lines, read in full)

1. Purpose. Builds, once, per machine: authored branches (for warnings), the first matching carried-proof offer, and whether
   the machine is a bodyless requirement.
2. Shape. `MachineInputs` (`:13`), `PreparedMachineInputs::new` (`:26`), `for_machine` (`:85`); two tests.
3. I/O. `MachineSources`, `MachineTables`, requirement ids, `Carried` offers in; table indexed by `MachineId` out.
4. Decision. Carry association is keyed on `offered.specialization == machine id`; first offer wins (`:69-76`).
5. Cases. None.
6. Same job elsewhere. None seen.
7. Callers. Built once (`lib.rs:531`).
8. Limits. None.
9. Fail-open. An offer or requirement naming no machine is silently ignored (`:51`, `:60`, `:69`). Comment `:72-75`: "the
   carrier still checks its actual proof goals". "stated".

## P06/nominal_calls.rs (185 lines, read in full)

1. Purpose. Checks that nominal-type call correspondence rows (caller, operation, argument ordinal) match the actual call
   operations and callee formals.
2. Shape. `validate(&Elaboration)` (`:7`).
3. I/O. Reads `module`, `nominal_types`, `nominal_calls`, `value_parameters`; returns `Checked<()>`; no mutation.
4. Decision. Rows must be strictly ordered by (caller, operation, argument) (`:45-55`). When the callee formal type is in
   `nominal_types` (`required`, `:114-117`) a row must exist (`:118-125`). The row must agree on callee, expected type, actual
   type (by place path), access, const-argument positions, type-description text validity (`valid_key`, `:81-85`, calls
   `application_identity::type_identity::validate_source_type_description`, a text parse) and
   `terminal_semantics::calls::nominal_argument_matches` (`:148-166`). Final count check `observed != rows.len()` (`:176`).
5. Cases. Early `Ok(())` when both rosters are empty (`:8-10`).
6. Same job elsewhere. `minted.rs:176-231` walks the same caller/operation/argument structure for array extents.
7. Callers. `nominal_calls::validate`: `formation.rs:37`, `verdict.rs:995` (2 production sites).
8. Limits. None.
9. Fail-open. A call input with no row and `!required` is skipped (`:118-127`). A const-dependent type that elaboration did
   not list in `nominal_types` would not be caught here; `nominal_types` is only checked for order and for naming a
   const-dependent formal (`:33-44`, `:56-80`). Producer of the roster not read. "open".

## P06/operational.rs (440 lines, read in full)

1. Purpose. Checks each call's possible suspend/block against its caller's ceilings and the source's acknowledgements, and
   checks that the module's suspension crossing roster equals what this checker reconstructs.
2. Shape. `Call` (`:19`), `calls` (`:47`), `judge` (`:154`), `crossings_are_stated` (`:354`).
3. I/O. Reads `Elaboration`; no mutation.
4. Decision. Fixed point over machines of `[suspends, blocks]` (declared flags fixed, omitted start false) (`:167-246`);
   published flags must equal the inferred ones (`:250-256`); per call, an authored suspend ack outside a suspension
   position is a violation (`:287-294`); a call flag with a false ceiling or missing ack is a violation (`:295-313`).
5. Cases. "a possibly suspending terminal call needs a checked suspension crossing" (`:333`): any possibly suspending call at a
   terminator or frame-reusing jump is refused (`:314-335`).
6. Same job elsewhere. Uses `call_contracts::inputs_at`, `terminator_site`, `TailCallSites` (defined outside the unit).
7. Callers. `operational::judge`: 1 production site (`lib.rs:483`), many tests.
8. Limits. `:333` above.
9. Fail-open. None seen (`unwrap_or(false)` at `:171` is the start of the fixed point).

## P06/owned_cleanup.rs (663 lines, read in full)

1. Purpose. Source-side "materialization" of invocation rows, requirement obligations and write rows for generated
   owned-cleanup actions; plus the cleanup write inventory and edge entry points the checker uses.
2. Shape. `Catalog` (`:14`), `materialize` (`:151`), `Write` (`:344`), `writes` (`:355`), `materialize_writes` (`:555`),
   `edge_entry_points` (`:624`), `exact_place` (`:96`), `inputs` (`:54`).
3. I/O. `materialize` mutates `Elaboration`: pushes `InvocationRow`, `ObligationRow`, source rows, sets
   `requirement_obligations` on the action, and writes rows into `tables.writes`.
4. Decision. Existing invocation row: exactly one is left alone, more is a refusal (`:226-241`); requirement obligations
   without an invocation row are refused (`:242-249`); targets with erased formals refused (`:182-191`). Write ceiling: for each
   non-shared reference leaf, a write is logged only if `reference_source_projection_accessible` says the access is reachable
   (`:518-527`); `unknown_contents` of kind `Value` refuses (`:451-457`); view payload refuses (`:438-442`).
5. Cases. Only `TerminalAffineCleanupAction::InvokeOwned` is handled (`:170`, `:364-372`). "owned cleanup of view payload
   needs its exact write correspondence" (`:440`).
6. Same job elsewhere. `minted.rs:1001-1033` re-checks the same cleanup rows (the receiving side).
7. Callers. `owned_cleanup::materialize`: `verdict.rs:997` plus 1 test; `Catalog::new`: `minted.rs:758`, in-file; `writes`:
   `places.rs:1180` and in-file; `edge_entry_points`: `lib.rs:1156`; `exact_place`: `minted.rs:551` (all read).
8. Limits. "A partially supplied invocation is never repaired" (`:148-150`); "needs its explicit erased argument
   correspondence" (`:189`).
9. Fail-open. In `writes`, a reference leaf whose projection is not accessible for the carried access is skipped with
   `continue` (`:518-527`); no write row is logged for it. Wrong input: a mutable leaf that `terminal_semantics` reports
   inaccessible yet which is writable. No reason at the site. "open".

## P06/minted.rs (1,461 lines, read in full)

1. Purpose. Before any proof, recomputes from the lowered form what obligations, invocation rows, edge rows and value rows each
   machine owes, and refuses tables that differ (`ForeignIdentity`, `:45-47`).
2. Shape. `Minted` (`:407`: `owed`, `claim`, `call`, `returns`), `follow_the_lowered_form` (`:722`),
   `width_questions_are_minted` (`:125`), `check_pending_operands` (`:1134`), `values_are_declared` (`:1261`),
   `scalar_formals_with_origins` (`:1188`), `domains_are_declared` (`:1373`), `contains_parameter_extent` (`:74`), `successors` (`:381`).
3. I/O. Reads lowered machine, tables, module, `Inhabitation`; pure checks.
4. Decision. Claim-and-count: each owed row must be exactly the expected `ObligationKind`, created at the expected
   `ProgramPoint`, claimed once (`claim`, `:469-507`); unclaimed rows of listed kinds are refused (`:1088-1103`). `owed` finds
   the row by kind equality, first match (`:430-435`). Keyed on identities, kinds, program points.
5. Cases. Obligation kinds `Claim|Loan|RestorationWindow|DomainDebt|EstablishmentDebt` skipped here (`:1104-1110`). A width
   subject must be a `u64` proof-static input (`:338-349`). Operations that owe an index/bounds obligation are an explicit
   match list: `ByteSequenceRead`, `StructuralByteSequenceFieldRead`, `ByteSequenceWrite`, `ElementViewRead`,
   `ByteSequenceSubslice`, `ElementViewSubslice`, `EstablishElementView` with `WholeRepresentation` (`:917-939`).
6. Same job elsewhere. `Storage::owed_are_minted` (`places.rs:1404`) does it for `DataDomain` rows.
7. Callers. `follow_the_lowered_form`, `domains_are_declared`, `check_pending_operands`: `lib.rs:1469-1472`;
   `width_questions_are_minted`: `lib.rs:482`.
8. Limits. "refused where they are decided (`goal`)" (`:1104`).
9. Fail-open.
   - Edge rows no terminator owns are not refused; "stated" (`:804-808`: "it can only add a way into its destination").
   - `Minted::call`: when `call_frontier(site)` is `None` the call must be `call_is_unreachable(site)` (`:587-636`);
     an unreachable call needs no claim frontier. Reason decided in `plain_affine`, not here. "open".
   - A type with no `domains` row is treated as stating no clauses (`:1395`); "stated" (`:1370-1371`).
   - `contains_parameter_extent` is conservative (`true` for a missing declaration) unless an authenticated abstract referent
     exists (`:86-97`); a test pins it (`:1409`).

## P06/places.rs (9,440 lines, read in full)

Header (`places.rs:1-73`): one walk (`Storage::reaching`) over stores derived from lowered operations (`log`), the one
overlap relation, and the lowered graph. Lines 8283-9440 are an in-file test module (`observation_tests`); everything
before it is production code. The write log itself (`log` module) is outside this unit.

1. Purpose. `Storage` answers "what does this scalar leaf or sum hold when this operation uses it" for reads, copies, the
   initialization judgment, data-type clause goals, invariant windows and call/return/join clause bindings.
2. Shape.
   - Types: `Lineage` (`:126`), `StorageAddresses` (`:133`), `Reached` (`:457`), `BoundaryDepth` (`:556`), `Supply` (`:582`),
     `Reaching` (`:672`: equation graph, `keys`, `crossed_entries`, `selector_entries`, `used`, `remaining` budget), `Shape`
     (`:799`), `Asked` (`:815`: `Contents`, `Case`, `Window`), `Owner` (`:914`), `Window` (`:939`), `Consumption` (`:956`),
     `Touching` (`:969`), `Touch` (`:999`), `Storage` (`:1041`), `Held` (`:6167`), `Named` (`:8116`), `Observed` (`:8225`),
     `Returned` (`:6325`), `AtEntry` (`:6597`), `AtCall` (`:6782`), `CallObservations` (`:6972`), `AtJoin` (`:7506`).
   - Walk: `overlap` (`:201`), `Storage::new` (`:1071`), `reaching` (`:3583`), `reaching_supply` (`:3695`),
     `reaching_backing_supply` (`:3790`), `supplied` (`:2901`), `case_on_edge` (`:3310`), `membership_test` (`:3239`),
     `call_preserves_extent` (`:3410`), `window_handed_off` (`:3514`), `Reaching::solve` (`:754`).
   - Judgments: `uses` (`:5843`), `judged` (`:4883`), `roster` (`:4972`), `touches` (`:5612`), `consumes` (`:6018`),
     `consumptions` (`:6110`), `open_at` (`:5921`), `open_before` (`:5947`), `byte_predicate_establishments` (`:5047`).
   - Knowledge: `read_equation` (`:4151`), `read_premises` (`:4217`), `premises` (`:4285`), `payload_equations` (`:4466`),
     `payload_premises` (`:4516`), `domain_goal` (`:1809`), `safety_goal` (`:2571`), `collection_index_goal` (`:2726`),
     `subslice_predicate_premises` (`:5516`).
3. I/O. Reads the lowered machine/module, `MachineTables` (values, places, roots, obligations, writes), `Graph`, and
   `DescriptorAnalysis` from `terminal_semantics`. Builds the write log in `Storage::new` (`:1178`), checks the tables' log and
   cleanup log against it (`:1181-1182`), then re-captures each write's backing (`:1185-1206`). Output: `Reached` answers,
   `Proposition` goals and premises, `StorageUse`/`Consumption` verdicts. No table mutation after `new`.
4. How it decides.
   - `Reached::merge` (`:498-509`): `Unestablished` and `Open` dominate; two different `Value`s give `Unknown`; `Cycle` is neutral.
   - `Asked::ended_by` (`:877-908`) keyed on place overlap and operation id: contents end on any overlapping write; case ends
     unless the write is strictly below; a window ends on the opening operation, a covering write, or a restoring write.
   - `supplied` (`:2901-3234`) chooses what the last ending log row supplies by `Stored` variant (`Value`, `Case`, `Zeroed`,
     `Established`, `Vacated`, `Moved`, `UnknownPayloadWrite`). `Moved` follows the source at the transfer's prestate
     (`:3173-3226`).
   - Entry rule (`:4018-4055`): a machine formal's contents are `Established(depth)` or its entry snapshot (`Boundary`);
     `BlockParameter`, `Result`, `OperationResult`, `ByteSequenceLiteral`, `ProviderAttachment`, `TrivialAffineLocal` roots give
     `Unestablished`; a window at entry is `Closed`.
   - Case facts (`case_on_edge`, `:3310-3401`): a dispatch edge into a block establishes its case unless several edges of the
     dispatch enter the block (folded with `merge`, `:3331-3335`); a conditional edge uses a membership test found by
     `membership_test` in the same block with no later retiring write.
   - `judged` (`:4883`): every leaf at or beneath each used place must not be `Reached::Unestablished` (`:4916-4931`); a `Case`
     step in a named place requires `case_at == Some(true)` (`:4953`).
   - Windows: `consumes` (`:6018`): a release or any call/return consumes every non-initial window open there; otherwise the
     use must overlap the window's backing under `Whole`, or a relied leaf must overlap.
   - Byte predicates: `byte_source_with` (`:5369`) keyed on the place's root kind; literal bytes evaluate the predicate
     (`:5409`); formals need a shared readable byte view; block parameters and call/`CallMachine`/subslice results are
     `Boolean(true)` when `byte_sequence_source_predicates` says the declared sources imply it (`:5444`, `:5477`).
   - Selectors (`index_segment`, `:6222-6289`): a literal selector becomes `FixedIndex` only inside a known literal extent;
     out of range refuses; otherwise a `RuntimeIndex{selector}` with the value's own identity.
5. Cases by shape (quoted).
   - `field_shape`: "a field type outside plain data" (`:1991`). `root_type`: `ProviderAttachment`/`TrivialAffineLocal`:
     "a root place kind outside this route" (`:1967-1969`).
   - `leaves_of`: "storage that is no plain data" (`:2406`); a literal array of length > 1 of primitive scalars may be established
     whole (`:2376-2384`).
   - `shape_with`: fixed byte index only on `BoundedView` (`:2206-2214`); index only into literal arrays, parameter-extent
     arrays with runtime index, or `ElementView` with runtime index: "an index needs a closed array or a live view bound"
     (`:2253-2258`).
   - `operation_uses`: `StructuralCaseLeafCopy` is "a case leaf copy" `Unsupported` (`:4608-4610`); other operations read
     "by the lowered form's own accessor" (`:4611-4617`).
   - `touches`: "storage passed on a conditional edge" `Unsupported` (`:5792-5799`); `Crash` and `PendingNormalCompletion` consume
     nothing (`:5696`, `:5805-5808`); external completion consumes nothing (`:5765-5770`).
   - `roster`: "storage passed on a dispatch edge" `Unsupported` (`:4993-4995`).
   - `provider_record_leaf`: scalar leaves only `bool`, `f32`, `f64`, or non-address integers of 8-64 bits in whole bytes
     (`:3653-3666`).
   - `CallObservations::field_crossings`: `const MAXIMUM_FIELD_CROSSINGS: usize = 256` (`:7077`), records and `Integer`/`Boolean`
     fields only (`:7096-7105`); for inference only ("every arrival still proves the resulting condition", `:7072-7074`).
   - `byte_literal_establishment`, `predicate_view_sources`: only `EstablishRecord`, `EstablishStructuralCase`,
     `StoreStructuralField`, `StructuralByteViewFieldStore` (`:5228-5285`).
6. Same job elsewhere.
   - `Storage::at_call` (`:1477`) and `at_cleanup_call` (`places/cleanup.rs:12`) build the same `AtCall`.
   - `Storage::backing` (`places/backing.rs:699`) and `cleanup_backing` (`places/cleanup.rs:54`) repeat the split/prefix/suffix code.
   - `Returned`/`AtEntry`/`AtCall`/`AtJoin`/`Held` each implement `Bindings` with near-identical `field`/`extent`/`byte_predicate`
     bodies (`:6417`, `:6650`, `:7386`, `:7576`, `:8032`).
   - `owed_are_minted` vs `minted.rs`.
7. Unused / one-caller.
   - `content_root_kind` and `content_root` (`:432`, `:440`): `joins.rs:306` and `:389`; 2 external sites (grep).
   - `Storage::arrival` (`:1528`): `entry_inputs.rs:242`, `joins.rs:474`, `joins.rs:886`; `at_join` (`:1567`): `entry_inputs.rs:220,246`,
     `joins.rs:105,489,924`.
   - `preserves_entry_storage` (`:7548`): 1 caller (`joins.rs:958`). `field_crossings` (`:7076`): 4 sites in `entry_inputs.rs`
     (`:236`, `:261`, `:332`). `formal_observation` (`:7143`): 1 caller (`entry_inputs.rs:973`). `open_before` (`:5947`): 1 caller
     (`lib.rs:1047`). `at_cleanup_call`: `lib.rs:1282`, `lib.rs:2219`.
   - `position_of` (`:6157`) returns `usize::MAX` for a missing case and is then used as an index (`:5015`); the case was
     already matched by `position` at `:4986`, so it cannot miss there.
8. Stated limits. `ponytail:` at `:1415` and `:5632` ("one scan of the obligation table per operation"; "one scan of the
   invocation table per operation"), both self-declared linear scans per operation. Comment `:1263-1266`, `:355-358`.
9. Fail-open risks.
   - `Reached` at entry for a formal gives `Established(depth)` with no value identity when observations are off
     (`:4025-4038`): "stated" (`:4022-4024`: "A formal arrives with its declared type established").
   - Provider shared references: `reaching_supply` returns `Established(depth)` for an admitted provider referent
     (`:3724-3755`). "stated" (`:3724-3725`: "The admitted shared reference establishes valid scalar contents"). Who admits it is
     `provider_referent` in `places/loans.rs` (not read).
   - Windows: `window_handed_off` (`:3514-3581`) returns `Reached::Closed` for a window on a block formal when every walked path
     passes the formal whole or never leaves scope (`:3592-3596`). A closed window means no proof is owed at later points.
     "stated" in the long comment (`:3499-3513`); "traced": a dispatch arm binding keeps the ordinary walk (`:3557-3559`).
   - A window at machine entry is `Closed` (`:4021`): "stated" ("A path that never passed the opening write").
   - `byte_predicate_establishments` (`:5047`): `byte_transfer_qualifications_hold` returning `Ok(None)` is accepted
     (`:5084`). Meaning of `None` is in `terminal_semantics`; no reason at the site. "open".
   - `byte_source_with` returns `Boolean(true)` through `establishes_predicate()` (`:5444`, `:5477`), which asks declared
     source predicates (`byte_sequence_source_predicates`, `:5398`). "stated" (`:5321-5322`: "may use only reconstructed source
     facts or its authenticated callee contract"). `byte_read_predicate_premises` has the same shape (`byte_views.rs:149-161`).
   - `AtJoin::field` for a root that is not a destination formal returns an unresolved `IntegerField`/`BooleanField` at the same
     storage version (`:7699-7723`). "stated" (`:7699-7700`: "keeps its existing storage version"); it is a term, not a verdict.
   - `AtJoin::case_predicate` for a non-formal root returns a `StructuralCaseMembership` proposition undecided (`:7845-7850`).
   - `judged` skips the whole-array shortcut when `cleanup_writes` is non-empty (`:4897-4899`) and treats any error from the
     whole-array query as "not established whole" (`:4911`), falling back to per-leaf judgment: fail-closed.
   - `Reaching::solve` + `reaching` turn an unresolved `Cycle` into `Unestablished` (`:3645`), and into `Unknown` for windows
     (`:3644`): fail-closed except that `Unknown` for a window is not `Open`, so a cycle with no real supply does not keep a
     window open. "open".
   - `case_on_edge` returns `None` (no fact) on any mismatch; never an invented case.
   - `entry_type_premises` (contents.rs) and `premises` drop unstatable clauses (`:4413`); fewer premises, not acceptance.

---

## P06/places/arrivals.rs (110 lines, read in full)

1. Purpose. Caches, per block structural parameter, whether it is a borrowed arrival and the argument lineages of its incoming edges.
2. Shape. `Arrival` (`:9`), `is_borrowed_arrival` (`:15`), `visit_incoming_storage` (`:47`).
3. I/O. Reads blocks, graph predecessors, terminator edge arguments; fills `OnceCell`s.
4. Decision. "Borrowed" means access is not `Owned` and type is not a descriptor (`:34-39`). Each incoming argument must cover the
   parameter's access (`:84`) and have exactly its structural type (`:91-99`); a formal with no incoming binding is refused (`:104-106`).
5. Cases. None.
6. Same job elsewhere. None seen.
7. Callers. `is_borrowed_arrival`: used in `backing.rs:707` and `cleanup.rs:66`; `visit_incoming_storage`: `places.rs:3986` (read).
8. Limits. None.
9. Fail-open. `is_borrowed_arrival` returns `Ok(false)` when the block id or position is out of range or the parameter place
   differs (`:21-30`), so `backing` treats the root as plain direct storage. Wrong input: a block formal with an inconsistent declaration.
   "open".

## P06/places/goal_observations.rs (134 lines, read in full)

1. Purpose. `Held::observation_alternatives`: after an indexed store, collects every candidate value (prior or stored operand)
   so a predicate must hold for each.
2. Shape. One method (`:9`).
3. I/O. Reads the same-block write log and backings; returns `Vec<ScalarTerm>`.
4. Decision. Walks back through same-block stores; store and read must each be a single region whose paths differ only where the
   store has `RuntimeIndex` and the read `FixedIndex` (`:75-92`), same scalar carrier (`:106-110`).
5. Cases. "no local indexed store correspondence" (`:50`) for a store from another block; only `Stored::Value` (`:52-56`).
6. Same job elsewhere. `backing.rs::stored_scalar_suffix` (`:557`) handles the ordinary read case.
7. Callers. `observation_alternatives`: `places.rs:8062-8068` (`Held::field_alternatives`), `:6525`, `:7462` (read).
8. Limits. None.
9. Fail-open. None seen; every failure is `Unproved`/`Unsupported`.

## P06/places/backing.rs (857 lines, read in full)

1. Purpose. `Backing`: the set of actual addresses a borrowed place stands for, with containment and overlap tests.
2. Shape. `Backing` (`:19`), `CapturedRegion` (`:9`), `definitely_contains`, `same_single_region`, `projections_from`,
   `region_contains` (`:181`), `relative_suffix` (`:258`); `Storage::backing` (`:699`), `captured_backing` (`:754`),
   `borrowed_transfer_depth` (`:317`), `selected_payload_cell_family` (`:441`), `stored_scalar_suffix` (`:557`); one test (`:797`).
3. I/O. Calls `DescriptorAnalysis::captured_storage_family` (`:739`).
4. Decision. With a runtime index, `region_contains` needs the same `Argument` origin, equal retained captured-address path and
   selectors (`:203-231`). `index_can_change_on_reentry` is set when a capture selector's block is in a cycle and the selector is
   not a machine parameter (`:773-780`).
5. Cases. "reference backing crosses a permission beneath an unresolved selector" (`:722`); non-`Argument` origins refused (`:765-769`);
   `borrowed_transfer_depth` returns 0 unless a list of preconditions all hold (`:339-386`).
6. Same job elsewhere. `places/cleanup.rs::cleanup_backing` (`:54-113`) is a near copy of `Storage::backing` (`:699-750`).
7. Callers. `Storage::backing`: very many in `places.rs`/`byte_*` (not counted).
8. Limits. Comment `:314-316`.
9. Fail-open. `borrowed_transfer_depth` and `stored_scalar_suffix` return `0`/`None` (nothing retained) on failed preconditions:
   fail-closed. None seen.

## P06/places/byte_payloads.rs (581 lines, read in full)

1. Purpose. Adds derived log rows for writes through byte views: stores into byte descriptors' payloads and calls that may write
   unknown payload through escaping pointers.
2. Shape. `byte_payload_writes` (`:37`), `escaping_payload_backings` (`:181`), `byte_payload_descriptors` (`:421`),
   `byte_payload_backings` (`:495`).
3. I/O. Extends `self.log` and re-sorts by `(block, position)` (`:175-177`).
4. Decision. Only calls with a non-shared structural argument that has a byte descriptor inventory (`:53-66`); regions narrowed by
   `call_write_regions` overlap (`:67-96`); escaping backings by backward relevance over the descriptor flow and forward propagation
   per exposed region (`:197-203`, `:392-413`).
5. Cases. "byte payload writes into owned byte storage need their resident payload and length distinction" (`:543`);
   "unserved captured backing correspondence" (`:573`).
6. Same job elsewhere. None seen.
7. Callers. `byte_payload_writes`: `places.rs:1207`.
8. Limits. The refusals above.
9. Fail-open. A borrowed byte carrier backing is skipped (`:533-535`); "stated" (`:528-530`: an incoming descriptor names external
   payload). `DescriptorCallOrigin::Empty` adds nothing (`:570`); no reason given. "open".

## P06/places/byte_views.rs (1,426 lines, read in full)

1. Purpose. Judgments on byte and element views: formation from arrays, whole-representation re-views, subslices, byte
   observations, the "length measured immediately before" requirement, extent equations.
2. Shape. `stable_byte_predicate_equation` (`:44`), `byte_read_predicate_premises` (`:93`), `byte_array_presentation` (`:237`),
   `collection_backing_source` (`:282`), `array_view_formation` (`:335`), `representation_count` (`:586`), `observed_byte_views`
   (`:766`), `rank_descriptor_kept` (`:812`), `subslice` (`:963`), `subslice_goal` (`:1150`), `extent_contents` (`:1270`).
3. I/O. Reads operations, domains, structural types; returns propositions or `CollectionExtent`.
4. Decision. Typed shape and access matches (e.g. `:415-429`). Adjacent-operation requirement: `after.checked_sub(2)` (`:702`,
   `:1176`) and `checked_sub(count + 1 - ordinal)` (`:788`). `byte_read_predicate_premises` hard-codes `AsciiOnly` (bound 127) and
   `NoNul` (lower bound 1) (`:142-145`).
5. Cases. "an exclusive whole-representation view without establishment and handback evidence" (`:378`); "collection view cannot
   preserve these backing qualifications" (`:481`); "symbolic collection view needs its exact whole backing and element descriptor"
   (`:523`); byte element must be `u8`, unsigned, non-address, no `requires`, no restriction (`is_plain_byte_element`, `:31-38`).
6. Same job elsewhere. `collection_index_goal` (`places.rs:2726`) uses the same adjacency rule.
7. Callers. `rank_descriptor_kept`: 2 sites (`rankings.rs:907,959`).
8. Limits. Above.
9. Fail-open. `byte_read_predicate_premises`: on a non-`Refused` error it falls back to the declared argument predicates
   (`byte_sequence_argument_predicates(...).implies(required)`) and uses `Boolean(true)` (`:149-161`). "traced" only to that
   callee (not read). `extent_contents` returns `Ok(None)` for unknown producers (`:1415`): no equation.

## P06/places/cleanup.rs (395 lines, read in full)

1. Purpose. Cleanup actions as storage epochs: call bindings and backing at a cleanup point, and the effect of earlier cleanup writes.
2. Shape. `AtCleanupCall` (`:5`), `at_cleanup_call` (`:12`), `cleanup_backing` (`:54`), `cleanup_untouched` (`:115`),
   `cleanup_at_point` (`:139`), `cleanup_log_is_stated` (`:190`), `cleanup_arrival` (`:232`), `cleanup_write_ends` (`:288`).
3. I/O. Reads `self.cleanup_writes` and `tables.writes`.
4. Decision. A `Content::Vacated` overlapping write gives `Unestablished` unless the query is a case query with a write strictly
   below (`:176-181`). A window closes on an incoming edge only if every incoming edge restored the owner (`:278-280`).
5. Cases. `at_call` refuses `InvocationSite::Cleanup` (`places.rs:1497-1502`), so this adapter exists.
6. Same job elsewhere. Mirrors `Storage::at_call`.
7. Callers. `at_cleanup_call`: `lib.rs:1282`, `lib.rs:2219`.
8. Limits. "cleanup requirement needs a fresh value after a preceding action's write" (`:325`).
9. Fail-open. `cleanup_log_is_stated` refuses if the tables' `AfterCleanup` rows differ from derived effects (`:220`). None seen.

## P06/places/contents.rs (563 lines, read in full)

1. Purpose. Authenticates storage-content value rows (entry snapshots, post-call snapshots) and states the data-type clauses that
   hold of formals and returned subtrees.
2. Shape. `byte_arrival_predicates` (`:12`), `call_content_boundary` (`:59`), `initial_contents_are_declared` (`:154`),
   `observation_block` (`:223`), `returned_type_premises` (`:252`), `entry_type_premises` (`:343`), `EntryOwner` (`:461`).
3. I/O. Reads `StorageContent` value rows; pushes block observations; returns premises.
4. Decision. A snapshot is accepted only if its leaf is a scalar of the declared type and is a bound formal field, a readable block
   formal, or inside the subtree a call establishes (`:175-203`). `call_content_boundary` picks the one `MutableBorrow` or
   `WriteOnlyBorrow` actual covering the leaf (`:96-145`).
5. Cases. `entry_type_premises` omits types reached through a `Case` segment (`:408-413`).
6. Same job elsewhere. None.
7. Callers. `call_content_boundary`: `contents.rs:194,265`, `places.rs:4342`. `byte_arrival_predicates`: `joins.rs:439`.
8. Limits. "which case a formal holds at entry is not established by structure" (`:559-562`).
9. Fail-open. `returned_type_premises` and `entry_type_premises` drop clauses that are `Unproved`/`Unsupported` (`:298`, `:444-447`):
   fewer premises, not acceptance. "stated" (`:441-443`). None seen as an accept path.

---

## Coverage

- Read in full: `lib.rs` 1-2750 (2,750), `machine_batch.rs` 1-127, `machine_inputs.rs` 1-198, `minted.rs` 1-1461,
  `nominal_calls.rs` 1-185, `operational.rs` 1-440, `owned_cleanup.rs` 1-663, `places.rs` 1-9440 (9,440),
  `places/arrivals.rs` 1-110, `places/backing.rs` 1-857, `places/byte_payloads.rs` 1-581, `places/byte_views.rs` 1-1426,
  `places/cleanup.rs` 1-395, `places/contents.rs` 1-563, `places/goal_observations.rs` 1-134.
  Total 19,330 lines (2,750+127+198+1,461+185+440+663+9,440+110+857+581+1,426+395+563+134 = 19,330; matches the unit's row sum).
- Skimmed: none. Not read: none.
- Within `places.rs`, lines 8283-9440 are tests; I read them but did not record each test's content beyond what they pin
  (`observation_tests`: first-readable-formal choice, whole-array initialization with partial vacancies, selector expiry on
  re-entry, recaptured index, runtime index in formal path, copy-trace row budget, saved-slice endpoints, block-entry snapshots).

## Five things a maintainer most needs to know

1. `minted.rs` is the gate that keeps proofs honest: tables must equal what the lowered form owes, by exact kind and program
   point, before any certificate is searched (`minted.rs:469-507`, `lib.rs:1469`).
2. Obligation verdicts are mixed: `Certificate` has a kernel-checked proof; `Lent`, `Preserved`, `Window`, `TargetCondition`,
   `FiniteIntegerRanking`, `EveryExit` are structural or deferred judgments with no certificate (`lib.rs:175-228`).
3. What a leaf "holds" is one backward walk (`Storage::reaching`, `places.rs:3583`) over a write log, keyed on the place-overlap
   relation (`overlap`, `places.rs:201`) and on `Backing` (captured addresses), with the same equation solver used for reads,
   initialization, case facts and windows (`Reached::merge`, `places.rs:498`).
4. The checker is shape-limited and says so by refusing: for example plain `u8` byte elements, no erased proof formals, no storage
   passed on conditional or dispatch edges, a fixed list of operations that read storage (quoted strings in the entries above).
5. Cleanup rows are minted by `owned_cleanup::materialize` (mutates elaboration, `verdict.rs:997`) and re-validated by
   `minted.rs:1001-1033` and `Storage::cleanup_log_is_stated`; `Storage` has three near-duplicate `Bindings` adapters
   (`AtCall`, `AtCleanupCall`, `AtJoin`) plus `Returned`, `AtEntry`, `Held`.

## What I am unsure about

- `places/log.rs` (`Logged`, `Stored`, `stores`, `reaching_writes`, `index_reaching_writes`) and `places/loans.rs`
  (`provider_referent`, `borrowed_resource`) are outside this unit; behavior I attribute to them (what a log row means, who
  admits a provider referent) is inferred from callers.
- Whether `ImpossibleArm` and `Unreached::mark_*` callers exist beyond those grepped (only `Unreached` type mentions were counted).
- Callers of most `pub(crate)` items in `places/` (`backing`, `cleanup_backing`, `captured_backing`) were not counted.
- The `plain_affine` claim-frontier and `terminal_semantics::*` decisions that several "open" fail-open items defer to.
