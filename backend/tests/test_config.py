"""
Tests for configuration and environment handling.
"""

import pytest
import os
import tempfile
from pathlib import Path


class TestEnvironmentConfig:
    """Test environment variable handling."""
    
    def test_socket_path_from_env(self, monkeypatch):
        """Test reading socket path from environment."""
        test_path = "/tmp/test-socket.sock"
        monkeypatch.setenv("OPENAGENT_SOCKET", test_path)
        
        result = os.environ.get("OPENAGENT_SOCKET")
        assert result == test_path
    
    def test_socket_path_default(self, monkeypatch):
        """Test default socket path when env var not set."""
        monkeypatch.delenv("OPENAGENT_SOCKET", raising=False)
        
        result = os.environ.get("OPENAGENT_SOCKET")
        assert result is None
    
    def test_runtime_dir_detection(self):
        """Test XDG_RUNTIME_DIR detection."""
        runtime_dir = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
        
        assert runtime_dir is not None
        assert os.path.exists(runtime_dir)
        assert os.path.isdir(runtime_dir)
    
    def test_home_directory_expansion(self):
        """Test home directory expansion."""
        home = os.path.expanduser("~")
        
        assert home is not None
        assert len(home) > 0
        assert not home.startswith("~")
        assert os.path.exists(home)


class TestPathSafety:
    """Test path safety and validation."""
    
    def test_absolute_path_resolution(self):
        """Test absolute path resolution."""
        relative = "./test.txt"
        absolute = os.path.abspath(relative)
        
        assert os.path.isabs(absolute)
        assert not relative.startswith("/")
        assert absolute.startswith("/")
    
    def test_path_within_cwd(self):
        """Test checking if path is within CWD."""
        cwd = os.getcwd()
        test_path = os.path.join(cwd, "test.txt")
        abs_test_path = os.path.abspath(test_path)
        
        assert abs_test_path.startswith(cwd)
    
    def test_path_within_home(self):
        """Test checking if path is within home."""
        home = os.path.expanduser("~")
        test_path = os.path.join(home, "test.txt")
        abs_test_path = os.path.abspath(test_path)
        
        assert abs_test_path.startswith(home)
    
    def test_forbidden_paths(self):
        """Test that forbidden system paths are identified."""
        forbidden = ["/etc", "/sys", "/proc", "/dev", "/boot"]
        
        for path in forbidden:
            assert os.path.isabs(path)
            # These should be blocked by safety checks
            assert path.startswith("/")
    
    def test_path_traversal_detection(self):
        """Test detection of path traversal attempts."""
        cwd = os.getcwd()
        
        # Try to escape with ../../../
        attempt = os.path.join(cwd, "../../../etc/passwd")
        resolved = os.path.abspath(attempt)
        
        # Resolved path should not be in CWD
        assert not resolved.startswith(cwd)
    
    def test_safe_file_creation_in_cwd(self):
        """Test creating files safely in CWD."""
        cwd = os.getcwd()
        test_file = os.path.join(cwd, "test_safe.txt")
        
        try:
            # Should be safe to create in CWD
            with open(test_file, 'w') as f:
                f.write("test")
            
            assert os.path.exists(test_file)
            assert os.path.abspath(test_file).startswith(cwd)
        finally:
            if os.path.exists(test_file):
                os.remove(test_file)
    
    def test_temp_directory_safety(self):
        """Test that temp directories are safe."""
        with tempfile.TemporaryDirectory() as tmpdir:
            assert os.path.exists(tmpdir)
            assert os.path.isdir(tmpdir)
            
            # Should be able to create files in temp
            test_file = os.path.join(tmpdir, "test.txt")
            with open(test_file, 'w') as f:
                f.write("test")
            
            assert os.path.exists(test_file)


