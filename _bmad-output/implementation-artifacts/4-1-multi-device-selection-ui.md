# Story 4.1: Multi-Device Selection UI

Status: done

## Story

As a **QA Engineer**, I want to select multiple devices using click and keyboard, so that I can quickly choose which devices to batch operate.

## Acceptance Criteria

1. **Single click to select device**
   - Given devices are displayed in the grid
   - When I click on a device card
   - Then the device is selected (highlighted with blue border)
   - And a checkmark appears on the card

2. **Ctrl+click for multi-select**
   - Given device A is already selected
   - When I Ctrl+click on device B
   - Then both device A and device B are selected
   - And the selection count shows "2 devices selected"

3. **Shift+click for range select**
   - Given devices are displayed in order
   - And device 1 is selected
   - When I Shift+click on device 5
   - Then devices 1 through 5 are all selected
   - And the selection count shows "5 devices selected"

4. **Select all devices**
   - Given 10 devices are displayed
   - When I press Ctrl+A or click "Select All"
   - Then all 10 devices are selected
   - And the selection count shows "10 devices selected"

5. **Clear selection**
   - Given multiple devices are selected
   - When I press Escape or click "Deselect All"
   - Then no devices are selected
   - And the selection count shows "0 devices selected"

## Tasks / Subtasks

- [x] Task 1: Add selection state management (AC: 1, 2, 3, 4, 5)
  - [x] Add JavaScript module for device selection state
  - [x] Track selected device UDIDs in a Set (using Vue.js array)
  - [x] Add selection count display in UI
  - [x] Persist selection state during session (lost on page reload - intentional)

- [x] Task 2: Implement click handlers (AC: 1, 2, 3)
  - [x] Add click handler for device cards (single select)
  - [x] Add Ctrl+click handler (toggle selection)
  - [x] Add Shift+click handler (range select)
  - [x] Track last-clicked device for range selection

- [x] Task 3: Add selection UI elements (AC: 1, 4, 5)
  - [x] Add CSS for selected state (blue border, checkmark)
  - [x] Add "Select All" button
  - [x] Add "Deselect All" button
  - [x] Add selection count badge/indicator

- [x] Task 4: Add keyboard shortcuts (AC: 4, 5)
  - [x] Add Ctrl+A handler for select all
  - [x] Add Escape handler for clear selection

- [x] Task 5: Integrate with batch operations
  - [x] Expose selected device IDs for batch operations
  - [x] Add visual indicator when batch mode is active
  - [x] Disable selection when single-device mode is needed

## Dev Notes

### Architecture Context

This is primarily a **frontend JavaScript feature**. The current device dashboard is server-side rendered using Tera templates. Multi-device selection needs to be implemented as client-side JavaScript.

### Current Device Grid Implementation

The device grid is rendered in `resources/templates/*.html` templates:

```html
<!-- Device card structure (approximate) -->
<div class="device-card" data-udid="{{ device.udid }}">
  <div class="device-screenshot">...</div>
  <div class="device-info">...</div>
</div>
```

### Required JavaScript Module

Create a new JavaScript module `resources/static/js/device-selection.js`:

```javascript
// Device Selection Module
const DeviceSelection = {
  selectedDevices: new Set(),
  lastClickedIndex: null,
  
  init() {
    this.bindEvents();
    this.updateSelectionCount();
  },
  
  toggleDevice(udid) { ... },
  selectRange(fromIndex, toIndex) { ... },
  selectAll() { ... },
  clearSelection() { ... },
  updateSelectionCount() { ... },
  getSelectedDevices() { return Array.from(this.selectedDevices); }
};
```

### CSS Styles for Selection

```css
/* Selected device card */
.device-card.selected {
  border: 2px solid #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.3);
}

.device-card.selected::before {
  content: '✓';
  position: absolute;
  top: 8px;
  right: 8px;
  background: #3b82f6;
  color: white;
  border-radius: 50%;
  width: 24px;
  height: 24px;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* Selection count badge */
.selection-count {
  position: fixed;
  bottom: 20px;
  right: 20px;
  background: #3b82f6;
  color: white;
  padding: 12px 20px;
  border-radius: 8px;
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  z-index: 1000;
}
```

