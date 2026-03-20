use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::HeaderValue,
    response::{
        Sse,
        sse::Event,
    },
    routing::{get, post},
};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

const CHANNEL_CAPACITY: usize = 1024;
const PORT: u16 = 7040;

type AppState = Arc<broadcast::Sender<String>>;

#[tokio::main]
async fn main() {
    let (tx, _) = broadcast::channel::<String>(CHANNEL_CAPACITY);
    let state: AppState = Arc::new(tx);

    let cors = CorsLayer::new()
        .allow_origin([
            "https://axonix.live".parse::<HeaderValue>().unwrap(),
            "https://stream.axonix.live".parse::<HeaderValue>().unwrap(),
        ])
        .allow_methods([axum::http::Method::GET]);

    let app = Router::new()
        .route("/pipe", post(pipe))
        .route("/stream", get(stream))
        .with_state(state)
        .layer(cors)
        .fallback_service(ServeDir::new("docs"));

    let addr = format!("0.0.0.0:{PORT}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: failed to bind to {addr}: {e}");
            eprintln!("hint: is port {PORT} already in use?");
            std::process::exit(1);
        }
    };
    println!("stream_server listening on {addr}");
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("error: server failed: {e}");
        std::process::exit(1);
    }
}

async fn pipe(State(tx): State<AppState>, body: Bytes) {
    let text = String::from_utf8_lossy(&body).into_owned();
    // Broadcast line by line so SSE clients get incremental updates
    for line in text.lines() {
        let _ = tx.send(line.to_owned());
    }
}

async fn stream(
    State(tx): State<AppState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result: Result<String, _>| {
        result.ok().map(|line| Ok(Event::default().data(line)))
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
