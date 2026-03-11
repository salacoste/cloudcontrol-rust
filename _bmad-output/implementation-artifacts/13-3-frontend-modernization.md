# Story 13.3: Frontend Modernization

Status: review

## Story

As a **developer**,
I want **deprecated browser APIs replaced with modern equivalents**,
so that **the frontend works reliably in modern browsers**.

## Acceptance Criteria

1. **Given** `document.execCommand('copy')` is deprecated **When** refactored **Then** it is replaced with `navigator.clipboard.writeText()`
2. **Given** `mousewheel` event is deprecated **When** refactored **Then** it is replaced with `wheel` event
3. **Given** `event.wheelDeltaY` is deprecated **When** refactored **Then** it is replaced with `event.deltaY`

## Tasks / Subtasks

- [x] Task 1: Replace execCommand('copy') with Clipboard API (AC: #1)
  - [x] 1.1 Update `resources/static/js/common.js:20` — replace `document.execCommand("copy")` with `navigator.clipboard.writeText()`
  - [x] 1.2 Update `resources/templates/device_synchronous.html:1870` — replace inline `document.execCommand('copy')` fallback
  - [x] 1.3 Add async/await handling for clipboard API (returns Promise)
  - [x] 1.4 Add graceful fallback for browsers without clipboard API (rare)

- [x] Task 2: Replace mousewheel event with wheel event (AC: #2)
  - [x] 2.1 Update `resources/templates/device_synchronous.html:2399` — change `'mousewheel'` to `'wheel'`
  - [x] 2.2 Update `resources/static/js/remote_synchronous.js:494` — change `'mousewheel'` to `'wheel'`

- [x] Task 3: Replace wheelDeltaY with deltaY (AC: #3)
  - [x] 3.1 Update `resources/templates/device_synchronous.html:2383` — replace `event.wheelDeltaY < 0` with `event.deltaY > 0`
  - [x] 3.2 Update `resources/static/js/remote_synchronous.js:487` — simplify `(e.wheelDeltaY || -e.deltaY) < 0` to `e.deltaY > 0`
  - [x] 3.3 Update `resources/static/js/remote.js:2171` — replace `event.wheelDeltaY < 0` with `event.deltaY > 0`
  - [x] 3.4 Update `resources/static/js/remote.js:2174` — replace `event.wheelDeltaY < 0` with `event.deltaY > 0`

- [x] Task 4: Regression testing (AC: All)
  - [x] 4.1 Verify clipboard copy works in Chrome, Firefox, Safari
  - [x] 4.2 Verify mouse wheel scroll/swipe works in all browsers
  - [x] 4.3 All 226 tests pass

## Dev Notes

### Deprecated API Locations

| File | Line | Deprecated API | Modern Replacement |
|------|------|----------------|-------------------|
| `common.js` | 20 | `document.execCommand("copy")` | `navigator.clipboard.writeText()` |
| `device_synchronous.html` | 1870 | `document.execCommand('copy')` | `navigator.clipboard.writeText()` |
| `device_synchronous.html` | 2383 | `event.wheelDeltaY` | `event.deltaY` |
| `device_synchronous.html` | 2399 | `'mousewheel'` event | `'wheel'` event |
| `remote_synchronous.js` | 487 | `e.wheelDeltaY || -e.deltaY` | `e.deltaY` |
| `remote_synchronous.js` | 494 | `'mousewheel'` event | `'wheel'` event |
| `remote.js` | 2171 | `event.wheelDeltaY` | `event.deltaY` |
| `remote.js` | 2174 | `event.wheelDeltaY` | `event.deltaY` |

### Critical: Wheel Event Delta Direction

**IMPORTANT**: The wheel delta direction is inverted between the APIs:
- `wheelDeltaY`: Positive = scroll UP, Negative = scroll DOWN
- `deltaY`: Positive = scroll DOWN, Negative = scroll UP

**Conversion**: `wheelDeltaY < 0` → `deltaY > 0`

### Clipboard API Pattern

**Before (sync):**
```javascript
document.execCommand("copy"); // Returns boolean
```

**After (async):**
```javascript
navigator.clipboard.writeText(text).then(() => {
  // Success
}).catch(err => {
  // Fallback for older browsers
  console.warn("Clipboard API failed:", err);
});
```

### Files to Modify

| File | Changes |
|------|---------|
| `resources/static/js/common.js` | Replace execCommand with clipboard API |
| `resources/templates/device_synchronous.html` | Replace execCommand, mousewheel, wheelDeltaY |
| `resources/static/js/remote_synchronous.js` | Replace mousewheel, wheelDeltaY |
| `resources/static/js/remote.js` | Replace wheelDeltaY |

### What NOT to Implement

- Do NOT change the scroll/swipe behavior logic — only the API calls
- Do NOT add new features — strictly API replacement
- Do NOT modify backend code — this is frontend-only
- Do NOT break IE11 support if currently working (use graceful fallback)

### Browser Compatibility

| API | Chrome | Firefox | Safari | Edge |
|-----|--------|---------|--------|------|
| `navigator.clipboard` | 66+ | 63+ | 13.1+ | 79+ |
| `wheel` event | 31+ | 17+ | 7+ | 12+ |
| `deltaY` | 31+ | 17+ | 7+ | 12+ |

All target browsers support the modern APIs. Keep fallback for edge cases.

### Previous Story Intelligence (Story 13.2)

Key learnings from Story 13.2:
- **Code review is essential** — Found 2 HIGH issues that needed fixing
- **AppError pattern established** — Type-safe error handling with thiserror
- **Test coverage is solid** — 226 tests catch regressions
- **IntoAppError trait useful** — For converting string errors to AppError

### Project Structure Notes

- Frontend files in `resources/static/js/` and `resources/templates/`
- No database changes
- No backend code changes
- No new files

### References

- [Source: _bmad-output/planning-artifacts/epics-v2.md#Story 13.3]
- [Source: resources/static/js/common.js:20 — execCommand]
- [Source: resources/templates/device_synchronous.html:1870, 2383, 2399 — execCommand, wheelDeltaY, mousewheel]
- [Source: resources/static/js/remote_synchronous.js:487, 494 — wheelDeltaY, mousewheel]
- [Source: resources/static/js/remote.js:2171, 2174 — wheelDeltaY]
- [MDN: Clipboard API](https://developer.mozilla.org/en-US/docs/Web/API/Clipboard_API)
- [MDN: WheelEvent](https://developer.mozilla.org/en-US/docs/Web/API/WheelEvent)

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

None

### Completion Notes List

- 2026-03-12: All deprecated browser APIs replaced with modern equivalents
- 2026-03-12: Clipboard API implementation includes fallback for older browsers
- 2026-03-12: Wheel delta direction correctly inverted (wheelDeltaY < 0 → deltaY > 0)
- 2026-03-12: All 226 tests pass - no regressions

### File List

- `resources/static/js/common.js` — Updated copyToClipboard with modern Clipboard API + fallback
- `resources/templates/device_synchronous.html` — Updated copyPhrase, wheel event, deltaY
- `resources/static/js/remote_synchronous.js` — Updated wheel event and deltaY
- `resources/static/js/remote.js` — Updated deltaY (2 locations)

## Change Log

- 2026-03-12: Story created from epics-v2.md
