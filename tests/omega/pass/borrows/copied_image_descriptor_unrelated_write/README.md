This fixture pins a copied record containing an image-backed shared byte view.
A later argument mutates unrelated scalar storage; the captured view still has
length 2. Native and Terminal execution must both exit 0.
