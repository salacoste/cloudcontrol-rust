# Epic 1A: Device Connection & Discovery - Stories

## Story 1A-1: WiFi Device Auto-Discovery

**Title:** WiFi Device Auto-Discovery

**User Story:**
> As a **Device Farm Operator**, I want devices on my network to be automatically discovered, so that I don't have to manually configure each device.

**Acceptance Criteria:**

```gherkin
Scenario: Discover devices on standard ATX port
  Given the system is running
  And there are Android devices running ATX Agent on port 7912 reachable via WiFi
  When the discovery scan executes
  Then all reachable devices appear in the device list
  And each device shows connection status "connected"

Scenario: Discover devices on alternate port
  Given there are devices running ATX Agent on port 9008
  When the discovery scan executes
  Then devices on port 9008 appear in the device list
  
Scenario: Handle network timeout gracefully
  Given a device is unreachable
  When the discovery scan executes
  Then the system logs the timeout
  And the device does not appear in the list
  And no error is thrown to the user
```

---

## Story 1A-2: USB Device Discovery with ADB Forwarding

**Title:** USB Device Auto-Detection

**User Story:**
> As a **QA Engineer**, I want USB-connected devices to be automatically detected and forwarded, so that I can use them immediately without manual ADB setup.

**Acceptance Criteria:**

```gherkin
Scenario: Detect USB device and forward port
  Given an Android device is connected via USB
  And ADB is available on the system
  When the USB detection routine runs
  Then the device is detected via "adb devices"
  And port 7912 is forwarded to the device
  And the device appears in the device list

Scenario: Handle multiple USB devices
  Given three Android devices are connected via USB
  When the USB detection routine runs
  Then all three devices are detected
  And each device gets a unique forwarded port
  And all devices appear in the device list

Scenario: Handle missing ADB
  Given ADB is not installed on the system
  When USB detection attempts to run
  Then a clear error message is logged
  And the system continues to function
  And WiFi discovery still works
```

---

## Story 1A-3: ATX Agent Protocol Connection

**Title:** ATX Agent Handshake

**User Story:**
> As a **System**, I want to establish reliable connections to devices running ATX Agent, so that all device operations work correctly.

**Acceptance Criteria:**

```gherkin
Scenario: Successful ATX Agent handshake
  Given a device is reachable at a known IP:port
  When the system initiates an ATX Agent connection
  Then the HTTP handshake completes successfully
  And device info (model, version, UDID) is retrieved
  And the connection is added to the pool

Scenario: Retrieve device info during handshake
  Given a device is connected via ATX Agent
  When the handshake completes
  Then device model is stored
  And Android version is stored
  And device UDID is stored
  And screen resolution is retrieved

Scenario: Handle incompatible ATX Agent version
  Given a device runs an incompatible ATX Agent version
  When handshake is attempted
  Then connection fails gracefully
  And an error is logged with version mismatch details
```

---

## Story 1A-4: Manual WiFi Device Addition

**Title:** Manual Device Connection

**User Story:**
> As a **Remote Support Technician**, I want to manually add a device by IP address, so that I can connect to devices that weren't auto-discovered.

**Acceptance Criteria:**

```gherkin
Scenario: Add device by IP and port
  Given I have a device's IP address "192.168.1.100" and port "7912"
  When I submit the manual connection form
  Then the system attempts to connect to the device
  And if successful, the device appears in the list
  And the device is marked as "manually added"

Scenario: Validate IP address format
  Given I enter an invalid IP address "not-an-ip"
  When I submit the form
  Then validation fails immediately
  And a clear error message shows the expected format

Scenario: Handle connection failure gracefully
  Given I enter a valid IP but the device is unreachable
  When I submit the form
  Then the system attempts connection with a timeout
  And a clear error message indicates the device is unreachable
  And no device is added to the list
```

---

## Story 1A-5: Connection Health Monitoring
**Title:** Disconnect Detection

**User Story:**
> As a **Device Farm Operator**, I want to know immediately when a device disconnects, so that I can address connectivity issues.

**Acceptance Criteria:**

```gherkin
Scenario: Detect WiFi device disconnection
  Given a device is connected via WiFi
  And the device loses network connectivity
  When the health check runs
  Then the device status changes to "disconnected"
  And the disconnection time is logged
  And the device remains in the list with updated status

Scenario: Detect USB device disconnection
  Given a device is connected via USB
  And the USB cable is unplugged
  When the system detects the disconnection
  Then the device status changes to "disconnected"
  And the disconnection is logged

Scenario: Handle reconnection after brief disconnect
  Given a device is marked "disconnected"
  And the device becomes reachable again within 30 seconds
  When the health check runs
  Then the device status changes to "connected"
  And the reconnection is logged
```

---

## Story 1A-6: Automatic Reconnection

**Title:** Network Recovery Auto-Reconnect

**User Story:**
> As a **QA Engineer**, I want devices to automatically reconnect after network blips, so that my testing sessions aren't interrupted.

**Acceptance Criteria:**

