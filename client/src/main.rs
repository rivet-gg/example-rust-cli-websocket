use anyhow::{Context, Result};
use futures_util::{
    future::Either as FutureEither,
    stream::{SplitSink, SplitStream},
    FutureExt, Sink, SinkExt, StreamExt, TryStreamExt,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

type MyWebSocketStream = WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let rivet_client_token = std::env::var("RIVET_CLIENT_TOKEN")?;

    // Build client
    let raw_client = rivet_matchmaker::Builder::dyn_https()
        .middleware(tower::layer::util::Identity::new())
        .sleep_impl(None)
        .build();
    let config = rivet_matchmaker::Config::builder()
        .set_uri("https://matchmaker.api.rivet.gg/v1")
        .set_bearer_token(rivet_client_token)
        .build();
    let mm_api = rivet_matchmaker::Client::with_config(raw_client, config);

    // Find lobby
    println!("Finding lobby");
    let lobby_res = mm_api.find_lobby().game_modes("default").send().await?;
    let lobby = lobby_res.lobby().context("lobby_res.lobby")?;
    let player_token = lobby
        .player()
        .and_then(|x| x.token())
        .context("lobby.player.token")?;
    let port = lobby
        .ports()
        .and_then(|x| x.get("default"))
        .context("lobby.ports[\"default\"]")?;
    let host = port.host().context("port.host")?;
    let proto = if port.is_tls().context("port.is_tls")? {
        "wss"
    } else {
        "ws"
    };
    let url = format!("{proto}://{host}");

    // Connect ot server
    println!("Connecting to {url}");
    let (ws_stream, _) = tokio_tungstenite::connect_async(url)
        .await
        .context("failed to connect")?;
    let (mut write, read) = ws_stream.split();
    println!("Connected");

    // Send player token
    println!("Sending player token {player_token}");
    write.send(Message::text(player_token)).await?;

    // Build input/ouptut futures
    let read_stdin_fut = read_stdin(write);
    let write_stdout_fut = write_stdout(read);

    // Wait for input to close or socket to close
    futures_util::pin_mut!(read_stdin_fut, write_stdout_fut);
    match futures_util::future::try_select(read_stdin_fut, write_stdout_fut).await {
        Ok(_) => Ok(()),
        Err(x) => match x {
            FutureEither::Left((err, _)) => Err(err),
            FutureEither::Right((err, _)) => Err(err),
        },
    }
}

async fn read_stdin(mut sink: SplitSink<MyWebSocketStream, Message>) -> Result<()> {
    let mut stdin = tokio::io::stdin();
    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(err) => {
                return Err(err.into());
            }
        };
        buf.truncate(n);
        let str = String::from_utf8(buf)?;
        sink.send(Message::text(str)).await?;
    }

    println!("stdin closed");

    Ok(())
}

async fn write_stdout(mut stream: SplitStream<MyWebSocketStream>) -> Result<()> {
    while let Some(msg) = stream.try_next().await? {
        println!("Message: {:?}", msg);
    }

    println!("Socket closed");

    Ok(())
}
