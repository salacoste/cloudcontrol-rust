use crate::device::adb::Adb;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Child;

const SCRCPY_SERVER_PATH: &str = "resources/scrcpy/scrcpy-server.jar";
const SCRCPY_DEVICE_PATH: &str = "/data/local/tmp/scrcpy-server.jar";
const SCRCPY_VERSION: &str = "2.7";

/// Metadata returned after scrcpy handshake.
#[derive(Debug, Clone)]
pub struct ScrcpyMeta {
    pub device_name: String,
    pub width: u32,
    pub height: u32,
}

/// A single parsed video frame from scrcpy.
pub struct ScrcpyFrame {
    pub pts: u64,
    pub is_config: bool,
    pub is_key: bool,
    pub data: Vec<u8>,
}

/// Manages a scrcpy session for one device.
pub struct ScrcpySession {
    pub serial: String,
    pub meta: ScrcpyMeta,
    pub video_stream: TcpStream,
    pub control_stream: TcpStream,
    pub local_port: u16,
    pub server_process: Option<Child>,
}

impl ScrcpySession {
    /// Full startup: push JAR, forward, launch server, connect, handshake.
    pub async fn start(serial: &str) -> Result<Self, String> {
        // 1. Verify local JAR exists, then push to device
        if !std::path::Path::new(SCRCPY_SERVER_PATH).exists() {
            return Err(format!("scrcpy-server.jar not found at {}", SCRCPY_SERVER_PATH));
        }

        let push_result = Adb::push(serial, SCRCPY_SERVER_PATH, SCRCPY_DEVICE_PATH).await;
        match &push_result {
            Ok(msg) => tracing::info!("[Scrcpy] JAR pushed to {}: {}", serial, msg.trim()),
            Err(e) => tracing::error!("[Scrcpy] JAR push failed for {}: {}", serial, e),
        }
        push_result.map_err(|e| format!("push scrcpy-server failed: {}", e))?;

        // Verify JAR exists on device
        let verify = Adb::shell(serial, &format!("ls -la {}", SCRCPY_DEVICE_PATH)).await;
        match &verify {
            Ok(msg) => tracing::info!("[Scrcpy] JAR verified on device: {}", msg.trim()),
            Err(e) => {
                tracing::error!("[Scrcpy] JAR not found on device after push: {}", e);
                return Err("scrcpy-server.jar not found on device after push".to_string());
            }
        }

        // 2. Forward to default abstract socket name "scrcpy"
        let socket_name = "scrcpy";
        let local_port = Adb::forward_abstract(serial, socket_name)
            .await
            .map_err(|e| format!("forward abstract failed: {}", e))?;

        tracing::info!(
            "[Scrcpy] Forward established: 127.0.0.1:{} -> localabstract:{}",
            local_port,
            socket_name
        );

        // 3. Launch scrcpy-server via adb shell (no scid = default socket name "scrcpy")
        let server_process = tokio::process::Command::new("adb")
            .args([
                "-s",
                serial,
                "shell",
                &format!(
                    "CLASSPATH={} app_process / com.genymobile.scrcpy.Server {} \
                     tunnel_forward=true audio=false video_codec=h264 \
                     max_size=1920 max_fps=60 video_bit_rate=4000000 \
                     send_frame_meta=true \
                     clipboard_autosync=false power_off_on_close=false",
                    SCRCPY_DEVICE_PATH,
                    SCRCPY_VERSION,
                ),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to spawn scrcpy-server: {}", e))?;

        // 4. Wait for server to be ready, then connect + handshake
        // Protocol (tunnel_forward=true, audio=false):
        //   1. Client connects → video socket, server sends dummy byte (0x00)
        //   2. Client connects AGAIN → control socket
        //   3. Server sends device_name (64 bytes) on video socket
        //   4. Video frames start flowing
        let addr = format!("127.0.0.1:{}", local_port);

        let mut video_stream = None;

        for attempt in 0..30 {
            let delay = if attempt < 3 { 500 } else { 300 };
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;

            // Connect video socket (first connection)
            let mut stream = match TcpStream::connect(&addr).await {
                Ok(s) => s,
                Err(_) if attempt < 29 => continue,
                Err(e) => {
                    let _ = Adb::forward_remove(serial, local_port).await;
                    return Err(format!("Failed to connect video socket: {}", e));
                }
            };

            // Read dummy byte (0x00) — if EOF, server isn't ready yet
            let mut dummy = [0u8; 1];
            match tokio::time::timeout(
                std::time::Duration::from_millis(500),
                stream.read_exact(&mut dummy),
            )
            .await
            {
                Ok(Ok(_)) => {
                    video_stream = Some(stream);
                    tracing::info!(
                        "[Scrcpy] Video connected on attempt {}, dummy=0x{:02x}",
                        attempt + 1,
                        dummy[0]
                    );
                    break;
                }
                _ => {
                    if attempt < 29 {
                        tracing::debug!(
                            "[Scrcpy] Handshake attempt {} failed, retrying...",
                            attempt + 1
                        );
                        continue;
                    }
                    let _ = Adb::forward_remove(serial, local_port).await;
                    return Err("handshake failed after all retries".to_string());
                }
            }
        }

        let mut video_stream = video_stream.ok_or("handshake never completed")?;

        // 5. Connect control socket (second connection — server waits for this)
        let control_stream = TcpStream::connect(&addr)
            .await
            .map_err(|e| format!("Failed to connect control socket: {}", e))?;
        tracing::info!("[Scrcpy] Control socket connected");

        // 6. NOW read device name (64 bytes) from video socket
        //    Server only sends this AFTER control socket is also connected
        let mut name_buf = [0u8; 64];
        tokio::time::timeout(std::time::Duration::from_secs(5), video_stream.read_exact(&mut name_buf))
            .await
            .map_err(|_| "device name read timed out".to_string())?
            .map_err(|e| format!("device name read failed: {}", e))?;

        let device_name = String::from_utf8_lossy(&name_buf)
            .trim_end_matches('\0')
            .to_string();
        tracing::info!("[Scrcpy] Device name: {}", device_name);

        // 7. Read codec metadata (12 bytes): codec_id(4B) + width(4B) + height(4B)
        //    scrcpy v2.7 sends this right after device name, before video frames
        let mut codec_meta = [0u8; 12];
        tokio::time::timeout(std::time::Duration::from_secs(5), video_stream.read_exact(&mut codec_meta))
            .await
            .map_err(|_| "codec metadata read timed out".to_string())?
            .map_err(|e| format!("codec metadata read failed: {}", e))?;

        let codec_id = String::from_utf8_lossy(&codec_meta[0..4]).to_string();
        let width = u32::from_be_bytes(codec_meta[4..8].try_into().unwrap());
        let height = u32::from_be_bytes(codec_meta[8..12].try_into().unwrap());
        tracing::info!("[Scrcpy] Codec: {}, dimensions: {}x{}", codec_id, width, height);

        let meta = ScrcpyMeta {
            device_name,
            width,
            height,
        };

        tracing::info!(
            "[Scrcpy] Session started for {}: {}x{}",
            serial,
            width,
            height
        );

        Ok(Self {
            serial: serial.to_string(),
            meta,
            video_stream,
            control_stream,
            local_port,
            server_process: Some(server_process),
        })
    }


    /// Read one video frame from the stream.
    ///
    /// Frame header (12 bytes):
    /// - bytes[0..8]: u64 BE — PTS microseconds. bit63 = config frame, bit62 = keyframe
    /// - bytes[8..12]: u32 BE — packet size
    /// - Then `packet_size` bytes of H.264 NAL data
    pub async fn read_frame(&mut self) -> Result<ScrcpyFrame, String> {
        let mut header = [0u8; 12];
        self.video_stream
            .read_exact(&mut header)
            .await
            .map_err(|e| format!("read frame header: {}", e))?;

        let pts_raw = u64::from_be_bytes(header[0..8].try_into().unwrap());
        let packet_size = u32::from_be_bytes(header[8..12].try_into().unwrap());

        let is_config = (pts_raw >> 63) & 1 == 1;
        let is_key = (pts_raw >> 62) & 1 == 1;
        let pts = pts_raw & 0x3FFF_FFFF_FFFF_FFFF;

        if packet_size == 0 || packet_size > 10 * 1024 * 1024 {
            return Err(format!("invalid packet size: {}", packet_size));
        }

        let mut data = vec![0u8; packet_size as usize];
        self.video_stream
            .read_exact(&mut data)
            .await
            .map_err(|e| format!("read frame data: {}", e))?;

        Ok(ScrcpyFrame {
            pts,
            is_config,
            is_key,
            data,
        })
    }

    /// Send a touch event through the control socket.
    ///
    /// Format (28 bytes):
    /// - type: u8 = 2 (INJECT_TOUCH_EVENT)
    /// - action: u8 (0=down, 1=up, 2=move)
    /// - pointer_id: u64 BE = 0xFFFFFFFFFFFFFFFF (finger)
    /// - x: u32 BE (device coordinates)
    /// - y: u32 BE (device coordinates)
    /// - width: u16 BE (screen width)
    /// - height: u16 BE (screen height)
    /// - pressure: u16 BE (0xFFFF for touch, 0 for release)
    /// - action_button: u32 BE = 0
    /// - buttons: u32 BE = 0
    pub async fn send_touch(
        &mut self,
        action: u8,
        x: u32,
        y: u32,
        width: u16,
        height: u16,
        pressure: u16,
    ) -> Result<(), String> {
        let mut buf = [0u8; 28];
        buf[0] = 2; // INJECT_TOUCH_EVENT
        buf[1] = action;
        // pointer_id = -1 (POINTER_ID_MOUSE)
        buf[2..10].copy_from_slice(&0xFFFFFFFFFFFFFFFFu64.to_be_bytes());
        buf[10..14].copy_from_slice(&x.to_be_bytes());
        buf[14..18].copy_from_slice(&y.to_be_bytes());
        buf[18..20].copy_from_slice(&width.to_be_bytes());
        buf[20..22].copy_from_slice(&height.to_be_bytes());
        buf[22..24].copy_from_slice(&pressure.to_be_bytes());
        // action_button and buttons = 0 (already zeroed)

        self.control_stream
            .write_all(&buf)
            .await
            .map_err(|e| format!("send touch failed: {}", e))
    }

    /// Send a key event through the control socket.
    ///
    /// Format (14 bytes):
    /// - type: u8 = 0 (INJECT_KEYCODE)
    /// - action: u8 (0=down, 1=up)
    /// - keycode: u32 BE (Android KEYCODE_*)
    /// - repeat: u32 BE = 0
    /// - metastate: u32 BE = 0
    pub async fn send_key(&mut self, action: u8, keycode: u32) -> Result<(), String> {
        let mut buf = [0u8; 14];
        buf[0] = 0; // INJECT_KEYCODE
        buf[1] = action;
        buf[2..6].copy_from_slice(&keycode.to_be_bytes());
        // repeat and metastate = 0 (already zeroed)

        self.control_stream
            .write_all(&buf)
            .await
            .map_err(|e| format!("send key failed: {}", e))
    }

    /// Clean up: remove forward, kill server process.
    pub async fn shutdown(mut self) {
        tracing::info!("[Scrcpy] Shutting down session for {}", self.serial);

        let _ = self.video_stream.shutdown().await;
        let _ = self.control_stream.shutdown().await;

        if let Some(mut proc) = self.server_process.take() {
            let _ = proc.kill().await;
        }

        let _ = Adb::forward_remove(&self.serial, self.local_port).await;
    }
}

