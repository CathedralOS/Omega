The consumer selects `MarkerMark` for the support package's public `Mark`
boundary. Its call to `fetch_mark` reaches `Mark::read` through the private
`read_private` helper. Neither helper authors a reach ceiling.

From `consumer/`, run the hosted program with:

```sh
omega run --both --build-input build.omg --build-input main.omg main.omg
```

Both engines must exit 0 after checking that the provider returns seven.
Changing only the consumer's `main` signature to `reaches none crashes Trap`
must reject the call to `fetch_mark` for contributing `Mark`.
