#[cfg(test)]
mod ai_memory_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;
    
    /// Test configuration for memory monitoring
    fn create_test_memory_config() -> MemoryMonitorConfig {
        MemoryMonitorConfig {
            cleanup_threshold_bytes: 1024 * 1024, // 1MB for testing
            check_interval: Duration::from_millis(100), // Fast checks for testing
            min_cleanup_interval: Duration::from_millis(200),
            aggressive_threshold_bytes: 2 * 1024 * 1024, // 2MB
            enable_background_cleanup: true,
            aggressive_mode: AggressiveCleanupMode {
                enabled: true,
                trigger_threshold_bytes: 1536 * 1024, // 1.5MB
                aggressive_check_interval: Duration::from_millis(50),
                aggressive_min_cleanup_interval: Duration::from_millis(100),
                aggressive_history_retention_hours: 1,
                aggressive_cache_size_limit: 100,
                aggressive_vacuum_frequency: 2,
            },
        }
    }

    #[test]
    fn test_memory_monitor_basic_functionality() {
        let config = create_test_memory_config();
        let monitor = MemoryMonitor::new(config);

        // Test initial state
        let stats = monitor.get_stats();
        assert_eq!(stats.current_usage, 0);
        assert_eq!(stats.peak_usage, 0);
        assert_eq!(stats.cleanup_count, 0);

        // Test memory tracking without cleanup threshold
        assert!(!monitor.update_memory_usage(512 * 1024)); // 512KB - should not trigger cleanup
        let stats = monitor.get_stats();
        assert_eq!(stats.current_usage, 512 * 1024);
        assert_eq!(stats.peak_usage, 512 * 1024);

        // Test cleanup triggering
        assert!(monitor.update_memory_usage(1200 * 1024)); // 1.2MB - should trigger cleanup
        let stats = monitor.get_stats();
        assert_eq!(stats.current_usage, 1200 * 1024);
        assert_eq!(stats.peak_usage, 1200 * 1024);

        // Mark cleanup performed and verify
        monitor.mark_cleanup_performed();
        let stats = monitor.get_stats();
        assert_eq!(stats.cleanup_count, 1);
        assert!(stats.last_cleanup.is_some());
    }

    #[test]
    fn test_memory_monitor_aggressive_mode() {
        let config = create_test_memory_config();
        let monitor = MemoryMonitor::new(config);

        // Normal cleanup threshold
        assert!(monitor.update_memory_usage(1200 * 1024));
        monitor.mark_cleanup_performed();

        // Wait for minimum interval to pass
        thread::sleep(Duration::from_millis(250));

        // Aggressive threshold - should allow more frequent cleanup
        assert!(monitor.update_memory_usage(1600 * 1024)); // Above aggressive threshold
        monitor.mark_cleanup_performed();

        // Should be able to cleanup again sooner due to aggressive mode
        thread::sleep(Duration::from_millis(150));
        assert!(monitor.update_memory_usage(1700 * 1024));
    }

    #[test]
    fn test_memory_monitor_min_interval_enforcement() {
        let config = create_test_memory_config();
        let monitor = MemoryMonitor::new(config);

        // Trigger cleanup
        assert!(monitor.update_memory_usage(1200 * 1024));
        monitor.mark_cleanup_performed();

        // Immediate second cleanup should be blocked by min interval
        assert!(!monitor.update_memory_usage(1300 * 1024));

        // After waiting for min interval, should allow cleanup
        thread::sleep(Duration::from_millis(250));
        assert!(monitor.update_memory_usage(1300 * 1024));
    }

    #[test]
    fn test_concurrent_memory_monitoring() {
        let config = create_test_memory_config();
        let monitor = Arc::new(MemoryMonitor::new(config));
        let cleanup_count = Arc::new(AtomicU64::new(0));

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let monitor = monitor.clone();
                let cleanup_count = cleanup_count.clone();
                thread::spawn(move || {
                    for j in 0..10 {
                        let usage = (1000 + i * 100 + j * 10) * 1024; // Varying memory usage
                        if monitor.update_memory_usage(usage) {
                            monitor.mark_cleanup_performed();
                            cleanup_count.fetch_add(1, Ordering::Relaxed);
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        let final_stats = monitor.get_stats();
        let total_cleanups = cleanup_count.load(Ordering::Relaxed);

        // Verify that cleanup was triggered and stats are consistent
        assert!(total_cleanups > 0);
        assert_eq!(final_stats.cleanup_count, total_cleanups);
        assert!(final_stats.peak_usage > 1024 * 1024);
    }

    /// Simulate a long-running session with periodic AI usage
    #[test]
    fn test_long_running_session_simulation() {
        let config = create_test_memory_config();
        let monitor = Arc::new(MemoryMonitor::new(config));
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));

        let monitor_clone = monitor.clone();
        let running_clone = running.clone();

        // Background memory growth simulation
        let growth_handle = thread::spawn(move || {
            let mut current_usage = 0u64;
            let mut iteration = 0;

            while running_clone.load(Ordering::Relaxed) {
                iteration += 1;
                
                // Simulate memory growth pattern
                current_usage += 50 * 1024; // 50KB per iteration
                
                // Occasional larger allocations
                if iteration % 10 == 0 {
                    current_usage += 200 * 1024; // 200KB spike
                }

                // Simulate some memory being freed occasionally
                if iteration % 15 == 0 {
                    current_usage = current_usage.saturating_sub(100 * 1024);
                }

                if monitor_clone.update_memory_usage(current_usage) {
                    // Simulate cleanup reducing memory usage
                    current_usage = (current_usage as f64 * 0.7) as u64; // 30% reduction
                    monitor_clone.mark_cleanup_performed();
                    println!("Cleanup performed at iteration {}, usage: {} KB", 
                             iteration, current_usage / 1024);
                }

                thread::sleep(Duration::from_millis(20));
            }
        });

        // Let simulation run for a reasonable duration
        thread::sleep(Duration::from_millis(2000));
        running.store(false, Ordering::Relaxed);
        growth_handle.join().unwrap();

        let final_stats = monitor.get_stats();
        println!("Final stats - Current: {} KB, Peak: {} KB, Cleanups: {}", 
                 final_stats.current_usage / 1024,
                 final_stats.peak_usage / 1024,
                 final_stats.cleanup_count);

        // Verify memory management was effective
        assert!(final_stats.cleanup_count > 0, "Should have performed cleanup operations");
        assert!(final_stats.peak_usage > 1024 * 1024, "Should have detected significant memory usage");
        assert!(final_stats.current_usage < final_stats.peak_usage, "Current usage should be less than peak due to cleanup");
    }

    #[test]
    fn test_sqlite_cleanup_robustness() {
        use tempfile::NamedTempFile;
        use rusqlite::{Connection, params};

        // Create a temporary SQLite database
        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        // Setup test database with sample data
        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute(
                "CREATE TABLE conversations (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    ts TEXT NOT NULL,
                    mode TEXT NOT NULL,
                    working_directory TEXT,
                    shell_kind TEXT,
                    input TEXT NOT NULL,
                    output TEXT NOT NULL
                )",
                [],
            ).unwrap();

            // Insert test data with various timestamps
            let now = chrono::Utc::now();
            for i in 0..100 {
                let ts = now - chrono::Duration::days(i % 30); // Some old, some recent
                conn.execute(
                    "INSERT INTO conversations (ts, mode, working_directory, shell_kind, input, output)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        ts.to_rfc3339(),
                        "test",
                        "/tmp",
                        "bash",
                        format!("input_{}", i),
                        format!("output_{}", i)
                    ],
                ).unwrap();
            }

            // Insert some invalid entries to test cleanup
            conn.execute(
                "INSERT INTO conversations (ts, mode, working_directory, shell_kind, input, output)
                 VALUES ('', 'test', '/tmp', 'bash', '', 'output')",
                [],
            ).unwrap();
            
            conn.execute(
                "INSERT INTO conversations (ts, mode, working_directory, shell_kind, input, output)
                 VALUES ('2023-01-01T00:00:00Z', 'test', '/tmp', 'bash', 'input', '')",
                [],
            ).unwrap();
        }

        // Test cleanup with temporary environment variables
        std::env::set_var("OPENAGENT_AI_HISTORY_SQLITE_MAX_AGE_DAYS", "15");
        std::env::set_var("OPENAGENT_AI_HISTORY_SQLITE_MAX_ROWS", "50");

        // Perform cleanup operation
        let result = AiRuntime::perform_sqlite_cleanup_internal(db_path);
        assert!(result.is_ok(), "SQLite cleanup should succeed");

        // Verify cleanup results
        {
            let conn = Connection::open(db_path).unwrap();
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversations",
                [],
                |row| row.get(0)
            ).unwrap();

            println!("Remaining conversations after cleanup: {}", count);
            assert!(count <= 50, "Should not exceed max rows limit");
            assert!(count > 0, "Should retain some recent conversations");

            // Verify invalid entries were cleaned up
            let invalid_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversations WHERE ts = '' OR ts IS NULL OR input = '' OR output IS NULL",
                [],
                |row| row.get(0)
            ).unwrap();
            assert_eq!(invalid_count, 0, "Invalid entries should be cleaned up");
        }

        // Clean up environment variables
        std::env::remove_var("OPENAGENT_AI_HISTORY_SQLITE_MAX_AGE_DAYS");
        std::env::remove_var("OPENAGENT_AI_HISTORY_SQLITE_MAX_ROWS");
    }

    #[test]
    fn test_concurrent_sqlite_cleanup() {
        use tempfile::NamedTempFile;
        use rusqlite::{Connection, params};

        let temp_file = NamedTempFile::new().unwrap();
        let db_path = temp_file.path();

        // Setup test database
        {
            let conn = Connection::open(db_path).unwrap();
            conn.execute(
                "CREATE TABLE conversations (
                    id INTEGER PRIMARY KEY,
                    ts TEXT NOT NULL,
                    mode TEXT NOT NULL,
                    working_directory TEXT,
                    shell_kind TEXT,
                    input TEXT NOT NULL,
                    output TEXT NOT NULL
                )",
                [],
            ).unwrap();

            // Add test data
            for i in 0..20 {
                let ts = chrono::Utc::now().to_rfc3339();
                conn.execute(
                    "INSERT INTO conversations (ts, mode, working_directory, shell_kind, input, output)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![ts, "test", "/tmp", "bash", format!("input_{}", i), format!("output_{}", i)],
                ).unwrap();
            }
        }

        // Attempt concurrent cleanups
        let handles: Vec<_> = (0..3)
            .map(|_| {
                let path = db_path.to_path_buf();
                thread::spawn(move || {
                    AiRuntime::perform_sqlite_cleanup_internal(&path)
                })
            })
            .collect();

        let mut success_count = 0;
        let mut skip_count = 0;

        for handle in handles {
            match handle.join().unwrap() {
                Ok(_) => success_count += 1,
                Err(e) => {
                    if e.contains("another cleanup in progress") {
                        skip_count += 1;
                    } else {
                        panic!("Unexpected error: {}", e);
                    }
                }
            }
        }

        println!("Concurrent cleanup results - Success: {}, Skipped: {}", success_count, skip_count);
        
        // At least one should succeed, others should be properly handled
        assert!(success_count >= 1, "At least one cleanup should succeed");
        assert_eq!(success_count + skip_count, 3, "All attempts should be accounted for");

        // Verify database integrity
        {
            let conn = Connection::open(db_path).unwrap();
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversations",
                [],
                |row| row.get(0)
            ).unwrap();
            assert!(count > 0, "Database should still contain data");
        }
    }

    #[test]
    fn test_jsonl_cleanup_functionality() {
        use std::fs;
        use std::io::Write;

        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().join("ai_history");
        fs::create_dir_all(&base_path).unwrap();

        // Create test JSONL files with different ages
        let now = std::time::SystemTime::now();
        let old_time = now - std::time::Duration::from_secs(10 * 24 * 3600); // 10 days old
        let recent_time = now - std::time::Duration::from_secs(1 * 24 * 3600); // 1 day old

        // Create rotated files
        for i in 0..5 {
            let file_name = format!("history-202401{:02}120000.jsonl", i + 1);
            let file_path = base_path.join(&file_name);
            let mut file = fs::File::create(&file_path).unwrap();
            writeln!(file, r#"{{"timestamp":"2024-01-{:02}T12:00:00Z","mode":"test","input":"test","output":"test"}}"#, i + 1).unwrap();
            
            // Set file modification time
            let time_to_set = if i < 2 { old_time } else { recent_time };
            filetime::set_file_mtime(&file_path, filetime::FileTime::from_system_time(time_to_set)).unwrap();
        }

        // Set environment variables for testing
        std::env::set_var("OPENAGENT_AI_HISTORY_JSONL_MAX_AGE_DAYS", "5");
        std::env::set_var("OPENAGENT_AI_HISTORY_ROTATED_KEEP", "3");

        // Perform cleanup
        let result = AiRuntime::cleanup_jsonl_history();
        assert!(result.is_ok(), "JSONL cleanup should succeed");

        // Verify cleanup results
        let remaining_files: Vec<_> = fs::read_dir(&base_path)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry.file_name().to_string_lossy().starts_with("history-") &&
                entry.file_name().to_string_lossy().ends_with(".jsonl")
            })
            .collect();

        println!("Remaining JSONL files: {}", remaining_files.len());
        for file in &remaining_files {
            println!("  - {}", file.file_name().to_string_lossy());
        }

        // Should keep recent files and respect the limit
        assert!(remaining_files.len() <= 3, "Should not exceed file limit");
        assert!(remaining_files.len() > 0, "Should keep some recent files");

        // Clean up environment variables
        std::env::remove_var("OPENAGENT_AI_HISTORY_JSONL_MAX_AGE_DAYS");
        std::env::remove_var("OPENAGENT_AI_HISTORY_ROTATED_KEEP");
    }

    #[test]
    fn test_cache_cleanup_functionality() {
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        
        // Simulate cache directories
        let cache_dirs = vec![
            temp_dir.path().join("openagent-terminal").join("warp_history_cache"),
            temp_dir.path().join("openagent-terminal").join("embeddings"),
            temp_dir.path().join("openagent-terminal").join("similarity_cache"),
            temp_dir.path().join("openagent-terminal").join("openai_cache"),
            temp_dir.path().join("openagent-terminal").join("anthropic_cache"),
        ];

        // Create cache directories and files
        let now = std::time::SystemTime::now();
        let old_time = now - std::time::Duration::from_secs(30 * 24 * 3600); // 30 days old
        let recent_time = now - std::time::Duration::from_secs(1 * 24 * 3600); // 1 day old

        for (i, cache_dir) in cache_dirs.iter().enumerate() {
            fs::create_dir_all(cache_dir).unwrap();
            
            // Create some old and recent cache files
            for j in 0..3 {
                let file_path = cache_dir.join(format!("cache_file_{}_{}.dat", i, j));
                fs::write(&file_path, format!("cache data {} {}", i, j)).unwrap();
                
                // Set modification time
                let time_to_set = if j == 0 { old_time } else { recent_time };
                filetime::set_file_mtime(&file_path, filetime::FileTime::from_system_time(time_to_set)).unwrap();
            }
        }

        // Mock the cache directory lookup by temporarily setting XDG_CACHE_HOME
        std::env::set_var("XDG_CACHE_HOME", temp_dir.path());

        // Test individual cleanup functions
        let results = vec![
            AiRuntime::cleanup_warp_history_caches(),
            AiRuntime::cleanup_ai_embeddings_cache(),
            AiRuntime::cleanup_provider_caches(),
        ];

        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Cache cleanup {} should succeed", i);
        }

        // Verify that old files were cleaned up and recent files remain
        for cache_dir in &cache_dirs {
            if cache_dir.exists() {
                let remaining_files: Vec<_> = fs::read_dir(cache_dir)
                    .unwrap()
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.file_type().unwrap().is_file())
                    .collect();

                println!("Cache dir: {}, remaining files: {}", 
                         cache_dir.display(), remaining_files.len());

                // Should have fewer files due to cleanup (exact count depends on age thresholds)
                assert!(remaining_files.len() < 3, 
                        "Should have cleaned up some old files in {}", cache_dir.display());
            }
        }

        // Clean up environment
        std::env::remove_var("XDG_CACHE_HOME");
    }

    #[test]
    fn test_memory_estimation_accuracy() {
        let initial_estimate = AiRuntime::estimate_ai_memory_usage();
        println!("Initial memory estimate: {} KB", initial_estimate / 1024);
        
        // Memory estimate should be reasonable (at least 1MB, less than 100MB for basic case)
        assert!(initial_estimate >= 1024 * 1024, "Should estimate at least 1MB base usage");
        assert!(initial_estimate <= 100 * 1024 * 1024, "Should not estimate excessive usage");

        // Multiple calls should return consistent results (within reason)
        let second_estimate = AiRuntime::estimate_ai_memory_usage();
        let difference = if initial_estimate > second_estimate {
            initial_estimate - second_estimate
        } else {
            second_estimate - initial_estimate
        };
        
        // Allow for some variation but not dramatic differences
        assert!(difference < initial_estimate / 2, 
                "Memory estimates should be reasonably consistent");
    }

    /// Integration test that simulates a realistic AI session with cleanup
    #[test]
    fn test_ai_runtime_memory_integration() {
        let null_provider = Box::new(openagent_terminal_ai::NullProvider);
        let mut runtime = AiRuntime::new(null_provider);
        
        // Configure for aggressive testing
        runtime.memory_monitor.config = create_test_memory_config();

        // Simulate a session with multiple interactions
        for i in 0..10 {
            runtime.ui.scratch = format!("test query {}", i);
            runtime.ui.history.push_front(format!("history entry {}", i));
            
            // Simulate proposal responses
            runtime.ui.proposals = vec![
                AiProposal {
                    title: format!("Proposal {}", i),
                    description: Some(format!("Description {}", i)),
                    proposed_commands: vec![format!("command_{}", i)],
                }
            ];

            // Trigger manual cleanup occasionally
            if i % 3 == 0 {
                runtime.trigger_memory_cleanup();
            }
        }

        // Get final memory statistics
        let stats = runtime.get_memory_stats();
        println!("Integration test final stats:");
        println!("  Current usage: {} KB", stats.current_usage / 1024);
        println!("  Peak usage: {} KB", stats.peak_usage / 1024);
        println!("  Cleanup count: {}", stats.cleanup_count);

        // Verify cleanup was triggered
        assert!(stats.cleanup_count > 0, "Should have performed cleanup operations");
        
        // Stop background cleanup
        runtime.stop_background_cleanup();
    }
}

