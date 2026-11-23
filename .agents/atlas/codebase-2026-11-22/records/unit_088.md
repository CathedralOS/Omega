# Record: unit_088 (area 17-omega-source)

Unit: 36 files in `source/subsystems/compiler/source_to_psi/pipeline/typed_trees_to_checked_trees/` (the Omega-written checker:
stage between typed trees and checked trees). Paths below are relative to that directory; "read" means I read the cited lines.
Every file is `.omg`: each `machine` is a state machine with `crashes Trap`; loops are recursive machines with explicit bounds, and
every table is a fixed-size array (limits listed per file). I saw no decision keyed on a name string, label, display text or
`format!`-style rendering anywhere in the unit (read, not grepped). Judgments are keyed on handles (expression, place, machine,
field, parameter), on `kind` enums, on zone subjects, or on computed intervals.

Shared vocabulary (defined here once so entries stay short):
- **Checker** (`checker.omg:44`): one large mutable record holding the current body's places, zone, tree values, tables.
- **Value** (`evaluation.omg:24`): what checking knows of one expression: `place`, `known`+`subject`+`offset` (a linear term), `ceiling`, `floor`
  (floor stored negated).
- **zone**: a difference-bound system over subjects (implemented outside this unit; `zone_*`, `constrain`, `Bound` are called, not defined here).
- **place key** (`owned_places.omg:237-240`): `statement*4+1` for a local, `parameter*4+2`, `field*4+3` for a pattern binder.
- **checking**: bool; false during fact-gathering walks (no rejects), true during the final walk (rejects).

---------------------------------------------------------------------------------------------------------------------------

## arithmetic.omg (382 lines)
1. Purpose: computes the value (interval/term) of a fixed-integer binary operator, negation or `as` conversion and enforces overflow,
   divisor and shift-count obligations by the result type's policy (Exact / Trapping / Wrapping / Saturating). Header comment cites
   `wiki/spec/language/numeric_values.md` (`:17-25`).
2. Shape: `binary_value` :28, `negation_value` :125, `conversion_value` :154, `policy_result` :207, `require_divisor` :292,
   `require_shift_count` :328, `checker_width` :368. All take `@mutable Checker`.
3. In/out: reads `value_at` of children, `Typing` rows, `ExactInteger` helpers (`foundation`). Writes `checker.trap_route` (:230,309,350),
   rejects through `checks_reject` with `ArithmeticOverflow`, `DivisorMayBeZero`, `ShiftCountOutOfRange`, `ConversionOutOfRange`. Returns a `Value`.
4. Decides: operator switch keyed on `OperatorKind` (:33-43); policy keyed on `row.domain` (`DomainBinding::Trapping/Wrapping/Saturating`,
   anything else = Exact, :221-226); fit keyed on interval `value_within(result, row.builtin)` (:210); `checker.exact_body` makes every
   result pass unchecked (:211, set by `prepare_body`, checker.omg:480 for `MachineContainer::Measure`). Divisor proven by `proves_nonzero` (:295).
5. Cases by shape: only Add, Subtract, Multiply, Divide, Modulo, BitAnd, ShiftLeft, ShiftRight get arithmetic; `_ -> carrier(row)` (:42)
   covers all other operators. Left shift is always "unknown" ("an Exact one would need a proof this slice does not make", :110-111).
   `checker_width` is 8/16/32 by name of builtin, `_ -> bits(64)` (:369-377). Bool source converts to 0..1 (:161-170).
6. Same job elsewhere: interval operations in `evaluation.omg` (`value_sum` etc.) are called from here, not duplicated. The
   "carrier range" fallback (`value_carrier`) is repeated as local `carrier` machines at :118, :197, :251 (three copies of one machine).
7. Unused code: none suspected; every pub machine is called inside the file or from `obligations.omg` / `fact_values.omg` (read: obligations.omg:295,260,362;
   fact_values.omg:159,171,195). `policy_result`'s only caller I saw is this file.
8. Stated limits: "an Exact one would need a proof this slice does not make" (:110-111).
9. Fail-open: (a) `_ -> carrier(row)` at :42 returns the type's own range for any operator not listed. Traced: only reached for integer rows
   (obligations.omg:287-292, fact_values.omg:163-168 gate on integer), so the result is the type's range, not a narrower claim: sound. (b) `checker_width`
   `_ -> bits(64)` (:376) gives 64 for any builtin not in the six-name list; I did not read `is_carrier`, so "open" whether a carrier other than
   `i64/u64` (e.g. a 128-bit or pointer-width carrier) can reach it. (c) `exact_body` skip at :211 accepts out-of-range results in a `measure`
   body; stated at checker.omg:149-152 ("mathematical computation over naturals").

## arrivals.omg (604 lines)
1. Purpose: represents and manages "arrival paths": field/`.length` projections below a state's parameters (up to four steps) that a jump
   projects onto the target state, plus the per-state tables that hold them (:7-16 comment).
2. Shape: `ArrivalStep` :17, `ArrivalPath` :23 (parameter, depth, `step0..step3`); `arrival_path_step` :33, `arrival_path_reversed` :48, `with_step` :84,
   `path_count`/`set_path_count` :109/:128, `arrival_path` :147, `store_arrival_path` :173, `path_below` :202, `same_path` :247, `path_index` :280,
   `add_arrival_path` :309, `enter_paths` :332, `enter_path` :362, `place_or_absent` :398, `project_paths` :420.
3. In/out: reads `checker.paths[5120]`, `checker.path_counts[128]`, `checker.places`; writes them and, through `set_projection`, `checker.projection`.
   `project_paths` returns the number of projected subjects.
4. Decides: path identity = same parameter, same depth and per-step (`length`, `field`) (:248, :263) - the step's `type_handle` is not compared.
   A path is accepted into a state's table only if new, `depth` 1..4 and fewer than 40 present (:314). A place counts as a projection if its kind
   is `Field` or `Length` with a parent (:221-222). `enter_paths` reports `numbered` iff each path's entered place takes the next subject number (:351).
5. Cases by shape: depth <= 4 (arrays `step0..step3`, `arrival_path_step` `_ -> step3` for index>=3, :34-39); 40 paths/state; 128 declarations
   (`offset < 128`, :112); 5120 slots; 64 subject slots (:443, :568, :595). Only places where `entry.subject > 0` and `value.subject == 0` and `value.place>0` are projected (:492, :547).
6. Same job elsewhere: building a path from a member chain is also done in `demand.omg:97` (`member_chain`) and walking a place up its parents in
   `path_below` (:202) parallels `path_root` (`owned_places.omg:160`) and `checker_within` (`places.omg:545`). Bounds-checked getter/setter pairs here
   (`path_count`, `set_path_count`, `arrival_path`, `store_arrival_path`) follow the same shape as ~10 others in the unit (e.g. `checker.omg:606-745`).
7. Unused: grep `path_count(` / `set_path_count(` across `source/`: used in arrivals.omg (:109,:128,:283,:313,:321,:335,:593) and demand.omg (:495, :689). No unused pub seen.
8. Stated limits: "A path one jump did not project is unknown in that jump's contribution" (:15-16); "a path beyond a state's 40 arrives unknown" (demand.omg:52).
9. Fail-open: `add_arrival_path` returns false (drops the path) when full (:314-317); effect is a fact becomes unknown at arrival, which loses
   facts only (stated, :15-16). `arrival_path_reversed` `swapped` returns `finished(path)` when `low>=4||high>=4` (:64-67) - the result is a possibly
   partly unreversed path; depth<=4 makes `high<4` always true for depth<=4, so traced as unreachable for valid depth. Everything else crashes with `Trap` on out-of-range (:123-125, :142-144, :168-170).

