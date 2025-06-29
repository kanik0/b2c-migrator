use crate::graph::user::*;
use log::{error, info, warn};
use tokio::time::{sleep, Duration};

// Asynchronous function that executes the POST request for a CSV row,
// handling the case where the API responds with 429 "Too Many Requests".
pub async fn make_async_rest_call(
    client: &reqwest::Client,
    endpoint: &str,
    body: RequestBody,
    token: &str,
) {
    loop {
        match client
            .post(endpoint)
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        "[{:?}] Request completed successfully with status: {}.",
                        body.identities[0].issuerAssignedId,
                        response.status()
                    );
                    break;
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
