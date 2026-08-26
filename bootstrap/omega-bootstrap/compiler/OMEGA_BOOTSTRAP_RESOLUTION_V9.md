# Omega-bootstrap normalized complete provider-plan witness, schema major 9

`OMGRSW9` is the frozen structural witness for the bounded checkpoint-000001
`Console` provider-plan relation. It consumes only a valid
[`OMGCOMP3`](OMEGA_BOOTSTRAP_COMPILATION_V3.md) Linux-x86-64/native envelope,
uses its explicit authoritative-build source role, and retains one exact
`BuildOverride` selection of `ConsoleNativeProvider` for `Console`.

This witness is not checked IR, an intrinsic-catalog receipt, provider
admission, package or compilation authority, or an executable artifact. It
never derives authority from `provider_defaults`, a readable filename or
machine name, declaration order alone, or uniqueness of a covering candidate.

## Source relation

The envelope has two packages, four independently lexed sources, and one
requester-local direct alias from the root package to the standard-library
package. The standard-library sources share one resolver-owned `console`
module. The root build and entry sources share one resolver-owned `app` module.
Source labels are opaque custody data. The explicit OMGCOMP3 source-role bit,
not a label, identifies the build source.

The resolver classifies the other roles from parsed declarations, package and
module ownership, and the explicit root identity. Source IDs, readable labels,
top-level declaration order, comments, and whitespace are not semantic role
selectors. Reordering independent portable declarations or target leaves
rebuilds affected spans and source-order realization ordinals.

The authoritative source contains one free `build(builder: &mut Build)`
machine. Its first statement is one `builder.application(...)` role and its
second and final statement is exactly
`builder.select_provider<Console, ConsoleNativeProvider>()`. The static
arguments resolve to the exact boundary trait and nominal provider below. A
selection elsewhere, another selection, a default, or a unique-candidate
fallback rejects.

The portable source contains `ByteRead`, the public boundary trait `Console`,
the nominal `ConsoleNativeProvider`, the ranked `console_write_bytes` helper,
and exactly two checked adapters. Requirements, in declaration order, are:

| ID | Requirement | Result | Candidate |
|---:|---|---|---|
| 0 | `write_line(text: &[u8])` | Unit | checked adapter `write_line` |
| 1 | `write(text: &[u8])` | Unit | checked adapter `write` |
| 2 | `read_line(out_line: &mut [u8])` | Unit | Linux-x64 intrinsic leaf |
| 3 | `read_byte()` | `ByteRead` | Linux-x64 intrinsic leaf |
| 4 | `write_byte(byte: i32)` | Unit | Linux-x64 intrinsic leaf |
| 5 | `exit_process(return_code: i32)` | Unit | Linux-x64 intrinsic leaf |

Every requirement and realization has exactly `reaches Console`. The checked
adapters have bodies, no `via`, and call the one helper with `false`/`true`.
The helper has `(Console, &[u8], bool)`, the exact private ranking
`terminates by bytes -> Slice::Length`, and the recurrent guarded head/tail
body with two calls to the exact `Console::write_byte` requirement. The four
target-qualified candidates are bodyless and have exactly
`via Binding::CompilerIntrinsic`. Other targets are inert and absent from this
focused input.

The root app retains six requirement calls separately from provider bindings:
the helper's two `write_byte` calls plus app calls `read_byte`, `write_byte`,
and two `exit_process` calls. The two adapter-to-helper calls are ordinary
calls. A requirement call is never rewritten to a realization ID.

## Identity, header, and bounds

All integers are little-endian. `NO_ID` is `0xffffffff` only in an explicitly
optional identity field. Source spans are relative to their named source's
content extent and align to independently lexed token boundaries. Name spans
cover the identifier token. Invocation spans begin at the resolved callee-name
token (`select_provider`, `write_byte`, and so on) and end after its matching
`)`; receiver syntax is checked source context but is not duplicated in the
span. Body spans include both braces.

The exact identity is magic `OMGRSW9\0`, major 9, minor zero, flags zero,
header size 144, and exact total length 2,304 bytes. The header is
`8s + 4*u16 + 32*u32`:

```text
16 total_length = 2304                 20 input_envelope_length
24 unit_count = 4                      28 normalized_type_count = 8
32 boundary_trait_count = 1            36 requirement_count = 6
40 requirement_parameter_count = 5     44 service_reach_count = 6
48 provider_count = 1                  52 helper_count = 1
56 checked_adapter_count = 2           60 candidate_count = 6
64 candidate_parameter_count = 7       68 build_machine_count = 1
72 provider_selection_count = 1        76 provider_plan_count = 1
80 provider_plan_row_count = 6         84 requirement_call_count = 6
88 ordinary_call_count = 2             92 authoritative_build_source_id
96 selected_root_source_id              100 selected_target = 1
104 selected_configuration = 1         108 selected_plan_id = 0
112 selected_trait_id = 0              116 selected_provider_id = 0
120..143 reserved = zero
```

The inherited OMGCOMP3 transport ceiling selects 252. Any unsupported,
malformed, noncanonical, incomplete, ambiguous, cross-version, or semantically
inconsistent relation selects 251. No bytes are written unless the whole
relation passes. The fixed witness itself is below the 524,288-byte resolver
publication ceiling.

## Tables

Tables occur in the order below. `flags` and enum fields admit only listed
values.

1. **Unit, 28 bytes, `7I`:** `id, source_id, owner_package_id,
   module_string_id, flags, source_start, source_length`. Flags are build `1`
   and root `2`; the two ordinary units have zero. `source_start` is zero and
   `source_length` is the exact content extent. Rows are dense source-ID order.
