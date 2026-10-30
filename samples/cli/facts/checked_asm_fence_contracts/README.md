# Checked fence contracts

Owning task: **CHECKED-ASM-LABEL-CONTRACTS**, catalog and transitive plans.

Checked fence sequences retain their catalog target, ordering and exact empty clobber set while carrying input 37 to a result requiring 37.

The x86 bodies execute LFENCE, SFENCE, MFENCE in that order. The AArch64
bodies execute full-system DMB LD, ST, SY in that order. These retain distinct
catalog identities and access masks; DMB LD also orders later stores and is
not equated with LFENCE. All four hosted targets bind `ProgramEntry`.

The root and nine places call a consumer requiring exactly 37, before their
runtime equality oracle. Each `invalid/` changes only the declared clobbers:
rax on x86, x0 on AArch64. Both are invented changes for this catalog sequence,
so every twin must reject for the exact clobber-union obligation. The sequence,
bound, scalar guarantees and consuming operation are otherwise byte-identical.
Pins are `tests/omega/{pass,fail}/facts/checked_asm_fence_contracts_through_*`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| root | [source](main.omg) | [invented clobber](invalid/main.omg) |
| local | [source](local/main.omg) | [invented clobber](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invented clobber](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invented clobber](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [invented clobber](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [invented clobber](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [invented clobber](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [invented clobber](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [invented clobber](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [invented clobber](target_body/invalid/main.omg) |


- Establishment: the caller proves input == 37; entry and exit assertions are
  goals. Each catalog operation preserves that exact value and changes no
  general register or flag. Its ordering event remains a separate obligation.
- Identity, lifetime and composition: each place carries the same scalar
  subject through ordinary fields, parameters, returns, joins, constants,
  nested machines, non-nested machines and selected target bodies. Helpers
  retain the ordering events; a join executes only its selected branch.
- Terminal realization: exact target and fence kind cross the canonical wire.
  Execution requires the receiving host's explicit private sequential memory
  model, records each completed fence in order, and retains progress across
  fuel pauses. Missing supply and mismatched targets must reject before effects.
  This model supplies neither shared/device memory nor synchronization proofs.
- Native realization: each selected kind has an independent byte decoder and
  a catalog-ordering footprint. The final span must retain the exact form,
  unchanged stack, empty clobbers and external-effect barrier. Tests must reject
  missing, doubled, truncated, substituted and wrong-target fence bytes.

Baseline f5d7c12b93bbd27d09cfa4ba8d129a7bbbc63ad2 accepted the known x86 source
but refused `omega run --both` with no executable body at the fence call. That
source/oracle and optimized producer are retained in the claim qualification.
Optimized producer `3020fc62f3be` executes the root and nine pins with native
exit 0 and Terminal exit 0 on Linux x86_64. Each pin also interprets all four
hosted target installations with the exact three-event stream (six events for
the two match-join calls), explicit missing/wrong-model refusal, and unchanged
progress across fuel pauses. The qualification retains these observations and
their exact producer identities. Foreign native execution remains unobserved.

The parent remains open for the other catalog families, hardware-effect linear
retirement, consumer-owned PortIo settlement, supplied device/interrupt/idle
models, external entry and customer canaries. Hosted execution observes Linux
x86_64; foreign target interpretation and emitted images do not establish foreign
native execution, hardware timing, device completion or multi-core ordering.

The supplied model applies to the ordinary hosted runtime's constructed private
entry RAM and invocation-owned storage. Unsupported placed ingress and host
realizations refuse; native record adapters refuse reference/atomic/device
carriers, reference outputs, callbacks and additional service reach. Their
serialized private sponsor still relies on the existing admitted native-provider
contract. This is simulated ordering under those premises, not evidence about
concurrent native providers or external memory. The governing instruction and
environment rules are [checked assembly](../../../../wiki/spec/language/assembly.md).

Additional local controls pin false exit goals, an invented clobber in an
unselected ARM body, mixed and wrong-body ISA demands, unsupported DMB scopes,
missing/numeric/extra DMB operands, and an operand on LFENCE. These are source
formation or contract refusals; malformed operands do not prove a hardware
ordering or ownership obligation.