```gherkin
Scenario: Auto-reconnect after WiFi recovery
  Given a device was connected via WiFi
  And the device disconnected due to network issues
  And the network recovers within 30 seconds
  When the reconnection logic runs
  Then the device automatically reconnects
  And the device status returns to "connected"
  And no manual intervention is required

Scenario: Preserve connection pool during reconnect
  Given a device reconnects automatically
  When reconnection completes
  Then the existing connection pool entry is reused or refreshed
  And connection statistics are updated

Scenario: Handle prolonged outage
  Given a device has been disconnected for more than 5 minutes
  When the reconnection attempts continue to fail
  Then the device remains in "disconnected" status
  And a warning is logged about prolonged outage
  And the system continues attempting reconnection
```

---

# Epic 1B: Device Dashboard & Management - Stories


# Epic 1B: Device Dashboard & Management - Stories

## Story 1B-1: Device Grid Dashboard

**Title:** Device List with Status Indicators

**User Story:**
> As a **Device Farm Operator**, I want to see all connected devices in a grid with status badges, so that I can monitor my entire fleet at a glance.

**Acceptance Criteria:**

```gherkin
Scenario: Display connected devices in grid
  Given multiple devices are connected
  When I open the dashboard
  Then all devices appear in a grid layout
  And each device card shows device name
  And each device card shows status badge (green=connected, yellow=connecting, red=disconnected)

Scenario: Status badge reflects real-time state
  Given a device is currently connected
  And its status badge shows green
  When the device disconnects
  Then the status badge changes to red within 5 seconds

Scenario: Empty state handling
  Given no devices are connected
  When I open the dashboard
  Then a friendly empty state message appears
  And instructions for adding devices are shown
```

---

## Story 1B-2: Device Metadata Panel

**Title:** Device Info Display

**User Story:**
> As a **QA Engineer**, I want to see device metadata like model and battery level, so that I know which device I'm working with.

**Acceptance Criteria:**

```gherkin
Scenario: Display device metadata
  Given a device is selected in the dashboard
  When I view the device info panel
  Then the device model is displayed (e.g., "SM-G990B")
  And the Android version is displayed (e.g., "Android 13")
  And the battery level is displayed (e.g., "85%")
  And the screen resolution is displayed (e.g., "1080x2400")

Scenario: Update battery level in real-time
  Given a device is connected with 90% battery
  And I'm viewing the device info panel
  When the battery drops to 89%
  Then the displayed battery level updates to "89%"
  And the update occurs within 30 seconds

Scenario: Handle missing metadata
  Given a device is connected but metadata retrieval fails
  When I view the device info panel
  Then "Unknown" is displayed for missing fields
  And no error blocks the UI
```

---

## Story 1B-3: Device State Persistence
**Title:** Device State Persistence

**User Story:**
> As a **Device Farm Operator**, I want device state to persist across server restarts, so that I don't lose my device configuration.

**Acceptance Criteria:**

```gherkin
Scenario: Persist device state on change
  Given a device is connected
  When any device state changes (status, metadata, tags)
  Then the state is persisted to SQLite
  And the persistence completes within 1 second

Scenario: Restore state after server restart
  Given devices were connected and persisted
  And the server restarts
  When the server starts up again
  Then all previously connected devices are loaded from persistence
  And the system attempts to reconnect to each device
  And device tags and labels are preserved

Scenario: Handle corrupted persistence file
  Given the SQLite database is corrupted
  When the server starts
  Then a new database is created
  And a warning is logged
  And the server starts successfully
```

---

## Story 1B-4: Device Tagging System
**Title:** Device Tags and Labels

**User Story:**
> As a **QA Engineer**, I want to tag devices with labels like "regression-tests" or "android-13", so that I can easily find the right devices.

**Acceptance Criteria:**

```gherkin
Scenario: Add tag to device
  Given a device is in the list
  When I add tag "regression-tests" to the device
  Then the tag appears on the device card
  And the tag is persisted to the database

Scenario: Filter devices by tag
  Given device A has tag "android-13"
  And device B has tag "android-12"
  When I filter by tag "android-13"
  Then only device A appears in the filtered list

Scenario: Add multiple tags to device
  Given a device has no tags
  When I add tags "physical", "us-market", "low-battery"
  Then all three tags appear on the device card
  And I can filter by any combination of tags

Scenario: Remove tag from device
  Given a device has tag "old-tag"
  When I remove the tag
  Then the tag no longer appears on the device card
  And the device no longer appears when filtering by that tag
```

---

## Story 1B-5: Connection History & Uptime
**Title:** Device Connection History

**User Story:**
> As a **Device Farm Operator**, I want to see connection history and uptime statistics, so that I can identify unreliable devices.

**Acceptance Criteria:**

```gherkin
Scenario: Display connection history
  Given a device has connected and disconnected multiple times
  When I view the device details
  Then the connection history shows timestamps
  And the connection history shows duration for each session
  And the history is ordered most recent first

Scenario: Calculate uptime percentage
  Given a device has been connected for 18 hours in the last 24 hours
  When I view device statistics
  Then the uptime percentage shows "75%"
  And the calculation is accurate to the hour

Scenario: Display total connection time
  Given a device has been monitored for 1 week
  When I view device statistics
  Then the total time connected is displayed
  And the total time disconnected is displayed
```

---

## Story 1B-6: Manual Device Disconnect
**Title:** Disconnect Device from UI

**User Story:**
> As a **Device Farm Operator**, I want to disconnect individual devices from the management interface, so that I can remove problematic devices without physical access.

**Acceptance Criteria:**

