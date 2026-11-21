# Saved Linux x86-64 Psi

The source compiler published `linux_x86_64.psi` from this fixture’s ordinary Build entry nomination and the bundled library. It compiled 252 source files. Its authenticated entry declaration is `toolchain-contract::targets::linux_x86_64::LinuxX86_64Application`. The native compiler regression reopens the artifact, checks the requested validators and executes the resulting ELF with the status in [expected_native_exit.txt](expected_native_exit.txt).

From the repository root, publish the fixture with this command.

```sh
omega --emit-psi /tmp/linux_x86_64.psi --target linux_x86_64 tests/fixtures/calls/run/scalar_contract_route_exit/main.omg
```

The bundled `source/library` tree is Git object `4fa1ae9d7ac72f6efa7b1367cec9093e9ac2ad51`.

| Input or receipt | Digest |
| --- | --- |
| Canonical artifact manifest | `5832612f0086193e5f311724887f42fb1d8eba038338b72999c5d48f175877d1` |
| Saved file SHA-256, 228974 bytes | `23c1f9ffccbcab9067e737437f535ec6e602cc686c27f43a7f98c0335afb2f11` |
| main.omg SHA-256 | `073768333496caabb3c28a753482d2888a9a527b0765f7f12941332eba6c8e58` |
| build.omg SHA-256 | `9483243b6476c41b95be5c949a0e39901e44096d881995872d51df51683d3d5d` |
| Linux entry source SHA-256 | `94aec6503de550f72f997279fc6316f973037edcd1ee1b433b205ef1afdc387c` |

## Historical wire specimen

`linux_x86_64.pre_namespace.psi` records the saved program from
`43499fe37809c7318739414a97c86697028e4786`. The codec regression decodes and reproduces these exact bytes. Native admission rejects the specimen’s unqualified physical arrival against the current target contract.

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
