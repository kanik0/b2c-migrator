#![allow(clippy::io_other_error)]
use clap::{Arg, Command};
use db::*;
use graph::*;
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Semaphore;

mod db;
mod graph;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Manage args
    let matches = Command::new("B2C Migrator")
        .version(env!("CARGO_PKG_VERSION"))
        .author("kanik0")
        .about("Migrate your users to Azure AD B2C using Microsoft Graph API")
        .arg(
            Arg::new("token")
                .short('t')
                .long("token")
                .help("Sets the bearer token used for authentication")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .help("Sets the path to the CSV data file")
                .required(true)
                .num_args(1),
        )
        .arg(
            Arg::new("nreqs")
                .short('n')
                .long("nreqs")
                .help("Sets the number of concurrent requests to use")
                .required(false)
                .default_value("4")
                .num_args(1),
        )
        .arg(
            Arg::new("logfile")
                .short('l')
                .long("logfile")
                .help("Sets the path to the log file")
                .required(false)
                .default_value("output.log")
                .num_args(1),
        )
        .arg(
            Arg::new("dbfile")
                .short('d')
                .long("dbfile")
                .help("Sets the path to the sqlite database file")
                .required(false)
                .default_value("output.db")
                .num_args(1),
        )
        .arg(
            Arg::new("url")
                .short('u')
                .long("url")
                .help("Sets the URL for the REST endpoint")
                .required(false)
                .default_value("https://graph.microsoft.com")
                .num_args(1),
        )
        .get_matches();

    // Bearer token for authentication
    let bearer_token = matches
        .get_one::<String>("token")
        .expect("Bearer token is required")
        .clone();

    // File path to the CSV data file
    let file_path = matches
        .get_one::<String>("file")
        .expect("CSV data file path is required")
        .clone();

    // Maximum number of concurrent requests (controls concurrency)
    let max_concurrent_requests_string = matches
        .get_one::<String>("nreqs")
        .expect("Number of concurrent requests is required")
        .clone();
    let max_concurrent_requests: usize = max_concurrent_requests_string.parse::<usize>().unwrap();
    let semaphore = Arc::new(Semaphore::new(max_concurrent_requests));

    // File path for the log file
    let log_file = matches
        .get_one::<String>("logfile")
        .expect("Log file path is required")
        .clone();

    // File path for the db file
    let db_file = matches
        .get_one::<String>("dbfile")
        .expect("DB file path is required")
        .clone();

    // REST endpoint
    let endpoint = matches
        .get_one::<String>("url")
        .expect("REST endpoint is required")
        .clone();
    let client = reqwest::Client::new();

    // Configure the logger
    setup_logger(log_file, db_file)?;

    // Open the CSV file.
    let mut rdr = csv::Reader::from_path(file_path.clone())?;

    // Check for authentication methods in the CSV columns
    let headers = rdr.headers()?;
    let has_phone_auth_method = headers.iter().any(|h| h == "phoneAuthMethod");
    let has_email_auth_method = headers.iter().any(|h| h == "emailAuthMethod");

    // Determine the number of records in the CSV file.
    let records: Vec<_> = csv::Reader::from_path(file_path.clone())?
        .records()
        .filter_map(Result::ok)
        .collect();
    let total_rows = records.len() as u64;

    // Create the progress bar with the total number of rows.
    let pb = Arc::new(ProgressBar::new(total_rows));
    let style = ProgressStyle::default_bar()
        .template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%)",
        )
        .unwrap()
        .progress_chars("#>-");
    pb.set_style(style);

    let mut handles = vec![];

    info!("Starting migration process. Using file {file_path} with {max_concurrent_requests} threads.");
    // Iterate over each row of the CSV, deserializing it into RequestBody
    for result in rdr.deserialize() {
        let record: RequestBody = result?;
        let client = client.clone();
        let endpoint = format!("{endpoint}/v1.0/users");
        let bearer_token = bearer_token.to_string();
        let semaphore_clone = semaphore.clone();
        // Acquire permission to respect the concurrency limit
        let permit = semaphore_clone.acquire_owned().await?;
        let pb = pb.clone();
        let handle = tokio::spawn(async move {
            info!(
                "[{:?}] Starting migration process for user.",
                record.identities[0].issuerAssignedId
            );
            create_user_api_call(
                &client,
                &endpoint,
                record,
                &bearer_token,
                has_phone_auth_method,
                has_email_auth_method,
            )
            .await;
            pb.inc(1);
            // The permit is automatically released at the end of the task (thanks to drop)
            drop(permit);
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        handle.await?;
    }

    pb.finish_with_message("CSV processing complete");
    info!("[END] All operations for the CSV have been completed.");
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
            "CREATE TABLE IF NOT EXISTS '{table_name}' (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT,
                level TEXT,
                username TEXT,
                message TEXT
            )",
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
                "SELECT timestamp, level, username, message FROM '{table_name}'",
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
                "SELECT timestamp, level, username, message FROM '{table_name}'",
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
                "SELECT level, username, message FROM '{table_name}'", // Not checking timestamp as it's Local::now()
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
            .prepare(&format!("SELECT username FROM '{table_name}'"))
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
            .query_row(&format!("SELECT COUNT(*) FROM '{table_name}'"), [], |r| {
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
            .query_row(&format!("SELECT COUNT(*) FROM '{table_name}'"), [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count2, 3, "Should have 3 rows after flush");

        let last_message: String = {
            let mut stmt = conn_check2
                .prepare(&format!(
                    "SELECT message FROM '{table_name}' ORDER BY id DESC LIMIT 1"
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
            .query_row(&format!("SELECT COUNT(*) FROM '{table_name}'"), [], |r| {
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
            .query_row(&format!("SELECT COUNT(*) FROM '{table_name}'"), [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 2);

        let mut stmt = conn
            .prepare(&format!("SELECT message FROM '{table_name}' ORDER BY id"))
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
            .query_row(&format!("SELECT COUNT(*) FROM '{table_name}'"), [], |r| {
                r.get(0)
            })
            .unwrap();
        // Empty write + flush on empty buffer should not insert anything
        assert_eq!(
            count, 0,
            "No rows should be inserted for empty write and flush"
        );
    }

    // --- Tests for create_user_api_call ---
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
            phoneAuthMethod: None,
            emailAuthMethod: None,
            custom_fields: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_make_async_rest_call_success() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_success");
        let bearer_token = "Bearer token";

        let mock = server
            .mock("POST", "/")
            .with_status(200)
            .with_body(r#"{"status": "ok"}"#)
            .create_async()
            .await;

        create_user_api_call(&client, &endpoint, body, bearer_token, false, false).await;
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
        let bearer_token = "Bearer token";

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
            create_user_api_call(
                &client_clone,
                &endpoint_clone,
                body, bearer_token,
                false,
                false
            )
            .await
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
        let bearer_token = "Bearer token";

        let mock = server
            .mock("POST", "/")
            .with_status(429)
            .with_header("Retry-After", "invalid_value") // Invalid header
            .with_body(r#"{"error": "Too Many Requests"}"#)
            .create_async()
            .await;

        // No need to pause/advance time here as it should not sleep with invalid header

        create_user_api_call(&client, &endpoint, body, bearer_token, false, false).await;
        mock.assert_async().await; // Should only be called once
    }

    #[tokio::test]
    async fn test_make_async_rest_call_429_no_retry_after() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_429_no_retry_header");
        let bearer_token = "Bearer token";

        let mock = server
            .mock("POST", "/")
            .with_status(429)
            // No Retry-After header
            .with_body(r#"{"error": "Too Many Requests"}"#)
            .create_async()
            .await;

        create_user_api_call(&client, &endpoint, body, bearer_token, false, false).await;
        mock.assert_async().await; // Should only be called once
    }

    #[tokio::test]
    async fn test_make_async_rest_call_other_error_400() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_400_error");
        let bearer_token = "Bearer token";

        let mock = server
            .mock("POST", "/")
            .with_status(400)
            .with_body(r#"{"error": "Bad Request"}"#)
            .create_async()
            .await;

        create_user_api_call(&client, &endpoint, body, bearer_token, false, false).await;
        mock.assert_async().await; // Should be called once, no retry
    }

    #[tokio::test]
    async fn test_make_async_rest_call_server_error_500() {
        let mut server = mockito::Server::new_async().await;
        let endpoint = server.url();
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_500_error");
        let bearer_token = "Bearer token";

        let mock = server
            .mock("POST", "/")
            .with_status(500)
            .with_body(r#"{"error": "Internal Server Error"}"#)
            .create_async()
            .await;

        create_user_api_call(&client, &endpoint, body, bearer_token, false, false).await;
        mock.assert_async().await; // Should be called once, no retry
    }

    #[tokio::test]
    async fn test_make_async_rest_call_network_error() {
        // For a network error, we use a non-existent server address.
        // Mockito isn't strictly needed here, but we need a client and body.
        let endpoint = "http://localhost:12345"; // Assuming this port is not in use
        let client = reqwest::Client::new();
        let body = create_dummy_request_body("user_network_error");
        let bearer_token = "Bearer token";

        // We can't easily assert logs here without a more complex setup,
        // but the main thing is that the function should complete and not panic.
        // The error will be logged by the function itself.
        create_user_api_call(&client, endpoint, body, bearer_token, false, false).await;
        // No mockito assertion here as we are not using a mockito server for this specific test.
        // We rely on the function's own error logging and graceful exit from the loop.
    }
}
