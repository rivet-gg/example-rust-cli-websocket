use anyhow::{Context, Result};
use futures_util::{future, StreamExt, TryStreamExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let rivet_lobby_token = std::env::var("RIVET_LOBBY_TOKEN")?;
    let port = std::env::var("PORT")
        .ok()
        .and_then(|x| x.parse::<u16>().ok())
        .unwrap_or(5000);

    let raw_client = rivet_matchmaker::Builder::dyn_https()
        .middleware(tower::layer::util::Identity::new())
        .sleep_impl(None)
        .build();
    let config = rivet_matchmaker::Config::builder()
        .set_uri("https://matchmaker.api.rivet.gg/v1")
        .set_bearer_token(rivet_lobby_token)
        .build();

    let mm_api = rivet_matchmaker::Client::with_config(raw_client, config);

    mm_api.lobby_ready().send().await?;
    println!("Lobby ready");

    // Create the event loop and TCP listener we'll accept connections on.
    let try_socket = TcpListener::bind(("0.0.0.0", port)).await;
    let listener = try_socket.context("failed to bind")?;
    println!("Listening on {}", port);

    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(accept_connection(stream));
    }

    Ok(())
}

async fn accept_connection(stream: TcpStream) -> Result<()> {
    let addr = stream
        .peer_addr()
        .context("connected streams should have a peer address")?;
    println!("Peer address: {}", addr);

    let ws_stream = tokio_tungstenite::accept_async(stream)
        .await
        .context("error during the websocket handshake occurred")?;

    println!("New WebSocket connection: {}", addr);

    let (write, read) = ws_stream.split();
    // We should not forward messages other than text or binary.
    read.try_filter(|msg| future::ready(msg.is_text() || msg.is_binary()))
        .forward(write)
        .await
        .context("Failed to forward messages")?;

    Ok(())
}
