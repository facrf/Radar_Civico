pub mod builder;
pub mod error;
pub mod travessia;

pub use builder::{ArestaRede, GrafoSincronizado, NoRede, RedeGrafo};
pub use error::{GraphError, Result};
pub use travessia::{CaminhoRede, CicloDetectado, GRAU_MAXIMO_PADRAO};
