#[tokio::main]
async fn main() {
    if let Err(err) = exedev_k8s::run().await {
        eprintln!("{}", exedev_k8s::format_error(format!("error: {err:#}")));
        std::process::exit(1);
    }
}
