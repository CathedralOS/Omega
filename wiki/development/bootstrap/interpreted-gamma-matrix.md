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
The stamped reference executable had SHA-256
`7f781f2ad3fe2b2985ad6d8392053a151a8ad5e9f538b058e42d6eae06fc6781`.

## Replay the compiled guest

The earlier seed-built Epsilon compiler emitted a 3,247-byte module, SHA-256
`b440b5d18c3abc5d68e2e440211693f32a3bfda5a46ae470be61d7b9a1e60e1a`.
Linking it to the checked-in Delta runtime on the pinned checkout reproduced
the 99,270-byte Delta program, SHA-256
`ac68099996a7883be50ad711d058fcd92a048cafafd433250e8375cd19101467`.
Interpreted Gamma then ran the Delta compiler on that program and produced the
81,742-byte Gamma source in 460.200 wall seconds, SHA-256
`cbd56e337c1d0c2374ba36fdfff71f1cebe5a2cc1b8d912a5a68d6b0843718bf`.
The 151,983-byte Delta compiler source had SHA-256
`27d81dff55aba5302ab3170715dfc1b082ab414519788695fffc657031d3da1e`. The 4,301-byte support section had SHA-256
`439fef90e10007baec81920e84b89f50ad815a27af80499e4240e1dcd55f9243`.

The freshly rebuilt Gamma interpreter executed that program directly. It also
ran both Gamma backends on the same program and reproduced both compiled-host
artifacts byte for byte. All three executions published the expected six bytes
with outer process status zero.

| Final execution | Artifact produced by interpreted Gamma | Build wall time | Guest wall time |
| --- | --- | ---: | ---: |
| Beta-written Gamma interpreter | Executed the shared Gamma source | — | 0.252 s |
| Alpha seed | 487,013-byte tape, SHA-256 `bec94d12b72c3f1ebe1b5a246df1677f2c17515a5a4592422b78fb0770b80298` | 793.966 s | 0.585 s |
| Linux x86-64 | 260,706-byte ELF, SHA-256 `fb9bf95cd4e572c67d6d024c8f698847de8a8ff9be802cd9c7d485dcf44a2dd2` | 558.226 s | 0.021 s |

These measurements join three stages by exact bytes. The target compilers
used a saved Gamma file that matched the fresh Delta translation. The
Epsilon-to-module result came from the earlier compiled route. Times were
measured on a shared host.

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

For the Delta translation, frame `subject.delta` and `support.bin` with
the runner's `delta_request` (profile 1). Pass that request as input to
`xtask gamma-invoke` with `$compiled/delta.gamma` as source and the rebuilt
reference as evaluator. The complete input to the reference was 255,574
bytes, SHA-256 `e848666217551587fdb2ddbd4a346d73dda5f79fbe7f1cc5563e87d3975ab29b`.
[Request framing](../../../tests/bootstrap/compiled.py)

Run the final replays against the retained artifacts.

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
