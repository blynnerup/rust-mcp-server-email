use axum::{routing::{get,post}, Router, Json, extract::State};
use serde::Deserialize;
use lettre::{Message, SmtpTransport, Transport, message::{header, SinglePart}};
use pulldown_cmark::{Parser, Options, html};
use std::sync::Arc;
use axum::response::IntoResponse;
use axum::response::Json as AxumJson;
use serde_json::json;

// Create a MCP server that can send emails
// Needs to expose an endpoint /send_email that accepts POST requests with JSON body containing
// Recipient email, recipient name, sender email, sender name, subject, body (markdown to html) and optional attachments (array of base64 encoded files with filename and mime type).
// The attachment should be downloaded from an URL provided in the JSON body.
// The server should convert the markdown body to HTML and send the email using an SMTP server.

#[derive(Deserialize)]
struct EmailRequest {
    recipient_email: String,
    recipient_name: String,
    sender_email: String,
    sender_name: String,
    subject: String,
    body: String, // markdown
}

struct AppState {
    mailer: SmtpTransport,
}

#[tokio::main]
async fn main() {
    // let mailer = SmtpTransport::relay("smtp.example.com")
    //     .unwrap()
    //     .credentials(lettre::transport::smtp::authentication::Credentials::new(
    //         "user".into(), "password".into(),
    //     ))
    //     .build();

    // For testing, we use a local SMTP server (MailHog))
    // TODO: Diable this code in production
    let mailer = SmtpTransport::builder_dangerous("localhost")
        .port(1025)
        .build();


    let app = Router::new()
        .route("/", get(root))
        .route("/send_email", post(send_email))
        .with_state(Arc::new(AppState { mailer }));

    println!("Starting server on on http://localhost:3001");
    axum::serve(
        tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap(),
        app
    )
        .await
        .unwrap();
}

// Handler for root path
async fn root() -> impl IntoResponse {
    "Welcome to the MCP Email server! Use POST /send_email to send emails./n\
    The JSON body should contain recipient_email, recipient_name, sender_email, sender_name, subject, body (markdown), and optional attachments (array of {url, filename, mime_type})."
}

// Handler for /send_email
async fn send_email(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<EmailRequest>,
) -> impl IntoResponse {
    // Convert markdown to HTML
    let mut html_body = String::new();
    let parser = Parser::new_ext(&payload.body, Options::all());
    html::push_html(&mut html_body, parser);

    // Only the HTML body part
    let part = SinglePart::builder()
        .header(header::ContentType::TEXT_HTML)
        .body(html_body);

    // Build email
    let email = Message::builder()
        .from(format!("{} <{}>", payload.sender_name, payload.sender_email).parse().unwrap())
        .to(format!("{} <{}>", payload.recipient_name, payload.recipient_email).parse().unwrap())
        .subject(payload.subject)
        .singlepart(part)
        .unwrap();

    // Send email
    match state.mailer.send(&email) {
        Ok(_) => AxumJson(json!({ "status": "Email sent" })),
        Err(e) => AxumJson(json!({ "status": "Failed to send email", "error": e.to_string() })),
    }
}

