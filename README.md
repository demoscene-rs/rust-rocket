Rust Rocket
===========

[Documentation](https://docs.rs/rust-rocket/)

A rust implementation of the client library of GNU Rocket.

Basic examples can be found in [examples](examples).
Open a Rocket tracker and try `cargo run --features bincode --example edit`

Features and MSRV
=================

Both `serde` and `bincode` are supported for saving and loading tracks.
Enable the optional features that you want to use in your project:
```
rust-rocket = { version = "0", features = ["bincode"] }
```

The minimum supported Rust version (MSRV) without any optional features is 1.61.
The `bincode` feature requires Rust 1.85.

Links
=====

* [GNU Rocket](https://github.com/rocket/rocket), Primary implementation of Rocket.
* [RocketEditor](https://github.com/emoon/rocket), An alternative server (editor).
