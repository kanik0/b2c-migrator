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

// Configura il logger con fern per inviare log sia su stdout che su file
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

// Funzione asincrona che esegue la richiesta POST per una riga CSV,
// gestendo il caso in cui l'API risponda con 429 "Too Many Requests".
async fn make_async_rest_call(client: &reqwest::Client, endpoint: &str, body: RequestBody) {
    loop {
        match client.post(endpoint).json(&body).send().await {
            Ok(response) => {
                if response.status().as_u16() == 429 {
                    // Estrae l'header Retry-After e attende il tempo necessario espresso in secondi
                    if let Some(retry_after_value) = response.headers().get("Retry-After") {
                        if let Ok(retry_after_str) = retry_after_value.to_str() {
                            if let Ok(wait_secs) = retry_after_str.parse::<u64>() {
                                info!(
                                    "Ricevuto 429. Attesa di {} secondi prima di riprovare.",
                                    wait_secs
                                );
                                sleep(Duration::from_secs(wait_secs)).await;
                                continue; // Ripete il loop per ritentare la richiesta
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
    // Configura il logger
    setup_logger()?;

    // Numero massimo di richieste concorrenti (controlla la concorrenza)
    let max_concurrent_requests = 4;
    let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));

    // Endpoint REST fisso
    let endpoint = "https://lillozzo.free.beeceptor.com";
    let client = reqwest::Client::new();

    // Apri il file CSV. Assicurati che il file "data.csv" sia presente nella directory corrente.
    let file_path = "data.csv";
    let mut rdr = csv::Reader::from_path(file_path)?;

    let mut handles = vec![];

    // Itera su ciascuna riga del CSV, deserializzandola in RequestBody
    for result in rdr.deserialize() {
        let record: RequestBody = result?;
        let client = client.clone();
        let endpoint = endpoint.to_string();
        let semaphore_clone = semaphore.clone();
        // Acquisizione del permesso per rispettare il limite di concorrenza
        let permit = semaphore_clone.acquire_owned().await?;
        let handle = tokio::spawn(async move {
            info!("Inizio elaborazione della riga: {:?}", record);
            make_async_rest_call(&client, &endpoint, record).await;
            // Il permesso viene rilasciato automaticamente al termine del task (grazie a drop)
            drop(permit);
        });
        handles.push(handle);
    }

    // Attende che tutti i task completino
    for handle in handles {
        handle.await?;
    }

    info!("Tutte le operazioni del CSV sono state completate.");
    Ok(())
}
