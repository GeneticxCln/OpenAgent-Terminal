(module
  (memory (export "memory") 1)
  ;; Simple bump allocator: allocate N bytes from a global heap pointer
  (global $heap (mut i32) (i32.const 1024))
  (func (export "plugin_alloc") (param $n i32) (result i32)
    (local $old i32)
    global.get $heap
    local.tee $old
    local.get $n
    i32.add
    global.set $heap
    local.get $old)

  ;; host imports
  (import "host" "host_read_file" (func $host_read_file (param i32 i32) (result i64)))
  (import "host" "host_log" (func $host_log (param i32 i32 i32) (result i32)))

  ;; Data: path string at offset 0
  (data (i32.const 0) "/etc/hostname")

  ;; On init, call host_read_file with path at (0, 14). Ignore return; log a simple message.
  (func (export "plugin_init") (result i32)
    (drop (call $host_read_file (i32.const 0) (i32.const 14)))
    (drop (call $host_log (i32.const 1) (i32.const 256) (i32.const 20)))
    (i32.const 0))

  ;; Message buffer at offset 256
  (data (i32.const 256) "read_file_demo started")
)
