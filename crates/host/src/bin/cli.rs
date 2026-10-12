// Copyright 2026 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use alloy::{eips::BlockId, primitives::B256, providers::ProviderBuilder};
use anyhow::{Context, ensure};
use clap::{Parser, Subcommand};
use std::{
    cmp::PartialEq,
    fs::{self},
    path::PathBuf,
};
use zeth_host::{BlockProcessor, to_zkvm_input_bytes};

/// Simple CLI to create Ethereum block execution proofs.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// URL of the Ethereum RPC endpoint to connect to.
    #[arg(long, env)]
    eth_rpc_url: String,

    /// Block number, tag, or hash (e.g., "latest", "0x1565483") to execute.
    #[arg(long, global = true, default_value = "latest")]
    block: BlockId,

    /// Cache folder for input files.
    #[arg(long, global = true, default_value = "./cache")]
    cache_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, PartialEq, Eq)]
enum Commands {
    /// Validate the block and generate a RISC Zero proof.
    Prove(ProveCommand),

    /// Validate the block on the host machine, without proving.
    Validate,
}

#[derive(Parser, Debug, PartialEq, Eq)]
struct ProveCommand {
    /// Optional segment limit po2
    #[arg(long, env)]
    segment_po2: Option<u32>,

    /// 控制 guest 侧单个 keccak batch 的最大规模。
    /// 数值越大，请求数通常越少，但显存/内存占用也会明显上升。
    /// Optional keccak batch limit po2
    #[arg(long, env)]
    keccak_max_po2: Option<u32>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    //总开始时刻
    let t_total = std::time::Instant::now();
    tracing_subscriber::fmt::init();

    // This is a hack to ensure that `blst` gets linked into this binary.
    let _ = unsafe { blst::blst_p1_sizeof() };

    let cli = Cli::parse();

    // ensure the cache directory exists
    fs::create_dir_all(&cli.cache_dir).context("failed to create cache directory")?;

    // set up the provider and processor
    let provider = ProviderBuilder::new().connect(&cli.eth_rpc_url).await?;
    let processor = BlockProcessor::new(provider).await?;
    println!("Current chain: {}", processor.chain());

    // 获取区块开始时刻
    let t_fetch = std::time::Instant::now();
    let input = processor.get_input_with_cache(cli.block, &cli.cache_dir).await?;
    // 获取区块所用时长
    let fetch_elapsed = t_fetch.elapsed();
    let block_hash = input.block.hash_slow();
    let input_bytes = to_zkvm_input_bytes(&input)?.len();
    let state_bytes: usize = input.witness.state.iter().map(|v| v.len()).sum();
    let code_bytes: usize = input.witness.codes.iter().map(|v| v.len()).sum();
    let key_bytes: usize = input.witness.keys.iter().map(|v| v.len()).sum();
    let header_bytes: usize = input.witness.headers.iter().map(|v| v.len()).sum();

    println!(
        "Input for block {} ({}): {:.3} MB",
        input.block.number,
        block_hash,
        input_bytes as f64 / 1e6
    );
    // 论文实验指标：分解stateless witness大小，total_input_bytes是实际写入zkVM的序列化输入。
    println!(
        "[METRIC] witness_size block:{} txs:{} total_input_bytes:{} state_nodes:{} state_bytes:{} code_entries:{} code_bytes:{} key_entries:{} key_bytes:{} header_entries:{} header_bytes:{}",
        input.block.number,
        input.block.body.transactions().count(),
        input_bytes,
        input.witness.state.len(),
        state_bytes,
        input.witness.codes.len(),
        code_bytes,
        input.witness.keys.len(),
        key_bytes,
        input.witness.headers.len(),
        header_bytes,
    );
    println!("[TIMING] 1. 获取区块: {:.3}s", fetch_elapsed.as_secs_f64());

    // 验证区块开始时刻
    let t_validate = std::time::Instant::now();
    // always validate
    processor.validate(input.clone()).context("host validation failed")?;
    let validate_elapsed = t_validate.elapsed();
    println!("Host validation successful");
    println!("[TIMING] 2. 验证区块: {:.3}s", validate_elapsed.as_secs_f64());

    // create proof if requested
    if let Commands::Prove(ProveCommand { segment_po2, keccak_max_po2 }) = cli.command {
        // 证明生成开始时刻
        let t_prove = std::time::Instant::now();
        let (receipt, image_id, stats) =
            processor.prove(input, segment_po2, keccak_max_po2).await.context("proving failed")?;
        // 证明生成所用时长
        let prove_elapsed = t_prove.elapsed();
        println!("[TIMING] 3. 生成证明: {:.3}s", prove_elapsed.as_secs_f64());
        // 论文实验指标：RISC0返回的guest cycles和segment数，用于规模扩展曲线。
        println!(
            "[METRIC] guest_cycles segments:{} total:{} user:{} paging:{} reserved:{}",
            stats.segments,
            stats.total_cycles,
            stats.user_cycles,
            stats.paging_cycles,
            stats.reserved_cycles
        );

        // 验证证明开始时刻
        let t_verify = std::time::Instant::now();
        receipt.verify(image_id).context("proof verification failed")?;
        // 验证证明所用时长
        let verify_elapsed = t_verify.elapsed();
        println!("[TIMING] 4. 验证证明: {:.3}s", verify_elapsed.as_secs_f64());

        let proven_hash =
            B256::try_from(receipt.journal.as_ref()).context("failed to decode journal")?;
        ensure!(proven_hash == block_hash, "journal output mismatch");

        // 总时长
        println!("----------------------------------------");
        println!("[TIMING] 汇总:");
        println!("  1. 获取区块:  {:.3}s", fetch_elapsed.as_secs_f64());
        println!("  2. 验证区块:  {:.3}s", validate_elapsed.as_secs_f64());
        println!("  3. 生成证明:  {:.3}s", prove_elapsed.as_secs_f64());
        println!("  4. 验证证明:  {:.3}s", verify_elapsed.as_secs_f64());
        println!(
            "  各阶段之和:   {:.3}s",
            (fetch_elapsed + validate_elapsed + prove_elapsed + verify_elapsed).as_secs_f64()
        );
        println!("  程序总耗时:   {:.3}s", t_total.elapsed().as_secs_f64()); // ← 真实总时长
        println!("----------------------------------------");
    }

    Ok(())
}
