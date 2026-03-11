//! Device Resolution Service
//!
//! Shared module for device client resolution and connection handling.
//! Extracted from control.rs and api_v1.rs to eliminate code duplication (Story 13-1).
//!
//! This module provides:
//! - `DeviceResolver`: Service for resolving device connections and getting ATX clients
//! - `DeviceError`: Error enum for device resolution failures

use crate::device::adb::Adb;
use crate::device::atx_client::AtxClient;
use crate::pool::connection_pool::ConnectionPool;
use crate::services::phone_service::PhoneService;
use crate::state::AppState;
use actix_web::HttpResponse;
use moka::future::Cache;
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;

// ═════════════════════════════════════════════════════════════════════════════
// Error Types
// ═════════════════════════════════════════════════════════════════════════════

/// Device resolution error types
#[derive(Debug)]
pub enum DeviceError {
    /// Device not found in database
    NotFound(String),
    /// Device is disconnected or unreachable
    Disconnected(String),
    /// Database query failed
    QueryFailed(String),
    /// Cache operation failed
    CacheError(String),
}

impl DeviceError {
    /// Get error code for API responses
    pub fn error_code(&self) -> &'static str {
        match self {
            DeviceError::NotFound(_) => "ERR_DEVICE_NOT_FOUND",
            DeviceError::Disconnected(_) => "ERR_DEVICE_DISCONNECTED",
            DeviceError::QueryFailed(_) => "ERR_DEVICE_QUERY_FAILED",
            DeviceError::CacheError(_) => "ERR_CACHE_ERROR",
        }
    }

    /// Get error message
    pub fn message(&self) -> &str {
        match self {
            DeviceError::NotFound(msg) => msg,
            DeviceError::Disconnected(msg) => msg,
            DeviceError::QueryFailed(msg) => msg,
            DeviceError::CacheError(msg) => msg,
        }
    }
}

impl From<DeviceError> for HttpResponse {
    fn from(err: DeviceError) -> HttpResponse {
        match err {
            DeviceError::NotFound(msg) => HttpResponse::NotFound().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_NOT_FOUND",
                "message": msg
            })),
            DeviceError::Disconnected(msg) => HttpResponse::ServiceUnavailable().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_DISCONNECTED",
                "message": msg
            })),
            DeviceError::QueryFailed(msg) => HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_DEVICE_QUERY_FAILED",
                "message": msg
            })),
            DeviceError::CacheError(msg) => HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "error": "ERR_CACHE_ERROR",
                "message": msg
            })),
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Device Resolver
// ═════════════════════════════════════════════════════════════════════════════

/// Service for resolving device connections and getting ATX clients.
///
/// This struct encapsulates the shared logic for:
/// 1. Checking device info cache
/// 2. Falling back to database queries
/// 3. Resolving WiFi vs USB connections
/// 4. Getting or creating ATX clients from the connection pool
///
/// # Example
///
/// ```ignore
/// let resolver = DeviceResolver::new(&state);
/// let (device_info, client) = resolver.get_device_client(udid).await?;
/// ```
pub struct DeviceResolver<'a> {
    device_info_cache: &'a Cache<String, Value>,
    connection_pool: &'a Arc<ConnectionPool>,
    phone_service: &'a PhoneService,
}

impl<'a> DeviceResolver<'a> {
    /// Create a new DeviceResolver from AppState
    pub fn new(state: &'a AppState) -> Self {
        Self {
            device_info_cache: &state.device_info_cache,
            connection_pool: &state.connection_pool,
            phone_service: &state.phone_service,
        }
    }

