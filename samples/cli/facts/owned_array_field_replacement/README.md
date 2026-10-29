# Owned array field replacement

Replacing the owned array field with [7, 4] establishes its current [7, 4] contents while the earlier owned copy remains [3, 1].

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, whole-value record-field and
owned-array predicate handback with current storage versions. The earlier
saved-observation row preserves an unwritten 7; this row replaces both owned
array cells and establishes new current [7,4] contents while the earlier
captured array retains [3,1]. It is a different store mechanism, not a count
or width variant.

## Exact acceptance

Holder neighbors start at 11/13, its items at [3,1]. Capture the whole previous
array before install. Root install Requires both old original cells and
installed(&incoming); its only body statement is holder.items = incoming and
its Ensures is installed(&holder.items). Packet contains that completed Holder,
the earlier owned previous array, and a marker. Consume requires the complete
current installed predicate plus previous[0]==3 and previous[1]==1, and declares
its result==3. No caller equality guard supplies any of those four value facts.

Every transport carries the completed Packet after the real store. Returning
producers/forwarders declare exactly the four current/previous value facts;
target requirements and the selected implementation have the same contract.
Markers, like the 11/13 neighbors, are runtime canaries only: construction and
checks remain explicit, without marker Requires or Ensures. FIRST is a scalar constexpr 7 used in concrete array construction,
not a constant runtime Packet.

The runtime consumer executes before canary branches. Each canary checks
observed==3, current[0]==7, current[1]==4, previous[0]==3, previous[1]==1,
Holder before==11/after==13, and the expected marker. The match place executes
both selected paths and independently checks markers 11 and 13. These checks
trap omitted/partial field copies, old/new aliasing, wrong coordinates, lost
old cells, neighbor corruption and the wrong selected branch.

Each twin changes only install's RHS from incoming to [6,incoming[1]], retaining
all source inputs, contracts and runtime checks. It must reject install's OWN
Ensures with `cannot prove ensures contract for exit from install`. A formation,
access, incoming Requires or consumer failure is not the required falsifier.
Positive samples and pins expect exit 0 with native/Terminal agreement via
omega run --both. All four hosted Build entries are authored; actual execution
and diagnostic qualification remain with the implementation owner.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [source](local/main.omg) | [twin](local/invalid/main.omg) | Frozen local Packet is constructed only after install, then consumed before its runtime canaries. |
| field | [source](field/main.omg) | [twin](field/invalid/main.omg) | The completed Packet moves into Envelope.packet; consume and runtime checks project that stored Packet. |
| parameter | [source](parameter/main.omg) | [twin](parameter/invalid/main.omg) | The completed Packet moves into accept(packet: Packet), whose own entry requires the full current/previous fact before consume. |
| call_result | [source](call_result/main.omg) | [twin](call_result/invalid/main.omg) | produce returns the post-install Packet with all four current/previous facts in Ensures; the call result is consumed. Its marker is checked only at runtime. |
| match_join | [source](match_join/main.omg) | [twin](match_join/invalid/main.omg) | selected_packet joins completed produce(11)/produce(13) results and publishes all four current/previous facts. Main executes and consumes both arms, checking distinct markers only at runtime. |
| const_name | [source](const_name/main.omg) | [twin](const_name/invalid/main.omg) | FIRST names exactly 7 in the concrete [FIRST,4] install argument. The completed runtime Packet is ordinary storage, not a runtime constant. |
| other_state | [source](other_state/main.omg) | [twin](other_state/invalid/main.omg) | main terminal-transfers the completed Packet to a genuinely nested accept machine with its own full fact requirement; the nested body consumes before canaries. |
| other_machine | [source](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) | A separate forward machine receives the completed Packet and returns it with all four current/previous facts in Ensures. Its stored marker is checked only at runtime. |
| target_body | [source](target_body/main.omg) | [twin](target_body/invalid/main.omg) | Build selects an ordinary HostedPacketBody implementation for all four targets. Its producer performs the real install and returns the completed Packet with all four current/previous facts; its marker is checked only at runtime. |

Pins: tests/omega/pass/facts/owned_array_field_replacement_<place>_exit and
 tests/omega/fail/facts/owned_array_field_replacement_<place>_rejected.
Sources and Build inputs are copied byte-for-byte. Invalid sample metadata
classifies intended-invalid and pins the same install Ensures refusal.

## Requirements and evidence

- **Establishment:** old [3,1] construction establishes install's original
  precondition; [7,4] and the checked installed predicate establish incoming
  admission. Actual whole-field replacement establishes the current predicate,
  then consume receives all four facts. See [collection invariants](../../../../wiki/spec/language/dependent_values.md#collection-invariants).
- **Identity and lifetime:** previous is a whole owned-array capture, not a
  saved scalar or alias of the new home. The current record version changes
  while that captured value remains immutable. See [fact validity](../../../../wiki/spec/terminal-psi/fact_validity.md).
- **Composition:** owned field replacement and completed Packet transport use
  exact declaration paths and explicit arguments. Wrapper postconditions carry
  both current and previous facts, not only current element zero. Nested-machine
  arrival and both match arms owe their full contracts independently. See
  [normal returns](../../../../wiki/spec/language/state_contracts.md#returns-and-proof-use).
- **Soundness boundaries:** input formation, copy/move custody, writable access
  and every normal exit remain independent. Installed current [7,4] must not be
  inferred from the obsolete old [3,1] snapshot, and the wanted postcondition
  cannot justify its own store. Descriptor/reference reseating is a separate
  mechanism. See [structural access](../../../../wiki/spec/terminal-psi/structural_access.md).
- **Psi 05–07:** owned copies, whole-field store, records, returns, joins and
  nested transitions must retain normal semantics through canonical Terminal
  Psi. Predicate facts erase; both arrays remain independently represented.
- **Omega 00–02:** ordinary Holder/Packet layout and aggregate call/result
  placement must preserve both complete arrays and record neighbors. Target
  selection uses ordinary Build rows and a checked named implementation.
- **Omega 03–06:** native storage homes and call preservation must keep old and
  current contents separate. Runtime reads explicitly check every promised cell,
  both neighbors and markers after static consumption, including both match arms.
- **Omega 07–09 and emission:** emitted native images must execute with exit 0
  and agree with Terminal execution. Four authored hosted entries do not imply
  actual four-host execution; qualification records identify producer and target.

## Frozen source and open frontier

Fixed discovery base: f6f6bbb07c0ae50e280fe64a209a54e60745e03e. Local source
commit: 2c9b82ced44aaf878f0ae9cbd72eea5201f53386. The local positive and twin
are byte-identical to the initial-source-snapshot.json retained under
build/owned-array-field-handback-20261029/. That existing snapshot and the
fact-selection/carrier-review records remain unchanged.

Before authoritative integration or baseline observation of the eight added
places, fixture authoring removed their redundant marker proof clauses. The
consumer still requires the same four value facts; marker construction, both
match paths and every runtime canary are unchanged. The seven frozen local
inputs are untouched. The earlier sidecar draft is retained as a non-integration
authoring snapshot beside its manifest.

This is a source-only sidecar draft. The frozen-local exact-source baseline
build/observation is owned by the main session and was pending at dispatch;
no baseline refusal or candidate success is claimed here. All eight added
places, nine corpus pairs and all runtime outcomes remain unverified. Freeze
and observe the full authored sources before counting a compiler defect or
acceptance; retain exact source/producer hashes and own-obligation diagnostics.
The implementation owner completes all checking, execution, review and landing
within this same outcome. A drafted row does not close the parent task.
