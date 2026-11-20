# Interpreted Gamma on the portable stdin program

These measurements use the same 339-byte Epsilon guest and one-byte stdin as
the [27-route comparison](README.md#same-stdin-program-across-routes). The
guest reads `Z`, writes it and exits 90. The complete successful observation
is `00 5a 00 00 00 5a`.

The runner checkout was `4a46aff7577`. Its `bootstrap/` tree is identical to
the earlier `f040d53f89d` run. The guest source SHA-256 is
`e29d25ce75425b3d9312012a018c6e64e868a4f71f0b3cd7c8ccc34c29557d5d`
and stdin SHA-256 is
`bbeebd879e1dff6918546dc0c179fdde505f2a21591c9a9c96e36b054ec5af83`.
The Beta-written Gamma interpreter ran on the seed-built 8,575-byte Gamma tape,
SHA-256 `9a5f9acc2eeadb46ae1f1922fe7e7efaf379045c719152d6e1bbde909be9df4a`.

## Replay the compiled guest

The existing seed-built Linux route compiled the Epsilon guest to Delta and
translated it to 81,742 bytes of Gamma, SHA-256
`cbd56e337c1d0c2374ba36fdfff71f1cebe5a2cc1b8d912a5a68d6b0843718bf`.
A freshly rebuilt Beta-written Gamma interpreter executed that program on
`Z` in 0.252 seconds, returned status zero and published all six expected
bytes. The same interpreter then ran the 42,442-byte Linux Gamma backend on
the linked guest, producing a byte-identical copy of the compiled-host
260,706-byte executable in 558.226 wall seconds. Its SHA-256 was
`fb9bf95cd4e572c67d6d024c8f698847de8a8ff9be802cd9c7d485dcf44a2dd2`.
The executable returned the same observation in 0.021 seconds.

These replays cover the final execution and target compilation. Construction of
the Epsilon compiler and translation of the guest were reused from the Linux
row. Times were measured on a shared host.

With the original route's `reference` executable and the compiled row's
`subject.gamma` and `stdin.bin` in hand, run from the checkout root:

```sh
target/debug/xtask gamma-invoke --evaluator "$interpreted/reference" \
  --source "$compiled/subject.gamma" --input "$compiled/stdin.bin" \
  --output /tmp/gamma-stdin.eeout --timeout 90
od -An -tx1 /tmp/gamma-stdin.eeout
target/debug/xtask gamma-invoke --evaluator "$interpreted/reference" \
  --source "$interpreted/gamma-to-native.gamma" --input "$compiled/subject.gamma" \
  --output /tmp/gamma-stdin.elf --timeout 1500
chmod 700 /tmp/gamma-stdin.elf
/tmp/gamma-stdin.elf < "$compiled/stdin.bin" > /tmp/linux-stdin.eeout
cmp /tmp/gamma-stdin.eeout /tmp/linux-stdin.eeout
```
