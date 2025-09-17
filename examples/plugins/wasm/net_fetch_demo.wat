(module
  (memory (export "memory") 1)
  (global $heap (mut i32) (i32.const 1024))
  (func (export "plugin_alloc") (param $n i32) (result i32)
    (local $old i32)
    global.get $heap
    local.tee $old
    local.get $n
    i32.add
    global.set $heap
    local.get $old)

  (import "host" "host_net_fetch" (func $host_net_fetch (param i32 i32) (result i64)))
  (import "host" "host_log" (func $host_log (param i32 i32 i32) (result i32)))

  ;; JSON request body at 0
  (data (i32.const 0) "{\"url\":\"https://example.com\",\"method\":\"GET\",\"headers\":[],\"body\":null}")

  (data (i32.const 256) "net_fetch_demo started")

  (func (export "plugin_init") (result i32)
    (drop (call $host_net_fetch (i32.const 0) (i32.const 70)))
    (drop (call $host_log (i32.const 1) (i32.const 256) (i32.const 21)))
    (i32.const 0))
)
