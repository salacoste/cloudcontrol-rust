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

impl Database {
    /// Get a clone of the underlying SQLite pool.
    pub fn get_pool(&self) -> SqlitePool {
        self.pool.clone()
    }
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

        // Migration: Add product_id column to existing databases
        // This will silently fail if column already exists, which is fine
        let _ = sqlx::query("ALTER TABLE devices ADD COLUMN product_id INTEGER")
            .execute(&self.pool)
            .await;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_product_id ON devices(product_id)")
            .execute(&self.pool)
            .await?;

        // Migration: Add property_id column to existing databases
        // This will silently fail if column already exists, which is fine
        let _ = sqlx::query("ALTER TABLE devices ADD COLUMN property_id TEXT")
            .execute(&self.pool)
            .await;

        // Recording sessions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS recordings (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                device_udid TEXT NOT NULL,
                action_count INTEGER DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Recorded actions table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS recorded_actions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                recording_id INTEGER NOT NULL,
                action_type TEXT NOT NULL,
                x INTEGER,
                y INTEGER,
                x2 INTEGER,
                y2 INTEGER,
                duration_ms INTEGER,
                text TEXT,
                key_code INTEGER,
                sequence_order INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                FOREIGN KEY (recording_id) REFERENCES recordings(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_recordings_device ON recordings(device_udid)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_recordings_created ON recordings(created_at)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_actions_recording ON recorded_actions(recording_id)")
            .execute(&self.pool)
            .await?;

        // Batch reports tables
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS batch_reports (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                operation_type TEXT NOT NULL,
                created_at TEXT NOT NULL,
                completed_at TEXT,
                total_devices INTEGER NOT NULL,
                successful INTEGER NOT NULL DEFAULT 0,
                failed INTEGER NOT NULL DEFAULT 0
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS batch_report_results (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                batch_report_id INTEGER NOT NULL,
                udid TEXT NOT NULL,
                status TEXT NOT NULL,
                error_code TEXT,
                error_message TEXT,
                duration_ms INTEGER,
                screenshot TEXT,
                sequence_order INTEGER NOT NULL,
                FOREIGN KEY (batch_report_id) REFERENCES batch_reports(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_batch_reports_created ON batch_reports(created_at)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_batch_report_results_report ON batch_report_results(batch_report_id)")
            .execute(&self.pool)
            .await?;

        // Products catalog table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS products (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                brand TEXT NOT NULL,
                model TEXT NOT NULL,
                name TEXT,
                cpu TEXT,
                gpu TEXT,
                link TEXT,
                coverage INTEGER DEFAULT 0
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_products_brand ON products(brand)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_products_brand_model ON products(brand, model)")
            .execute(&self.pool)
            .await?;

        // Providers table (server nodes in the device farm)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS providers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ip TEXT NOT NULL UNIQUE,
                notes TEXT DEFAULT '',
                present INTEGER DEFAULT 0,
                presence_changed_at INTEGER,
                created_at INTEGER NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_providers_ip ON providers(ip)")
            .execute(&self.pool)
            .await?;

        // Videos table (JPEG-to-MP4 video recordings, Story 11-2)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS videos (
                id TEXT PRIMARY KEY,
                udid TEXT NOT NULL,
                file_path TEXT NOT NULL,
                started_at TEXT NOT NULL,
                stopped_at TEXT,
                frame_count INTEGER DEFAULT 0,
                fps INTEGER DEFAULT 2,
                status TEXT DEFAULT 'recording',
                duration_ms INTEGER,
                file_size INTEGER,
                device_name TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_videos_udid ON videos(udid)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_videos_status ON videos(status)")
            .execute(&self.pool)
            .await?;

        // Users table (Story 14-1: Multi-User Authentication)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                email TEXT UNIQUE NOT NULL,
                password_hash TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'agent',
                team_id TEXT,
                created_at TEXT NOT NULL,
                last_login_at TEXT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_users_email ON users(email)")
            .execute(&self.pool)
            .await?;

        // Refresh tokens table (Story 14-1: JWT + Refresh Token Pattern)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS refresh_tokens (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                token_hash TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                revoked INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user ON refresh_tokens(user_id)")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_refresh_tokens_hash ON refresh_tokens(token_hash)")
            .execute(&self.pool)
            .await?;

        // Migration: Add session metadata columns to refresh_tokens (Story 14-4)
        // These will silently fail if columns already exist, which is fine
        let _ = sqlx::query("ALTER TABLE refresh_tokens ADD COLUMN last_used_at TEXT")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("ALTER TABLE refresh_tokens ADD COLUMN user_agent TEXT")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("ALTER TABLE refresh_tokens ADD COLUMN ip_address TEXT")
            .execute(&self.pool)
            .await;

        // Teams table (Story 14-3: Team/Organization Scoping)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS teams (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL UNIQUE,
                description TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_teams_name ON teams(name)")
            .execute(&self.pool)
            .await?;

        // Migration: Add team_id column to devices table (Story 14-3)
        // This will silently fail if column already exists, which is fine
        let _ = sqlx::query("ALTER TABLE devices ADD COLUMN team_id TEXT REFERENCES teams(id)")
            .execute(&self.pool)
            .await;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_devices_team ON devices(team_id)")
            .execute(&self.pool)
            .await?;

        // Audit log table (Story 14-3: Team operations audit trail)
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                action TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                actor_email TEXT,
                target_type TEXT NOT NULL,
                target_id TEXT NOT NULL,
                team_id TEXT,
                details TEXT,
                created_at TEXT NOT NULL
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_actor ON audit_log(actor_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_target ON audit_log(target_type, target_id)")
            .execute(&self.pool)
            .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_team ON audit_log(team_id)")
            .execute(&self.pool)
            .await?;

        // Index for date range queries (Story 14-5)
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at)")
            .execute(&self.pool)
            .await?;

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
            else if col == "port" || col == "sdk" || col == "product_id" {
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

    // ─── Recording Operations ───

    /// Create a new recording session.
    pub async fn create_recording(
        &self,
        name: &str,
        device_udid: &str,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now().timestamp();
        let result = sqlx::query(
            "INSERT INTO recordings (name, device_udid, action_count, created_at, updated_at) VALUES (?1, ?2, 1, ?3, ?3)"
        )
        .bind(name)
        .bind(device_udid)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get a recording by ID.
    pub async fn get_recording(&self, id: i64) -> Result<Option<Value>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, name, device_udid, action_count, created_at, updated_at FROM recordings WHERE id = ?1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(Self::recording_row_to_json))
    }

    /// List all recordings, optionally filtered by device.
    pub async fn list_recordings(&self, device_udid: Option<&str>) -> Result<Vec<Value>, sqlx::Error> {
        let rows = if let Some(udid) = device_udid {
            sqlx::query(
                "SELECT id, name, device_udid, action_count, created_at, updated_at FROM recordings WHERE device_udid = ?1 ORDER BY created_at DESC"
            )
            .bind(udid)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, name, device_udid, action_count, created_at, updated_at FROM recordings ORDER BY created_at DESC"
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.iter().map(Self::recording_row_to_json).collect())
    }

    /// Delete a recording and all its actions.
    pub async fn delete_recording(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM recordings WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Add an action to a recording.
    pub async fn add_recorded_action(
        &self,
        recording_id: i64,
        action_type: &str,
        x: Option<i32>,
        y: Option<i32>,
        x2: Option<i32>,
        y2: Option<i32>,
        duration_ms: Option<i32>,
        text: Option<&str>,
        key_code: Option<i32>,
    ) -> Result<(i64, i32), sqlx::Error> {
        let now = chrono::Utc::now().timestamp();

        // Get next sequence order
        let max_order: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(sequence_order), 0) FROM recorded_actions WHERE recording_id = ?1"
        )
        .bind(recording_id)
        .fetch_one(&self.pool)
        .await?;

        let sequence_order = max_order + 1;

        let result = sqlx::query(
            r#"
            INSERT INTO recorded_actions
            (recording_id, action_type, x, y, x2, y2, duration_ms, text, key_code, sequence_order, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#
        )
        .bind(recording_id)
        .bind(action_type)
        .bind(x)
        .bind(y)
        .bind(x2)
        .bind(y2)
        .bind(duration_ms)
        .bind(text)
        .bind(key_code)
        .bind(sequence_order)
        .bind(now)
        .execute(&self.pool)
        .await?;

        let action_id = result.last_insert_rowid();

        // Update recording action count and updated_at
        sqlx::query(
            "UPDATE recordings SET action_count = action_count + 1, updated_at = ?1 WHERE id = ?2"
        )
        .bind(now)
        .bind(recording_id)
        .execute(&self.pool)
        .await?;

        Ok((action_id, sequence_order))
    }

    /// Get all actions for a recording.
    pub async fn get_recording_actions(&self, recording_id: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            r#"
            SELECT id, recording_id, action_type, x, y, x2, y2, duration_ms, text, key_code, sequence_order, created_at
            FROM recorded_actions
            WHERE recording_id = ?1
            ORDER BY sequence_order ASC
            "#
        )
        .bind(recording_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(Self::action_row_to_json).collect())
    }

    /// Get a recording with all its actions.
    pub async fn get_recording_with_actions(&self, id: i64) -> Result<Option<Value>, sqlx::Error> {
        let recording = self.get_recording(id).await?;
        if recording.is_none() {
            return Ok(None);
        }

        let recording = recording.unwrap();
        let actions = self.get_recording_actions(id).await?;

        let mut result = recording.clone();
        if let Some(obj) = result.as_object_mut() {
            obj.insert("actions".to_string(), Value::Array(actions));
        }

        Ok(Some(result))
    }

    /// Convert a recording row to JSON.
    fn recording_row_to_json(row: &SqliteRow) -> Value {
        let id: i64 = row.get("id");
        let name: String = row.get("name");
        let device_udid: String = row.get("device_udid");
        let action_count: i32 = row.get("action_count");
        let created_at: i64 = row.get("created_at");
        let updated_at: i64 = row.get("updated_at");

        json!({
            "id": id,
            "name": name,
            "device_udid": device_udid,
            "action_count": action_count,
            "createdAt": created_at,
            "updatedAt": updated_at
        })
    }

    /// Convert an action row to JSON.
    fn action_row_to_json(row: &SqliteRow) -> Value {
        let id: i64 = row.get("id");
        let recording_id: i64 = row.get("recording_id");
        let action_type: String = row.get("action_type");
        let x: Option<i32> = row.get("x");
        let y: Option<i32> = row.get("y");
        let x2: Option<i32> = row.get("x2");
        let y2: Option<i32> = row.get("y2");
        let duration_ms: Option<i32> = row.get("duration_ms");
        let text: Option<String> = row.get("text");
        let key_code: Option<i32> = row.get("key_code");
        let sequence_order: i32 = row.get("sequence_order");
        let created_at: i64 = row.get("created_at");

        let mut obj = serde_json::Map::new();
        obj.insert("id".to_string(), json!(id));
        obj.insert("recording_id".to_string(), json!(recording_id));
        obj.insert("action_type".to_string(), json!(action_type));
        obj.insert("sequence_order".to_string(), json!(sequence_order));
        obj.insert("created_at".to_string(), json!(created_at));

        if let Some(v) = x {
            obj.insert("x".to_string(), json!(v));
        }
        if let Some(v) = y {
            obj.insert("y".to_string(), json!(v));
        }
        if let Some(v) = x2 {
            obj.insert("x2".to_string(), json!(v));
        }
        if let Some(v) = y2 {
            obj.insert("y2".to_string(), json!(v));
        }
        if let Some(v) = duration_ms {
            obj.insert("duration_ms".to_string(), json!(v));
        }
        if let Some(v) = text {
            obj.insert("text".to_string(), json!(v));
        }
        if let Some(v) = key_code {
            obj.insert("key_code".to_string(), json!(v));
        }

        Value::Object(obj)
    }

    // ─── Batch Report Operations ───

    /// Create a new batch report.
    pub async fn create_batch_report(
        &self,
        operation_type: &str,
        total_devices: i32,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = sqlx::query(
            "INSERT INTO batch_reports (operation_type, created_at, total_devices, successful, failed) VALUES (?1, ?2, ?3, 0, 0)"
        )
        .bind(operation_type)
        .bind(&now)
        .bind(total_devices)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Complete a batch report by setting completed_at and final counts.
    pub async fn complete_batch_report(
        &self,
        id: i64,
        successful: i32,
        failed: i32,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE batch_reports SET completed_at = ?1, successful = ?2, failed = ?3 WHERE id = ?4"
        )
        .bind(&now)
        .bind(successful)
        .bind(failed)
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Add a device result to a batch report.
    pub async fn add_batch_report_result(
        &self,
        batch_report_id: i64,
        udid: &str,
        status: &str,
        error_code: Option<&str>,
        error_message: Option<&str>,
        duration_ms: Option<i32>,
        screenshot: Option<&str>,
        sequence_order: i32,
    ) -> Result<i64, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO batch_report_results (batch_report_id, udid, status, error_code, error_message, duration_ms, screenshot, sequence_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        )
        .bind(batch_report_id)
        .bind(udid)
        .bind(status)
        .bind(error_code)
        .bind(error_message)
        .bind(duration_ms)
        .bind(screenshot)
        .bind(sequence_order)
        .execute(&self.pool)
        .await?;

        Ok(result.last_insert_rowid())
    }

    /// Get a batch report by ID.
    pub async fn get_batch_report(&self, id: i64) -> Result<Option<Value>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, operation_type, created_at, completed_at, total_devices, successful, failed FROM batch_reports WHERE id = ?1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(|r| {
            use sqlx::Row;
            let id: i64 = r.get("id");
            let operation_type: String = r.get("operation_type");
            let created_at: String = r.get("created_at");
            let completed_at: Option<String> = r.get("completed_at");
            let total_devices: i32 = r.get("total_devices");
            let successful: i32 = r.get("successful");
            let failed: i32 = r.get("failed");

            json!({
                "id": id,
                "operation_type": operation_type,
                "createdAt": created_at,
                "completedAt": completed_at,
                "total_devices": total_devices,
                "successful": successful,
                "failed": failed
            })
        }))
    }

    /// Get all device results for a batch report.
    pub async fn get_batch_report_results(&self, batch_report_id: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, batch_report_id, udid, status, error_code, error_message, duration_ms, screenshot, sequence_order FROM batch_report_results WHERE batch_report_id = ?1 ORDER BY sequence_order ASC"
        )
        .bind(batch_report_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.iter().map(|r| {
            use sqlx::Row;
            let id: i64 = r.get("id");
            let udid: String = r.get("udid");
            let status: String = r.get("status");
            let error_code: Option<String> = r.get("error_code");
            let error_message: Option<String> = r.get("error_message");
            let duration_ms: Option<i32> = r.get("duration_ms");
            let screenshot: Option<String> = r.get("screenshot");
            let sequence_order: i32 = r.get("sequence_order");

            json!({
                "id": id,
                "udid": udid,
                "status": status,
                "error_code": error_code,
                "error_message": error_message,
                "duration_ms": duration_ms,
                "screenshot": screenshot,
                "sequence_order": sequence_order
            })
        }).collect())
    }

