# Field-scoped arrival premises in a call component

The mutually recursive machines count down under `saved <= limits.ceiling`.
The subordinate state writes `limits.observed`, which is disjoint from the
observed ceiling. The caller must receive result 70, retain ceiling four and
observe the write of one. Native and Terminal execution must both exit zero.

```sh
omega run --both tests/omega/pass/termination/runtime_rank_arrival_field_premise_disjoint_write_exit/main.omg
python3 tools/corpus_gate.py --both --filter termination/runtime_rank_arrival_field_premise_disjoint_write_exit
```

Normal local package/entry review applies to the public command. No broad
acceptance policy is included. A mechanical probe moves the sibling write to a
checked helper with a renamed parameter; both engines still observe it. The
maintained program uses the direct store. This exercises the shared exact-field
premise roster rather than a separate whole-record protection rule.
Changing that store to `limits.ceiling = 0` refuses both the next call's
requirement and the component's entry-relative rank proof.
