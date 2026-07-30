//! flutter_rust_bridge surface over [`websave_core`].
//!
//! Thin by design: this crate holds no logic of its own, only the shape the
//! Dart side sees. It sits alongside `ffi/` (UniFFI, for the Swift menubar
//! app) — two bindings over one engine, both deliberately small.

pub mod api;
mod frb_generated;
