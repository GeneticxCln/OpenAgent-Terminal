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

  (import "host" "host_spawn" (func $host_spawn (param i32 i32) (result i64)))
  (import "host" "host_log" (func $host_log (param i32 i32 i32) (result i32)))

  ;; JSON request at 0: echo "hi"
  (data (i32.const 0) "{\"cmd\":\"echo\",\"args\":[\"hi\"],\"cwd\":null}")

  (data (i32.const 256) "spawn_demo started")

  (func (export "plugin_init") (result i32)
    (drop (call $host_spawn (i32.const 0) (i32.const 30)))
    (drop (call $host_log (i32.const 1) (i32.const 256) (i32.const 16)))
    (i32.const 0))
)
