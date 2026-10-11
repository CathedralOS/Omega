# Independent review

Reviewer: GPT-6.1 Sol, Medium, supported local collaboration reviewer
`/root/new077_review`, same executor; no implementation delegation or new executor.
Review was read-only. No Omega/Rust invocation or package acceptance by reviewer.

Two source/coverage findings were resolved by the primary:

* Mutable `job` was forwarded before later arguments read its fields. Staging
  width/plane, width/height and next_lane in locals preserves the formulas and
  follows `wiki/spec/language/expressions.md` argument evaluation order.
* Trailing-input rejection covered `x` but omitted a syntactically valid extra
  digit token. `data/invalid_extra_digit.in` now pins that distinction. Frozen
  valid fixture inputs, outputs, and `ORACLE.json` were not changed.

Final finding: no remaining blocking source issue after these corrections.
The reviewer independently reconstructed all nine exact regions and interval
oracles, enumerated output coverage/audits and verified the frozen hashes.
Native/Terminal execution was pending at initial source review. Primary execution
has since verified all 19 exact native cases, all 19 Terminal exits before
integration and three representative integrated Terminal cases. The bounds control
rejected with exactly its intended diagnostic. CLI `--both` compares termination;
the validator checks native stdout independently. No claim of Terminal stdout
equivalence is made.

Reviewed exact sources (SHA-256):

```text
381df559701c975a7bef156bd1ae9ac17fef993add9a2d6ee288d53ad55feca8  main.omg
8915394b3c0d5698c58da576bbfb620fc96bc6179ced51b1e794581d926bc9a7  build.omg
ebb87ffd08bff8ee1738e7ac6fae8a7afe6cca7e9080e442b24401b7df2587d0  domain/model.omg
be5c18a306ee065429bd17fa4d2fdd7df1e83b247eade9568123dc00add2d808  domain/input.omg
67e6ead530a6f3f88043c31dcf0857d450c35e390c39e199cf39d768d02f6700  domain/paths.omg
96236d70e6da22b948fb24d831e30e45af75f4ab990403f9da27b55f3488b8cf  domain/report.omg
15d259bca245d62e4cd1eaa704c126bad0eb0006cd7577962cd2ea8453f6b202  reference.py
e1517eaee4e8f5b86ec4a5ea957079f118d20dcc2ff64911793dd0e54d8b223c  controls/out_of_bounds/main.omg
a39d9f07282e7533163dcadfa2a35e8e70e67385025df8c5e9e6658ff2edec27  controls/out_of_bounds/build.omg
```

The same reviewer separately approved the row-driven stage03 input transport
repair, SHA-256 `428714a51e0597fdd203923ff0a5aa081357c181e1b9967f70d911b5134ed487`,
after tracing fixed-use custody, source lineage, early-clobber separation and
preservation of arithmetic obligations/fuel. The corrected complete runtime
regression `tests/omega/pass/arithmetic/reused_fixed_integer_operands_exit/main.omg`
has SHA-256 `57ae00fe41f401db2de04c0f4d6066a638086871925ff025538668f38ba59f36`.
Reviewer corrected the initial ByteRead/scalar API assumption; the primary added
an explicit EOF conversion while preserving all five frozen arithmetic values.
Nine focused native/Terminal controls and the separate divide-by-zero rejection
passed. Full retained-evidence and Library transfer review is recorded in the
final bundle, independently of these source-review findings.
