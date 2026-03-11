/// OpenAPI 3.0 specification models
/// Generated schema definitions for API v1 endpoints

use serde::{Deserialize, Serialize};

/// OpenAPI 3.0 Document structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenApiDocument {
    pub openapi: String,
    pub info: Info,
    pub servers: Vec<Server>,
    pub paths: std::collections::HashMap<String, PathItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub title: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub url: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<Operation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<Operation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub summary: String,
    pub operation_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<Vec<Parameter>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_body: Option<RequestBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responses: Option<std::collections::HashMap<String, Response>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "in")]
    pub in_location: String,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<Schema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBody {
    pub description: String,
    pub required: bool,
    pub content: std::collections::HashMap<String, MediaType>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaType {
    pub schema: Schema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<std::collections::HashMap<String, MediaType>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Schema {
    #[serde(rename = "type")]
    pub schema_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<std::collections::HashMap<String, Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<Schema>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub example: Option<serde_json::Value>,
}

/// Maximum batch size for batch operations (NFR5)
pub const MAX_BATCH_SIZE: usize = 20;

/// Generate the full OpenAPI 3.0 specification
pub fn generate_openapi_spec() -> OpenApiDocument {
    OpenApiDocument {
        openapi: "3.0.0".to_string(),
        info: Info {
            title: "CloudControl Rust API".to_string(),
            version: "1.0.0".to_string(),
            description: "REST API for Android device control and automation".to_string(),
        },
        servers: vec![Server {
            url: "http://localhost:8000".to_string(),
            description: "Development server".to_string(),
        }],
        paths: generate_paths(),
        components: Some(serde_json::json!({
            "securitySchemes": {
                "ApiKeyHeader": {
                    "type": "apiKey",
                    "in": "header",
                    "name": "Authorization",
                    "description": "API key via Bearer token: 'Bearer <api_key>'"
                },
                "ApiKeyQuery": {
                    "type": "apiKey",
                    "in": "query",
                    "name": "api_key",
                    "description": "API key via query parameter"
                }
            }
        })),
        security: Some(vec![
            serde_json::json!({"ApiKeyHeader": []}),
            serde_json::json!({"ApiKeyQuery": []}),
        ]),
    }
}

fn generate_paths() -> std::collections::HashMap<String, PathItem> {
    let mut paths = std::collections::HashMap::new();

    // GET /api/v1/devices
    paths.insert("/api/v1/devices".to_string(), PathItem {
        get: Some(Operation {
            summary: "List all connected devices".to_string(),
            operation_id: "listDevices".to_string(),
            description: Some("Returns a list of all currently connected devices with their metadata".to_string()),
            parameters: None,
            request_body: None,
            responses: Some(generate_list_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // GET /api/v1/devices/{udid}
    paths.insert("/api/v1/devices/{udid}".to_string(), PathItem {
        get: Some(Operation {
            summary: "Get device information".to_string(),
            operation_id: "getDevice".to_string(),
            description: Some("Returns detailed information about a specific device".to_string()),
            parameters: Some(vec![Parameter {
                name: "udid".to_string(),
                in_location: "path".to_string(),
                required: true,
                description: Some("Unique Device Identifier".to_string()),
                schema: Some(Schema {
                    schema_type: "string".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: None,
                    example: None,
                }),
            }]),
            request_body: None,
            responses: Some(generate_device_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // GET /api/v1/devices/{udid}/screenshot
    paths.insert("/api/v1/devices/{udid}/screenshot".to_string(), PathItem {
        get: Some(Operation {
            summary: "Get device screenshot".to_string(),
            operation_id: "getScreenshot".to_string(),
            description: Some("Returns a base64-encoded screenshot from the device".to_string()),
            parameters: Some(vec![
                Parameter {
                    name: "udid".to_string(),
                    in_location: "path".to_string(),
                    required: true,
                    description: Some("Unique Device Identifier".to_string()),
                    schema: Some(Schema {
                        schema_type: "string".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: None,
                    }),
                },
                Parameter {
                    name: "format".to_string(),
                    in_location: "query".to_string(),
                    required: false,
                    description: Some("Image format: jpeg (default) or png".to_string()),
                    schema: Some(Schema {
                        schema_type: "string".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: Some(serde_json::json!("jpeg")),
                    }),
                },
                Parameter {
                    name: "quality".to_string(),
                    in_location: "query".to_string(),
                    required: false,
                    description: Some("JPEG quality (1-100, default: 50)".to_string()),
                    schema: Some(Schema {
                        schema_type: "integer".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: Some(serde_json::json!(50)),
                    }),
                },
            ]),
            request_body: None,
            responses: Some(generate_screenshot_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // POST /api/v1/devices/{udid}/tap
    paths.insert("/api/v1/devices/{udid}/tap".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Execute tap command".to_string(),
            operation_id: "tap".to_string(),
            description: Some("Executes a tap at the specified coordinates".to_string()),
            parameters: Some(vec![Parameter {
                name: "udid".to_string(),
                in_location: "path".to_string(),
                required: true,
                description: Some("Unique Device Identifier".to_string()),
                schema: Some(Schema {
                    schema_type: "string".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: None,
                    example: None,
                }),
            }]),
            request_body: Some(RequestBody {
                description: "Tap coordinates".to_string(),
                required: true,
                content: generate_tap_request_body(),
            }),
            responses: Some(generate_success_response()),
        }),
        put: None,
        delete: None,
    });

    // POST /api/v1/devices/{udid}/swipe
    paths.insert("/api/v1/devices/{udid}/swipe".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Execute swipe gesture".to_string(),
            operation_id: "swipe".to_string(),
            description: Some("Executes a swipe gesture between two coordinates".to_string()),
            parameters: Some(vec![udid_path_param()]),
            request_body: Some(RequestBody {
                description: "Swipe coordinates and duration".to_string(),
                required: true,
                content: generate_swipe_request_body(),
            }),
            responses: Some(generate_success_response()),
        }),
        put: None,
        delete: None,
    });

    // POST /api/v1/devices/{udid}/input
    paths.insert("/api/v1/devices/{udid}/input".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Send text input".to_string(),
            operation_id: "input".to_string(),
            description: Some("Sends text input to the device, optionally clearing existing text first".to_string()),
            parameters: Some(vec![udid_path_param()]),
            request_body: Some(RequestBody {
                description: "Text input payload".to_string(),
                required: true,
                content: generate_input_request_body(),
            }),
            responses: Some(generate_success_response()),
        }),
        put: None,
        delete: None,
    });

    // POST /api/v1/devices/{udid}/keyevent
    paths.insert("/api/v1/devices/{udid}/keyevent".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Send key event".to_string(),
            operation_id: "keyevent".to_string(),
            description: Some("Sends a physical key event (home, back, enter, volume_up, volume_down, etc.)".to_string()),
            parameters: Some(vec![udid_path_param()]),
            request_body: Some(RequestBody {
                description: "Key event payload".to_string(),
                required: true,
                content: generate_keyevent_request_body(),
            }),
            responses: Some(generate_success_response()),
        }),
        put: None,
        delete: None,
    });

    // GET /api/v1/devices/{udid}/hierarchy (Story 10-4)
    paths.insert("/api/v1/devices/{udid}/hierarchy".to_string(), PathItem {
        get: Some(Operation {
            summary: "Get UI hierarchy".to_string(),
            operation_id: "hierarchy".to_string(),
            description: Some("Returns the device UI hierarchy tree as JSON".to_string()),
            parameters: Some(vec![udid_path_param()]),
            request_body: None,
            responses: Some(generate_success_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // POST /api/v1/devices/{udid}/upload (Story 10-4)
    paths.insert("/api/v1/devices/{udid}/upload".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Upload file to device".to_string(),
            operation_id: "upload".to_string(),
            description: Some("Uploads a file to the device. Images go to /sdcard/DCIM/, videos to /sdcard/Movies/, others to /sdcard/Download/".to_string()),
            parameters: Some(vec![udid_path_param()]),
            request_body: Some(RequestBody {
                description: "File to upload (multipart/form-data)".to_string(),
                required: true,
                content: {
                    let mut c = std::collections::HashMap::new();
                    c.insert("multipart/form-data".to_string(), MediaType {
                        schema: Schema {
                            schema_type: "object".to_string(),
                            properties: Some({
                                let mut p = std::collections::HashMap::new();
                                p.insert("file".to_string(), Schema {
                                    schema_type: "string".to_string(),
                                    properties: None,
                                    items: None,
                                    required: None,
                                    description: Some("File to upload (max 100 MB). Images → /sdcard/DCIM/, videos → /sdcard/Movies/, others → /sdcard/Download/".to_string()),
                                    example: None,
                                });
                                p
                            }),
                            items: None,
                            required: Some(vec!["file".to_string()]),
                            description: None,
                            example: None,
                        },
                    });
                    c
                },
            }),
            responses: Some(generate_success_response()),
        }),
        put: None,
        delete: None,
    });

    // POST /api/v1/devices/{udid}/rotation (Story 10-4)
    paths.insert("/api/v1/devices/{udid}/rotation".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Fix device rotation".to_string(),
            operation_id: "rotation".to_string(),
            description: Some("Triggers a rotation fix via the ATX agent on the device".to_string()),
            parameters: Some(vec![udid_path_param()]),
            request_body: None,
            responses: Some(generate_success_response()),
        }),
        put: None,
        delete: None,
    });

    // GET /api/v1/videos (Story 11-1, enhanced in 11-2)
    paths.insert("/api/v1/videos".to_string(), PathItem {
        get: Some(Operation {
            summary: "List video recordings".to_string(),
            operation_id: "listVideos".to_string(),
            description: Some("Returns all video recordings with metadata. Supports optional query filters.".to_string()),
            parameters: Some(vec![
                Parameter {
                    name: "udid".to_string(),
                    in_location: "query".to_string(),
                    required: false,
                    description: Some("Filter recordings by device UDID".to_string()),
                    schema: Some(Schema {
                        schema_type: "string".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: None,
                    }),
                },
                Parameter {
                    name: "status".to_string(),
                    in_location: "query".to_string(),
                    required: false,
                    description: Some("Filter recordings by status (recording, completed, failed, recovered)".to_string()),
                    schema: Some(Schema {
                        schema_type: "string".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: None,
                    }),
                },
            ]),
            request_body: None,
            responses: Some(generate_success_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // GET/DELETE /api/v1/videos/{id} (Story 11-1)
    let video_id_param = Parameter {
        name: "id".to_string(),
        in_location: "path".to_string(),
        required: true,
        description: Some("Video recording ID".to_string()),
        schema: Some(Schema {
            schema_type: "string".to_string(),
            properties: None,
            items: None,
            required: None,
            description: None,
            example: None,
        }),
    };

    paths.insert("/api/v1/videos/{id}".to_string(), PathItem {
        get: Some(Operation {
            summary: "Get video recording".to_string(),
            operation_id: "getVideo".to_string(),
            description: Some("Returns metadata for a specific video recording".to_string()),
            parameters: Some(vec![video_id_param.clone()]),
            request_body: None,
            responses: Some(generate_success_response()),
        }),
        post: None,
        put: None,
        delete: Some(Operation {
            summary: "Delete video recording".to_string(),
            operation_id: "deleteVideo".to_string(),
            description: Some("Deletes a video recording and its file".to_string()),
            parameters: Some(vec![video_id_param.clone()]),
            request_body: None,
            responses: Some(generate_success_response()),
        }),
    });

    // GET /api/v1/videos/{id}/download (Story 11-1)
    paths.insert("/api/v1/videos/{id}/download".to_string(), PathItem {
        get: Some(Operation {
            summary: "Download video file".to_string(),
            operation_id: "downloadVideo".to_string(),
            description: Some("Downloads the MP4 video file for a completed recording".to_string()),
            parameters: Some(vec![video_id_param.clone()]),
            request_body: None,
            responses: Some({
                let mut r = std::collections::HashMap::new();
                r.insert("200".to_string(), Response {
                    description: "MP4 video file".to_string(),
                    content: Some({
                        let mut c = std::collections::HashMap::new();
                        c.insert("video/mp4".to_string(), MediaType {
                            schema: Schema {
                                schema_type: "string".to_string(),
                                properties: None,
                                items: None,
                                required: None,
                                description: Some("Binary MP4 file".to_string()),
                                example: None,
                            },
                        });
                        c
                    }),
                });
                r
            }),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // POST /api/v1/videos/{id}/stop (Story 11-1)
    paths.insert("/api/v1/videos/{id}/stop".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Stop video recording".to_string(),
            operation_id: "stopVideo".to_string(),
            description: Some("Force-stops an in-progress video recording".to_string()),
            parameters: Some(vec![video_id_param]),
            request_body: None,
            responses: Some(generate_success_response()),
        }),
        put: None,
        delete: None,
    });

    // POST /api/v1/batch/tap
    paths.insert("/api/v1/batch/tap".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Batch tap operation".to_string(),
            operation_id: "batchTap".to_string(),
            description: Some("Executes tap on multiple devices simultaneously".to_string()),
            parameters: None,
            request_body: Some(RequestBody {
                description: "Batch tap request with device list and coordinates".to_string(),
                required: true,
                content: generate_batch_tap_request_body(),
            }),
            responses: Some(generate_batch_response()),
        }),
        put: None,
        delete: None,
    });

    // POST /api/v1/batch/swipe
    paths.insert("/api/v1/batch/swipe".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Batch swipe operation".to_string(),
            operation_id: "batchSwipe".to_string(),
            description: Some("Executes swipe on multiple devices simultaneously".to_string()),
            parameters: None,
            request_body: Some(RequestBody {
                description: "Batch swipe request with device list and coordinates".to_string(),
                required: true,
                content: generate_batch_swipe_request_body(),
            }),
            responses: Some(generate_batch_response()),
        }),
        put: None,
        delete: None,
    });

    // POST /api/v1/batch/input
    paths.insert("/api/v1/batch/input".to_string(), PathItem {
        get: None,
        post: Some(Operation {
            summary: "Batch text input operation".to_string(),
            operation_id: "batchInput".to_string(),
            description: Some("Sends text input to multiple devices simultaneously".to_string()),
            parameters: None,
            request_body: Some(RequestBody {
                description: "Batch input request with device list and text".to_string(),
                required: true,
                content: generate_batch_input_request_body(),
            }),
            responses: Some(generate_batch_response()),
        }),
        put: None,
        delete: None,
    });

    // GET /api/v1/status
    paths.insert("/api/v1/status".to_string(), PathItem {
        get: Some(Operation {
            summary: "Device farm status summary".to_string(),
            operation_id: "getDeviceStatus".to_string(),
            description: Some("Returns a summary of all devices including counts by status and average battery level".to_string()),
            parameters: None,
            request_body: None,
            responses: Some(generate_status_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // GET /api/v1/health
    paths.insert("/api/v1/health".to_string(), PathItem {
        get: Some(Operation {
            summary: "System health check".to_string(),
            operation_id: "healthCheck".to_string(),
            description: Some("Returns system health status. HTTP 200 when healthy, HTTP 503 when unhealthy".to_string()),
            parameters: None,
            request_body: None,
            responses: Some(generate_health_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // GET /api/v1/metrics
    paths.insert("/api/v1/metrics".to_string(), PathItem {
        get: Some(Operation {
            summary: "Prometheus-compatible metrics".to_string(),
            operation_id: "getMetrics".to_string(),
            description: Some("Returns Prometheus-compatible plain text metrics including device counts, WebSocket connections, and screenshot latency".to_string()),
            parameters: None,
            request_body: None,
            responses: Some(generate_metrics_response()),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // GET /api/v1/version
    paths.insert("/api/v1/version".to_string(), PathItem {
        get: Some(Operation {
            summary: "Server version info".to_string(),
            operation_id: "getVersion".to_string(),
            description: Some("Returns server name, version, and identifier for compatibility verification".to_string()),
            parameters: None,
            request_body: None,
            responses: Some({
                let mut r = std::collections::HashMap::new();
                r.insert("200".to_string(), Response {
                    description: "Server version information with name, version, and server identifier".to_string(),
                    content: None,
                });
                r
            }),
        }),
        post: None,
        put: None,
        delete: None,
    });

    // GET /api/v1/ws/nio (WebSocket upgrade)
    paths.insert("/api/v1/ws/nio".to_string(), PathItem {
        get: Some(Operation {
            summary: "JSON-RPC WebSocket Interface".to_string(),
            description: Some("Device-agnostic JSON-RPC 2.0 WebSocket endpoint for real-time automation. Methods: tap, swipe, input, keyevent, batchTap, batchSwipe, batchInput, listDevices, getDevice, screenshot, getStatus.".to_string()),
            operation_id: "wsNio".to_string(),
            parameters: None,
            request_body: None,
            responses: Some({
                let mut r = std::collections::HashMap::new();
                r.insert("101".to_string(), Response {
                    description: "WebSocket upgrade successful — send JSON-RPC 2.0 messages".to_string(),
                    content: None,
                });
                r
            }),
        }),
        post: None,
        put: None,
        delete: None,
    });

    paths
}

/// Reusable UDID path parameter
fn udid_path_param() -> Parameter {
    Parameter {
        name: "udid".to_string(),
        in_location: "path".to_string(),
        required: true,
        description: Some("Unique Device Identifier".to_string()),
        schema: Some(Schema {
            schema_type: "string".to_string(),
            properties: None,
            items: None,
            required: None,
            description: None,
            example: None,
        }),
    }
}

fn rate_limit_response() -> Response {
    Response {
        description: "Rate limit exceeded. Retry after the number of seconds in the Retry-After header.".to_string(),
        content: None,
    }
}

fn generate_list_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "List of devices".to_string(),
        content: Some({
            let mut content = std::collections::HashMap::new();
            content.insert("application/json".to_string(), MediaType {
                schema: Schema {
                    schema_type: "object".to_string(),
                    properties: Some({
                        let mut props = std::collections::HashMap::new();
                        props.insert("status".to_string(), Schema {
                            schema_type: "string".to_string(),
                            properties: None,
                            items: None,
                            required: None,
                            description: None,
                            example: Some(serde_json::json!("success")),
                        });
                        props.insert("data".to_string(), Schema {
                            schema_type: "array".to_string(),
                            properties: None,
                            items: Some(Box::new(Schema {
                                schema_type: "object".to_string(),
                                properties: Some({
                                    let mut device_props = std::collections::HashMap::new();
                                    device_props.insert("udid".to_string(), Schema {
                                        schema_type: "string".to_string(),
                                        properties: None,
                                        items: None,
                                        required: None,
                                        description: None,
                                        example: None,
                                    });
                                    device_props.insert("model".to_string(), Schema {
                                        schema_type: "string".to_string(),
                                        properties: None,
                                        items: None,
                                        required: None,
                                        description: None,
                                        example: None,
                                    });
                                    device_props.insert("status".to_string(), Schema {
                                        schema_type: "string".to_string(),
                                        properties: None,
                                        items: None,
                                        required: None,
                                        description: None,
                                        example: None,
                                    });
                                    device_props
                                }),
                                items: None,
                                required: None,
                                description: None,
                                example: None,
                            })),
                            required: None,
                            description: None,
                            example: None,
                        });
                        props.insert("timestamp".to_string(), Schema {
                            schema_type: "string".to_string(),
                            properties: None,
                            items: None,
                            required: None,
                            description: None,
                            example: None,
                        });
                        props
                    }),
                    items: None,
                    required: Some(vec!["status".to_string(), "data".to_string(), "timestamp".to_string()]),
                    description: None,
                    example: None,
                },
            });
            content
        }),
    });
    responses
}

fn generate_device_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "Device information".to_string(),
        content: None,
    });
    responses.insert("404".to_string(), Response {
        description: "Device not found".to_string(),
        content: None,
    });
    responses
}

fn generate_screenshot_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "Screenshot data".to_string(),
        content: None,
    });
    responses
}

fn generate_success_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "Operation successful".to_string(),
        content: None,
    });
    responses.insert("404".to_string(), Response {
        description: "Device not found".to_string(),
        content: None,
    });
    responses
}

fn generate_tap_request_body() -> std::collections::HashMap<String, MediaType> {
    let mut content = std::collections::HashMap::new();
    content.insert("application/json".to_string(), MediaType {
        schema: Schema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = std::collections::HashMap::new();
                props.insert("x".to_string(), Schema {
                    schema_type: "integer".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("X coordinate".to_string()),
                    example: Some(serde_json::json!(540)),
                });
                props.insert("y".to_string(), Schema {
                    schema_type: "integer".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Y coordinate".to_string()),
                    example: Some(serde_json::json!(960)),
                });
                props
            }),
            items: None,
            required: Some(vec!["x".to_string(), "y".to_string()]),
            description: None,
            example: Some(serde_json::json!({"x": 540, "y": 960})),
        },
    });
    content
}

fn generate_batch_tap_request_body() -> std::collections::HashMap<String, MediaType> {
    let mut content = std::collections::HashMap::new();
    content.insert("application/json".to_string(), MediaType {
        schema: Schema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = std::collections::HashMap::new();
                props.insert("udids".to_string(), Schema {
                    schema_type: "array".to_string(),
                    properties: None,
                    items: Some(Box::new(Schema {
                        schema_type: "string".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: None,
                    })),
                    required: None,
                    description: Some("List of device UDIDs".to_string()),
                    example: None,
                });
                props.insert("x".to_string(), Schema {
                    schema_type: "integer".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("X coordinate".to_string()),
                    example: None,
                });
                props.insert("y".to_string(), Schema {
                    schema_type: "integer".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Y coordinate".to_string()),
                    example: None,
                });
                props
            }),
            items: None,
            required: Some(vec!["udids".to_string(), "x".to_string(), "y".to_string()]),
            description: None,
            example: Some(serde_json::json!({"udids": ["device1", "device2"], "x": 100, "y": 200})),
        },
    });
    content
}

