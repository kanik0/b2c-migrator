#![allow(clippy::io_other_error)]
use fern::colors::{Color, ColoredLevelConfig};
use graph::*;
use log::{error, info, warn};
use rusqlite::{params, Connection};
use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use tokio::sync::Semaphore;
use tokio::time::{sleep, Duration};

mod graph;

// Configure the logger that writes to SQLite
struct DBLogger {
    conn: Arc<Mutex<Connection>>,
    table: String,
    buffer: String,
}
unsafe impl Send for DBLogger {}
unsafe impl Sync for DBLogger {}

// Implement the Write trait for DBLogger
impl DBLogger {
    /// Inserts a complete line into the database
    fn insert_line(&self, line: &str) -> io::Result<()> {
        let conn_lock = self.conn.lock().unwrap();
        let sql = format!(
            "INSERT INTO '{}' (timestamp, level, username, message) VALUES (?, ?, ?, ?)",
            self.table
        );
        // Expecting the format: "YYYY-MM-DD HH:MM:SS [LEVEL] [USERNAME] actual message..."
        if line.len() >= 30 {
            let timestamp = &line[0..19];
            // Extract level
            let level_start = line.find('[').unwrap_or(0);
            let level_end = line.find(']').unwrap_or(0);
            let level = if level_end > level_start {
                line[(level_start + 1)..level_end].trim()
            } else {
                ""
            };
            // The rest of the message (starting after level)
            let full_message = if level_end + 2 <= line.len() {
                line[level_end + 2..].trim()
            } else {
                ""
            };
            // Now, if full_message starts with '[', extract the username (without quotes) between brackets.
            let (raw_username, message) = if full_message.starts_with('[') {
                if let Some(user_end) = full_message.find(']') {
                    let user = full_message[1..user_end].trim();
                    let msg = full_message[(user_end + 1)..].trim();
                    (user, msg)
                } else {
                    ("", full_message)
                }
            } else {
                ("", full_message)
            };
            let username = raw_username.replace("\"", "");
            conn_lock
                .execute(&sql, params![timestamp, level, username, message])
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        } else {
            // fallback: insert the entire line as the message without username and level.
            conn_lock
                .execute(
                    &sql,
                    params![chrono::Local::now().to_string(), "", "", line.trim()],
                )
                .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        }
        Ok(())
    }
}

impl Write for DBLogger {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = String::from_utf8_lossy(buf);
        self.buffer.push_str(&s);
        // If there is at least one newline in the buffer, extract all complete lines
        while let Some(newline_pos) = self.buffer.find('\n') {
            let line = self.buffer[..newline_pos].to_string();
            // Remove the processed line from the buffer (including the newline)
            self.buffer.drain(..=newline_pos);
            self.insert_line(&line)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.trim().is_empty() {
            self.insert_line(&self.buffer)?;
            self.buffer.clear();
        }
        Ok(())
    }
}

// Function to configure the logger to write to stdout, file, and SQLite
fn setup_logger() -> Result<(), Box<dyn Error>> {
    let colors_line = ColoredLevelConfig::new()
        .info(Color::Green)
        .error(Color::Red);

    // Configure the SQLite database "logs.db" (will be created if it doesn't exist)
    let db_conn = Connection::open("logs.db")?;
    // Create a table with a name based on the current timestamp (format yyyymmddhhmmss)
    let table_name = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let create_table_sql = format!(
        "CREATE TABLE IF NOT EXISTS '{}' (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT,
            level TEXT,
            username TEXT,
            message TEXT
        )",
        table_name
    );
    db_conn.execute(&create_table_sql, [])?;

    // Create our logger for SQLite with an empty buffer initially
    let db_logger = DBLogger {
        conn: Arc::new(Mutex::new(db_conn)),
        table: table_name.clone(),
        buffer: String::new(),
    };

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
        // Wrap db_logger in a Box to satisfy the 'Send' bound
        .chain(Box::new(db_logger) as Box<dyn Write + Send>)
        .apply()?;
    Ok(())
}

