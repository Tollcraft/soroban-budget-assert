"use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.create = createFetchClient;
exports.fetchClient = void 0;
var _feaxios = _interopRequireDefault(require("feaxios"));
var _types = require("./types");
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
function _regenerator() { var e, t, r = "function" == typeof Symbol ? Symbol : {}, n = r.iterator || "@@iterator", o = r.toStringTag || "@@toStringTag"; function i(r, n, o, i) { var c = n && n.prototype instanceof Generator ? n : Generator, u = Object.create(c.prototype); return _regeneratorDefine2(u, "_invoke", function (r, n, o) { var i, c, u, f = 0, p = o || [], y = !1, G = { p: 0, n: 0, v: e, a: d, f: d.bind(e, 4), d: function d(t, r) { return i = t, c = 0, u = e, G.n = r, a; } }; function d(r, n) { for (c = r, u = n, t = 0; !y && f && !o && t < p.length; t++) { var o, i = p[t], d = G.p, l = i[2]; r > 3 ? (o = l === n) && (u = i[(c = i[4]) ? 5 : (c = 3, 3)], i[4] = i[5] = e) : i[0] <= d && ((o = r < 2 && d < i[1]) ? (c = 0, G.v = n, G.n = i[1]) : d < l && (o = r < 3 || i[0] > n || n > l) && (i[4] = r, i[5] = n, G.n = l, c = 0)); } if (o || r > 1) return a; throw y = !0, n; } return function (o, p, l) { if (f > 1) throw TypeError("Generator is already running"); for (y && 1 === p && d(p, l), c = p, u = l; (t = c < 2 ? e : u) || !y;) { i || (c ? c < 3 ? (c > 1 && (G.n = -1), d(c, u)) : G.n = u : G.v = u); try { if (f = 2, i) { if (c || (o = "next"), t = i[o]) { if (!(t = t.call(i, u))) throw TypeError("iterator result is not an object"); if (!t.done) return t; u = t.value, c < 2 && (c = 0); } else 1 === c && (t = i.return) && t.call(i), c < 2 && (u = TypeError("The iterator does not provide a '" + o + "' method"), c = 1); i = e; } else if ((t = (y = G.n < 0) ? u : r.call(n, G)) !== a) break; } catch (t) { i = e, c = 1, u = t; } finally { f = 1; } } return { value: t, done: y }; }; }(r, o, i), !0), u; } var a = {}; function Generator() {} function GeneratorFunction() {} function GeneratorFunctionPrototype() {} t = Object.getPrototypeOf; var c = [][n] ? t(t([][n]())) : (_regeneratorDefine2(t = {}, n, function () { return this; }), t), u = GeneratorFunctionPrototype.prototype = Generator.prototype = Object.create(c); function f(e) { return Object.setPrototypeOf ? Object.setPrototypeOf(e, GeneratorFunctionPrototype) : (e.__proto__ = GeneratorFunctionPrototype, _regeneratorDefine2(e, o, "GeneratorFunction")), e.prototype = Object.create(u), e; } return GeneratorFunction.prototype = GeneratorFunctionPrototype, _regeneratorDefine2(u, "constructor", GeneratorFunctionPrototype), _regeneratorDefine2(GeneratorFunctionPrototype, "constructor", GeneratorFunction), GeneratorFunction.displayName = "GeneratorFunction", _regeneratorDefine2(GeneratorFunctionPrototype, o, "GeneratorFunction"), _regeneratorDefine2(u), _regeneratorDefine2(u, o, "Generator"), _regeneratorDefine2(u, n, function () { return this; }), _regeneratorDefine2(u, "toString", function () { return "[object Generator]"; }), (_regenerator = function _regenerator() { return { w: i, m: f }; })(); }
function _regeneratorDefine2(e, r, n, t) { var i = Object.defineProperty; try { i({}, "", {}); } catch (e) { i = 0; } _regeneratorDefine2 = function _regeneratorDefine(e, r, n, t) { function o(r, n) { _regeneratorDefine2(e, r, function (e) { return this._invoke(r, n, e); }); } r ? i ? i(e, r, { value: n, enumerable: !t, configurable: !t, writable: !t }) : e[r] = n : (o("next", 0), o("throw", 1), o("return", 2)); }, _regeneratorDefine2(e, r, n, t); }
function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _createForOfIteratorHelper(r, e) { var t = "undefined" != typeof Symbol && r[Symbol.iterator] || r["@@iterator"]; if (!t) { if (Array.isArray(r) || (t = _unsupportedIterableToArray(r)) || e && r && "number" == typeof r.length) { t && (r = t); var _n = 0, F = function F() {}; return { s: F, n: function n() { return _n >= r.length ? { done: !0 } : { done: !1, value: r[_n++] }; }, e: function e(r) { throw r; }, f: F }; } throw new TypeError("Invalid attempt to iterate non-iterable instance.\nIn order to be iterable, non-array objects must have a [Symbol.iterator]() method."); } var o, a = !0, u = !1; return { s: function s() { t = t.call(r); }, n: function n() { var r = t.next(); return a = r.done, r; }, e: function e(r) { u = !0, o = r; }, f: function f() { try { a || null == t.return || t.return(); } finally { if (u) throw o; } } }; }
function _unsupportedIterableToArray(r, a) { if (r) { if ("string" == typeof r) return _arrayLikeToArray(r, a); var t = {}.toString.call(r).slice(8, -1); return "Object" === t && r.constructor && (t = r.constructor.name), "Map" === t || "Set" === t ? Array.from(r) : "Arguments" === t || /^(?:Ui|I)nt(?:8|16|32)(?:Clamped)?Array$/.test(t) ? _arrayLikeToArray(r, a) : void 0; } }
function _arrayLikeToArray(r, a) { (null == a || a > r.length) && (a = r.length); for (var e = 0, n = Array(a); e < a; e++) n[e] = r[e]; return n; }
function asyncGeneratorStep(n, t, e, r, o, a, c) { try { var i = n[a](c), u = i.value; } catch (n) { return void e(n); } i.done ? t(u) : Promise.resolve(u).then(r, o); }
function _asyncToGenerator(n) { return function () { var t = this, e = arguments; return new Promise(function (r, o) { var a = n.apply(t, e); function _next(n) { asyncGeneratorStep(a, r, o, _next, _throw, "next", n); } function _throw(n) { asyncGeneratorStep(a, r, o, _next, _throw, "throw", n); } _next(void 0); }); }; }
function ownKeys(e, r) { var t = Object.keys(e); if (Object.getOwnPropertySymbols) { var o = Object.getOwnPropertySymbols(e); r && (o = o.filter(function (r) { return Object.getOwnPropertyDescriptor(e, r).enumerable; })), t.push.apply(t, o); } return t; }
function _objectSpread(e) { for (var r = 1; r < arguments.length; r++) { var t = null != arguments[r] ? arguments[r] : {}; r % 2 ? ownKeys(Object(t), !0).forEach(function (r) { _defineProperty(e, r, t[r]); }) : Object.getOwnPropertyDescriptors ? Object.defineProperties(e, Object.getOwnPropertyDescriptors(t)) : ownKeys(Object(t)).forEach(function (r) { Object.defineProperty(e, r, Object.getOwnPropertyDescriptor(t, r)); }); } return e; }
function _classCallCheck(a, n) { if (!(a instanceof n)) throw new TypeError("Cannot call a class as a function"); }
function _defineProperties(e, r) { for (var t = 0; t < r.length; t++) { var o = r[t]; o.enumerable = o.enumerable || !1, o.configurable = !0, "value" in o && (o.writable = !0), Object.defineProperty(e, _toPropertyKey(o.key), o); } }
function _createClass(e, r, t) { return r && _defineProperties(e.prototype, r), t && _defineProperties(e, t), Object.defineProperty(e, "prototype", { writable: !1 }), e; }
function _defineProperty(e, r, t) { return (r = _toPropertyKey(r)) in e ? Object.defineProperty(e, r, { value: t, enumerable: !0, configurable: !0, writable: !0 }) : e[r] = t, e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
var CANCELED_MARKER = Symbol.for("@stellar/stellar-sdk.canceled");
function makeCanceledError(reason) {
  var err = new Error(reason || "Request canceled");
  err[CANCELED_MARKER] = true;
  return err;
}
var InterceptorManager = function () {
  function InterceptorManager() {
    _classCallCheck(this, InterceptorManager);
    _defineProperty(this, "handlers", []);
  }
  return _createClass(InterceptorManager, [{
    key: "use",
    value: function use(fulfilled, rejected) {
      this.handlers.push({
        fulfilled: fulfilled,
        rejected: rejected
      });
      return this.handlers.length - 1;
    }
  }, {
    key: "eject",
    value: function eject(id) {
      if (this.handlers[id]) {
        this.handlers[id] = null;
      }
    }
  }, {
    key: "forEach",
    value: function forEach(fn) {
      this.handlers.forEach(function (h) {
        if (h !== null) {
          fn(h);
        }
      });
    }
  }]);
}();
function getFormConfig(config) {
  var formConfig = config || {};
  formConfig.headers = new Headers(formConfig.headers || {});
  formConfig.headers.set("Content-Type", "application/x-www-form-urlencoded");
  return formConfig;
}
function mergeWithDefaults(defaults, config) {
  if (!config) return _objectSpread({}, defaults);
  var merged = _objectSpread(_objectSpread({}, defaults), config);
  if ((defaults === null || defaults === void 0 ? void 0 : defaults.headers) !== undefined || config.headers !== undefined) {
    var headers = new Headers((defaults === null || defaults === void 0 ? void 0 : defaults.headers) || {});
    new Headers(config.headers || {}).forEach(function (v, k) {
      headers.set(k, v);
    });
    merged.headers = headers;
  }
  if ((defaults === null || defaults === void 0 ? void 0 : defaults.params) !== undefined || config.params !== undefined) {
    merged.params = _objectSpread(_objectSpread({}, (defaults === null || defaults === void 0 ? void 0 : defaults.params) || {}), config.params || {});
  }
  return merged;
}
function buildBoundedUrl(config) {
  var url = config.url || "";
  if (config.baseURL && url && !/^https?:\/\//i.test(url)) {
    url = url.replace(/^\/?/, "".concat(config.baseURL.replace(/\/$/, ""), "/"));
  }
  if (config.params && Object.keys(config.params).length > 0) {
    var qs = new URLSearchParams(config.params).toString();
    url += (url.includes("?") ? "&" : "?") + qs;
  }
  return url;
}
function encodeRequestBody(data, headers) {
  if (data === undefined || data === null) return undefined;
  if (typeof data === "string") return data;
  if (data instanceof URLSearchParams) {
    if (!headers.has("content-type")) {
      headers.set("content-type", "application/x-www-form-urlencoded");
    }
    return data;
  }
  if (data instanceof Blob || data instanceof ArrayBuffer || ArrayBuffer.isView(data)) {
    if (!headers.has("content-type")) {
      headers.set("content-type", "application/octet-stream");
    }
    return data;
  }
  if (typeof FormData !== "undefined" && data instanceof FormData) {
    return data;
  }
  if (!headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  return JSON.stringify(data);
}
function readBodyBounded(_x, _x2) {
  return _readBodyBounded.apply(this, arguments);
}
function _readBodyBounded() {
  _readBodyBounded = _asyncToGenerator(_regenerator().m(function _callee(response, maxContentLength) {
    var headerLen, reader, chunks, total, _yield$reader$read, done, _value, out, offset, _i, _chunks, c;
    return _regenerator().w(function (_context) {
      while (1) switch (_context.n) {
        case 0:
          if (!(maxContentLength !== undefined)) {
            _context.n = 1;
            break;
          }
          headerLen = response.headers.get("content-length");
          if (!(headerLen && Number(headerLen) > maxContentLength)) {
            _context.n = 1;
            break;
          }
          throw new Error("maxContentLength size of ".concat(maxContentLength, " exceeded"));
        case 1:
          if (response.body) {
            _context.n = 2;
            break;
          }
          return _context.a(2, new Uint8Array(0));
        case 2:
          reader = response.body.getReader();
          chunks = [];
          total = 0;
        case 3:
          if (!true) {
            _context.n = 9;
            break;
          }
          _context.n = 4;
          return reader.read();
        case 4:
          _yield$reader$read = _context.v;
          done = _yield$reader$read.done;
          _value = _yield$reader$read.value;
          if (!done) {
            _context.n = 5;
            break;
          }
          return _context.a(3, 9);
        case 5:
          if (!_value) {
            _context.n = 8;
            break;
          }
          total += _value.byteLength;
          if (!(maxContentLength !== undefined && total > maxContentLength)) {
            _context.n = 7;
            break;
          }
          _context.n = 6;
          return reader.cancel();
        case 6:
          throw new Error("maxContentLength size of ".concat(maxContentLength, " exceeded"));
        case 7:
          chunks.push(_value);
        case 8:
          _context.n = 3;
          break;
        case 9:
          out = new Uint8Array(total);
          offset = 0;
          for (_i = 0, _chunks = chunks; _i < _chunks.length; _i++) {
            c = _chunks[_i];
            out.set(c, offset);
            offset += c.byteLength;
          }
          return _context.a(2, out);
      }
    }, _callee);
  }));
  return _readBodyBounded.apply(this, arguments);
}
function createTimeoutSignal(ms) {
  if (typeof AbortSignal !== "undefined" && typeof AbortSignal.timeout === "function") {
    return AbortSignal.timeout(ms);
  }
  var controller = new AbortController();
  setTimeout(function () {
    var err = new Error("Timeout");
    err.name = "TimeoutError";
    controller.abort(err);
  }, ms);
  return controller.signal;
}
function composeSignals(signals) {
  if (signals.length === 0) return undefined;
  if (signals.length === 1) return signals[0];
  if (typeof AbortSignal !== "undefined" && typeof AbortSignal.any === "function") {
    return AbortSignal.any(signals);
  }
  var controller = new AbortController();
  var _iterator = _createForOfIteratorHelper(signals),
    _step;
  try {
    var _loop = function _loop() {
      var s = _step.value;
      if (s.aborted) {
        controller.abort(s.reason);
        return 1;
      }
      s.addEventListener("abort", function () {
        return controller.abort(s.reason);
      }, {
        once: true
      });
    };
    for (_iterator.s(); !(_step = _iterator.n()).done;) {
      if (_loop()) break;
    }
  } catch (err) {
    _iterator.e(err);
  } finally {
    _iterator.f();
  }
  return controller.signal;
}
function canInspectManualRedirects() {
  return typeof process !== "undefined" && !!process.versions && !!process.versions.node;
}
function applyRedirectSemantics(init, status) {
  if (status === 307 || status === 308) return init;
  var next = _objectSpread(_objectSpread({}, init), {}, {
    method: "GET",
    body: undefined
  });
  var headers = new Headers(init.headers || {});
  headers.delete("content-type");
  headers.delete("content-length");
  headers.delete("transfer-encoding");
  next.headers = headers;
  return next;
}
function stripCrossOriginAuth(init, fromUrl, toUrl) {
  var sameOrigin;
  try {
    sameOrigin = new URL(fromUrl).origin === new URL(toUrl).origin;
  } catch (_unused) {
    sameOrigin = false;
  }
  if (sameOrigin) return init;
  var headers = new Headers(init.headers || {});
  headers.delete("authorization");
  headers.delete("proxy-authorization");
  headers.delete("cookie");
  return _objectSpread(_objectSpread({}, init), {}, {
    headers: headers
  });
}
function buildHttpError(response, config, data) {
  var err = new Error("Request failed with status code ".concat(response.status));
  err.response = {
    status: response.status,
    statusText: response.statusText,
    headers: response.headers,
    data: data,
    config: config
  };
  return err;
}
function boundedFetchAdapter(_x3) {
  return _boundedFetchAdapter.apply(this, arguments);
}
function _boundedFetchAdapter() {
  _boundedFetchAdapter = _asyncToGenerator(_regenerator().m(function _callee2(config) {
    var maxRedirects, maxContentLength, timeout, signals, signal, managedRedirects, canManage, redirect, headers, body, currentInit, currentUrl, redirectsRemaining, response, isManualRedirectResponse, location, nextUrl, errBody, errBytes, errText, bytes, text, data, _t, _t2;
    return _regenerator().w(function (_context2) {
      while (1) switch (_context2.p = _context2.n) {
        case 0:
          maxRedirects = config.maxRedirects, maxContentLength = config.maxContentLength, timeout = config.timeout;
          signals = [];
          if (timeout && timeout > 0) {
            signals.push(createTimeoutSignal(timeout));
          }
          signal = composeSignals(signals);
          managedRedirects = maxRedirects !== undefined;
          canManage = canInspectManualRedirects();
          if (!managedRedirects) {
            redirect = "follow";
          } else if (canManage) {
            redirect = "manual";
          } else if (maxRedirects === 0) {
            redirect = "error";
          } else {
            redirect = "follow";
          }
          headers = new Headers(config.headers || {});
          body = encodeRequestBody(config.data, headers);
          currentInit = _objectSpread(_objectSpread({}, config.fetchOptions), {}, {
            method: (config.method || "get").toUpperCase(),
            headers: headers,
            body: body,
            redirect: redirect
          }, signal ? {
            signal: signal
          } : {});
          currentUrl = buildBoundedUrl(config);
          redirectsRemaining = maxRedirects !== null && maxRedirects !== void 0 ? maxRedirects : 0;
        case 1:
          if (!true) {
            _context2.n = 11;
            break;
          }
          _context2.p = 2;
          _context2.n = 3;
          return fetch(currentUrl, currentInit);
        case 3:
          response = _context2.v;
          _context2.n = 6;
          break;
        case 4:
          _context2.p = 4;
          _t = _context2.v;
          if (!((_t === null || _t === void 0 ? void 0 : _t.name) === "TimeoutError")) {
            _context2.n = 5;
            break;
          }
          throw new Error("timeout of ".concat(config.timeout, "ms exceeded"));
        case 5:
          throw _t;
        case 6:
          isManualRedirectResponse = redirect === "manual" && response.status >= 300 && response.status < 400;
          if (isManualRedirectResponse) {
            _context2.n = 7;
            break;
          }
          return _context2.a(3, 11);
        case 7:
          if (!(redirectsRemaining <= 0)) {
            _context2.n = 9;
            break;
          }
          if (!(maxRedirects === 0)) {
            _context2.n = 8;
            break;
          }
          throw buildHttpError(response, config);
        case 8:
          throw new Error("Maximum number of redirects exceeded");
        case 9:
          location = response.headers.get("location");
          if (location) {
            _context2.n = 10;
            break;
          }
          return _context2.a(3, 11);
        case 10:
          nextUrl = new URL(location, currentUrl).toString();
          currentInit = applyRedirectSemantics(currentInit, response.status);
          currentInit = stripCrossOriginAuth(currentInit, currentUrl, nextUrl);
          currentUrl = nextUrl;
          redirectsRemaining -= 1;
          _context2.n = 1;
          break;
        case 11:
          if (response.ok) {
            _context2.n = 16;
            break;
          }
          _context2.p = 12;
          _context2.n = 13;
          return readBodyBounded(response, maxContentLength);
        case 13:
          errBytes = _context2.v;
          errText = new TextDecoder().decode(errBytes);
          try {
            errBody = JSON.parse(errText);
          } catch (_unused2) {
            errBody = errText;
          }
          _context2.n = 15;
          break;
        case 14:
          _context2.p = 14;
          _t2 = _context2.v;
          throw _t2;
        case 15:
          throw buildHttpError(response, config, errBody);
        case 16:
          _context2.n = 17;
          return readBodyBounded(response, maxContentLength);
        case 17:
          bytes = _context2.v;
          text = new TextDecoder().decode(bytes);
          data = text;
          try {
            data = JSON.parse(text);
          } catch (_unused3) {}
          return _context2.a(2, {
            data: data,
            headers: response.headers,
            config: config,
            status: response.status,
            statusText: response.statusText
          });
      }
    }, _callee2, null, [[12, 14], [2, 4]]);
  }));
  return _boundedFetchAdapter.apply(this, arguments);
}
function createFetchClient() {
  var fetchConfig = arguments.length > 0 && arguments[0] !== undefined ? arguments[0] : {};
  var defaults = _objectSpread(_objectSpread({}, fetchConfig), {}, {
    headers: fetchConfig.headers || {}
  });
  var instance = _feaxios.default.create(defaults);
  var requestInterceptors = new InterceptorManager();
  var responseInterceptors = new InterceptorManager();
  var httpClient = {
    interceptors: {
      request: requestInterceptors,
      response: responseInterceptors
    },
    defaults: _objectSpread(_objectSpread({}, defaults), {}, {
      adapter: function adapter(config) {
        if (config.maxRedirects !== undefined || config.maxContentLength !== undefined) {
          return boundedFetchAdapter(config);
        }
        return instance.request(config);
      }
    }),
    create: function create(config) {
      return createFetchClient(_objectSpread(_objectSpread({}, this.defaults), config));
    },
    makeRequest: function makeRequest(config) {
      var _this = this;
      return new Promise(function (resolve, reject) {
        function processRequest(finalConfig, res, rej) {
          var adapter = finalConfig.adapter || this.defaults.adapter;
          if (!adapter) {
            throw new Error("No adapter available");
          }
          var responsePromise = adapter(finalConfig).then(function (axiosResponse) {
            var httpClientResponse = {
              data: axiosResponse.data,
              headers: axiosResponse.headers,
              config: axiosResponse.config,
              status: axiosResponse.status,
              statusText: axiosResponse.statusText
            };
            return httpClientResponse;
          });
          if (responseInterceptors.handlers.length > 0) {
            var chain = responseInterceptors.handlers.filter(function (interceptor) {
              return interceptor !== null;
            }).flatMap(function (interceptor) {
              return [interceptor.fulfilled, interceptor.rejected];
            });
            var _loop2 = function _loop2(i) {
              responsePromise = responsePromise.then(function (response) {
                var fulfilledInterceptor = chain[i];
                if (typeof fulfilledInterceptor === "function") {
                  return fulfilledInterceptor(response);
                }
                return response;
              }, function (error) {
                var rejectedInterceptor = chain[i + 1];
                if (typeof rejectedInterceptor === "function") {
                  return rejectedInterceptor(error);
                }
                throw error;
              }).then(function (interceptedResponse) {
                return interceptedResponse;
              });
            };
            for (var i = 0, len = chain.length; i < len; i += 2) {
              _loop2(i);
            }
          }
          responsePromise.then(res).catch(rej);
        }
        var abortController = new AbortController();
        config.signal = abortController.signal;
        if (config.cancelToken) {
          var cancelToken = config.cancelToken;
          cancelToken.promise.then(function () {
            abortController.abort();
            reject(makeCanceledError(cancelToken.reason));
          });
        }
        var modifiedConfig = config;
        if (requestInterceptors.handlers.length > 0) {
          var chain = requestInterceptors.handlers.filter(function (interceptor) {
            return interceptor !== null;
          }).flatMap(function (interceptor) {
            return [interceptor.fulfilled, interceptor.rejected];
          });
          var configPromise = Promise.resolve(modifiedConfig);
          for (var i = 0, len = chain.length; i < len; i += 2) {
            configPromise = configPromise.then(chain[i], chain[i + 1]);
          }
          configPromise.then(function (resolvedConfig) {
            processRequest.call(_this, resolvedConfig, resolve, reject);
          }).catch(reject);
          return;
        }
        processRequest.call(_this, modifiedConfig, resolve, reject);
      });
    },
    get: function get(url, config) {
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, config)), {}, {
        url: url,
        method: "get"
      }));
    },
    delete: function _delete(url, config) {
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, config)), {}, {
        url: url,
        method: "delete"
      }));
    },
    head: function head(url, config) {
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, config)), {}, {
        url: url,
        method: "head"
      }));
    },
    options: function options(url, config) {
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, config)), {}, {
        url: url,
        method: "options"
      }));
    },
    post: function post(url, data, config) {
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, config)), {}, {
        url: url,
        method: "post",
        data: data
      }));
    },
    put: function put(url, data, config) {
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, config)), {}, {
        url: url,
        method: "put",
        data: data
      }));
    },
    patch: function patch(url, data, config) {
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, config)), {}, {
        url: url,
        method: "patch",
        data: data
      }));
    },
    postForm: function postForm(url, data, config) {
      var formConfig = getFormConfig(config);
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, formConfig)), {}, {
        url: url,
        method: "post",
        data: data
      }));
    },
    putForm: function putForm(url, data, config) {
      var formConfig = getFormConfig(config);
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, formConfig)), {}, {
        url: url,
        method: "put",
        data: data
      }));
    },
    patchForm: function patchForm(url, data, config) {
      var formConfig = getFormConfig(config);
      return this.makeRequest(_objectSpread(_objectSpread({}, mergeWithDefaults(this.defaults, formConfig)), {}, {
        url: url,
        method: "patch",
        data: data
      }));
    },
    CancelToken: _types.CancelToken,
    isCancel: function isCancel(value) {
      return value instanceof Error && value[CANCELED_MARKER] === true;
    }
  };
  return httpClient;
}
var fetchClient = exports.fetchClient = createFetchClient();