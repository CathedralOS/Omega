# Extent and external-loan foundation

Contracts: [extent authority](../../../../wiki/spec/resources/extents.md) and
[device custody](../../../../wiki/spec/resources/device_access.md).
[lib.rs](src/lib.rs) owns normalized authority, conservation, and external-loan
carriers; [mapping.rs](src/mapping.rs) owns pending/active mapping and
reclamation.

These non-clonable carriers validate exact authority and provider receipts
and return consumed inputs on rejection. This validation does not run a
provider, establish a source domain, or prove that firmware/devices honor their
admitted claims. The source conservation contract allows
compatible common lineage; this crate's limit to sibling merges is not language
semantics. Source resource-frontier integration and concrete DMA/mapping
providers are separately checked consumers.