```gherkin
Scenario: Disconnect device via button
  Given a device is connected
  When I click the disconnect button on the device card
  Then the device connection is closed
  And the device status changes to "disconnected"
  And the disconnection is logged in history

Scenario: Disconnect with confirmation
  Given a device is actively being used for testing
  When I click the disconnect button
  Then a confirmation dialog appears
  And the dialog warns about active operations
  And I can confirm or cancel the disconnect

Scenario: Reconnect after manual disconnect
  Given a device was manually disconnected
  And the device is still reachable on the network
  When I click the reconnect button
  Then the system attempts to reconnect
  And if successful, the device status changes to "connected"
```

---

# Epic 2: Real-Time Visual Monitoring - Stories
# Epic 2: Real-Time Visual Monitoring - Stories
## Story 2-1: Single Screenshot Capture
**Title:** HTTP Screenshot Endpoint

**User Story:**
> As an **Automation Engineer**, I want to request a screenshot via HTTP API, so that I can capture device state in my CI/CD pipeline.

**Acceptance Criteria:**

```gherkin
Scenario: Capture screenshot via HTTP
  Given a device is connected
  When I request GET /inspector/{udid}/screenshot
  Then a JPEG screenshot is returned
  And the response time is under 500ms
  And the Content-Type header is "image/jpeg"

Scenario: Handle disconnected device gracefully
  Given a device is listed but disconnected
  When I request GET /inspector/{udid}/screenshot
  Then HTTP 503 is returned
  And the error code is "ERR_DEVICE_DISCONNECTED"
  And the message explains the device is not connected

Scenario: Specify screenshot quality
  Given a device is connected
  When I request GET /inspector/{udid}/screenshot?quality=50
  Then a JPEG screenshot with quality 50% is returned
  And the file size is smaller than default quality
```

---

## Story 2-2: Real-Time WebSocket Screenshot Streaming
**Title:** Binary WebSocket Screenshot Stream

**User Story:**
> As a **QA Engineer**, I want to see real-time screenshots via WebSocket, so that I can monitor device activity as it happens.

**Acceptance Criteria:**

```gherkin
Scenario: Start WebSocket screenshot stream
  Given a device is connected
  When I connect to /ws/screenshot/{udid}
  Then binary screenshot frames start streaming
  And each frame latency is under 200ms
  And frames are JPEG-encoded images

Scenario: Handle WebSocket connection drop
  Given a WebSocket stream is active
  When the device disconnects
  Then the WebSocket closes with code 1001
  And a close message indicates "device_disconnected"

Scenario: Stream to multiple clients
  Given a device is connected
  And two clients connect to the same device's screenshot stream
  When screenshots are captured
  Then both clients receive the same frames
  And no frame duplication occurs on the device side
```

---

## Story 2-3: Configurable Screenshot Quality
**Title:** Screenshot Quality Levels

**User Story:**
> As a **Device Farm Operator**, I want to configure screenshot quality, so that I can balance between image clarity and bandwidth usage.

**Acceptance Criteria:**

```gherkin
Scenario: Set quality to low (bandwidth optimized)
  Given I'm streaming screenshots
  When I set quality to "low" (quality=30)
  Then screenshots are compressed with JPEG quality 30
  And average frame size is under 50KB
  And streaming latency improves

Scenario: Set quality to high (clarity optimized)
  Given I'm streaming screenshots
  When I set quality to "high" (quality=90)
  Then screenshots are compressed with JPEG quality 90
  And text on screen remains readable
  And average frame size is under 200KB

Scenario: Default quality level
  Given no quality is specified
  When I request a screenshot
  Then JPEG quality defaults to 70
  And the balance between size and clarity is reasonable
```

---

## Story 2-4: Multi-Device Screenshot Batch
**Title:** Batch Screenshot Capture

**User Story:**
> As a **QA Engineer**, I want to capture screenshots from multiple devices at once, so that I can compare states across my test fleet.

**Acceptance Criteria:**

```gherkin
Scenario: Capture screenshots from multiple devices
  Given 5 devices are connected and selected
  When I request POST /screenshot/batch with the device UDIDs
  Then screenshots from all 5 devices are returned
  And each screenshot is keyed by device UDID
  And total response time is under 2 seconds

Scenario: Handle partial failures in batch
  Given 5 devices are selected
  And device 3 is disconnected
  When I request batch screenshot
  Then 4 successful screenshots are returned
  And device 3 has error "ERR_DEVICE_DISCONNECTED"
  And HTTP 207 Multi-Status is returned

Scenario: Progress indicator for batch capture
  Given 10 devices are selected for batch screenshot
  When the batch operation starts
  Then a progress indicator shows completion percentage
  And the indicator updates as each device completes
```

---

## Story 2-5: Screenshot Resizing
**Title:** Screenshot Resize for Bandwidth Optimization

**User Story:**
> As a **Remote Support Technician**, I want screenshots resized for my connection, so that I can work over slower networks.
**Acceptance Criteria:**

```gherkin
Scenario: Resize screenshot to half resolution
  Given a device has resolution 1080x2400
  When I request screenshot with scale=0.5
  Then the returned image is 540x1200
  And the file size is proportionally smaller
  And aspect ratio is preserved

Scenario: Resize screenshot to thumbnail
  Given I need a quick preview
  When I request screenshot with scale=0.25
  Then the returned image is 270x600 (for 1080x2400 device)
  And the image loads quickly even on slow connections

Scenario: Preserve original size by default
  Given no scale parameter is provided
  When I request a screenshot
  Then the original device resolution is returned
  And no resizing processing occurs
```

