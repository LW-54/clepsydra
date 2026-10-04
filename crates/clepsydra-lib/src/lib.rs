//! The public Clepsydra facade.
//!
//! Most users only need the two macros re-exported here. The core crate remains
//! available for advanced topology and symbolic-expression use.

pub use clepsydra_core;
pub use clepsydra_core::runtime::{
    RuntimeError, RuntimeEvaluator, evaluate_config, evaluate_config_array,
};
pub use clepsydra_macros::{clepsydra, clepsydra_closure};
