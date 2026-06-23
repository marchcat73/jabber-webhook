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
    info!("Подключение к XMPP серверу как {}", bot_jid);

    // В xmpp 0.7.0 используется BareJid для подключения
    let bare_jid = BareJid::from_str(bot_jid)?;

    // Создаем агент через ClientBuilder
    let agent = ClientBuilder::new(bare_jid, bot_password).build();

    info!("XMPP агент успешно создан");
    Ok(agent)
}

// ---- HTTP обработчики ----
async fn webhook_handler(
    State(state): State<AppState>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    info!(
        "Получено сообщение от {} ({})",
        payload.name, payload.email
    );

    let jabber_message = format!(
        "Новое сообщение с сайта:\n\nИмя: {}\nEmail: {}\nСообщение:\n{}",
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

            info!("Сообщение успешно передано в Jabber");
            (
                StatusCode::OK,
                Json(WebhookResponse {
                    success: true,
                    message: "Сообщение успешно отправлено".to_string(),
                }),
            )
        }
        None => {
            error!("XMPP агент не инициализирован");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(WebhookResponse {
                    success: false,
                    message: "Сервис временно недоступен".to_string(),
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

    // Убраны лишние пробелы в названиях переменных окружения
    let bot_jid = std::env::var("BOT_JID").expect("BOT_JID не задан в .env");
    let bot_password = std::env::var("BOT_PASSWORD").expect("BOT_PASSWORD не задан в .env");
    let recipient_jid_raw = std::env::var("RECIPIENT_JID").expect("RECIPIENT_JID не задан в .env");

    let server_host = std::env::var("SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let server_port = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8082".to_string())
        .parse::<u16>()
        .expect("SERVER_PORT должен быть числом");

    let recipient_jid = BareJid::from_str(&recipient_jid_raw)?;

    let shared_agent = Arc::new(Mutex::new(None));

    info!("Инициализация XMPP агента...");
    match init_xmpp_agent(&bot_jid, &bot_password).await {
        Ok(agent) => {
            info!("XMPP агент успешно инициализирован");
            *shared_agent.lock().await = Some(agent);
            // Фоновая задача keepalive больше не нужна: Agent внутри tokio-xmpp
            // сам запускает необходимые задачи для поддержания соединения.
        }
        Err(e) => {
            error!("Не удалось инициализировать XMPP агент: {}", e);
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
    info!("Сервер запущен на {}", addr);
    info!("POST эндпоинт: http://{}/webhook", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
