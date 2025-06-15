# b2c-migrator

A command-line tool written in Rust to migrate user data from a CSV file to a target API endpoint.

## Core Features

*   Reads user data from a CSV file (default: `data.csv`).
*   Makes asynchronous HTTP POST requests to a configurable API endpoint.
*   Handles API rate limiting, specifically HTTP 429 "Too Many Requests" responses, by respecting the `Retry-After` header.
*   Manages concurrent requests using a semaphore to avoid overloading the server or hitting rate limits too quickly.
*   Provides structured logging to both `stdout` and a file (`output.log`).
*   Flexible mapping of user data from CSV to JSON request bodies via the `RequestBody` struct, which supports numerous optional fields and a mechanism for handling custom/additional fields.

## Prerequisites

*   **Rust:** Ensure you have a recent version of Rust installed. You can find installation instructions at [https://www.rust-lang.org/tools/install](https://www.rust-lang.org/tools/install).
*   **Input CSV File:** A CSV file containing the user data to be migrated. By default, the application looks for a file named `data.csv` in its root directory.

## Configuration

The primary configuration for the application can be found directly within the `src/main.rs` file:

*   `max_concurrent_requests`: (Default: `4`) Sets the maximum number of concurrent HTTP requests the application will make. Adjust this value based on the target API's rate limits and your network capacity.
*   `endpoint`: (Default: `"https://lillozzo.free.beeceptor.com"`) The URL of the API endpoint where user data will be POSTed.
*   `file_path`: (Default: `"data.csv"`) The name and path of the input CSV file.

## Input CSV Format (`data.csv`)

The application expects a CSV file (default name `data.csv`) where each row represents a user to be migrated. The column headers in the CSV should correspond to the fields in the `RequestBody` struct defined in `src/graph/user.rs`.

Key fields include:

*   `accountEnabled` (boolean: `true` or `false`)
*   `displayName` (string)
*   `mailNickname` (string)
*   `userPrincipalName` (string, typically an email address)
*   `passwordProfile` (JSON string): This field **must** contain a JSON string representing the user's password profile.
    *   Example: `"{""forceChangePasswordNextSignIn"": true, ""password"": ""P@$$wOrd123""}"` (Note the use of double quotes to escape quotes within the JSON string if your CSV encloses fields in quotes).
*   `identities` (JSON string, optional): This field, if present, **must** contain a JSON string representing a list of user identities.
    *   Example: `"[{""signInType"": ""emailAddress"", ""issuer"": ""mytenant.onmicrosoft.com"", ""issuerAssignedId"": ""user@example.com""}]"`

In general, any field name that is part of the `RequestBody` struct definition in `src/graph/user.rs` (including its optional fields) can be used as a column header in the CSV. Many other fields are supported (e.g., `givenName`, `surname`, `city`, `country`, `jobTitle`, etc.). Any columns in the CSV that do not map to a predefined field in the `RequestBody` struct will be collected in the `custom_fields` map, allowing for flexibility.

**Example CSV Snippet:**

```csv
accountEnabled,displayName,mailNickname,userPrincipalName,passwordProfile,givenName,surname
true,"John Doe","john.doe","john.doe@example.com","{""forceChangePasswordNextSignIn"": false, ""password"": ""Str0ngP@ss!""}","John","Doe"
false,"Jane Smith","jane.smith","jane.smith@example.com","{""forceChangePasswordNextSignIn"": true, ""password"": ""AnotherP@ssw0rd""}","Jane","Smith"
```

## How to Run

1.  **Clone the repository (if you haven't already):**
    ```bash
    git clone <repository-url>
    cd b2c-migrator
    ```
2.  **Prepare your input file:** Ensure your `data.csv` (or the CSV file specified in `src/main.rs`) is present in the root directory of the project.
3.  **Build the project:**
    ```bash
    cargo build
    ```
    For a release build (optimized):
    ```bash
    cargo build --release
    ```
4.  **Run the application:**
    After a successful build, you can run the application using:
    ```bash
    cargo run
    ```
    If you built a release version, the executable will be in `target/release/`:
    ```bash
    ./target/release/b2c-migrator
    ```

## Logging

The application uses the `fern` logging framework to provide detailed logs:

*   **Console Output:** Logs are printed to `stdout` with color-coded severity levels.
*   **File Output:** All log messages are also written to a file named `output.log` in the root directory of the project. This file is useful for auditing and debugging purposes after a run.
