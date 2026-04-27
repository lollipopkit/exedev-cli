#[tokio::main]
async fn main() {
    if let Err(err) = exedevctl::run_k8s().await {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
