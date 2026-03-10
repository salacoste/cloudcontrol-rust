/**
 * NIO-style WebSocket client
 * Inspired by Java NIO design: Channel + Buffer + Selector
 *
 * Features:
 * 1. Single WebSocket connection handles all communication
 * 2. Event-driven, asynchronous non-blocking
 * 3. Auto-reconnect and heartbeat
 * 4. Message queue and batch processing
 */

class NIOChannel {
    constructor(udid, options = {}) {
        this.udid = udid;
        this.options = Object.assign({
            reconnectInterval: 2000,
            maxReconnectAttempts: 10,
            heartbeatInterval: 25000,
            messageQueueSize: 100
        }, options);

        this.ws = null;
        this.connected = false;
        this.reconnectAttempts = 0;

        // Event handlers - similar to Selector
        this.handlers = new Map();

        // Message buffer - similar to Buffer
        this.messageQueue = [];
        this.pendingRequests = new Map();

        // State
        this.subscriptions = new Set();

        // Heartbeat
        this.heartbeatTimer = null;

        // Monotonic message ID counter
        this._msgSeq = 0;
    }

    /**
     * Connect to server
     */
    connect() {
        return new Promise((resolve, reject) => {
            const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
            const url = `${protocol}//${location.host}/nio/${this.udid}/ws`;

            console.log('[NIO] Connecting:', url);

            this.ws = new WebSocket(url);
            // Receive binary frames as Blob for zero-copy screenshot rendering
            this.ws.binaryType = 'blob';

            this.ws.onopen = () => {
                console.log('[NIO] Connected');
                this.connected = true;
                this.reconnectAttempts = 0;
                this._startHeartbeat();
                this._flushQueue();
                this._emit('connected');
                resolve();
            };

            this.ws.onclose = (event) => {
                console.log('[NIO] Connection closed:', event.code, event.reason);
                this.connected = false;
                this._stopHeartbeat();
                this._emit('disconnected');
                this._tryReconnect();
            };

            this.ws.onerror = (error) => {
                console.error('[NIO] Connection error:', error);
                this._emit('error', error);
                if (!this.connected) {
                    reject(error);
                }
            };

            this.ws.onmessage = (event) => {
                if (event.data instanceof Blob) {
                    // Binary frame = screenshot JPEG
                    this._emit('screenshot', { status: 'ok', blob: event.data });
                } else {
                    this._handleMessage(event.data);
                }
            };
        });
    }

    /**
     * Disconnect
     */
    disconnect() {
        this.reconnectAttempts = this.options.maxReconnectAttempts; // Prevent reconnection
        if (this.ws) {
            this.ws.close();
            this.ws = null;
        }
        this._stopHeartbeat();
        this.connected = false;
        // Reject all pending requests
        this.pendingRequests.forEach(function(pending) {
            clearTimeout(pending.timer);
            pending.reject(new Error('Disconnected'));
        });
        this.pendingRequests.clear();
    }

    /**
     * Send message
     */
    send(type, data = {}) {
        const message = {
            type: type,
            data: data,
            id: (++this._msgSeq) + '_' + Math.random().toString(36).substr(2, 5)
        };

        if (this.connected && this.ws.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(message));
        } else {
            // Queue for later sending
            if (this.messageQueue.length < this.options.messageQueueSize) {
                this.messageQueue.push(message);
            }
        }

