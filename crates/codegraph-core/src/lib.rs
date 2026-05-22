//! Core types shared across codegraph crates: NodeKind, EdgeKind, Node, Edge, errors.

pub mod error;
pub mod kinds;
pub mod model;

pub use error::{Error, Result};
pub use kinds::{EdgeKind, NodeKind};
pub use model::{Edge, Node, NodeId};
