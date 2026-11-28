# Saved Linux x86-64 Psi

The source compiler published `linux_x86_64.psi` from this fixture’s ordinary Build entry nomination and the bundled library. Its authenticated entry declaration is `toolchain-contract::targets::linux_x86_64::LinuxX86_64Application`. The native compiler regression reopens the artifact, checks the requested validators and executes the resulting ELF with the status in [expected_native_exit.txt](expected_native_exit.txt).

From this directory, publish the selected Build with the bundled library.

```sh
omega --emit-psi /tmp/linux_x86_64.psi --target linux_x86_64 --build-input build.omg --build-input main.omg build.omg
```

At `7fdb7bf61e0515e44cac03675e9270ab5e57553b`, the compiler read 51 source files and published this fixture under semantic format marker 310 and proof-system marker 10. Two independent compilations produced identical bytes. The compiler executable SHA-256 was `dc90a74616ebec65363ccace28024386389851fb675577863e9d4fc195125fb8`. The bundled `source/library` tree was `172675ba9f599cc20a5e9eed612265f7f6e06d66`.

Format 310 records routed domain membership on each exact machine input. It preserves independently exported supplier records (305), typed native borrows (306), borrowed alignment (307), callable-field invocation paths (308) and per-value package evidence (309).

| Input or receipt | Digest |
| --- | --- |
| Canonical artifact manifest | `666da55bb20ca5d2a029f6b7c7678d8700654cb822e1d01227c540cf51bc8960` |
| Saved file SHA-256, 227206 bytes | `b8a0bec53994f0b8104994605ab263ea254e68731c909fdae6dd2a5f1d279d28` |
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
