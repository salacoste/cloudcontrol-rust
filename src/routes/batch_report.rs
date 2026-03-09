use crate::state::AppState;
use actix_web::{web, HttpResponse};
use serde_json::json;
use std::collections::HashMap;

/// GET /api/batch/reports - List all batch reports
pub async fn list_batch_reports(
    state: web::Data<AppState>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let operation_type = query.get("operation_type").map(|s| s.as_str());

    match state.db.list_batch_reports(operation_type).await {
        Ok(reports) => HttpResponse::Ok().json(json!({
            "status": "success",
            "reports": reports
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_LIST_REPORTS_FAILED",
            "message": e.to_string()
        })),
    }
}

/// GET /api/batch/reports/{id} - Get a batch report
pub async fn get_batch_report(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    query: web::Query<HashMap<String, String>>,
) -> HttpResponse {
    let report_id = path.into_inner();
    let format = query.get("format").map(|s| s.as_str()).unwrap_or("json");

    match state.db.get_batch_report_with_results(report_id).await {
        Ok(Some(report)) => {
            match format {
                "csv" => generate_csv_report(&report),
                "html" => generate_html_report(&report),
                _ => HttpResponse::Ok().json(report),
            }
        },
        Ok(None) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_REPORT_NOT_FOUND",
            "message": format!("Batch report {} not found", report_id)
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_GET_REPORT_FAILED",
            "message": e.to_string()
        })),
    }
}

/// DELETE /api/batch/reports/{id} - Delete a batch report
pub async fn delete_batch_report(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> HttpResponse {
    let report_id = path.into_inner();

    match state.db.delete_batch_report(report_id).await {
        Ok(true) => HttpResponse::Ok().json(json!({
            "status": "success",
            "message": format!("Batch report {} deleted", report_id)
        })),
        Ok(false) => HttpResponse::NotFound().json(json!({
            "status": "error",
            "error": "ERR_REPORT_NOT_FOUND",
            "message": format!("Batch report {} not found", report_id)
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "error": "ERR_DELETE_REPORT_FAILED",
            "message": e.to_string()
        })),
    }
}

/// Generate CSV format report
fn generate_csv_report(report: &serde_json::Value) -> HttpResponse {
    let mut csv = String::from("Device UDID,Status,Error Code,Error Message,Duration (ms)\n");

    if let Some(results) = report.get("results").and_then(|r| r.as_array()) {
        for result in results {
            let udid = result.get("udid").and_then(|v| v.as_str()).unwrap_or("");
            let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let error_code = result.get("error_code").and_then(|v| v.as_str()).unwrap_or("");
            let error_message = result.get("error_message").and_then(|v| v.as_str()).unwrap_or("");
            let duration_ms = result.get("duration_ms").and_then(|v| v.as_i64()).unwrap_or(0);

            csv.push_str(&format!(
                "{},{},{},{},{}\n",
                escape_csv_field(udid),
                escape_csv_field(status),
                escape_csv_field(error_code),
                escape_csv_field(error_message),
                duration_ms
            ));
        }
    }

    HttpResponse::Ok()
        .content_type("text/csv")
        .insert_header(("Content-Disposition", "attachment; filename=\"batch_report.csv\""))
        .body(csv)
}

/// Escape a CSV field
fn escape_csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace("\"", "\"\""))
    } else {
        s.to_string()
    }
}

/// Generate HTML format report
fn generate_html_report(report: &serde_json::Value) -> HttpResponse {
    let operation_type = report.get("operation_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let created_at = report.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");
    let total_devices = report.get("total_devices").and_then(|v| v.as_i64()).unwrap_or(0);
    let successful = report.get("successful").and_then(|v| v.as_i64()).unwrap_or(0);
    let failed = report.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

    let mut results_rows = String::new();
    if let Some(results) = report.get("results").and_then(|r| r.as_array()) {
        for result in results {
            let udid = result.get("udid").and_then(|v| v.as_str()).unwrap_or("");
            let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let error_code = result.get("error_code").and_then(|v| v.as_str()).unwrap_or("");
            let error_message = result.get("error_message").and_then(|v| v.as_str()).unwrap_or("");
            let duration_ms = result.get("duration_ms").and_then(|v| v.as_i64()).unwrap_or(0);

            let status_class = match status {
                "success" => "status-success",
                "failed" => "status-failed",
                _ => "status-unknown",
            };

            results_rows.push_str(&format!(
                r#"<tr>
                    <td>{}</td>
                    <td class="{}">{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{} ms</td>
                </tr>"#,
                udid,
                status_class,
                status,
                error_code,
                error_message,
                duration_ms
            ));
        }
    }

    let html = format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Batch Report - {}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background: #1a1a1a; color: #e5e5e5; }}
        .container {{ max-width: 1000px; margin: 0 auto; }}
        h1 {{ color: #fff; margin-bottom: 10px; }}
        .summary {{ display: flex; gap: 20px; margin-bottom: 20px; }}
        .stat {{ background: #2a2a2a; padding: 15px; border-radius: 8px; text-align: center; }}
        .stat-value {{ font-size: 24px; font-weight: bold; color: #fff; }}
        .stat-label {{ font-size: 12px; color: #888; }}
        .stat-success .stat-value {{ color: #4ade80; }}
        .stat-failed .stat-value {{ color: #ef4444; }}
        table {{ width: 100%; border-collapse: collapse; background: #2a2a2a; }}
        th, td {{ padding: 12px; text-align: left; border-bottom: 1px solid #3a3a3a; }}
        th {{ background: #333; font-weight: bold; color: #fff; }}
        .status-success {{ color: #4ade80; }}
        .status-failed {{ color: #ef4444; }}
        .status-unknown {{ color: #888; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Batch Report</h1>
        <div class="summary">
            <div class="stat">
                <div class="stat-value">{}</div>
                <div class="stat-label">Total Devices</div>
            </div>
            <div class="stat stat-success">
                <div class="stat-value">{}</div>
                <div class="stat-label">Successful</div>
            </div>
            <div class="stat stat-failed">
                <div class="stat-value">{}</div>
                <div class="stat-label">Failed</div>
            </div>
        </div>
        <table>
            <thead>
                <tr>
                    <th>Device UDID</th>
                    <th>Status</th>
                    <th>Error Code</th>
                    <th>Error Message</th>
                    <th>Duration</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>
        <p style="margin-top: 20px; color: #666; font-size: 12px;">Generated: {}</p>
    </div>
</body>
</html>"#,
        operation_type,
        total_devices,
        successful,
        failed,
        results_rows,
        created_at
    );

    HttpResponse::Ok()
        .content_type("text/html")
        .insert_header(("Content-Disposition", "attachment; filename=\"batch_report.html\""))
        .body(html)
}
