# AGENTS.md — Kore Project Context

## What is Kore

Kore is a **WASM evaluation script** for the [Telegraph Protocol](https://telegraphprotocol.com) that scores Miner responses against ground truth. It competes in **Track 2: Script Authors** of the Telegraph Hackathon (Aug 17–31, 2026).

**Judging**: 75% Normalized Performance + 25% X Engagement & Updates (must tag `@Telegraphprotoc`).

**Prize**: $1,000 (1st), $500 (2nd), $200 (3rd) for Script Author track.

## Tech Stack

- Rust → WASM via `wasm-pack` + `wasm-bindgen`
- `serde` + `serde_json` for serialization
- Target: `cdylib` + `rlib` crate types
- On-chain: Base Sepolia testnet, Foundry (`forge`/`cast`)

## Repository Structure

```
kore/
├── Cargo.toml          # Rust config + WASM target
├── AGENTS.md           # This file
├── src/
│   ├── lib.rs          # WASM entry point — exports evaluate()
│   └── scoring.rs      # Core scoring logic + unit tests
├── tests/              # Empty — no integration tests yet
├── scripts/            # Placeholder — no Foundry scripts yet
│   └── README.md
├── tweets/             # Tweet drafts for X updates
│   ├── kore-hackathon-update-1.md
│   └── kore-hackathon-update-2.md
└── README.md
```

## Current State (as of Aug 19, 2026)

### ✅ Done

- **WASM entry point** (`src/lib.rs`): `evaluate(ground_truth, miner_response) -> f64` exported via `wasm_bindgen`. Deserializes JSON, delegates to scoring, returns 0.0 on parse failure.
- **Full scoring engine** (`src/scoring.rs`):
  - Exact match → 1.0
  - Numeric relative error with configurable tolerance (default 0.1%)
  - Normalized Jaccard text similarity (lowercase + punctuation-stripped)
  - Boolean handling: bool-vs-bool exact, bool-vs-number coercion (true→1, false→0)
  - Type coercion: string-encoded numbers parsed when GT is numeric
  - Confidence-based anti-gaming: `score = answer_score * (1 - confidence * 0.5)` when wrong
  - Fuzzy object scoring: key-by-key recursive comparison with averaged similarity
  - Fuzzy array scoring: index-by-index comparison
  - Metadata tolerance override via `metadata.tolerance`
  - All scores clamped to [0.0, 1.0]
- **41 unit tests** passing — covers all 4 subnet response shapes (ItsAI, Bitmind, DeSearch, Groq LLM)
- **README** with full protocol explanation, build instructions, on-chain registration flow

### 🔴 Not Done

#### 1. WASM build verification
- `wasm-pack build --target web` hasn't been confirmed working
- Need to verify the full build pipeline works end-to-end

#### 2. Integration tests
- `tests/` directory is empty
- Should cover end-to-end `evaluate()` calls with real subnet response payloads

#### 3. Foundry scripts
- `scripts/` has only a README placeholder
- Need on-chain registration + update scripts for Base Sepolia

### 🟡 Not Started

- **X/Twitter updates** — Need to post every 1-2 days for the 25% engagement score
- **Foundry scripts** — On-chain registration + update
- **WASM build verification** — Confirm build pipeline works

## Telegraph Subnet Response Formats

These are the actual response shapes Kore needs to handle:

| Subnet | ID | Response Shape |
|---|---|---|
| ItsAI | 32 | `{ answer: 0 \| 1 }` (integer classification) |
| Bitmind | 34 | `{ isAI: bool, confidence: float }` |
| DeSearch | 101 | `{ articles: [{ title, snippet, source }] }` (complex) |
| Groq LLM | 102 | `{ choices: [{ message: { content: string } }] }` (OpenAI format) |

**Key insight**: The eval script needs to handle at least 4 distinct response shapes. The current code only handles flat numbers and strings correctly.

## How the Protocol Works

1. **Validator** calls `evaluate(ground_truth, miner_response)` with two JSON payloads
2. Kore compares miner's answer against known ground truth
3. Returns 0.0–1.0 score
4. Scores rank miners — higher = more traffic + USD
5. If Kore outperforms current Canonical script, it replaces it after 3 test epochs

## Hackathon Timeline

| Track | Dates | Status |
|---|---|---|
| Track 1: Miners | Aug 17–31 | Active |
| Track 2: Script Authors | Aug 17–31 | **Active — Kore competes here** |
| Track 3: Applications | Aug 31–Sep 7 | Starts after Track 1+2 close |
| Winner Selection | Sep 8–18 | |
| Announcement | Sep 19–25 | |

**Guardrail**: An Intent must have ≥3 active Miners and ≥100 real requests from Track 3 apps to be eligible for global cash prizes.

## Non-Negotiable Rules

1. Track 3 apps must use real Telegraph Miners (no mocks)
2. Miners and Scripts must remain live throughout Track 3
3. All X updates must be public and tag `@Telegraphprotoc`
4. Artificial inflation of metrics = disqualification
5. Must join the official Hackathon Discord

## Next Steps (Priority Order)

1. **Verify WASM build** — Run `wasm-pack build --target web` and confirm it works
2. **Write integration tests** — Cover all 4 subnet response shapes with end-to-end `evaluate()` calls
3. **Build Foundry scripts** — On-chain registration + update for Base Sepolia
4. **Post X updates** — Every 1-2 days for the 25% engagement score (drafts in `tweets/`)
5. **Performance optimization** — Profile scoring for large payloads (DeSearch article arrays)

## Tweet Strategy

- Post updates on X every 1-2 days
- Always tag `@Telegraphprotoc`
- Focus on: what's built, what's next, technical insights
- Current draft: `tweets/kore-hackathon-update-1.md`
- Voice: specific numbers, no AI vocabulary, hook-first

## Build Commands

```bash
# Run tests
cargo test

# Build for WASM
wasm-pack build --target web

# Release build (optimized for size)
wasm-pack build --target web --release

# Compute WASM hash for on-chain registration
sha256sum pkg/kore_bg.wasm
```
