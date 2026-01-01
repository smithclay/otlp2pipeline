use anyhow::{bail, Result};
use futures::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::cli::url::resolve_worker_url;
use crate::cli::TailArgs;

pub async fn execute_tail(args: TailArgs) -> Result<()> {
    // Validate signal
    if args.signal != "logs" && args.signal != "traces" {
        bail!("Signal must be 'logs' or 'traces', got: {}", args.signal);
    }

    let base_url = resolve_worker_url(args.url.as_deref())?;

    // Convert https:// to wss:// or http:// to ws://
    let ws_url = if base_url.starts_with("https://") {
        base_url.replace("https://", "wss://")
    } else if base_url.starts_with("http://") {
        base_url.replace("http://", "ws://")
    } else {
        format!("wss://{}", base_url)
    };

    let url = format!("{}/v1/tail/{}/{}", ws_url, args.service, args.signal);

    eprintln!("Connecting to {}...", url);

    let (ws_stream, _) = connect_async(&url).await?;
    let (_, mut read) = ws_stream.split();

    eprintln!(
        "Connected. Streaming {} for service '{}'...",
        args.signal, args.service
    );
    eprintln!("Press Ctrl+C to stop.\n");

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                println!("{}", text);
            }
            Ok(Message::Close(_)) => {
                eprintln!("Connection closed by server");
                break;
            }
            Err(e) => {
                eprintln!("WebSocket error: {}", e);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
