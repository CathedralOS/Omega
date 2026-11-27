# Saved Linux x86-64 Psi

The source compiler published `linux_x86_64.psi` from this fixture’s ordinary Build entry nomination and the bundled library. It compiled 154 source files. Its authenticated entry declaration is `toolchain-contract::targets::linux_x86_64::LinuxX86_64Application`. The native compiler regression reopens the artifact, checks the requested validators and executes the resulting ELF with the status in [expected_native_exit.txt](expected_native_exit.txt).

From the repository root, publish the fixture with this command.

```sh
omega --emit-psi /tmp/linux_x86_64.psi --target linux_x86_64 tests/fixtures/calls/run/scalar_contract_route_exit/main.omg
```

It was published from commit `c5c47a53b080` with the change that introduced semantic format marker 296, under that marker. The bundled `source/library` tree is Git object `fa9e604f2d73cf878c1a7ffa764e27f34f7122ae`.

| Input or receipt | Digest |
| --- | --- |
| Canonical artifact manifest | `1f091b28b9e3a5598385365b10a760d42e91a65d9358ea19cb952f24fb999077` |
| Saved file SHA-256, 228690 bytes | `f053200ece36b7f6b113dc47e7b3cbdb1b886225f7eed9e69b4d67fac9362845` |
| main.omg SHA-256 | `073768333496caabb3c28a753482d2888a9a527b0765f7f12941332eba6c8e58` |
| build.omg SHA-256 | `9483243b6476c41b95be5c949a0e39901e44096d881995872d51df51683d3d5d` |
| `source/library/system/targets/linux_x86_64/entry.omg` SHA-256 | `cce757bb5e3cc2ddbfe2db44fe03510a97b56fa6cc80db210d4d0402bdb2878b` |

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