2. **Normalized type, 24 bytes, `I B B H 4I`:** `id, kind, flags,
   reserved, declaration_source, declaration_id, payload0, payload1`. Kinds in
   order are Unit 0, full i32 1, full u8 2, bool 3, nominal `ByteRead` 4,
   shared byte view 5, mutable byte view 6, and boundary service `Console` 7.
   Unit/scalars use `NO_ID` declarations; i32 payloads are
   `0x80000000,0x7fffffff`, u8 `0,255`, bool `0,1`. Nominals name their exact
   declaration source/ID. View payload0 is type 2 and payload1 zero. Console
   names trait 0. Flags are zero.
3. **Boundary trait, 32 bytes, `8I`:** `id, source, owner_package,
   name_start, name_length, requirement_start, requirement_count, flags`.
   Flags public `1` plus boundary `2`; the sole row has flags 3.
4. **Requirement, 48 bytes, `12I`:** `id, trait, ordinal, source,
   name_start, name_length, parameter_start, parameter_count, result_type,
   reach_start, reach_count, flags`. Unit result is type 0. Flags are zero.
5. **Requirement parameter, 24 bytes, `6I`:** `id, requirement,
   ordinal, type, name_start, name_length`. Rows follow requirement order and
   omit requirement 3, which has no parameter.
6. **Service reach, 24 bytes, `6I`:** `id, requirement, trait,
   source, name_start, name_length`. One exact Console reach per requirement.
7. **Provider, 24 bytes, `6I`:** `id, source, owner_package,
   name_start, name_length, flags`; flags 1 means nominal data provider.
8. **Helper, 48 bytes, `12I`:** `id, source, name_start, name_length,
   console_type, bytes_type, newline_type, ranking_parameter_ordinal,
   ranking_kind, reach_trait, body_start, body_length`. The sole row uses types
   7, 5, 3; ordinal 1; ranking kind 1 (`Slice::Length`); trait 0.
9. **Checked adapter, 52 bytes, `13I`:** `id, source, provider,
   requirement, name_start, name_length, console_type, argument_type, helper,
   ordinary_call_id, body_start, body_length, flags`. Rows are source order:
   adapter 0 is `write`/requirement 1/ordinary call 0; adapter 1 is
   `write_line`/requirement 0/ordinary call 1. Types are 7 and 5; flags checked
   body `1`.
10. **Candidate, 56 bytes, `14I`:** `id, kind, source, provider,
    requirement, implementation_id, name_start, name_length, parameter_start,
    parameter_count, result_type, reach_trait, target, binding`. Rows follow
    requirement order. Kinds/bindings are checked `1` or compiler intrinsic
    `2`; checked rows target 0 (portable), intrinsic rows target 1. Candidates
    0 and 1 reference adapters 1 and 0 respectively; candidates 2..5 reference
    dense selected-target realization ordinals 0..3.
11. **Candidate parameter, 24 bytes, `6I`:** `id, candidate,
    ordinal, type, name_start, name_length`. Rows are candidate order:
    candidate 0 `(Console,&[u8])`, candidate 1 `(Console,&[u8])`, candidate 2
    `(&mut[u8])`, candidate 4 `(i32)`, candidate 5 `(i32)`; candidate 3 has
    none. Thus the dense types are `7,5,7,5,6,1,1`.
12. **Build machine, 40 bytes, `10I`:** `id, source, name_start,
    name_length, application_start, application_length, selection_start,
    selection_length, flags, reserved`. Flags 1 means free build machine.
13. **Provider selection, 36 bytes, `9I`:** `id, source,
    build_machine, trait, provider, call_start, call_length, provenance, flags`.
    Provenance 1 is `BuildOverride`; flags zero.
14. **Provider plan, 36 bytes, `9I`:** `id, provider, trait, target,
    selection, row_start, row_count, origin_package, flags`. Flags complete `1`.
15. **Provider-plan row, 24 bytes, `6I`:** `id, plan, ordinal,
    requirement, candidate, binding`. Six rows follow requirement order.
16. **Requirement call, 44 bytes, `11I`:** `id, source,
    caller_kind, caller_id, requirement, call_start, call_length, receiver_type,
    argument_count, result_type, flags`. Caller kinds are helper 1 and app 2.
    Rows are helper `write_byte` twice, then app `read_byte`, `write_byte`, and
    `exit_process` twice, each in source order. Receiver type is 7; result is
    type 4 only for `read_byte`, otherwise type 0. Flags zero.
17. **Ordinary call, 36 bytes, `9I`:** `id, source, caller_kind,
    caller_id, helper, call_start, call_length, argument_count, flags`. Caller
    kind 1 is adapter. Rows follow adapter source order and each has three
    arguments and flags zero.

The input envelope length and every source/package/string/span identity remain
input-derived. Readable spellings never repair a mismatched exact identity.

## Producer

[`omega-bootstrap-provider-plan.alp`](omega-bootstrap-provider-plan.alp)
accepts the OMGCOMP envelope on stdin, parses declarations, signatures,
reaches, bodies, selections, candidates, and calls by structural productions,
writes the witness on stdout only after complete validation, and returns
0/251/252. It does not select by a source digest, whole-file token census,
fixed token ordinal, or readable source label. The historical resolver remains
the producer for OMGRSW1/2/3/4/6/7/8 and retains its frozen selectors and bytes.
