<!-- BEGIN WJH RESEARCH OVERVIEW -->
# zeth-v2 — Ethereum Block Proving and Benchmark Harness

**Upstream:** [Zeth](https://github.com/boundless-xyz/zeth) · **Research repository:** [`wjh-x/zeth-v2`](https://github.com/wjh-x/zeth-v2)  
**Related:** [risc0-v2](https://github.com/wjh-x/risc0-v2) (optimized prover) · [N42-zeth](https://github.com/wjh-x/N42-zeth) (N42 chain adapter)

This repository is an instrumented Zeth derivative used to run **Ethereum execution-block proofs** and measure the behavior of a modified RISC Zero prover. It connects RPC/witness preparation, a Reth-based stateless execution guest, host validation, zkVM proof generation and receipt verification.

**Division of responsibility:** this repository owns the **Ethereum block-proving workflow and measurement interface**. Segment pipelining, batched FRI Merkle openings, targeted CUDA register limits, scratch compaction, generated-code fusion, buffer pooling and other prover optimizations are implemented in [`risc0-v2`](https://github.com/wjh-x/risc0-v2), **not independently in `zeth-v2`**.

## Research-specific workflow and instrumentation

| Area | What it provides |
| --- | --- |
| Ethereum block input | Fetch blocks and state witnesses through the configured RPC endpoint/proxy, prepare the execution input, and optionally reuse fixed cached inputs for controlled runs. |
| Stateless execution | Run the selected execution guest and check the block execution commitments supported by that guest. |
| Phase timing | Record block retrieval, host validation, proof generation, receipt verification and total execution time (`[TIMING]`). |
| Witness size | Report serialized zkVM input size, state trie nodes, bytecode entries and header bytes (`[METRIC] witness_size`). |
| Execution work | Track guest cycles, RV32IM segment counts and relevant proof-stage statistics. |
| Prover controls | Expose `--segment-po2` and `--keccak-max-po2` for controlled experiment configurations. |
| Correctness | Require successful receipt verification and comparison with the expected journal/block identity for evaluated guest configurations. |

### Representative inputs

| Ethereum mainnet block | Workload characteristics | Role in experiments |
| --- | --- | --- |
| **19,000,000** | **133 transactions**, 246 RV32IM segments at `segment-po2=20`; approximately **2.535 MB** serialized witness input | Pipeline/FRI 2×2, optimization waterfall, register × codegen 2×2, kernel analysis. |
| **19,000,005** | **474 transactions** in the audited cross-workload experiment | Five interleaved baseline/full-stack pairs to test full-stack gains on a second, larger block. |

Inputs, cache hashes, image IDs and release binaries were pinned in the source experiment audit. The benchmark timings refer to **proof generation**, not RPC retrieval or receipt verification. Numbers from different workloads/builds must not be merged into a single incremental speedup claim.

## Example: prove an Ethereum block

Prerequisites: a compatible Ethereum RPC source with required historical witness data (possibly via the included proxy), a correctly built Zeth CLI, and a compatible RISC Zero prover (`r0vm`). Use a locally controlled endpoint; never commit secrets or private RPC tokens.

Terminal 1 — when the RPC needs the Zeth witness proxy:

```bash
cargo run --release --bin zeth-rpc-proxy -- \
  --rpc-url "$ETHEREUM_RPC_URL" \
  --bind-address 127.0.0.1:18545
```

Terminal 2 — generate and verify a Composite proof with the experimental prover:

```bash
ETH_RPC_URL=http://127.0.0.1:18545 \
RISC0_PROVER=ipc \
RISC0_SERVER_PATH=/absolute/path/to/risc0/target/release/r0vm \
cargo run --release --bin cli -- \
  prove --block 19000000 --segment-po2 20 --keccak-max-po2 17
```

For host-only validation (without generating a zkVM receipt):

```bash
ETH_RPC_URL=http://127.0.0.1:18545 \
cargo run --release --bin cli -- validate --block 19000000
```

The parameters above match one tested workload; `--keccak-max-po2 17` was used because a larger configuration exhausted memory in earlier testing. An RPC proxy must actually support the necessary archive/proof operations; simply supplying an arbitrary endpoint is insufficient.

## Where the performance improvements come from

The **five-pair, interleaved** Ethereum #19,000,005 experiment measured Composite receipt generation at **558.6190 s (baseline)** and **339.3434 s (full optimized prover)**: **39.25% lower generation time** on the evaluated RTX 5080 system. Both configurations used the same Zeth execution workload and proof parameters, and all accepted runs verified their receipts.

This result is evidence for the **integrated RISC Zero prover stack run through Zeth**, **not** a claim that Zeth host instrumentation alone accelerates proof generation by 39.25%. For the mechanisms and their separate control groups, see the detailed [risc0-v2 research overview](https://github.com/wjh-x/risc0-v2).

## Experimental discipline and limitations

- Keep the input witness bytes, guest image, target block hash, segment size, Keccak setting, receipt kind, binary fingerprint and GPU configuration fixed within comparisons.
- Measure `[TIMING] 3. 生成证明` (Composite generation) separately from receipt verification and total wall-clock time.
- Record full errors and retries instead of silently dropping failed runs. Successful-run latency is not a proof of absence of CUDA OOM/watchdog issues.
- The reported results are for a **single RTX 5080 platform**; they do not rank this implementation against other zkVMs or establish performance on other GPUs.
- Zeth/guest correctness and accepted public output require the corresponding guest and application verification contract; a passing host-side validation alone is not a zkVM receipt.

## Attribution

Derived from the open-source **Zeth** project with **RISC Zero** and **Reth** components. Upstream history, licenses and copyright notices remain applicable. This is a research/benchmark fork, not an official Zeth or RISC Zero release. Private raw logs and RPC credentials are not included.
<!-- END WJH RESEARCH OVERVIEW -->

---

## Original upstream README

# Zeth: A RISC Zero zkVM Block Prover for Ethereum

Zeth is an open-source, execution-layer block prover for Ethereum, built on the RISC Zero zkVM and leveraging the power of [Reth](https://reth.rs/).

By using Reth's stateless execution capabilities within the zkVM, Zeth makes it possible to generate a cryptographic proof of a block's validity. This allows you to verify the entire block execution process—from transaction validation to state root updates—without trusting a third-party node provider.

## How It Works

Zeth streamlines the process of proving block execution by taking advantage of reth's stateless execution feature. The core logic involves:

1. **Fetching Data**: Zeth requires an archival Ethereum RPC provider to fetch the block header and an "execution witness." The witness contains all the necessary pre-state data (account info, storage slots, bytecodes) required to execute the block from scratch.
2. **Stateless Execution**: The execution witness and the block data are provided as inputs to the RISC Zero zkVM.
3. **Proving**: Inside the zkVM, the guest program uses reth's stateless validation function to execute all transactions in the block, apply rewards, and compute the final state root.
4. **Journal Output**: The guest program commits the calculated state root to the public journal. The host can then verify this against the known, correct state root from the block header.

## Prerequisites

You'll need the following installed to run Zeth:

1. [Rust](https://www.rust-lang.org/tools/install) (see rust-toolchain.toml for the exact version).
2. The [RISC Zero toolchain](https://dev.risczero.com/api/zkvm/install).

## Building the Project

To build the host CLI and guest programs, run:
```bash
cargo build --release
```

### Deterministic Guest Builds with Docker

For ZK proofs, it is critical that the guest binary is built deterministically. This ensures that every developer builds the exact same guest program, resulting in the same Image ID, which is essential for verification.

This project is configured to use Docker to achieve reproducible builds. If you have Docker installed, you can enable this feature by setting an environment variable:
```bash
RISC0_USE_DOCKER=1 cargo build --release
```
This will build the guest programs inside a controlled Docker environment, guaranteeing a deterministic output. The project's release workflow uses this method to build the official guest binaries.

## Usage

Zeth requires an archival Ethereum RPC provider to fetch block and state data. You can provide this using the `ETH_RPC_URL` environment variable or the `--eth-rpc-url` command-line argument.

### RPC Provider Requirements

Zeth's core functionality depends on the non-standard `debug_executionWitness` RPC method to generate the execution witness. Not all RPC providers support this method.
If your provider does not support `debug_executionWitness`, you can use the included `zeth-rpc-proxy`. This proxy server will intercept requests, generate the witness locally using a standard archival node, and forward all other requests to your provider. You can run it with:

```bash
ETH_RPC_URL="<YOUR_ARCHIVAL_RPC_URL>" cargo run --release --bin zeth-rpc-proxy
```

You can then point the Zeth CLI to the proxy, which runs on `127.0.0.1:8545` by default.

### CLI Commands

The CLI provides two main commands for interacting with blocks.

```
$ cargo run --bin cli -- --help

Simple CLI to create Ethereum block execution proofs

Usage: cli [OPTIONS] --eth-rpc-url <ETH_RPC_URL> <COMMAND>

Commands:
  prove     Validate the block and generate a RISC Zero proof
  validate  Validate the block on the host machine, without proving
  help      Print this message or the help of the given subcommand(s)

Options:
      --eth-rpc-url <ETH_RPC_URL>  URL of the Ethereum RPC endpoint to connect to [env: ETH_RPC_URL=]
      --block <BLOCK>              Block number, tag, or hash (e.g., "latest", "0x1565483") to execute [default: latest]
      --cache-dir <CACHE_DIR>      Cache folder for input files [default: ./cache]
  -h, --help                       Print help
  -V, --version                    Print version
```

### `validate`

This command runs the block execution logic on your local machine (the "host") without generating a proof. It's useful for quickly verifying that a block can be correctly processed and for populating the local cache.

#### Validate the latest block
```bash
ETH_RPC_URL="<YOUR_RPC_URL>" cargo run --release --bin cli -- validate
```

#### Validate a specific block by number
```bash
ETH_RPC_URL="<YOUR_RPC_URL>" cargo run --release --bin cli -- validate
```

Upon first run, this will fetch the necessary data from the RPC and save it to the cache/ directory. Subsequent runs for the same block will be much faster as they will use the cached data.

### `prove`

This command first validates the block on the host and then proceeds to generate a full cryptographic proof of execution inside the RISC Zero zkVM.

#### Prove the latest block  
```bash
ETH_RPC_URL="<YOUR_RPC_URL>" cargo run --release --bin cli -- prove
```

**Developer Mode**: For faster iteration during development, you can generate a "mock" proof by enabling RISC Zero's dev mode. This runs the guest code in the zkVM but skips the expensive proof generation step.

#### Generate a mock proof for a specific block
```bash
RISC0_DEV_MODE=1 ETH_RPC_URL="<YOUR_RPC_URL>" cargo run --release --bin cli -- prove 19000000
```

#### Proving Pre-Merge (Proof-of-Work) Blocks

By default, Zeth only supports proving post-merge (Proof-of-Stake) blocks. This is because the underlying `reth-stateless` library does not fully validate the total difficulty, making proofs of pre-merge blocks underconstrained.

For development and research purposes, you can enable the proving of historical blocks by using the `unsafe-pre-merge` feature flag:
```bash
ETH_RPC_URL="<YOUR_RPC_URL>" cargo run --release --bin cli --features "unsafe-pre-merge" -- prove --block 1
```

**Warning**: Proofs generated with this flag are not fully sound and should not be used in production.

## Additional Resources

* [RISC Zero Developer Portal](https://dev.risczero.com/)
* [reth \- The Rust Ethereum Book](https://reth.rs/)
* Say hi on the [RISC Zero Discord](https://discord.gg/risczero)
