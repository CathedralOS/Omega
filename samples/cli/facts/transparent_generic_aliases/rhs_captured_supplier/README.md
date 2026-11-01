# Captured alias RHS supplier

Fact: A default trait member keeps an alias's captured const supplier separate from its same-named fresh invocation binder while forwarding the same array value.

The imported `Selected<3>` supplies `Padded<Count>` as the trait’s type argument.
Its default member introduces a fresh Const `Count=17`, while the copied alias
keeps captured `Count=3` and declaration `Bias=1`, separate from caller `Bias=9`.
The same four-byte array retains 17/42 through forwarding, explicit/alias
annotations and borrowing. The sole twin changes `Selected<3>` to `Selected<4>` with
the unchanged four-byte input and rejects required extent 5 versus actual 4.

[Actual run 41360110833](https://github.com/CathedralOS/Omega/actions/runs/41360110833)
at source `24d35f3b86c4118a0c153f412634479b3c47c10d` passed the sample pair
and its canonical mirrors on hosted Linux x86-64: source checked; native exit 0,
Terminal exit 0 and agreement. The checking rows retain their reported default
check target; execution is Linux x86-64. CLI SHA-256
`d57f10132ae6c18bde62181e66970bdfc8f7333892a597533546aa54d26f6bee` and runner
`85cd5ef56fbe781a9887d39da36eb346b48c043c6157c288cc4367bace11846a`
authenticate opt1 under pinned Rust `a69a63265`. Preparation 398.575s,
authentication 1.522s, checking 6.995s, canonical execution 6.107s and samples 16.230s.
The final rejection fragments are stricter than the initial placeholders and
match that retained diagnostic exactly; source/runtime observations keep the measured `24d35` revision.

The producer preserves complete prepared occurrences and their existing
supplier receipts. Supplier symbols bind before copied operands resolve;
foreign aliases form under their exact declaration telescope rather than a
same-named fresh receiver. Receiver facts do not reappear through nested type
or implementation arguments. Computed array extents apply the same expression
substitution and publish a literal only after the existing full RHS formation
judgment succeeds. Unknown or conditional recipes remain unchanged; no new
evaluator, owner authority, identity grammar or type-matching fallback is added.

Earlier source failures and one cheap Rust-check API failure remain false in
the raw evidence; no native result is claimed for them. Logs/JSON, frozen
source and bundles remain outside scratch at
`/Users/zcanann/Omega-build/alias-rhs-captured-supplier-20261101/`.
This one default-member place does not complete other mixed/open RHS contexts,
RHS calls/qualified value names, Product/Build transport, nine-place rows,
other hosts or the Omega-written producer.
