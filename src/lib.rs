//! This crate implements a client library and a player for the [rocket sync tracker](https://github.com/rocket/rocket).
//! You can connect to a rocket tracker, get values from tracks, and live-edit your production.
//!
//! # Usage
//!
//! See the [`rocket`] module.
//!
//! # Low-level API
//!
//! Most of the crate implementation is public in the [`lowlevel`] module.
//! It's not recommended to use the low level API directly, use [`rocket`] instead.
//!
//! The [`lowlevel::client`] module contains the types which you need to connect to a Rocket tracker.
//!
//! # Features
//!
//! | Feature   | Purpose                                                                           |
//! | ---       | ---                                                                               |
//! | `serde`   | Derive [serde](https://crates.io/crates/serde)'s traits on the [`Tracks`]-type     |
//! | `bincode` | Derive [bincode](https://crates.io/crates/bincode)'s traits on the [`Tracks`]-type |
//! | `client`  | Enable the rocket client, making it possible to connect to a tracker              |
//!
//! All features are mutually compatible, but if you choose to use `bincode` as your serialization library,
//! you don't need to use `serde`.
//!
//! Omitting the `client` feature makes the crate function as a player for [`Tracks`], which you can load from a file
//! using the `serde` or `bincode` features.
//! See [`examples/simple.rs`](https://github.com/demoscene-rs/rust-rocket/blob/master/examples/simple.rs).

pub mod lowlevel;
pub mod rocket;

pub use lowlevel::Tracks;
pub use rocket::{Event, Rocket};
