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
    phone_auth_method: bool,
    email_auth_method: bool,
) {
    loop {
        // Clone body to use it in eventual create_auth_method_api_call
        let original_body = body.clone();

        // Clean body from auth methods values
        body.phoneAuthMethod = None;
        body.emailAuthMethod = None;

        match client
            .post(endpoint)
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();

                if status.is_success() {
                    // Extract JSON body from response
                    let json_body: serde_json::Value = match response.json().await {
                        Ok(v) => v,
                        Err(e) => {
                            error!(
                                "[{:?}] Error parsing JSON response: {e:?}",
                                body.identities[0].issuerAssignedId
                            );
                            break;
                        }
                    };

                    info!(
                        "[{:?}] User created successfully with status: {status}.",
                        body.identities[0].issuerAssignedId
                    );

                    // Extract objectId from json body
                    let user_id = json_body
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(str::to_owned);

                    if user_id.is_none() && (phone_auth_method || email_auth_method) {
                        error!(
                            "[{:?}] The 'id' field was not found in the response.",
                            body.identities[0].issuerAssignedId
                        );
                    }

                    if let Some(id) = user_id {
                        if phone_auth_method {
                            let auth_endpoint =
                                format!("{endpoint}/{id}/authentication/phoneMethods");
                            create_phone_auth_method_api_call(
                                client,
                                &auth_endpoint,
                                original_body.clone(),
                                token,
                            )
                            .await;
                        }
                        if email_auth_method {
                            let auth_endpoint =
                                format!("{endpoint}/{id}/authentication/emailMethods");
                            create_email_auth_method_api_call(
                                client,
                                &auth_endpoint,
                                original_body,
                                token,
                            )
                            .await;
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

// Asynchronous function that creates the phone authentication method for a user
pub async fn create_phone_auth_method_api_call(
    client: &reqwest::Client,
    endpoint: &str,
    body: RequestBody,
    token: &str,
) {
    loop {
        // Create request body from original body
        let phone_auth_method = body.clone().phoneAuthMethod.unwrap();
        let auth_body = PhoneAuthMethodRequestBody {
            phoneNumber: phone_auth_method,
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
                        "[{:?}] Phone authentication method created successfully with status: {}.",
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

// Asynchronous function that creates the email authentication method for a user
pub async fn create_email_auth_method_api_call(
    client: &reqwest::Client,
    endpoint: &str,
    body: RequestBody,
    token: &str,
) {
    loop {
        // Create request body from original body
        let email_auth_method = body.clone().emailAuthMethod.unwrap();
        let auth_body = EmailAuthMethodRequestBody {
            emailAddress: email_auth_method,
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
                        "[{:?}] Email authentication method created successfully with status: {}.",
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
