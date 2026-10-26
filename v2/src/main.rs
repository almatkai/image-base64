#[tokio::main(worker_threads = 2)]
async fn main() {
    if let Err(error) = tezbar_extension_sdk::serve(image_base64_native::Adapter::default()).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
