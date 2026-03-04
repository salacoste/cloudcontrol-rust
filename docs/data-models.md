# Data Models

## Device Model

**Table:** `devices`
**File:** `src/models/device.rs`

### Schema

| Column | Type | JSON Key | Description |
|--------|------|-----------|-------------|
| id | INTEGER | - | Auto-increment primary key |
| udid | TEXT | `udid` | Unique device identifier |
| serial | TEXT | `serial` | Device serial number |
| ip | TEXT | `ip` | Device IP address |
| port | INTEGER | `port` | ATX agent port (default: 9008) |
| present | INTEGER | `present` | Online status (0/1) |
| ready | INTEGER | `ready` | Ready for operations (0/1) |
| using_device | INTEGER | `using` | In-use flag (0/1) |
| is_server | INTEGER | `is_server` | Server device flag (0/1) |
| is_mock | INTEGER | `is_mock` | Mock device flag (0/1) |
| update_time | TEXT | `updateTime` | Last update timestamp |
| model | TEXT | `model` | Device model name |
| brand | TEXT | `brand` | Device brand |
| version | TEXT | `version` | Android version |
| sdk | INTEGER | `sdk` | Android SDK level |
| memory | TEXT | `memory` | Memory info (JSON) |
| cpu | TEXT | `cpu` | CPU info (JSON) |
| battery | TEXT | `battery` | Battery info (JSON) |
| display | TEXT | `display` | Display info (JSON) |
| owner | TEXT | `owner` | Device owner |
| provider | TEXT | `provider` | Device provider |
| agent_version | TEXT | `agentVersion` | ATX agent version |
| hwaddr | TEXT | `hwaddr` | Hardware address |
| created_at | TEXT | `createdAt` | Creation timestamp |
| updated_at | TEXT | `updatedAt` | Update timestamp |
| extra_data | TEXT | - | Additional JSON data |

### Rust Struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Device {
    pub udid: String,
    pub serial: Option<String>,
    pub ip: Option<String>,
    pub port: Option<i64>,
    pub present: bool,
    pub ready: bool,
    #[serde(rename = "using")]
    pub using_device: bool,
    pub is_server: bool,
    pub is_mock: bool,
    pub model: Option<String>,
    pub brand: Option<String>,
    pub version: Option<String>,
    pub sdk: Option<i64>,
    pub memory: Option<Value>,
    pub cpu: Option<Value>,
    pub battery: Option<Value>,
    pub display: Option<Value>,
    pub agent_version: Option<String>,
    #[serde(rename = "agentVersion")]
    pub created_at: Option<String>,
    #[serde(rename = "createdAt")]
    pub updated_at: Option<String>,
    #[serde(rename = "updatedAt")]
    // ...
}
```

### JSON Example

```json
{
  "udid": "abc123",
  "serial": "ABC123",
  "ip": "192.168.1.100",
  "port": 7912,
  "present": true,
  "ready": true,
  "using": false,
  "model": "Pixel 5",
  "brand": "Google",
  "version": "12",
  "sdk": 31,
  "display": {"width": 1080, "height": 1920},
  "memory": {"total": 8192},
  "cpu": {"cores": 8},
  "battery": {"level": 85},
  "agentVersion": "0.10.0"
}
```

---

## InstalledFile Model

**Table:** `installed_file`
**File:** `src/models/file.rs`

### Schema

| Column | Type | JSON Key | Description |
|--------|------|-----------|-------------|
| id | INTEGER | - | Auto-increment primary key |
| group_name | TEXT | `group` | File group/category |
| filename | TEXT | `filename` | File name |
| filesize | INTEGER | `filesize` | File size in bytes |
| upload_time | TEXT | `upload_time` | Upload timestamp |
| who | TEXT | `who` | Uploader name |
| extra_data | TEXT | - | Additional JSON data |

### JSON Example

```json
{
  "group": "0",
  "filename": "app.apk",
  "filesize": 1024,
  "upload_time": "2024-01-01 12:00:00",
  "who": "admin"
}
```

---

## Field Mapping

The database layer maps between JSON API names and SQLite column names:

| JSON Key | SQLite Column | Notes |
|----------|---------------|-------|
| `using` | `using_device` | Boolean as INTEGER |
| `agentVersion` | `agent_version` | CamelCase to snake_case |
| `createdAt` | `created_at` | CamelCase to snake_case |
| `updatedAt` | `updated_at` | CamelCase to snake_case |
| `memory`, `cpu`, `battery`, `display` | TEXT | Stored as JSON strings |

---

## Indexes

```sql
CREATE INDEX idx_devices_udid ON devices(udid);
CREATE INDEX idx_devices_present ON devices(present);
CREATE INDEX idx_files_group ON installed_file(group_name);
```
