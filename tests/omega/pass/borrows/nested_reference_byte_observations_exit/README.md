# Byte observations across ordered reference crossings

After ordinary package review:

```sh
omega run --both tests/omega/pass/borrows/nested_reference_byte_observations_exit/main.omg
```

Both engines exit 70. Each reference is stored after a nonzero stamp field.
Length and equality must load both pointer words in order and then observe
the original text's two UTF-8 bytes, not either descriptor or a copied referent.
