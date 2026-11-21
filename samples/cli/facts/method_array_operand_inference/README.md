# Inferring the element type of a generic boundary method

`Bytes::last::<3u64>(@bytes)` supplies the count and leaves the element type
open. The caller passes a shared borrow of `[u8; 3]`, so the selected method
takes `@[u8; 3]` and returns a `u8`. Its body reads `bytes[Count - 1]`.
The root program calls that same provider with two `u16` values and checks that it returns `513`.
The explicit count stays fixed during inference. Passing the same three
elements to `Bytes::last::<4u64>` must fail at the argument.

This example exercises generic argument inference and Build's selection of a
named implementation under **MODULE-NAMESPACE-RESOLUTION**.
[Task](../../../../TASKS.md) · [Generic arguments](../../../../wiki/spec/language/generics.md) · [Provider selection](../../../../wiki/spec/build/provider_selection.md)

## Programs

The [root program](main.omg) checks that the last of 7, 11 and 13 is 13.
The nine pairs below vary how the array reaches the same call. Each invalid
program changes only the count supplied to the method.

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [positive](local/main.omg) | [Count 4](invalid_local/main.omg) | array copied into a second local |
| field | [positive](field/main.omg) | [Count 4](invalid_field/main.omg) | `Packet::items` between `before`/`after` canaries |
| parameter | [positive](parameter/main.omg) | [Count 4](invalid_parameter/main.omg) | `consume(bytes: [u8; 3])` makes the call |
| call_result | [positive](call_result/main.omg) | [Count 4](invalid_call_result/main.omg) | `make_bytes()` result |
| match_join | [positive](match_join/main.omg) | [Count 4](invalid_match_join/main.omg) | both arms join one `[u8; 3]`; each arm has its own last element |
| const_name | [positive](const_name/main.omg) | [Count 4](invalid_const_name/main.omg) | `const BYTES: [u8; 3]` |
| other_state | [positive](other_state/main.omg) | [Count 4](invalid_other_state/main.omg) | nested `arrived(bytes: [u8; 3])` |
| other_machine | [positive](other_machine/main.omg) | [Count 4](invalid_other_machine/main.omg) | `forward` relays to `consume` |
| target_body | [positive](target_body/main.omg) | [Count 4](invalid_target_body/main.omg) | Build selects `X86Bytes` or `ArmBytes`, the target entry checks tag 17 or 29 |

## What the programs check

- The requirement and its selected implementation use the same count and
  element type. The ordinary argument supplies the omitted type. The
  `T [copy]` bound permits the method to copy the selected element.
- The shared input stays live while the method reads it. A copy can contain a
  borrow, so checking the generic body must preserve where that element came
  from even before the caller supplies a concrete element type.
- The field program checks the neighbouring bytes 41 and 43 after the call.
  The match program checks both branches, whose last elements are 13 and 29.
- Every positive Build file binds an entry for all four hosted targets.
  In the target-specific program, the x86-64 and ARM64 entries check distinct
  tags before invoking the array method. Both array implementations perform
  the same indexed read.
- Each wrong-count call must report that four elements were expected and
  three were supplied. An unsupported-feature message would leave the
  mismatch untested.

The [inferred array equation](../inferred_array_equation_call/README.md)
examples cover a related case where the compiler infers a const argument
from a structural equation on a free machine. Here the count is explicit and
the compiler infers the element type at a boundary method call.
