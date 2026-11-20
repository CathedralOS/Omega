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

## Direct Delta compiles the Epsilon program

Two complete routes matched on the pinned stdin program with interpreted
Gamma and direct Delta. The Gamma interpreter executed the 100,654-byte
Delta interpreter, SHA-256
`e0aaa160cff057c524b2f313df24ad4ee9d091f3872ea594f6028e781fdbdc72`.
Its input was the 629,618-byte, Delta-written Epsilon compiler, SHA-256
`b98b87f006d493066639af46a78d93e13904c1e04cf6ed2df5415f772ed15196`,
followed by the 339-byte Epsilon program. The outer four-byte little-endian
length framed the Delta interpreter source. A second length framed the Epsilon
compiler source. The remaining bytes were the Epsilon program. The complete
730,619-byte input to the reference executable had SHA-256
`a520d687477929cbcca10d453777dbd4cf4974f4c4961672354bb70756d0e8b7`.

In 3,692.730 wall seconds the interpreter emitted the 3,247-byte module
`b440b5d18c3abc5d68e2e440211693f32a3bfda5a46ae470be61d7b9a1e60e1a`.
Linking that output reproduced the exact 99,270-byte Delta source used in the
[measured target builds](#replay-the-compiled-guest). Both routes reused
that Delta-to-Gamma translation and its target image. Each image ran again
on `Z` and returned process status zero.

| Gamma / Delta / Epsilon | Guest bytes | Guest wall time |
| --- | --- | ---: |
| `interpret / interpret / alpha` | `00 5a 00 00 00 5a` | 0.841 s |
| `interpret / interpret / native` | `00 5a 00 00 00 5a` | 0.003 s |

To rebuild each route independently from the pinned checkout, run the
following commands with fresh output paths. The measured routes shared their
byte-identical stages across runs.

```sh
python3 tests/bootstrap/route.py /tmp/direct-to-alpha --gamma interpret --delta interpret --epsilon alpha --seconds 4500
python3 tests/bootstrap/route.py /tmp/direct-to-linux --gamma interpret --delta interpret --epsilon native --seconds 4500
cmp /tmp/direct-to-alpha/guest.eeout /tmp/direct-to-linux/guest.eeout
```

This brings the total to 21 of 27 for this guest and the frozen `bootstrap/`
Git tree `a0829d60b057e65aa9fef059dfbb4bd7362cad27`. Eighteen completed in
the initial comparison. The separately rebuilt `interpret / native / interpret`
route and the two routes joined above account for the other three.

## Replaying those two stdin routes after the evaluator changed

At `5144ec25e810`, the Epsilon interpreter closure had changed to SHA-256
`7b214d058c086ffcf5e5481e5dd08e4e6f47275ae07a4cb2cb6b4d26764b1bae`.
Both compiled Epsilon routes still selected the same Delta-written compiler.
We rebuilt the Beta and Gamma tapes, stamped the reference executable and
rematerialized the direct Delta interpreter, both compiler sources, support,
linked guest and Gamma backends from that checkout. The reference executable
and its complete inputs matched the original physical executions byte for byte.

| Work done by the Beta-written Gamma interpreter | Input bytes | Input SHA-256 |
| --- | ---: | --- |
| Run the Epsilon compiler through direct Delta | 730,619 | `a520d687477929cbcca10d453777dbd4cf4974f4c4961672354bb70756d0e8b7` |
| Translate the linked Delta program | 255,574 | `e848666217551587fdb2ddbd4a346d73dda5f79fbe7f1cc5563e87d3975ab29b` |
| Produce the Alpha tape | 118,334 | `bcf0510aba80c6b9d021b2358a1c6c5e76bfe998c92b67c4fd0c757fd63a509b` |
| Produce the Linux executable | 124,188 | `69d08f91082e9adfc7ee570cb63909ab52ad8f1678138e743579597611908ea7` |

The retained Alpha and Linux executables also matched the corresponding target
artifacts in the later inventory. Each ran again on its one-byte stdin and
returned `00 5a 00 00 00 5a` with status zero. That proves the same two
route selections on `bootstrap/` tree `2ddf96a8e34b8d46067f8128f96e8d9d3a525aeb`.
At `bcffe3ee694`, rematerializing the direct Delta interpreter, Delta compiler,
Epsilon compiler and linked Delta program produced different bytes. Those
inputs require separate measurements.

On that same `5144ec25e810` inventory, the original Gamma interpreter also
ran the Delta compiler on the 629,618-byte Epsilon **compiler**. The complete
785,922-byte reference input had SHA-256
`b550e9fe5e8eebfa380e5a47baa61925ffd15f7f0779200bf67d98b72d43c800`.
It produced 810,690 bytes of Gamma, SHA-256
`ba4cb39da467b4dad9ca874c65b480389fa5b6632c459cfb33f5d2a0ee0140ca`.
Both lower source inventories had the identical reference executable and
complete input. This compiler source is the shared intermediate for building
an Alpha- or Linux-hosted Epsilon compiler through the original interpreter.

## Original Gamma on the named classifier sources

The named `omega-byte-classes` case at `8da0d6a9f82` has the same
`bootstrap/` sources as `93ad7259148`, tree
`734d5741ab11680c414eda1fcd0e6db0e7f96b27`. On those sources a freshly
reconstructed Gamma interpreter ran the Delta compiler on the Epsilon
evaluator. The complete 781,056-byte input had SHA-256
`04c259e8ddf2a4c0322b54f3a4c88b38417a8bcdc21ee9f23939c69c729a1730`.
It produced 729,482 bytes of Gamma, SHA-256
`3d0f600184534c82ff03bb1eb9197648ba7420656a915bfc04c7e4f49598c9bf`.
The compiled Gamma host produced the identical evaluator source.

On the default `interpret / native / interpret` route the Delta translation
took 4,935.209 wall seconds. The same seed-built Gamma interpreter then ran
the 40,367-byte Linux backend, SHA-256
`bd78a58f9187a770293bbb04466942b05dde50ea57b0e234d4d691c6d26971a4`, on
the freshly emitted Gamma. Its 769,853-byte request had SHA-256
`b4ad50ae3989a08962ececce948817b842efe861e8663e8e2e980beb05d95fee`.
In 6,972.424 seconds it produced a 2,004,942-byte Epsilon evaluator, SHA-256
`023d970589e30d0806ddd7f38041a7e81660224e46fc8700b363a4d13d96721a`.
That executable matched the separately compiled host's image byte for byte.

Each request bound the 618,016-byte Epsilon closure, SHA-256
`02be715b60938be45bd89984df6bfb197e1b46310435feae77b9b6271c4a0cd0`,
the respective classifier source and all 256 input bytes. The same newly
built evaluator returned process status zero for each observation.

| Named customer | Sealed request SHA-256 | Complete observation | Wall time |
| --- | --- | --- | ---: |
| Classifier | `9bc8b494413bb8799c8b838510aada78ea6e96a296eaa7fa0e8a4b54654c13b7` | 3,077 bytes, SHA-256 `9e66d3ae8c042ba3c425a4f559e85c4850608fda04d0640110eaf20599f5e225` | 49.450 s |
| Wrong-base twin | `03e491438f40f36242717cdd85cbf2e7ec0ce86695ec9fc175a39968f52ae53d` | `02 12 f0 07 00 00` | 0.611 s |

The independent direct-Delta compiler attempt on this customer used the
101,068-byte checked interpreter and the 630,512-byte Epsilon compiler. The
complete 735,336-byte input to the Gamma interpreter had SHA-256
`d5a737caa4cd89e0caf77d3b4d4681b99b66c93afed2895031c340dea6fce9b3`.
It reached its 10,800-second wall limit in the Epsilon-to-Delta compilation
stage before publishing a module. The process had accumulated 850.36 CPU
seconds at the last sample. This leaves the direct-Delta compiled routes open.

The original Gamma interpreter then executed that evaluator directly on the
classifier in which one `omega_digit_in_base` call passes `value` in place
of `selected`. This is the selectable `--delta-interpreter gamma` route.
The 733,539-byte input to the reference executable had SHA-256
`81abb880306f9bbdbbc22262cc46f0826d718e12f0dda46a3467e28dd557517f`.
The interpreter returned process status zero and `02 12 f0 07 00 00` in
200.586 seconds, locating the type error at byte 2,032. On positive stdin
`09 30 32 38 41 5f 66 ff` the same route returned the expected 101 bytes,
SHA-256 `d83a48cc949b3bf8309b5b6e38df5d9640b08f259aa2329458e8aa42f375f666`,
in 393.973 seconds. These cover guest exit 42 and twelve classifications for
each of the eight supplied bytes. On the full 256-byte stdin this route
reached its 6,600-second wall bound before publishing an observation.

## Running the translated Epsilon compiler on the original Gamma interpreter

On `8da0d6a9f82`, `--delta-interpreter gamma` translates the Delta-written
Epsilon compiler to Gamma and runs that source on the selected Gamma host.
The Beta-written interpreter on Alpha, executable SHA-256
`7f781f2ad3fe2b2985ad6d8392053a151a8ad5e9f538b058e42d6eae06fc6781`,
read the 813,584-byte compiler, SHA-256
`da44deee6b511e9328507a000c77b6258367af4c31ce1544ac84703b425b620c`,
with the Epsilon customer on guest stdin. Each input below consists of a
four-byte little-endian compiler length, the compiler and the named customer.

| Customer | Complete interpreter input SHA-256 | Compiler output | CPU / elapsed seconds |
| --- | --- | --- | ---: |
| `omega-byte-classes` | `c83d09ecd745b3cc6719529a02242fbddb8b6f6fd323b2b3671de1a1e7bb2d54` | 65,013-byte Delta module, SHA-256 `bd1d5bd47e663ecf27518374302e276913543fa67bc47479439dce60c0376314` | 332.85 / 3,580.71 |
| `omega-wrong-base` | `35959384a94471903dbf77a5110d142534937e390791b4452225fd795bd3c72a` | 16 ASCII bytes `rejected 18 2032` | 15.08 / 126.55 |

The same Beta-written interpreter ran the Gamma-written Delta compiler on the
630,512-byte Epsilon compiler and its bound support. Its 787,193-byte input
had SHA-256 `4e15a439ff582761befc975f907d1d24f360cf1b4315f2ac77709f67be31bd0b`.
In 720.37 CPU seconds (9,279.88 elapsed) it emitted the 813,584-byte Gamma
compiler above. To overlap the builds, the two Epsilon compilations began with
a separately generated copy. Reframing both customer inputs with the
original interpreter's bytes reproduced both complete inputs byte for byte.
This supplies the compiler for two additional route selections with
`--delta-interpreter gamma`, outside the default 27 selections.

Linking the emitted module with the Epsilon runtime reproduced the complete
317,906-byte original-Gamma request for translating the linked guest, SHA-256
`0ca3591ff322757b015632cc040b0f2a0eeb136a322e19edbf320efd46f3e996`.
The interpreter produced 170,803 Gamma bytes, SHA-256
`99befce038db962823e7f37ef242c2ebe1f543adb73b1ab892c35298740afbd6`.
The same host ran both final backends. Both ready-to-run programs received
all 256 byte values in order and returned the expected 3,077 bytes,
SHA-256 `9e66d3ae8c042ba3c425a4f559e85c4850608fda04d0640110eaf20599f5e225`,
encoding guest exit 42 and twelve classifications per byte.

| Target | Complete backend input SHA-256 | Output bytes and SHA-256 |
| --- | --- | --- |
| Alpha tape | `5fb9425c89d0576b7cba169d1339d4879fe018a5f886f4a3736fe52020bc52dc` | 880,161, `d260052ecef6d2c08f6e70c7698c582f22d7ac30ddfd4cc306c1ddc72046a6a5` |
| Linux x86-64 | `e527de12ecdadd009674ba6de540c5aeee31705d97078a44d444fa211eb9527e` | 468,679, `fc95d11d86c87961bd463f804b4471f21d2e9ff8b155fdad820fdc9b7408496c` |

To reproduce an entire selection, run the following from `8da0d6a9f82`, with a
fresh directory per target. Replace `alpha` with `native` for Linux. For
the refusal on this revision, copy the generated `subject.epsilon`, replace
the single `omega_digit_in_base(value, selected)` call with
`omega_digit_in_base(value, value)`, and pass that file with `--source`
in place of `--case omega-byte-classes`.

```sh
python3 tests/bootstrap/route.py NEW --gamma interpret --delta interpret \
  --delta-interpreter gamma --epsilon alpha --case omega-byte-classes --seconds 16300
```

## Omega lexical byte classes on the original Gamma route

The Omega compiler's `byte_classes.epsilon` ran over all 256 byte values
through the `interpret / native / interpret` route. The 3,748-byte Epsilon
customer includes the production classifiers, their `OmegaNumericBase` type
and a console entry. Its SHA-256 was
`f508abda2da975caec1aa6dc9d5823e4be9ccb8774d724fd59f41935d70bf9fd`.
Stdin contained `00` through `ff` in order, SHA-256
`40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880`.

This execution reused the Epsilon evaluator built by original Gamma on the
frozen bootstrap tree named above. Its 616,939-byte Delta closure had SHA-256
`a2830415a4d98176e9bd97e7f28a9c4512d926fdb5c502d73e19d874ed58ca4c`.
The Gamma interpreter had run the Delta compiler on a 783,806-byte request,
SHA-256 `5cac5fd208be1adc5f560da1312f3db26fc5b787e975e8f24a03dceefad89933`,
to produce 726,601 Gamma bytes, SHA-256
`1d3a700d970acda04ee4699c19bf03a3654eb481a46755399d0daf759fe030af`.
The same interpreter had run the Linux backend on a 769,047-byte request,
SHA-256 `077bda40007b19ac8c8e8e02ec0aaffa6e5b20da635f425ad7f66bbd729eafe8`,
to produce the 1,977,558-byte evaluator executable, SHA-256
`36773be943484f3452a1e4fc81dd2875dd3c4e279339e8fbdacf54b48aea552c`.

The measured guest invocation was fresh. Its 4,056-byte `EEREQ` included the
closure digest, classifier source and all 256 input bytes, SHA-256
`4848f270d9c9f21398762dce2b75bcbb4f4988e817d3974fa1ce01c7cd0a033f`.
In 15.065 wall seconds the evaluator returned status zero and the expected
3,077-byte observation, SHA-256
`9e66d3ae8c042ba3c425a4f559e85c4850608fda04d0640110eaf20599f5e225`.
The observation encodes guest exit 42 followed by twelve classification bytes
for each input. This customer has its own results, separate from the stdin
control's 21-of-27 count.

To replay, use the frozen checkout's `epsilon_request` with the retained
`epsilon.delta`, the classifier source and `bytes(range(256))`. Pass the
resulting bytes on stdin to the retained `evaluator.elf` and compare the
complete output to the expected digest above.

The same classifier completed on the separately rebuilt `5144ec25e810`
evaluator. Its Delta implementation was 623,481 bytes, SHA-256
`605901123cdfa5537a81c7001a797aa66b8f1b14420d41a35d18bd3203fe3d0f`.
The seed-built Gamma interpreter ran both compiler invocations.

| Compiled output | SHA-256 of complete input to the reference | Output SHA-256 | Wall time |
| --- | --- | --- | ---: |
| 726,582-byte Gamma evaluator | `7bdf6b4aa5adf935fa053b6996b40a6553bccfadee82cb31b6de16ccd82a612c` | `e31ecf91fe85ce23b62206df2becfba0ac36d27d9e0dcbedf1f0c9804eca4d1b` | 3,268.098 s |
| 1,985,896-byte Linux evaluator | `7a71d8abc39b015b24d2d5429daa989cd44bcef5e88b74fd9c2ed1d0c410ec20` | `1e688c2e8776d29d8f7ea932c527a850f13556470c4083c0c90b30ac8f9d4e47` | 5,308.292 s |

The cold build ran the portable stdin control successfully. A fresh request
then supplied the Omega classifiers and all 256 inputs to its Linux evaluator.
That 4,056-byte request had SHA-256
`9ddd09aa1a4f155a4872976d2946d508d5a6bc2aa2caa5e7410816d7cb48348e`.
In 39.012 wall seconds the evaluator returned status zero and the same
3,077-byte observation, SHA-256
`9e66d3ae8c042ba3c425a4f559e85c4850608fda04d0640110eaf20599f5e225`.

Changing the single call `omega_digit_in_base(value, selected)` to
`omega_digit_in_base(value, value)` passed a byte value where the classifier
requires an `OmegaNumericBase`. The 3,745-byte source had SHA-256
`927259ff3b779b8109a2b747f4ba98ef6284210004e8032c82ca2be00a5af318`.
The same executable and stdin received the resulting 4,053-byte request,
SHA-256 `ca6c411cbc9efdfcf0e614399af5d11dcaecb8242196c45ffe26fb36251667b1`.
In 0.131 seconds the evaluator returned process status zero and refusal bytes
`02 12 f0 07 00 00`, locating the wrong-typed argument at byte 2,032.

## Original Gamma builds the classifier's Alpha evaluator

On the bootstrap tree from change #4593,
the freshly reconstructed Beta-written Gamma interpreter ran the 153,051-byte
Delta compiler on the 620,128-byte Epsilon evaluator and its bound support.
The 777,500-byte input had SHA-256 `6f25961168d5194389090bcef4fcddfdbdb4145d0c3a66e3a217653a60cc0907`.
It produced 717,930 bytes of Gamma in 563.205 CPU seconds
(4,435.791 elapsed), SHA-256 `fcbeef7424dcdee87986bd81a0cb1254031dabcc39f17d22a873e5d36746f048`. [Gamma interpreter change](https://github.com/CathedralOS/Omega/pull/4593)

Three hosts ran the same 35,345-byte Gamma-to-Alpha compiler on that evaluator.
Each produced the same 2,204,117-byte Alpha tape, SHA-256
`b311cc23de0f34a018dcc62c35079a36a3ced038916198d4d1f5505f8b0f1e53`.

| Gamma host for Alpha emission | CPU seconds | Elapsed seconds | Peak MiB |
| --- | ---: | ---: | ---: |
| Beta-written interpreter on Alpha | 784.047 | 6,574.399 | 227.5 |
| Gamma compiler running on Alpha | 48.749 | 403.606 | 182.8 |
| Gamma compiler running on Linux x86-64 | 7.869 | 54.205 | 182.5 |

The original host read the emitter with the same 48,043-byte Beta interpreter
used for the Delta build. The Alpha-hosted compiler reused the emitter's
35,345 bytes of source for its own fixed point. The Linux host shared the
frontend and lowering and added an 8,703-byte x86-64 backend.
To overlap the two long builds, Alpha emission began with an independently
produced evaluator. The complete 753,279-byte input matched byte
for byte when reconstructed from the fresh Delta result, SHA-256
`ce4fbd4ffaa784c885eb67598fb0131c65eee6c94fecbdfa0c3c3e5d0b906e58`. The earlier
`4a46aff75770` run reached its 4,500-second wall limit before producing a tape.

The Alpha seed ran the freshly emitted tape on both classifier packets. The
3,748-byte program processed all 256 byte values and returned exit 42 with
3,072 output bytes (31.727 CPU seconds). Its complete 3,077-byte
observation had SHA-256 `9e66d3ae8c042ba3c425a4f559e85c4850608fda04d0640110eaf20599f5e225`.
The 3,745-byte wrong-base program returned `02 12 f0 07 00 00`, a type
error at byte 2,032 (0.634 CPU seconds). Both observations matched
the independent replay on the Linux-produced tape.

Replay on `9e902e188371`, which has the same bootstrap tree, with
`python3 tests/bootstrap/route.py NEW --gamma interpret --delta alpha --epsilon interpret --case omega-byte-classes --seconds 16300`.
Use `omega-wrong-base` in a fresh directory for the paired refusal.

### Run the generated evaluator directly on Gamma

The `--delta-interpreter gamma` route lets the Beta-written interpreter execute
the same 717,930-byte generated Epsilon evaluator. The table starts after the
shared Delta-to-Gamma build and gives CPU seconds. The seed-stamped Gamma
interpreter had SHA-256
`e0e5046e537ce578f7b9b816a5fd52522cfd094441f2eea92a822e93652e2833`.
It read Delta compiler source with SHA-256
`ceb6e8bfe734e581afcdf1b06a07093d3f6a6cb5b9673b0ced907d184dfef3f4`.

| Route after Delta emits Gamma | Build the Alpha evaluator | All 256 bytes | Wrong-base refusal |
| --- | ---: | ---: | ---: |
| Interpret the generated Gamma | 0 | 703.145 | 14.662 |
| Emit Alpha using original Gamma, then run the tape | 784.047 | 31.727 | 0.634 |

The 721,990-byte framed Gamma invocation had SHA-256
`e327fd34bcd48808c3dfd3f0dbe5297305390b3667fb7b7c894e9c6576b4c38b`.
It ran the same 3,748-byte classifier on all 256 bytes and returned the same 3,077-byte observation. The original host
took 7,193.823 elapsed seconds on the shared host. The wrong-base packet
returned the same six-byte type refusal at byte 2,032 in 144.988 elapsed
seconds. The framed refusal request had SHA-256
`4ca848e67feff76bd1f18100e11cb93c0265044746e46b9a255e9d5f91b2f595`.

The 27-route table uses direct Delta interpretation in its interpret column.
This alternate executes Delta-generated Gamma. Replay either case above with
`--gamma interpret --delta interpret --delta-interpreter gamma --epsilon interpret`.