fn generate_batch_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "Batch operation result".to_string(),
        content: None,
    });
    responses
}

fn generate_swipe_request_body() -> std::collections::HashMap<String, MediaType> {
    let mut content = std::collections::HashMap::new();
    content.insert("application/json".to_string(), MediaType {
        schema: Schema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = std::collections::HashMap::new();
                for (name, desc) in [("x1", "Start X coordinate"), ("y1", "Start Y coordinate"),
                                     ("x2", "End X coordinate"), ("y2", "End Y coordinate")] {
                    props.insert(name.to_string(), Schema {
                        schema_type: "integer".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: Some(desc.to_string()),
                        example: None,
                    });
                }
                props.insert("duration".to_string(), Schema {
                    schema_type: "number".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Swipe duration in seconds (default: 0.3)".to_string()),
                    example: Some(serde_json::json!(0.3)),
                });
                props
            }),
            items: None,
            required: Some(vec!["x1".to_string(), "y1".to_string(), "x2".to_string(), "y2".to_string()]),
            description: None,
            example: Some(serde_json::json!({"x1": 500, "y1": 1500, "x2": 500, "y2": 500, "duration": 0.3})),
        },
    });
    content
}

fn generate_input_request_body() -> std::collections::HashMap<String, MediaType> {
    let mut content = std::collections::HashMap::new();
    content.insert("application/json".to_string(), MediaType {
        schema: Schema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = std::collections::HashMap::new();
                props.insert("text".to_string(), Schema {
                    schema_type: "string".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Text to input".to_string()),
                    example: Some(serde_json::json!("hello world")),
                });
                props.insert("clear".to_string(), Schema {
                    schema_type: "boolean".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Clear existing text before input (default: false)".to_string()),
                    example: Some(serde_json::json!(false)),
                });
                props
            }),
            items: None,
            required: Some(vec!["text".to_string()]),
            description: None,
            example: Some(serde_json::json!({"text": "hello world", "clear": true})),
        },
    });
    content
}

