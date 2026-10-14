# Erased array evidence

A checked record containing a fixed array crosses an erased formal and is
forwarded to another call alongside standalone array evidence. The retained
runtime argument remains 70. Expected native and captured Terminal exit: 70.
The runtime oracle is unchanged from `erased_record_parameter_exit` and does
not inspect or depend on the evidence.

Run `omega update --project samples/cli/erased_array_evidence --target linux_x86_64`,
complete this checkout's package review, then run
`omega run --both samples/cli/erased_array_evidence/main.omg`.

`python3 tools/validate_erased_array_evidence.py runtime /absolute/path/to/omega`
checks the separately captured native and Terminal statuses and empty output.
The `sources` mode checks the related positive and invalid fixtures;
`inspection` observes their retained evidence and separate runtime signatures.

The base compiler at `763aa6f057109d9d4eb1b74f3746e178d286917d` rejects
array-bearing erased formals because they have no contract-term representation.
Array evidence must retain its exact element/extent identity and ordered values,
including through records, calls and state transitions, without runtime storage.
