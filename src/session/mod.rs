pub mod io;
mod types;

pub use types::Session;

// Breadcrumb and QueryEntry are only referenced in test code.
#[cfg(test)]
pub use types::{Breadcrumb, QueryEntry};
