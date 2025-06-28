# b2c-migrator

A command-line tool written in Rust to migrate user data from a CSV file to a target API endpoint. It's designed to be robust, handling API rate limits, providing detailed logging, and offering flexible configuration through command-line arguments.

## Core Features

*   **CSV Input:** Reads user data from a user-specified CSV file.
*   **Command-line Configuration:** Utilizes `clap` for easy configuration of API token, data file path, and concurrency level.
*   **Asynchronous API Calls:** Makes asynchronous HTTP POST requests to the target API endpoint.
*   **Rate Limit Handling:** Intelligently handles HTTP 429 "Too Many Requests" responses by respecting the `Retry-After` header.
*   **Concurrency Management:** Uses a semaphore to control the maximum number of concurrent requests, configurable via a command-line argument.
*   **Comprehensive Logging:** Provides structured logging to:
    *   `stdout` (console) with colored severity levels.
    *   A local file (`output.log`).
    *   An SQLite database (`logs.db`) with dynamically named tables for each run (e.g., `YYYYMMDDHHMMSS`), including parsed usernames in log entries.
*   **Flexible Data Mapping:** Maps CSV data to JSON request bodies. Explicit fields like `displayName`, `passwordProfile`, and `identities` are handled directly, while any other CSV columns are collected as custom fields in the JSON payload.

## Prerequisites

*   **Rust:** Ensure you have a recent version of Rust installed (compiles with Rust 2021 edition). You can find installation instructions at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).
*   **Input CSV File:** A CSV file containing the user data, specified via a command-line argument.

## Configuration

The application is configured via command-line arguments:

*   **API Endpoint:** The target API endpoint is currently fixed to `https://frillo.free.beeceptor.com` in `src/main.rs`.
*   **Log files:** `output.log` (text log) and `logs.db` (SQLite log) are generated in the root directory.

**Command-line Arguments:**

*   `-t, --token <TOKEN>`: **Required**. Sets the Bearer token used for authentication with the API.
*   `-d, --data <FILE_PATH>`: **Required**. Sets the path to the input CSV data file.
*   `-n, --threads <NUMBER>`: Optional. Sets the number of concurrent threads (requests) to use. Defaults to `4`.

## Input CSV Format

The application processes each row of the CSV file to construct a JSON payload for the target API. The CSV column headers become keys in the JSON object.

The `RequestBody` struct in `src/graph/user.rs` defines how data is structured. It has the following explicit top-level fields:
*   `displayName` (string): The display name for the user.
*   `passwordProfile` (JSON string): This field **must** contain a JSON string representing the user's password profile. The keys within this JSON string must match the fields of the `PasswordProfile` struct (e.g., `forceChangePasswordNextSignIn`, `password`).
    *   Example CSV value: `"{""forceChangePasswordNextSignIn"": true, ""password"": ""P@$$wOrd123""}"` (Note the use of double quotes to escape quotes within the JSON string if your CSV encloses fields in quotes).
*   `identities` (JSON string): This field **must** contain a JSON string representing a list of user identities. The keys within the objects in this JSON array must match the fields of the `Identity` struct (e.g., `signInType`, `issuer`, `issuerAssignedId`).
    *   Example CSV value: `"[{""signInType"": ""emailAddress"", ""issuer"": ""mytenant.onmicrosoft.com"", ""issuerAssignedId"": ""user@example.com""}]"`

All other columns in your CSV (e.g., `accountEnabled`, `mailNickname`, `userPrincipalName`, `givenName`, `surname`, custom extension attributes) will be collected into a `custom_fields` map. This means they will appear as top-level keys in the final JSON payload alongside `displayName`, `passwordProfile`, and `identities`.

**Example CSV Snippet:**

```csv
displayName,passwordProfile,identities,accountEnabled,mailNickname,userPrincipalName,givenName,surname
"John Doe","{""forceChangePasswordNextSignIn"":false,""password"":""Str0ngP@ss!""}","[{""signInType"":""emailAddress"",""issuer"":""mydomain.com"",""issuerAssignedId"":""john.doe@mydomain.com""}]",true,john.doe,john.doe@mydomain.com,John,Doe
"Jane Smith","{""forceChangePasswordNextSignIn"":true,""password"":""AnotherP@ssw0rd""}","[{""signInType"":""userName"",""issuer"":""mydomain.com"",""issuerAssignedId"":""janes""}]",true,janes,janes@mydomain.com,Jane,Smith
```
*(Ensure the JSON within CSV cells is correctly formatted and escaped as per CSV standards.)*

