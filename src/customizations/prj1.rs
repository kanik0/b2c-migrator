#![allow(non_snake_case)]
use chrono::Utc;
use log::{error, info};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use uuid::Uuid;

/// Mirrors the keys that appear in the TOML configuration file.
///
/// Example file:
/// ```toml
/// url             = "https://yourendpoint.com"
/// applicationCode = "ABC"
/// name            = "Azure"
/// surname         = "B2C"
/// userId          = "abc.user"
/// actualUserId    = "abc.user"
/// X-LAGKey        = "YOUR-LAG-KEY"
/// ```
#[derive(Debug, Deserialize, Clone)]
pub struct Prj1AppConfig {
    url: String,
    applicationCode: String,
    name: String,
    surname: String,
    userId: String,
    actualUserId: String,

    #[serde(rename = "X-LAGKey")]
    x_lag_key: String,
}

// Structs REST API request
#[derive(Serialize)]
struct Prj1RequestBody {
    payload: Payload,
    headers: Headers,
}

#[derive(Serialize)]
struct Payload {
    data: Data,
}

#[derive(Serialize)]
struct Data {
    subject: String,
    body: String,
    to: Vec<String>,
}

#[derive(Serialize)]
struct Headers {
    headers: HeadersContainer,
}

#[derive(Serialize)]
struct HeadersContainer {
    #[serde(rename = "tech_header")]
    tech_header: TechHeader,

    #[serde(rename = "user_header")]
    user_header: UserHeader,
}

#[derive(Serialize)]
struct TechHeader {
    id: String,
    applicationCode: String,
    correlationId: String,
    channel: String,
    timeStamp: String,
}

#[derive(Serialize)]
struct UserHeader {
    id: String,
    archUser: ArchUser,
}

#[derive(Serialize)]
struct ArchUser {
    name: String,
    surname: String,
    userId: String,
    actualUserId: String,

    // Can be null
    #[serde(default)]
    userGroups: Vec<String>,
    #[serde(default)]
    userRoles: Vec<String>,
}

/// Load and parse a configuration file.
///
/// # Errors
/// * I/O failures while reading the file
/// * TOML-syntax or type mismatches while parsing
pub fn prj1_load_config<P: AsRef<Path>>(
    path: P,
) -> Result<Prj1AppConfig, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config = toml::from_str::<Prj1AppConfig>(&contents)?;
    Ok(config)
}

/// Build an instance of `Prj1RequestBody` using values stored in the
/// `Prj1AppConfig` plus the message-specific parameters.
///
/// `subject`, `body_text`, and `recipients` are the dynamic parts that
/// vary for each request.
fn build_request_body(
    cfg: &Prj1AppConfig,
    subject: impl Into<String>,
    body_text: impl Into<String>,
    recipient: String,
) -> Prj1RequestBody {
    Prj1RequestBody {
        payload: Payload {
            data: Data {
                subject: subject.into(),
                body: body_text.into(),
                to: vec![recipient],
            },
        },
        headers: Headers {
            headers: HeadersContainer {
                tech_header: TechHeader {
                    id: "tech_header".into(),
                    applicationCode: cfg.applicationCode.clone(),
                    correlationId: Uuid::new_v4().to_string(),
                    channel: "B2B".into(),
                    timeStamp: Utc::now().timestamp_millis().to_string(),
                },
                user_header: UserHeader {
                    id: "user_header".into(),
                    archUser: ArchUser {
                        name: cfg.name.clone(),
                        surname: cfg.surname.clone(),
                        userId: cfg.userId.clone(),
                        actualUserId: cfg.actualUserId.clone(),
                        userGroups: Vec::new(),
                        userRoles: Vec::new(),
                    },
                },
            },
        },
    }
}

/// Send `Prj1RequestBody` to the REST endpoint described in `Prj1AppConfig`.
///
/// * `client` – a `reqwest::Client`
/// * `cfg`    – the configuration loaded from the TOML file
/// * `body`   – fully-populated request payload
///
/// Returns a raw `reqwest::Response`.
pub async fn send_notification(client: &Client, cfg: &Prj1AppConfig, email: &String) {
    // Initialize request body
    let body = build_request_body(
        cfg,
        "Hello from Rust",
        "This message was generated automatically.",
        email.into(),
    );

    match client
        .post(&cfg.url)
        // mandatory headers ---------------------------------------------------
        .header(header::CONTENT_TYPE, "application/json")
        .header("X-LAGKey", &cfg.x_lag_key)
        // ---------------------------------------------------------------------
        .json(&body)
        .send()
        .await
    {
        Ok(response) => {
            info!(
                "[{:?}] Successfully sent notification email, with status: {}.",
                email,
                response.status()
            );
        }
        Err(e) => {
            error!("[{email:?}] Something went wrong when sending the email: {e:?}");
        }
    }
}