---

## Story 2-6: Screenshot Download
**Title:** Download Screenshot as File

**User Story:**
> As a **QA Engineer**, I want to download screenshots as JPEG or PNG, so that I can save them for bug reports.
**Acceptance Criteria:**

```gherkin
Scenario: Download screenshot as JPEG
  Given a screenshot is displayed
  When I click download and select JPEG format
  Then the file downloads with .jpg extension
  And the Content-Disposition header includes filename
  And filename includes device UDID and timestamp

Scenario: Download screenshot as PNG
  Given a screenshot is displayed
  When I click download and select PNG format
  Then the file downloads with .png extension
  And the image quality is lossless

Scenario: Filename format
  Given I download a screenshot
  When the download completes
  Then the filename follows pattern: {udid}_{timestamp}.jpg
  And the timestamp is in ISO 8601 format
  And example: "abc123_2026-03-05T14-30-00.jpg"
```

---
# Epic 3: Remote Device Control - Stories
# Epic 3: Remote Device Control - Stories
## Story 3-1: Tap Command Execution

**Title:** Tap at Coordinates

**User Story:**
> As a **QA Engineer**, I want to send tap commands to specific screen coordinates, so that I can interact with UI elements remotely.

**Acceptance Criteria:**

```gherkin
Scenario: Execute tap at coordinates
  Given a device is connected with screen visible
  When I send POST /tap with x=540, y=1200
  Then a tap is executed at coordinates (540, 1200)
  And the response confirms tap execution
  And response time is under 100ms

Scenario: Tap on screenshot preview
  Given a screenshot is displayed in the control panel
  When I click on the screenshot at position (200, 300)
  Then the tap coordinates are calculated relative to actual screen
  And the tap executes on the device
  And a new screenshot refreshes to show the result

Scenario: Validate coordinates within screen bounds
  Given a device has resolution 1080x2400
  When I send tap with x=2000, y=3000 (out of bounds)
  Then HTTP 400 is returned
  And error code is "ERR_INVALID_REQUEST"
  And the message explains coordinate bounds
```

---

## Story 3-2: Swipe Gesture Execution

**Title:** Swipe Gestures

**User Story:**
> As a **QA Engineer**, I want to send swipe gestures with direction and duration, so that I can scroll and navigate through apps.
**Acceptance Criteria:**

```gherkin
Scenario: Execute swipe gesture
  Given a device is connected
  When I send POST /swipe with from=(100,500) to=(100,200) duration=300ms
  Then a swipe gesture is executed from (100,500) to (100,200)
  And the gesture takes approximately 300ms
  And the response confirms swipe execution

Scenario: Common swipe patterns
  Given I need to scroll quickly
  When I use predefined swipe "scroll_up"
  Then a swipe from bottom-center to top-center executes
  And the duration is 200ms (optimized for scrolling)

Scenario: Swipe for navigation
  Given I need to go back via gesture
  When I use predefined swipe "back" (edge swipe)
  Then a swipe from left edge to right executes
  And the Android back gesture is triggered

Scenario: Invalid swipe parameters
  Given I send a swipe request
  When duration is negative or zero
  Then HTTP 400 is returned
  And error code is "ERR_INVALID_REQUEST"
```

---

## Story 3-3: Text Input to Device
**Title:** Text Input Command

**User Story:**
> As a **Remote Support Technician**, I want to input text into focused text fields, so that I can fill forms and enter data remotely.

**Acceptance Criteria:**

```gherkin
Scenario: Input text to focused field
  Given a text field is focused on the device
  When I send POST /input with text="hello@example.com"
  Then "hello@example.com" is typed into the focused field
  And special characters (@, .) are handled correctly
  And response time is under 100ms

Scenario: Handle long text input
  Given I need to input a paragraph of text
  When I send POST /input with 500 characters
  Then all 500 characters are input
  And the input completes within 2 seconds
  And no characters are lost

Scenario: Clear field before input
  Given a text field already contains text
  When I send POST /input with text="new text" and clear=true
  Then the field is cleared first
  And then "new text" is input
  And the result is exactly "new text"

Scenario: Handle non-focused state
  Given no text field is focused on the device
  When I send POST /input with text="test"
  Then the input is still sent (ATX Agent behavior)
  And a warning is included in the response
```

---

## Story 3-4: Physical Key Events
**Title:** Key Event Commands

**User Story:**
> As a **QA Engineer**, I want to send physical key events like home and back, so that I can navigate the device without touch gestures.

**Acceptance Criteria:**

```gherkin
Scenario: Send HOME key
  Given a device is on any screen
  When I send POST /key with action="home"
  Then the device returns to home screen
  And response time is under 100ms

Scenario: Send BACK key
  Given an app is open
  When I send POST /key with action="back"
  Then the device navigates back
  And the previous screen appears

Scenario: Send VOLUME keys
  Given a device is connected
  When I send POST /key with action="volume_up"
  Then the volume increases by one step
  And when I send action="volume_down"
  Then the volume decreases by one step

Scenario: Send POWER key
  Given a device screen is on
  When I send POST /key with action="power"
  Then the screen turns off
  And when I send power again
  Then the screen turns on

Scenario: Invalid key action
  Given I send POST /key with action="invalid_key"
  Then HTTP 400 is returned
  And supported keys are listed in the error message
```