## How to Run

1.  **Clone the repository:**
    ```bash
    git clone <repository-url>
    cd b2c-migrator
    ```
2.  **Prepare your input file:** Ensure your CSV data file is accessible and you have its path.
3.  **Build the project:**
    ```bash
    cargo build
    ```
    For an optimized release build:
    ```bash
    cargo build --release
    ```
4.  **Run the application:**
    From the project root, you must provide the token and data file path. You can optionally specify the number of threads.

    Using `cargo run`:
    ```bash
    cargo run -- --token "YOUR_API_TOKEN" --data "path/to/your/data.csv"
    ```
    With a specific number of threads:
    ```bash
    cargo run -- --token "YOUR_API_TOKEN" --data "path/to/your/data.csv" --threads 8
    ```
    If you built a release version, the executable is in `target/release/`:
    ```bash
    ./target/release/b2c-migrator --token "YOUR_API_TOKEN" --data "path/to/your/data.csv"
    ```

## Logging & Error Handling

The application provides detailed logging:
*   **Console (stdout):** Real-time logs with color-coded severity.
*   **File (`output.log`):** All log messages are saved for review.
*   **SQLite (`logs.db`):** Structured logs are stored in a new table for each run, named with the current timestamp (e.g., `20231027153000`). User-specific log messages include the `issuerAssignedId` (parsed as username) for traceability.

**Error Handling:**
*   Critical errors during setup (e.g., cannot open the specified CSV data file, database issues) will cause the program to terminate and print an error message to `stderr`.
*   Errors related to processing individual user records (e.g., API call failures for a specific user, invalid data for a user) are logged with `ERROR` severity, but the application will continue processing other records.
*   HTTP 429 "Too Many Requests" errors are handled by pausing and retrying according to the `Retry-After` header. If the `Retry-After` header is missing or invalid for a 429 response, the task for that specific user will be aborted after logging an error, and the application will move on to the next user.

## Dependencies

This project relies on several key Rust crates:

*   **`tokio`**: Asynchronous runtime for concurrent operations.
*   **`reqwest`**: HTTP client for making API requests.
*   **`serde` (with `serde_json`):** For data serialization (Rust structs to JSON) and deserialization (CSV to Rust structs, JSON strings in CSV to Rust structs).
*   **`csv`**: For reading and parsing the input CSV file.
*   **`log` & `fern`**: For flexible and structured logging.
*   **`chrono`**: For timestamping log entries.
*   **`rusqlite`**: For SQLite database interaction (logging).
*   **`clap`**: For parsing command-line arguments.
*   **`indicatif`**: For displaying progress bars.

## Testing

The project includes unit and integration tests to help ensure reliability and correctness.

*   **Unit tests for data structures:** Located in `src/graph/user.rs`, these tests verify the custom deserialization logic for `passwordProfile` and `identities` fields.
*   **Unit tests for logging:** Located in `src/main.rs`, these tests verify the functionality of the `DBLogger`, ensuring log messages are correctly parsed and stored in the SQLite database.
*   **Integration tests for API calls:** Also in `src/main.rs`, these tests use `mockito` to simulate an HTTP server and verify the behavior of `make_async_rest_call`, including success cases, rate limit handling (429 errors with `Retry-After`), and other error scenarios.

To run all tests:
```bash
cargo test
```

## Future Enhancements

Potential areas for future development include:

*   **Configuration File/Environment Variables:** Allow API endpoint and other less frequently changed settings to be configured via a file or environment variables, in addition to command-line arguments.
*   **Enhanced Error Reporting:** Provide summaries of successful/failed migrations at the end of the run, possibly with counts and lists of problematic `issuerAssignedId`s.
*   **Input Validation:** More granular validation of CSV data (e.g., specific formats for certain fields) before attempting API calls.
*   **Dry Run Mode:** Implement a mode to simulate the migration without making actual API calls, useful for validating data and configurations.

---

This README provides a comprehensive guide to understanding, running, and potentially extending the `b2c-migrator` tool.
