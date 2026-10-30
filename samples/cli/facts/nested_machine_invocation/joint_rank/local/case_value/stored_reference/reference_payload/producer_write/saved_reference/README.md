# Saved reference and reseated descriptor

Owner: `NESTED-MACHINE-UNIFICATION` in `TASKS.md`. This continues the same
[producer-written reference-payload rank fact](../README.md).

A private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.

The producer writes 7 or 0 to the original Leaf, moves its mutable reference
through `current` into `saved`, reseats `current` to a distinct Leaf, and writes
count 19 and stamp 929 through that reseated descriptor. It returns `saved` inside Pending.
The public result bound remains 7; the cycle consumes it directly without a
caller guard rebuilding the bound. The original Leaf begins with count 55 and stamp 413; seven
iterations take the counter from 3 to 10, and the empty path preserves 10. Callers check the
original leaf, guard 827, canary 641 and the replacement's count 19 and stamp 929. Payload
observations finish before the replacement is inspected, respecting the public
lifetime's possible-origin loan frontier.

Each of the nine sibling places has a twin changing only the producer write from 7 to 8.
Both source checking and the own-Ensures diagnostic are frozen against main
`71d80181a45253213d2f167b812ebf4e0ff902a9`: all nine valid positives initially
refused `input` statement 6, as do the deliberately invalid twins. The exact
source/Build hashes and optimized baseline identity are retained in
`build/saved-reference-postcondition/`.

Acceptance is all nine positives running with native/Terminal agreement and
all nine fact-sensitive twins rejecting at the producer. It includes precise
snapshot/backing identity and effect invalidation, independent case and loan
formation, canonical Terminal production and cleanup, ordinary target layout/
ABI realization, native instruction/storage realization and emitted-image
execution. Omitting or redirecting the reseat/write changes the observed
replacement or original backing and makes main trap. Other-host runtime
acceptance and the parent capability's other requirements remain open.


The implementation captures completed reference suppliers for invalidation,
retaining current deeper pointer loads without depending on a moved descriptor's
old home. Source ownership records exclusive carrier moves, and receiving
storage retains the exact pre-disposal vacancy for cleanup. Shared copies,
borrowed ingress, explicit reborrows and writes to the referent stay distinct.
Unknown storage geometry grants no borrowing or vacancy.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](main.omg) | [invalid](invalid/main.omg) |
| field | [program](../../../../../../field/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../field/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |
| parameter | [program](../../../../../../parameter/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../parameter/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |
| call_result | [program](../../../../../../call_result/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../call_result/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |
| match_join | [program](../../../../../../match_join/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../match_join/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |
| const_name | [program](../../../../../../const_name/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../const_name/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |
| other_state | [program](../../../../../../other_state/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../other_state/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |
| other_machine | [program](../../../../../../other_machine/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../other_machine/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |
| target_body | [program](../../../../../../target_body/case_value/stored_reference/reference_payload/producer_write/saved_reference/main.omg) | [invalid](../../../../../../target_body/case_value/stored_reference/reference_payload/producer_write/saved_reference/invalid/main.omg) |

Ordered guards use completed reference storage rather than replaying a saved
initializer after its selectors change. The [unchanged-index runtime control](../../../../../../../../../../../tests/omega/pass/borrows/reference_selector_guard_current_index_exit/main.omg) and
[rebound-index reject control](../../../../../../../../../../../tests/omega/fail/borrows/reference_selector_guard_rebound_index_rejected/main.omg) pin this identity:
a guard on the saved element cannot prove subtraction on a later selected element.
