//! The public Clepsydra facade.
//!
//! Most users only need the evaluator macros re-exported here. The core crate
//! remains available for advanced topology and symbolic-expression use.

pub use clepsydra_core;
pub use clepsydra_core::errors::{ConfigError, EvalError, TopologyError};
pub use clepsydra_core::runtime::{
    MapEvaluator, NamedEvaluator, RuntimeEvaluator, clepsydra, clepsydra_eval, clepsydra_map_eval,
    clepsydra_vec_eval,
};
pub use clepsydra_macros::{clepsydra, clepsydra_eval, clepsydra_map_eval, clepsydra_vec_eval};