// Asynchronous function that executes the POST request for a CSV row,
// handling the case where the API responds with 429 "Too Many Requests".
async fn make_async_rest_call(client: &reqwest::Client, endpoint: &str, body: RequestBody) {
    loop {
        match client.post(endpoint).json(&body).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!(
                        "[{:?}] Chiamata completata con stato: {}.",
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
                                    "[{:?}] Ricevuto 429. Attesa di {} secondi prima di riprovare.",
                                    body.identities[0].issuerAssignedId, wait_secs
                                );
                                sleep(Duration::from_secs(wait_secs)).await;
                                continue; // Repeat the loop to retry the request
                            }
                        }
                    }
                    error!(
                        "[{:?}] 429 ricevuto, ma header Retry-After non valido. Interruzione del task.",
                        body.identities[0].issuerAssignedId
                    );
                    break;
                } else {
                    error!(
                        "[{:?}] Errore nella chiamata con stato: {}.",
                        body.identities[0].issuerAssignedId,
                        response.status()
                    );
                    break;
                }
            }
            Err(e) => {
                error!(
                    "[{:?}] Errore nella chiamata: {:?}.",
                    body.identities[0].issuerAssignedId, e
                );
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
    let endpoint = "https://lilonz.free.beeceptor.com";
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
            info!(
                "[{:?}] Inizio elaborazione dell'utente.",
                record.identities[0].issuerAssignedId
            );
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

    info!("[END] Tutte le operazioni del CSV sono state completate.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::io::Write;
    use std::sync::{Arc, Mutex};

    // Helper to create an in-memory DBLogger for testing
    fn setup_test_db_logger(table_name: &str) -> (DBLogger, Arc<Mutex<Connection>>) {
        let conn = Connection::open_in_memory().unwrap();
        let create_table_sql = format!(
            "CREATE TABLE IF NOT EXISTS '{}' (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT,
                level TEXT,
                username TEXT,
                message TEXT
            )",
            table_name
        );
        conn.execute(&create_table_sql, []).unwrap();
        let arc_conn = Arc::new(Mutex::new(conn));
        let db_logger = DBLogger {
            conn: Arc::clone(&arc_conn),
            table: table_name.to_string(),
            buffer: String::new(),
        };
        (db_logger, arc_conn)
    }

    #[test]
    fn test_dblogger_insert_line_full_format() {
        let table_name = "test_log_full";
        let (logger, conn_arc) = setup_test_db_logger(table_name);
        let line = "2024-01-01 10:00:00 [INFO] [\"testuser\"] This is a test message.";
        logger.insert_line(line).unwrap();

        let conn = conn_arc.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT timestamp, level, username, message FROM '{}'",
                table_name
            ))
            .unwrap();
        let row: (String, String, String, String) = stmt
            .query_row([], |r| {
                Ok((
                    r.get(0).unwrap(),
                    r.get(1).unwrap(),
                    r.get(2).unwrap(),
                    r.get(3).unwrap(),
                ))
            })
            .unwrap();

        assert_eq!(row.0, "2024-01-01 10:00:00");
        assert_eq!(row.1, "INFO");
        assert_eq!(row.2, "testuser");
        assert_eq!(row.3, "This is a test message.");
    }

    #[test]
    fn test_dblogger_insert_line_no_username() {
        let table_name = "test_log_no_user";
        let (logger, conn_arc) = setup_test_db_logger(table_name);
        let line = "2024-01-01 10:00:00 [ERROR] This is an error message without username.";
        logger.insert_line(line).unwrap();

        let conn = conn_arc.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT timestamp, level, username, message FROM '{}'",
                table_name
            ))
            .unwrap();
        let row: (String, String, String, String) = stmt
            .query_row([], |r| {
                Ok((
                    r.get(0).unwrap(),
                    r.get(1).unwrap(),
                    r.get(2).unwrap(), // Username should be empty
                    r.get(3).unwrap(),
                ))
            })
            .unwrap();

        assert_eq!(row.0, "2024-01-01 10:00:00");
        assert_eq!(row.1, "ERROR");
        assert_eq!(row.2, "");
        assert_eq!(row.3, "This is an error message without username.");
    }

    #[test]
    fn test_dblogger_insert_line_short_fallback() {
        let table_name = "test_log_short";
        let (logger, conn_arc) = setup_test_db_logger(table_name);
        let line = "Short message"; // Less than 30 chars
        logger.insert_line(line).unwrap();

        let conn = conn_arc.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!(
                "SELECT level, username, message FROM '{}'", // Not checking timestamp as it's Local::now()
                table_name
            ))
            .unwrap();
        // We don't check timestamp here because it's generated by chrono::Local::now()
        let row: (String, String, String) = stmt
            .query_row([], |r| {
                Ok((r.get(0).unwrap(), r.get(1).unwrap(), r.get(2).unwrap()))
            })
            .unwrap();

        assert_eq!(row.0, ""); // level
        assert_eq!(row.1, ""); // username
        assert_eq!(row.2, "Short message"); // message
    }

    #[test]
    fn test_dblogger_insert_line_username_without_quotes() {
        let table_name = "test_log_user_no_quotes";
        let (logger, conn_arc) = setup_test_db_logger(table_name);
        let line = "2024-01-01 10:00:00 [DEBUG] [anotheruser] Debug message.";
        logger.insert_line(line).unwrap();

        let conn = conn_arc.lock().unwrap();
        let mut stmt = conn
            .prepare(&format!("SELECT username FROM '{}'", table_name))
            .unwrap();
        let username: String = stmt.query_row([], |r| r.get(0)).unwrap();
        assert_eq!(username, "anotheruser");
    }

    #[test]
    fn test_dblogger_write_and_flush() {
        let table_name = "test_log_write_flush";
        let (mut logger, conn_arc) = setup_test_db_logger(table_name);

        // Write part of a line, then the rest, then another full line
        logger
            .write_all(b"2024-01-02 11:00:00 [INFO] [user1] First part.")
            .unwrap();
        logger.write_all(b" Still user1.\n").unwrap();
        logger
            .write_all(b"2024-01-02 11:01:00 [WARN] [user2] Second line fully.\n")
            .unwrap();

        // At this point, two lines should be in the DB
        let conn_check1 = conn_arc.lock().unwrap();
        let count1: i64 = conn_check1
            .query_row(&format!("SELECT COUNT(*) FROM '{}'", table_name), [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            count1, 2,
            "Should have 2 rows after two full lines with newlines"
        );
        drop(conn_check1);

        // Write a partial line, then flush
        logger
            .write_all(b"2024-01-02 11:02:00 [ERROR] [user3] Partial flush")
            .unwrap();
        logger.flush().unwrap();

        let conn_check2 = conn_arc.lock().unwrap();
        let count2: i64 = conn_check2
            .query_row(&format!("SELECT COUNT(*) FROM '{}'", table_name), [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count2, 3, "Should have 3 rows after flush");

        let last_message: String = {
            let mut stmt = conn_check2
                .prepare(&format!(
                    "SELECT message FROM '{}' ORDER BY id DESC LIMIT 1",
                    table_name
                ))
                .unwrap();
            stmt.query_row([], |r| r.get(0)).unwrap()
        };
        assert_eq!(last_message, "Partial flush");
        drop(conn_check2);

        // Test flushing an empty buffer (should do nothing)
        logger.flush().unwrap();
        let conn_check3 = conn_arc.lock().unwrap();
        let count3: i64 = conn_check3
            .query_row(&format!("SELECT COUNT(*) FROM '{}'", table_name), [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            count3, 3,
            "Count should remain 3 after flushing empty buffer"
        );
    }

    #[test]
    fn test_dblogger_write_multiple_lines_in_one_buffer() {
        let table_name = "test_log_multi_in_buf";
        let (mut logger, conn_arc) = setup_test_db_logger(table_name);

        let log_data = "2024-01-03 12:00:00 [INFO] [userA] Line A.\n2024-01-03 12:01:00 [INFO] [userB] Line B.\n";
        logger.write_all(log_data.as_bytes()).unwrap();

        let conn = conn_arc.lock().unwrap();
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM '{}'", table_name), [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);

        let mut stmt = conn
            .prepare(&format!("SELECT message FROM '{}' ORDER BY id", table_name))
            .unwrap();
        let messages: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|res| res.unwrap())
            .collect();
        assert_eq!(messages, vec!["Line A.", "Line B."]);
    }

    #[test]
    fn test_dblogger_write_empty_string() {
        let table_name = "test_log_empty_write";
        let (mut logger, conn_arc) = setup_test_db_logger(table_name);

        logger.write_all(b"").unwrap(); // Write empty bytes
        logger.flush().unwrap(); // Flush

        let conn = conn_arc.lock().unwrap();
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM '{}'", table_name), [], |r| {
                r.get(0)
            })
            .unwrap();
        // Empty write + flush on empty buffer should not insert anything
        assert_eq!(
            count, 0,
            "No rows should be inserted for empty write and flush"
        );
    }

    // --- Tests for make_async_rest_call ---
    // We need to bring in RequestBody, Identity for these tests.
    // Since they are in graph::mod, and graph is a sibling module, we use crate::graph::*
    use crate::graph::{Identity, PasswordProfile, RequestBody};
    use std::collections::HashMap;
    use tokio::time::Duration as TokioDuration; // Removed pause, advance

    fn create_dummy_request_body(issuer_assigned_id: &str) -> RequestBody {
        RequestBody {
            displayName: "Test User".to_string(),
            passwordProfile: PasswordProfile {
                forceChangePasswordNextSignIn: false,
                password: "password".to_string(),
            },
            identities: vec![Identity {
                signInType: "emailAddress".to_string(),
                issuer: "test.com".to_string(),
                issuerAssignedId: issuer_assigned_id.to_string(),
            }],
            custom_fields: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_make_async_rest_call_success() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_success");

        let mock = server
            .mock("POST", "/")
            .with_status(200)
            .with_body(r#"{"status": "ok"}"#)
            .create_async()
            .await;

        make_async_rest_call(&client, &endpoint, body).await;
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_make_async_rest_call_429_with_retry_after_and_success() {
        // Pause time to control sleep
        tokio::time::pause(); // Corrected path

        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new(); // Keep client for reuse
        let body = create_dummy_request_body("user_429_retry");

        // First call: 429 with Retry-After
        let mock429 = server
            .mock("POST", "/")
            .with_status(429)
            .with_header("Retry-After", "1") // 1 second
            .with_body(r#"{"error": "Too Many Requests"}"#)
            .create_async()
            .await;

        // Second call: 200 OK
        let mock200 = server
            .mock("POST", "/")
            .with_status(200)
            .with_body(r#"{"status": "ok"}"#)
            .create_async()
            .await;

        // Clone client and endpoint for the spawned task to own them
        let client_clone = client.clone();
        let endpoint_clone = endpoint.to_string(); // server.url() returns String, so cloning is fine.
        let task = tokio::spawn(async move {
            make_async_rest_call(&client_clone, &endpoint_clone, body).await
        });

        // Allow the first call to happen
        // We need to advance time enough for the first attempt to be made, but not for sleep yet.
        // This is a bit tricky as the internal HTTP call itself takes some time.
        // Let's yield to allow the task to run.
        tokio::task::yield_now().await; // Yield to let the spawned task run the first attempt

        // Advance time by the Retry-After duration to trigger the sleep completion
        tokio::time::advance(TokioDuration::from_secs(2)).await; // Corrected path & Advance slightly more

        // Wait for the task to complete
        task.await.unwrap();

        mock429.assert_async().await;
        mock200.assert_async().await; // Should be called after retry
    }

    #[tokio::test]
    async fn test_make_async_rest_call_429_invalid_retry_after() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_429_invalid_retry");

        let mock = server
            .mock("POST", "/")
            .with_status(429)
            .with_header("Retry-After", "invalid_value") // Invalid header
            .with_body(r#"{"error": "Too Many Requests"}"#)
            .create_async()
            .await;

        // No need to pause/advance time here as it should not sleep with invalid header

        make_async_rest_call(&client, &endpoint, body).await;
        mock.assert_async().await; // Should only be called once
    }

    #[tokio::test]
    async fn test_make_async_rest_call_429_no_retry_after() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_429_no_retry_header");

        let mock = server
            .mock("POST", "/")
            .with_status(429)
            // No Retry-After header
            .with_body(r#"{"error": "Too Many Requests"}"#)
            .create_async()
            .await;

        make_async_rest_call(&client, &endpoint, body).await;
        mock.assert_async().await; // Should only be called once
    }

    #[tokio::test]
    async fn test_make_async_rest_call_other_error_400() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_400_error");

        let mock = server
            .mock("POST", "/")
            .with_status(400)
            .with_body(r#"{"error": "Bad Request"}"#)
            .create_async()
            .await;

        make_async_rest_call(&client, &endpoint, body).await;
        mock.assert_async().await; // Should be called once, no retry
    }

    #[tokio::test]
    async fn test_make_async_rest_call_server_error_500() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_500_error");

        let mock = server
            .mock("POST", "/")
            .with_status(500)
            .with_body(r#"{"error": "Internal Server Error"}"#)
            .create_async()
            .await;

        make_async_rest_call(&client, &endpoint, body).await;
        mock.assert_async().await; // Should be called once, no retry
    }

    #[tokio::test]
    async fn test_make_async_rest_call_network_error() {
        // For a network error, we use a non-existent server address.
        // Mockito isn't strictly needed here, but we need a client and body.
        let endpoint = "http://localhost:12345"; // Assuming this port is not in use
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_network_error");

        // We can't easily assert logs here without a more complex setup,
        // but the main thing is that the function should complete and not panic.
        // The error will be logged by the function itself.
        make_async_rest_call(&client, endpoint, body).await;
        // No mockito assertion here as we are not using a mockito server for this specific test.
        // We rely on the function's own error logging and graceful exit from the loop.
    }
}