---

## Story 3-5: UI Hierarchy Inspector
**Title:** UI Hierarchy Inspection

**User Story:**
> As a **Remote Support Technician**, I want to view the UI hierarchy inspector, so that I can debug accessibility issues and find hidden elements.
**Acceptance Criteria:**

```gherkin
Scenario: Load UI hierarchy
  Given a device is connected
  When I request GET /inspector/{udid}/hierarchy
  Then the UI hierarchy XML is returned
  And the response time is under 2 seconds
  And elements include bounds, text, resource-id, and class

Scenario: Interactive element highlighting
  Given the UI hierarchy is displayed
  When I hover over an element in the hierarchy
  Then the element's bounds are highlighted on the screenshot
  And element attributes are shown in a tooltip

Scenario: Search hierarchy by text
  Given the UI hierarchy is loaded
  When I search for "Login"
  Then all elements containing "Login" text are highlighted
  And the number of matches is displayed

Scenario: Handle large hierarchy
  Given a complex app screen with 500+ elements
  When I request the hierarchy
  Then the hierarchy is returned within 2 seconds
  And the response is paginated or truncated if too large
```

---

## Story 3-6: Shell Command Execution
**Title:** Shell Command Interface

**User Story:**
> As an **Automation Engineer**, I want to execute shell commands on devices, so that I can perform advanced debugging and automation.

**Acceptance Criteria:**

```gherkin
Scenario: Execute simple shell command
  Given a device is connected
  When I send POST /shell with command="getprop ro.build.version.release"
  Then the Android version is returned
  And the command executes in under 1 second
  And stdout and stderr are separated in response

Scenario: Execute command with timeout
  Given a potentially slow command
  When I send POST /shell with command="logcat -d" and timeout=5000
  Then the command executes with 5-second timeout
  And if timeout occurs, the process is killed
  And a timeout error is returned

Scenario: Handle dangerous commands safely
  Given I attempt to run "reboot" or "rm -rf"
  When the command is received
  Then the command is blocked or requires confirmation
  And a warning is logged
  And the user is notified of the restriction

Scenario: Real-time command output
  Given I run a long-running command
  When I use WebSocket shell endpoint
  Then output streams in real-time
  And I can send input to interactive commands
  And I can terminate the command early
```

---
# Epic 4: Multi-Device Batch Operations - Stories
# Epic 4: Multi-Device Batch Operations - Stories
## Story 4-1: Multi-Device Selection UI
**Title:** Device Multi-Select Interface

**User Story:**
> As a **QA Engineer**, I want to select multiple devices using click and keyboard, so that I can quickly choose which devices to batch operate.

**Acceptance Criteria:**

```gherkin
Scenario: Single click to select device
  Given devices are displayed in the grid
  When I click on a device card
  Then the device is selected (highlighted with blue border)
  And a checkmark appears on the card

Scenario: Ctrl+click for multi-select
  Given device A is already selected
  When I Ctrl+click on device B
  Then both device A and device B are selected
  And the selection count shows "2 devices selected"

Scenario: Shift+click for range select
  Given devices are displayed in order
  And device 1 is selected
  When I Shift+click on device 5
  Then devices 1 through 5 are all selected
  And the selection count shows "5 devices selected"

Scenario: Select all devices
  Given 10 devices are displayed
  When I press Ctrl+A or click "Select All"
  Then all 10 devices are selected
  And the selection count shows "10 devices selected"

Scenario: Clear selection
  Given multiple devices are selected
  When I press Escape or click "Deselect All"
  Then no devices are selected
  And the selection count shows "0 devices selected"
```

---

## Story 4-2: Synchronized Batch Operations
**Title:** Execute Actions Across All Selected Devices

**User Story:**
> As a **QA Engineer**, I want to execute the same action on all selected devices, so that I can test multiple devices simultaneously.

**Acceptance Criteria:**

```gherkin
Scenario: Execute batch tap
  Given 5 devices are selected
  When I execute a tap at (540, 1200)
  Then the tap is sent to all 5 devices in parallel
  And all responses are collected
  And a summary shows success/failure count

Scenario: Execute batch swipe
  Given 3 devices are selected
  When I execute a scroll-up swipe
  Then the swipe is sent to all 3 devices
  And each device scrolls simultaneously
  And screenshots update for all devices

Scenario: Execute batch text input
  Given 4 devices are selected with text fields focused
  When I input "test@example.com"
  Then the text is sent to all 4 devices
  And all devices show the input

Scenario: Handle partial batch failures
  Given 5 devices are selected
  And device 3 is disconnected
  When I execute batch tap
  Then 4 taps succeed
  And 1 tap fails with "ERR_DEVICE_DISCONNECTED"
  And the summary shows "4/5 successful"
  And failed device is highlighted in results

Scenario: Batch operation progress indicator
  Given 10 devices are selected
  When I execute a batch operation
  Then a progress bar shows completion status
  And it updates as each device responds
  And the bar shows "5/10 complete" during execution
```

---

## Story 4-3: Action Recording System
**Title:** Record User Actions

