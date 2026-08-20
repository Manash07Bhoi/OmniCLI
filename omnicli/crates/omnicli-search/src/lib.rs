pub mod error;
pub mod index;
pub mod search;

pub use error::SearchError;
pub use index::{open_index_db, rebuild_index, IndexStats};
pub use search::{search_query, SearchOptions, SearchResult};
