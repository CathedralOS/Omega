# Native execution of the provider fixture

This temporary branch runs the same public Omega program on four matching operating systems and architectures. It checks the source, image format, architecture, binary digest and actual process exit for each compiler independently.

The source is [target_selected_implementation](../tests/fixtures/run/providers/target_selected_implementation). The fixture calls an abstract service and checks the constants returned by Build's provider for that target. Success returns zero. The files under `images/` are emitted native programs. The compiler binaries are identified by SHA-256 in [images.json](images.json).

Both compilers used the official `f658f30ec9660b8d66c8b3c2fd704def734fab7c` library and the same fixture bytes. The candidate came from the linear-pipeline integration on [PR #4030](https://github.com/CathedralOS/Omega/pull/4030). Its digest identifies the executable tested.

Run this build command from the fixture directory with the chosen compiler.

```sh
OMEGA_LIBRARY_ROOT=/path/to/f658/source/library RAYON_NUM_THREADS=1 \
  /path/to/omega --build-dir /tmp/omega-programs \
  --target linux_x86_64 --target linux_arm64 --target macos_arm64 --target windows_x86_64 \
  --build-input build.omg --build-input main.omg main.omg
```

Each job executes both verified files directly. The workflow records their digests and exit codes in the job log.
