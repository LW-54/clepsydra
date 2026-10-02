# Clepsydra

Clepsydra turns a description of a directed flow system into a Rust evaluator.

The model has three building blocks:

- **Buckets** hold volume up to a configured capacity and forward overflow to a target node.
- **Sinks** hold any amount of volume.
- **Flows** transfer a fixed volume from one node to another.

At compile time, Clepsydra validates the topology, builds a symbolic representation of the transfers, and generates Rust code. At runtime, the generated evaluator accepts the current balance of each named node and returns the resulting balances.

## Installation

Clepsydra is currently experimental. The public facade crate is `clepsydra-lib`; use the Git repository until a release is published:

```toml
[dependencies]
clepsydra-lib = { git = "https://github.com/LW-54/clepsydra" }
```

`clepsydra-lib` re-exports the public macros. `clepsydra-core` and
`clepsydra-macros` remain separate workspace crates so they can evolve and be
published in dependency order.

## Usage

### Configuration

Configuration describes `nodes` and fixed-volume `flow` entries. A node with `capacity` is a bucket; a node without it is a sink. Buckets may use `target` to forward overflow. The optional defaults are `default_bucket_target`, `default_flow_source`, and `default_flow_target`.

TOML, JSON, and YAML are supported. File formats are selected by `.toml`, `.json`, `.yaml`, or `.yml` extension; inline input is tried as TOML, then JSON, then YAML. Banking vocabulary aliases are also accepted: `accounts`/`nodes`, `transfers`/`flow`, `amount`/`volume`, and corresponding account/transfer default names.

The complete schema is available in [`clepsydra.schema.json`](clepsydra.schema.json). Regenerate it with `devenv tasks run schema:generate` or `cargo run -p clepsydra-core --example generate-schema`.

```rust
// Default file: clepsydra.toml.
let evaluate = clepsydra_closure!();

// File or inline configuration.
let evaluate = clepsydra_closure!("config/ledger.yaml");
let evaluate = clepsydra_closure!(r#"[nodes.sink]"#);
```

### Evaluators

```rust
use clepsydra_lib::clepsydra_closure;

let evaluate = clepsydra_closure!(r#"
    [nodes.sink]

    [nodes.checking]
    capacity = 10
    target = "sink"

    [[flow]]
    source = "checking"
    target = "sink"
    volume = 3
"#);

let next = evaluate([
    ("checking", 7),
    ("sink", 0),
]);

assert_eq!(next, [
    ("checking", 4),
    ("sink", 3),
]);
```

The named macro generates an evaluator and topology metadata:

```rust
use clepsydra_lib::clepsydra;

clepsydra!(Network, "clepsydra.toml");

let next = Network::evaluate([
    ("checking", 7),
    ("sink", 0),
]);
```

`clepsydra!` also accepts no configuration argument or inline configuration. With no identifier, it generates a struct named `Clepsydra`:

```rust
clepsydra!();
clepsydra!(Network, r#"[nodes.sink]"#);
```

The closure macro expands to a value with this shape, where `N` is the number of configured nodes:

```rust
FnOnce(
    [(&'static str, u64); N]
) -> [(&'static str, u64); N]
```

The named macro generates a unit struct with this interface:

```rust
pub struct Network;

impl Network {
    pub const NODE_COUNT: usize = N;
    pub const NAMES: [&'static str; N] = /* topology names */;

    pub fn evaluate(
        state: [(&'static str, u64); N],
    ) -> [(&'static str, u64); N];
}
```

`NAMES` follows topology construction order. Input names should identify each configured node exactly once. Configuration errors are reported during compilation.

The current API is experimental and may change.

## License

Licensed under either the Apache License, Version 2.0 or the MIT License, at your option:

- [Apache-2.0](LICENSE-APACHE)
- [MIT](LICENSE-MIT)
