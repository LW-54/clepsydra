//! Core types and algorithms for building and evaluating Clepsydra topologies.

pub mod arena;
pub mod ast;
pub mod config;
pub mod errors;
pub mod runtime;
pub mod symbolic;
pub mod topology;

pub use errors::ConfigError;
