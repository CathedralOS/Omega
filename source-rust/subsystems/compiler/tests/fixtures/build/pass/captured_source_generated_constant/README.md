# Captured source generates a checked, sealed artifact

The generator stages the handwritten `main.omg`, the declaration in
`asset.txt` as `generated.omg`, and the receiving Build as `finish.omg`. The
receiver calls the handwritten code to check the generated value, then
publishes the same declaration bytes as its required output.

Run this from the fixture directory.

```sh
omega build --build-input build.omg --build-input main.omg --build-input asset.txt --build-input finish.template build.omg
```

The completed set contains `files/generated.omg` with the contents of
`asset.txt`. An invalid declaration in that asset fails in `generated.omg`.
