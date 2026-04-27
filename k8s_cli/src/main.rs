#[tokio::main]
async fn main() {
    if let Err(err) = exedev_k8s::run().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