**User Story:**
> As a **QA Engineer**, I want to record my actions on one device, so that I can replay them on multiple devices later.

**Acceptance Criteria:**

```gherkin
Scenario: Start recording session
  Given a device is selected
  When I click "Start Recording"
  Then recording mode is activated
  And a red recording indicator appears
  And all subsequent actions are recorded

Scenario: Record tap action
  Given recording is active
  When I tap at (100, 200)
  Then the action is recorded with type "tap", x=100, y=200
  And the timestamp is recorded
  And the action appears in the action list

Scenario: Record swipe action
  Given recording is active
  When I swipe from (100, 500) to (100, 200)
  Then the action is recorded with type "swipe", coordinates, and duration
  And the action appears in the action list

Scenario: Record text input action
  Given recording is active
  When I input "test text"
  Then the action is recorded with type "input" and text="test text"
  And the action appears in the action list

Scenario: Stop recording and save
  Given recording is active with 5 recorded actions
  When I click "Stop Recording"
  Then I'm prompted to name the recording
  And the recording is saved to the recording library
  And I can replay it later
```

---

## Story 4-4: Recording Session Management
**Title:** Recording Playback Controls

**User Story:**
> As a **QA Engineer**, I want to control recording sessions with start/stop/pause, so that I can manage the recording process.

**Acceptance Criteria:**

```gherkin
Scenario: Pause and resume recording
  Given recording is active with 3 actions recorded
  When I click "Pause Recording"
  Then no new actions are recorded
  And the indicator changes to "Paused"
  When I click "Resume Recording"
  Then new actions are recorded again
  And actions after pause are appended to the same recording

Scenario: Delete recorded action
  Given a recording has 5 actions
  When I delete action 3
  Then the recording has 4 actions
  And remaining actions are renumbered

Scenario: Edit recorded action
  Given a recorded tap at (100, 200)
  When I edit it to (150, 250)
  Then the action is updated
  And the recording reflects the change

Scenario: Cancel recording without saving
  Given recording is active with 5 actions
  When I click "Cancel Recording"
  Then a confirmation dialog appears
  And when confirmed, the recording is discarded
  And no recording is saved
```

---

## Story 4-5: Recording Playback
**Title:** Replay Recording on Multiple Devices

**User Story:**
> As a **QA Engineer**, I want to replay recorded actions on selected devices, so that I can automate repetitive testing.

**Acceptance Criteria:**

```gherkin
Scenario: Replay recording on single device
  Given a recording exists with 5 actions
  And one device is selected
  When I select the recording and click "Play"
  Then all 5 actions execute in sequence
  And each action waits for the previous to complete
  And screenshots update after each action

Scenario: Replay on multiple devices
  Given a recording exists with 5 actions
  And 3 devices are selected
  When I click "Play"
  Then actions execute on all 3 devices in parallel
  And all devices stay synchronized
  And progress shows for each device

Scenario: Set playback speed
  Given a recording exists
  When I set playback speed to "0.5x"
  Then actions execute at half speed
  And delays between actions are doubled

Scenario: Set playback speed to fast
  Given a recording exists
  When I set playback speed to "2x"
  Then actions execute at double speed
  And delays between actions are halved

Scenario: Stop playback mid-execution
  Given playback is in progress (action 3 of 10)
  When I click "Stop"
  Then playback stops immediately
  And no further actions execute
  And devices remain in current state
```

---

## Story 4-6: Batch Test Report Export
**Title:** Export Batch Test Reports

**User Story:**
> As a **QA Engineer**, I want to export batch test reports with per-device results, so that I can document testing outcomes.

**Acceptance Criteria:**

```gherkin
Scenario: Export basic test report
  Given a batch operation completed on 5 devices
  And 4 succeeded, 1 failed
  When I click "Export Report"
  Then a JSON report is generated
  And the report includes timestamp, operation type, and device results
  And each device result includes UDID, status, and duration

Scenario: Export report with screenshots
  Given a batch operation completed with screenshots
  When I export the report with "Include Screenshots"
  Then the report includes base64-encoded screenshots
  Or screenshots are bundled as separate files in a ZIP

Scenario: Export report in multiple formats
  Given a batch operation completed
  When I select "Export as CSV"
  Then a CSV file is generated with tabular results
  When I select "Export as HTML"
  Then an HTML report with styling is generated

Scenario: Per-device error details in report
  Given a batch operation had partial failures
  When the report is generated
  Then failed devices include error messages
  And stack traces are included if available
  And error codes are included for programmatic parsing
```

---
# Epic 5: External API & CI/CD Integration - Stories
# Epic 5: External API & CI/CD Integration - Stories
## Story 5-1: REST API Device Operations
**Title:** REST API for Device Connection

**User Story:**
> As an **Automation Engineer**, I want to connect to devices via REST API, so that my CI/CD pipeline can manage devices programmatically.

**Acceptance Criteria:**

