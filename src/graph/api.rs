use crate::graph::user::*;
use log::{error, info, warn};
use tokio::time::{sleep, Duration};

// Asynchronous function that creates the user on Azure B2C for a CSV row,
// handling the case where the API responds with 429 "Too Many Requests".
pub async fn create_user_api_call(
    client: &reqwest::Client,
    endpoint: &str,
    mut body: RequestBody,
    token: &str,
    patch_auth_methods: bool,
) {
    loop {
        let original_body = body.clone();

        // Clean body from auth methods values
        body.authMethodType = None;
        body.authMethodValue = None;

        match client
            .post(endpoint)
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    if !patch_auth_methods {
                        info!(
                            "[{:?}] Request completed successfully with status: {}.",
                            body.identities[0].issuerAssignedId,
                            response.status()
                        );
                    } else {
                        info!(
                            "[{:?}] User created successfully with status: {}. Attempting to patch authentication methods.",
                            body.identities[0].issuerAssignedId,
                            response.status()
                        );
                        // Parse the JSON body into a serde_json::Value.
                        match response.json::<serde_json::Value>().await {
                            Ok(json_body) => {
                                if let Some(id) = json_body.get("id").and_then(|v| v.as_str()) {
                                    let auth_endpoint =
                                        format!("{endpoint}/{}/authentication/phoneMethods", id);
                                    create_auth_method_api_call(
                                        client,
                                        &*auth_endpoint,
                                        original_body,
                                        token,
                                    )
                                    .await;
                                } else {
                                    warn!(
                                        "[{:?}] The 'id' field was not found in the response.",
                                        body.identities[0].issuerAssignedId
                                    );
                                }
                            }
                            Err(e) => {
                                error!(
                                    "[{:?}] Error parsing JSON response: {:?}",
                                    body.identities[0].issuerAssignedId, e
                                );
                            }
                        }
                    }

                    break;
                } else if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
                    error!(
                        "[{:?}] Something went wrong. Received {}. Maybe token is invalid or expired? Exiting..",
                        body.identities[0].issuerAssignedId,
                        response.status()
                    );
                    std::process::exit(0);
                } else if response.status().as_u16() == 429 {
                    // Extract the Retry-After header and wait for the necessary time expressed in seconds
                    if let Some(retry_after_value) = response.headers().get("Retry-After") {
                        if let Ok(retry_after_str) = retry_after_value.to_str() {
                            if let Ok(wait_secs) = retry_after_str.parse::<u64>() {
                                warn!(
                                    "[{:?}] Received 429. Waiting for {} seconds before retrying.",
                                    body.identities[0].issuerAssignedId, wait_secs
                                );
                                sleep(Duration::from_secs(wait_secs)).await;
                                continue; // Repeat the loop to retry the request
                            }
                        }
                    }
                    error!(
                        "[{:?}] Received 429, but Retry-After header is invalid. Task interruption.",
                        body.identities[0].issuerAssignedId
                    );
                    break;
                } else {
                    error!(
                        "[{:?}] Error in request with status: {}.",
                        body.identities[0].issuerAssignedId,
                        response.status()
                    );
                    break;
                }
            }
            Err(e) => {
                error!(
                    "[{:?}] Error in request: {:?}.",
                    body.identities[0].issuerAssignedId, e
                );
                break;
            }
        }
    }
}

// Asynchronous function that creates the authentication method for a user
pub async fn create_auth_method_api_call(
    client: &reqwest::Client,
    endpoint: &str,
    body: RequestBody,
    token: &str,
) {
    loop {
        // Create request body from original body
        let _auth_method_type = body.clone().authMethodType.unwrap();
        let auth_method_value = body.clone().authMethodValue.unwrap();
        let auth_body = AuthMethodBody {
            phoneNumber: auth_method_value,
            phoneType: "mobile".to_string(),
        };

        match client
            .post(endpoint)
            .header("Authorization", format!("Bearer {token}"))
            .json(&auth_body)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        "[{:?}] Authentication method created successfully with status: {}.",
                        body.identities[0].issuerAssignedId,
                        response.status()
                    );
                    break;
                } else if response.status().as_u16() == 401 || response.status().as_u16() == 403 {
                    error!(
                        "[{:?}] Something went wrong. Received {}. Maybe token is invalid or expired? Exiting..",
                        body.identities[0].issuerAssignedId,
                        response.status()
                    );
                    std::process::exit(0);
                } else if response.status().as_u16() == 429 {
                    // Extract the Retry-After header and wait for the necessary time expressed in seconds
                    if let Some(retry_after_value) = response.headers().get("Retry-After") {
                        if let Ok(retry_after_str) = retry_after_value.to_str() {
                            if let Ok(wait_secs) = retry_after_str.parse::<u64>() {
                                warn!(
                                    "[{:?}] Received 429. Waiting for {} seconds before retrying.",
                                    body.identities[0].issuerAssignedId, wait_secs
                                );
                                sleep(Duration::from_secs(wait_secs)).await;
                                continue; // Repeat the loop to retry the request
                            }
                        }
                    }
                    error!(
                        "[{:?}] Received 429, but Retry-After header is invalid. Task interruption.",
                        body.identities[0].issuerAssignedId
                    );
                    break;
                } else {
                    error!(
                        "[{:?}] Error in request with status: {}.",
                        body.identities[0].issuerAssignedId,
                        response.status()
                    );
                    break;
                }
            }
            Err(e) => {
                error!(
                    "[{:?}] Error in request: {:?}.",
                    body.identities[0].issuerAssignedId, e
                );
                break;
            }
        }
    }
}
