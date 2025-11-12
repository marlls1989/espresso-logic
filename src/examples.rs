//! Complete usage examples for all features
//!
//! This module provides comprehensive examples demonstrating the full range of capabilities
//! in the espresso-logic library.
//!
//! # Quick Navigation
//!
//! - **Boolean Expressions** - High-level expression API with parsing and composition
//! - **Truth Tables** - Manual cover construction with cubes
//! - **PLA Files** - Reading and writing PLA format
//! - **Multiple Outputs** - Multi-output function minimisation
//! - **BDD Operations** - Working with Binary Decision Diagrams
//! - **Concurrent Execution** - Thread-safe parallel minimisation
//! - **Low-Level API** - Direct access to Espresso C library
//!
//! # Examples Overview
//!
//! All examples can be run with `cargo run --example <name>`:
//!
//! - `boolean_expressions` - Expression API demonstration
//! - `xor_function` - Simple XOR minimisation
//! - `pla_file` - PLA file I/O
//! - `concurrent_transparent` - Thread-safe execution
//! - `threshold_gate_example` - Complex multi-output design
//! - And many more in the `examples/` directory
//!
//! # Comprehensive Examples Guide
//!
//! For detailed examples with complete code, see the embedded documentation below:
#![doc = include_str!("../docs/EXAMPLES.md")]
