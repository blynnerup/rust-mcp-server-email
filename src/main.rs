use axum::{routing::{get,post}, Router, Json, extract::State};
use serde::Deserialize;
use lettre::{Message, SmtpTransport, Transport, message::{header, SinglePart, MultiPart, Attachment}};
use pulldown_cmark::{Parser, Options, html};
use std::sync::Arc;
use axum::response::IntoResponse;
use axum::response::Json as AxumJson;
use axum::response::Html;
use serde_json::json;
use reqwest;

#[derive(Deserialize)]
struct AttachmentRequest {
    url: String,
    filename: String,
    mime_type: String,
}

#[derive(Deserialize)]
struct EmailRequest {
    recipient_email: String,
    recipient_name: String,
    sender_email: String,
    sender_name: String,
    subject: String,
    body: String, // markdown
    attachments: Option<Vec<AttachmentRequest>>, // optional attachments
}

struct AppState {
    mailer: SmtpTransport,
}

#[tokio::main]
async fn main() {
    // TODO: Configure SMTP server here (once it is ready in Azure)
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
    Html(r#"
        <style>
            body { 
                font-family: fangsong, sans-serif; 
                margin: 40px;
                background: linear-gradient(135deg, #e0f7fa 0%, #b3e5fc 100%) 
            }
            h1 { color: #333; }
            p { font-size: 18px; }
            code { background-color: #f4f4f4; padding: 2px 4px; border-radius: 4px; }
        </style>
        <h1>Welcome to the MCP Email server!</h1>
        <p>Use <strong>POST /send_email</strong> to send emails.</p>
        <p>
            The JSON body should contain:<br>
            <code>recipient_email</code>, <code>recipient_name</code>, <code>sender_email</code>, <code>sender_name</code>, <code>subject</code>, <code>body</code> (markdown), and optional <code>attachments</code> (array of {url, filename, mime_type}).
        </p>
    "#)
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

    let html_part = SinglePart::builder()

        .header(header::ContentType::TEXT_HTML)
        .body(html_body);

    let mut parts: Vec<SinglePart> = vec![html_part];
    // Download and add attachments
    if let Some(attachments) = &payload.attachments {
        for att in attachments {
            match reqwest::get(&att.url).await {
                Ok(resp) => {
                    match resp.bytes().await {
                        Ok(bytes) => {
                            let content_type = att.mime_type.parse().unwrap_or_else(|_| header::ContentType::parse("application/octet-stream").unwrap());
                            let attachment = Attachment::new(att.filename.clone())
                                .body(bytes.to_vec(), content_type);
                            parts.push(attachment);
                        },
                        Err(e) => {
                            return AxumJson(json!({ "status": "Failed to download attachment", "error": e.to_string() }));
                        }
                    }
                },
                Err(e) => {
                    return AxumJson(json!({ "status": "Failed to download attachment", "error": e.to_string() }));
                }
            }
        }
    }

    // Build multipart email
    let mut parts_iter = parts.into_iter();
    let first = match parts_iter.next() {
        Some(p) => p,
        None => {
            return AxumJson(json!({ "status": "No email body or attachments provided" }));
        }
    };
    let mut multipart = MultiPart::mixed().singlepart(first);
    for part in parts_iter {
        multipart = multipart.singlepart(part);
    }

    let email = Message::builder()
        .from(format!("{} <{}>", payload.sender_name, payload.sender_email).parse().unwrap())
        .to(format!("{} <{}>", payload.recipient_name, payload.recipient_email).parse().unwrap())
        .subject(payload.subject)
        .multipart(multipart)
        .unwrap();

    // Send email
    match state.mailer.send(&email) {
        Ok(_) => AxumJson(json!({ "status": "Email sent" })),
        Err(e) => AxumJson(json!({ "status": "Failed to send email", "error": e.to_string() })),
    }
}

