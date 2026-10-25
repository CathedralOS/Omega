# Mutable slice writes preserve their extent

Writing an element through a mutable slice view (`&mut [u64]`) whose length is
guarded (`view.len >= 2u64`) proves safe indexed element mutation at
`view[1u64]` while preserving the slice extent lower bound (`view.len >= 2u64`)
to discharge exact subtraction `view.len - 2u64` across all nine places.

`storage_view(&mut backing)` returns the fixed-array field of a record as a
mutable slice. Its captured origin remains that field of the original record;
the array and slice descriptor do not acquire an equal type identity. Each
existing transport feeds the independently checked mutation helper, whose
only extent assumption remains `requires len >= 2`. It writes 45 at index 1
and returns `len - 2`. The caller observes 45, difference 1, the untouched
neighboring elements, and a separate guard word before the array. Both match
choices execute and check those observations separately.

On base `90f04fe2b584`, the returned-field composition in the root and all nine
positive places stops before execution with `statement sequence: reference
result`; their index-2 twins reject at `cannot prove index`. The unchanged
`borrows/returned_slice_record_reference_custody_exit` fixture also pins
record elements, nested calls and distinct input roots. Qualification records
bind the actual optimized producer, host, sources, outputs and controls.

## Places & witnesses

| Place | Witness (main.omg) | Reject twin (invalid/main.omg) | Note |
|---|---|---|---|
| root | [main.omg](main.omg) | - | Root package probe exercising mutable slice view write and extent subtraction |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | mutable slice view is transported through an intermediate local binding |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | mutable slice view is stored within a nominal record field (`Holder`) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | guarded mutable slice view is operated on directly through its parameter binding |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | helper call returns the guarded mutable slice view with preserved length contract |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | mutable slice view joins across conditional match branches |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constant `MIN_LEN: u64 = 2` defines the extent guard obligation |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | successor machine `active(view: &mut [u64])` receives and mutates the view |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | forwarding machine passes the mutable view through an intermediate call |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | target-scoped implementations across all four platforms perform mutation and extent verification |

Every positive, including the root, declares `Expected exit: 0` and binds
`ProgramEntry` for Windows x86-64, Linux x86-64, macOS ARM64 and Linux ARM64.
Native/Terminal execution in this repair is measured on macOS ARM64 only.
Other target bodies are checked as source; that is not native execution there.

```sh
omega run --both samples/cli/facts/mutable_slice_element_write_bound/<place>/main.omg
omega --check samples/cli/facts/mutable_slice_element_write_bound/<place>/invalid/main.omg
python3 tools/corpus_gate.py --both --filter facts/mutable_slice_element_write_bound
```

Each twin keeps its positive's calls and runtime observations and changes only
the helper's write from index 1 to index 2 (plus the expected-rejection comment).
Even though the caller supplies three elements, the helper must reject index 2:
its independently checked contract promises only `len >= 2`. No caller guard
is used to prove the write or subtraction. Corrupting the caller's observed 45
to 44 is a separate runtime discrimination probe, not a language reject twin.

### Pinned fixtures

Nine positive places and nine reject twins are pinned under `tests/omega/`. The
fixture family is `mutable_slice_element_write_bound_through_<place>_exit` /
`_unproved`, which does not spell the sample folder name:

| Place | Positive fixture | Reject twin |
| --- | --- | --- |
| local | [`..._through_local_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_local_exit/main.omg) | [`..._through_local_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_local_unproved/main.omg) |
| field | [`..._through_field_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_field_exit/main.omg) | [`..._through_field_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_field_unproved/main.omg) |
| parameter | [`..._through_parameter_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_parameter_exit/main.omg) | [`..._through_parameter_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_parameter_unproved/main.omg) |
| call result | [`..._through_call_result_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_call_result_exit/main.omg) | [`..._through_call_result_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_call_result_unproved/main.omg) |
| match join | [`..._through_match_join_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_match_join_exit/main.omg) | [`..._through_match_join_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_match_join_unproved/main.omg) |
| const name | [`..._through_const_name_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_const_name_exit/main.omg) | [`..._through_const_name_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_const_name_unproved/main.omg) |
| a nested machine | [`..._through_other_state_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_other_state_exit/main.omg) | [`..._through_other_state_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_other_state_unproved/main.omg) |
| another machine | [`..._through_other_machine_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_other_machine_exit/main.omg) | [`..._through_other_machine_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_other_machine_unproved/main.omg) |
| another target body | [`..._through_target_body_exit`](../../../../tests/omega/pass/facts/mutable_slice_element_write_bound_through_target_body_exit/main.omg) | [`..._through_target_body_unproved`](../../../../tests/omega/fail/facts/mutable_slice_element_write_bound_through_target_body_unproved/main.omg) |

The recorded goldens carry this family on both legs:
`corpus_native_macos_arm64.txt` holds the nine `built ... exit:0` rows and
`corpus_outcomes.txt` the check leg's nine `checked` plus nine `rejected` rows,
the rejects pinning `cannot prove index` for the index-2 write.
`python3 tools/corpus_gate.py --filter facts/mutable_slice_element_write_bound`
reports 0 rejected without its expected fragment and 0 fail fixtures that did
not reject.

## Requirements and evidence

Parent: **TERMINAL-SLICE-VIEW-VOCABULARY** in [TASKS.md](../../../../TASKS.md).
This regression repair establishes returned collection-region provenance for
mutable scalar and record element slices. The ongoing **CML4** assignment
resumes afterward; this does not close its remaining cleanup acceptance.

- **Establishment:** The getter's fixed-array field establishes its result
  length. The mutation helper consumes `requires len >= 2` directly for index 1
  and exact subtraction by 2. See [typed slice operations](../../../../wiki/spec/terminal-psi/byte_views.md#element-typed-slice-views).
- **Identity and lifetime:** The source loan retains its root, field path,
  mutable access and captured formation. A returned slice is contained in that
  collection; its descriptor supplies the actual address and length. No copy of
  the referent, selector replay or type-only loan is substituted. See
  [structural access](../../../../wiki/spec/terminal-psi/structural_access.md) and
  [loans](../../../../wiki/spec/terminal-psi/loans.md).
- **Composition:** The same fact survives all nine transports. The match place
  exercises both choices; the pinned record-element fixture adds nested calls
  and separate actual roots. Caller observations discriminate writeback and
  preserve untouched fields rather than establishing the static bound.
- **Soundness:** Index-2 twins still reject the missing bound. Separate controls
  retain local-escape rejection, shared-to-mutable refusal and all possible
  backing loans. Representation-view controls retain their admitted codecs.
- **Psi 05–07:** Returned descriptors and their checked region relation survive
  canonical Terminal Psi lowering; ordinary Terminal execution must agree with
  each authored native result. This is not checked-tree interpretation.
- **Omega 00–02:** Hosted entry bindings, record-field geometry and descriptor
  call/return placement are consumed by the ordinary macOS ARM64 realization.
  Guard and neighbor checks discriminate incorrect field placement or copying.
- **Omega 03–06:** The host backend realizes indirect element writes, subtraction,
  calls and state transfers while retaining original backing through returns.
  These runs do not independently inspect particular register assignments.
- **Omega 07–09 and emission:** The ordinary pipeline emits and executes Mach-O
  images; native exit 0 and Terminal exit 0 are the runtime oracle. PE/ELF
  emission and execution on other hosts are not established by these runs.

## Remaining scope

This row does not establish general dynamic-index proofs, nested mutable view
carriers, every subslice composition, cross-host native execution or full
CML4 cleanup acceptance. Those remain with their owning board requirements.
