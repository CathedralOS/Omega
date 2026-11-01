# Borrowed Build configuration

Owner: **TERMINAL-BUILD-SEED** in [TASKS.md](../../../../TASKS.md).

A borrowed configuration preserves exclusive access to the original Build
activation, so binding through its stored view selects the authored entry
without copying or replacing the activation.

`Configuration` holds `&mut Build`, not an owned activation. Its ordinary
construction, projection and helper calls must retain the compiler-issued
receiver. Each Build binds all four hosted ProgramEntry slots through that
stored view. The target-body place binds `SelectedEntry::enter`, an ordinary
named implementation member, through the same record.

| Place | Positive | Readonly control |
| --- | --- | --- |
| Local | [source](local/main.omg) | [control](local/invalid/main.omg) |
| Field | [source](field/main.omg) | [control](field/invalid/main.omg) |
| Parameter | [source](parameter/main.omg) | [control](parameter/invalid/main.omg) |
| Call result | [source](call_result/main.omg) | [control](call_result/invalid/main.omg) |
| Match join | [source](match_join/main.omg) | [control](match_join/invalid/main.omg) |
| Const name | [source](const_name/main.omg) | [control](const_name/invalid/main.omg) |
| Nested machine | [source](other_state/main.omg) | [control](other_state/invalid/main.omg) |
| Other machine | [source](other_machine/main.omg) | [control](other_machine/invalid/main.omg) |
| Selected implementation | [source](target_body/main.omg) | [control](target_body/invalid/main.omg) |

Each folder's `build.omg` contains the Build transport. The constant place
names scalar configuration and store operands; the activation itself is issued
at runtime. Every control changes only the stored Build reference from exclusive
to shared. Direct binding rejects the missing mutable receiver; the match and
constant places reject their attempted setting writes through that shared view.
The control's `sample.json` pins its precise obligation.

Every product also exercises the corresponding ordinary carried-loan route.
Two calls select different original cells. Each writes 41, observes that write,
then writes 73. The caller checks the selected cell, the unselected cell and
four nonzero neighbor canaries. An inverted selection, omitted write or wrong
field displacement traps. This observes writes through live caller-owned storage.

## Reproduction and retention

```sh
omega run --both samples/cli/facts/build_activation_borrow/field/main.omg
python3 tools/corpus_gate.py --both --filter facts/build_activation_borrow,build/activation_owned_field_refused,build/activation_borrowed_field_replacement_refused
```

The unchanged minimal field program rejects on `aea6289579e399b931161740f7ffd6f770c78830`
with `the compiler-owned Build activation cannot be stored in field builder`
(the diagnostic quotes the field name). A helper-returned original builder
already runs on that base; the defect is the field admission rule.

The positive and negative projects are copied into
[`tests/omega/pass/facts/build_activation_borrow`](../../../../tests/omega/pass/facts/build_activation_borrow)
and
[`tests/omega/fail/facts/build_activation_borrow`](../../../../tests/omega/fail/facts/build_activation_borrow).
Additional controls retain rejection of
[owned Build fields](../../../../tests/omega/fail/build/activation_owned_field_refused)
and
[whole-activation replacement through a borrowed field](../../../../tests/omega/fail/build/activation_borrowed_field_replacement_refused).
A [reference-rebind control](controls/reference_rebind/main.omg) retains legal
shared-view descriptor assignment inside an ordinary helper. Its product
selects a different cell through the rebound view and checks both referents
and their neighbors. Existing counterfeit-local controls remain unchanged.

## Requirements and evidence

- **Establishment:** only the admitted Build entry receives the activation.
  A record stores its explicit borrow. No constructor or constant manufactures
  an activation, and the normal binding host still authenticates the original
  receiver.
- **Identity and lifetime:** the record carries the exact borrowed backing and
  access under [carried-loan transport](../../../../wiki/spec/language/lifetimes.md#carried-loan-transport).
  Shared references cannot authorize the binding or setting writes. Owned
  activation storage, copying and construction remain forbidden.
- **Composition:** locals, nested records, owned carrier parameters, returned
  carriers, joins and nested/ordinary calls compose without a Build-specific
  carrier shape. Constants supply scalar operands. The selected implementation
  remains an ordinary exact entry operand.
- **Soundness:** the integrity pass distinguishes a borrow from owned storage.
  Whole-value assignment uses the shared assignment receiving-type judgment,
  rather than a separate name-only recognizer. A reference-valued field store
  rebinds its descriptor; an owned Build replacement remains forbidden.
  Ordinary loan checking still owns exclusivity, access and lifetime; the
  runtime binding host retains its issuer check.
- **Psi 06–08:** ordinary reference-carrying records and helper completions lower
  to Terminal Psi. The product observes both selection paths and both writes;
  readonly controls reject before execution.
- **Omega 00–02:** the executed Build companion binds the exact entry for each
  authored hosted slot. Carrier fields and calls use ordinary layout and ABI
  planning; selecting a named implementation supplies no hidden receiver.
- **Omega 03–06:** caller-visible writes and neighbor checks exercise reference
  homes, field projection and preservation across calls. No write is confined
  to a dying temporary.
- **Omega 07–09:** direct Linux x86-64 ELF execution is compared with Terminal
  interpretation under the authored exit-zero oracle. The four hosted bindings
  are source coverage, not four matching-host execution observations.

Producer identities, commands, raw diagnostics and actual engine outcomes are
retained in the change's validation record. The parent outcome remains open:
remaining Build operations, live-storage accounting, admitted host loan returns,
staged dependency/provider preparation and the other matching-host acceptance
are broader than this borrowed-configuration repair.
