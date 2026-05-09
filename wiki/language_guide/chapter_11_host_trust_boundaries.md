# Chapter 11: Host Trust Boundaries

Omega should not pretend the trusted root does not exist.

The current direction is that project build policy is Omega source, usually in `build.omg`, rather than a sidecar config format. That file selects a target host provider, enables root capabilities, and explicitly names the trust policy accepted by the build.

```omega
target cross_platform_cli {
    host: StandardHost {
        stdout = enabled
        process = enabled
        filesystem = disabled
        threads = disabled
        clock = disabled
        random = disabled
    }

    trust host_contracts
}
```

Capabilities describe what host objects promise. Omega code can be proven relative to those contracts, but the host implementation itself is part of the trusted computing base.

```omega
capability Stdout {
    state write_line(text: String) -> Result<(), IOError>
        requires text.utf8
        ensures result.Ok => stdout_observed(text + "\n") || host_buffered(text + "\n")
        ensures result.Err(e) => e is IOError
        trusted host
}
```

In proof vocabulary, a host capability state has a contract:

- `requires` lists the facts the caller must provide.
- `ensures` lists the guarantees the host operation contributes.
- `trust` names the authority used when those guarantees cannot be proven from Omega source.

Omega should still prove the requirements before the call. For example, it can prove a buffer is initialized, a slice length is valid, and an error result is handled. What it cannot prove from Omega code is that the OS implementation really obeys `ensures`. That is exactly what `trust host` means.

The important split is:

- `Proven`: Omega verified this from Omega code.
- `Checked`: a runtime check enforces this before continuing.
- `Trusted`: the target host/runtime claims this contract is true.

Unchecked modes should be explicit and auditable:

```omega
target local_unchecked {
    host: StandardHost {
        stdout = enabled
        process = enabled
    }

    trust host_contracts
    trust unchecked invariant_proofs
}
```

Compiler artifacts should list every trusted contract and unchecked policy. A build with proofs or contracts disabled should be stamped loudly rather than silently behaving like a normal safe build.

For hello world, the trusted root is intentionally tiny:

- `Stdout.write_line` maps to the target OS stdout mechanism.
- `Process.exit` maps to the target OS process termination mechanism.
- Omega proves the string literal is initialized and valid UTF-8 once those proof passes exist.
- Omega trusts the host wrapper only because `build.omg` explicitly accepts `host_contracts`.

## Standard Library vs Host Bindings

The standard library should mostly be portable Omega code. It can provide data structures, algorithms, string helpers, slices, formatting, numeric helpers, and high-level APIs. Those pieces should be proven like any other Omega code.

Host bindings are the bottom edge where Omega touches the outside world. They are target-specific and trusted:

- Windows should normally bind documented Win32 APIs such as `WriteFile`, `GetStdHandle`, and `ExitProcess`.
- Linux can choose a raw syscall ABI for tiny targets.
- Darwin should usually bind through `libSystem`.

That means Windows may still involve system DLL imports. Those are not Omega runtime DLLs; they are the OS user-mode ABI boundary. A good Windows target can still mean:

- no Omega runtime DLL
- no C runtime dependency
- no transpiled C host layer
- explicit imports from Windows system DLLs
- audited trust policies for loader, ABI, and Win32 contracts