```gherkin
Scenario: List all devices via API
  Given devices are connected to the system
  When I request GET /api/v1/list
  Then a JSON array of devices is returned
  And each device includes UDID, status, model, and connection type
  And response time is under 100ms

Scenario: Connect to WiFi device via API
  Given I have a device at IP 192.168.1.100:7912
  When I request POST /api/v1/wifi-connect {"ip":"192.168.1.100","port":7912}
  Then the device connects
  And the device info is returned
  And HTTP 200 is returned on success

Scenario: Get single device info
  Given a device with UDID "abc123" is connected
  When I request GET /api/v1/device/abc123
  Then full device info is returned
  And includes battery, screen resolution, Android version
  And includes current connection status

Scenario: Handle device not found
  Given no device with UDID "nonexistent" exists
  When I request GET /api/v1/device/nonexistent
  Then HTTP 404 is returned
  And error code is "ERR_DEVICE_NOT_FOUND"
```

---

## Story 5-2: WebSocket Screenshot Streaming API
**Title:** WebSocket Screenshot Streaming API

**User Story:**
> As an **Automation Engineer**, I want to stream screenshots via WebSocket API, so that my automated tests can monitor device state in real-time.

**Acceptance Criteria:**

```gherkin
Scenario: Connect to screenshot WebSocket
  Given a device is connected
  When I connect to /api/v1/ws/screenshot/{udid}
  Then binary screenshot frames start streaming
  And frames are JPEG-encoded images
  And frame latency is under 200ms

Scenario: Control stream via JSON-RPC
  Given a screenshot WebSocket is open
  When I send {"jsonrpc":"2.0","method":"setQuality","params":50,"id":1}
  Then screenshot quality changes to 50
  And a success response is returned

  And response: {"jsonrpc":"2.0","result":"ok","id":1}

Scenario: Handle device disconnect during stream
  Given a screenshot WebSocket stream is active
  When the device disconnects
  Then a JSON message is sent: {"event":"device_disconnected"}
  And the WebSocket closes gracefully
```

---

## Story 5-3: Device Status and Health API
**Title:** Device Health Check Endpoints

**User Story:**
> As a **DevOps Engineer**, I want to check device status via API, so that I can monitor device farm health in my monitoring system.
**Acceptance Criteria:**

```gherkin
Scenario: Get all device statuses
  Given multiple devices are connected
  When I request GET /api/v1/status
  Then a summary of all devices is returned
  And includes count by status (connected, disconnected, error)
  And includes average battery level

Scenario: Health check for load balancer
  Given the system is running
  When I request GET /api/v1/health
  Then HTTP 200 is returned if system is healthy
  And includes connection_pool_status and database_status
  And response time is under 50ms

Scenario: Metrics endpoint for monitoring
  Given the system is running
  When I request GET /api/v1/metrics
  Then Prometheus-compatible metrics are returned
  And includes: connected_devices, websocket_connections, screenshot_latency_p95
```

---

## Story 5-4: CI/CD Integration Examples
**Title:** CI/CD Pipeline Integration Guide

**User Story:**
> As an **Automation Engineer**, I want example integrations for CI/CD tools, so that I can quickly set up automated testing.
**Acceptance Criteria:**

```gherkin
Scenario: GitHub Actions integration example
  Given the documentation exists
  When I view CI/CD integration docs
  Then a complete GitHub Actions workflow example is provided
  And shows: device connection, screenshot capture, test execution
  And includes error handling patterns

Scenario: Jenkins pipeline example
  Given the documentation exists
  When I view CI/CD integration docs
  Then a Jenkinsfile example is provided
  And shows multi-device parallel testing
  And shows report generation
```

---

## Story 5-5: JSON-RPC WebSocket Interface
**Title:** JSON-RPC Command Interface

**User Story:**
> As an **Automation Engineer**, I want to send JSON-RPC commands over WebSocket, so that I have a standardized protocol for automation.
**Acceptance Criteria:**

```gherkin
Scenario: Execute JSON-RPC tap command
  Given a WebSocket connection to /api/v1/ws/nio is open
  When I send {"jsonrpc":"2.0","method":"tap","params":{"x":100,"y":200},"id":1}
  Then a tap executes at (100, 200)
  And response {"jsonrpc":"2.0","result":"ok","id":1} is returned

Scenario: Batch operations via JSON-RPC
  Given a WebSocket connection is open
  When I send {"method":"batchTap","params":{"udids":["a","b","c"],"x":100,"y":200},"id":3}
  Then the tap executes on all specified devices
  And the result includes per-device status

Scenario: Handle JSON-RPC errors
  Given a WebSocket connection is open
  When I send {"method":"invalidMethod","id":4}
  Then response {"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":4} is returned
```

---
# Epic 6: High-Fidelity Screen Mirroring - Stories
# Epic 6: High-Fidelity Screen Mirroring - Stories
## Story 6-1: Scrcpy Session Management

**Title:** Start/Stop Scrcpy Sessions

**User Story:**
> As a **QA Engineer**, I want to start and stop high-fidelity screen mirroring via scrcpy, so that I can view detailed screen content when JPEG quality isn't enough.

**Acceptance Criteria:**

```gherkin
Scenario: Start scrcpy session
  Given a device is connected via USB
  And scrcpy is installed on the server
  When I request POST /scrcpy/{udid}/start
  Then a scrcpy process spawns
  And the session ID is returned
  And H.264 video stream begins

Scenario: Stop scrcpy session
  Given a scrcpy session is running
  When I request POST /scrcpy/{udid}/stop
  Then the scrcpy process terminates
  And resources are cleaned up
  And the session is marked as ended

Scenario: Handle scrcpy not installed
  Given scrcpy is not available on the server
  When I request to start scrcpy
  Then HTTP 503 is returned
  And error code is "ERR_SCRCPY_NOT_AVAILABLE"
  And installation instructions are included

Scenario: List active scrcpy sessions
  Given multiple scrcpy sessions are running
  When I request GET /scrcpy/sessions
  Then all active sessions are listed
  And each session shows UDID, start time, and status
```

