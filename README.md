# Kore

Kore is a WASM evaluation script for the [Telegraph Protocol](https://telegraphprotocol.com) that scores miner responses against ground truth with high accuracy and gaming resistance. It enters the Testing Cohort to compete for Canonical status, aiming to become the authoritative quality layer for intelligent signal ranking on the network.

## How it works

1. The Telegraph validator calls `evaluate(ground_truth, miner_response)` with two JSON payloads.
2. Kore compares the miner's answer against the known ground truth.
3. It returns a score between `0.0` (completely wrong) and `1.0` (perfect match).
4. Scores are used to rank miners — higher scores mean more traffic and earnings.
5. If Kore outperforms the current Canonical script, it autonomously replaces it after 3 test epochs.

## Scoring approach

| Input type | Method |
|---|---|
| Exact match | 1.0 |
| Numeric values | Relative error: `1.0 - |gt - mr| / gt` |
| Text/string values | Jaccard word-overlap similarity |
| Type mismatch | 0.0 |

## Repository structure

```
kore/
├── Cargo.toml          # Rust project config + WASM target
├── src/
│   ├── lib.rs          # WASM entry point — exports evaluate()
│   └── scoring.rs      # Core scoring logic + unit tests
├── tests/              # Integration tests
├── scripts/            # Foundry scripts for on-chain submission
└── README.md
```

## Quick start

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- `wasm-pack` for building the WASM binary

```bash
# Install wasm-pack
cargo install wasm-pack

# Run tests
cargo test

# Build for WASM
wasm-pack build --target web
```

### Build for Telegraph sandbox

```bash
# Release build (optimized for size)
wasm-pack build --target web --release

# The output binary is in pkg/kore_bg.wasm
```

## Tech stack

| Layer | Technology |
|---|---|
| Language | Rust |
| Compilation target | WebAssembly (WASM) |
| Serialization | serde + serde_json |
| WASM bindings | wasm-bindgen |
| Blockchain | Base Sepolia (testnet) |
| Contract interaction | Foundry (forge + cast) |

## Registering on-chain

After building, register your script on the Telegraph Diamond contract:

```bash
# Compute the WASM binary hash
WASM_HASH="0x$(sha256sum pkg/kore_bg.wasm | awk '{print $1}')"

# Submit to the Diamond (testnet)
cast send $DIAMOND \
  "registerScript(string,bytes32)" \
  "ipfs://<your-cid>" \
  "$WASM_HASH" \
  --rpc-url $EVM_HTTP_URL \
  --private-key $PRIVATE_KEY
```

Your script enters the **Testing Cohort** — a 10% shadow sample of Validator rounds. If it outperforms the current Canonical, it replaces it after 3 test epochs.

## License

MIT
