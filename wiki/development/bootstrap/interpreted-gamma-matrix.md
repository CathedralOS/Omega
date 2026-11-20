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
The Alpha seed executed the 8,575-byte Beta-written Gamma interpreter tape,
SHA-256 `9a5f9acc2eeadb46ae1f1922fe7e7efaf379045c719152d6e1bbde909be9df4a`.

## Replay the compiled guest

The existing seed-built Linux route compiled the Epsilon guest to Delta and
translated it to 81,742 bytes of Gamma, SHA-256
`cbd56e337c1d0c2374ba36fdfff71f1cebe5a2cc1b8d912a5a68d6b0843718bf`.
The freshly rebuilt Gamma interpreter executed that program directly. It also
ran both Gamma backends on the same program and reproduced both compiled-host
artifacts byte for byte. All three executions published the expected six bytes
with outer process status zero.

| Final execution | Artifact produced by interpreted Gamma | Build wall time | Guest wall time |
| --- | --- | ---: | ---: |
| Beta-written Gamma interpreter | Executed the shared Gamma source | — | 0.252 s |
| Alpha seed | 487,013-byte tape, SHA-256 `bec94d12b72c3f1ebe1b5a246df1677f2c17515a5a4592422b78fb0770b80298` | 793.966 s | 0.585 s |
| Linux x86-64 | 260,706-byte ELF, SHA-256 `fb9bf95cd4e572c67d6d024c8f698847de8a8ff9be802cd9c7d485dcf44a2dd2` | 558.226 s | 0.021 s |

These replays cover final execution and target compilation. Construction of
the Epsilon compiler and translation of the guest were reused from earlier
compiled rows. Times were measured on a shared host.

Set `interpreted` to the earlier `interpret-native-interpret` output directory
and `compiled` to the `native-native-native` directory. The former retains the
Linux backend source, SHA-256
`b691cd212e3222bf5908af87a544e9d49b31e84b8cf3511dacf4e4904ae0a09f`.
If those directories have not been retained, create them from the pinned
checkout with the route runner.

```sh
interpreted=/tmp/omega-interpreted-inputs
compiled=/tmp/omega-compiled-inputs
python3 tests/bootstrap/route.py "$interpreted" --gamma interpret --delta native --epsilon interpret --seconds 1500
python3 tests/bootstrap/route.py "$compiled" --gamma native --delta native --epsilon native --seconds 1500
```

Run the replay against the retained artifacts.

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

The Alpha replay uses the 36,588-byte Gamma backend, SHA-256
`b5c98fe794bf725e9e85318ee37de55941244700f1161cf6721bcec5c901b186`.
A prior `native-alpha-alpha` row retains it as `gamma-to-alpha.gamma`.
Select that source in the same `xtask gamma-invoke` command and publish its
output as `/tmp/gamma-stdin.tape`. To execute those bytes on the seed:

```sh
OMEGA_REPO_ROOT=$PWD
. tools/bootstrap/paths.sh
. tools/bootstrap/alpha/seed_env.sh
stamp_seed /tmp/gamma-stdin.tape "$OMEGA_PATH_ALPHA/$ALPHA_SEED" /tmp/gamma-stdin.vm
/tmp/gamma-stdin.vm < "$compiled/stdin.bin" > /tmp/alpha-stdin.eeout
cmp /tmp/gamma-stdin.eeout /tmp/alpha-stdin.eeout
```
