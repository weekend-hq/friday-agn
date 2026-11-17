#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ollama = backend::OllamaClient::from_env()?;
    let address =
        std::env::var("FRIDAY_AGN_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;

    println!("FRIDAY-AGN API listening on http://{address}");
    println!(
        "Ollama endpoint: {}",
        std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_owned())
    );
    println!(
        "Ollama model: {}",
        std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3:8b".to_owned())
    );
    axum::serve(listener, backend::app(ollama)).await?;
    Ok(())
}