#[cfg(test)]
mod ai_memory_benchmarks {
    use super::*;
    use std::time::Instant;

    /// Benchmark memory monitoring performance
    #[test]
    fn bench_memory_monitoring() {
        let config = MemoryMonitorConfig::default();
        let monitor = MemoryMonitor::new(config);

        let start = Instant::now();
        let iterations = 10000;

        for i in 0..iterations {
            monitor.update_memory_usage((i % 1000) * 1024);
        }

        let duration = start.elapsed();
        println!("Memory monitoring benchmark:");
        println!("  {} iterations in {:?}", iterations, duration);
        println!("  Average per operation: {:?}", duration / iterations);

        // Should be very fast
        assert!(duration.as_millis() < 1000, "Memory monitoring should be efficient");
    }

    /// Benchmark cleanup operations
    #[test]
    fn bench_cleanup_operations() {
        let start = Instant::now();

        // Test different cleanup operations
        let operations = vec![
            ("memory estimation", || AiRuntime::estimate_ai_memory_usage()),
        ];

        for (name, op) in operations {
            let op_start = Instant::now();
            let _ = op();
            let op_duration = op_start.elapsed();
            println!("Benchmark {}: {:?}", name, op_duration);
        }

        let total_duration = start.elapsed();
        println!("Total benchmark time: {:?}", total_duration);

        // All operations should complete within reasonable time
        assert!(total_duration.as_millis() < 5000, "Cleanup operations should be efficient");
    }
}