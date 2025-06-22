# b2c-migrator

A command-line tool written in Rust to migrate user data from a CSV file to a target API endpoint. It's designed to be robust, handling API rate limits and providing detailed logging.

## Core Features

*   **CSV Input:** Reads user data from a CSV file (default: `data.csv`).
*   **Asynchronous API Calls:** Makes asynchronous HTTP POST requests to a configurable API endpoint.
*   **Rate Limit Handling:** Intelligently handles HTTP 429 "Too Many Requests" responses by respecting the `Retry-After` header.
*   **Concurrency Management:** Uses a semaphore to control the maximum number of concurrent requests, preventing server overload.
*   **Comprehensive Logging:** Provides structured logging to:
    *   `stdout` (console) with colored severity levels.
    *   A local file (`output.log`).
    *   An SQLite database (`logs.db`) with timestamped tables for each run.
*   **Flexible Data Mapping:** Maps CSV data to JSON request bodies. Explicit fields like `displayName`, `passwordProfile`, and `identities` are handled directly, while any other CSV columns are collected as custom fields in the JSON payload.

## Prerequisites

*   **Rust:** Ensure you have a recent version of Rust installed (compiles with Rust 2021 edition). You can find installation instructions at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).
*   **Input CSV File:** A CSV file containing the user data. By default, the application looks for `data.csv` in its root directory.

## Configuration

Currently, primary configuration parameters are hardcoded in `src/main.rs`:

*   `max_concurrent_requests`: (Default: `4`) Maximum concurrent HTTP requests.
*   `endpoint`: (Default: `"https://rullo.free.beeceptor.com"`) Target API endpoint URL.
*   `file_path`: (Default: `"data.csv"`) Path to the input CSV file.
*   Log files: `output.log` (text log) and `logs.db` (SQLite log) are generated in the root directory.

Modifying these requires editing `src/main.rs` and recompiling.

## Input CSV Format (`data.csv`)

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
2.  **Prepare your input file:** Ensure your `data.csv` (or the CSV file you configure in `src/main.rs`) is in the project's root directory.
3.  **Build the project:**
    ```bash
    cargo build
    ```
    For an optimized release build:
    ```bash
    cargo build --release
    ```
4.  **Run the application:**
    From the project root:
    ```bash
    cargo run
    ```
    If you built a release version, the executable is in `target/release/`:
    ```bash
    ./target/release/b2c-migrator
    ```

## Logging & Error Handling

The application provides detailed logging:
*   **Console (stdout):** Real-time logs with color-coded severity.
*   **File (`output.log`):** All log messages are saved for review.
*   **SQLite (`logs.db`):** Structured logs are stored in a new table (named with current timestamp) for each run, allowing for easier querying and analysis. User-specific log messages include the `issuerAssignedId` for traceability.

**Error Handling:**
*   Critical errors during setup (e.g., cannot open `data.csv`, database issues) will cause the program to terminate and print an error message to `stderr`.
*   Errors related to processing individual user records (e.g., API call failures for a specific user, invalid data for a user) are logged with `ERROR` severity, but the application will continue processing other records.
*   HTTP 429 "Too Many Requests" errors are handled by pausing and retrying according to the `Retry-After` header. If the header is missing or invalid for a 429 response, the task for that user will be aborted after logging an error.

## Dependencies

This project relies on several key Rust crates:

*   **`tokio`**: Asynchronous runtime for concurrent operations.
*   **`reqwest`**: HTTP client for making API requests.
*   **`serde` (with `serde_json`):** For data serialization (Rust structs to JSON) and deserialization (CSV to Rust structs, JSON strings in CSV to Rust structs).
*   **`csv`**: For reading and parsing the input CSV file.
*   **`log` & `fern`**: For flexible and structured logging.
*   **`chrono`**: For timestamping log entries.
*   **`rusqlite`**: For SQLite database interaction (logging).

## Future Enhancements

Potential areas for future development include:

*   **Dynamic Configuration:** Allow `endpoint`, `file_path`, `max_concurrent_requests`, and log settings to be configured via command-line arguments or environment variables instead of hardcoding.
*   **Enhanced Error Reporting:** Provide summaries of successful/failed migrations at the end of the run.
*   **Test Suite:** Develop unit and integration tests to ensure reliability.
*   **Input Validation:** More granular validation of CSV data before attempting API calls.

---

This README provides a comprehensive guide to understanding, running, and potentially extending the `b2c-migrator` tool.
