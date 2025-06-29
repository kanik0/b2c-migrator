use fern::colors::{Color, ColoredLevelConfig};
use rusqlite::{params, Connection};
use std::error::Error;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};

// Configure the logger that writes to SQLite
pub struct DBLogger {
    pub conn: Arc<Mutex<Connection>>,
    pub table: String,
    pub buffer: String,
}
unsafe impl Send for DBLogger {}
unsafe impl Sync for DBLogger {}

// Implement the Write trait for DBLogger
impl DBLogger {
    /// Inserts a complete line into the database
    pub fn insert_line(&self, line: &str) -> io::Result<()> {
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
pub fn setup_logger(logfile: String, dbfile: String) -> Result<(), Box<dyn Error>> {
    let colors_line = ColoredLevelConfig::new()
        .info(Color::Green)
        .error(Color::Red);

    // Configure the SQLite database (will be created if it doesn't exist)
    let db_conn = Connection::open(dbfile)?;
    // Create a table with a name based on the current timestamp (format yyyymmddhhmmss)
    let table_name = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
    let create_table_sql = format!(
        "CREATE TABLE IF NOT EXISTS '{table_name}' (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT,
            level TEXT,
            username TEXT,
            message TEXT
        )",
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
        .chain(fern::log_file(logfile)?)
        // Wrap db_logger in a Box to satisfy the 'Send' bound
        .chain(Box::new(db_logger) as Box<dyn Write + Send>)
        .apply()?;
    Ok(())
}
