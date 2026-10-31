//! Semantic document model for Kernsetzer.
//!
//! A `Book` describes *what* a manuscript contains (chapters, paragraphs,
//! emphasis, scene breaks, front and back matter), never *how* it is laid
//! out. Layout decisions live in `StyleSettings` (theme + parameters) and in
//! the themes themselves.
//!
//! This crate must not depend on any other setzer crate.

pub mod asset;
pub mod book;
pub mod isbn;
pub mod presets;
pub mod project;
pub mod table;
pub mod text;

pub use asset::Asset;
pub use book::*;
pub use project::{Meta, Project, ProjectError};
pub use table::{ColumnAlign, PlacedCell, Table, TableCell, TableColumn, TableLayout};

/// Version of the `book.json` schema (and `.kernsetzer` container) written by
/// this build. Bump when a change is not backwards compatible, append a
/// step to `project::MIGRATIONS` and commit a fixture of the old version.
pub const FORMAT_VERSION: u32 = 2;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;

    /// Model types are plain data: JSON in `book.json` and over the app's
    /// IPC, comparable in tests. Not `Asset` and `Project`: they hold image
    /// bytes, travel only inside the `.kernsetzer` archive, and asset metadata
    /// is derived from the bytes.
    #[test]
    fn model_types_are_plain_data() {
        fn plain<T: Serialize + DeserializeOwned + Clone + PartialEq + Debug>() {}
        plain::<Book>();
        plain::<Lang>();
        plain::<Metadata>();
        plain::<Series>();
        plain::<Imprint>();
        plain::<Credit>();
        plain::<SectionKind>();
        plain::<Section>();
        plain::<Numbering>();
        plain::<Epigraph>();
        plain::<Block>();
        plain::<Inline>();
        plain::<Table>();
        plain::<TableColumn>();
        plain::<TableCell>();
        plain::<ColumnAlign>();
        plain::<StyleSettings>();
        plain::<BlockRef>();
        plain::<presets::TrimSize>();
        plain::<presets::Retailer>();
        plain::<presets::MinMargins>();
        plain::<Meta>();
    }
}
