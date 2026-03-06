use serde_json::{json, Map, Value};
use sqlx::sqlite::{SqlitePool, SqlitePoolOptions, SqliteRow};
use sqlx::{Column, Row};
use std::collections::HashMap;
use std::path::PathBuf;

/// Async SQLite database matching the Python `sqlite_helper.py` interface.
#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

/// MongoDB→SQLite field mapping (service layer key → DB column name)
const FIELD_MAPPING: &[(&str, &str)] = &[
    ("udid", "udid"),
    ("serial", "serial"),
    ("ip", "ip"),
    ("port", "port"),
    ("present", "present"),
    ("ready", "ready"),
    ("using", "using_device"),
    ("is_server", "is_server"),
    ("is_mock", "is_mock"),
    ("update_time", "update_time"),
    ("model", "model"),
    ("brand", "brand"),
    ("version", "version"),
    ("sdk", "sdk"),
    ("owner", "owner"),
    ("provider", "provider"),
    ("agentVersion", "agent_version"),
    ("hwaddr", "hwaddr"),
    ("createdAt", "created_at"),
    ("updatedAt", "updated_at"),
];

/// JSON fields stored as TEXT in SQLite
const JSON_FIELDS: &[&str] = &["memory", "cpu", "battery", "display", "tags"];

/// Boolean fields stored as INTEGER (0/1)
const BOOL_FIELDS: &[&str] = &["present", "ready", "using_device", "is_server", "is_mock"];

/// Reverse mapping: DB column → JSON output key
fn column_to_json_key(col: &str) -> &str {
    match col {
        "using_device" => "using",
        "agent_version" => "agentVersion",
        "created_at" => "createdAt",
        "updated_at" => "updatedAt",
        "group_name" => "group",
        _ => col,
    }
}

