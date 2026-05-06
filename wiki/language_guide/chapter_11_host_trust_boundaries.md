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