## call_bindings.omg (192 lines)
1. Purpose: holds the "argument roster" (values substituted for a callee's parameters while its `requires`/`ensures` are decided) and saves/restores
   the checker's substitution state around nested decisions (:7-17).
2. Shape: `ArgumentValues` :9 (valid, count, `values: [Value;64]`), `ArgumentScope` :17; `argument_values_empty` :26, `capture_argument_scope` :46,
   `restore_argument_scope` :95, `capture_call_arguments` :119, `decide_bound_facts` :144, `install_argument_values` :171.
3. In/out: reads `checker.arguments[64]` and flags `substituting`, `field_values`, `argument_first/count`, `field_first`, `fields_parent`; writes the same.
4. Decides: `decide_bound_facts` runs only if `arguments.valid && arguments.count == clauses.parameter_count && count <= 64` (:151); else returns `false`
   (:153). A captured scope with `valid == false` also returns false (:158-161).
5. Cases by shape: roster size 64 (:12, :33, :82, :106).
6. Same job elsewhere: comment :141-143 says this is "the same fact evaluator for real calls and universal refinement". grep `decide_bound_facts\(` in `source/`: 5 hits =
   declaration + `contracts.omg:241` + `refinement.omg:59,82,83`; so two consumers files outside its own.
7. Unused: none suspected.
8. Stated limits: "Missing capacity is not an unbound formal: no fact is admitted from an incomplete substitution" (:7-8).
9. Fail-open: none seen. A roster that cannot be built returns `false`, so a proving caller rejects; an assuming caller (e.g. `refinement.omg:59` passes `proving=true`; `contracts.omg:241` passes its own flag) ignores the bool and so assumes nothing.

## callable_effects.omg (335 lines)
1. Purpose: decides whether a callee's operational markers (`suspends`, `blocks`, `invokes`, `reaches` service ceilings) and termination promise are within
   what a given/required contract allows, over the selected-call graph (:17-19), and checks every `reaches` ceiling on a declared machine (`check_service_ceilings`).
2. Shape: `RequiredEffects` :20; `callable_markers_refine` :22, `check_service_ceilings` :34, `service_target` :89, `callable_effects_within` :96,
   `queue_callable_effect` :209, `callable_clause_ceiling` :237, `service_row_within` :261, `permits_service` :280, `callable_has_clause` :311, `selected_progress` :327.
3. In/out: reads `checker.invocations` (graph from `invocations.omg`), `CallClauses`, `BinderContract`, `service_trait` identities; writes
   `invocations.visited/work_nodes/work_count` (used as scratch) and rejects with `ServiceOutsideCeiling` (:84) or `UnsupportedCheck` (:80).
4. Decides: markers keyed on `ClauseKind` of each clause row (:251-252); a service is permitted iff its `service_trait` identity (a `u16`, `:272`) equals one in an allowed
   `Reaches` row (:303-308) - identity, not name. Graph walk visits every node reachable via edges; `reach_only` mode stops at nodes that author a `Reaches` row (:159-160)
   and requires a body otherwise (:167).
5. Cases by shape: 10240 nodes, 32768 edges (:111, :173, :212); `Selection::Machine` and `Selection::Binder` only; any other selection `_ -> false` (:102, :333).
6. Same job elsewhere: `callable_has_clause` (:311) does the same job as `declares` (`contracts.omg:80`): "does a clause run contain kind K" (two copies of one function,
   different argument shapes: `CallClauses` vs `first,count`). `binder_terminates` (`termination.omg:636`) is a third scan of a clause run for a bare `terminates`.
   The graph traversal here (:96-207) is a fourth DFS-like pass over `InvocationGraph` besides `invocations.omg:214,541` and `completions.omg`.
7. Unused: none. Comment :17-19 says "The same selected-call graph answers callable refinement and the service reach"; users: `callable_markers_refine` (called from supply.omg:271,322), `check_service_ceilings` (checker.omg:243).
8. Stated limits: `unresolved` rejects "UnsupportedCheck" when the graph is incomplete or unsealed (:52-55, :78-81).
9. Fail-open: `callable_effects_within` returns true when it finds no edge from a non-reach-only node with no body (`examine` allows `!required.reach_only`, :167-171, then `ended` handle==0 -> next, :178-181):
   a bodiless callee with no authored markers is accepted for `suspends/blocks` etc. only if its own clauses pass `callable_clause_ceiling` (:149). Traced: the callee's own clause run is the whole
   contract for a bodiless machine, so no hidden effect exists; "traced". Binder with `contract.present == false` is rejected (:107, :202).

## checker.omg (1561 lines)
1. Purpose: defines the `Checker` scratch record and drives checking: clear, derive frames and completions, check each root machine (gathering arrival facts to a fixed point,
   then the checking walk, then ownership), settle crash routes, termination, service ceilings, supply, slots.
2. Shape: `Checker` :44-206 (fields: places 256, reference_origins 256, values 4096, frames 8, saved zones 8, matches 16, assumptions 128, bodies/arrivals/reached/queue 128 each,
   paths 5120, callers/callees 32768, supplied 1024, whole_writes 32768, unknown_writes 10240, field_writes 163840 words, completions 10240, `invocations`, `ownership`, `aliases`, `pending` 32).
   Entries: `checker_check` :208, `checker_clear` :250, `declared_routes` :283, `crash_ceiling` :295, `check_machine` :346, `prepare_body` :463, `open_entry` :493, `walk_machine` :507,
   `parameter_place` :597, `contribute` :754, `join_arrival` :889, `record_call` :1005, `record_supply` :1055, `binds_machines` :1095, `release_exclusive_arguments` :1129,
   `frame_changes` :1248, `forget_written_fields` :1278, `settle_routes` :1326, `checker_published` :1400, `back_edge` :1451, `collect_bodies` :1476, `body_handle` :1517,
   `body_slot` :1528, `publish_body_routes` :1549, `SuppliedCall` :1046.
3. In/out: reads `SyntaxTrees`, `NameResolution`, `Typing`; writes `checker.output` (a `Checks`: accepted, machine_accepted, diagnostics, crash routes, slot records).
4. Decides: root machines are those with `has_body && parent == 0` (:226); nested machines are reached as bodies of their root through lexical ancestry (:1487-1499) "Lexical ancestry locates
   declarations but supplies no facts" (:39). Arrival joins widen only after 3 loosenings and only at a back edge (discovery order, :938, :1463-1465). Crash route settle: a caller gains every cause a callee publishes
   (declared if not inferred, derived if inferred, :1344-1345); a non-inferring machine with an undeclared derived cause is rejected `CrashNotDeclared` (:1382-1392). A private machine with a body and no
   crash clause infers (`infer_omitted = !is_public && has_body`, :286). Processing stops after 64 diagnostics (:218, `diagnostic_count < 64`).
5. Cases by shape: 128 bodies per root (:1501, over-limit sets `overflowed` -> `CheckCapacityExceeded`, :445-453, :455-457); 32768 calls (:1023), 1024 supplied calls (:1073); `release_exclusive_arguments`
   handles `Selection::Machine` with a write frame else treats everything as unknown writes (:1132-1135, :1145-1147); after a changing call an exclusive argument with no storage origin is rejected `UnsupportedCheck` (:1182-1195).
6. Same job elsewhere: `checker_published` (:1400) is used by `supply.omg:263-264,337-340`; the four-way "transition-statement/arm walkers" are in other files (see flow/loans/demand/ownership). The
   bounds-checked getter/setter pattern repeats at :606-745 (`reset_arrival`, `was_reached`, `arrive`, `checker_dequeue`, `checker_enqueue`).
7. Unused: `declared_causes` (:289, pub): grep `declared_causes(` finds checker.omg (2: decl + :1016) and supply.omg (2: :262, :288); not unused. `binds_machines` (:1095, pub): grep shows only the
   declaration and the call at :1065 in the same file (1 call site). `body_handle` and `body_slot` have callers in demand.omg (:71, :467), obligations.omg (:1022), checker.omg.
8. Stated limits: "A body that rejected may have stopped before some of its crash sites; its routes are then a subset of what it derives" (:237-239); "Crash routes come from the checking walk alone: a fact-gathering walk may see a site its settled facts disprove" (:405-406).
9. Fail-open: (a) `check_machine` `drain`: after the arrival fixed-point it walks bodies with `machine_accepted` only (:387, :430); if a declaration was unreached it is checked "from its parameters' carrier ranges and its own requires alone" (:436-437) - stated, not accept-by-default. (b) the 64-diagnostic cutoff (:218) stops checking later machines; traced: `diagnostic_count >= 64` already means the output is rejected. (c) `settle_routes` loop ends when no change (:1366-1368); a `callee` outside `0..machine_count` would crash `Trap`. No unmatched-case acceptance seen.

## completions.omg (217 lines)
1. Purpose: derives a "numeric normal completion" interval per integer-returning machine (what its normal return value lies in), from the machine's own entry contracts, in a global order independent
   of declaration order (:16-21). Later callers read it instead of the type's carrier range.
2. Shape: `NumericCompletion` :22 (available, builtin, value); `derive_completions` :28, `universal_completion_walk` :122, `observe_selected_invocation` :145, `observe_normal_completion` :168, `completed_numeric_result` :191.
3. In/out: reads the graph in `checker.invocations`, bodies via `walk_body`; writes `checker.completions[index]`, `checker.collecting_completion/completion_seen/completion_value`, restores `checker.output` counters (:125-141).
4. Decides: a summary is published only if `complete && invocations.complete && completion_seen && index<10240` (:105); available for a call only if source and target are in different, both nonzero components (:196), the target's builtin equals the call row's builtin and `completion.available` (:204).
5. Cases by shape: only machines with `has_body && has_return_type` whose return type is a builtin integer carrier (:85, :93); 10240 machines.
6. Same job elsewhere: the "record this invocation" half (`observe_selected_invocation`) writes the same graph that `termination.omg`/`ranking.omg` read (:157-163). The walk saves/restores `output` counters like `universal_completion_walk` is also used by nothing else (checked: only called in this file :63,:103).
7. Unused: none. `completed_numeric_result` has one caller (obligations.omg:505).
8. Stated limits: "A fresh call-result place receives only the universal interval" (:188-190).
9. Fail-open: none seen: unavailable => `false` => caller falls through to the contract/typed path (obligations.omg:519-521).

## constructions.omg (470 lines)
1. Purpose: values of data constructions. A construction is a place keyed by its expression handle; each integer/Boolean field it writes is "pending" below it until read, then entered as a place (:21-31).
2. Shape: `PendingField` :34; `construction_value` :45, `write_pending` :188, `take_pending` :229, `equate_subject` :264, `remove_pending` :284, `forget_pending` :305, `enter_pending` :355, `copy_pending` :375, `settle_pending` :423,
   `settle_written_pending` :432, `settle_pending_fields` :438.
3. In/out: reads `ConstructField`, `MemberTarget`, `DataField`, place table; writes `checker.pending[32]`/`pending_count`, places, and zone constraints via `constrain_difference`.
4. Decides: a field is tracked iff its carrier `has_subject` (integer/Bool) (:107-110); matching a pending row to a place is by (`parent`, `field`) equality (:248); copying keeps only entries `owned_below` the source (reference-typed parents excluded unless the parent is the source, :398-400) and reachable by a path of depth>0 (:400).
5. Cases by shape: only `MemberTarget::Common` fields are written as pending (:89-90); payload fields (case payloads) get `_ -> next_field` i.e. ignored; 32 pending entries; fields that are references/records are "adopted" only if the value has a place (:146-148).
6. Same job elsewhere: copying known places in `places.omg:900` (`copy_fields`) is the counterpart for represented fields; `copy_pending` is called from it (:903).
7. Unused: none seen.
8. Stated limits: "A full table drops the field, which only forgets facts" (:185-187); "Keeping only immediate children loses a nested construction's values..." (:372-374).
9. Fail-open: `write_pending` drops a field when 32 are pending (:212-216): stated as "only forgets facts" - fact loss only (stated).

## contracts.omg (424 lines)
1. Purpose: contract facts: assume a body's `requires`, owe a callee's `requires` at calls/jumps, owe `ensures` at returns, assume a callee's `ensures` after a call. Also the `CallClauses` view of a callee (machine or binder contract).
2. Shape: `CallClauses` :38; `call_clauses` :47, `declares` :80, `checker_constrained` :111, `typed_fact` :141, `assume_requirements` :161, `require_guarantees` :176, `assume_guarantees` :210,
   `decide_call_facts` :232, `FactClause` :246, `fact_clause` :252, `decide_facts` :281, `decide_predicates` :337, `set_argument` :387, `argument_value` :407.
3. In/out: reads `syntax_trees_clause`, `Typing`; calls `decide_fact` (flow.omg:92) / `decide_tree`; may reject `GuaranteeUnproven` (:200).
4. Decides: a clause is a candidate iff its `kind` equals the requested `ClauseKind` and it has an expression (:267); it counts as typed iff the expression's type handle equals `typing.boolean` (:151); an untyped fact is neither assumed nor proved (:322-327, header :31-32) - proving a clause fails (:322), assuming it is skipped (:323 `holds || !proving`).
5. Cases by shape: only `Selection::Machine` and `Selection::Binder` have clauses; `_ -> none()` (:53). `checker_constrained` only for `TypeKind::Data` whose declaration has a `Requires` clause (:115-126).
6. Same job elsewhere: `decide_facts` (:281), `decide_predicates` (:337, only differs by calling `decide_tree` instead of `decide_fact` and fixing kind to Requires) and `decide_requires_facts` (domains.omg:216) are three
   clause-run iterators. `declares` (:80) duplicates `callable_has_clause` (callable_effects.omg:311).
7. Unused: `declares` has callers in obligations.omg:512, contracts.omg:122. `set_argument` (private) is called from domains.omg, ranking.omg, termination.omg.
8. Stated limits: "A binder without a contract and every other selection declare none" (:36-37).
9. Fail-open: when there is no contract, `call_clauses` yields an empty roster (:75), so a call through a contractless binder owes nothing and gets no `ensures`: stated (:36-37). `decide_facts` with `proving=false` returns true regardless of what was assumed (:308) - by design (assume mode).

## demand.omg (839 lines)
1. Purpose: before a machine's facts are gathered, derives for each state which field paths below its parameters need to be carried across jumps (its "demand"), settling to a fixed point (:35-53).
2. Shape: `derive_demand` :54; `MemberChain` :89, `member_chain` :97, `arrival_path_joined` :229, `demand_statements` :263, `demand_valued` :288, `demand_arms` :325, `demand_tree` :384, `demand_node` :410,
   `demand_successor` :492, `demand_callee` :566, `add_rooted` :663, `demand_siblings` :683, `demand_field_relatives` :724.
3. In/out: reads syntax trees, resolution (`value_binding`), typing selection; writes `checker.paths`/`path_counts` through `add_arrival_path`.
4. Decides: a node demands a path iff it is a `Field` read of integer/Bool type whose `member_chain` is rooted at a parameter of the current body with depth>0 (:428); an `Index` demands the collection's `.length` iff the collection is a `Slice` row (:447);
   a call/binary node passes its callee's `requires` reads through, per argument, when `body_slot(handle)>0` (a jump into the same body group, :467-469) or from the callee's `requires` clauses otherwise (:480-482). Siblings: fields named in a data declaration's `requires` beside a demanded field are demanded too (:724-834), found by scanning every data item for the one whose field range holds the field (:736-741).
5. Cases by shape: chains are at most 4 steps (:162); only `Field`, `Unary` borrow, `Name` are chain links (:123-128); only `ExpressionKind::Field/Index/Call/Binary` nodes are demand sites (:415-421); 40 paths per state.
6. Same job elsewhere: `member_chain` builds the same four-step path as `path_below` (arrivals.omg:202) but from syntax rather than from places; `demand_arms` (:325) is one of four hand-written transition-arm walkers (others: loans.omg:105, ownership.omg:316, flow.omg:1231).
   `mentions_field` (domains.omg:298) and `named` (:792) scan the same requires tree.
7. Unused: `arrival_path_joined` (pub): used at :446, :553, :651. `member_chain` used 3+ times in this file.
8. Stated limits: "a path beyond a state's 40 arrives unknown" (:52-53).
9. Fail-open: none seen. A path that does not fit (`add_arrival_path` false) means the fact arrives unknown (loses facts).

## domains.omg (606 lines)
1. Purpose: default domains: an established value of data whose declaration has `requires` facts satisfies them. Facts are installed field-by-field when a field becomes a place; also decides whether a store or a construction keeps/establishes the domain (:25-35).
2. Shape: `DomainMark` :36 (Unseen/Pending/Installed); `domain_mark` :42, `set_domain_mark` :61, `note_field` :81, `install_domains` :89, `checker_established` :170, `decide_requires_facts` :216, `mentions_field` :298,
   `reopen_domains` :337, `domain_holds` :370, `construction_holds` :401, `selected_case` :550, `member_slot` :576.
3. In/out: reads places, `DataItem`, `Clause`; writes `checker.domains[256]`, `domains_pending`, `fields_parent`, argument slots, zone via assumed facts (also into the 8 saved zones, :143-162).
4. Decides: a place root counts as "established" iff its kind is `Parameter`, `Binder`, `Call`, `View`, `Construction`, `Snapshot`, or a `Local` whose `let` has an initializer (:187-196); `_ -> no()` for others (e.g. a bare `let mutable t: T;`). A fact applies to a field iff the fact tree names that field by `ValueBinding::Field` (:314).
   `construction_holds` proves a construction iff every `requires` fact holds with written field values and zero for omitted fields (:401-541); `total <= 64` else unproved (:417-419).
5. Cases by shape: "Common and payload fields together beyond 64 are not proved" (:400); `member_slot` returns `18446744073709551615` (u64 max) for no slot (:604).
6. Same job elsewhere: see contracts.omg (three clause iterators). `mentions_field` and `demand.omg:792` both look for `ValueBinding::Field` in a requires tree.
7. Unused: `selected_case` (pub): grep shows declaration (:550) and its use at :415 only (1 call site). `reopen_domains` has callers flow.omg:958 and places.omg:503. `domain_holds` caller flow.omg:950.
8. Stated limits: "Common and payload fields together beyond 64 are not proved" (:400).
9. Fail-open: `domain_holds` returns `held()` (true) when the owner row is not data or the field has no parent (:376-379, :390-392): storing into a field of non-data / parentless place cannot break a data domain (traced). `decide_requires_facts` ends `held()` true when no clause names the field (:233) - no obligation exists (traced).

## evaluation.omg (674 lines)
1. Purpose: the `Value` type and its interval arithmetic: constants, carrier ranges, sums, products, quotients, remainders, `&`, `>>`, linear-term offsets, plus the proof helpers `proves_less`, `proves_at_most`, `proves_nonzero` used by every obligation.
2. Shape: `Value` :24; `value_unknown` :33, `value_constant` :44, `value_truth` :57, `value_carrier` :75, `is_constant` :91, `value_least` :96, `value_within` :102, `value_clamped` :110, `value_interval` :120, `value_hull` :127, `term_value` :148, `value_at` :165, `read_interval` :208,
   `checker_read_type` :232, `checker_set_value` :258, `value_sum` :287, `value_negation` :294, `value_difference` :299, `value_nonnegative` :306, `value_product` :311, `value_quotient` :340, `value_remainder` :399, `value_conjunction` :473, `value_shifted_right` :498, `offset_sum` :533, `offset_difference` :566,
   `difference_bound` :592, `proves_less` :615, `proves_at_most` :628, `proves_nonzero` :641, `at_place` :651, `value_typed` :658.
3. In/out: reads `checker.values[4096]` through `tree_base/tree_low/tree_size`, the zone (`zone_bound`), `TypeRow`; writes `checker.values[index]`.
4. Decides: `value_at` re-reads a linear term's interval from the zone each time (:185-189) so later-installed facts apply. Proofs combine the zone's relation between terms and the operands' intervals (:621-624, :634-637). Product is bounded only for two nonnegative intervals (:311-318); quotient only for nonnegative dividend and positive divisor (:340-345); `&` only if one side is nonnegative (:476).
   `checker_read_type` strips up to 8 reference layers and answers the builtin if reached, else the written type (:240-251).
5. Cases by shape: remainder: constants fold; divisor of one known sign (:420-429, :431-436); right shift folds only constants with count < 64 (:500-503); `offset_sum` linear only when one side is constant (:536-546), `offset_difference` only when the right side is constant (:568).
6. Same job elsewhere: `value_carrier` is the "range of a carrier"; `value_typed` (:658) wraps it for Bool/integer rows and is repeated as local `typed` machines in `obligations.omg:623`, `fact_values.omg:199`. `proves_less`/`proves_at_most` bodies are copies differing by the constant (-1 vs 0), :615-638.
7. Unused: `value_least` (pub): used 6 times in this file (grep: evaluation.omg:96 decl, :320,:321,:342,:352,:421). `value_truth` has callers in facts.omg:52,430, obligations.omg:146,807, fact_values.omg:126.
8. Stated limits: product/quotient unbounded outside the listed shapes (:310, :338).
9. Fail-open: `proves_less`, `proves_at_most` and `proves_nonzero` return true when `checker.zone.infeasible` (:624, :637, :647). Stated by meaning ("holds wherever the zone does", :613, :627): an unreachable path proves everything. A wrong program would get through only if the zone was wrongly marked infeasible (set at `facts.omg:81`, `flow.omg:982, 986, 1330`, `matches.omg:462`).
   `value_remainder` `unbounded`/`natural` use `right.ceiling.value.magnitude - 1` without checking `finite` for the `negative_divisor` path (:433) - `negative` requires `finite` (:424), so traced safe.

## fact_values.omg (203 lines)
1. Purpose: evaluates the nodes of a fact (`requires`) tree, children first. A fact is a proof expression: nodes take the same values as body nodes but owe no obligations and have no effects (:21-29).
2. Shape: `evaluate_fact_run` :30, `fact_value` :58 with sub-machines for Name, Path, Integer, Boolean, Field, Unary, Binary, Cast, Call.
3. In/out: reads syntax, resolution, typing; writes `checker.values` via `checker_set_value`.
4. Decides: keyed on `ExpressionKind` (:72-83); a literal that `typing_landed_integer` landed is a constant (:66-67); a call has a known value only if it is a primitive byte predicate (`ValueBinding::BytePredicate`, :87-91), otherwise its type's value (:90); `Binary` only for integer rows, calling `binary_value(..., checking=false)` (:171).
5. Cases by shape: `_ -> typed(row)` for every other expression kind (:82), so e.g. an index in a fact has only its type's range. Casts: integer target -> `conversion_value`, else only a reference cast keeps the operand's place (:188-192).
6. Same job elsewhere: `evaluate_node` (obligations.omg:40) does the same per-node value for bodies; two parallel switches over `ExpressionKind`, the fact one drops obligations (header :24-29 says this is intentional).
7. Unused: none.
8. Stated limits: "a call in it is known only by its result type, except a primitive byte predicate" (:25-27).
9. Fail-open: `_ -> typed(row)` (:82) yields the type's interval (sound). None seen.

## facts.omg (463 lines)
1. Purpose: converts a Boolean expression into zone facts (`assume`), and decides whether the zone proves a Boolean (`proves`, `proves_atom`).
2. Shape: `assume` :22, `slack` :165, `polarized` :182, `constrain_difference` :212, `push_assumption` :264, `pop_assumption` :307, `proves` :333, `proves_atom` :418.
3. In/out: reads `value_at` of nodes; writes `checker.zone` via `constrain` and `checker.zone.infeasible`; uses `checker.assumptions[128]` as a stack of `handle*2+holds`.
4. Decides: `assume` is keyed on `ExpressionKind` and `OperatorKind` (:41-47, :89-99): `&&` holding / `||` failing push both operands (:104); `!` swaps (:61); a Boolean literal contradicting its assumption sets `zone.infeasible = true` (:75-83); comparisons become difference bounds (:117-121); `!=` only adds a bound next to an order the zone already proves (:131-160); any other Boolean is `value == 1/0` (:50-57).
   `proves` needs both sides of `&&` (or failed `||`) and only one side of `||` (or failed `&&`) provided a side is a comparison or literal that proves (`proves_atom`, :376-383, :418-454). `proves_atom` treats two values of one place as equal (`same`, :442).
5. Cases by shape: `_ -> unproved()` for operators other than the six comparisons (:452); "An equality of other non-integers establishes nothing" (:18-19).
6. Same job elsewhere: `constrain_difference` is the shared "x - y <= c" primitive used by `domains`, `flow`, `places`, `constructions`, `snapshots` (grep not run; seen in constructions.omg:276, flow.omg:1222, places.omg:972, snapshots.omg:221).
7. Unused: `polarized` (pub) used at :88, :439. None unused seen.
8. Stated limits: "A deeper Boolean tree than the stack holds is simply not assumed" (:288).
9. Fail-open: **`push_assumption` full (128 entries) does nothing** (:269-289), and the same function serves `proves` (:337, :371-372, :393). For `assume` this only loses facts (stated). For `proves`, a goal that cannot be pushed is dropped and the loop ends `proved()` when the stack empties (:341-343, :405-407): a conjunction deep enough to hold more than 128 pending goals would report proved without checking the dropped ones. "open": no comment at the proving site; I did not trace whether the parser/typer caps expression depth below this.
   `proves_atom` `same` (:442) accepts `x <= x` etc. when both operands are the same place; sound for non-NaN integers.

## flow.omg (1584 lines)
1. Purpose: walks one body: statement by statement (let, assign, crash, transition, expression), evaluating trees, keeping zone facts, saving/restoring zones for `&&`, `||`, transition arms and matches, joining facts, and owing `ensures` at exits.
2. Shape: `evaluate_tree` :44, `decide_fact` :92, `decide_tree` :127, `fact_operand` :216, `applies_proposition` :269, `decide` :298, `evaluate_run` :318 (scan, node), `TreeFrame` :451, `tree_frame`/`restore_tree` :461/:468, `CapturedSubject` :476, `next_base` :483, push/pop frames :493/:516, `replay_tree` :539, `add_context` :547,
   `open_contexts` :572, `close_context` :619, `save_zone` :654, `restore_zone` :678, `resave_zone` :699, `drop_saved` :719, `walk_body` :735, `walk_statement` :837, `alias_replayable` :1021, `tree_reads_place` :1060, `store_value` :1093, `bind_place` :1171, `walk_transition` :1231, `select_guard` :1343, `select_arm` :1370, `arm_facts` :1429, `walk_target` :1510.
3. In/out: reads syntax/resolution/typing; writes zone, aliases, places, `checker.output`, values; calls into obligations, loans via ownership walk later.
4. Decides: statements keyed on `StatementKind` (:848-854). Assignment: target is captured before the value is evaluated (:913-920), then the value's qualification is required when checking (:923), then `store_value`. A transition's arms are walked in order, each under its selection facts and the failure of every earlier guard (:1313-1322);
   after all arms the path is marked infeasible (:1330) because typing proved the arms cover the subject (comment :1325-1326). An arm guard's failure is assumed only when the pattern is `Wildcard` or `Binder` (:1354). A store whose destination carrier differs from the storage's forgets the whole storage (:1119-1121).
5. Cases by shape: 8 saved zones (:657), 64 contexts (:550), 8 frames (:497), 4096 retained+tree values (:57); `walk_target.evaluated` matches `typing_selection` with only a wildcard arm `_ -> terminal` (:1562-1564): every arm target is treated as a terminal value, even though the comment above (:1507-1509) says a jump contributes facts to its state's arrival (that happens in obligations.omg:1008-1029 `observe_invocation`, read).
   A store with no place and no storage origin is rejected `UnsupportedCheck` (:1161-1163).
6. Same job elsewhere: `walk_transition` (:1231) is one of four transition-arm walkers (loans.omg:105, demand.omg:325, ownership.omg:316). The three-pass "evaluate tree with frame push/pop" is repeated in `decide_tree` (:182-206), `fact_operand` (:235-259), `alias_replayable` (:1040-1050), and `termination.omg:345-374`, `:399-437`.
7. Unused: `fact_operand` (pub) one caller (qualification.omg:229). `drop_saved` callers: flow.omg:1329, matches.omg:263. `restore_tree` and `tree_frame` used in snapshots.omg and flow.omg. `evaluate_tree` many callers. None unused.
8. Stated limits: "Typing proved the arms cover the subject... no path passes over every arm" (:1325-1326); a stale duplicated comment "would open an invariant window this slice does not close" (:934-935) precedes a comment that says the window must be closed (:936-939).
9. Fail-open: (a) `finished` marks the path infeasible after the last arm (:1330) - relies on typing coverage (stated). A transition whose arms did not actually cover the subject would have the path after it treated as unreachable (facts vacuously true). traced to "coverage.omg" by the comment only; I did not read it. (b) `decide_tree.undecided` returns `!proving` (:209-211): too-deep facts are assumed true only in assume mode, never proved. (c) `add_context` full => "An operand without a context simply gains no assumption" (:563): fact loss only (stated). (d) `window` with `checking=false` returns without rejecting (:961-966): the checking walk rejects later.

## frames.omg (690 lines)
1. Purpose: derives each machine's "write frame": which exclusive parameters it may replace whole, which data fields it may write, and whether its writes are unknown; settles to a fixed point over the call graph before any body is checked (:25-46).
2. Shape: `derive_frames` :47, `frame_machine` :111, `frame_stores` :147, `frame_call` :180, `frame_arguments` :220, `frame_write` :309, `lends_exclusively` :446, `exclusive_value` :479, `shared_value` :489, `writes_unknown` :499, `mark_unknown` :517, `writes_whole` :536, `mark_whole` :553, `writes_field` :574, `add_field_write` :604, `merge_field_writes` :636, `clear_field_writes` :666.
3. In/out: reads syntax, typing selection/adaptation; writes `checker.whole_writes[32768]`, `unknown_writes[10240]`, `field_writes[163840]` (a 1024-bit row of 16 words per machine, :578).
4. Decides: a machine with no body has unknown writes from the start (:77). A write's root is walked through `Field`, `Index`, borrow `Unary`, `Name` (:326-331); the first field met is the written field, else the exclusive parameter is written whole (:410-412). A root `Local`/`PatternBinder` holding a reference (`exclusive_value || shared_value`) is an unknown write (:394-395, :428-433); by-value roots are the body's own (not recorded).
   `exclusive_value` = expression type is a non-shared reference and the adaptation is not `Read` (:485).
5. Cases by shape: 1024 fields, 10240 machines, 32768 parameters: beyond them `writes_unknown`, `writes_whole`, `writes_field` answer **true** (:500-502, :537-539, :577-579, :597-599); `add_field_write` beyond the table marks the machine's writes unknown (:629-631). Calls only (`Call` or `Binary`, :185); a selection that is not `Machine` (binder, none) is treated as opaque and marks unknown only when an operand is lent exclusively (:206-210).
6. Same job elsewhere: `exclusive_value` (:479) vs `exclusive_access` (holds.omg:523) vs `access_excludes` (loans.omg:1362): three versions of "this use is exclusive". `frame_write`'s chain walk is another "root of a place expression" besides `path_root` (owned_places.omg:160) and `storage_origin` (references.omg:41).
7. Unused: `writes_unknown`, `writes_whole`, `writes_field` have callers in checker.omg, snapshots.omg, refinement.omg, constructions.omg. None unused seen.
8. Stated limits: "A write through a reference local or pattern binder ... such a machine keeps the conservative frame" (:38-42).
9. Fail-open: none seen: table-overflow answers are conservative (true = may write). `frame_call` for a non-`Machine` selection with no exclusive operand marks nothing (:206-210): a binder call with only shared args is "pure" for the frame; plausible, "open" whether a shared reference to a cell/atomic data could still be written (shared writes not modelled here).

## holds.omg (709 lines)
1. Purpose: checks the places held together by one call: an earlier argument that lends a place holds it until the call returns; it conflicts with any overlapping access in a later argument when either side is exclusive (:31-45). Also builds the "terminal call" roster.
2. Shape: `Hold` :49 (Released/Shared/Exclusive); `check_holds` :57, `pair_hold` :147, `retained_holds` :222, `outlives_calls` :316, `argument_hold` :354, `forwards_reference` :427, `scan_accesses` :439, `exclusive_access` :523, `callee_parameters` :541, `shared_parameter` :562, `initialize_terminal_calls` :594, `terminal_call` :702.
3. In/out: reads syntax/typing/resolution and `Checker.ownership.roles`; writes `checker.terminal_calls[131072]`; rejects `OverlappingArguments` (:515).
4. Decides: an argument holds a place by its adaptation and expression kind: `SharedLend` -> Shared (:360), `Read` -> Released (:361), borrow operators -> Shared/Exclusive of the operand (:394-398), a reference value -> its own handle (:412-415). Overlap = `path_relation != Apart` (:502-506); conflict when either exclusive and not a "snapshot" read (`copied && !excludes && same_root && Inside`, :505-506). A terminal call may transfer an existing mutable descriptor (:424-426, :101).
5. Cases by shape: `check_holds` only runs for `ExpressionKind::Call` with `count > 1` (:61) - an operator that typing resolved to a machine call (a `Binary` node, see `operand_count`, obligations.omg:966) is not checked here. Nodes with `role == Base` or `path_root == 0` are skipped (:484-485). Terminal roster: transition arm targets, `Suspend/Block/SuspendBlock` wrappers' operands, and each non-unit arm of a value `Match` (:616-685); expressions beyond 131072 are not marked (:689-690, :705-707).
6. Same job elsewhere: `argument_hold` (:354) and `lent_place` (loans.omg:708) are near-duplicates (borrow/recast/referenced cases); `exclusive_access` (:523) overlaps `access_excludes` (loans.omg:1362) and `exclusive_value` (frames.omg:479).
   Comment :588-593 says "One completion-position roster serves every selected invocation": grep `terminal_call\(|initialize_terminal_calls\(` in `source/`: checker.omg:1, holds.omg:3; the only reader is `check_holds` (:69) - one consumer.
7. Unused: `callee_parameters` (pub): grep shows holds.omg:68 and :541 (declaration) only: one call site, in the same file. `shared_parameter` used at :102 only (also pub). `check_holds` called from ownership.omg:597.
8. Stated limits: "A malformed/missing coordinate gains no terminal borrowing permission" (:687).
9. Fail-open: (a) `check_holds` ignores binary-operator calls (:61); "open": whether typing allows exclusive operands on operator-selected machines; a `Binary` call with two overlapping exclusive arguments would skip this check. (b) `access` skips nodes with `root == 0` (:484): not a place; traced fine. (c) `terminal_calls` default false (:607, :707): the failure direction is stricter, not fail-open.

## invocations.omg (753 lines)
1. Purpose: the invocation graph: nodes are machine declarations, edges are selected calls, each recorded once with an authenticated owner and target, then sealed; provides components (strongly connected), a postorder, and the Kahn-style ranking certificate (`certify_ranking`) (:1-9, :211-213, :537-540).
2. Shape: `InvocationTarget` :10 (Machine/Binder), `InvocationEdge` :15, `InvocationGraph` :26 (edges 32768, slots 131072, first/reverse_first/components/order/visited/work_* 10240 each), `invocation_target_same` :52, `invocation_graph_initialize` :71, `invocation_graph_record` :119, `invocation_graph_seal` :214, `invocation_graph_component` :456, `edge_handle` :470, `observe_rank` :500, `certify_ranking` :541, `rank_queue` :730.
3. In/out: written during the fact-gathering walks (`observe_selected_invocation`, completions.omg:145) then sealed (completions.omg:68); read by termination, ranking, completions, callable_effects.
4. Decides: recording a second time for the same expression slot succeeds only if the owner and target are identical, else the graph becomes incomplete (:129-150, `invalid` :208); recording after seal needs an existing slot (:152). A sealed or incomplete graph returns 0 (unavailable) from `invocation_graph_component` and `edge_handle` (:459, :473).
   `observe_rank` combines repeated observations by conjunction: `non_increasing && ...`, `strict && non_increasing && strict` (:520-521). `certify_ranking`: every internal edge (same component) must be `examined && non_increasing` (:597); non-strict internal edges form a residual graph that must be acyclic by Kahn elimination (:598-609, :631-698, :706-725).
5. Cases by shape: 10240 machines, 131072 expressions, 32768 edges (:74, :152); binder targets get no concrete edge and no reverse entry (:160, :193) and are skipped in sealing and ranking (:281, :573, :664).
6. Same job elsewhere: comment :1 says the topology "is shared by completion inference and termination". grep `invocation_graph_(record|seal|component|initialize)\(|certify_ranking\(|observe_rank\(|edge_handle\(` in `source/`: invocations.omg 7, completions.omg 6, ranking.omg 4, termination.omg 5 (22 hits); `callable_effects.omg` reads `checker.invocations` fields directly (:52, :111, :219) without these functions.
7. Unused: `invocation_target_same` (pub) one caller (:145). None else.
8. Stated limits: "a missing edge can never be interpreted as evidence of acyclicity or a completed callee" (:8-9).
9. Fail-open: none seen. Every capacity miss sets `graph.complete = false` (:208, :451, :727) which disables summaries and rank certificates. `visited`, `work_nodes`, `work_edges` are scratch shared with `callable_effects.omg:119,124` and `certify_ranking`; "traced": each user clears first.

## loans.omg (1387 lines)
1. Purpose: loans held by reference-bearing locals: records each local's last use, creates the loan rows an initializer lends, checks later accesses against live loans, and checks that an exit does not return a lent owned place (:36-49).
2. Shape: `find_last_uses` :53, `record_tree_uses` :207, `create_loans` :265, `copy_carrier_loans` :360, `copy_holder_loans` :371, `carrier_root` :422, `capture_local_loans` :467, `replace_lent_holders` :548, `replace_loan_holder` :669, `lent_place` :708, `holds_references` :796 (explicit stack, depth 9),
   `loan_access` :916, `own_escape` :989, `lends_owned` :1058, `carrier_owns_loan` :1085, `use_local`/`use_slot`/`last_use` :1116/:1159/:1187, `extend_uses` :1217, `add_loan` :1245, `loan_live` :1270, accessors :1282-1358, `access_excludes` :1362.
3. In/out: reads syntax/resolution/typing, `checker.ownership` (loan arrays 64, use arrays 128); writes them and rejects `LoanConflict` (:981), `LoanEscapes` (:1047), `CheckCapacityExceeded`.
4. Decides: a loan is live while its holder's recorded last use >= the current statement (:1270-1275); holders are keyed by place key (`statement*4+1`). A conflict = access path overlaps a live loan's place from a different holder and (loan exclusive or access excludes) (:953-972). An escape = a returned node lends a place whose `place_root` > 0 and is a borrow/slice node (`lends_owned`, :1058-1068) or a holder whose loans are `owned` (:1085-1107).
   `holds_references` is true for `Reference`, and for data/array types containing one; at depth >= 8 it answers true (:830, :847-850 "its limit conservatively retains possible loans", :794-795).
5. Cases by shape: 64 loans, 128 tracked locals; `lent_place` only for node roles Value/Borrow/Exclusive/WriteLend (:714-715); loan creation only when the initializer type holds references (:271); `replace_lent_holders` only for `@write` borrow of a Name (:568-569).
6. Same job elsewhere: `lent_place` (:708) vs `argument_hold` (holds.omg:354). The use/last-use table duplicates in spirit the owed/moved/borrowed tables in ownership.omg (all linear-scan arrays of keys). `find_last_uses` (:53) is one of three statement walkers with `demand_statements` (demand.omg:263) and `frame_stores` (frames.omg:147).
7. Unused: none suspected. `replace_loan_holder` pub: callers loans.omg? (declared :669; called at ownership.omg:288, loans.omg:587).
8. Stated limits: "A body with more locals than the table tracks forgets the rest, whose loans then never last past their statement" (:1138-1139).
9. Fail-open: **(a) the 128-local use table** (:1138-1155): a local named after the 128th distinct local is not recorded, `last_use` returns 0 (:1186-1194), so `loan_live` (`0 >= statement`) is true only at statement 0 and the loan expires immediately: a body with more than 128 distinct locals and a reference holder beyond the 128th could have a later conflicting access accepted. Stated by the comment ("never last past their statement"), whether the accepting effect is intended is not stated: "stated" for the table limit, "open" for the consequence.
   (b) `carrier_root` only locals (`root % 4 == 1`, :448); parameters keep ordinary lending (:447). (c) `add_loan` full => rejected (:341-343, :407-410).

## matches.omg (519 lines)
1. Purpose: tracks value-position `match` expressions inside an evaluated tree: each arm runs under the facts after the subject plus its own pattern, with earlier patterns passed over; facts after the match are the join of arm ends; the match value is the hull of arm values (:15-29).
2. Shape: `checker_add_match` :31, `advance_matches` :55, `fire_events` :104, `event_handle` :174, `match_event` :207, `checker_pattern_value` :323, `close_match` :345, `end_arm` :371, `holds_match` :427, `unreach_saved` :450, `join_arms` :472, `checker_take_join` :500.
3. In/out: uses `checker.matches[16]`, `match_events[16]`, saved zones/aliases; writes values via `checker_set_value`, zone via save/restore/join.
4. Decides: an event index is `2*arm` (pattern start; for `Wildcard`/`Binder` the value start) and `2*arm+1` (value start) (:177-190). A match is "held" (tracked) iff its two saved zones' owner ids equal `match_handle + 1` (:440). If the checker could not track it (full table) it is rejected `CheckCapacityExceeded` and its value is its type's (:47-50, :360-365).
5. Cases by shape: 16 matches per tree; each match needs two zone slots out of 8.
6. Same job elsewhere: the ownership walk has a parallel machinery for matches (`own_match_events`, ownership.omg:1283) with its own 8-slot tables; the transition-arm version of the same zone logic is `flow.omg:1303-1323`.
7. Unused: none.
8. Stated limits: "A match whose events the checker cannot track rejects as exceeding its capacity" (:25-27).
9. Fail-open: none seen. `match_event.selected` when not held silently does nothing (:303-306) but a non-held match was already rejected at record/open time (:47-50, :248-259); traced.

## obligations.omg (1029 lines)
1. Purpose: per-node evaluation of a body expression tree: each node's `Value` and its definedness obligations (index bounds, arithmetic, conversions, qualification of casts, call `requires`/effects/`ensures`, construction domains).
2. Shape: `evaluate_node` :40 (sub-machines per ExpressionKind), `checker_range_end` :632, `range_endpoint` :643, `require_callee_facts` :663, `collection_row` :688, `referent_row` :697, `exclusive_arguments_placed` :724, `predicate_value` :758, `roster_bit` :818, `domain_requires` :833, `array_extent` :891, `collection_length` :913, `selects_machine` :948, `operand_count` :966, `checker_operand` :983, `observe_invocation` :1008.
3. In/out: reads syntax, resolution, typing; writes values, zone, places, and many reject codes (`IndexOutOfBounds`, `RequirementUnproven`, `DomainUnproven`, ...).
4. Decides: `evaluate_node` switch on `ExpressionKind` (:61-76). Index: element in `0..len` via `proves_less`/`proves_at_most` (:169-172); range slice valid iff start >= 0 and (inclusive end index valid and ordered, or exclusive end <= len and start <= end) (:198); the call path (:373-419): replay returns the old call place; a byte predicate is evaluated; else the selection is recorded, and when `checking` qualifications, callee `requires`, descent (`require_invocation_descent`) are owed; result: returned reference becomes a place (:456-487), callee `ensures` make a call place and are assumed (:523-529), else numeric completion or the type's range.
   Byte predicates: literal -> from the string's octet roster bit (:769-772); domain subject -> roster when quoting a literal (:781-787); a value whose qualifying domain `requires` that predicate -> true (:798-802); else unknown. Roster bits ValidUtf8=1, NoNul=2, AsciiOnly=4, NonEmpty=8 (:820-823).
5. Cases by shape: `_ -> typed(...)` for other expression kinds (:75). Slice view place only when the base has a place (:211). A returned reference that is exclusive needs every exclusive argument to have a storage origin else the call's value is typed (:466-472). Array extent: `StaticInteger` row -> constant else any `u64` (:895-908).
6. Same job elsewhere: `predicate_value` and `fact_values.omg:94-98` share it (fact_values calls it). `operand_count`/`checker_operand` are the "call or operator" accessors used by 10+ places. `referent_row` (:697) vs `qualifying_domain` (qualification.omg:48) one-layer reference peeling; `checker_read_type` (evaluation.omg:232) another.
7. Unused: `selects_machine` (pub): callers obligations.omg:280, :435. `collection_length` / `array_extent` have callers places.omg:793, obligations.omg:159. `exclusive_arguments_placed` pub, callers: obligations.omg:467 only (1).
8. Stated limits: "Comparisons and Boolean operators carry no obligation, and float arithmetic follows its format's rules, outside this slice" (:264-265).
9. Fail-open: (a) `index.element` rejects only when `checking && !(below && above)` (:172); a non-checking walk never rejects (by design). (b) `construct`: obligation only when `checking && constrained` (:537); an unconstrained type (no `requires`) owes nothing (traced). (c) `path` for a `Constant` binding returns the constant's value (:556). (d) `ValueBinding::Data/DataCase` otherwise `composed` (:547) - no domain proof for other bindings: "open" whether a construction through another binding kind could skip a data domain.

## owned_places.omg (662 lines)
1. Purpose: which places a body owns, how two overlap, and a type's multiplicity (copy/affine/linear) (:27-32).
2. Shape: `checker_multiplicity` :38, `declared_linear` :109, `place_root` :150, `path_root` :160, `field_place` :250, `index_steps` :266, `place_depth` :293, `place_ancestor` :320, `same_field_path` :350, `distinct_indexes` :428, `skip_indexes` :454, `selected_field` :473, `PlaceRelation` :489 (Apart/Inside/Encloses), `place_relation` :496, `path_relation` :504, `through_write_only` :583, `borrowed_place` :613, `same_owned_place` :643.
3. In/out: pure functions over syntax/resolution/typing.
4. Decides: multiplicity keyed on type row: data = declared multiplicity unless a type argument is linear (:65-71); array = element's, 16 levels then `Affine` (:54-57); generic parameter = its bound (:60-63); everything else `Unrestricted` (:49). Place keys: Local `statement*4+1`, Parameter `parameter*4+2`, PatternBinder `field*4+3` (:237-239). Two places relate by root equality, field depth and per-step field identity (`selected_field`) and "alike" indexes: indexes are alike unless both are integer literals with different values (:428-444). A `.length` field selection is not a place (:250-262).
5. Cases by shape: `path_root` follows Name, Index, Field, reference `Cast`; other expression kinds -> 0 (:173); depth cap 64 (:196, :203, :227); casts root only for reference-typed casts (:183-186).
6. Same job elsewhere: see arrivals (`path_below`), frames (`frame_write`), references (`storage_origin`).
7. Unused: `through_write_only` (pub) callers: ownership.omg:935 (1). `index_steps` callers: ownership.omg:1205, :1206 (2 sites). `distinct_indexes` (pub): called at :374 only inside this file. `same_owned_place`: ownership.omg:1545. `place_relation`: owned_places.omg:652 and ownership.omg:1121,1167,1234 (4 callers).
8. Stated limits: none.
9. Fail-open: **depth cap 64 returns root 0** (`base` :227-230, `recast` :196-199, `recast_reference` :203-206) which `path_relation` reads as `Apart` (:518-520) and `loan_access`/`moved_conflict` read as "no conflict" (loans.omg:920, ownership.omg:924): a place expression nested more than 64 field/index/cast steps deep is treated as unrelated to everything. "open": I did not trace whether the parser limits nesting below 64.
   `place_ancestor` returns the current node when `walked >= 64` (:327-329) - a mismatch would answer Apart later.

## ownership.omg (2160 lines)
1. Purpose: ownership of owned places (spec `ownership.md`): moves, use after move, linear obligations on every exit, branch joins, writes re-establishing moved places, and a use-role assignment (`UseRole`) per tree node.
2. Shape: `UseRole` :61, `MovedSet` :76, `PlaceCover` :83, `Ownership` :90 (moved, arm_saved, owed 64, borrowed 64, roles 4096, conditionals 32, matches 8, before/joined 8 sets, votes 512, uses 128, loans 64, statement), `own_body` :140, `own_statement` :250, `own_transition` :316, `own_target` :428,
   `borrow_binders` :481, `own_exit` :512, `owe_linear` :541, `own_tree` :567, `assign_roles` :612, `pass_role` :650, `scan_owned_tree` :850, `own_node` :905, `own_write` :1038, `moved_conflict` :1107, `moved_cover` :1134, `reestablish` :1220, `root_moved` :1251, `own_match_events` :1283, `close_owned_match` :1385,
   `end_owned_arm` :1436, `require_joined_linear` :1558, set/table accessors :1597-2160.
3. In/out: reads syntax/resolution/typing and the checker's origins; writes `checker.ownership.*` and rejects `UseAfterMove`, `MoveOutOfBorrow`, `LinearUnconsumed`, `LinearOverwritten`, `WriteOnlyObservation`, `CheckCapacityExceeded`, `UnsupportedCheck`.
4. Decides: by-value use (`role == Value || Drop`) of an affine/linear place moves it unless multiplicity is `Unrestricted` (:960-967); a move out of a place reached through a reference or a binder of a borrowed subject rejects `MoveOutOfBorrow` (:982-984); a linear place may move only into a destination and only where every path moves it (`discarded = Drop || conditional`, :973-979). Writing: inside a moved place rejects (`Within`), exact re-establishes, a live owed linear place overwritten rejects (:1052-1071). Exits (tail, jump, terminal call, end of Unit body) check each owed key is moved whole (`root_moved`: root equal and depth 0, :1267). A match joins arms; a linear place moved by only some arms rejects (:1573-1585).
   Roles: a node's children take roles by its kind (:655-665): call args and construction fields `Value`, field/index base `Base`, borrows -> `Borrow/Exclusive/WriteLend`; reference casts inspect only; value casts need `Unrestricted` result except `Value`/`Drop` role (:696-720).
5. Cases by shape: 64 moved, 64 owed, 64 borrowed binders, 32 conditionals, 8 matches per tree, 1024 arms per match (:2048), 512 vote cells; all table-full cases reject `CheckCapacityExceeded` (:558, :897, :992, :505). `ownership` walk ends at a `Transition` or `Tail` or `Crash` statement (`ended = true`, :295, :299, :304). Casts: "Until that is retained, only copyable results use this path; a noncopyable value is not silently left in its operand" (:710-713).
6. Same job elsewhere: the match machinery here (`own_match_events`, `end_owned_arm`, `close_owned_match`) parallels `matches.omg`; the `owed`/`moved`/`borrowed` linear-scan tables repeat the shape of `use_*` in loans.omg:1116-1214. `own_transition` is one of four arm walkers.
7. Unused: the `ownership_*`, `owes`, `owed_at`, `borrows`, `votes_at` accessors are used within this file (read); `remove` (pub, :1640) used at :1243 only. None unused seen.
8. Stated limits: "Until that is retained, only copyable results use this path" (:711-713).
9. Fail-open: none seen: every capacity miss rejects. `own_node.used` treats a node with `root == 0` and not `through` as a plain value (`value`, :924-926) which only matters for `Drop` temporaries (:997-999, :1005-1024): a `Drop`ped `Call/Construct/Match/Array` of linear type rejects; any other expression kind with linear value is not rejected (:1007-1010) - "open": e.g. a dropped `Cast` or `Field` temporary of linear type (cast handled at :714-720).

## places.omg (988 lines)
1. Purpose: the places a body names (parameter, local, binder, field, length, referent, call result, construction, snapshot, rank entry, contract result, view) and the integer/Boolean subjects among them; entering, forgetting, copying places and reading names' values.
2. Shape: `PlaceKind` :32, `Place` :59, `place_at` :70, `checker_carrier` :92, `has_subject` :113, `open_places` :118, `checker_place` :132, `place_carrier` :285, `find_place` :305, `enter_range` :357, `restore_ranges` :396, `forget_place` :428, `forget_backing` :438, `forget_contents` :464, `forget_storage` :471, `window_owner` :510,
   `checker_within` :545, `name_value` :583, `checker_constant_value` :738, `member_value` :765, `place_value` :830, `subject_place` :864, `copy_fields` :900, `enter_place_range` :983.
3. In/out: `checker.places[256]`, `place_count`, `subject_count` (<64), zone via `enter_range`/`zone_forget`; domain marks.
4. Decides: places are equal iff `kind`, `handle`, `parent` equal (:336) (type is not compared); a place gets a subject iff its carrier is integer/Bool (:189, :205); table full (256 places or 64 subjects) -> returns 0 and sets `checker.overflowed` (:278-280) (the machine then rejects `CheckCapacityExceeded`, checker.omg:445-453).
   `name_value` keyed on `ValueBinding` (:594-604): Constant, Local, Parameter (with substitution), PatternBinder, MatchedSubject, Field (construction field values vs installed domain), ReturnedValue/ContractReturnedValue, DomainSubject, `_ -> unbounded` (:604). A constant is exact only when its initializer was `Adaptation::Landed` at an integer type (:745).
   `forget_storage` forgets subjects inside the place and re-enters carrier ranges, skipping a collection's own `.length` for content stores (:488-489).
5. Cases by shape: `checker_within` climbs only `Field`, `Length`, `Referent` parents (:566), depth 256; `copy_fields` copies paths of depth <= 4 only (via `path_below`) (:920-922).
6. Same job elsewhere: `checker_within` vs `owned_below` (reference_origins.omg:188): two ancestor tests with different parent-kind sets (owned_below excludes Referent and requires non-reference owner types).
7. Unused: `restore_ranges` caller checker.omg:535; `subject_place` many. None unused seen.
8. Stated limits: "A full table loses the place; the body rejects once it ends" (:277).
9. Fail-open: `name_value` `_ -> unbounded()` (:604) gives the unknown value for state/machine/data names (sound). `checker_carrier` returns `Never` for non-subject types (:99-100) - read as no subject. None seen.

## qualification.omg (483 lines)
1. Purpose: declared-domain qualification: a value whose type a declared domain qualifies satisfies its predicates; a value meeting a qualified destination must already carry that qualification or satisfy the predicates; a routed domain is never minted from predicates (:31-44).
2. Shape: `qualifying_domain` :48, `decide_domain` :80, `names_result` :126, `routes_to` :143, `decide_membership` :183, `require_qualification` :260, `assume_qualification` :324, `require_argument_qualifications` :342, `require_call_qualifications` :398, `assume_parameter_qualifications` :406, `require_result_qualification` :438, `require_stored_qualification` :460.
3. In/out: reads domain items, path bindings, typing rosters; rejects `DomainUnproven` (:316), `UnsupportedFact` (:246).
4. Decides: a destination is qualified iff its type (or a reference's referent, one layer) has `domain == DomainBinding::Declared` (:63); the given value already carries it iff its qualifying-domain handle equals the destination's and adaptation is `None` (:293). A routed domain (`route_count > 0`) is proved only when `checker.minting` and the current machine is one of its routes (:102-107, minting set when the machine's own return value is checked, :450-453, or in a `result` membership while proving a returning `ensures`, :232). Otherwise predicates are decided with the value as the domain subject (:109-118).
5. Cases by shape: one layer of reference peeling (:52-60). `decide_membership` only for `PathBinding::Domain`; a case membership `value in Type::Case` establishes nothing and is rejected `UnsupportedFact` when it must hold (:197, :238-247).
6. Same job elsewhere: none.
7. Unused: `names_result` caller :230. None unused.
8. Stated limits: header :44-46 says "Primitive byte predicates evaluate to no known value yet, so a domain naming one is likewise established only by carrying it" - but `predicate_value` (obligations.omg:758-815) does evaluate them (literal roster, domain-subject roster, declared requires). The header and the code disagree (read).
9. Fail-open: `decide_membership.declared` returns `held()` when the value already carries the domain (:213-214) - intended. `assume_qualification` ignores the result (:334-336) (assume mode). None seen.

## ranking.omg (462 lines)
1. Purpose: normalizes a termination witness into a "rank" of up to 64 component values, snapshots entry ranks, compares ranks at recursive calls (preserving/strict/unknown) and owes the rank range at entry and at every internal call (:18-21, :200-202, :335-377).
2. Shape: `NormalizedRank` :22 (valid, view, measure, count, `values: [Value;64]`), `normalized_rank_initialize` :30, `evaluate_rank` :50, `natural_rank_value` :167, `snapshot_rank` :174, `compare_ranks` :202, `require_rank_range_entry` :261, `snapshot_rank_bound` :302, `normalized_rank_within` :310, `require_invocation_descent` :335, `capture_rank_actuals` :379, `check_rank_arrival_range` :405, `rank_endpoints_pinned` :428, `validate_rank_values` :441.
3. In/out: reads termination clause, ranking views from resolution, graph (`edge_handle`, `observe_rank`), zone; writes `checker.entry_rank`, `entry_rank_bounds`, `entry_rank_has_range`, and observations into the invocation graph edges; rejects `RankRangeUnproven` (:298, :424).
4. Decides: a rank is valid only if its view elaborates (not `Default`) and its component count matches the view (:70, :90, :104, :164) and every component is proved a natural (`0 <= v`, :458). Comparison requires same view, measure and count (:205): 2 = some component strictly decreases with all earlier ones kept (:229-233), 1 = every component kept, else 0; for distance views, compares lower/upper endpoints (:240-254). Internal call needs `pinned` entry endpoints (:418, :431-437).
5. Cases by shape: views `Measure` (declared measure; lexicographic components <= 64 or one computed value), `SliceLength`, `BoundedDistance` (2 args), `IncreasingTo`, `Descending` (:78-85); `_ -> {}` leaves the rank invalid. 64 components.
6. Same job elsewhere: range test duplicates `rank_within` (termination.omg:212) used from `normalized_rank_within`.
7. Unused: `natural_rank_value` (pub) used only at :458; `snapshot_rank` pub used at checker? grep: ranking.omg only (decl + call :275). Not found elsewhere: one call site.
8. Stated limits: "Entry components are immutable snapshots, separate from live parameter homes" (:21).
9. Fail-open: none seen: invalid or unknown comparisons return 0 (:205-207); `observe_rank` stores `non_increasing = comparison > 0` and strict = `comparison == 2` (:373).

## recasts.omg (112 lines)
1. Purpose: checks `reference as @T` (a recast views storage under another scalar shape): shared view needs every source representation valid as target; mutable view both directions (:15-29).
2. Shape: `check_recast` :30 with `referents`, `source_domain`, `shapes`, `mixed`, `boolean_source`, `bounded_field`, `proved`, `fenced`, `unproven`.
3. In/out: reads typing rows, the operand value's place, zone (`proves_at_most`); rejects `UnsupportedCheck` (:104) or `RepresentationUnproven` (:108).
4. Decides: skipped unless `checking` and the source is a reference with a different `inner` type handle (:35). Declared domain on the target -> fenced (`UnsupportedCheck`, :46-47). Qualified source viewed mutably -> unproven (:53-55). Both Bool or both non-Bool -> accepted (`source_boolean == target_boolean`, :60-61). Integer-to-Bool needs the borrowed place be a field of a data type with `requires` and the zone proves 0..1 (:82-101). Bool source mutably -> unproven (:73-77).
5. Cases by shape: Bool/non-Bool classification only (:44-45). "Typing admitted one scalar width on both sides" (:21-22).
6. Same job elsewhere: none.
7. Unused: `check_recast` caller obligations.omg:348 (1).
8. Stated limits: "a qualified target asks for predicates checking does not compare and is unsupported unless both referents are the one type" (:26-29).
9. Fail-open: `shapes` accepts every pair where both are non-Bool (:60-61) without comparing widths or float vs integer: relies on typing having admitted "one scalar width" (stated, :21-22, and the header says same-width integers/floats share all bit patterns). "stated". A source `Never`/aggregate recast would be accepted if typing let it through; not traced.

## reference_backings.omg (200 lines)
1. Purpose: at each call that returns a reference-holding type, publishes the "backing roster": the set of origins (exact, footprint, view, unknown) the result may alias, one immutable group per publication (:15-17).
2. Shape: `ReferenceView` :18 (first, count); `publish_reference_backings` :25 (machines `opened`, `group`, `next`, `argument`, `home`, `publish`, `full`), `append_argument_backing` :96, `append_reference_backing` :179.
3. In/out: reads call operands, `checker.reference_origins`; writes `checker.reference_views[256]`, `reference_backings[256]`, `reference_origins[]`, a `PlaceKind::View` home.
4. Decides: per operand: `Adaptation::Read` contributes nothing (:101); a reference or `SharedLend` argument with a place contributes its exact origin (:119-129), else its footprint if one can be recovered (:133-138), else `Unknown` (:136); an owned carrier holding references contributes its inherited view roster or `Unknown` (:156-161). A call result is published only if its type holds references (:29-30).
5. Cases by shape: 256 groups/backings; full => `CheckCapacityExceeded` (:197).
6. Same job elsewhere: none.
7. Unused: `publish_reference_backings` caller obligations.omg:436 (1).
8. Stated limits: "An owned carrier without a complete published roster may contain unmaterialized reference fields/elements ... a later write through this result must refuse incomplete backing" (:168-171).
9. Fail-open: none seen: unknown backing is the pessimistic answer.

## reference_origins.omg (288 lines)
1. Purpose: provenance of reference descriptors: `ReferenceOrigin` (Unrecorded, Unknown, Exact, Footprint, View) and operations to read, record, copy and forget them (:6-16).
2. Shape: `ReferenceOrigin` :9, `exact_reference_origin` :20, `reference_view_backing` :36, `reference_origin_place` :62, `reference_origin` :71, `record_reference_origin` :85, `same_reference_origin` :100, `incoming_reference_field` :130, `copy_reference_origin` :156, `owned_below` :188, `forget_reference_origins` :221, `forget_reference_storage` :239, `clear_reference_origins` :266.
3. In/out: reads/writes `checker.reference_origins[256]`.
4. Decides: inside an open `&&`/`||`/transition context (`saved_count != 0`) a new origin replaces an existing one only if identical, else the slot becomes `Unknown` (:91-93); zone restore does not restore captures (:83-84). An `Unrecorded` field is "incoming" (arbitrary referent) only when rooted in `Parameter`, `Call`, `ContractResult`, `View` (:143-146).
5. Cases by shape: 256 places; parent chain depth 256 (:41, :135, :193).
6. Same job elsewhere: `owned_below` vs `checker_within` (places.omg:545).
7. Unused: `reference_origin_place` caller loans.omg:535, checker; `incoming_reference_field` callers reference_writes.omg:158, references.omg:401. None unused.
8. Stated limits: "Missing local provenance is not evidence of disjoint storage" (:128-129).
9. Fail-open: none seen; unknown is pessimistic.

## reference_writes.omg (385 lines)
1. Purpose: when a call, assignment or refinement writes a referent, collects the transitive storage reachable through exclusive reference descriptors (including backing rosters and views) and retires facts about it, after snapshotting old operand reads (:15-25, :286-288).
2. Shape: `ReferenceRelease` :17 (Refinement, Invocation, Assignment); private `ReferenceWrites` :26 (pending/seen/written/contents[256], views[256]); `forget_exclusive_referents` :39, `collect_reference_writes` :99, `collect_reference_descriptor` :140, `mark_reference_write` :185, `collect_reference_view` :246, `apply_reference_writes` :289 (two passes: snapshot then retire), `prepare_reference_store` :370, `reject_reference_release` :361.
3. In/out: reads places, origins, views, backings; writes forgets facts (`forget_place`, `forget_contents`, `forget_reference_descendants`), snapshots operands (`snapshot_operands`, `snapshot_assignment`), bumps `write_epoch`, may reject `UnsupportedCheck` (:364).
4. Decides: a descriptor's origin selects what is written: `Exact` whole, `Footprint` contents only, `View` the place plus every backing in its roster, `Unrecorded` incoming -> logical exact origin, anything else Unknown -> reject (:152-161, :188-193, :230-243). Same reject for Invocation, Assignment and (via the item span) Refinement (:231-235).
5. Cases by shape: 256 places/views; full queues silently stop enqueuing (:84, :217) - see 9.
6. Same job elsewhere: `snapshot_operands` (snapshots.omg:76) is the other half; `forget_written_fields` (checker.omg:1278) is the narrow-frame version.
7. Unused: `prepare_reference_store` caller flow.omg:1113, :1139. None unused.
8. Stated limits: none.
9. Fail-open: `candidate` and `enqueue_reference_write` drop work silently when `writes.count`/`view_count` reach 256 (:84, :217): "open" - a write whose reachable-storage set exceeds 256 place indexes (the place table itself is 256 and `seen` dedups, so `count < 256` is bounded by the table; traced safe for `writes`; the `views` queue is bounded by 256 groups, also bounded).

## references.omg (454 lines)
1. Purpose: proof provenance of borrowed storage: how a local reference captures its initializer's storage origin, which storage a write lands in, and the value a reference denotes (:20-36).
2. Shape: `storage_origin` :41, `initialize_place` :80, `replace_reference` :114, `capture_footprint` :158, `local_footprint` :195, `written_origin` :225, `store_origin` :287, `reference_value` :318.
3. In/out: reads places/origins, syntax/resolution/typing; writes origins, zone via `bind_place`; rejects `UnsupportedCheck` (:144).
4. Decides: `reference_value` of a place that is a reference: Local -> captured origin Exact/View, else unknown (:371-388); Parameter/Call/View/ContractResult -> the referent's scalar subject (lazily entered) (:360-366); Field -> its recorded origin or an incoming parameter-rooted referent (:390-405); anything else unknown (:367). A view of a different carrier than the storage reads only as a place (`foreign`, :415-423).
   `replace_reference` handles a `Name` of shared-reference type bound to a `Local`; other shared-reference stores are rejected `UnsupportedCheck` (:127-130, :143-146).
5. Cases by shape: `storage_origin` and `written_origin` follow borrow operators, and Index/Field of non-reference rows, depth < 256 (:63, :247).
6. Same job elsewhere: `storage_origin` (:41) vs `written_origin` (:225): the second adds a name-footprint lookup; the unplaced-walk bodies are duplicated nearly line for line (:54-66 vs :238-251).
7. Unused: `storage_origin` callers: checker.omg:1175, obligations.omg:738. `local_footprint` caller :270 only. None unused.
8. Stated limits: "Conditional and returned references without a proved storage identity remain unknown" (:30-31); "Other reference binding storage ... remains an explicit refusal" (:112-113).
9. Fail-open: none seen; unknown reads give `value_unknown`.

## refinement.omg (232 lines)
1. Purpose: universal implication for a satisfier or static supplier: in an isolated "world", the given contract's `requires` must imply the required contract's, and the required's `ensures` must follow from the given's, for arbitrary formal values (:16-21, :22-52).
2. Shape: `callable_facts_refine` :22, `refinement_entry` :54, `refinement_post` :62, `contract_result_type` :88, `open_refinement_world` :97, `clear_refinement_world` :134, `retire_refinement_arguments` :176, `retire_refinement_value` :208.
3. In/out: resets checker scratch (`prepare_body`, `open_places`, values), writes places; returns bool. Output, call/arrival rosters, settled routes, completion summaries and write frames are not touched (:18-19).
4. Decides: skipped (true) when neither side has `requires` premises nor the required has `ensures` (:29-35); entry: `given`'s requires proved under `required`'s requirements assumed (:59); post: rebuild the world, retire exclusive arguments through the callee's frame (whole when `writes_unknown` or `writes_whole`, else field-wise, :215-221), assume given `ensures` and prove required `ensures` for one arbitrary `PlaceKind::ContractResult` (:79-84). Result type: the given's return type or `typing.unit` (:91-94).
5. Cases by shape: roster 64 (:103); `value.place > 0` needed for every parameter else the roster is invalid (:123).
6. Same job elsewhere: uses `decide_bound_facts` (:59, :82, :83) as comment says.
7. Unused: `callable_facts_refine` callers: supply.omg:272, :323 (2).
8. Stated limits: "This owner does not execute a supplier or borrow caller argument facts" (:20-21).
9. Fail-open: `callable_facts_refine` returns true when both sides are empty (:33-35, stated as "Empty implications need no arbitrary value construction", :31-32). Wrong program: a supplier with no `requires` against a requirement with no `ensures` - no fact to prove, so nothing wrong passes (traced).

## slot_review.omg (113 lines)
1. Purpose: admits the public slot records a trait requirement publishes: every returned-borrow origin and parameter qualification must be in range, and every qualification must pair with one origin of the same parameter index when both exist (:8-11).
2. Shape: `SlotReview` (empty data) :13, `prepare` :15 (no-op), `admits` :17 (`origins`, `qualifications`, `paired`, `seek`, `find`, `compare`).
3. In/out: reads `Checks` slot rows via `slot_origin`, `slot_qualification`; returns bool.
4. Decides: indexes `< 1024` (:43, :58); pairing: if there are both qualifications and origins, each qualification must find an origin with `kind == Parameter` and equal `parameter_index` (:98-99); if either list is empty, admitted (:65-68).
5. Cases by shape: at most 64 rows per list (:23, :50).
6. Same job elsewhere: the product consumer is `driver/slot_review.omg` (header :12); the rows are produced by `slots.omg`.
7. Unused: `admits`/`prepare`/`SlotReview` re-exported by `driver/slot_review.omg:1-15` (crate map); I did not count further callers.
8. Stated limits: "An empty roster formed no selector slot" (:9).
9. Fail-open: `paired` admits when `slot_origin_count == 0` even if qualifications exist (:65-68): stated in header ("An empty roster formed no selector slot. A qualification that shares the compilation with a returned borrow must name one of those origins", :9-10): a qualification is checked against origins only when a returned borrow exists. "stated". Rows beyond index 64 are not examined (:23, :50): `index < 64` stops early and `admits` then returns true for the examined prefix; "open": counts above 64 cannot happen if `push_slot_*` caps at 64 (not read).

## slots.omg (216 lines)
1. Purpose: projects the public slot records of each trait requirement from its signature: for every reference parameter with a declared domain, a qualification precondition; if the return type is a reference to the same type as a parameter's referent, that parameter is the returned-borrow origin (:17-21).
2. Shape: `project_slots` :23 (`next_machine`, `requirement`, `typed_return`, `parameters`, `one_parameter`, `reference_parameter`, `qualify`, `advance`, `taken`, `finish`, `record_origin`).
3. In/out: reads `MachineContainer::Trait` items, parameter/return type rows; writes `push_slot_qualification`, `push_slot_origin` on `checker.output`.
4. Decides: trait requirements keyed on `item.container == MachineContainer::Trait` (:37); reference-parameter index counts reference parameters only (:19-21); the first reference parameter whose referent handle equals the returned referent's handle is the origin (`same`, `found`, :127, :167-169) - type-handle identity.
5. Cases by shape: only the first match is recorded (`same && !found` -> `taken`); a returned non-reference records no origin (:50-52, :65-68).
6. Same job elsewhere: none.
7. Unused: `project_slots` caller checker.omg:245.
8. Stated limits: none.
9. Fail-open: `record_origin` ignores a full table (both arms of `transition recorded` go to `next_machine`, :209-212): a dropped origin leads `admits` to refuse unpaired qualifications, so the failure direction is stricter. traced.

## snapshots.omg (348 lines)
1. Purpose: before a call (or an assignment) may write a place, captures every earlier-evaluated operand whose value reads that place into a snapshot, so a later fact is about the captured value, not the mutated storage (:13-25).
2. Shape: `WriteFootprint` :29 (place, callee, whole); `written_subject` :38, `snapshot_operands` :76, `snapshot_tree` :128, `snapshot_node` :154, `capture_subject` :275, `snapshot_assignment` :333.
3. In/out: reads checker values/places/zone; writes snapshot places (`PlaceKind::Snapshot`), re-sets node values, zone equalities.
4. Decides: a scalar term is captured iff its owning place lies under the written place (`checker_within`) and, for a named (non-whole) frame, the path has a field the callee may write (`writes_field`) (:41-68, :184); a record value is captured iff its place overlaps the written place and its type is `Data` (:235-243); any other value loses its place and keeps its interval (:264-266). References keep their place (not captured) unless the adaptation is `Read` (:167).
5. Cases by shape: `Data` rows only for record snapshots (:243); operand scanning goes back through earlier trees until `tree_low` (:106-118).
6. Same job elsewhere: `forget_exclusive_referents`/`apply_reference_writes` call `snapshot_operands` (reference_writes.omg:343) and `snapshot_assignment`; `checker.omg:1222` also calls it.
7. Unused: `snapshot_assignment` callers reference_writes.omg:338? `snapshot_assignment(...)` called at reference_writes.omg:338, :383 (not counted by grep). Fine.
8. Stated limits: "Aggregate operands are captured separately: their written fields can still be pending" (:35-37).
9. Fail-open: none seen: a failed snapshot (`unplaced`) leaves an interval-only value (:264-266).

## supply.omg (356 lines)
1. Purpose: after bodies, crash routes and termination settle, checks supply: each `satisfies` clause refines its requirement, each static machine argument refines its binder's contract, and promised termination is settled (:34-37).
2. Shape: `check_supply` :38, `check_progress` :120, `check_supplied_calls` :140, `refine_static_arguments` :174, `refine_machine_argument` :239, `selected_routes` :283, `check_refinement` :293.
3. In/out: reads machines/clauses/contracts/`checker.supplied`, routes from `checker.output`; rejects `RefinementUnproven`, `CrashNotDeclared`, `ProgressUnproven`, `UnsupportedCheck`.
4. Decides: a supplier's published crash causes (declared if not inferred, else derived) must be within the required contract's declared causes (:263-266, :341); markers (`suspends`, `blocks`, `invokes`, termination, service reach) via `callable_markers_refine` and facts via `callable_facts_refine` (:271-272); a machine with a `terminates` promise whose settled termination is false is `ProgressUnproven` (:126-127); a body-less machine with a ranked `terminates` is `UnsupportedCheck` (:103-110).
   Static argument pairing: binder parameters of kind `Machine` take, in order, the next static argument that names a machine (`MachineArgument != Absent`) (:172-173, :207-212).
5. Cases by shape: only calls whose callee has machine binders are recorded (checker.omg:1065); `refine_static_arguments` needs the call's callee expression to be `ExpressionKind::Apply` (:180); `Declared` and `Forwarded` arguments only, else `RefinementUnproven` (:254-256).
6. Same job elsewhere: `check_refinement` (:293) and `facts` (:270) share the refine steps; both use `checker_published`.
7. Unused: `check_supply` caller checker.omg:244.
8. Stated limits: none beyond headers.
9. Fail-open: (a) `refine_machine_argument`: `contract.present == false` -> `{}` accepts (:246-248): a binder without a contract is unconstrained (stated in contracts.omg:36-37 "A binder without a contract ... declare none"). (b) `refine_static_arguments`: `applied.kind != Apply` -> `done()` (:180-183), i.e. a recorded supply call whose callee expression is not an `Apply` is not refined at all: "open" (I did not read how typing records `supplied` calls or whether non-Apply callees can reach `record_supply`, checker.omg:1055-1070).
   (c) `unranked` stops the machine loop with `next_machine` skipping `check_supplied_calls` and `check_progress` for that machine (:109-111), but only after rejecting; fine.

## termination.omg (694 lines)
1. Purpose: termination as a property of the global invocation component: a machine's termination clause (promise vs ranked witness), rank evaluation helpers (`rank_bounds`, `rank_within`, `elaborated_view`), source-parameter capture, witness substitution, and `settle_termination` (component settlement and ranking validation) (:33-37).
2. Shape: `TerminationClause` :43; `checker_termination_clause` :50, `requirement_promises` :99, `reset_termination` :137, `record_binder_call` :154, `settled_termination` :174, `rank_bounds` :197, `rank_within` :212, `elaborated_view` :249, `natural_row` :290, `capture_source` :307, `substituted_value` :333, `measured_value` :385, `component_value` :448,
   `RankBounds` :481, `settle_termination` :486, `binder_terminates` :636, `validate_rank_component` :653.
3. In/out: reads the invocation graph and termination clauses; writes `checker.terminating[10240]`, `components_done`, `rank_reported`, `binder_calls`; rejects `RankUnproven` (:692).
4. Decides: `settle_termination` marks a node terminating iff its component is ranked (`certify_ranking`, :489, :533) **and** every member has a body or a termination promise (:547), and every concrete edge leads to a node of the same component or a node already terminating (:576), and every binder edge targets a binder whose contract promises `terminates` (:580) (identity, via graph handles). A ranked clause on a machine in an unranked internal cycle -> `RankUnproven` once per owner (:673-692). `elaborated_view`: default view by the first argument: one unsigned integer -> `Descending`; one slice -> `SliceLength`; else `Default` (:258-282, `natural_row` :290-299 = builtin carrier with nonnegative minimum).
5. Cases by shape: `satisfies` inherits a promise only if the requirement has a bare `terminates;` (:99-134); `measured_value` only for a measure with one statement with a value (:393); components of measure <= 64.
6. Same job elsewhere: `binder_terminates` (:636), `requirement_promises` (:99) and `callable_has_clause` (callable_effects.omg:311) all scan clause runs. The frame push/pop in `substituted_value` (:345-374) and `measured_value` (:399-437) repeats `fact_operand` (flow.omg:235-259).
7. Unused: `record_binder_call` caller checker.omg:1019; `reset_termination` caller checker.omg:274. `settled_termination` callers: callable_effects.omg:331, supply.omg:125, checker (1). None unused seen.
8. Stated limits: "Lexical parents scope declarations and never supply a hidden witness or parameter correspondence" (:33-34).
9. Fail-open: `terminating` is false whenever the graph is incomplete (`ranked = complete && component_ranked`, :533), so incomplete graphs refuse. A cycle with only a bare `terminates;` and no witness has `component_ranked == false` -> `terminating == false` -> `check_progress` (supply.omg:126-127) rejects: traced. `validate_rank_component` rejects only machines that wrote a witness (`termination.ranked`, :673); an unranked promise is rejected by `check_progress` instead. None seen.

---------------------------------------------------------------------------------------------------------------------------

## Coverage
Read in full, every line, in order (file: lines): arithmetic.omg 382; arrivals.omg 604; call_bindings.omg 192; callable_effects.omg 335; checker.omg 1561 (read as 1-1321 then 1322-1562); completions.omg 217; constructions.omg 470; contracts.omg 424; demand.omg 839;
domains.omg 606; evaluation.omg 674; fact_values.omg 203; facts.omg 463; flow.omg 1584 (1-1000 then 1001-1585); frames.omg 690; holds.omg 709; invocations.omg 753; loans.omg 1387 (1-1000 then 1001-1388); matches.omg 519; obligations.omg 1029; owned_places.omg 662;
ownership.omg 2160 (1-1100 then 1101-2161); places.omg 988; qualification.omg 483; ranking.omg 462; recasts.omg 112; reference_backings.omg 200; reference_origins.omg 288; reference_writes.omg 385; references.omg 454; refinement.omg 232; slot_review.omg 113; slots.omg 216;
snapshots.omg 348; supply.omg 356; termination.omg 694. Total read in full: **21,794 lines** (sum of the unit's 36 line counts). Skimmed: 0. Not read: none.
Greps run (Grep tool, `source/`): `place_relation\(`; `declared_causes\(`; a combined count and a combined `-o` listing for `array_extent|value_least|selected_case|index_steps|fact_operand|callee_parameters|through_write_only|distinct_indexes|binds_machines|drop_saved`;
`invocation_graph_(record|seal|component|initialize)\(|certify_ranking\(|observe_rank\(|edge_handle\(`; `decide_bound_facts\(`; `terminal_call\(|initialize_terminal_calls\(`; `path_count|set_path_count|natural_rank_value|compare_ranks|value_truth` listing; a "stated limits" keyword grep. Counts quoted above come from those. Items marked "callers seen in read" were not grepped.

## Five things a maintainer most needs to know about this unit
1. Everything is bounded fixed-size tables; every overflow is handled in one of three ways, and they differ in direction: reject `CheckCapacityExceeded` (places 256, subjects 64, bodies 128, moved/owed/loans 64, matches 8/16, contexts via `overflowed`), conservative answer (frame tables beyond 10240/1024/32768 answer "may write"), or silent drop (assumption stack 128 in `facts.omg:289`, pending fields 32, context table 64, arrival paths 40, local-use table 128 in `loans.omg:1138`).
2. The silent drops are the places to audit for acceptance: `push_assumption` is shared by `assume` (drop = fact loss, safe) and `proves` (drop = a goal not checked, `facts.omg:264-289, 333-412`), and the 128-local use table makes later locals' loans expire immediately (`loans.omg:1138-1155, 1186-1194`).
3. Walks run several times: fact-gathering walks (`checking=false`, no rejects) to a fixed point of arrival facts, then one checking walk (`checking=true`), then the ownership walk; crash routes come only from the checking walk (`checker.omg:405-406`). Reading any "when checking" test is essential.
4. Place identity is a (kind, handle, parent) triple in the checker's table (`places.omg:336`) and a place key `4n+1/2/3` in ownership (`owned_places.omg:237-239`); both are indexes into the current body, never names. "Same job" duplicates are numerous (four transition-arm walkers, three clause-run iterators, three "exclusive access" tests, two hold computations `argument_hold`/`lent_place`), so a fix in one often needs the others.
5. The invocation graph (`invocations.omg`) is the shared substrate for completion summaries, termination and ranking: incomplete or unsealed means "unavailable" and every consumer refuses (`invocation_graph_component` returns 0, `terminating` false), so graph capacity (10240 machines, 32768 edges, 131072 expressions) is a hard compile limit for the checker.

## What you are unsure about
- Whether the parser or typer caps expression depth and Boolean-tree size below the points where silent drops occur (`facts.omg` 128-goal stack, `owned_places.omg` 64-step place chains): I did not read those stages.
- `checker_width` `_ -> bits(64)` (`arithmetic.omg:376`): I did not read `foundation\is_carrier` to see which builtins can reach it.
- `check_holds` skipping `Binary` operator calls (`holds.omg:61`) and `refine_static_arguments` skipping non-`Apply` callees (`supply.omg:180`): I did not trace whether typing can produce those shapes with exclusive/machine arguments.
- Whether flow's "arms cover the subject, so the path after is infeasible" (`flow.omg:1325-1330`) holds for every transition typing admits (relies on `coverage.omg`, not read).
- Callers of a few pub machines (`snapshot_assignment`, `replace_loan_holder`, `restore_ranges`, `shared_parameter`) were taken from my reading, not from a grep count.
- The `aliases.omg` file in the same directory is not in this unit; calls to `forget_aliases`, `record_alias`, `alias_table_join`, `assume_alias`, `copy_alias`, `alias_replayable` are cited but their definitions (other than `alias_replayable`, `flow.omg:1021`) were not read.
- Zone operations (`zone_open`, `constrain`, `zone_join`, `zone_widen`, `zone_bound`, `zone_forget`) and `Bound`/`exactly`/`bound_add` are defined outside this unit and were treated as black boxes.
