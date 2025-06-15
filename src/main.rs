use chrono;
use csv;
use fern::colors::{Color, ColoredLevelConfig};
use graph::*;
use log::{error, info};
use reqwest;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

mod graph;

// Configure the logger with fern to send logs to both stdout and a file
fn setup_logger() -> Result<(), Box<dyn Error>> {
    let colors_line = ColoredLevelConfig::new()
        .info(Color::Green)
        .error(Color::Red);

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{} [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                colors_line.color(record.level()),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .chain(fern::log_file("output.log")?)
        .apply()?;
    Ok(())
}

// Asynchronous function that executes the POST request for a CSV row,
// handling the case where the API responds with 429 "Too Many Requests".
async fn make_async_rest_call(client: &reqwest::Client, endpoint: &str, body: RequestBody) {
    loop {
        match client.post(endpoint).json(&body).send().await {
            Ok(response) => {
                if response.status().as_u16() == 429 {
                    // Extract the Retry-After header and wait for the necessary time expressed in seconds
                    if let Some(retry_after_value) = response.headers().get("Retry-After") {
                        if let Ok(retry_after_str) = retry_after_value.to_str() {
                            if let Ok(wait_secs) = retry_after_str.parse::<u64>() {
                                info!(
                                    "Ricevuto 429. Attesa di {} secondi prima di riprovare.",
                                    wait_secs
                                );
                                sleep(Duration::from_secs(wait_secs)).await;
                                continue; // Repeat the loop to retry the request
                            }
                        }
                    }
                    error!(
                        "429 ricevuto, ma header Retry-After non valido. Interruzione del task."
                    );
                    break;
                } else {
                    info!(
                        "Chiamata a {} completata con stato: {}",
                        endpoint,
                        response.status()
                    );
                    break;
                }
            }
            Err(e) => {
                error!("Errore nella chiamata a {}: {:?}", endpoint, e);
                break;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Configure the logger
    setup_logger()?;

    // Maximum number of concurrent requests (controls concurrency)
    let max_concurrent_requests = 4;
    let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));

    // Fixed REST endpoint
    let endpoint = "https://lillozzo.free.beeceptor.com";
    let client = reqwest::Client::new();

    // Open the CSV file. Ensure that the "data.csv" file is present in the current directory.
    let file_path = "data.csv";
    let mut rdr = csv::Reader::from_path(file_path)?;

    let mut handles = vec![];

    // Iterate over each row of the CSV, deserializing it into RequestBody
    for result in rdr.deserialize() {
        let record: RequestBody = result?;
        let client = client.clone();
        let endpoint = endpoint.to_string();
        let semaphore_clone = semaphore.clone();
        // Acquire permission to respect the concurrency limit
        let permit = semaphore_clone.acquire_owned().await?;
        let handle = tokio::spawn(async move {
            info!("Inizio elaborazione della riga: {:?}", record);
            make_async_rest_call(&client, &endpoint, record).await;
            // The permit is automatically released at the end of the task (thanks to drop)
            drop(permit);
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await?;
    }

    info!("Tutte le operazioni del CSV sono state completate.");
    Ok(())
}
