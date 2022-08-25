use anyhow::{Context, Result};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let rivet_client_token = std::env::var("RIVET_CLIENT_TOKEN")?;

    let raw_client = rivet_matchmaker::Builder::dyn_https()
        .middleware(tower::layer::util::Identity::new())
        .sleep_impl(None)
        .build();
    let config = rivet_matchmaker::Config::builder()
        .set_uri("https://matchmaker.api.rivet.gg/v1")
        .set_bearer_token(rivet_client_token)
        .build();

    let mm_api = rivet_matchmaker::Client::with_config(raw_client, config);

    println!("Finding lobby");
    let lobby_res = mm_api.find_lobby().game_modes("default").send().await?;
    let lobby = lobby_res.lobby().context("lobby_res.lobby")?;
    let host = lobby
        .ports()
        .and_then(|x| x.get("default"))
        .and_then(|x| x.host())
        .context("lobby.ports[\"default\"].host")?;
    println!("Connecting to {}", host);

    Ok(())
}
