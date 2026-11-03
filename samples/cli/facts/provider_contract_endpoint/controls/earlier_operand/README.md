# Form arithmetic after the earlier provider result

Even when a constant is never read, the compiler must check the calculation
that defines it. This example adds one to a provider's result. The compiler can
check that addition once the call returns, before the addition runs.

The declaration is `const selected: u64 = Endpoint::upper<13u64>() + 1u64`.
The selected provider returns seven. Its exact captured result must justify the
addition before that addition executes; the declaration cannot skip formation
because no executable expression reads `selected`. This does not grant an
ordinary runtime call a postcondition absent from its selected contract.

The [program](main.omg) and [Build configuration](build.omg) are byte-for-byte
copies of the frozen passing R6 probe, including its original header. The
[retained main](../../../../../../tests/omega/pass/facts/provider_contract_endpoint_control_earlier_operand_exit/main.omg)
and [retained Build configuration](../../../../../../tests/omega/pass/facts/provider_contract_endpoint_control_earlier_operand_exit/build.omg)
match those same bytes. The [exit oracle](../../../../../../tests/omega/pass/facts/provider_contract_endpoint_control_earlier_operand_exit/expected_native_exit.txt)
is zero, and Build binds all four hosted entry targets.

This control belongs to **CANONICALIZE-SCALAR-RANGE-CONTRACTS** and the existing
provider endpoint fact. Its preservation requirement is to distinguish a later
operation's captured-operand obligation from a prerequisite for starting the
whole evaluation entry. The governing rules are [invocation admission](../../../../../../wiki/spec/language/evaluation.md#invocation-admission),
[exact arithmetic](../../../../../../wiki/spec/language/numeric_values.md), and
[constant declarations](../../../../../../wiki/spec/language/constants.md).

The immutable optimized R6 producer, source `0f7da6f1532`, accepted the ignored
probe's source and the public run observed native exit zero and Terminal exit
zero on macOS ARM64. The raw receipts are
`prerequisite-scout-r6-earlier-call-plus-one-unused-{update,check,both}.json`
in the claim evidence. Their exact main/Build hashes also identify this tracked
copy. This is applicable retained evidence, not a new run of the tracked paths
or the current candidate compiler.

Main still checks the original bound seven and exact `7u64 - value` result. It
does not read `selected`, so the runtime oracle does not directly observe eight.
The unused declaration's required formation is the source-admission control;
main's ordinary exit check is the runtime oracle. No claim of direct runtime
materialization of eight is made. The separate attempted consumer that compared
`selected` with eight refused in R6 at an arithmetic proof in main; that failure
remains preserved and is not substituted for this supported source.

The optimized R12 compiler checks the tracked control and observes native exit
zero and Terminal exit zero on macOS ARM64. Its exact inputs and raw results are
retained under `pending-admission-r12-integrated-earlier_operand-positive-*` in
the claim evidence. This confirms the unused declaration is still checked; it
does not add a runtime observation of eight. Execution on other hosted targets
and broader scalar-range acceptance remain unverified.
