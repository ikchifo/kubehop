// Rust guideline compliant 2026-02-21
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> anyhow::Result<()> {
    let config = khop::Config::from_env();
    khop::run(&config)
}