### Integration with Batch Operations

The selected devices will be used by:
- Story 4-2: Synchronized Batch Operations
- Story 4-3: Action Recording System

The `getSelectedDevices()` function returns an array of UDIDs that batch operation handlers can use.

### Project Structure Notes

- JavaScript: `resources/static/js/device-selection.js` (new file)
- CSS: `resources/static/css/device-selection.css` (new file or add to existing)
- Templates: Modify device grid template to include selection attributes
- No backend changes required for this story (selection is client-side only)

### Performance Considerations

- Selection state should be lightweight (just UDID strings)
- Avoid re-rendering the entire grid on selection changes
- Use CSS classes for visual updates, not DOM reconstruction
- Selection should persist during the session but not across page reloads (unless explicitly saved)

### Previous Story Learnings

1. **Client-side state**: Use JavaScript modules for UI state management
2. **Event delegation**: Use event delegation for dynamic device lists
3. **Keyboard accessibility**: Ensure all selection actions work with keyboard
4. **Visual feedback**: Immediate visual feedback for selection changes

### References

- [Source: resources/templates/*.html] - Current device grid templates
- [Source: resources/static/js/*.js] - Existing JavaScript patterns
- [Source: _bmad-output/planning-artifacts/epics-stories.md:833-872] - Story definition
- [Source: _bmad-output/planning-artifacts/ux-design-specification.md] - UX patterns

## Acceptance Criteria Met

- ✅ **AC1: Single click to select device** - Click on device card toggles selection with blue border and checkmark
- ✅ **AC2: Ctrl+click for multi-select** - Works with toggle logic (already implemented)
- ✅ **AC3: Shift+click for range select** - Implemented using `lastClickedIndex` and range selection logic
- ✅ **AC4: Select all devices** - Ctrl+A keyboard shortcut and SELECT ALL button both work
- ✅ **AC5: Clear selection** - Escape key and DESELECT button both clear selection

## Dev Agent Record

### Agent Model Used

Claude Opus 4.6 (claude-opus-4-6)

### Debug Log References

None - implementation was straightforward.

### Completion Notes List

1. **Already Partially Implemented**: The Vue.js app in `index.html` already had basic selection functionality (`selectedDevices` array, `toggleSelect`, `selectAll`, `deselectAll` methods).

2. **Added CSS Selected State**: Added `.term-device-card.selected` styles with blue border, glow effect, and checkmark indicator.

3. **Enhanced Click Handler**: Updated `toggleSelect()` to accept an event parameter and handle:
   - Regular click: Toggle single device selection
   - Shift+click: Range selection from last clicked to current
   - Ctrl+click: Already worked with toggle logic

4. **Added Keyboard Shortcuts**:
   - `Ctrl+A`: Select all devices (only when not in input field)
   - `Escape`: Clear selection

5. **Selection Count**: Already displayed in System Status section.

6. **Range Selection**: Implemented using `lastClickedIndex` data property and `filteredDevices` computed property.

**Code Review Fixes Applied:**
7. **Removed unused `data-index`**: Removed unnecessary `data-index` attribute from device card template (index calculated dynamically via `filteredDevices.indexOf()`).
8. **Clarified persistence task**: Updated Task 1 subtask to clarify that persistence is session-only, not across page reloads.

### File List

- `resources/templates/index.html` - Added `lastClickedIndex`, keyboard handlers, enhanced `toggleSelect()` with event handling
- `resources/static/css/terminal-theme.css` - Added `.term-device-card.selected` styles with blue border and checkmark
- `_bmad-output/implementation-artifacts/sprint-status.yaml` - Updated Epic 4 and Story 4-1 status to in-progress