#[allow(dead_code)]
impl Database {
    /// Open (or create) the SQLite database and ensure tables exist.
    /// If the database is corrupted, creates a backup and initializes a fresh one.
    pub async fn new(db_dir: &str, db_name: &str) -> Result<Self, sqlx::Error> {
        let db_path = PathBuf::from(db_dir).join(db_name);
        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

        // Try to connect to existing database
        match SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&db_url)
            .await
        {
            Ok(pool) => {
                let db = Database { pool };
                // Verify database integrity by running ensure_initialized
                match db.ensure_initialized().await {
                    Ok(()) => {
                        tracing::info!("SQLite database initialized: {}", db_path.display());
                        Ok(db)
                    }
                    Err(e) => {
                        tracing::warn!("Database corrupted, attempting recovery: {}", e);
                        db.recover_corrupted(&db_path).await
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Database connection failed, attempting recovery: {}", e);
                // Try to recover by backing up and creating fresh
                Self::recover_from_scratch(&db_path).await
            }
        }
    }

    /// Recover a corrupted database by backing it up and creating a fresh one.
    async fn recover_corrupted(&self, db_path: &PathBuf) -> Result<Self, sqlx::Error> {
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let backup_path = format!("{}.corrupted.{}", db_path.display(), timestamp);

        tracing::warn!(
            "Creating backup of corrupted database: {}",
            backup_path
        );

        // Try to rename the corrupted file
        if let Err(e) = std::fs::rename(db_path, &backup_path) {
            tracing::warn!("Failed to rename corrupted database: {}", e);
            // Try to delete if rename fails
            let _ = std::fs::remove_file(db_path);
        }

        Self::recover_from_scratch(db_path).await
    }

    /// Create a fresh database after removing corrupted one.
    async fn recover_from_scratch(db_path: &PathBuf) -> Result<Self, sqlx::Error> {
        tracing::warn!("Creating fresh database after recovery");

        let db_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(&db_url)
            .await?;

        let db = Database { pool };
        db.ensure_initialized().await?;
        tracing::info!(
            "Fresh database created successfully: {}",
            db_path.display()
        );
        Ok(db)
    }

    async fn ensure_initialized(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS devices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                udid TEXT UNIQUE NOT NULL,
                serial TEXT,
                ip TEXT,
                port INTEGER,
                present INTEGER DEFAULT 0,
                ready INTEGER DEFAULT 0,
                using_device INTEGER DEFAULT 0,
                is_server INTEGER DEFAULT 0,
                is_mock INTEGER DEFAULT 0,
                update_time TEXT,
                model TEXT,
                brand TEXT,
                version TEXT,
                sdk INTEGER,
                memory TEXT,
                cpu TEXT,
                battery TEXT,
                display TEXT,
                owner TEXT,
                provider TEXT,
                agent_version TEXT,
                hwaddr TEXT,
                created_at TEXT,
                updated_at TEXT,
                extra_data TEXT,
                tags TEXT DEFAULT '[]'
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS installed_file (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                group_name TEXT NOT NULL,
                filename TEXT NOT NULL,
                filesize INTEGER,
                upload_time TEXT,
                who TEXT,
                extra_data TEXT,
                UNIQUE(group_name, filename)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_udid ON devices(udid)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_present ON devices(present)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_files_group ON installed_file(group_name)")
            .execute(&self.pool)
            .await?;

        // Connection history table for tracking device connections
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS connection_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                udid TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                FOREIGN KEY (udid) REFERENCES devices(udid)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_udid ON connection_history(udid)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_history_timestamp ON connection_history(timestamp)")
            .execute(&self.pool)
            .await?;

        // Migration: Add tags column to existing databases
        // This will silently fail if column already exists, which is fine
        let _ = sqlx::query("ALTER TABLE devices ADD COLUMN tags TEXT DEFAULT '[]'")
            .execute(&self.pool)
            .await;

        Ok(())
    }

    // ─── Device Operations ───

    /// Convert a SQLite row into a JSON object with MongoDB-style field names.
    fn device_row_to_json(row: &SqliteRow) -> Value {
        let mut map = Map::new();
        let columns: Vec<String> = row
            .columns()
            .iter()
            .map(|c| Column::name(c).to_string())
            .collect();

        for col_name in columns.iter() {
            let col: &str = col_name.as_str();
            if col == "id" {
                continue; // skip id (mimic MongoDB _id:0)
            }

            let json_key = column_to_json_key(col);

            // JSON fields
            if JSON_FIELDS.contains(&col) || col == "extra_data" {
                let raw: Option<String> = row.try_get(col).ok().flatten();
                if let Some(s) = raw {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&s) {
                        map.insert(json_key.to_string(), parsed);
                    } else {
                        map.insert(json_key.to_string(), Value::String(s));
                    }
                } else {
                    map.insert(json_key.to_string(), Value::Null);
                }
            }
            // Boolean fields
            else if BOOL_FIELDS.contains(&col) {
                let v: Option<i32> = row.try_get(col).ok().flatten();
                map.insert(json_key.to_string(), Value::Bool(v.unwrap_or(0) != 0));
            }
            // Integer fields
            else if col == "port" || col == "sdk" {
                let v: Option<i64> = row.try_get(col).ok().flatten();
                match v {
                    Some(n) => map.insert(json_key.to_string(), Value::Number(n.into())),
                    None => map.insert(json_key.to_string(), Value::Null),
                };
            }
            // String fields
            else {
                let v: Option<String> = row.try_get(col).ok().flatten();
                match v {
                    Some(s) => map.insert(json_key.to_string(), Value::String(s)),
                    None => map.insert(json_key.to_string(), Value::Null),
                };
            }
        }

        Value::Object(map)
    }

    /// Prepare device data from a MongoDB-style JSON map to SQLite columns.
    fn prepare_device_data(item: &Value) -> HashMap<String, Value> {
        let mut data = HashMap::new();
        let obj = match item.as_object() {
            Some(o) => o,
            None => return data,
        };

        // Mapped fields
        for &(mongo_key, sqlite_key) in FIELD_MAPPING {
            if let Some(v) = obj.get(mongo_key) {
                let val = match v {
                    Value::Bool(b) => Value::Number(if *b { 1.into() } else { 0.into() }),
                    other => other.clone(),
                };
                data.insert(sqlite_key.to_string(), val);
            }
        }

        // JSON fields
        for &json_field in JSON_FIELDS {
            if let Some(v) = obj.get(json_field) {
                if v.is_null() {
                    data.insert(json_field.to_string(), Value::Null);
                } else {
                    data.insert(json_field.to_string(), Value::String(v.to_string()));
                }
            }
        }

        // Extra data: anything not in FIELD_MAPPING or JSON_FIELDS
        let known_keys: Vec<&str> = FIELD_MAPPING
            .iter()
            .map(|(k, _)| *k)
            .chain(JSON_FIELDS.iter().copied())
            .collect();

        let extra: Map<String, Value> = obj
            .iter()
            .filter(|(k, _)| !known_keys.contains(&k.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        if !extra.is_empty() {
            data.insert(
                "extra_data".to_string(),
                Value::String(serde_json::to_string(&extra).unwrap_or_default()),
            );
        }

        data
    }

    /// Upsert a device by udid (INSERT ... ON CONFLICT DO UPDATE).
    pub async fn upsert(&self, udid: &str, item: &Value) -> Result<(), sqlx::Error> {
        let mut data = Self::prepare_device_data(item);
        data.entry("udid".to_string())
            .or_insert_with(|| Value::String(udid.to_string()));

        let columns: Vec<String> = data.keys().cloned().collect();
        let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{}", i)).collect();
        let update_clause: String = columns
            .iter()
            .filter(|c| c.as_str() != "udid")
            .map(|c| format!("{} = excluded.{}", c, c))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "INSERT INTO devices ({}) VALUES ({}) ON CONFLICT(udid) DO UPDATE SET {}",
            columns.join(", "),
            placeholders.join(", "),
            update_clause
        );

        let mut query = sqlx::query(&sql);
        for col in &columns {
            query = bind_value(query, data.get(col).unwrap_or(&Value::Null));
        }

        query.execute(&self.pool).await?;
        tracing::debug!("[SQLite] Device upserted: {}", udid);
        Ok(())
    }

    /// Update device by udid (no insert).
    pub async fn update(&self, udid: &str, item: &Value) -> Result<(), sqlx::Error> {
        let data = Self::prepare_device_data(item);
        if data.is_empty() {
            return Ok(());
        }

        let set_clause: String = data
            .keys()
            .enumerate()
            .map(|(i, k)| format!("{} = ?{}", k, i + 1))
            .collect::<Vec<_>>()
            .join(", ");

        let sql = format!(
            "UPDATE devices SET {} WHERE udid = ?{}",
            set_clause,
            data.len() + 1
        );

        let mut query = sqlx::query(&sql);
        for col in data.keys() {
            query = bind_value(query, data.get(col).unwrap_or(&Value::Null));
        }
        query = query.bind(udid);

        query.execute(&self.pool).await?;
        Ok(())
    }

    /// Find device by udid.
    pub async fn find_by_udid(&self, udid: &str) -> Result<Option<Value>, sqlx::Error> {
        let row = sqlx::query("SELECT * FROM devices WHERE udid = ?")
            .bind(udid)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.as_ref().map(Self::device_row_to_json))
    }

    /// Find all online devices (present=1).
    pub async fn find_device_list(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query("SELECT * FROM devices WHERE present = 1")
            .fetch_all(&self.pool)
            .await?;

        Ok(rows.iter().map(Self::device_row_to_json).collect())
    }

    /// Alias for find_device_list.
    pub async fn query_device_list_by_present(&self) -> Result<Vec<Value>, sqlx::Error> {
        self.find_device_list().await
    }

    /// Batch insert devices.
    pub async fn insert_many(&self, items: &[Value]) -> Result<(), sqlx::Error> {
        for item in items {
            let data = Self::prepare_device_data(item);
            if data.is_empty() {
                continue;
            }

            let columns: Vec<String> = data.keys().cloned().collect();
            let placeholders: Vec<String> =
                (1..=columns.len()).map(|i| format!("?{}", i)).collect();

            let sql = format!(
                "INSERT OR REPLACE INTO devices ({}) VALUES ({})",
                columns.join(", "),
                placeholders.join(", ")
            );

            let mut query = sqlx::query(&sql);
            for col in &columns {
                query = bind_value(query, data.get(col).unwrap_or(&Value::Null));
            }

            if let Err(e) = query.execute(&self.pool).await {
                tracing::error!("Insert error: {}", e);
            }
        }
        Ok(())
    }

    /// Delete all devices.
    pub async fn delete_all_devices(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM devices")
            .execute(&self.pool)
            .await?;
        tracing::debug!("All devices deleted from SQLite");
        Ok(())
    }

    // ─── Tag Operations ───

    /// Add tags to a device. Returns the updated tags list.
    pub async fn add_tags(&self, udid: &str, new_tags: &[String]) -> Result<Vec<String>, String> {
        // Get current device
        let device = self.find_by_udid(udid).await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("Device not found: {}", udid))?;

        // Parse current tags
        let mut tags: Vec<String> = device.get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();

        // Add new tags (deduplicate)
        for tag in new_tags {
            if !tags.contains(tag) && !tag.is_empty() {
                tags.push(tag.clone());
            }
        }

        // Update device
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query("UPDATE devices SET tags = ?1 WHERE udid = ?2")
            .bind(&tags_json)
            .bind(udid)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to update tags: {}", e))?;

        Ok(tags)
    }

