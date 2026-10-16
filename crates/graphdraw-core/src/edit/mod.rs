//! Edição do documento (marco M1): conversão em curvas, nós, histórico
//! (desfazer/refazer), transformações, alinhamento, encaixe e consultas de
//! seleção.

pub mod convert;
pub mod history;
pub mod nodes;
pub mod ops;
pub mod pages;
pub mod query;
pub mod snap;
pub mod tree;

pub use history::{Editor, MASTER, ObjLoc};
pub use tree::{Container, ObjRef};