fn generate_keyevent_request_body() -> std::collections::HashMap<String, MediaType> {
    let mut content = std::collections::HashMap::new();
    content.insert("application/json".to_string(), MediaType {
        schema: Schema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = std::collections::HashMap::new();
                props.insert("key".to_string(), Schema {
                    schema_type: "string".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Key name: home, back, enter, recent, power, volume_up, volume_down".to_string()),
                    example: Some(serde_json::json!("home")),
                });
                props
            }),
            items: None,
            required: Some(vec!["key".to_string()]),
            description: None,
            example: Some(serde_json::json!({"key": "home"})),
        },
    });
    content
}

fn generate_batch_swipe_request_body() -> std::collections::HashMap<String, MediaType> {
    let mut content = std::collections::HashMap::new();
    content.insert("application/json".to_string(), MediaType {
        schema: Schema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = std::collections::HashMap::new();
                props.insert("udids".to_string(), Schema {
                    schema_type: "array".to_string(),
                    properties: None,
                    items: Some(Box::new(Schema {
                        schema_type: "string".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: None,
                    })),
                    required: None,
                    description: Some("List of device UDIDs".to_string()),
                    example: None,
                });
                for (name, desc) in [("x1", "Start X coordinate"), ("y1", "Start Y coordinate"),
                                     ("x2", "End X coordinate"), ("y2", "End Y coordinate")] {
                    props.insert(name.to_string(), Schema {
                        schema_type: "integer".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: Some(desc.to_string()),
                        example: None,
                    });
                }
                props.insert("duration".to_string(), Schema {
                    schema_type: "number".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Swipe duration in seconds (default: 0.3)".to_string()),
                    example: None,
                });
                props
            }),
            items: None,
            required: Some(vec!["udids".to_string(), "x1".to_string(), "y1".to_string(), "x2".to_string(), "y2".to_string()]),
            description: None,
            example: Some(serde_json::json!({"udids": ["device1"], "x1": 500, "y1": 1500, "x2": 500, "y2": 500, "duration": 0.3})),
        },
    });
    content
}

