# Build settings

After an authored Build returns normally, the interpreter still owns its original borrowed Build storage. The settings decoder reads that storage using the instance’s field identities, case selections and byte views. It passes the resulting value to the Build-value protocol for validation. This yields the application, deployment and optimization settings for the Product. [Settings decoder](../settings.rs) · [Build-value protocol](../../inputs/build_execution/build_value.rs)

To change how a Build runs or how a host call suspends and resumes it, start in [Build execution](../README.md). To change how a returned configuration becomes a compiler request, start in [Product planning](../../planning/product.rs).
