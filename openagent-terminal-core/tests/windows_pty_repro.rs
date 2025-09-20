#![cfg(windows)]

use std::io::{Read, Write};
use std::time::{Duration, Instant};

use openagent_terminal_core::event::WindowSize;
use openagent_terminal_core::tty::{EventedPty, Options, Shell};

/// Utility: drain any currently available bytes from PTY to avoid backpressure.
fn drain_available<T: EventedPty>(pty: &mut T) -> u64 {
    let mut total = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        match pty.reader().read(&mut buf) {
            Ok(0) => break,             // EOF or no data
            Ok(n) => total += n as u64, // drained some
            Err(_) => break,            // WouldBlock or other benign error
        }
        // If we didn't fill the buffer, try once more; otherwise continue draining.
        if total == 0 {
            break;
        }
    }
    total
}

fn default_ws() -> WindowSize {
    WindowSize { num_cols: 100, num_lines: 30, cell_width: 8, cell_height: 16 }
}

/// Wait for child exit, draining output periodically to avoid deadlocks.
fn wait_for_exit<T: EventedPty>(pty: &mut T, timeout: Duration) -> Option<i32> {
    let start = Instant::now();
    let mut last_drain = Instant::now();
    loop {
        if let Some(ev) = pty.next_child_event() {
            match ev {
                openagent_terminal_core::tty::ChildEvent::Exited(code) => return code,
            }
        }
        // Periodically drain to keep pipes flowing
        if last_drain.elapsed() >= Duration::from_millis(25) {
            let _ = drain_available(pty);
            last_drain = Instant::now();
        }
        if start.elapsed() > timeout {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn pty_powershell_interactive_resize_then_exit() {
    // Spawn an interactive PowerShell, perform resizes, then send exit.
    let shell = Shell::new(
        "powershell.exe".to_string(),
        vec!["-NoProfile".to_string(), "-NoLogo".to_string()],
    );
    let mut opts = Options {
        shell: Some(shell),
        working_directory: None,
        drain_on_exit: true,
        env: Default::default(),
        #[cfg(target_os = "windows")]
        escape_args: true,
    };

    let ws = default_ws();
    let window_id = 1u64;

    let mut pty = match openagent_terminal_core::tty::new(&opts, ws, window_id) {
        Ok(p) => p,
        Err(e) => panic!("Failed to create PTY: {e}"),
    };

    // Small startup grace
    std::thread::sleep(Duration::from_millis(200));

    // Perform a few resizes to exercise ConPTY resize path.
    for i in 0..5u16 {
        let ws2 = WindowSize {
            num_cols: 100 + (i as u16 % 5) as u16,
            num_lines: 30 + (i as u16 % 5) as u16,
            cell_width: 8,
            cell_height: 16,
        };
        pty.on_resize(ws2);
        std::thread::sleep(Duration::from_millis(30));
    }

    // Send a small command then exit to ensure input path works without poller registration.
    let _ = pty.writer().write(b"Write-Output 'ready'\r\nexit\r\n");

    let exit = wait_for_exit(&mut pty, Duration::from_secs(10));
    assert!(exit.is_some(), "PowerShell did not exit within timeout");
}

#[test]
fn pty_cmd_noninteractive_large_burst_exits() {
    // Run a non-interactive burst of output and ensure no hangs due to backpressure.
    // This intentionally creates >64KB of output to exercise pipe draining.
    let shell = Shell::new(
        "cmd.exe".to_string(),
        vec![
            "/C".to_string(),
            // 5000 lines of moderate width text
            "for /L %i in (1,1,5000) do @echo this_is_a_test_line_%i_ABCDEFGHIJKLMNOPQRSTUVWXYZ"
                .to_string(),
        ],
    );
    let opts = Options {
        shell: Some(shell),
        working_directory: None,
        drain_on_exit: true,
        env: Default::default(),
        #[cfg(target_os = "windows")]
        escape_args: true,
    };

    let ws = default_ws();
    let window_id = 2u64;

    let mut pty = openagent_terminal_core::tty::new(&opts, ws, window_id)
        .expect("Failed to create PTY for cmd.exe");

    // Drain until exit, with a slightly longer timeout to allow for output.
    let exit = wait_for_exit(&mut pty, Duration::from_secs(20));
    assert!(exit.is_some(), "cmd.exe did not exit within timeout");
    // cmd.exe typically returns 0 for successful FOR loop execution.
    assert_eq!(exit.unwrap_or_default(), 0);
}