fn generate_batch_input_request_body() -> std::collections::HashMap<String, MediaType> {
    let mut content = std::collections::HashMap::new();
    content.insert("application/json".to_string(), MediaType {
        schema: Schema {
            schema_type: "object".to_string(),
            properties: Some({
                let mut props = std::collections::HashMap::new();
                props.insert("udids".to_string(), Schema {
                    schema_type: "array".to_string(),
                    properties: None,
                    items: Some(Box::new(Schema {
                        schema_type: "string".to_string(),
                        properties: None,
                        items: None,
                        required: None,
                        description: None,
                        example: None,
                    })),
                    required: None,
                    description: Some("List of device UDIDs".to_string()),
                    example: None,
                });
                props.insert("text".to_string(), Schema {
                    schema_type: "string".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Text to input".to_string()),
                    example: None,
                });
                props.insert("clear".to_string(), Schema {
                    schema_type: "boolean".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Clear existing text before input (default: false)".to_string()),
                    example: None,
                });
                props
            }),
            items: None,
            required: Some(vec!["udids".to_string(), "text".to_string()]),
            description: None,
            example: Some(serde_json::json!({"udids": ["device1"], "text": "hello", "clear": false})),
        },
    });
    content
}

fn generate_status_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "Device farm status summary with device counts by status and average battery".to_string(),
        content: None,
    });
    responses
}

fn generate_health_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "System is healthy".to_string(),
        content: None,
    });
    responses.insert("503".to_string(), Response {
        description: "System is unhealthy (database or connection pool issues)".to_string(),
        content: None,
    });
    responses
}

fn generate_metrics_response() -> std::collections::HashMap<String, Response> {
    let mut responses = std::collections::HashMap::new();
    responses.insert("429".to_string(), rate_limit_response());
    responses.insert("200".to_string(), Response {
        description: "Prometheus-compatible plain text metrics".to_string(),
        content: Some({
            let mut content = std::collections::HashMap::new();
            content.insert("text/plain".to_string(), MediaType {
                schema: Schema {
                    schema_type: "string".to_string(),
                    properties: None,
                    items: None,
                    required: None,
                    description: Some("Prometheus exposition format".to_string()),
                    example: None,
                },
            });
            content
        }),
    });
    responses
}
