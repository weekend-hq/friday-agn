#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // now for localhost
    let address =
        std::env::var("FRIDAY_AGN_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;

    println!("FRIDAY-AGN API listening on http://{address}");
    axum::serve(listener, backend::app()).await?;
    Ok(())
}
