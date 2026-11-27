# Saved Linux x86-64 Psi

The source compiler published `linux_x86_64.psi` from this fixture’s ordinary Build entry nomination and the bundled library. Its authenticated entry declaration is `toolchain-contract::targets::linux_x86_64::LinuxX86_64Application`. The native compiler regression reopens the artifact, checks the requested validators and executes the resulting ELF with the status in [expected_native_exit.txt](expected_native_exit.txt).

From this directory, publish the selected Build with the bundled library.

```sh
omega --emit-psi /tmp/linux_x86_64.psi --target linux_x86_64 --build-input build.omg --build-input main.omg build.omg
```

The compiler built from `0d03f4c37b7` with the case-exhaustion proof rule compiled 51 source files and published the fixture under semantic format marker 302 and proof-system marker 10. Its executable SHA-256 was `27b47d6b20753a34a4d50c896f5064a7e6a85f975ea9ae41aa181293d5aaa222`. The bundled `source/library` tree is Git object `30ef73494382b587b5d0cbd21a3dd1ed3159525c`.

| Input or receipt | Digest |
| --- | --- |
| Canonical artifact manifest | `e3bb2f8242b9e6151ccd7edbb235cb0ccbe9017ef7c6c4d1799fa597317ce983` |
| Saved file SHA-256, 226930 bytes | `25241cbdb5eb2253109faba193441a9d90ec7b444b505f944c19efe8d5d7c065` |
| main.omg SHA-256 | `073768333496caabb3c28a753482d2888a9a527b0765f7f12941332eba6c8e58` |
| build.omg SHA-256 | `95e9fe2fa4d640385511dfecef410202674234e1fd9181a02caaea77cc862e54` |
| `source/library/system/targets/linux_x86_64/entry.omg` SHA-256 | `da744cd48323520484545a412be9d6334b81d3b55806e056dcf920cda444ac2b` |

## Historical wire specimen

`linux_x86_64.pre_namespace.psi` records the saved program from
`43499fe37809c7318739414a97c86697028e4786` under semantic format marker 289. Format 290 retired that marker, and the encoding rejects older markers before reading their payload ([encoding](../../../../../source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#machine-rows)). The codec and native regressions check that the specimen is refused that way, so its unqualified physical arrival never reaches native admission.

The program was issued through source stages 00–07 using this directory's
`main.omg`, the sorted top-level Omega sources in `source/library/core`, and
three bundled standard-library sources, `calling.omg`, `entry_shapes.omg` and
`targets/linux_x86_64/entry.omg`. The source caller supplied the fixture's
`linux_x86_64::ProgramEntry -> main` nomination directly. It did not execute
the outer Build. Source publication authenticated the selected
`LinuxX86_64Application` schema with `ToolchainContract` custody. The test
package identity is 32 repetitions of byte `bc`. No host providers are needed.

| Input or receipt | Digest |
| --- | --- |
| Canonical artifact manifest | `b0e903a139095a3e99ccaf9771df7036d403983b81191df1615731fc21d1d721` |
| Saved file SHA-256, 226205 bytes | `9c0b0799e91b0097e51173b6c58377a18b499c89eeda46a024172aa91e1ea4d7` |
| main.omg SHA-256 | `abe2e510430a8bf7caf7abc2641bd18e4f6b40b09a82b69c554158e675a66280` |
| build.omg SHA-256 | `94fc742d3f10426152b6d4f232cafa9c0d45233eeb631c34ed3f92c3c736d35f` |
| Linux entry source SHA-256 | `3957491d886fc3c92ff5266dddd473339906fb2f43cf1d57442d346abd6fa591` |
