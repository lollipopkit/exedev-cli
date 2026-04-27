#[tokio::main]
async fn main() {
    if let Err(err) = exedev_ctl::run().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