---

## Story 6-2: Scrcpy Device Control
**Title:** Control Device Through Scrcpy Stream

**User Story:**
> As a **QA Engineer**, I want to control devices through the scrcpy video stream, so that I can interact while viewing high-quality video.
**Acceptance Criteria:**

```gherkin
Scenario: Tap through scrcpy stream
  Given a scrcpy session is active
  When I send input events through the scrcpy socket
  Then the tap executes on the device
  And the video stream shows the result
  And latency is under 100ms (NFR17)

Scenario: Keyboard input through scrcpy
  Given a scrcpy session is active
  And a text field is focused
  When I send keyboard events
  Then text appears on the device
  And special keys (Enter, Backspace) work correctly

Scenario: Handle input during high latency
  Given network latency is high
  When I send rapid inputs
  Then inputs are queued and executed in order
  And no inputs are dropped
  And visual feedback indicates pending inputs
```

---

## Story 6-3: H.264 WebSocket Relay
**Title:** H.264 Video Stream Relay

**User Story:**
> As a **Remote Support Technician**, I want to view the H.264 stream via WebSocket, so that I can watch high-fidelity video in my browser.
**Acceptance Criteria:**

```gherkin
Scenario: Connect to H.264 WebSocket stream
  Given a scrcpy session is running
  When I connect to /ws/scrcpy/{udid}
  Then H.264 video frames are relayed in real-time
  And the stream uses binary frames
  And latency is under 100ms (NFR17)

Scenario: Handle multiple viewers
  Given a scrcpy session is active
  And three clients connect to the stream
  When the video frames arrive
  Then all three clients receive the same frames
  And the device-side scrcpy process is not duplicated
  And frame broadcasting is efficient

Scenario: Stream metadata
  Given a WebSocket stream is connected
  When the stream starts
  Then a JSON metadata message is sent first
  And includes: width, height, frame_rate, codec info

```

---

## Story 6-4: Scrcpy Session Recording
**Title:** Session Recording and Playback

**User Story:**
> As a **QA Engineer**, I want to record scrcpy sessions for later review, so that I can analyze issues that occurred during testing.
**Acceptance Criteria:**

```gherkin
Scenario: Start recording scrcpy session
  Given a scrcpy session is active
  When I enable recording
  Then the H.264 stream is saved to a .mp4 file
  And the file is named with UDID and timestamp
  And recording continues until stopped

Scenario: Stop recording and access file
  Given recording is in progress
  When I stop recording
  Then the MP4 file is finalized
  And the file is accessible via download endpoint
  And file metadata (duration, size) is returned

Scenario: List recorded sessions
  Given multiple sessions were recorded
  When I request GET /scrcpy/recordings
  Then a list of recordings is returned
  And each shows: UDID, start_time, duration, file_size

Scenario: Delete recorded session
  Given a recording exists
  When I request DELETE /scrcpy/recordings/{id}
  Then the file is deleted
  And it no longer appears in the list
```

---

# Story Summary

## Epic 1A: Device Connection & Discovery
- **6 Stories:** WiFi discovery, USB discovery, ATX Agent handshake, Manual addition, Disconnect detection, Auto-reconnection
- **Dependencies:** None (foundational)
- **Phase:** MVP (Core)

## Epic 1B: Device Dashboard & Management
- **6 Stories:** Device grid dashboard, Device metadata panel, State persistence, Device tagging, Connection history, Manual disconnect
- **Dependencies:** Epic 1A
- **Phase:** MVP (Phase 1)

## Epic 2: Real-Time Visual Monitoring
- **6 Stories:** HTTP screenshot, WebSocket streaming, Screenshot quality, Batch screenshot, Screenshot resizing, Screenshot download
- **Dependencies:** Epic 1A, 1B
- **Phase:** MVP
## Epic 3: Remote Device Control
- **6 Stories:** Tap commands, Swipe gestures, Text input, Key events, UI hierarchy inspector, Shell commands
- **Dependencies:** Epic 1A, 2
- **Phase:** MVP
## Epic 4: Multi-Device Batch Operations
- **6 Stories:** Multi-select UI, Synchronized operations, Action recording, Recording session control, Recording playback, Batch test reports
- **Dependencies:** Epic 1A-3
- **Phase:** MVP
## Epic 5: External API & CI/CD Integration
- **5 Stories:** REST API, WebSocket API, Device health API, CI/CD examples, JSON-RPC interface
- **Dependencies:** Epic 1A-4
- **Phase:** Growth (Post-MVP)
## Epic 6: High-Fidelity Screen Mirroring
- **4 Stories:** Scrcpy sessions, Scrcpy control, H.264 relay, Session recording
- **Dependencies:** Epic 1A-4
- **Phase:** Post-MVP (Future)

---

**Total Stories: 39**

**MVP Stories: 29 (Epics 1A-4)**
**Post-MVP Stories: 10 (Epic 5, Scrcpy - 4 stories)**