    /// Get device info and ATX client for a device UDID.
    ///
    /// This method:
    /// 1. Checks the device info cache first
    /// 2. Falls back to database query if not cached
    /// 3. Resolves the connection (WiFi vs USB)
    /// 4. Gets or creates an ATX client from the connection pool
    ///
    /// # Arguments
    ///
    /// * `udid` - Device UDID to look up
    ///
    /// # Returns
    ///
    /// * `Ok((Value, Arc<AtxClient>))` - Device info JSON and ATX client
    /// * `Err(DeviceError)` - Resolution failure
    pub async fn get_device_client(
        &self,
        udid: &str,
    ) -> Result<(Value, Arc<AtxClient>), DeviceError> {
        // Try device info cache first
        if let Some(cached) = self.device_info_cache.get(udid).await {
            let ip = cached.get("ip").and_then(|v| v.as_str()).unwrap_or("");
            let port = cached.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
            let (final_ip, final_port) = self.resolve_device_connection(&cached, ip, port).await;
            let client = self.connection_pool.get_or_create(udid, &final_ip, final_port).await;
            return Ok((cached, client));
        }

        // Fall back to database query
        let device = self.phone_service
            .query_info_by_udid(udid)
            .await
            .map_err(|e| {
                if e.contains("not found") {
                    DeviceError::NotFound(e)
                } else if e.contains("disconnected") || e.contains("unreachable") {
                    DeviceError::Disconnected(e)
                } else {
                    DeviceError::QueryFailed(e)
                }
            })?
            .ok_or_else(|| DeviceError::NotFound(format!("Device not found: {}", udid)))?;

        // Cache the result
        self.device_info_cache.insert(udid.to_string(), device.clone()).await;

        // Resolve connection and get client
        let ip = device.get("ip").and_then(|v| v.as_str()).unwrap_or("");
        let port = device.get("port").and_then(|v| v.as_i64()).unwrap_or(9008);
        let (final_ip, final_port) = self.resolve_device_connection(&device, ip, port).await;
        let client = self.connection_pool.get_or_create(udid, &final_ip, final_port).await;
        Ok((device, client))
    }

    /// Resolve device connection details (WiFi vs USB).
    ///
    /// For USB-connected devices (IP 127.0.0.1, port 9008), this method
    /// sets up ADB port forwarding and returns the local forwarded port.
    ///
    /// For WiFi-connected devices, returns the IP and port directly.
    ///
    /// # Arguments
    ///
    /// * `device` - Device info JSON
    /// * `ip` - Device IP address
    /// * `port` - Device port
    ///
    /// # Returns
    ///
    /// Tuple of (final_ip, final_port) to connect to
    pub async fn resolve_device_connection(
        &self,
        device: &Value,
        ip: &str,
        port: i64,
    ) -> (String, i64) {
        // WiFi connection - use directly
        if !ip.is_empty() && ip != "127.0.0.1" {
            return (ip.to_string(), port);
        }

        // Already forwarded port
        if ip == "127.0.0.1" && port != 9008 {
            return (ip.to_string(), port);
        }

        // USB connection - need ADB port forwarding
        let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");
        if !serial.is_empty() {
            if let Ok(local_port) = Adb::forward(serial, 9008).await {
                return ("127.0.0.1".to_string(), local_port as i64);
            }
        }

        (ip.to_string(), port)
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Standalone Helper Functions
// ═════════════════════════════════════════════════════════════════════════════

/// Resolve device connection without a DeviceResolver instance.
///
/// This is a convenience function for cases where you have device info
/// but don't need the full resolver.
///
/// # Arguments
///
/// * `device` - Device info JSON
/// * `ip` - Device IP address
/// * `port` - Device port
///
/// # Returns
///
/// Tuple of (final_ip, final_port) to connect to
pub async fn resolve_device_connection(device: &Value, ip: &str, port: i64) -> (String, i64) {
    // WiFi connection - use directly
    if !ip.is_empty() && ip != "127.0.0.1" {
        return (ip.to_string(), port);
    }

    // Already forwarded port
    if ip == "127.0.0.1" && port != 9008 {
        return (ip.to_string(), port);
    }

    // USB connection - need ADB port forwarding
    let serial = device.get("serial").and_then(|v| v.as_str()).unwrap_or("");
    if !serial.is_empty() {
        if let Ok(local_port) = Adb::forward(serial, 9008).await {
            return ("127.0.0.1".to_string(), local_port as i64);
        }
    }

    (ip.to_string(), port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_error_error_code() {
        assert_eq!(DeviceError::NotFound("test".into()).error_code(), "ERR_DEVICE_NOT_FOUND");
        assert_eq!(DeviceError::Disconnected("test".into()).error_code(), "ERR_DEVICE_DISCONNECTED");
        assert_eq!(DeviceError::QueryFailed("test".into()).error_code(), "ERR_DEVICE_QUERY_FAILED");
        assert_eq!(DeviceError::CacheError("test".into()).error_code(), "ERR_CACHE_ERROR");
    }

    #[test]
    fn test_device_error_message() {
        assert_eq!(DeviceError::NotFound("device missing".into()).message(), "device missing");
        assert_eq!(DeviceError::CacheError("cache failed".into()).message(), "cache failed");
    }
}
