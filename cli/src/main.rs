#[tokio::main]
async fn main() {
    if let Err(err) = exedevctl::run().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
