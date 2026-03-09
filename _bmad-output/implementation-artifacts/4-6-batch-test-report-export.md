# Story 4.6: Batch Test Report Export

Epic: 4 (Multi-Device Batch Operations)
Status: done
Story: 4-5-recording-playback (completed)
Priority: P2

## Story

As a **QA Engineer**, I want to export batch test reports with per-device results, so that I can document testing outcomes.

## Acceptance Criteria

```gherkin
Feature: Export basic test report
  Given a batch operation completed on 5 devices
  And 4 succeeded, 1 failed
  When I click "Export Report"
  Then a JSON report is generated
  And the report includes timestamp, operation type, and device results
  And each device result includes UDID, status, and duration

Feature: Export report with screenshots
  Given a batch operation completed with screenshots
  When I export the report with "Include Screenshots"
  Then the report includes base64-encoded screenshots
  Or screenshots are bundled as separate files in a ZIP

Feature: Export report in multiple formats
  Given a batch operation completed
  When I select "Export as CSV"
  Then a CSV file is generated with tabular results
  When I select "Export as HTML"
  Then an HTML report with styling is generated

Feature: Per-device error details in report
  Given a batch operation had partial failures
  When the report is generated
  Then failed devices include error messages
  And stack traces are included if available
  And error codes are included for programmatic parsing
```

## Tasks/Subtasks

- [x] Task 1: Create batch report data model
  - [x] Add BatchReport struct with metadata
  - [x] Add DeviceResult struct with status/error info
  - [x] Add ReportFormat enum (JSON, CSV, HTML)

- [x] Task 2: Add batch operation tracking
  - [x] Create BatchOperationTracker to store operation results
  - [x] Track operation type, timestamps, device results
  - [x] Store in SQLite for persistence

- [x] Task 3: Add report export API endpoints
  - [x] GET /api/batch/reports - List all batch reports
  - [x] GET /api/batch/reports/{id} - Get report in JSON
  - [x] GET /api/batch/reports/{id}?format=csv - Get as CSV
  - [x] GET /api/batch/reports/{id}?format=html - Get as HTML
  - [x] DELETE /api/batch/reports/{id} - Delete report

- [x] Task 4: Implement report formatters
  - [x] JSON formatter with full details
  - [x] CSV formatter with tabular data
  - [x] HTML formatter with styling

- [x] Task 5: Write E2E tests for report export
  - [x] Test JSON report generation
  - [x] Test CSV report generation
  - [x] Test HTML report generation
  - [x] Test error details in reports

## Dev Notes

### Architecture Context
This story adds report export capabilities to batch operations. The batch operations from Stories 4-1 and 4-2 already execute operations on multiple devices - this story adds the ability to track results and export them as reports.

### Implementation Pattern
- Create a `BatchReport` model to track batch operation results
- Extend existing batch endpoints to record results
- Add new endpoints for report generation
- Support multiple output formats via query parameter

### Implementation Summary
- Added `src/models/batch_report.rs` with BatchReport, DeviceOperationResult, DeviceOperationStatus, BatchOperationType structs
- Extended `src/db/sqlite.rs` with batch_reports and batch_report_results tables
- Added database methods: create_batch_report, complete_batch_report, add_batch_report_result, get_batch_report, get_batch_report_results, get_batch_report_with_results, list_batch_reports, delete_batch_report
- Added `src/routes/batch_report.rs` with list_batch_reports, get_batch_report, delete_batch_report endpoints
- Added CSV and HTML formatters for report export
- Registered routes in `src/main.rs`
- Added routes to test setup macro
- Tests pass: 143 total tests

### References
- [Source: src/routes/batch_report.rs](./routes/batch_report.rs) - Batch report routes
- [Source: src/models/batch_report.rs](./models/batch_report.rs) - Batch report models
- [Source: src/db/sqlite.rs](./db/sqlite.rs) - Database schema
- [Source: _bmad-output/implementation-artifacts/4-1-multi-device-selection-ui.md] - Previous story
- [Source: _bmad-output/implementation-artifacts/4-2-synchronized-batch-operations.md] - Previous story
