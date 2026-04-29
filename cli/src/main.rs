#[tokio::main]
async fn main() {
    if let Err(err) = exedev_ctl::run().await {
        eprintln!(
            "{}",
            exedev_core::terminal::error(format!("error: {err:#}"))
        );
        std::process::exit(1);
    }
}