        return message.id;
    }

    /**
     * Send request and wait for response
     */
    request(type, data = {}, timeout = 5000) {
        return new Promise((resolve, reject) => {
            const id = this.send(type, data);

            const timer = setTimeout(() => {
                this.pendingRequests.delete(id);
                reject(new Error('Request timeout'));
            }, timeout);

            this.pendingRequests.set(id, { resolve, reject, timer });
        });
    }

    /**
     * Subscribe to event stream
     */
    subscribe(target, options = {}) {
        this.subscriptions.add(target);
        return this.send('subscribe', { target, ...options });
    }

    /**
     * Unsubscribe
     */
    unsubscribe(target) {
        this.subscriptions.delete(target);
        return this.send('unsubscribe', { target });
    }

    /**
     * Register event handler
     */
    on(event, handler) {
        if (!this.handlers.has(event)) {
            this.handlers.set(event, []);
        }
        this.handlers.get(event).push(handler);
        return this;
    }

    /**
     * Remove event handler
     */
    off(event, handler) {
        if (this.handlers.has(event)) {
            const handlers = this.handlers.get(event);
            const index = handlers.indexOf(handler);
            if (index > -1) {
                handlers.splice(index, 1);
            }
        }
        return this;
    }

    /**
     * Emit event
     */
    _emit(event, data) {
        if (this.handlers.has(event)) {
            this.handlers.get(event).forEach(handler => {
                try {
                    handler(data);
                } catch (e) {
                    console.error('[NIO] Event handler error:', e);
                }
            });
        }
    }

    /**
     * Handle received message
     */
    _handleMessage(raw) {
        try {
            const message = JSON.parse(raw);

            // Check if this is a response to a pending request
            if (message.id && this.pendingRequests.has(message.id)) {
                const { resolve, timer } = this.pendingRequests.get(message.id);
                clearTimeout(timer);
                this.pendingRequests.delete(message.id);
                resolve(message);
                return;
            }

            // Dispatch event by type
            const eventType = message.type || 'message';
            this._emit(eventType, message);
            this._emit('message', message);

        } catch (e) {
            console.error('[NIO] Message parse error:', e);
        }
    }

    /**
     * Flush message queue
     */
    _flushQueue() {
        while (this.messageQueue.length > 0 && this.connected) {
            const message = this.messageQueue.shift();
            this.ws.send(JSON.stringify(message));
        }

        // Re-subscribe
        this.subscriptions.forEach(target => {
            this.send('subscribe', { target });
        });
    }

    /**
     * Attempt reconnection
     */
    _tryReconnect() {
        if (this.reconnectAttempts >= this.options.maxReconnectAttempts) {
            console.log('[NIO] Max reconnection attempts reached, stopping');
            this._emit('reconnect_failed');
            return;
        }

        this.reconnectAttempts++;
        console.log(`[NIO] Attempting reconnection in ${this.options.reconnectInterval}ms (${this.reconnectAttempts}/${this.options.maxReconnectAttempts})`);

        setTimeout(() => {
            this.connect().catch(() => {
                // Connection failure triggers onclose, continue reconnecting
            });
        }, this.options.reconnectInterval);
    }

    /**
     * Start heartbeat
     */
    _startHeartbeat() {
        this._stopHeartbeat();
        this.heartbeatTimer = setInterval(() => {
            if (this.connected) {
                this.send('heartbeat');
            }
        }, this.options.heartbeatInterval);
    }

    /**
     * Stop heartbeat
     */
    _stopHeartbeat() {
        if (this.heartbeatTimer) {
            clearInterval(this.heartbeatTimer);
            this.heartbeatTimer = null;
        }
    }
}


/**
 * NIO Screen Controller
 * Encapsulates screen-related operations
 */
class NIOScreenController {
    constructor(channel, canvas) {
        this.channel = channel;
        this.canvas = canvas;
        this.ctx = canvas.getContext('2d');
        this.running = false;
        this.lastFrame = null;
        this.fps = 0;
        this.frameCount = 0;
        this.lastFpsTime = Date.now();

        // Bind events
        this.channel.on('screenshot', (msg) => this._onScreenshot(msg));
    }

    /**
     * Start screen stream
     */
    start(interval = 50) {
        if (this.running) return;
        this.running = true;
        this.channel.subscribe('screenshot', { interval });
        console.log('[NIO Screen] Screen stream started');
    }

    /**
     * Stop screen stream
     */
    stop() {
        if (!this.running) return;
        this.running = false;
        this.channel.unsubscribe('screenshot');
        console.log('[NIO Screen] Screen stream stopped');
    }

    /**
     * Handle screenshot - supports binary Blob (new) and base64 JSON (legacy)
     */
    _onScreenshot(msg) {
        if (!this.running || msg.status !== 'ok') return;

        // Binary blob path (zero-copy, no base64 decode)
        var source = msg.blob || null;
        if (!source && msg.data) {
            // Legacy base64 fallback
            source = b64toBlob(msg.data, 'image/jpeg');
        }
        if (!source) return;

        var self = this;
        createImageBitmap(source).then(function(bitmap) {
            requestAnimationFrame(function() {
                if (self.canvas.width !== bitmap.width || self.canvas.height !== bitmap.height) {
                    self.canvas.width = bitmap.width;
                    self.canvas.height = bitmap.height;
                }
                self.ctx.drawImage(bitmap, 0, 0);
                bitmap.close();
                self.lastFrame = Date.now();

                // Calculate FPS
                self.frameCount++;
                var now = Date.now();
                if (now - self.lastFpsTime >= 1000) {
                    self.fps = self.frameCount;
                    self.frameCount = 0;
                    self.lastFpsTime = now;
                }
            });
        });
    }

    /**
     * Send touch event
     */
    touch(x, y) {
        this.channel.send('touch', { x, y });
    }

    /**
     * Send swipe event
     */
    swipe(x1, y1, x2, y2, duration = 200) {
        this.channel.send('swipe', { x1, y1, x2, y2, duration: duration / 1000 });
    }

    /**
     * Send text input
     */
    input(text) {
        this.channel.send('input', { text });
    }

    /**
     * Send key event
     */
    keyevent(key) {
        this.channel.send('keyevent', { key });
    }
}


// Export
window.NIOChannel = NIOChannel;
window.NIOScreenController = NIOScreenController;