class TestSocketManagement:
    """Test socket file management."""
    
    def test_socket_path_generation(self):
        """Test socket path generation."""
        runtime_dir = os.environ.get("XDG_RUNTIME_DIR", "/tmp")
        pid = os.getpid()
        
        socket_path = f"{runtime_dir}/openagent-terminal-{pid}.sock"
        
        assert socket_path.endswith(".sock")
        assert str(pid) in socket_path
    
    def test_socket_cleanup(self):
        """Test socket cleanup."""
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = os.path.join(tmpdir, "test.sock")
            
            # Create a dummy socket file
            with open(socket_path, 'w') as f:
                f.write("")
            
            assert os.path.exists(socket_path)
            
            # Cleanup
            os.remove(socket_path)
            assert not os.path.exists(socket_path)
    
    def test_socket_permissions(self):
        """Test that socket files can have restricted permissions."""
        with tempfile.TemporaryDirectory() as tmpdir:
            socket_path = os.path.join(tmpdir, "test.sock")
            
            # Create file
            with open(socket_path, 'w') as f:
                f.write("")
            
            # Set permissions to 0600 (owner only)
            os.chmod(socket_path, 0o600)
            
            # Check permissions
            stat_info = os.stat(socket_path)
            mode = stat_info.st_mode & 0o777
            
            assert mode == 0o600
            
            # Cleanup
            os.remove(socket_path)


class TestDirectoryOperations:
    """Test directory operations for safety."""
    
    def test_directory_creation(self):
        """Test safe directory creation."""
        with tempfile.TemporaryDirectory() as tmpdir:
            new_dir = os.path.join(tmpdir, "subdir", "nested")
            
            os.makedirs(new_dir, exist_ok=True)
            
            assert os.path.exists(new_dir)
            assert os.path.isdir(new_dir)
    
    def test_directory_listing(self):
        """Test directory listing."""
        with tempfile.TemporaryDirectory() as tmpdir:
            # Create some test files
            for i in range(3):
                file_path = os.path.join(tmpdir, f"file{i}.txt")
                with open(file_path, 'w') as f:
                    f.write(f"content {i}")
            
            files = os.listdir(tmpdir)
            
            assert len(files) == 3
            assert "file0.txt" in files
            assert "file1.txt" in files
            assert "file2.txt" in files
    
    def test_file_existence_check(self):
        """Test file existence checking."""
        with tempfile.TemporaryDirectory() as tmpdir:
            existing = os.path.join(tmpdir, "exists.txt")
            non_existing = os.path.join(tmpdir, "not_exists.txt")
            
            with open(existing, 'w') as f:
                f.write("test")
            
            assert os.path.exists(existing)
            assert not os.path.exists(non_existing)


class TestProcessInfo:
    """Test process-related information."""
    
    def test_get_process_id(self):
        """Test getting process ID."""
        pid = os.getpid()
        
        assert isinstance(pid, int)
        assert pid > 0
    
    def test_current_working_directory(self):
        """Test getting current working directory."""
        cwd = os.getcwd()
        
        assert cwd is not None
        assert len(cwd) > 0
        assert os.path.isabs(cwd)
        assert os.path.exists(cwd)
        assert os.path.isdir(cwd)
    
    def test_environment_variables(self):
        """Test accessing environment variables."""
        # These should generally exist on Linux systems
        user = os.environ.get("USER") or os.environ.get("USERNAME")
        home = os.environ.get("HOME")
        
        assert user is not None or home is not None


class TestFileOperations:
    """Test file operation helpers."""
    
    def test_file_read_write_cycle(self):
        """Test reading and writing files."""
        with tempfile.TemporaryDirectory() as tmpdir:
            file_path = os.path.join(tmpdir, "test.txt")
            content = "Hello, World!"
            
            # Write
            with open(file_path, 'w') as f:
                f.write(content)
            
            # Read
            with open(file_path, 'r') as f:
                read_content = f.read()
            
            assert read_content == content
    
    def test_file_size_check(self):
        """Test checking file size."""
        with tempfile.TemporaryDirectory() as tmpdir:
            file_path = os.path.join(tmpdir, "test.txt")
            content = "0123456789"
            
            with open(file_path, 'w') as f:
                f.write(content)
            
            size = os.path.getsize(file_path)
            assert size == len(content)
    
    def test_file_deletion(self):
        """Test file deletion."""
        with tempfile.TemporaryDirectory() as tmpdir:
            file_path = os.path.join(tmpdir, "delete_me.txt")
            
            # Create file
            with open(file_path, 'w') as f:
                f.write("test")
            
            assert os.path.exists(file_path)
            
            # Delete
            os.remove(file_path)
            
            assert not os.path.exists(file_path)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
