use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use xmpp::XmppClient;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

// Структура для состояния приложения
#[derive(Clone)]
struct AppState {
    xmpp_client: Arc<Mutex<Option<XmppClient>>>,
    recipient_jid: String,
    bot_jid: String,
    bot_password: String,
}

// Структура данных, которую ожидаем от Next.js
#[derive(Debug, Deserialize)]
struct WebhookPayload {
    name: String,
    email: String,
    message: String,
}

// Ответ сервера
#[derive(Debug, serde::Serialize)]
struct WebhookResponse {
    success: bool,
    message: String,
}

// POST эндпоинт для приема сообщений
async fn webhook_handler(
    State(state): State<AppState>,
    Json(payload): Json<WebhookPayload>,
) -> impl IntoResponse {
    info!("Получено сообщение от {} ({})", payload.name, payload.email);

    // Формируем сообщение для Jabber
    let jabber_message = format!(
        "Новое сообщение с сайта:\n\n\
         Имя: {}\n\
         Email: {}\n\
         Сообщение:\n{}",
        payload.name, payload.email, payload.message
    );

    // Отправляем в Jabber
    let mut client_guard = state.xmpp_client.lock().await;

    match client_guard.as_mut() {
        Some(client) => {
            match client.send_message(&state.recipient_jid, &jabber_message).await {
                Ok(_) => {
                    info!("Сообщение успешно отправлено в Jabber");
                    (
                        StatusCode::OK,
                        Json(WebhookResponse {
                            success: true,
                            message: "Сообщение успешно отправлено".to_string(),
                        }),
                    )
                }
                Err(e) => {
                    error!("Ошибка отправки в Jabber: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(WebhookResponse {
                            success: false,
                            message: format!("Ошибка отправки: {}", e),
                        }),
                    )
                }
            }
        }
        None => {
            error!("XMPP клиент не инициализирован");
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

// Результат инициализации XMPP клиента
struct XmppInitResult {
    client: XmppClient,
    events: tokio::sync::mpsc::Receiver<xmpp::XmppEvent>,
}

// Функция инициализации XMPP клиента
async fn init_xmpp_client(bot_jid: &str, bot_password: &str) -> Result<XmppInitResult, anyhow::Error> {
    info!("Подключение к XMPP серверу как {}", bot_jid);
    let (client, events) = XmppClient::new(bot_jid, bot_password).await?;
    Ok(XmppInitResult { client, events })
}

// Фоновая задача для обработки событий XMPP и переподключения
async fn xmpp_event_loop(
    mut events: tokio::sync::mpsc::Receiver<xmpp::XmppEvent>,
    xmpp_client: Arc<Mutex<Option<XmppClient>>>,
    bot_jid: String,
    bot_password: String,
) {
    const RECONNECT_DELAY_SECS: u64 = 5;
    const MAX_RECONNECT_DELAY_SECS: u64 = 60;

    let mut current_delay = RECONNECT_DELAY_SECS;

    loop {
        // Ждём события пока канал открыт
        while let Some(_) = events.recv().await {
            // Получили событие - соединение активно
        }

        // Канал закрыт - клиент отключился, пробуем переподключиться
        warn!("XMPP соединение потеряно (канал событий закрыт)");

        loop {
            info!(
                "Попытка переподключения через {} сек...",
                current_delay
            );

            // Копируем данные для переподключения
            let retry_client = xmpp_client.clone();
            let retry_bot_jid = bot_jid.clone();
            let retry_bot_password = bot_password.clone();

            // Ждем перед переподключением
            tokio::time::sleep(tokio::time::Duration::from_secs(current_delay)).await;

            // Увеличиваем задержку для следующей попытки (экспоненциальный backoff)
            current_delay = (current_delay * 2).min(MAX_RECONNECT_DELAY_SECS);

            // Пробуем переподключиться
            match init_xmpp_client(&retry_bot_jid, &retry_bot_password).await {
                Ok(init_result) => {
                    info!("Успешно переподключен к XMPP серверу");
                    *retry_client.lock().await = Some(init_result.client);
                    current_delay = RECONNECT_DELAY_SECS; // Сбрасываем задержку

                    // Обновляем events для нового подключения
                    events = init_result.events;
                    break; // Выходим из внутреннего цикла, продолжаем ждать события
                }
                Err(e) => {
                    error!(
                        "Не удалось переподключиться (следующая попытка через {} сек): {:?}",
                        current_delay, e
                    );
                    // Продолжаем пытаться переподключиться
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Загружаем переменные окружения
    dotenvy::dotenv().ok();

    // Настраиваем логирование
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Читаем настройки из .env
    let bot_jid = std::env::var("BOT_JID")
        .expect("BOT_JID не задан в .env");
    let bot_password = std::env::var("BOT_PASSWORD")
        .expect("BOT_PASSWORD не задан в .env");
    let recipient_jid = std::env::var("RECIPIENT_JID")
        .expect("RECIPIENT_JID не задан в .env");
    let server_host = std::env::var("SERVER_HOST")
        .unwrap_or_else(|_| "127.0.0.1".to_string());
    let server_port = std::env::var("SERVER_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("SERVER_PORT должен быть числом");

    // Инициализируем XMPP клиент
    info!("Инициализация XMPP клиента...");
    let xmpp_client = match init_xmpp_client(&bot_jid, &bot_password).await {
        Ok(init_result) => {
            info!("XMPP клиент успешно инициализирован");

            // Запускаем фоновую задачу для обработки событий XMPP
            let initial_events = init_result.events;
            let first_client = init_result.client;

            // Создаем клонируемый клиент через Option (для переподключения)
            let client_for_loop = Arc::new(Mutex::new(Some(first_client)));

            tokio::spawn(xmpp_event_loop(
                initial_events,
                client_for_loop.clone(),
                bot_jid.clone(),
                bot_password.clone(),
            ));

            // Сохраняем клиент для webhook handler
            // Примечание: после переподключения клиент будет обновлен в client_for_loop
            None
        }
        Err(e) => {
            error!("Не удалось инициализировать XMPP клиент: {}", e);
            None
        }
    };

    // Создаем состояние приложения
    let state = AppState {
        xmpp_client: Arc::new(Mutex::new(xmpp_client)),
        recipient_jid,
        bot_jid,
        bot_password,
    };

    // Создаем маршруты
    let app = Router::new()
        .route("/webhook", post(webhook_handler))
        .with_state(state);

    let addr = format!("{}:{}", server_host, server_port);
    info!("Сервер запущен на {}", addr);
    info!("POST эндпоинт: http://{}/webhook", addr);

    // Запускаем сервер
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
