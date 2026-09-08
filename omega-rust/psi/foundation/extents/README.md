# Extent and external-loan foundation

Contracts: [extent authority](../../../../wiki/spec/resources/extents.md) and
[device custody](../../../../wiki/spec/resources/device_access.md).
[lib.rs](src/lib.rs) owns normalized authority, conservation, and external-loan
carriers; [mapping.rs](src/mapping.rs) owns pending/active mapping and reclamation.

These non-clonable carriers validate exact authority and provider receipts and
return consuming inputs on rejection. Foundation validation does not execute a
provider, establish a source domain, or prove that firmware/devices honor their
admitted claims. The ordinary source conservation contract permits compatible
common lineage; a foundation restriction to sibling merge is not language
semantics. Source resource-frontier integration and concrete DMA/mapping
providers remain separately checked consumers.
