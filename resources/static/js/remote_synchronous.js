/* Javascript */
window.app = new Vue({
    el: '#app',
    data: {
        deviceList: [],
        deviceUdid: deviceUdid,
        masterIndex: 0,
        device: {},
        deviceInfo: {},
        fixConsole: '', // log for fix minicap and rotation
        navtabs: {
            active: location.hash.slice(1) || 'home',
            tabs: [],
        },
        error: '',
        loading: false,
        canvas: {
            bg: null,
            fg: null,
        },
        canvasStyle: {
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
        // Parameters for non-capture window view
        lastScreenSize2: {
            screen: {},
            canvas: {
                width: 1,
                height: 1
            }
        },
        screenWS: null,
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
        videoUrl: '',
        videoReceiver: null, // sub function to receive image
        inputText: '',
    },
    watch: {},
    computed: {
        // deviceUrl removed — all calls proxied through server via /inspector/{udid}/...
    },
    mounted: function () {
        // this.deviceList=window.deviceList;
        for(var i=0;i<deviceList.length;i++){
            this.deviceList.push({
                src: "/devices/" + deviceList[i]["udid"] + "/screenshot/0",
                des: deviceList[i]["src"],
                remote: "/devices/" + deviceList[i]["udid"] + "/remote",
                udid: deviceList[i]["udid"],
                width: deviceList[i]["width"] || 1080,
                height: deviceList[i]["height"] || 1920
            })
        }

        var self = this;
        $.notify.defaults({ className: "success" });
        this.canvas.bg = document.getElementById('bgCanvas0')
        this.canvas.fg = document.getElementById('fgCanvas')


        $(window).resize(function () {
            self.resizeScreen();
        })

        this.initDragDealer();

        // get device info (via server proxy, not direct to device)
        $.ajax({
            url: "/devices/" + this.deviceUdid + "/info",
            dataType: "json"
        }).then(function (ret) {
            this.deviceInfo = ret;
            document.title = ret.model || 'Device';
        }.bind(this));
        this.enableTouch();
        this.openScreenStream();

        // wakeup device on connect
        setTimeout(function () {
            this.keyevent("WAKEUP");
        }.bind(this), 1)


    },
    watch: {
        // inputText watcher removed — inputWS was never initialized (dead code)
    },
    methods: {
        // Get target devices for batch operations — returns all devices in the list
        getTargetDevices: function () {
            return this.deviceList;
        },

        toggleScreen: function () {
            if (this.screenWS) {
                this.screenWS.close();
                this.screenWS = null;
                this.canvasStyle.opacity = 0;
            } else {
                this.openScreenStream();
                this.canvasStyle.opacity = 1;
            }
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
        hold: function (msecs) {
            // Long-press at center of screen via server proxy
            var master = this.deviceList[this.masterIndex];
            var devW = (master && master.width) || 1080;
            var devH = (master && master.height) || 1920;
            var cx = Math.floor(devW / 2);
            var cy = Math.floor(devH / 2);
            this.sendTouchToDevices(cx, cy, cx, cy, msecs || 1000);
        },
        keyevent: function (meta) {
            console.log("keyevent", meta);
            var targets = this.getTargetDevices();
            for (var i = 0; i < targets.length; i++) {
                $.ajax({
                    url: "/inspector/" + targets[i].udid + "/keyevent",
                    method: "POST",
                    contentType: "application/json",
                    data: JSON.stringify({ key: meta.toUpperCase() })
                });
            }
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
                }
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
        delayReload: function (msec) {
            setTimeout(this.screenDumpUI, msec || 1000);
        },
        openScreenStream: function () {
          var self = this;
          var BLANK_IMG =
            'data:image/gif;base64,R0lGODlhAQABAAAAACH5BAEKAAEALAAAAAABAAEAAAICTAEAOw=='
          // Use NIO WebSocket proxy instead of direct device minicap
          var protocol = location.protocol === "https:" ? "wss:" : "ws:";
          var ws = new WebSocket(protocol + "//" + location.host + "/nio/" + this.deviceUdid + "/ws");
          ws.binaryType = 'arraybuffer';
          var canvas = document.getElementById('bgCanvas0')
          var ctx = canvas.getContext('2d');

          this.screenWS = ws;
          var imagePool = new ImagePool(100);

          ws.onopen = function (ev) {
            console.log('[NIO] screen websocket connected');
            // Subscribe to screenshot streaming
            ws.send(JSON.stringify({
              type: "subscribe",
              target: "screenshot",
              interval: 100
            }));
          };

          var imageBlobBuffer = self.imageBlobBuffer;
          var imageBlobMaxLength = 300;

          ws.onmessage = function (message) {
            // NIO sends JPEG as binary frames
            if (message.data instanceof ArrayBuffer) {
              var blob = new Blob([message.data], { type: 'image/jpeg' });

              imageBlobBuffer.push(blob);
              if (imageBlobBuffer.length > imageBlobMaxLength) {
                imageBlobBuffer.shift();
              }

              var img = imagePool.next();
              img.onload = function () {
                canvas.width = img.width;
                canvas.height = img.height;
                ctx.drawImage(img, 0, 0, img.width, img.height);
                self.resizeScreen(img);

                img.onload = img.onerror = null;
                img.src = BLANK_IMG;
                img = null;
                blob = null;
                URL.revokeObjectURL(url);
                url = null;
              }

              img.onerror = function () {
                img.onload = img.onerror = null;
                img.src = BLANK_IMG;
                img = null;
                blob = null;
                URL.revokeObjectURL(url);
                url = null;
              }

              var url = URL.createObjectURL(blob);
              img.src = url;
            } else if (typeof message.data === 'string') {
              // Handle JSON text messages (status, errors)
              try {
                var data = JSON.parse(message.data);
                console.log("[NIO] message:", data.type || data.status, data);
              } catch(e) {
                console.log("[NIO] text:", message.data);
              }
            }
          }

          ws.onclose = function (ev) {
            console.log("[NIO] screen websocket closed", ev.code);
          }

          ws.onerror = function (ev) {
            console.log("[NIO] screen websocket error");
          }
        },
        // Send touch to all target devices via server proxy
        sendTouchToDevices: function (x, y, x2, y2, duration) {
            var targets = this.getTargetDevices();
            for (var i = 0; i < targets.length; i++) {
                var data = { x: x, y: y };
                if (x2 !== undefined && y2 !== undefined) {
                    data.x2 = x2;
                    data.y2 = y2;
                    data.duration = duration || 200;
                }
                $.ajax({
                    url: "/inspector/" + targets[i].udid + "/touch",
                    method: "POST",
                    contentType: "application/json",
                    data: JSON.stringify(data)
                });
            }
        },
        enableTouch: function () {
            /**
             * TOUCH HANDLING — uses server proxy HTTP endpoints instead of minitouch WebSocket
             */
            var self = this;
            var element = this.canvas.bg;

            var screen = { bounds: {} }
            var touchStartPos = null;

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
                $(".finger-" + index)
                    .addClass("active")
                    .css("transform", 'translate3d(' + x + 'px,' + y + 'px,0)');
            }

            function deactiveFinger(index) {
                $(".finger-" + index).removeClass("active");
            }

            // Convert screen coordinates to device pixel coordinates
            function toDeviceCoords(pageX, pageY) {
                calculateBounds();
                var x = pageX - screen.bounds.x;
                var y = pageY - screen.bounds.y;
                var px = x / screen.bounds.w;
                var py = y / screen.bounds.h;
                // Use master device resolution for coordinate mapping
                var master = self.deviceList[self.masterIndex];
                var devW = (master && master.width) || 1080;
                var devH = (master && master.height) || 1920;
                return {
                    px: px,
                    py: py,
                    x: Math.floor(px * devW),
                    y: Math.floor(py * devH),
                };
            }

            function markPosition(pos) {
                var ctx = self.canvas.fg.getContext("2d");
                ctx.fillStyle = '#ff0000';
                ctx.beginPath();
                ctx.arc(pos.x, pos.y, 12, 0, 2 * Math.PI);
                ctx.closePath();
                ctx.fill();

                ctx.fillStyle = "#fff";
                ctx.beginPath();
                ctx.arc(pos.x, pos.y, 8, 0, 2 * Math.PI);
                ctx.closePath();
                ctx.fill();
            }

            function mouseDownListener(event) {
                var e = event.originalEvent || event;
                if (e.which === 3) return;
                e.preventDefault();

                activeFinger(0, e.pageX, e.pageY);
                touchStartPos = toDeviceCoords(e.pageX, e.pageY);

                element.addEventListener('mousemove', mouseMoveListener);
                document.addEventListener('mouseup', mouseUpListener);
            }

            function mouseMoveListener(event) {
                var e = event.originalEvent || event;
                if (e.which === 3) return;
                e.preventDefault();
                activeFinger(0, e.pageX, e.pageY);
            }

            function mouseUpListener(event) {
                var e = event.originalEvent || event;
                if (e.which === 3) return;
                e.preventDefault();

                var endPos = toDeviceCoords(e.pageX, e.pageY);
                var dx = Math.abs(endPos.x - touchStartPos.x);
                var dy = Math.abs(endPos.y - touchStartPos.y);

                if (dx < 10 && dy < 10) {
                    // Tap — start and end positions are close enough
                    self.sendTouchToDevices(touchStartPos.x, touchStartPos.y);
                } else {
                    // Swipe — significant movement detected
                    self.sendTouchToDevices(touchStartPos.x, touchStartPos.y, endPos.x, endPos.y, 200);
                }

                // Update cursor marker on canvas
                var canvasPx = Math.floor(endPos.px * element.width);
                var canvasPy = Math.floor(endPos.py * element.height);
                self.cursor = { px: endPos.px, py: endPos.py, x: canvasPx, y: canvasPy };
                markPosition(self.cursor);

                stopMousing();
                touchStartPos = null;
            }

            function stopMousing() {
                element.removeEventListener('mousemove', mouseMoveListener);
                document.removeEventListener('mouseup', mouseUpListener);
                deactiveFinger(0);
            }

            function mouseWheelListener(event) {
                var e = event.originalEvent || event;
                e.preventDefault();
                var pos = toDeviceCoords(e.pageX, e.pageY);
                var scrollAmount = (e.wheelDeltaY || -e.deltaY) < 0 ? 300 : -300;
                // Simulate swipe for scroll
                self.sendTouchToDevices(pos.x, pos.y, pos.x, pos.y + scrollAmount, 300);
            }

            /* bind listeners */
            element.addEventListener('mousedown', mouseDownListener);
            element.addEventListener('mousewheel', mouseWheelListener);
        }
    }
})