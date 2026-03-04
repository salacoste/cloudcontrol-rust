# Shared Code

## Utilities

### Hierarchy Parser

**File:** `src/utils/hierarchy.rs`

**Purpose:** Convert XML UI hierarchy to JSON

```rust
pub fn xml_to_json(xml: &str) -> Result<Value, String> {
    // Parses Android UI hierarchy XML
    // Returns structured JSON for web inspector
}
```

**Input (XML):**
```xml
<hierarchy>
  <node text="Button" bounds="[0,0][100,50]">
    <node text="Label" bounds="[0,0][50,20]"/>
  </node>
</hierarchy>
```

**Output (JSON):**
```json
{
  "tag": "hierarchy",
  "children": [{
    "tag": "node",
    "text": "Button",
    "bounds": [[0,0],[100,50]],
    "children": [...]
  }]
}
```

---

### Host IP

**File:** `src/utils/host_ip.rs`

**Purpose:** Get host machine IP for WiFi connections

```rust
pub fn get_host_ip() -> String {
    // Returns primary network interface IP
    // Used for device connections
}
```

---

## Connection Pool

**File:** `src/pool/connection_pool.rs`

**Purpose:** LRU cache for AtxClient connections

```rust
pub struct ConnectionPool {
    cache: Cache<String, Arc<AtxClient>>,
}

impl ConnectionPool {
    pub fn new(max_size: u64, idle_timeout: Duration) -> Self {
        Cache::builder()
            .max_capacity(max_size)
            .time_to_idle(idle_timeout)
            .build()
    }

    pub async fn get_or_create(&self, udid: &str, ip: &str, port: i64) -> Arc<AtxClient> {
        if let Some(client) = self.cache.get(udid).await {
            return client;
        }
        let client = Arc::new(AtxClient::new(ip, port, udid));
        self.cache.insert(udid.to_string(), client.clone()).await;
        client
    }

    pub async fn remove(&self, udid: &str) {
        self.cache.invalidate(udid).await;
    }
}
```

---

## Batch Processor

**File:** `src/pool/batch_processor.rs`

**Purpose:** Event-driven batch processing

```rust
pub struct BatchProcessor {
    tx: mpsc::Sender<BatchItem>,
}

impl BatchProcessor {
    pub fn new(batch_size: usize, flush_interval: Duration, handlers: HashMap<String, Handler>) -> Self {
        // Spawn processing loop
        // Collects events and processes in batches
    }

    pub async fn submit(&self, event_type: &str, data: Value) -> Result<Value, String> {
        // Submit event for batch processing
        // Returns response via oneshot channel
    }
}
```

---

## Screenshot Cache

**File:** `src/pool/screenshot_cache.rs`

**Purpose:** Request deduplication for screenshots

- Prevents duplicate concurrent screenshot requests
- Caches recent screenshots
- Thread-safe concurrent access

---

## Error Handlers

**File:** `src/error.rs`

**Purpose:** Custom HTTP error pages

```rust
pub fn handle_404<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    // Render 404.html template
}

pub fn handle_500<B>(res: ServiceResponse<B>) -> Result<ErrorHandlerResponse<B>> {
    // Render 500.html template
}
```

---

## ADB Wrapper

**File:** `src/device/adb.rs`

**Purpose:** ADB command execution

### Methods

| Method | Purpose |
|--------|---------|
| `devices()` | List connected devices |
| `forward(serial, port)` | Setup port forwarding |
| `screencap(serial)` | Capture screenshot (PNG) |
| `shell(serial, cmd)` | Execute shell command |
| `is_usb_serial(serial)` | Check if USB device |

---

## ATX Client

**File:** `src/device/atx_client.rs`

**Purpose:** HTTP client for uiautomator2 agent

### Methods

| Method | Purpose |
|--------|---------|
| `screenshot()` | Capture screenshot (JPEG) |
| `screenshot_scaled(scale, quality)` | Scaled screenshot |
| `click(x, y)` | Touch click |
| `swipe(x1, y1, x2, y2, duration)` | Swipe gesture |
| `input_text(text)` | Type text |
| `press_key(key)` | Press key |
| `dump_hierarchy()` | Get UI hierarchy (XML) |
