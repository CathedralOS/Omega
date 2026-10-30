# Root and dependency native bindings

The root and its private dependency independently construct the same `DllImport` const tuple in separate Build activations. Each exact requirement retains its own package owner, complete signature, Build producer and review row. The root checks `abs(-91) == 91`; the dependency checks `abs(-73) == 73`, and either wrong result traps. All four hosted rows are authored, with actual native and Terminal exit 0 observed on macOS arm64 using producer `8b4b6ec49266da04da31e7a00aeef5707b45f7bd` after ordinary update, individual review decisions and resume.

The private-operand reject control is `tests/omega/fail/build/dependency_native_binding_private_operand`: application Build cannot bind the dependency-private `RawMagnitude` requirement. This fixture establishes repeated declaration/instance custody across activations, not general applied native, callback, SDK or other-host qualification.
