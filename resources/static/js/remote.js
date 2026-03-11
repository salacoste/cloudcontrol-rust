/* Javascript — v2-swipe-fix */
$(function () {
  try {
    $('.btn-copy')
      .mouseleave(function () {
        var $element = $(this);
        $element.tooltip('hide').tooltip('disable');
      })

    var clipboard = new Clipboard('.btn-copy');
    clipboard.on('success', function (e) {
      $(e.trigger)
        .attr('title', 'Copied')
        .tooltip('fixTitle')
        .tooltip('enable')
        .tooltip('show');
    })

    $('[data-toggle=tooltip]').tooltip({
      trigger: 'hover',
    });
  } catch(e) { console.warn('Clipboard init skipped:', e.message); }
})
// Server-relative URLs used throughout — no hardcoded IPs or versions

window.app = new Vue({
  el: '#app',
  delimiters: ['[[', ']]'],
  data: {
    deviceUdid: deviceUdid,
    device: {},
    deviceInfo: {},
    activeTab: 'home',
    fixConsole: '', // log for fix minicap and rotation
    navtabs: {
      active: location.hash.slice(1) || 'home',
      tabs: [],
    },
    error: '',
    control: null,
    loading: true,
    canvas: {
      bg: null,
      fg: null,
    },
    canvasStyle: {
      opacity: 1,
      width: 'inherit',
      height: 'inherit'
    },
    canvasStyleTree: {
      opacity: 1,
      width: 'inherit',
      height: 'inherit'
    },
    lastScreenSize: {
      screen: {},
      canvas: {
        width: 1,
        height: 1
      }
    },
    whatsinput: {
      text: "",
      disabled: true,
    },
    websockets:{
      winput: null,
    },
    power:"755",
    path:"/data/local/tmp/",
    screenWS: null,
    // scrcpy mode state
    useScrcpyMode: false,
    scrcpyClient: null,
    browserURL: "",
    logcat: {
      follow: true,
      tagColors: {},
      lineNumber: 0,
      maxKeep: 1500,
      cachedScrollTop: 0,
      logs: [{
        lineno: 1,
        tag: "EsService2",
        level: "W",
        content: "loaded /system/lib/egl/libEGL_adreno200.so",
      }]
    },
    imageBlobBuffer: [],
    videoReceiver: null,
    videoRecordings: [],
    showVideoPanel: false,
    inputText: '',
    uploadStatus: '',
    inputWS: null,
    platform: localStorage.platform || 'Android',
    imagePool:null,
    showCursorPercent: true,
    cursor: {},
    rotation: 0,
    elem:{"name":"","description":"","text":"","touchable":"","resourceId":"","clickable":"",
          "package":"","label":"","width":"","height":"","enabled":"","visible":"","tag":"","anchor":"",
          "className":"","type":""},
    userSettings: Object.assign({
      inputName: '',
      inputCommand: '',
      visible: false,
      shortcuts: [{
        command: "input keyevent POWER",
        name: 'Power',
      }]
    }, {}),
    topApp: {
      packageName: '',
      activity: '',
      pid: '',
    },
    // Performance monitoring
    perfStats: {
      fps: 0,
      screenshot: 0,
      command: 0
    },
    perfHistory: {
      screenshots: [],
      commands: []
    },
    // Quick phrases
    phrases: [],
    newPhrase: '',
    phrasesCollapsed: false,

  },
  watch: {
    platform: function (newval) {
      localStorage.setItem('platform', newval);
    },
    serial: function (newval) {
      localStorage.setItem('serial', newval);
    }
  },
  computed: {
    cursorValue: function () {
      if (this.showCursorPercent) {
        return { x: this.cursor.px, y: this.cursor.py }
      } else {
        return this.cursor
      }
    },
    nodes: function () {
      return this.originNodes
    },
    // elem: function () {
    //   return this.nodeSelected || {};
    // },
    elemXpath: function () {
      var xpath = '//' + (this.elem.className || '*');
      if (this.elem.text) {
        xpath += "[@text='" + this.elem.text + "']";
      }
      return xpath;
    },
    // deviceUrl removed — all calls proxied through server via /inspector/{udid}/...
    batteryLevel: function () {
      return this.deviceInfo.battery ? this.deviceInfo.battery.level : 0;
    },
    batteryTemp: function () {
      if (!this.deviceInfo.battery || this.deviceInfo.battery.temp == null) return '--';
      return (this.deviceInfo.battery.temp / 10).toFixed(1) + '\u00B0C';
    },
    batteryStatus: function () {
      if (!this.deviceInfo.battery) return '--';
      if (this.deviceInfo.battery.acPowered) return 'AC';
      if (this.deviceInfo.battery.usbPowered) return 'USB';
      return 'DISCHARGE';
    },
    batteryClass: function () {
      if (this.batteryLevel > 60) return '';
      if (this.batteryLevel > 20) return 'yellow';
      return 'red';
    },
    memoryPercent: function () {
      if (!this.deviceInfo.memory) return 0;
      var total = this.deviceInfo.memory.total;
      if (total && total > 0) return 50;
      return 0;
    }
  },
  mounted: function () {
    this.imagePool = new ImagePool(100);
    var self = this;
    $.notify.defaults({ className: "success" });

    this.canvas.bg = document.getElementById('bgCanvas');
    this.canvas.fg = document.getElementById('fgCanvas');
    this.canvas.bg_tree = document.getElementById('bgCanvas_tree');
    this.canvas.fg_tree = document.getElementById('fgCanvas_tree');
    window.c = this.canvas.bg;

    // Resize
    $(window).resize(function () {
      self.resizeScreen();
    });

    // Check server version
    this.checkVersion();
    // Initialize jstree
    this.initJstree();
    // For reference
    this.activeMouseControl();
    this.initDragDealer();

    (function (that,_device) {
      that.deviceInfo = _device;
      document.title = _device.model;
      try { $('#json-renderer').jsonViewer(device, {}); } catch(e) {}
    })(this,device);

    // Load video recordings for this device
    this.loadVideoRecordings();

    // Three-level fallback: scrcpy (hardware encoding, low latency) -> NIO (WebSocket screenshots) -> HTTP (polling)
    var httpModeInitialized = false;
    var initHttpMode = function() {
      if (httpModeInitialized) return;
      httpModeInitialized = true;
      console.log('[Fallback] Falling back to HTTP mode');
      self.enableTouch();
      self.openScreenStream();
    };

    var initNIOMode = function() {
      return self.tryNIOMode();
    };

    // 18-second timeout protection, ensure at least one mode works (scrcpy needs ~10s to start)
    setTimeout(function() {
      if (!self.useScrcpyMode && !self.useNIOMode) {
        console.log('[Fallback] Timeout, using HTTP mode');
        initHttpMode();
      }
    }, 18000);

    // Try scrcpy -> NIO -> HTTP
    this.tryScrcpyMode()
      .catch(function(err) {
        console.log('[Scrcpy] Not available:', err.message || err, ', trying NIO mode');
        return initNIOMode();
      })
      .catch(function(err) {
        console.log('[NIO] Not available:', err.message || err, ', falling back to HTTP mode');
        initHttpMode();
      });

    // reserveDevice handled separately, failure does not affect main functionality
    this.reserveDevice().catch(function(err) {
      console.log("reserveDevice failed:", err);
    });

    // wakeup device on connect
    setTimeout(function () {
      this.keyevent("WAKEUP");
    }.bind(this), 1)

    window.k = setTimeout(function () {
      var lineno = (this.logcat.lineNumber += 1);
      this.logcat.logs.push({
        lineno: lineno,
        tag: "EsService2",
        level: "W",
        content: "loaded /system/lib/egl/libEGL_adreno200.so",
      });
      if (this.logcat.follow) {
        // only keep maxKeep lines
        var maxKeep = Math.max(20, this.logcat.maxKeep);
        var size = this.logcat.logs.length;
        this.logcat.logs = this.logcat.logs.slice(size - maxKeep, size);

        // scroll to end
        var el = this.$refs.tab_content;
        var logcat = this.logcat;
        if (el.scrollTop < logcat.cachedScrollTop) {
          this.logcat.follow = false;
        } else {
          setTimeout(function () {
            logcat.cachedScrollTop = el.scrollTop = el.scrollHeight - el.clientHeight;
          }, 2);
        }
      }
    }.bind(this), 200);

    // Load whatsinput IME
    this.loadWhatsinput()

    // Enable keyboard input
    this.enableKeyboardInput()

    // Load quick phrases
    this.loadPhrases()
  },
  methods: {
    // Scrcpy mode - hardware H.264 encoding, lowest latency
    tryScrcpyMode: function() {
      var self = this;
      return new Promise(function(resolve, reject) {
        // Check WebCodecs support
        if (typeof ScrcpyClient === 'undefined' || !ScrcpyClient.isSupported()) {
          reject(new Error('WebCodecs not supported'));
          return;
        }

        // First check if scrcpy is available
        $.ajax({
          url: '/scrcpy/' + self.deviceUdid + '/status',
          method: 'GET',
          dataType: 'json',
          timeout: 3000
        }).done(function(resp) {
          if (!resp.available) {
            reject(new Error('scrcpy not available: ' + (resp.reason || 'jar missing')));
            return;
          }

          console.log('[Scrcpy] Status check passed, connecting...');
          var canvas = document.getElementById('bgCanvas');
          var client = new ScrcpyClient(self.deviceUdid, canvas);

          client.onInit = function(msg) {
            console.log('[Scrcpy] Initialization complete: ' + msg.width + 'x' + msg.height);
            // Stop existing HTTP/NIO screenshot streams
            if (self.screenWS) {
              try { self.screenWS.close(); } catch(e) {}
              self.screenWS = null;
            }
            self.useScrcpyMode = true;
            self.scrcpyClient = client;
            self.loading = false;
          };

          client.onFrame = function() {
            self.resizeScreen({
              width: client.width,
              height: client.height
            });
            // Update FPS
            self.perfStats.fps = client.fps;
          };

          client.onDisconnect = function() {
            console.log('[Scrcpy] Disconnected');
            self.useScrcpyMode = false;
            // Auto-fallback to NIO
            if (!self.useNIOMode) {
              console.log('[Scrcpy] Attempting fallback to NIO mode');
              self.tryNIOMode().catch(function() {
                self.enableTouch();
                self.openScreenStream();
              });
            }
          };

          client.connect()
            .then(function() {
              console.log('[Scrcpy] Connected, enabling scrcpy touch');
              self.enableScrcpyTouch();

              // Create fake screenWS for toggleScreen compatibility
              self.screenWS = {
                close: function() {
                  client.disconnect();
                }
              };

              resolve();
            })
            .catch(function(err) {
              reject(err);
            });

        }).fail(function(err) {
          reject(new Error('scrcpy status check failed'));
        });
      });
    },

    // Scrcpy mode touch - direct binary touch, latency < 5ms
    enableScrcpyTouch: function() {
      var self = this;
      var element = this.canvas.fg;
      var screen = { bounds: {} };
      var touchStart = null;

      function calculateBounds() {
        var el = element;
        screen.bounds.w = el.offsetWidth;
        screen.bounds.h = el.offsetHeight;
        screen.bounds.x = 0;
        screen.bounds.y = 0;
        while (el.offsetParent) {
          screen.bounds.x += el.offsetLeft;
          screen.bounds.y += el.offsetTop;
          el = el.offsetParent;
        }
      }

      function activeFinger(index, x, y) {
        $(".finger-" + index).addClass("active")
          .css("transform", 'translate3d(' + x + 'px,' + y + 'px,0)');
      }

      function deactiveFinger(index) {
        $(".finger-" + index).removeClass("active");
      }

      element.addEventListener('mousedown', function(e) {
        if (e.which === 3) return; // ignore right click
        e.preventDefault();
        calculateBounds();

        var xP = (e.pageX - screen.bounds.x) / screen.bounds.w;
        var yP = (e.pageY - screen.bounds.y) / screen.bounds.h;
        var client = self.scrcpyClient;
        var devX = Math.floor(xP * client.width);
        var devY = Math.floor(yP * client.height);

        touchStart = { xP: xP, yP: yP, devX: devX, devY: devY };

        // Send touch down immediately
        client.sendTouch(0, devX, devY, client.width, client.height, 0xFFFF);
        activeFinger(0, e.pageX, e.pageY);

        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
      });

      function onMouseMove(e) {
        if (!touchStart) return;
        e.preventDefault();
        calculateBounds();

        var xP = (e.pageX - screen.bounds.x) / screen.bounds.w;
        var yP = (e.pageY - screen.bounds.y) / screen.bounds.h;
        var client = self.scrcpyClient;
        var devX = Math.floor(xP * client.width);
        var devY = Math.floor(yP * client.height);

        touchStart.endDevX = devX;
        touchStart.endDevY = devY;

        // Send touch move for real-time dragging
        client.sendTouch(2, devX, devY, client.width, client.height, 0xFFFF);
        activeFinger(0, e.pageX, e.pageY);
      }

      function onMouseUp(e) {
        if (!touchStart) return;
        e.preventDefault();
        deactiveFinger(0);

        var client = self.scrcpyClient;
        var x = touchStart.endDevX !== undefined ? touchStart.endDevX : touchStart.devX;
        var y = touchStart.endDevY !== undefined ? touchStart.endDevY : touchStart.devY;

        // Send touch up
        client.sendTouch(1, x, y, client.width, client.height, 0);

        touchStart = null;
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
      }

      console.log('[Scrcpy] Touch enabled (binary protocol, latency <5ms)');
    },

    // NIO WebSocket mode - faster communication
    tryNIOMode: function() {
      var self = this;
      return new Promise(function(resolve, reject) {
        if (typeof NIOChannel === 'undefined') {
          reject(new Error('NIO not available'));
          return;
        }

        console.log('[NIO] Connecting...');
        var channel = new NIOChannel(self.deviceUdid);

        channel.connect()
          .then(function() {
            console.log('[NIO] Connected, enabling NIO mode');
            self.nioChannel = channel;
            self.useNIOMode = true;

            // Start screenshot stream
            self.openNIOScreenStream();
            // Enable NIO touch
            self.enableNIOTouch();

            resolve();
          })
          .catch(function(err) {
            console.log('[NIO] Connection failed:', err);
            reject(err);
          });
      });
    },

    // NIO mode screen stream
    openNIOScreenStream: function() {
      var self = this;
      var canvas = document.getElementById('bgCanvas');
      var ctx = canvas.getContext('2d');
      var lastWidth = 0, lastHeight = 0;
      var nioFrameCount = 0;
      var nioStartTime = Date.now();
      var nioLastLogTime = Date.now();

      // Listen for screenshot events - binary blob direct to canvas, zero base64 overhead
      self.nioChannel.on('screenshot', function(msg) {
        if (msg.status !== 'ok') return;
        var t0 = performance.now();

        // Binary blob (from binary WebSocket frame) or base64 fallback
        var source = msg.blob || null;
        var isBinary = !!source;
        if (!source && msg.data) {
          source = b64toBlob(msg.data, 'image/jpeg');
        }
        if (!source) return;

        var blobSize = source.size || 0;

        // createImageBitmap decodes in background thread, does not block main thread
        createImageBitmap(source).then(function(bitmap) {
          var t1 = performance.now();
          // Render on next vsync
          requestAnimationFrame(function() {
            // Only update canvas on size change
            if (bitmap.width !== lastWidth || bitmap.height !== lastHeight) {
              canvas.width = bitmap.width;
              canvas.height = bitmap.height;
              lastWidth = bitmap.width;
              lastHeight = bitmap.height;
              self.resizeScreen(bitmap);
            }

            ctx.drawImage(bitmap, 0, 0);
            bitmap.close();
            window.app.loading = false;

            var t2 = performance.now();
            nioFrameCount++;

            // Log every 20 frames
            var now = Date.now();
            if (now - nioLastLogTime >= 2000) {
              var elapsed = (now - nioStartTime) / 1000;
              var avgFps = nioFrameCount / elapsed;
              console.log(
                '[NIO] frame#' + nioFrameCount +
                ' | ' + (isBinary ? 'binary' : 'base64') +
                ' | decode=' + (t1 - t0).toFixed(0) + 'ms' +
                ' | render=' + (t2 - t1).toFixed(0) + 'ms' +
                ' | total=' + (t2 - t0).toFixed(0) + 'ms' +
                ' | ' + Math.round(blobSize / 1024) + 'KB' +
                ' | avg ' + avgFps.toFixed(1) + 'fps'
              );
              nioLastLogTime = now;
            }
          });
        });
      });

      // Subscribe to screenshot stream, 50ms interval
      self.nioChannel.subscribe('screenshot', { interval: 50 });
      self.screenWS = {
        close: function() {
          self.nioChannel.unsubscribe('screenshot');
        }
      };

      console.log('[NIO] Screen stream started');
    },

    // NIO mode touch
    enableNIOTouch: function() {
      var self = this;
      var element = this.canvas.fg;
      var screen = { bounds: {} };
      var touchStart = null;

      function calculateBounds() {
        var el = element;
        screen.bounds.w = el.offsetWidth;
        screen.bounds.h = el.offsetHeight;
        screen.bounds.x = 0;
        screen.bounds.y = 0;
        while (el.offsetParent) {
          screen.bounds.x += el.offsetLeft;
          screen.bounds.y += el.offsetTop;
          el = el.offsetParent;
        }
      }

      function activeFinger(index, x, y) {
        $(".finger-" + index).addClass("active")
          .css("transform", 'translate3d(' + x + 'px,' + y + 'px,0)');
      }

      function deactiveFinger(index) {
        $(".finger-" + index).removeClass("active");
      }

      element.addEventListener('mousedown', function(e) {
        if (e.which === 3) return;
        e.preventDefault();
        calculateBounds();

        var x = e.pageX - screen.bounds.x;
        var y = e.pageY - screen.bounds.y;
        touchStart = {
          xP: x / screen.bounds.w,
          yP: y / screen.bounds.h,
          pageX: e.pageX,
          pageY: e.pageY
        };
        activeFinger(0, e.pageX, e.pageY);

        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
      });

      function onMouseMove(e) {
        if (!touchStart) return;
        e.preventDefault();
        activeFinger(0, e.pageX, e.pageY);
      }

      function onMouseUp(e) {
        if (!touchStart) return;
        e.preventDefault();
        deactiveFinger(0);

        var canvas = document.getElementById('bgCanvas');
        var x = Math.floor(touchStart.xP * canvas.width);
        var y = Math.floor(touchStart.yP * canvas.height);

        // Use absolute pixel distance for reliable swipe detection
        var pixelDx = Math.abs(e.pageX - touchStart.pageX);
        var pixelDy = Math.abs(e.pageY - touchStart.pageY);

        if (pixelDx > 10 || pixelDy > 10) {
          // Swipe — compute end position from mouseUp event directly
          calculateBounds();
          var endX = e.pageX - screen.bounds.x;
          var endY = e.pageY - screen.bounds.y;
          // Clamp to canvas bounds (mouse may exit canvas during swipe)
          endX = Math.max(0, Math.min(screen.bounds.w - 1, endX));
          endY = Math.max(0, Math.min(screen.bounds.h - 1, endY));
          var x2 = Math.floor((endX / screen.bounds.w) * canvas.width);
          var y2 = Math.floor((endY / screen.bounds.h) * canvas.height);
          console.log('[NIO] Swipe:', x, y, '->', x2, y2, '(px:', pixelDx, pixelDy, ')');
          self.nioChannel.send('swipe', { x1: x, y1: y, x2: x2, y2: y2 });
        } else {
          // Click
          console.log('[NIO] Touch:', x, y);
          self.nioChannel.send('touch', { x: x, y: y });
        }

        touchStart = null;
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
      }

      console.log('[NIO] Touch enabled');
    },

    // Keyboard input functionality
    enableKeyboardInput: function() {
      var self = this;

      // Listen for keyboard events
      document.addEventListener('keydown', function(e) {
        // If focus is in an input field, do not handle
        if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') {
          return;
        }

        // Special key handling
        var specialKeys = ['Enter', 'Backspace', 'Delete', 'Tab', 'Escape',
                          'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight',
                          'Home', 'End'];

        if (specialKeys.includes(e.key)) {
          e.preventDefault();
          self.sendKeyEvent(e.key);
          return;
        }

        // Regular character input
        if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
          e.preventDefault();
          self.sendTextInput(e.key);
        }
      });

      console.log('Keyboard input enabled - type directly to input to phone');
    },

    sendTextInput: function(text) {
      var self = this;
      $.ajax({
        url: '/inspector/' + self.deviceUdid + '/input',
        method: 'POST',
        contentType: 'application/json',
        data: JSON.stringify({ text: text })
      }).fail(function(err) {
        console.log('Input failed:', err);
      });
    },

    sendKeyEvent: function(key) {
      var self = this;
      $.ajax({
        url: '/inspector/' + self.deviceUdid + '/keyevent',
        method: 'POST',
        contentType: 'application/json',
        data: JSON.stringify({ key: key })
      }).fail(function(err) {
        console.log('Key press failed:', err);
      });
    },

    // Send input text to phone
    sendTextToPhone: function() {
      var self = this;
      var text = this.inputText;
      console.log('sendTextToPhone called, text:', text);
      if (!text) {
        console.log('text is empty, returning');
        return;
      }

      var startTime = Date.now();
      console.log('Sending to:', '/inspector/' + self.deviceUdid + '/input');
      $.ajax({
        url: '/inspector/' + self.deviceUdid + '/input',
        method: 'POST',
        contentType: 'application/json',
        data: JSON.stringify({ text: text })
      }).done(function(response) {
        self.updateCommandLatency(Date.now() - startTime);
        console.log('Input succeeded:', response);
        self.inputText = '';  // Clear input field
      }).fail(function(err) {
        console.log('Input failed:', err);
        alert('Input failed: ' + JSON.stringify(err));
      });
    },

    // Send backspace key
    sendBackspace: function() {
      var self = this;
      $.ajax({
        url: '/inspector/' + self.deviceUdid + '/keyevent',
        method: 'POST',
        contentType: 'application/json',
        data: JSON.stringify({ key: 'Backspace' })
      });
    },

    // Upload file to phone
    uploadFile: function(event) {
      var self = this;
      var file = event.target.files[0];
      if (!file) return;

      self.uploadStatus = 'Uploading: ' + file.name + '...';

      var formData = new FormData();
      formData.append('file', file);

      $.ajax({
        url: '/inspector/' + self.deviceUdid + '/upload',
        method: 'POST',
        data: formData,
        processData: false,
        contentType: false
      }).done(function(response) {
        self.uploadStatus = '✓ ' + response.message;
        setTimeout(function() { self.uploadStatus = ''; }, 5000);
      }).fail(function(err) {
        self.uploadStatus = '✗ Upload failed';
        console.log('Upload failed:', err);
      });

      // Clear file selection, allow re-selecting the same file
      event.target.value = '';
    },

    loadWhatsinput(callback) {
      // Whatsinput requires direct device access on port 6677 — not available in proxied mode
      console.log("[whatsinput] Skipped — direct device access not available in server-proxied mode");
      let defer = $.Deferred();
      defer.reject();
      return defer;
    },
    sendInputText() {
        let ws = this.websockets.winput;
        if (!ws) { console.warn("[whatsinput] Not connected — input sync disabled"); return; }
        console.log("sync", this.whatsinput.text);
        ws.send(JSON.stringify({
          type: "InputEdit",
          text: this.whatsinput.text,
        }))
    },
    sendInputKey(key) {
      let ws = this.websockets.winput;
      if (!ws) { console.warn("[whatsinput] Not connected — key sync disabled"); return; }
      console.log("Sync key", key)
      let code = { "enter": 66, "tab": 61 }[key] || key;
      ws.send(JSON.stringify({
        type: "InputKey",
        code: "" + code,
      }))
    },
    runShell(command) {
      return $.ajax({
        method: "get",
        url: "/inspector/" + this.deviceUdid + "/shell",
        data: {
          "command": command,
        },
        dataType: "json"
      }).then(ret => {
        console.log("runShell", command, ret)
        return ret;
      })
    },
    nodes: function () {
      return this.originNodes
    },
    // elem: function () {
    //   return this.nodeSelected || {};
    // },
    screenDumpUI: function () {
      var self = this;
      this.loading = true;
      this.canvasStyleTree.opacity = 0.5;
      return this.screenRefresh()
        .fail(function (ret) {
          self.showAjaxError(ret);
        })
        .then(function () {
          return $.getJSON('/inspector/' + deviceUdid + '/hierarchy')
        })
        .fail(function (ret) {
          self.showAjaxError(ret);
        })
        .then(function (source) {
          localStorage.setItem('windowHierarchy', JSON.stringify(source));
          self.drawAllNodeFromSource(source);
        })
    },
    drawAllNodeFromSource: function (source) {
      var jstreeData = this.sourceToJstree(source);
      var jstree = this.$jstree.jstree(true);
      jstree.settings.core.data = jstreeData;
      jstree.refresh();

      var nodeMaps = this.originNodeMaps = {}

      function sourceToNodes(source) {
        var node = Object.assign({}, source, { children: undefined });
        nodeMaps[node.id] = node;
        var nodes = [node];
        if (source.children) {
          source.children.forEach(function (s) {
            nodes = nodes.concat(sourceToNodes(s))
          })
        }
        return nodes;
      }
      this.originNodes = sourceToNodes(source) //ret.nodes;
      this.drawAllNode();
      this.loading = false;
      this.canvasStyleTree.opacity = 1.0;
    },
    sourceToJstree: function (source) {
      var n = {}
      n.id = source.id;
      n.text = source.type || source.className
      if (source.name) {
        n.text += " - " + source.name;
      }
      if (source.resourceId) {
        n.text += " - " + source.resourceId;
      }
      n.icon = this.sourceTypeIcon(source.type);
      if (source.children) {
        n.children = []
        source.children.forEach(function (s) {
          n.children.push(this.sourceToJstree(s))
        }.bind(this))
      }
      return n;
    },
    sourceTypeIcon: function (widgetType) {
      switch (widgetType) {
        case "Scene":
          return "glyphicon glyphicon-tree-conifer"
        case "Layer":
          return "glyphicon glyphicon-equalizer"
        case "Camera":
          return "glyphicon glyphicon-facetime-video"
        case "Node":
          return "glyphicon glyphicon-leaf"
        case "ImageView":
          return "glyphicon glyphicon-picture"
        case "Button":
          return "glyphicon glyphicon-inbox"
        case "Layout":
          return "glyphicon glyphicon-tasks"
        case "Text":
          return "glyphicon glyphicon-text-size"
        default:
          return "glyphicon glyphicon-object-align-horizontal"
      }
    },
    drawRefresh: function () {
      this.drawAllNode()
      if (this.nodeHovered) {
        this.drawNode(this.nodeHovered, "blue")
      }
      if (this.nodeSelected) {
        this.drawNode(this.nodeSelected, "red")
        //selector update
        this.elem=this.nodeSelected
      }
    },
    drawNode: function (node, color, dashed) {
      if (!node || !node.rect) {
        return;
      }
      var x = node.rect.x,
        y = node.rect.y,
        w = node.rect.width,
        h = node.rect.height;
      color = color || 'black';
      var ctx = this.canvas.fg_tree.getContext('2d');
      var rectangle = new Path2D();
      rectangle.rect(x, y, w, h);
      if (dashed) {
        ctx.lineWidth = 1;
        ctx.setLineDash([8, 10]);
      } else {
        ctx.lineWidth = 5;
        ctx.setLineDash([]);
      }
      ctx.strokeStyle = color;
      ctx.stroke(rectangle);
    },
    generateNodeSelectorCode: function (node) {
      var params = this.generateNodeSelectorParams(node);
      return 'd(' + params.join(', ') + ')';
    },
    generateNodeSelectorParams: function (node) {
      var self = this;

      function combineKeyValue(key, value) {
        value = '"' + value + '"';
        if (['text', 'name', 'label', 'description'].indexOf(key) >= 0) {
          value = "u" + value; // python unicode
        }
        return key + '=' + value;
      }
      var index = 0;
      var params = [];
      var kvs = [];
      // iOS: name, label, className
      // Android: text, description, resourceId, className
      ['label', 'resourceId', 'name', 'text', 'type', 'tag', 'description', 'className'].some(function (key) {
        if (!node[key]) {
          return false;
        }
        params.push(combineKeyValue(key, node[key]));
        kvs.push([key, node[key]]);
        index = self.getNodeIndex(node.id, kvs);
        return self.codeShortFlag && index == 0;
      });
      if (index > 0) {
        params.push('instance=' + index);
      }
      return params;
    },
    generateNodeSelectorCode: function (node) {
      var params = this.generateNodeSelectorParams(node);
      return 'd(' + params.join(', ') + ')';
    },
    getNodeIndex: function (id, kvs) {
      var skip = false;
      return this.nodes().filter(function (node) {
        if (skip) {
          return false;
        }
        var ok = kvs.every(function (kv) {
          var k = kv[0],
            v = kv[1];
          return node[k] == v;
        })
        if (ok && id == node.id) {
          skip = true;
        }
        return ok;
      }).length - 1;
    },
    screenRefresh: function () {
      return $.getJSON('/inspector/' + deviceUdid + '/screenshot')
        .then(function (ret) {
          var blob = b64toBlob(ret.data, 'image/' + ret.type);
          this._drawBlobImageToScreen(blob);
          localStorage.setItem('screenshotBase64', ret.data);
        }.bind(this))
    },
    _drawBlobImageToScreen: function (blob) {
      // Support jQuery Promise
      var dtd = $.Deferred();
      var bgcanvas = this.canvas.bg_tree,
        fgcanvas = this.canvas.fg_tree,
        ctx = bgcanvas.getContext('2d'),
        self = this,
        URL = window.URL || window.webkitURL,
        BLANK_IMG = 'data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw==',
        img = this.imagePool.next();

      img.onload = function () {
        console.log("image")
        fgcanvas.width = bgcanvas.width=img.width
        fgcanvas.height = bgcanvas.height=img.height


        ctx.drawImage(img, 0, 0, img.width, img.height);
        self.resizeScreenTree(img);

        // Try to forcefully clean everything to get rid of memory
        // leaks. Note self despite this effort, Chrome will still
        // leak huge amounts of memory when the developer tools are
        // open, probably to save the resources for inspection. When
        // the developer tools are closed no memory is leaked.
        img.onload = img.onerror = null
        img.src = BLANK_IMG
        img = null
        blob = null

        URL.revokeObjectURL(url)
        url = null
        dtd.resolve();
      }

      img.onerror = function () {
        // Happily ignore. I suppose this shouldn't happen, but
        // sometimes it does, presumably when we're loading images
        // too quickly.

        // Do the same cleanup here as in onload.
        img.onload = img.onerror = null
        img.src = BLANK_IMG
        img = null
        blob = null

        URL.revokeObjectURL(url)
        url = null
        dtd.reject();
      }
      var url = URL.createObjectURL(blob)
      img.src = url;
      return dtd;
    },
    drawAllNode: function () {

      if (this.originNodes==undefined){
        return
      }
      var self = this;
      var canvas = self.canvas.fg_tree;
      var ctx = canvas.getContext('2d');
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      self.nodes().forEach(function (node) {
        // ignore some types
        if (['Layout'].includes(node.type)) {
          return;
        }
        self.drawNode(node, 'black', true);
      })
    },
    drawHoverNode: function (pos) {
      if(this.originNodes==undefined){
        return
      }
      var self = this;
      var canvas = self.canvas.fg_tree;
      self.nodeHovered = null;
      var minArea = Infinity;

      function isInside(node, x, y) {
        if (!node.rect) {
          return false;
        }
        var lx = node.rect.x,
          ly = node.rect.y,
          rx = node.rect.width + lx,
          ry = node.rect.height + ly;
        return lx < x && x < rx && ly < y && y < ry;
      }
      var activeNodes = self.nodes().filter(function (node) {
        if (!isInside(node, pos.x, pos.y)) {
          return false;
        }
        if (!node.rect) {
          return false;
        }
        // skip some types
        console.log(node.type);
        if (['Layout', 'Sprite'].includes(node.type)) {
          return false;
        }
        var area = node.rect.width * node.rect.height;
        if (area <= minArea) {
          minArea = area;
          self.nodeHovered = node;
        }
        return true;
      })
      activeNodes.forEach(function (node) {
        self.drawNode(node, "blue", true)
      })
      self.drawNode(self.nodeHovered, "blue");
    },
    checkVersion: function () {
      var self = this;
      $.get("/api/v1/version", function (ret) {
        console.log("Server version:", ret.data.name, ret.data.version, "(" + ret.data.server + ")");
      }).fail(function () {
        self.showError("<p>Server not reachable</p>");
      });
    },
    activeMouseControl: function () {
      var self = this;
      var element = this.canvas.fg_tree;

      var screen = {
        bounds: {}
      }

      function calculateBounds() {
        var el = element;
        screen.bounds.w = el.offsetWidth
        screen.bounds.h = el.offsetHeight
        screen.bounds.x = 0
        screen.bounds.y = 0

        while (el.offsetParent) {
          screen.bounds.x += el.offsetLeft
          screen.bounds.y += el.offsetTop
          el = el.offsetParent
        }
      }

      function activeFinger(index, x, y, pressure) {
        var scale = 0.5 + pressure
        $(".finger-" + index)
          .addClass("active")
          .css("transform", 'translate3d(' + x + 'px,' + y + 'px,0)')
      }

      function deactiveFinger(index) {
        $(".finger-" + index).removeClass("active")
      }

      function mouseMoveListener(event) {
        var e = event
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault()

        var pressure = 0.5
        activeFinger(0, e.pageX, e.pageY, pressure);
        // that.touchMove(0, x / screen.bounds.w, y / screen.bounds.h, pressure);
      }

      function mouseUpListener(event) {
        var e = event
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault()

        var pos = coord(e);
        // change precision
        pos.px = Math.floor(pos.px * 1000) / 1000;
        pos.py = Math.floor(pos.py * 1000) / 1000;
        pos.x = Math.floor(pos.px * element.width);
        pos.y = Math.floor(pos.py * element.height);
        self.cursor = pos;
        markPosition(self.cursor)

        stopMousing()
      }

      function stopMousing() {
        element.removeEventListener('mousemove', mouseMoveListener);
        element.addEventListener('mousemove', mouseHoverListener);
        document.removeEventListener('mouseup', mouseUpListener);
        deactiveFinger(0);
      }

      function coord(event) {
        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        calculateBounds()
        var x = e.pageX - screen.bounds.x
        var y = e.pageY - screen.bounds.y
        var px = x / screen.bounds.w;
        var py = y / screen.bounds.h;
        return {
          px: px,
          py: py,
          x: Math.floor(px * element.width),
          y: Math.floor(py * element.height),
        }
      }

      function mouseHoverListener(event) {
        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault()
        // startMousing()

        var x = e.pageX - screen.bounds.x
        var y = e.pageY - screen.bounds.y
        var pos = coord(event);

        self.drawAllNode()
        if (self.nodeSelected) {
          self.drawNode(self.nodeSelected, "red");
        }
        self.drawHoverNode(pos);
        if (self.cursor.px) {
          markPosition(self.cursor)
        }
      }

      // Screen click
      function mouseDownListener(event) {
        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault()

        fakePinch = e.altKey
        calculateBounds()
        // startMousing()

        var x = e.pageX - screen.bounds.x
        var y = e.pageY - screen.bounds.y
        var pressure = 0.5
        activeFinger(0, e.pageX, e.pageY, pressure);

        if (self.nodeHovered) {
          self.nodeSelected = self.nodeHovered;
          self.drawAllNode();
          // self.drawHoverNode(pos);
          self.drawNode(self.nodeSelected, "red");
          var generatedCode = self.generateNodeSelectorCode(self.nodeSelected);
          if (self.autoCopy) {
            copyToClipboard(generatedCode);
          }
          self.generatedCode = generatedCode;
          // self.editor.setValue(generatedCode);

          self.$jstree.jstree("deselect_all");
          self.$jstree.jstree("close_all");
          self.$jstree.jstree("select_node", "#" + self.nodeHovered.id);
          self.$jstree.jstree(true)._open_to("#" + self.nodeHovered.id);
          document.getElementById(self.nodeHovered.id).scrollIntoView(false);
          self.elem=self.nodeSelected
        }
        // self.touchDown(0, x / screen.bounds.w, y / screen.bounds.h, pressure);

        element.removeEventListener('mousemove', mouseHoverListener);
        element.addEventListener('mousemove', mouseMoveListener);
        document.addEventListener('mouseup', mouseUpListener);
      }

      function markPosition(pos) {
        var ctx = self.canvas.fg.getContext("2d");
        ctx.fillStyle = '#ff0000'; // red
        ctx.beginPath()
        ctx.arc(pos.x, pos.y, 12, 0, 2 * Math.PI)
        ctx.closePath()
        ctx.fill()

        ctx.fillStyle = "#fff"; // white
        ctx.beginPath()
        ctx.arc(pos.x, pos.y, 8, 0, 2 * Math.PI)
        ctx.closePath()
        ctx.fill();
      }

      /* bind listeners */
      element.addEventListener('mousedown', mouseDownListener);
      element.addEventListener('mousemove', mouseHoverListener);
    },
    initJstree: function () {
      var $jstree = $("#jstree-hierarchy");
      this.$jstree = $jstree;
      var self = this;
      $jstree.jstree({
        plugins: ["search"],
        core: {
          multiple: false,
          themes: {
            "variant": "small"
          },
          data: []
        }
      })
        .on('ready.jstree refresh.jstree', function () {
          $jstree.jstree("open_all");
        })
        .on("changed.jstree", function (e, data) {
          var id = data.selected[0];
          var node = self.originNodeMaps[id];
          if (node) {
            self.nodeSelected = node;
            self.drawAllNode();
            self.drawNode(node, "red");
            var generatedCode = self.generateNodeSelectorCode(self.nodeSelected);
            if (self.autoCopy) {
              copyToClipboard(generatedCode);
            }
            self.generatedCode = generatedCode;
            self.elem=self.nodeSelected
          }
        })
        .on("hover_node.jstree", function (e, data) {
          var node = self.originNodeMaps[data.node.id];
          if (node) {
            self.nodeHovered = node;
            self.drawRefresh()
          }
        })
        .on("dehover_node.jstree", function () {
          self.nodeHovered = null;
          self.drawRefresh()
        })
      $("#jstree-search").on('propertychange input', function (e) {
        var ret = $jstree.jstree(true).search($(this).val());
      })
    },
    reserveDevice: function () {
      var dtd = $.Deferred();
      var ws = new WebSocket("ws://" + location.host + "/devices/" + this.deviceUdid + "/reserved")
      ws.onmessage = function (message) {
        console.log("WebSocket receive", message)
      }
      var key = setInterval(function () {
        ws.send("ping")
      }, 5000);
      ws.onopen = function () {
        dtd.resolve();
      }
      ws.onerror = function (err) {
        console.log("WebSocket Error " + err)
      }
      ws.onclose = function () {
        dtd.reject();
        clearInterval(key);
        console.log("websocket reserved closed");
      }
      return dtd.promise();
    },
    toggleScreen: function () {
      if (this.screenWS) {
        this.screenWS.close();
        this.canvasStyle.opacity = 0;
        this.screenWS = null;
      } else {
        this.openScreenStream();
        this.canvasStyle.opacity = 1;
      }
    },
    connectImage2VideoWebSocket: function (fps) {
      var self = this;
      var protocol = location.protocol == "http:" ? "ws:" : "wss:";
      var wsURL = protocol + "//" + location.host + "/video/convert";
      var wsQueries = "fps=" + fps + "&udid=" + encodeURIComponent(this.deviceUdid) + "&name=" + encodeURIComponent(this.deviceInfo.model || '');
      var ws = new WebSocket(wsURL + "?" + wsQueries);
      var def = $.Deferred();
      ws.onopen = function () {
        def.resolve(ws);
      };
      ws.onmessage = function (evt) {
        try {
          var msg = JSON.parse(evt.data);
          if (msg.type === 'recording_stopped' && msg.id) {
            $.notify("Recording saved", "success");
            self.loadVideoRecordings();
          }
        } catch (e) { /* ignore non-JSON messages */ }
      };
      ws.onclose = function () {
        def.reject("WebSocket disconnected");
        self.loadVideoRecordings();
      };
      ws.onerror = function () {
        def.reject("WebSocket error");
      };
      return def.promise();
    },
    startScreenRecord: function () {
      var self = this;
      this.connectImage2VideoWebSocket(2)
        .done(function (ws) {
          var key = setInterval(function () {
            $.ajax({
              url: self.deviceUrl + "/screenshot/0?thumbnail=800x800",
              method: "get",
              processData: false,
              cache: false,
              xhr: function () {
                var xhr = new XMLHttpRequest();
                xhr.responseType = "blob";
                return xhr;
              },
              success: function (data) {
                if (ws.readyState === WebSocket.OPEN) {
                  ws.send(data);
                }
              }
            });
          }, 500); // ~2 FPS
          self.videoReceiver = { ws: ws, key: key };
        })
        .fail(function (err) {
          console.error("Video recording failed:", err);
        });
    },
    stopScreenRecord: function () {
      if (this.videoReceiver) {
        clearInterval(this.videoReceiver.key);
        if (this.videoReceiver.ws && this.videoReceiver.ws.readyState === WebSocket.OPEN) {
          this.videoReceiver.ws.close();
        }
        this.videoReceiver = null;
      }
    },
    toggleVideoRecord: function () {
      if (this.videoReceiver) {
        this.stopScreenRecord();
      } else {
        this.startScreenRecord();
      }
    },
    loadVideoRecordings: function () {
      var self = this;
      $.getJSON('/api/v1/videos?udid=' + encodeURIComponent(this.deviceUdid), function (resp) {
        if (resp.status === 'success' && resp.data) {
          self.videoRecordings = resp.data;
        }
      });
    },
    deleteVideo: function (id) {
      if (!confirm('Delete this video recording?')) return;
      var self = this;
      $.ajax({
        url: '/api/v1/videos/' + id,
        method: 'DELETE',
        success: function () {
          $.notify("Video deleted", "success");
          self.loadVideoRecordings();
        },
        error: function () {
          $.notify("Failed to delete video", "error");
        }
      });
    },
    formatDuration: function (ms) {
      if (!ms) return '—';
      var seconds = Math.floor(ms / 1000);
      var minutes = Math.floor(seconds / 60);
      seconds = seconds % 60;
      return minutes + ':' + (seconds < 10 ? '0' : '') + seconds;
    },
    formatFileSize: function (bytes) {
      if (!bytes) return '—';
      if (bytes < 1024) return bytes + ' B';
      if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
      return (bytes / 1048576).toFixed(1) + ' MB';
    },
    saveScreenshot: function () {
      $.ajax({
        url: "/inspector/" + this.deviceUdid + "/screenshot/img",
        cache: false,
        xhrFields: {
          responseType: 'blob'
        },
      }).then(function (blob) {
        saveAs(blob, "screenshot.jpg") // saveAs require FileSaver.js
      })
    },
    openBrowser: function (url) {
      if (!/^https?:\/\//.test(url)) {
        url = "http://" + url;
      }
      return this.shell("am start -a android.intent.action.VIEW -d " + url);
    },
    uploadFile: function (event) {
      var formData = new FormData(event.target);
      $(event.target).notify("Uploading ...");
      $.ajax({
        method: "post",
        url: "/inspector/" + this.deviceUdid + "/upload",
        data: formData,
        processData: false,
        contentType: false,
        enctype: 'multipart/form-data',
      }).then(function (ret) {
          $(event.target).notify("Upload success");
        }, function (err) {
          $(event.target).notify("Upload failed:" + err.responseText, "error")
          console.error(err)
        })
    },
    addTabItem: function (item) {
      this.navtabs.tabs.push(item);
    },
    changeTab: function (tabId) {
      location.hash = tabId;
    },
    fixRotation: function () {
      $.ajax({
        url: "/inspector/" + this.deviceUdid + "/rotation",
        method: "post",
      }).then(function (ret) {
        console.log("rotation fixed")
      })
    },
    tabScroll: function (ev) {
      // var el = ev.target;
      // var el = this.$refs.tab_content;
      // var bottom = (el.scrollTop == (el.scrollHeight - el.clientHeight));
      // console.log("Bottom", bottom, el.scrollTop, el.scrollHeight, el.clientHeight, el.scrollHeight - el.clientHeight)
      // console.log(ev.target.scrollTop, ev.target.scrollHeight, ev.target.clientHeight);
      this.logcat.follow = false;
    },
    followLog: function () {
      this.logcat.follow = !this.logcat.follow;
      if (this.logcat.follow) {
        var el = this.$refs.tab_content;
        el.scrollTop = el.scrollHeight - el.clientHeight;
      }
    },
    logcatTag2Color: function (tag) {
      var color = this.logcat.tagColors[tag];
      if (!color) {
        color = this.logcat.tagColors[tag] = getRandomRgb(5);
      }
      return color;
    },
    logcatLevel2Color: function (level) {
      switch (level) {
        case "W":
          return "goldenrod";
        case "I":
          return "darkgreen";
        case "D":
          return "gray";
        default:
          return "gray";
      }
    },
    hold: function (msecs) {
      this.control.touchDown(0, 0.5, 0.5, 5, 0.5)
      this.control.touchCommit();
      this.control.touchWait(msecs);
      this.control.touchUp(0)
      this.control.touchCommit();
    },
    keyevent: function (meta) {
      var self = this;
      console.log("keyevent", meta);

      // Use backend proxy API to avoid CORS issues
      return $.ajax({
        url: '/inspector/' + self.deviceUdid + '/keyevent',
        method: 'POST',
        contentType: 'application/json',
        data: JSON.stringify({ key: meta })
      }).done(function(ret) {
        console.log("keyevent succeeded:", ret);
      }).fail(function(err) {
        console.log("keyevent failed:", err);
      });
    },
    shell: function (command) {
      return $.ajax({
        url: "/inspector/" + this.deviceUdid + "/shell",
        method: "post",
        data: {
          command: command,
        },
        success: function (ret) {
          console.log(ret);
        },
        error: function (ret) {
          console.log(ret)
        }
      })
    },
    showError: function (error) {
      this.loading = false;
      this.error = error;
      $('.modal').modal('show');
    },
    showAjaxError: function (ret) {
      if (ret.responseJSON && ret.responseJSON.description) {
        this.showError(ret.responseJSON.description);
      } else {
        this.showError("<p>Server not reachable</p>");
      }
    },
    // Left screen drag
    initDragDealer: function () {
      var self = this;
      var updateFunc = null;

      function dragMoveListener(evt) {
        evt.preventDefault();
        updateFunc(evt);
        self.resizeScreen();
      }

      function dragStopListener(evt) {
        document.removeEventListener('mousemove', dragMoveListener);
        document.removeEventListener('mouseup', dragStopListener);
        document.removeEventListener('mouseleave', dragStopListener);
      }

      $('#vertical-gap1').mousedown(function (e) {
        e.preventDefault();
        updateFunc = function (evt) {
          $("#left").width(evt.clientX);
        };
        document.addEventListener('mousemove', dragMoveListener);
        document.addEventListener('mouseup', dragStopListener);
        document.addEventListener('mouseleave', dragStopListener)
      });
    },
    resizeScreen: function (img) {
      // check if need update
      if (img) {
        if (this.lastScreenSize.canvas.width == img.width &&
          this.lastScreenSize.canvas.height == img.height) {
          return;
        }
      } else {
        img = this.lastScreenSize.canvas;
        if (!img) {
          return;
        }
      }
      var screenDiv = document.getElementById('screen');
      this.lastScreenSize = {
        canvas: {
          width: img.width,
          height: img.height
        },
        screen: {
          width: screenDiv.clientWidth,
          height: screenDiv.clientHeight,
        }
      }

      var screenDiv = document.getElementById('screen');
      this.lastScreenSize = {
        canvas: {
          width: img.width,
          height: img.height
        },
        screen: {
          width: screenDiv.clientWidth,
          height: screenDiv.clientHeight,
        }
      }


      var canvasAspect = img.width / img.height;
      var screenAspect = screenDiv.clientWidth / screenDiv.clientHeight;
      if (canvasAspect > screenAspect) {
        Object.assign(this.canvasStyle, {
          width: Math.floor(screenDiv.clientWidth) + 'px', //'100%',
          height: Math.floor(screenDiv.clientWidth / canvasAspect) + 'px', // 'inherit',
        })
      } else if (canvasAspect < screenAspect) {
        Object.assign(this.canvasStyle, {
          width: Math.floor(screenDiv.clientHeight * canvasAspect) + 'px', //'inherit',
          height: Math.floor(screenDiv.clientHeight) + 'px', //'100%',
        })
      }
    },

    resizeScreenTree: function (img) {
      // check if need update
      if (img) {
        if (this.lastScreenSize.canvas.width == img.width &&
          this.lastScreenSize.canvas.height == img.height) {
          return;
        }
      } else {
        img = this.lastScreenSize.canvas;
        if (!img) {
          return;
        }
      }
      var screenDiv = document.getElementById('screen_tree');
      this.lastScreenSize = {
        canvas: {
          width: img.width,
          height: img.height
        },
        screen: {
          width: screenDiv.clientWidth,
          height: screenDiv.clientHeight,
        }
      }

      var screenDiv = document.getElementById('screen_tree');
      this.lastScreenSize = {
        canvas: {
          width: img.width,
          height: img.height
        },
        screen: {
          width: screenDiv.clientWidth,
          height: screenDiv.clientHeight,
        }
      }


      var canvasAspect = img.width / img.height;
      var screenAspect = screenDiv.clientWidth / screenDiv.clientHeight;
      if (canvasAspect > screenAspect) {
        Object.assign(this.canvasStyleTree, {
          width: Math.floor(screenDiv.clientWidth) + 'px', //'100%',
          height: Math.floor(screenDiv.clientWidth / canvasAspect) + 'px', // 'inherit',
        })
      } else if (canvasAspect < screenAspect) {
        Object.assign(this.canvasStyleTree, {
          width: Math.floor(screenDiv.clientHeight * canvasAspect) + 'px', //'inherit',
          height: Math.floor(screenDiv.clientHeight) + 'px', //'100%',
        })
      }
    },

    delayReload: function (msec) {
      setTimeout(this.screenDumpUI, msec || 1000);
    },
    openScreenStream: function () {
      var self = this;
      var canvas = document.getElementById('bgCanvas');
      var ctx = canvas.getContext('2d');
      var running = true;
      var lastWidth = 0, lastHeight = 0;
      var httpFrameCount = 0;
      var httpStartTime = Date.now();
      var httpLastLogTime = Date.now();

      self.screenWS = { close: function() { running = false; } };

      // Raw JPEG endpoint — no JSON parse, no base64 decode overhead
      var screenshotUrl = '/inspector/' + self.deviceUdid + '/screenshot/img?q=60&s=0.6';

      // ═══════════════════════════════════════════════
      //  Pipeline Double Buffering
      // ═══════════════════════════════════════════════

      // Fetch one frame: returns Promise<{bitmap, latency, fetchTime, decodeTime}>
      function fetchFrame() {
        var t0 = performance.now();
        var t_fetch_done;
        return fetch(screenshotUrl)
          .then(function(resp) {
            if (!resp.ok) throw new Error('HTTP ' + resp.status);
            return resp.blob();
          })
          .then(function(blob) {
            t_fetch_done = performance.now();
            return createImageBitmap(blob).then(function(bitmap) {
              var t_decode_done = performance.now();
              return {
                bitmap: bitmap,
                latency: t_decode_done - t0,
                fetchTime: t_fetch_done - t0,
                decodeTime: t_decode_done - t_fetch_done,
                size: blob.size
              };
            });
          });
      }

      // Pipeline core: process current frame while starting next frame fetch
      function pipeline(frameProm) {
        if (!running) return;

        frameProm.then(function(frame) {
          if (!running) { frame.bitmap.close(); return; }

          // Step 1: Immediately start fetching next frame (without waiting for current frame to render)
          var nextProm = fetchFrame();

          // Step 2: Render current frame on next vsync
          requestAnimationFrame(function() {
            if (!running) { frame.bitmap.close(); return; }

            // Only update canvas on size change (avoid layout reflow)
            if (frame.bitmap.width !== lastWidth || frame.bitmap.height !== lastHeight) {
              canvas.width = frame.bitmap.width;
              canvas.height = frame.bitmap.height;
              lastWidth = frame.bitmap.width;
              lastHeight = frame.bitmap.height;
              self.resizeScreen(frame.bitmap);
            }

            // Draw frame
            ctx.drawImage(frame.bitmap, 0, 0);
            frame.bitmap.close(); // Release GPU memory

            window.app.loading = false;
            self.updateScreenshotLatency(Math.round(frame.latency));

            httpFrameCount++;
            var now = Date.now();
            if (now - httpLastLogTime >= 2000) {
              var elapsed = (now - httpStartTime) / 1000;
              var avgFps = httpFrameCount / elapsed;
              console.log(
                '[HTTP] frame#' + httpFrameCount +
                ' | fetch=' + frame.fetchTime.toFixed(0) + 'ms' +
                ' | decode=' + frame.decodeTime.toFixed(0) + 'ms' +
                ' | total=' + frame.latency.toFixed(0) + 'ms' +
                ' | ' + Math.round(frame.size / 1024) + 'KB' +
                ' | avg ' + avgFps.toFixed(1) + 'fps'
              );
              httpLastLogTime = now;
            }
          });

          // Step 3: Continue pipeline (repeat when next frame arrives)
          pipeline(nextProm);

        }).catch(function(err) {
          // Network error, retry after brief wait
          if (running) {
            console.warn('[HTTP] Screenshot error:', err.message);
            setTimeout(function() { pipeline(fetchFrame()); }, 100);
          }
        });
      }

      // Start pipeline
      pipeline(fetchFrame());
      console.log('[Screenshot] Pipeline double-buffer started (raw JPEG + createImageBitmap)');
    },
    enableTouch: function () {
      /**
       * TOUCH HANDLING - implemented via backend API
       */
      var self = this;
      var element = this.canvas.fg;

      var screen = {
        bounds: {}
      }

      // Simplified touch variables
      var touchStart = null;

      // Simulated control object
      var control = this.control = {
        touchDown: function(id, xP, yP, pressure) {
          touchStart = { xP: xP, yP: yP };
        },
        touchMove: function(id, xP, yP, pressure) {
          if (touchStart) {
            touchStart.endXP = xP;
            touchStart.endYP = yP;
          }
        },
        touchUp: function(id) {
          if (!touchStart) return;

          var canvas = document.getElementById('bgCanvas');
          var x = Math.floor(touchStart.xP * canvas.width);
          var y = Math.floor(touchStart.yP * canvas.height);

          var cmdStartTime = Date.now();
          // Use pixel distance for reliable swipe detection
          var pixelDx = touchStart.pixelDx || 0;
          var pixelDy = touchStart.pixelDy || 0;
          var isSwipe = (pixelDx > 10 || pixelDy > 10);

          if (isSwipe && touchStart.endXP !== undefined) {
            // Clamp to valid range (mouse may exit canvas during swipe)
            var endXP = Math.max(0, Math.min(1, touchStart.endXP));
            var endYP = Math.max(0, Math.min(1, touchStart.endYP));
            // Swipe
            var x2 = Math.floor(endXP * canvas.width);
            var y2 = Math.floor(endYP * canvas.height);
            console.log("[HTTP] Swipe:", x, y, "->", x2, y2, "(px:", pixelDx, pixelDy, ")");
            $.ajax({
              url: '/inspector/' + self.deviceUdid + '/touch',
              method: 'POST',
              contentType: 'application/json',
              data: JSON.stringify({ action: 'swipe', x: x, y: y, x2: x2, y2: y2 })
            }).always(function() {
              self.updateCommandLatency(Date.now() - cmdStartTime);
            });
          } else {
            // Click
            console.log("[HTTP] Touch:", x, y);
            $.ajax({
              url: '/inspector/' + self.deviceUdid + '/touch',
              method: 'POST',
              contentType: 'application/json',
              data: JSON.stringify({ action: 'click', x: x, y: y })
            }).always(function() {
              self.updateCommandLatency(Date.now() - cmdStartTime);
            });
          }
          touchStart = null;
        },
        touchCommit: function() {},
        touchWait: function(ms) {}
      };

      function calculateBounds() {
        var el = element;
        screen.bounds.w = el.offsetWidth
        screen.bounds.h = el.offsetHeight
        screen.bounds.x = 0
        screen.bounds.y = 0

        while (el.offsetParent) {
          screen.bounds.x += el.offsetLeft
          screen.bounds.y += el.offsetTop
          el = el.offsetParent
        }
      }

      function activeFinger(index, x, y, pressure) {
        var scale = 0.5 + pressure
        $(".finger-" + index)
          .addClass("active")
          .css("transform", 'translate3d(' + x + 'px,' + y + 'px,0)')
      }

      function deactiveFinger(index) {
        $(".finger-" + index).removeClass("active")
      }

      function mouseDownListener(event, type) {
        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault();

        fakePinch = e.altKey;
        calculateBounds();
        var x, y, pageX, pageY;
        var pressure = 0.5
        if (type == "touchstart"){
            x = e.targetTouches[0].pageX - screen.bounds.x;
            y = e.targetTouches[0].pageY - screen.bounds.y;
            pageX = e.targetTouches[0].pageX;
            pageY = e.targetTouches[0].pageY;
            element.removeEventListener('touchmove', mouseHoverListener);
            element.addEventListener('touchmove', touchMoveListener);
        }else{
            x = e.pageX - screen.bounds.x;
            y = e.pageY - screen.bounds.y;
            pageX = e.pageX;
            pageY = e.pageY;
            element.removeEventListener('mousemove', mouseHoverListener);
            document.addEventListener('mousemove', mouseMoveListener);
        }

        activeFinger(0, pageX, pageY, pressure);
        // Calculate click coordinates
        var scaled = coords(screen.bounds.w, screen.bounds.h, x, y, self.rotation);
        control.touchDown(0, scaled.xP, scaled.yP, pressure);
        // Store page coordinates for pixel-distance swipe detection
        touchStart.pageX = pageX;
        touchStart.pageY = pageY;
        control.touchCommit();

        document.addEventListener('mouseup', mouseUpListener);
      }

      function touchMoveListener(event) {

        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault();
        var pressure = 0.5;
        activeFinger(0, e.targetTouches[0].pageX, e.targetTouches[0].pageY, pressure);

        var x = e.targetTouches[0].pageX - screen.bounds.x;
        var y = e.targetTouches[0].pageY - screen.bounds.y;
        var scaled = coords(screen.bounds.w, screen.bounds.h, x, y, self.rotation);
        console.log(x,y, scaled, self.rotation);
        control.touchMove(0, scaled.xP, scaled.yP, pressure);
        control.touchCommit();
        document.addEventListener('touchend', mouseUpListener);
      }

      function mouseMoveListener(event) {
        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault();
        var pressure = 0.5;
        activeFinger(0, e.pageX, e.pageY, pressure);
        var x = e.pageX - screen.bounds.x
        var y = e.pageY - screen.bounds.y
        var scaled = coords(screen.bounds.w, screen.bounds.h, x, y, self.rotation);
        console.log(x,y, scaled, self.rotation);
        control.touchMove(0, scaled.xP, scaled.yP, pressure);
        control.touchCommit();
      }

      function mouseUpListener(event) {
        var e = event
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault()

        // Compute pixel distance and set end coords from mouseUp event
        if (touchStart && touchStart.pageX !== undefined) {
          var upPageX = e.pageX || (e.changedTouches && e.changedTouches[0].pageX) || 0;
          var upPageY = e.pageY || (e.changedTouches && e.changedTouches[0].pageY) || 0;
          touchStart.pixelDx = Math.abs(upPageX - touchStart.pageX);
          touchStart.pixelDy = Math.abs(upPageY - touchStart.pageY);
          // Ensure end position is set for swipe (in case mousemove missed it)
          if ((touchStart.pixelDx > 10 || touchStart.pixelDy > 10) && touchStart.endXP === undefined) {
            calculateBounds();
            var endX = upPageX - screen.bounds.x;
            var endY = upPageY - screen.bounds.y;
            var scaled = coords(screen.bounds.w, screen.bounds.h, endX, endY, self.rotation);
            touchStart.endXP = scaled.xP;
            touchStart.endYP = scaled.yP;
          }
        }

        control.touchUp(0)
        control.touchCommit();
        stopMousing()
      }

      function stopMousing() {
        document.removeEventListener('mousemove', mouseMoveListener);
        document.removeEventListener('mouseup', mouseUpListener);
        document.removeEventListener('touchend', mouseUpListener);
        element.removeEventListener('touchmove', touchMoveListener);
        deactiveFinger(0);
      }

      function mouseHoverListener(event) {
        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        // Skip secondary click
        if (e.which === 3) {
          return
        }
        e.preventDefault()

        var x = e.pageX - screen.bounds.x
        var y = e.pageY - screen.bounds.y
      }

      var wheelTimer, fromYP;

      function mouseWheelDelayTouchUp() {
        clearTimeout(wheelTimer);
        wheelTimer = setTimeout(function () {
          fromYP = null;
          control.touchUp(1)
          control.touchCommit();
          // deactiveFinger(0);
          // deactiveFinger(1);
        }, 100)
      }

      function mouseWheelListener(event) {
        var e = event;
        if (e.originalEvent) {
          e = e.originalEvent
        }
        e.preventDefault()
        calculateBounds()

        var x = e.pageX - screen.bounds.x
        var y = e.pageY - screen.bounds.y
        var pressure = 0.5;
        var scaled;

        if (!fromYP) {
          fromYP = y / screen.bounds.h; // display Y percent
          // touch down when first detect mousewheel
          scaled = coords(screen.bounds.w, screen.bounds.h, x, y, self.rotation);
          control.touchDown(1, scaled.xP, scaled.yP, pressure);
          control.touchCommit();
          // activeFinger(0, e.pageX, e.pageY, pressure);
        }
        // caculate position after scroll
        var toYP = fromYP + (event.wheelDeltaY < 0 ? -0.05 : 0.05);
        toYP = Math.max(0, Math.min(1, toYP));

        var step = Math.max((toYP - fromYP) / 5, 0.01) * (event.wheelDeltaY < 0 ? -1 : 1);
        for (var yP = fromYP; yP < 1 && yP > 0 && Math.abs(yP - toYP) > 0.0001; yP += step) {
          y = screen.bounds.h * yP;
          var pageY = y + screen.bounds.y;
          scaled = coords(screen.bounds.w, screen.bounds.h, x, y, self.rotation);
          // activeFinger(1, e.pageX, pageY, pressure);
          control.touchMove(1, scaled.xP, scaled.yP, pressure);
          control.touchWait(10);
          control.touchCommit();
        }
        fromYP = toYP;
        mouseWheelDelayTouchUp()
      }
      // TODO optimize and support mobile browser gestures
      /* bind listeners */
      element.addEventListener('mousedown', function (event){mouseDownListener(event,"mousedown")});
      element.addEventListener('touchstart', function (event){mouseDownListener(event,"touchstart")});
    },
    refreshTopApp() {
            this.runShell("dumpsys activity top").then(ret => {
              const reActivity = String.raw`\s*ACTIVITY ([A-Za-z0-9_.]+)\/([A-Za-z0-9_.]+) \w+ pid=(\d+)`
              let matches = ret.output.match(new RegExp(reActivity, "g"))
              if (matches.length > 0) {
                let m = matches.pop().match(new RegExp(reActivity))
                this.topApp.packageName = m[1];
                this.topApp.activity = m[2]
                this.topApp.pid = m[3]
              }
            })
    },
    addTopAppToShortcut() {
      this.$prompt('Name for ' + this.topApp.packageName, 'Add Shortcut', {
        confirmButtonText: 'OK',
        cancelButtonText: 'Cancel',
      }).then(({ value }) => {
        let command = ["monkey", "-p", this.topApp.packageName, "-c", "android.intent.category.LAUNCHER", "1"].join(" ")
        this.addShortcut(value, command)
      }).catch(() => {
      })
    },
    atxAgentManager(v){
      $.ajax({
        type: "get",
        url: "/atxagent",
        data: {
          "method": v,
          "udid": deviceUdid
        },
        dataType: "json",
        success: function (data) {
           console.log(data)
        },
        error:function (data) {
          console.log(data);
          if(data.status == 200){
            toastr.success(data.responseText)
          }else{
            toastr.error(data.responseText)
          }

        }
      })
    },

    // ===== Performance monitoring =====
    updateScreenshotLatency: function(latency) {
      this.perfHistory.screenshots.push(latency);
      if (this.perfHistory.screenshots.length > 10) {
        this.perfHistory.screenshots.shift();
      }
      var sum = this.perfHistory.screenshots.reduce(function(a, b) { return a + b; }, 0);
      this.perfStats.screenshot = Math.round(sum / this.perfHistory.screenshots.length);
      if (this.perfStats.screenshot > 0) {
        this.perfStats.fps = Math.min(60, Math.round(1000 / this.perfStats.screenshot));
      }
    },

    updateCommandLatency: function(latency) {
      this.perfHistory.commands.push(latency);
      if (this.perfHistory.commands.length > 10) {
        this.perfHistory.commands.shift();
      }
      var sum = this.perfHistory.commands.reduce(function(a, b) { return a + b; }, 0);
      this.perfStats.command = Math.round(sum / this.perfHistory.commands.length);
    },

    // ===== Quick phrase functionality =====
    loadPhrases: function() {
      try {
        var saved = localStorage.getItem('cloudcontrol_phrases');
        if (saved) {
          this.phrases = JSON.parse(saved);
        }
      } catch (e) {
        console.log('Failed to load quick phrases:', e);
      }
    },

    savePhrases: function() {
      try {
        localStorage.setItem('cloudcontrol_phrases', JSON.stringify(this.phrases));
      } catch (e) {
        console.log('Failed to save quick phrases:', e);
      }
    },

    addPhrase: function() {
      var text = this.newPhrase.trim();
      if (!text) return;
      if (this.phrases.indexOf(text) === -1) {
        this.phrases.unshift(text);
        this.savePhrases();
      }
      this.newPhrase = '';
    },

    deletePhrase: function(index) {
      this.phrases.splice(index, 1);
      this.savePhrases();
    },

    sendPhrase: function(phrase) {
      var self = this;
      var startTime = Date.now();
      $.ajax({
        url: '/inspector/' + self.deviceUdid + '/input',
        method: 'POST',
        contentType: 'application/json',
        data: JSON.stringify({ text: phrase })
      }).done(function(response) {
        self.updateCommandLatency(Date.now() - startTime);
        $.notify('Sent: ' + (phrase.length > 20 ? phrase.substring(0, 20) + '...' : phrase), 'success');
      }).fail(function(err) {
        $.notify('Send failed', 'error');
        console.log('Phrase send failed:', err);
      });
    },

    clearAllPhrases: function() {
      if (confirm('Are you sure you want to clear all quick phrases?')) {
        this.phrases = [];
        this.savePhrases();
      }
    }
  }
})

// Fallback to ensure touch is always initialized
window.addEventListener('load', function() {
  setTimeout(function() {
    if (window.app && !window.app.control && !window.app.useScrcpyMode) {
      console.log('[Fallback] Force initializing touch and screen stream');
      try {
        window.app.enableTouch();
        window.app.openScreenStream();
      } catch (e) {
        console.error('[Fallback] Initialization failed:', e);
      }
    }
  }, 2000);
})