    /// Remove a tag from a device. Returns the updated tags list.
    pub async fn remove_tag(&self, udid: &str, tag_to_remove: &str) -> Result<Vec<String>, String> {
        // Get current device
        let device = self.find_by_udid(udid).await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("Device not found: {}", udid))?;

        // Parse and filter tags
        let tags: Vec<String> = device.get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|t| t != tag_to_remove)
                    .collect()
            })
            .unwrap_or_default();

        // Update device
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query("UPDATE devices SET tags = ?1 WHERE udid = ?2")
            .bind(&tags_json)
            .bind(udid)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("Failed to update tags: {}", e))?;

        Ok(tags)
    }

    /// Find devices by tag (filter devices where tags array contains the tag).
    pub async fn find_devices_by_tag(&self, tag: &str) -> Result<Vec<Value>, sqlx::Error> {
        // SQLite JSON search: tags column contains the tag string
        let pattern = format!("%\"{}\"%", tag);
        let rows = sqlx::query(
            "SELECT * FROM devices WHERE present = 1 AND tags LIKE ?1"
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(Self::device_row_to_json).collect())
    }

    // ─── File Operations ───

    /// Save or replace an installed file record.
    pub async fn save_install_file(
        &self,
        group: &str,
        filename: &str,
        filesize: Option<i64>,
        upload_time: &str,
        who: &str,
        extra_data: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO installed_file
            (group_name, filename, filesize, upload_time, who, extra_data)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
        )
        .bind(group)
        .bind(filename)
        .bind(filesize)
        .bind(upload_time)
        .bind(who)
        .bind(extra_data)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Paginated query of installed files.
    pub async fn query_install_file(
        &self,
        group: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT * FROM installed_file WHERE group_name = ?1 LIMIT ?2 OFFSET ?3",
        )
        .bind(group)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let result: Vec<Value> = rows
            .iter()
            .map(|row| {
                let mut map = Map::new();
                let columns: Vec<String> = row
                    .columns()
                    .iter()
                    .map(|c| Column::name(c).to_string())
                    .collect();

                for col_name in columns.iter() {
                    let col: &str = col_name.as_str();
                    if col == "id" {
                        continue;
                    }

                    if col == "extra_data" {
                        let raw: Option<String> = row.try_get(col).ok().flatten();
                        if let Some(s) = raw {
                            if let Ok(extra) = serde_json::from_str::<Map<String, Value>>(&s) {
                                for (k, v) in extra {
                                    map.insert(k, v);
                                }
                                continue;
                            }
                        }
                        continue;
                    }

                    let json_key = column_to_json_key(col);
                    if col == "filesize" {
                        let v: Option<i64> = row.try_get(col).ok().flatten();
                        match v {
                            Some(n) => map.insert(json_key.to_string(), Value::Number(n.into())),
                            None => map.insert(json_key.to_string(), Value::Null),
                        };
                    } else {
                        let v: Option<String> = row.try_get(col).ok().flatten();
                        match v {
                            Some(s) => map.insert(json_key.to_string(), Value::String(s)),
                            None => map.insert(json_key.to_string(), Value::Null),
                        };
                    }
                }

                Value::Object(map)
            })
            .collect();

        Ok(result)
    }

    /// Count all installed files.
    pub async fn query_all_install_file(&self) -> Result<i64, sqlx::Error> {
        let row = sqlx::query("SELECT COUNT(*) as cnt FROM installed_file")
            .fetch_one(&self.pool)
            .await?;
        let count: i64 = row.try_get("cnt").unwrap_or(0);
        Ok(count)
    }

    // ─── Connection History Operations ───

    /// Record a connection event (connect or disconnect) for a device.
    pub async fn record_connection_event(
        &self,
        udid: &str,
        event_type: &str,
        timestamp: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO connection_history (udid, event_type, timestamp) VALUES (?1, ?2, ?3)"
        )
        .bind(udid)
        .bind(event_type)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get connection history for a device, ordered by timestamp descending.
    pub async fn get_connection_history(&self, udid: &str, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT event_type, timestamp FROM connection_history WHERE udid = ?1 ORDER BY timestamp DESC LIMIT ?2"
        )
        .bind(udid)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        let events: Vec<Value> = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                let event_type: String = row.get("event_type");
                let timestamp: String = row.get("timestamp");
                json!({
                    "event_type": event_type,
                    "timestamp": timestamp
                })
            })
            .collect();

        Ok(events)
    }

    /// Get connection history with calculated session durations.
    /// Returns events with session_duration_seconds for disconnect events.
    pub async fn get_connection_history_with_durations(&self, udid: &str, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        // Get events ordered by timestamp ascending for duration calculation
        let rows = sqlx::query(
            "SELECT event_type, timestamp FROM connection_history WHERE udid = ?1 ORDER BY timestamp ASC"
        )
        .bind(udid)
        .fetch_all(&self.pool)
        .await?;

        let events: Vec<(String, String)> = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                let event_type: String = row.get("event_type");
                let timestamp: String = row.get("timestamp");
                (event_type, timestamp)
            })
            .collect();

        // Calculate session durations
        let mut result: Vec<Value> = Vec::new();
        for (i, (event_type, timestamp)) in events.iter().enumerate() {
            let duration: Option<u64> = if event_type == "disconnect" && i > 0 {
                // Find the preceding connect event
                let mut found_duration: Option<u64> = None;
                for j in (0..i).rev() {
                    if events[j].0 == "connect" {
                        // Calculate duration between connect and disconnect
                        if let (Ok(connect_time), Ok(disconnect_time)) = (
                            chrono::DateTime::parse_from_rfc3339(&events[j].1),
                            chrono::DateTime::parse_from_rfc3339(timestamp),
                        ) {
                            let secs = (disconnect_time - connect_time).num_seconds();
                            found_duration = Some(secs.max(0) as u64);
                            break;
                        }
                    }
                }
                found_duration
            } else {
                None
            };

            result.push(json!({
                "event_type": event_type,
                "timestamp": timestamp,
                "session_duration_seconds": duration
            }));
        }

        // Reverse for most recent first
        result.reverse();

        // Apply limit
        if result.len() > limit as usize {
            result.truncate(limit as usize);
        }

        Ok(result)
    }

    /// Calculate uptime statistics for a device.
    /// Returns uptime percentages and total connected/disconnected time.
    pub async fn get_connection_stats(&self, udid: &str) -> Result<Value, sqlx::Error> {
        let now = chrono::Utc::now();

        // Get all events for this device
        let rows = sqlx::query(
            "SELECT event_type, timestamp FROM connection_history WHERE udid = ?1 ORDER BY timestamp ASC"
        )
        .bind(udid)
        .fetch_all(&self.pool)
        .await?;

        let events: Vec<(String, String)> = rows
            .iter()
            .map(|row| {
                use sqlx::Row;
                let event_type: String = row.get("event_type");
                let timestamp: String = row.get("timestamp");
                (event_type, timestamp)
            })
            .collect();

        // Calculate total connected time and uptime percentages
        let mut total_connected_seconds: i64 = 0;
        let mut connected_24h_seconds: i64 = 0;
        let mut connected_7d_seconds: i64 = 0;
        let mut first_seen: Option<String> = None;
        let mut last_connected: Option<String> = None;

        let window_24h = now - chrono::Duration::hours(24);
        let window_7d = now - chrono::Duration::days(7);

        for (i, (event_type, timestamp)) in events.iter().enumerate() {
            if first_seen.is_none() {
                first_seen = Some(timestamp.clone());
            }

            if event_type == "connect" {
                last_connected = Some(timestamp.clone());
            }

            // Calculate session duration for disconnect events
            if event_type == "disconnect" && i > 0 {
                for j in (0..i).rev() {
                    if events[j].0 == "connect" {
                        if let (Ok(connect_time), Ok(disconnect_time)) = (
                            chrono::DateTime::parse_from_rfc3339(&events[j].1),
                            chrono::DateTime::parse_from_rfc3339(timestamp),
                        ) {
                            let session_secs = (disconnect_time - connect_time).num_seconds().max(0);
                            total_connected_seconds += session_secs;

                            // Check if session falls within time windows
                            let connect_utc = connect_time.with_timezone(&chrono::Utc);
                            let disconnect_utc = disconnect_time.with_timezone(&chrono::Utc);

                            // 24h window
                            if disconnect_utc > window_24h {
                                let effective_start = if connect_utc > window_24h {
                                    connect_utc
                                } else {
                                    window_24h
                                };
                                let secs_in_window = (disconnect_utc - effective_start).num_seconds().max(0);
                                connected_24h_seconds += secs_in_window;
                            }

                            // 7d window
                            if disconnect_utc > window_7d {
                                let effective_start = if connect_utc > window_7d {
                                    connect_utc
                                } else {
                                    window_7d
                                };
                                let secs_in_window = (disconnect_utc - effective_start).num_seconds().max(0);
                                connected_7d_seconds += secs_in_window;
                            }
                        }
                        break;
                    }
                }
            }
        }

        // If device is currently connected, add time from last connect to now
        // Check the last event
        if let Some((last_event_type, last_timestamp)) = events.last() {
            if last_event_type == "connect" {
                if let Ok(connect_time) = chrono::DateTime::parse_from_rfc3339(last_timestamp) {
                    let connect_utc = connect_time.with_timezone(&chrono::Utc);
                    let session_secs = (now - connect_utc).num_seconds().max(0);
                    total_connected_seconds += session_secs;

                    // Add to 24h and 7d windows
                    if connect_utc > window_24h {
                        connected_24h_seconds += session_secs;
                    } else {
                        connected_24h_seconds += (now - window_24h).num_seconds();
                    }

                    if connect_utc > window_7d {
                        connected_7d_seconds += session_secs;
                    } else {
                        connected_7d_seconds += (now - window_7d).num_seconds();
                    }
                }
            }
        }

        // Calculate percentages
        let uptime_24h_percent = (connected_24h_seconds as f64 / 86400.0) * 100.0;
        let uptime_7d_percent = (connected_7d_seconds as f64 / (86400.0 * 7.0)) * 100.0;

        Ok(json!({
            "uptime_24h_percent": (uptime_24h_percent * 100.0).round() / 100.0,
            "uptime_7d_percent": (uptime_7d_percent * 100.0).round() / 100.0,
            "total_connected_seconds": total_connected_seconds,
            "first_seen": first_seen,
            "last_connected": last_connected
        }))
    }

    /// Delete a file record.
    pub async fn delete_install_file(
        &self,
        group: &str,
        filename: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM installed_file WHERE group_name = ?1 AND filename = ?2")
            .bind(group)
            .bind(filename)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Helper to bind a serde_json::Value to a sqlx query.
fn bind_value<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    val: &'q Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    match val {
        Value::Null => query.bind(None::<String>),
        Value::Bool(b) => query.bind(if *b { 1i32 } else { 0i32 }),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                query.bind(i)
            } else if let Some(f) = n.as_f64() {
                query.bind(f)
            } else {
                query.bind(n.to_string())
            }
        }
        Value::String(s) => query.bind(s.as_str()),
        _ => query.bind(val.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    async fn create_temp_db() -> (tempfile::TempDir, Database) {
        let tmp = tempfile::TempDir::new().unwrap();
        let db = Database::new(tmp.path().to_str().unwrap(), "test.db")
            .await
            .unwrap();
        (tmp, db)
    }

    #[tokio::test]
    async fn test_database_new_creates_tables() {
        let (_tmp, db) = create_temp_db().await;
        // Verify tables exist by running queries against them
        let devices = db.find_device_list().await;
        assert!(devices.is_ok());
        let count = db.query_all_install_file().await;
        assert!(count.is_ok());
        assert_eq!(count.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_upsert_insert() {
        let (_tmp, db) = create_temp_db().await;
        let data = json!({
            "udid": "test-device-1",
            "serial": "ABC123",
            "ip": "192.168.1.100",
            "port": 7912,
            "present": true,
            "ready": true,
            "model": "Pixel 5",
        });
        db.upsert("test-device-1", &data).await.unwrap();

        let result = db.find_by_udid("test-device-1").await.unwrap();
        assert!(result.is_some());
        let device = result.unwrap();
        assert_eq!(device["udid"], "test-device-1");
        assert_eq!(device["serial"], "ABC123");
        assert_eq!(device["ip"], "192.168.1.100");
        assert_eq!(device["model"], "Pixel 5");
        assert_eq!(device["present"], true);
    }

    #[tokio::test]
    async fn test_upsert_update() {
        let (_tmp, db) = create_temp_db().await;
        let data1 = json!({"udid": "dev1", "model": "Phone A", "present": true});
        db.upsert("dev1", &data1).await.unwrap();

        let data2 = json!({"udid": "dev1", "model": "Phone B", "present": false});
        db.upsert("dev1", &data2).await.unwrap();

        let result = db.find_by_udid("dev1").await.unwrap().unwrap();
        assert_eq!(result["model"], "Phone B");
        assert_eq!(result["present"], false);
    }

    #[tokio::test]
    async fn test_update_existing() {
        let (_tmp, db) = create_temp_db().await;
        let data = json!({"udid": "dev1", "model": "Phone A", "ip": "10.0.0.1", "present": true});
        db.upsert("dev1", &data).await.unwrap();

        let update = json!({"present": false});
        db.update("dev1", &update).await.unwrap();

        let result = db.find_by_udid("dev1").await.unwrap().unwrap();
        assert_eq!(result["present"], false);
        assert_eq!(result["model"], "Phone A"); // unchanged
        assert_eq!(result["ip"], "10.0.0.1"); // unchanged
    }

    #[tokio::test]
    async fn test_update_nonexistent() {
        let (_tmp, db) = create_temp_db().await;
        let update = json!({"present": false});
        // Should not error even if device doesn't exist
        let result = db.update("nonexistent", &update).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_find_by_udid_not_found() {
        let (_tmp, db) = create_temp_db().await;
        let result = db.find_by_udid("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_find_device_list_present() {
        let (_tmp, db) = create_temp_db().await;
        db.upsert("dev1", &json!({"udid": "dev1", "present": true})).await.unwrap();
        db.upsert("dev2", &json!({"udid": "dev2", "present": false})).await.unwrap();
        db.upsert("dev3", &json!({"udid": "dev3", "present": true})).await.unwrap();

        let list = db.find_device_list().await.unwrap();
        assert_eq!(list.len(), 2);
        let udids: Vec<&str> = list.iter()
            .map(|d| d["udid"].as_str().unwrap())
            .collect();
        assert!(udids.contains(&"dev1"));
        assert!(udids.contains(&"dev3"));
    }

    #[tokio::test]
    async fn test_insert_many() {
        let (_tmp, db) = create_temp_db().await;
        let items = vec![
            json!({"udid": "dev1", "present": true}),
            json!({"udid": "dev2", "present": true}),
            json!({"udid": "dev3", "present": true}),
        ];
        db.insert_many(&items).await.unwrap();

        let list = db.find_device_list().await.unwrap();
        assert_eq!(list.len(), 3);
    }

    #[tokio::test]
    async fn test_delete_all_devices() {
        let (_tmp, db) = create_temp_db().await;
        db.upsert("dev1", &json!({"udid": "dev1", "present": true})).await.unwrap();
        db.upsert("dev2", &json!({"udid": "dev2", "present": true})).await.unwrap();

        db.delete_all_devices().await.unwrap();

        let list = db.find_device_list().await.unwrap();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_field_mapping_bool() {
        let (_tmp, db) = create_temp_db().await;
        let data = json!({
            "udid": "dev1",
            "using": true,
            "is_mock": true,
            "present": true,
        });
        db.upsert("dev1", &data).await.unwrap();

        let result = db.find_by_udid("dev1").await.unwrap().unwrap();
        assert_eq!(result["using"], true);
        assert_eq!(result["is_mock"], true);
        assert_eq!(result["present"], true);
    }

    #[tokio::test]
    async fn test_field_mapping_json() {
        let (_tmp, db) = create_temp_db().await;
        let data = json!({
            "udid": "dev1",
            "present": true,
            "memory": {"total": 8192, "free": 4096},
            "cpu": {"cores": 8},
            "battery": {"level": 85},
            "display": {"width": 1080, "height": 1920},
        });
        db.upsert("dev1", &data).await.unwrap();

        let result = db.find_by_udid("dev1").await.unwrap().unwrap();
        assert_eq!(result["memory"]["total"], 8192);
        assert_eq!(result["memory"]["free"], 4096);
        assert_eq!(result["cpu"]["cores"], 8);
        assert_eq!(result["battery"]["level"], 85);
        assert_eq!(result["display"]["width"], 1080);
        assert_eq!(result["display"]["height"], 1920);
    }

    #[tokio::test]
    async fn test_save_and_query_install_file() {
        let (_tmp, db) = create_temp_db().await;
        db.save_install_file("group1", "app.apk", Some(1024), "2024-01-01", "admin", None)
            .await
            .unwrap();

        let files = db.query_install_file("group1", 0, 10).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["group"], "group1");
        assert_eq!(files[0]["filename"], "app.apk");
        assert_eq!(files[0]["filesize"], 1024);
    }

    #[tokio::test]
    async fn test_delete_install_file() {
        let (_tmp, db) = create_temp_db().await;
        db.save_install_file("g1", "test.apk", Some(512), "2024-01-01", "user", None)
            .await
            .unwrap();

        db.delete_install_file("g1", "test.apk").await.unwrap();

        let files = db.query_install_file("g1", 0, 10).await.unwrap();
        assert_eq!(files.len(), 0);
    }
}
