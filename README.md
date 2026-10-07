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
let evaluate = clepsydra_eval!();

// File or inline configuration.
let evaluate = clepsydra_eval!("config/ledger.yaml");
let evaluate = clepsydra_eval!(r#"[nodes.sink]"#);
```

### Evaluators

```rust
use clepsydra_lib::clepsydra_eval;

let evaluate = clepsydra_eval!(r#"
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

assert_eq!(next, Ok([
    ("checking", 4),
    ("sink", 3),
]));
```

`clepsydra_eval!` accepts names in any order and preserves that order in its
result. Names are checked at runtime, so unknown or duplicate names return an
`EvalError`. For dynamic inputs, `clepsydra_vec_eval!` accepts a borrowed slice
and returns a `Vec`; `clepsydra_map_eval!` accepts and returns a `HashMap`.

The named macro generates both a canonical positional evaluator and the named
adapter:

```rust
use clepsydra_lib::{clepsydra, clepsydra_map_eval, clepsydra_vec_eval};

clepsydra!(Network, "clepsydra.toml");

let next = Network::eval([
    ("checking", 7),
    ("sink", 0),
]);

let canonical = Network::ordered_eval([0, 7]);

let balances = std::collections::HashMap::from([
    ("checking".to_owned(), 7),
    ("sink".to_owned(), 0),
]);
let next_by_name = Network::map_eval(balances)?;
let next_by_vec = clepsydra_vec_eval!("clepsydra.toml")(&[
    ("checking", 7),
    ("sink", 0),
])?;
```

`clepsydra!` also accepts no configuration argument or inline configuration. With no identifier, it generates a struct named `Evaluator`:

```rust
clepsydra!(); // generates `Evaluator`
clepsydra!(Network, r#"[nodes.sink]"#);
```

The evaluation macro expands to a named adapter with this shape, where `N` is
the number of configured nodes:

```rust
for<'state> fn(
    [(&'state str, u64); N]
) -> Result<[(&'state str, u64); N], EvalError>
```

The named macro generates a unit struct with this interface:

```rust
pub struct Network;

impl Network {
    pub const NODE_COUNT: usize = N;
    pub const NAMES: [&'static str; N] = /* topology names */;

    pub fn ordered_eval(state: [u64; N]) -> [u64; N];

    pub fn eval<'state>(
        state: [(&'state str, u64); N],
    ) -> Result<[(&'state str, u64); N], EvalError>;

    pub fn vec_eval<'state>(
        state: &'state [(&'state str, u64)],
    ) -> Result<Vec<(&'state str, u64)>, EvalError>;

    pub fn map_eval(
        state: std::collections::HashMap<String, u64>,
    ) -> Result<std::collections::HashMap<String, u64>, EvalError>;
}
```

The vector and map macro forms are:

```rust
let vector_eval = clepsydra_vec_eval!("clepsydra.toml");
let next = vector_eval(&[("checking", 7), ("sink", 0)])?;

let map_eval = clepsydra_map_eval!("clepsydra.toml");
let next = map_eval(std::collections::HashMap::from([
    ("checking".to_owned(), 7),
    ("sink".to_owned(), 0),
]))?;
```

`NAMES` follows topology construction order, which is also the order expected by
`ordered_eval`. The named adapter maps into that order, evaluates, and restores
the caller's original tuple order. Input names must identify each configured
node exactly once. Configuration errors are reported during compilation.
`map_eval` is the owned, unordered boundary form: it accepts a `HashMap`,
normalizes it through the same generated computation, and returns a `HashMap`.

For runtime configuration or as a fallback when generated code is unavailable,
the facade exposes a checked evaluator:

```rust
use clepsydra_lib::{EvalError, clepsydra};

let config: clepsydra_lib::clepsydra_core::config::Config = toml::from_str(
    "[nodes.sink]\n[nodes.checking]\ncapacity = 10\ntarget = 'sink'",
).unwrap();
let next: Result<_, EvalError> = clepsydra(&config)
    .and_then(|evaluator| evaluator.eval([("checking", 7), ("sink", 0)]));
assert_eq!(next.unwrap(), [("checking", 4), ("sink", 3)]);
```

`clepsydra` returns an owned runtime evaluator with `eval`, `vec_eval`,
`ordered_eval`, and `map_eval` methods. Runtime `ordered_eval` is fallible
because the topology size is known only after configuration is parsed. Its
`node_count()` and `names()` methods provide the runtime equivalents of
`NODE_COUNT` and `NAMES`.

The standalone runtime constructors are:

```rust
let named = clepsydra_eval(&config)?;
let next = named(&[("checking", 7), ("sink", 0)])?;

let vector = clepsydra_vec_eval(&config)?;
let next = vector(&[("checking", 7), ("sink", 0)])?;

let map = clepsydra_map_eval(&config)?;
let next = map(std::collections::HashMap::from([
    ("checking".to_owned(), 7),
    ("sink".to_owned(), 0),
]))?;
```
Runtime configuration errors are returned as `EvalError` because topology
construction happens at runtime.

The current API is experimental and may change.

## License

Licensed under either the Apache License, Version 2.0 or the MIT License, at your option:

- [Apache-2.0](LICENSE-APACHE)
- [MIT](LICENSE-MIT)