    /// Get a batch report with all its device results.
    pub async fn get_batch_report_with_results(&self, id: i64) -> Result<Option<Value>, sqlx::Error> {
        let report = self.get_batch_report(id).await?;
        if report.is_none() {
            return Ok(None);
        }

        let results = self.get_batch_report_results(id).await?;

        let mut report = report.unwrap();
        if let Some(obj) = report.as_object_mut() {
            obj.insert("results".to_string(), Value::Array(results));
        }

        Ok(Some(report))
    }

    /// List all batch reports, optionally filtered by operation type.
    pub async fn list_batch_reports(&self, operation_type: Option<&str>) -> Result<Vec<Value>, sqlx::Error> {
        let rows = if let Some(op_type) = operation_type {
            sqlx::query(
                "SELECT id, operation_type, created_at, completed_at, total_devices, successful, failed FROM batch_reports WHERE operation_type = ?1 ORDER BY created_at DESC"
            )
            .bind(op_type)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query(
                "SELECT id, operation_type, created_at, completed_at, total_devices, successful, failed FROM batch_reports ORDER BY created_at DESC"
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(rows.iter().map(|r| {
            use sqlx::Row;
            let id: i64 = r.get("id");
            let operation_type: String = r.get("operation_type");
            let created_at: String = r.get("created_at");
            let completed_at: Option<String> = r.get("completed_at");
            let total_devices: i32 = r.get("total_devices");
            let successful: i32 = r.get("successful");
            let failed: i32 = r.get("failed");

            json!({
                "id": id,
                "operation_type": operation_type,
                "createdAt": created_at,
                "completedAt": completed_at,
                "total_devices": total_devices,
                "successful": successful,
                "failed": failed
            })
        }).collect())
    }

    /// Delete a batch report and all its results.
    pub async fn delete_batch_report(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM batch_reports WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Delete all batch reports.
    pub async fn delete_all_batch_reports(&self) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM batch_reports")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ─── Product Catalog Operations ───

    /// Create a new product in the catalog.
    pub async fn create_product(
        &self,
        brand: &str,
        model: &str,
        name: Option<&str>,
        cpu: Option<&str>,
        gpu: Option<&str>,
        link: Option<&str>,
        coverage: Option<i64>,
    ) -> Result<crate::models::product::Product, sqlx::Error> {
        let result = sqlx::query(
            "INSERT INTO products (brand, model, name, cpu, gpu, link, coverage) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        )
        .bind(brand)
        .bind(model)
        .bind(name)
        .bind(cpu)
        .bind(gpu)
        .bind(link)
        .bind(coverage.unwrap_or(0))
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        Ok(crate::models::product::Product {
            id,
            brand: brand.to_string(),
            model: model.to_string(),
            name: name.map(|s| s.to_string()),
            cpu: cpu.map(|s| s.to_string()),
            gpu: gpu.map(|s| s.to_string()),
            link: link.map(|s| s.to_string()),
            coverage: Some(coverage.unwrap_or(0)),
        })
    }

    /// Get a product by ID.
    pub async fn get_product(&self, id: i64) -> Result<Option<crate::models::product::Product>, sqlx::Error> {
        sqlx::query_as::<_, crate::models::product::Product>(
            "SELECT id, brand, model, name, cpu, gpu, link, coverage FROM products WHERE id = ?1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// List products with optional brand/model partial-match filters.
    pub async fn list_products(
        &self,
        brand_filter: Option<&str>,
        model_filter: Option<&str>,
    ) -> Result<Vec<crate::models::product::Product>, sqlx::Error> {
        // Escape LIKE wildcards (% and _) in user input
        let brand_like = brand_filter.map(|b| format!("%{}%", b.replace('%', "\\%").replace('_', "\\_")));
        let model_like = model_filter.map(|m| format!("%{}%", m.replace('%', "\\%").replace('_', "\\_")));

        match (brand_like.as_ref(), model_like.as_ref()) {
            (Some(b), Some(m)) => {
                sqlx::query_as::<_, crate::models::product::Product>(
                    "SELECT id, brand, model, name, cpu, gpu, link, coverage FROM products WHERE brand LIKE ?1 ESCAPE '\\' AND model LIKE ?2 ESCAPE '\\' ORDER BY brand, model"
                )
                .bind(b)
                .bind(m)
                .fetch_all(&self.pool)
                .await
            }
            (Some(b), None) => {
                sqlx::query_as::<_, crate::models::product::Product>(
                    "SELECT id, brand, model, name, cpu, gpu, link, coverage FROM products WHERE brand LIKE ?1 ESCAPE '\\' ORDER BY brand, model"
                )
                .bind(b)
                .fetch_all(&self.pool)
                .await
            }
            (None, Some(m)) => {
                sqlx::query_as::<_, crate::models::product::Product>(
                    "SELECT id, brand, model, name, cpu, gpu, link, coverage FROM products WHERE model LIKE ?1 ESCAPE '\\' ORDER BY brand, model"
                )
                .bind(m)
                .fetch_all(&self.pool)
                .await
            }
            (None, None) => {
                sqlx::query_as::<_, crate::models::product::Product>(
                    "SELECT id, brand, model, name, cpu, gpu, link, coverage FROM products ORDER BY brand, model"
                )
                .fetch_all(&self.pool)
                .await
            }
        }
    }

    /// Update a product by ID. Returns the updated product, or None if not found.
    pub async fn update_product(
        &self,
        id: i64,
        brand: Option<&str>,
        model: Option<&str>,
        name: Option<&str>,
        cpu: Option<&str>,
        gpu: Option<&str>,
        link: Option<&str>,
        coverage: Option<i64>,
    ) -> Result<Option<crate::models::product::Product>, sqlx::Error> {
        let mut set_clauses = Vec::new();
        let mut param_idx = 1;

        // Build dynamic SET clause
        if brand.is_some() { set_clauses.push(format!("brand = ?{}", param_idx)); param_idx += 1; }
        if model.is_some() { set_clauses.push(format!("model = ?{}", param_idx)); param_idx += 1; }
        if name.is_some() { set_clauses.push(format!("name = ?{}", param_idx)); param_idx += 1; }
        if cpu.is_some() { set_clauses.push(format!("cpu = ?{}", param_idx)); param_idx += 1; }
        if gpu.is_some() { set_clauses.push(format!("gpu = ?{}", param_idx)); param_idx += 1; }
        if link.is_some() { set_clauses.push(format!("link = ?{}", param_idx)); param_idx += 1; }
        if coverage.is_some() { set_clauses.push(format!("coverage = ?{}", param_idx)); param_idx += 1; }

        if set_clauses.is_empty() {
            return self.get_product(id).await;
        }

        let sql = format!(
            "UPDATE products SET {} WHERE id = ?{}",
            set_clauses.join(", "),
            param_idx
        );

        let mut query = sqlx::query(&sql);
        if let Some(v) = brand { query = query.bind(v); }
        if let Some(v) = model { query = query.bind(v); }
        if let Some(v) = name { query = query.bind(v); }
        if let Some(v) = cpu { query = query.bind(v); }
        if let Some(v) = gpu { query = query.bind(v); }
        if let Some(v) = link { query = query.bind(v); }
        if let Some(v) = coverage { query = query.bind(v); }
        query = query.bind(id);

        let result = query.execute(&self.pool).await?;
        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_product(id).await
    }

    /// Delete a product by ID. Returns true if deleted, false if not found.
    pub async fn delete_product(&self, id: i64) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM products WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected() > 0)
    }

    /// List products by exact brand and model match (legacy endpoint for edit.html).
    pub async fn list_products_by_brand_model(
        &self,
        brand: &str,
        model: &str,
    ) -> Result<Vec<crate::models::product::Product>, sqlx::Error> {
        sqlx::query_as::<_, crate::models::product::Product>(
            "SELECT id, brand, model, name, cpu, gpu, link, coverage FROM products WHERE brand = ?1 AND model = ?2 ORDER BY name"
        )
        .bind(brand)
        .bind(model)
        .fetch_all(&self.pool)
        .await
    }

    // ─── Device-Product Association ───

    /// Update a device's product_id. Returns true if device was found and updated.
    pub async fn update_device_product(
        &self,
        udid: &str,
        product_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE devices SET product_id = ?1 WHERE udid = ?2")
            .bind(product_id)
            .bind(udid)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update a device's property_id (asset/inventory number). Returns true if device was found.
    pub async fn update_device_property(
        &self,
        udid: &str,
        property_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("UPDATE devices SET property_id = ?1 WHERE udid = ?2")
            .bind(property_id)
            .bind(udid)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // ─── Provider Operations ───

    /// Create a new provider. Returns the created provider.
    pub async fn create_provider(
        &self,
        ip: &str,
        notes: Option<&str>,
    ) -> Result<crate::models::provider::Provider, sqlx::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let result = sqlx::query(
            "INSERT INTO providers (ip, notes, present, created_at) VALUES (?1, ?2, 0, ?3)",
        )
        .bind(ip)
        .bind(notes.unwrap_or(""))
        .bind(now)
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        Ok(crate::models::provider::Provider {
            id,
            ip: ip.to_string(),
            notes: Some(notes.unwrap_or("").to_string()),
            present: false,
            presence_changed_at: None,
            created_at: now,
        })
    }

    /// List all providers ordered by id.
    pub async fn list_providers(
        &self,
    ) -> Result<Vec<crate::models::provider::Provider>, sqlx::Error> {
        sqlx::query_as::<_, crate::models::provider::Provider>(
            "SELECT id, ip, notes, present, presence_changed_at, created_at FROM providers ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Get a provider by ID.
    pub async fn get_provider(
        &self,
        id: i64,
    ) -> Result<Option<crate::models::provider::Provider>, sqlx::Error> {
        sqlx::query_as::<_, crate::models::provider::Provider>(
            "SELECT id, ip, notes, present, presence_changed_at, created_at FROM providers WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Update a provider's notes. Returns the updated provider if found.
    pub async fn update_provider_notes(
        &self,
        id: i64,
        notes: &str,
    ) -> Result<Option<crate::models::provider::Provider>, sqlx::Error> {
        let result = sqlx::query("UPDATE providers SET notes = ?1 WHERE id = ?2")
            .bind(notes)
            .bind(id)
            .execute(&self.pool)
            .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        self.get_provider(id).await
    }

    /// Count devices associated with a provider by IP.
    pub async fn count_devices_by_provider(&self, ip: &str) -> Result<i64, sqlx::Error> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM devices WHERE provider = ?1")
                .bind(ip)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }

    /// Update a provider's presence status. Sets presence_changed_at to current timestamp.
    pub async fn update_provider_presence(
        &self,
        id: i64,
        present: bool,
    ) -> Result<Option<crate::models::provider::Provider>, sqlx::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let result = sqlx::query(
            "UPDATE providers SET present = ?1, presence_changed_at = ?2 WHERE id = ?3",
        )
        .bind(present)
        .bind(now)
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }
        self.get_provider(id).await
    }

    // ─── Video Recording Operations (Story 11-2) ───

    /// Insert a new video recording record.
    pub async fn insert_video(
        &self,
        info: &crate::services::video_service::VideoRecordingInfo,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO videos (id, udid, file_path, started_at, stopped_at, frame_count, fps, status, duration_ms, file_size, device_name) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)"
        )
        .bind(&info.id)
        .bind(&info.udid)
        .bind(&info.file_path)
        .bind(&info.started_at)
        .bind(&info.stopped_at)
        .bind(info.frame_count as i64)
        .bind(info.fps as i64)
        .bind(&info.status)
        .bind(info.duration_ms.map(|d| d as i64))
        .bind(info.file_size.map(|s| s as i64))
        .bind(&info.device_name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update an existing video recording record.
    pub async fn update_video(
        &self,
        info: &crate::services::video_service::VideoRecordingInfo,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE videos SET stopped_at = ?1, frame_count = ?2, status = ?3, duration_ms = ?4, file_size = ?5 WHERE id = ?6"
        )
        .bind(&info.stopped_at)
        .bind(info.frame_count as i64)
        .bind(&info.status)
        .bind(info.duration_ms.map(|d| d as i64))
        .bind(info.file_size.map(|s| s as i64))
        .bind(&info.id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a single video recording by ID.
    pub async fn get_video(
        &self,
        id: &str,
    ) -> Result<Option<crate::services::video_service::VideoRecordingInfo>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, udid, file_path, started_at, stopped_at, frame_count, fps, status, duration_ms, file_size, device_name FROM videos WHERE id = ?1"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Self::video_row_to_info(&r)))
    }

    /// List video recordings with optional filters.
    pub async fn list_videos(
        &self,
        udid: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<crate::services::video_service::VideoRecordingInfo>, sqlx::Error> {
        let rows = match (udid, status) {
            (Some(u), Some(s)) => {
                sqlx::query(
                    "SELECT id, udid, file_path, started_at, stopped_at, frame_count, fps, status, duration_ms, file_size, device_name FROM videos WHERE udid = ?1 AND status = ?2 ORDER BY started_at DESC"
                )
                .bind(u)
                .bind(s)
                .fetch_all(&self.pool)
                .await?
            }
            (Some(u), None) => {
                sqlx::query(
                    "SELECT id, udid, file_path, started_at, stopped_at, frame_count, fps, status, duration_ms, file_size, device_name FROM videos WHERE udid = ?1 ORDER BY started_at DESC"
                )
                .bind(u)
                .fetch_all(&self.pool)
                .await?
            }
            (None, Some(s)) => {
                sqlx::query(
                    "SELECT id, udid, file_path, started_at, stopped_at, frame_count, fps, status, duration_ms, file_size, device_name FROM videos WHERE status = ?1 ORDER BY started_at DESC"
                )
                .bind(s)
                .fetch_all(&self.pool)
                .await?
            }
            (None, None) => {
                sqlx::query(
                    "SELECT id, udid, file_path, started_at, stopped_at, frame_count, fps, status, duration_ms, file_size, device_name FROM videos ORDER BY started_at DESC"
                )
                .fetch_all(&self.pool)
                .await?
            }
        };

        Ok(rows.iter().map(|r| Self::video_row_to_info(r)).collect())
    }

    /// Delete a video recording by ID. Returns true if a row was deleted.
    pub async fn delete_video(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM videos WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update status for all videos matching a given status.
    pub async fn update_videos_status(
        &self,
        from_status: &str,
        to_status: &str,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query("UPDATE videos SET status = ?1 WHERE status = ?2")
            .bind(to_status)
            .bind(from_status)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Convert a SQLite row into VideoRecordingInfo.
    fn video_row_to_info(row: &SqliteRow) -> crate::services::video_service::VideoRecordingInfo {
        crate::services::video_service::VideoRecordingInfo {
            id: row.get("id"),
            udid: row.get("udid"),
            file_path: row.get("file_path"),
            started_at: row.get("started_at"),
            stopped_at: row.get("stopped_at"),
            frame_count: row.get::<i64, _>("frame_count") as u64,
            fps: row.get::<i64, _>("fps") as u32,
            status: row.get("status"),
            duration_ms: row.get::<Option<i64>, _>("duration_ms").map(|d| d as u64),
            file_size: row.get::<Option<i64>, _>("file_size").map(|s| s as u64),
            device_name: row.get("device_name"),
        }
    }

    /// List all devices associated with a provider by IP.
    pub async fn list_devices_by_provider(&self, ip: &str) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query("SELECT * FROM devices WHERE provider = ?1 ORDER BY udid")
            .bind(ip)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(Self::device_row_to_json).collect())
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

    // ── Story 11-2: Video CRUD tests ──

    fn test_video_info(id: &str, udid: &str, status: &str) -> crate::services::video_service::VideoRecordingInfo {
        crate::services::video_service::VideoRecordingInfo {
            id: id.to_string(),
            udid: udid.to_string(),
            file_path: format!("recordings/video_{}_{}.mp4", udid, id),
            started_at: "2026-03-11T12:00:00Z".to_string(),
            stopped_at: if status != "recording" { Some("2026-03-11T12:05:00Z".to_string()) } else { None },
            frame_count: 600,
            fps: 2,
            status: status.to_string(),
            duration_ms: Some(300000),
            file_size: Some(1024000),
            device_name: Some("Test Device".to_string()),
        }
    }

    #[tokio::test]
    async fn test_video_insert_and_get() {
        let (_tmp, db) = create_temp_db().await;
        let info = test_video_info("v1", "dev-1", "completed");
        db.insert_video(&info).await.unwrap();

        let retrieved = db.get_video("v1").await.unwrap();
        assert!(retrieved.is_some());
        let r = retrieved.unwrap();
        assert_eq!(r.id, "v1");
        assert_eq!(r.udid, "dev-1");
        assert_eq!(r.status, "completed");
        assert_eq!(r.frame_count, 600);
        assert_eq!(r.fps, 2);
        assert_eq!(r.file_size, Some(1024000));
        assert_eq!(r.device_name, Some("Test Device".to_string()));
    }

    #[tokio::test]
    async fn test_video_update() {
        let (_tmp, db) = create_temp_db().await;
        let mut info = test_video_info("v2", "dev-2", "recording");
        db.insert_video(&info).await.unwrap();

        // Update to completed
        info.status = "completed".to_string();
        info.stopped_at = Some("2026-03-11T12:10:00Z".to_string());
        info.frame_count = 1200;
        info.file_size = Some(2048000);
        db.update_video(&info).await.unwrap();

        let r = db.get_video("v2").await.unwrap().unwrap();
        assert_eq!(r.status, "completed");
        assert_eq!(r.frame_count, 1200);
        assert_eq!(r.file_size, Some(2048000));
        assert!(r.stopped_at.is_some());
    }

    #[tokio::test]
    async fn test_video_delete() {
        let (_tmp, db) = create_temp_db().await;
        let info = test_video_info("v3", "dev-3", "completed");
        db.insert_video(&info).await.unwrap();

        let deleted = db.delete_video("v3").await.unwrap();
        assert!(deleted);

        let retrieved = db.get_video("v3").await.unwrap();
        assert!(retrieved.is_none());

        // Deleting again returns false
        let deleted_again = db.delete_video("v3").await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_video_list_with_filters() {
        let (_tmp, db) = create_temp_db().await;
        db.insert_video(&test_video_info("a1", "dev-A", "completed")).await.unwrap();
        db.insert_video(&test_video_info("a2", "dev-A", "failed")).await.unwrap();
        db.insert_video(&test_video_info("b1", "dev-B", "completed")).await.unwrap();

        // No filter — all 3
        let all = db.list_videos(None, None).await.unwrap();
        assert_eq!(all.len(), 3);

        // Filter by udid
        let dev_a = db.list_videos(Some("dev-A"), None).await.unwrap();
        assert_eq!(dev_a.len(), 2);

        // Filter by status
        let completed = db.list_videos(None, Some("completed")).await.unwrap();
        assert_eq!(completed.len(), 2);

        // Filter by both
        let dev_a_completed = db.list_videos(Some("dev-A"), Some("completed")).await.unwrap();
        assert_eq!(dev_a_completed.len(), 1);
        assert_eq!(dev_a_completed[0].id, "a1");
    }

    #[tokio::test]
    async fn test_video_bulk_status_update() {
        let (_tmp, db) = create_temp_db().await;
        db.insert_video(&test_video_info("r1", "dev-1", "recording")).await.unwrap();
        db.insert_video(&test_video_info("r2", "dev-2", "recording")).await.unwrap();
        db.insert_video(&test_video_info("f1", "dev-3", "finalizing")).await.unwrap();
        db.insert_video(&test_video_info("c1", "dev-4", "completed")).await.unwrap();

        // Mark all "recording" as "failed"
        let count = db.update_videos_status("recording", "failed").await.unwrap();
        assert_eq!(count, 2);

        // Mark all "finalizing" as "failed"
        let count = db.update_videos_status("finalizing", "failed").await.unwrap();
        assert_eq!(count, 1);

        // "completed" should be unaffected
        let completed = db.list_videos(None, Some("completed")).await.unwrap();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].id, "c1");

        // All others should be "failed"
        let failed = db.list_videos(None, Some("failed")).await.unwrap();
        assert_eq!(failed.len(), 3);
    }
}
