// Target publication consumes retained canonical choices. The old checked
// source preparation/derivation path has no replacement producer here.
mod prepared_providers;
pub use prepared_providers::ProviderPublicationCatalog;
mod provider_publication;
pub use provider_publication::{PreparedTargetProviderPublication, provider_selection_roots};
