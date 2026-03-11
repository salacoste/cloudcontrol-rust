// Copies a string to the clipboard using the modern Clipboard API.
// Falls back to execCommand for older browsers.
// Browser support: Chrome 66+, Firefox 63+, Safari 13.1+, Edge 79+
// Returns a Promise that resolves to true on success, false on failure.
function copyToClipboard(text) {
  // Modern Clipboard API (preferred)
  if (navigator.clipboard && navigator.clipboard.writeText) {
    return navigator.clipboard.writeText(text).then(function() {
      return true;
    }).catch(function(err) {
      console.warn("Clipboard API failed, trying fallback:", err);
      return fallbackCopyToClipboard(text);
    });
  }

  // Fallback for browsers without Clipboard API
  return Promise.resolve(fallbackCopyToClipboard(text));
}

// Fallback copy method using deprecated execCommand (for older browsers)
function fallbackCopyToClipboard(text) {
  // IE specific code path
  if (window.clipboardData && window.clipboardData.setData) {
    return clipboardData.setData("Text", text);
  }

  // Standard fallback using textarea
  if (document.queryCommandSupported && document.queryCommandSupported("copy")) {
    var textarea = document.createElement("textarea");
    textarea.textContent = text;
    textarea.style.position = "fixed"; // Prevent scrolling to bottom of page in MS Edge.
    document.body.appendChild(textarea);
    textarea.select();
    try {
      return document.execCommand("copy"); // Security exception may be thrown by some browsers.
    } catch (ex) {
      console.warn("Copy to clipboard failed.", ex);
      return false;
    } finally {
      document.body.removeChild(textarea);
    }
  }

  return false;
}

/* Image Pool */
function ImagePool(size) {
  this.size = size
  this.images = []
  this.counter = 0
}

ImagePool.prototype.next = function() {
  if (this.images.length < this.size) {
    var image = new Image()
    this.images.push(image)
    return image
  } else {
    if (this.counter >= this.size) {
      // Reset for unlikely but theoretically possible overflow.
      this.counter = 0
    }
  }

  return this.images[this.counter++ % this.size]
}

// convert to blob data
function b64toBlob(b64Data, contentType, sliceSize) {
  contentType = contentType || '';
  sliceSize = sliceSize || 512;

  var byteCharacters = atob(b64Data);
  var byteArrays = [];

  for (var offset = 0; offset < byteCharacters.length; offset += sliceSize) {
    var slice = byteCharacters.slice(offset, offset + sliceSize);

    var byteNumbers = new Array(slice.length);
    for (var i = 0; i < slice.length; i++) {
      byteNumbers[i] = slice.charCodeAt(i);
    }

    var byteArray = new Uint8Array(byteNumbers);
    byteArrays.push(byteArray);
  }

  return new Blob(byteArrays, {
    type: contentType
  });
}

var MiniTouch = {
  createNew: function(ws) {
    // ws: Websocket connection communication with minitouch
    var control = {}

    function sendJSON(obj) {
      ws.send(JSON.stringify(obj))
    }

    // control.coords = function(w, h, x, y, rotation) {
    //   console.log(w, h, x, y, rotation)
    //   return {
    //     xP: x / w,
    //     yP: y / h,
    //   }
    // };

    control.touchDown = function(index, xP, yP, pressure) {
      sendJSON({
        operation: 'd',
        index: index,
        pressure: pressure,
        xP: xP,
        yP: yP,
      })
    };

    control.touchWait = function(mseconds) {
      sendJSON({ operation: 'w', milliseconds: mseconds })
    }

    control.touchCommit = function() {
      sendJSON({ operation: 'c' })
    };

    control.touchMove = function(index, xP, yP, pressure) {
      sendJSON({
        operation: 'm',
        index: index,
        pressure: pressure,
        xP: xP,
        yP: yP,
      })
    };

    control.touchUp = function(index) {
      sendJSON({ operation: 'u', index: index })
    };

    return control;
  }
}

/**
 * Rotation affects the screen as follows:
 *
 *                   0deg
 *                 |------|
 *                 | MENU |
 *                 |------|
 *            -->  |      |  --|
 *            |    |      |    v
 *                 |      |
 *                 |      |
 *                 |------|
 *        |----|-|          |-|----|
 *        |    |M|          | |    |
 *        |    |E|          | |    |
 *  90deg |    |N|          |U|    | 270deg
 *        |    |U|          |N|    |
 *        |    | |          |E|    |
 *        |    | |          |M|    |
 *        |----|-|          |-|----|
 *                 |------|
 *            ^    |      |    |
 *            |--  |      |  <--
 *                 |      |
 *                 |      |
 *                 |------|
 *                 | UNEM |
 *                 |------|
 *                  180deg
 *
 * Which leads to the following mapping:
 *
 * |--------------|------|---------|---------|---------|
 * |              | 0deg |  90deg  |  180deg |  270deg |
 * |--------------|------|---------|---------|---------|
 * | CSS rotate() | 0deg | -90deg  | -180deg |  90deg  |
 * | bounding w   |  w   |    h    |    w    |    h    |
 * | bounding h   |  h   |    w    |    h    |    w    |
 * | pos x        |  x   |   h-y   |   w-x   |    y    |
 * | pos y        |  y   |    x    |   h-y   |   h-x   |
 * |--------------|------|---------|---------|---------|
 */
function coords(boundingW, boundingH, relX, relY, rotation) {
  var w, h, x, y;

  switch (rotation) {
    case 0:
      w = boundingW
      h = boundingH
      x = relX
      y = relY
      break
    case 90:
      w = boundingH
      h = boundingW
      x = boundingH - relY
      y = relX
      break
    case 180:
      w = boundingW
      h = boundingH
      x = boundingW - relX
      y = boundingH - relY
      break
    case 270:
      w = boundingH
      h = boundingW
      x = relY
      y = boundingW - relX
      break
  }

  return {
    xP: x / w,
    yP: y / h,
  }
}

/* accepts parameters
 * h  Object = {h:x, s:y, v:z}
 * OR 
 * h, s, v
 * This code expects 0 <= h, s, v <= 1
 * The returned 0 <= r, g, b <= 255 are rounded to the nearest Integer
 */
function HSVtoRGB(h, s, v) {
  var r, g, b, i, f, p, q, t;
  if (arguments.length === 1) {
    s = h.s, v = h.v, h = h.h;
  }
  i = Math.floor(h * 6);
  f = h * 6 - i;
  p = v * (1 - s);
  q = v * (1 - f * s);
  t = v * (1 - (1 - f) * s);
  switch (i % 6) {
    case 0:
      r = v, g = t, b = p;
      break;
    case 1:
      r = q, g = v, b = p;
      break;
    case 2:
      r = p, g = v, b = t;
      break;
    case 3:
      r = p, g = q, b = v;
      break;
    case 4:
      r = t, g = p, b = v;
      break;
    case 5:
      r = v, g = p, b = q;
      break;
  }
  return [
    Math.round(r * 255),
    Math.round(g * 255),
    Math.round(b * 255)
  ]
}

function getRandomRgb(brightness) {
  var rgb = HSVtoRGB(Math.random(), Math.random(), 0.8);
  return 'rgb(' + rgb.join(",") + ")";
}