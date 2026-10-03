# Foreign generic case selection

Two borrowed foreign `Optional<T>` instances must select cases using each
subject's retained generic application, including an empty second instance.
The payload types deliberately have different layouts. This graduates
`pending/generics/foreign_case_instance_identity`: the helpers still return
case-specific constants, isolating membership identity from payload reads.

```sh
omega run --both tests/omega/pass/generics/foreign_case_membership_dispatch_exit/main.omg
python3 tools/corpus_gate.py --both --filter generics/foreign_case_membership_dispatch_exit
```

Expected native and checked-interpreter exit: 70. Before referent normalization,
checking rejected the two instances with exact-declaring-type and
payload-classifier-as-constructor diagnostics. The stronger direct payload-read
program now also runs unchanged under `pass/generics/foreign_case_payload_reads_exit`.
The public command also requires ordinary package review; corpus evidence does
not manufacture or substitute that approval.
