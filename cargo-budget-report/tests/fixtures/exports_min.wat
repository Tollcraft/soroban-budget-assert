;; Text form of `exports_min.wasm`, the fixture that pins WASM
;; export-section parsing in `src/wasm_exports.rs`.
;;
;; Exports, in section order:
;;   "add"       func 0   -> kept
;;   "sub"       func 0   -> kept
;;   "memory"    mem  0   -> dropped (name == "memory")
;;   "_internal" func 0   -> dropped (name starts with '_')
;;
;; The checked-in .wasm is hand-assembled rather than compiled so the fixture
;; has no toolchain dependency: it carries a single real function body and
;; points all three function exports at function index 0. `wasmparser::Parser`
;; does not validate, so this is enough to exercise the export-section walk.
;; Regenerate it with:
;;
;;   python3 - <<'PY'
;;   b = bytearray([0x00,0x61,0x73,0x6d, 0x01,0x00,0x00,0x00])
;;   b += bytes([0x01,0x04, 0x01, 0x60,0x00,0x00])       # type   () -> ()
;;   b += bytes([0x03,0x02, 0x01, 0x00])                 # func   1 x type 0
;;   b += bytes([0x05,0x03, 0x01, 0x00,0x01])            # memory 1 x min 1
;;   ec = bytearray([0x04])
;;   def exp(n,k):
;;       n = n.encode(); return bytes([len(n)]) + n + bytes([k, 0x00])
;;   ec += exp("add",0x00); ec += exp("sub",0x00)
;;   ec += exp("memory",0x02); ec += exp("_internal",0x00)
;;   b += bytes([0x07, len(ec)]) + ec                    # export section
;;   b += bytes([0x0a,0x04, 0x01, 0x02,0x00,0x0b])       # code   1 empty body
;;   open("exports_min.wasm","wb").write(bytes(b))
;;   PY

(module
  (func (export "add"))
  (func (export "sub"))
  (memory (export "memory") 1)
  (func (export "_internal")))
