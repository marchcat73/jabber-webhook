use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::Deserialize;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};
use xmpp::jid::BareJid;
use xmpp::message::send::MessageSettings;
use xmpp::{Agent, ClientBuilder};

// ---- Структуры ----
#[derive(Clone)]
struct AppState {
    xmpp_agent: Arc<Mutex<Option<Agent>>>,
    recipient_jid: BareJid,
}

#[derive(Debug, Deserialize)]
struct WebhookPayload {
    name: String,
    email: String,
    message: String,
}

#[derive(Debug, serde::Serialize)]
struct WebhookResponse {
    success: bool,
    message: String,
}

// ---- XMPP логика ----
/// Инициализация XMPP агента
async fn init_xmpp_agent(
    bot_jid: &str,
    bot_password: &str,
) -> Result<Agent, anyhow::Error> {
    info!("Connecting to an XMPP server {}", bot_jid);

    // В xmpp 0.7.0 используется BareJid для подключения
    let bare_jid = BareJid::from_str(bot_jid)?;

    // Создаем агент через ClientBuilder
    let agent = ClientBuilder::new(bare_jid, bot_password).build();

    info!("XMPP agent successfully created");
    Ok(agent)
}

// ---- HTTP обработчики ----
async fn webhook_handler(
    State(state): State<AppState>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    info!(
        "Received message from {} ({})",
        payload.name, payload.email
    );

    let jabber_message = format!(
        "New message from the site:\n\nName: {}\nEmail: {}\nMessage:\n{}",
        payload.name, payload.email, payload.message
    );

    let mut agent_guard = state.xmpp_agent.lock().await;

    match agent_guard.as_mut() {
        Some(agent) => {
            // Формируем настройки сообщения
            let settings = MessageSettings::new(state.recipient_jid.clone(), &jabber_message);

            // Отправляем сообщение.
            // Метод асинхронный, но не возвращает Result, так как передача
            // происходит через внутренние каналы tokio-xmpp.
            agent.send_message(settings).await;

            info!("The message was successfully sent to Jabber.");
            (
                StatusCode::OK,
                Json(WebhookResponse {
                    success: true,
                    message: "The message has been sent successfully.".to_string(),
                }),
            )
        }
        None => {
            error!("XMPP agent not initialized");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(WebhookResponse {
                    success: false,
                    message: "The service is temporarily unavailable.".to_string(),
                }),
            )
        }
    }
}

// ---- Главная функция ----
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let bot_jid = std::env::var("BOT_JID").expect("BOT_JID not specified in .env");
    let bot_password = std::env::var("BOT_PASSWORD").expect("BOT_PASSWORD not specified in .env");
    let recipient_jid_raw = std::env::var("RECIPIENT_JID").expect("RECIPIENT_JID not specified in .env");

    let server_host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let server_port = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8082".to_string())
        .parse::<u16>()
        .expect("SERVER_PORT must be a number");

    let recipient_jid = BareJid::from_str(&recipient_jid_raw)?;

    let shared_agent = Arc::new(Mutex::new(None));

    info!("Initializing the XMPP agent...");
    match init_xmpp_agent(&bot_jid, &bot_password).await {
        Ok(agent) => {
            info!("XMPP agent initialized successfully");
            *shared_agent.lock().await = Some(agent);
            // Фоновая задача keepalive больше не нужна: Agent внутри tokio-xmpp
            // сам запускает необходимые задачи для поддержания соединения.
        }
        Err(e) => {
            error!("Failed to initialize XMPP agent: {}", e);
        }
    };

    let state = AppState {
        xmpp_agent: shared_agent,
        recipient_jid,
    };

    let app = Router::new()
        .route("/webhook", post(webhook_handler))
        .with_state(state);

    let addr = format!("{}:{}", server_host, server_port);
    info!("The server is running on {}", addr);
    info!("POST endpoint: http://{}/webhook", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
