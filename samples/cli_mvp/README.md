# CLI MVP Sample

Smallest console-style Omega sketch: print one line, then exit.

This sample intentionally avoids input so the entry and platform boundary are easy to inspect.

## Trusted Root Sketch

`build.omg` and `host_contracts.omg` are parsed as top-level `target` and `capability` items. Their bodies are still ahead of full validation, but the compiler now records their structure and emits a trust report so we can design the boundary in Omega source instead of inventing a sidecar config format.

For cross-platform hello world, the trusted computing base is tiny:

- `Stdout.write_line`: host claims it can write initialized UTF-8 text to process stdout and report `IOError`.
- `Process.exit`: host claims it can terminate the process with a target-specific observable exit code.

Omega should prove the literal is initialized/UTF-8 and that errors are handled once `Result` exists. Omega should trust the OS wrapper contract only because `build.omg` explicitly accepts `host_contracts`.
