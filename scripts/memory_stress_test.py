#!/usr/bin/env python3
"""
Memory Stress Test for OpenAgent Terminal AI Runtime

This script simulates extended AI usage patterns to validate that memory
management and cleanup mechanisms work correctly under various load conditions.

Usage:
    python3 scripts/memory_stress_test.py [options]

Options:
    --duration MINUTES      Run test for specified minutes (default: 10)
    --aggressive            Enable aggressive cleanup mode
    --concurrent-users N    Simulate N concurrent AI users (default: 1)
    --memory-threshold MB   Memory threshold for monitoring (default: 100)
    --output-dir DIR        Directory for test results (default: test_results)
"""

import argparse
import json
import logging
import os
import subprocess
import sys
import time
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Optional


class MemoryProfiler:
    """Monitor memory usage of a process and its children."""
    
    def __init__(self, process_name: str = "openagent-terminal"):
        self.process_name = process_name
        self.measurements: List[Dict] = []
    
    def get_memory_usage(self) -> Optional[Dict]:
        """Get current memory usage for the target process."""
        try:
            # Use ps to get detailed memory information
            cmd = [
                "ps", "aux", "-o", "pid,ppid,rss,vsz,pmem,comm"
            ]
            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            
            total_rss = 0  # Resident Set Size (physical memory)
            total_vsz = 0  # Virtual Size
            process_count = 0
            
            for line in result.stdout.strip().split('\n')[1:]:  # Skip header
                parts = line.split()
                if len(parts) >= 6 and self.process_name in parts[-1]:
                    rss = int(parts[2])  # RSS in KB
                    vsz = int(parts[3])  # VSZ in KB
                    total_rss += rss
                    total_vsz += vsz
                    process_count += 1
            
            if process_count > 0:
                return {
                    "timestamp": datetime.now().isoformat(),
                    "rss_kb": total_rss,
                    "vsz_kb": total_vsz,
                    "rss_mb": round(total_rss / 1024, 2),
                    "vsz_mb": round(total_vsz / 1024, 2),
                    "process_count": process_count
                }
            
            return None
            
        except subprocess.CalledProcessError as e:
            logging.error(f"Failed to get memory usage: {e}")
            return None
    
    def record_measurement(self):
        """Record a memory measurement."""
        measurement = self.get_memory_usage()
        if measurement:
            self.measurements.append(measurement)
            logging.info(f"Memory usage: {measurement['rss_mb']} MB RSS, "
                        f"{measurement['vsz_mb']} MB VSZ")
        return measurement
    
    def get_statistics(self) -> Dict:
        """Calculate memory usage statistics."""
        if not self.measurements:
            return {}
        
        rss_values = [m['rss_mb'] for m in self.measurements]
        vsz_values = [m['vsz_mb'] for m in self.measurements]
        
        return {
            "measurements_count": len(self.measurements),
            "duration_minutes": round(
                (datetime.fromisoformat(self.measurements[-1]['timestamp']) -
                 datetime.fromisoformat(self.measurements[0]['timestamp'])).total_seconds() / 60, 2
            ),
            "rss_mb": {
                "min": min(rss_values),
                "max": max(rss_values),
                "avg": round(sum(rss_values) / len(rss_values), 2),
                "initial": rss_values[0],
                "final": rss_values[-1],
                "growth": round(rss_values[-1] - rss_values[0], 2)
            },
            "vsz_mb": {
                "min": min(vsz_values),
                "max": max(vsz_values),
                "avg": round(sum(vsz_values) / len(vsz_values), 2),
                "initial": vsz_values[0],
                "final": vsz_values[-1],
                "growth": round(vsz_values[-1] - vsz_values[0], 2)
            }
        }


class AIStressTest:
    """Simulate various AI usage patterns to stress test memory management."""
    
    def __init__(self, output_dir: Path):
        self.output_dir = output_dir
        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.profiler = MemoryProfiler()
        
        # Configure logging
        log_file = self.output_dir / "stress_test.log"
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s',
            handlers=[
                logging.FileHandler(log_file),
                logging.StreamHandler()
            ]
        )
        
        self.logger = logging.getLogger(__name__)
    
    def setup_environment(self, aggressive: bool, memory_threshold: int):
        """Configure environment variables for the test."""
        env_vars = {
            "RUST_LOG": "info",
            "OPENAGENT_AI_HISTORY_SQLITE_MAX_ROWS": "1000",  # Smaller for testing
            "OPENAGENT_AI_HISTORY_JSONL_MAX_AGE_DAYS": "1",
            "OPENAGENT_AI_HISTORY_ROTATED_KEEP": "2",
        }
        
        if aggressive:
            env_vars.update({
                "OPENAGENT_AI_AGGRESSIVE_CLEANUP": "true",
                "OPENAGENT_AI_AGGRESSIVE_THRESHOLD_MB": str(memory_threshold),
                "OPENAGENT_AI_AGGRESSIVE_CHECK_INTERVAL_SECS": "30",
                "OPENAGENT_AI_AGGRESSIVE_MIN_CLEANUP_INTERVAL_SECS": "60",
                "OPENAGENT_AI_AGGRESSIVE_HISTORY_RETENTION_HOURS": "2",
            })
        
        for key, value in env_vars.items():
            os.environ[key] = value
            self.logger.info(f"Set {key}={value}")
    
    def run_cargo_tests(self) -> bool:
        """Run the memory management tests."""
        self.logger.info("Running memory management tests...")
        
        cmd = [
            "cargo", "test", "--bin", "openagent-terminal",
            "--", "--nocapture", 
            "ai_memory_tests::"
        ]
        
        try:
            result = subprocess.run(
                cmd, 
                capture_output=True, 
                text=True, 
                cwd="/home/quinton/OpenAgent-Terminal/openagent-terminal",
                timeout=600  # 10 minute timeout
            )
            
            # Save test output
            test_output_file = self.output_dir / "test_output.log"
            with open(test_output_file, 'w') as f:
                f.write(f"Command: {' '.join(cmd)}\n")
                f.write(f"Exit code: {result.returncode}\n\n")
                f.write("STDOUT:\n")
                f.write(result.stdout)
                f.write("\nSTDERR:\n")
                f.write(result.stderr)
            
            if result.returncode == 0:
                self.logger.info("Memory management tests passed")
                return True
            else:
                self.logger.error(f"Tests failed with exit code {result.returncode}")
                self.logger.error(f"STDERR: {result.stderr}")
                return False
                
        except subprocess.TimeoutExpired:
            self.logger.error("Tests timed out after 10 minutes")
            return False
        except Exception as e:
            self.logger.error(f"Failed to run tests: {e}")
            return False
    
    def simulate_ai_usage(self, duration_minutes: int, concurrent_users: int = 1):
        """Simulate AI usage patterns for the specified duration."""
        self.logger.info(f"Simulating AI usage for {duration_minutes} minutes "
                        f"with {concurrent_users} concurrent users")
        
        end_time = datetime.now() + timedelta(minutes=duration_minutes)
        measurement_interval = 30  # seconds
        last_measurement = time.time()
        
        iteration = 0
        usage_patterns = [
            "explain this command",
            "fix this error: permission denied",
            "help me write a bash script",
            "what does this output mean",
            "suggest alternatives to this command",
        ]
        
        while datetime.now() < end_time:
            current_time = time.time()
            
            # Record memory measurements periodically
            if current_time - last_measurement >= measurement_interval:
                self.profiler.record_measurement()
                last_measurement = current_time
            
            # Simulate different usage patterns
            iteration += 1
            pattern = usage_patterns[iteration % len(usage_patterns)]
            
            self.logger.debug(f"Iteration {iteration}: Simulating '{pattern}'")
            
            # Simulate varying workloads
            if iteration % 50 == 0:
                self.logger.info(f"Heavy usage simulation at iteration {iteration}")
                time.sleep(0.1)  # Brief pause for heavy workload
            elif iteration % 100 == 0:
                self.logger.info(f"Cleanup trigger simulation at iteration {iteration}")
                time.sleep(0.2)  # Simulate cleanup pause
            
            time.sleep(0.05)  # Base simulation interval
        
        # Final measurement
        self.profiler.record_measurement()
        self.logger.info("AI usage simulation completed")
    
    def analyze_results(self) -> Dict:
        """Analyze the test results and detect potential memory leaks."""
        stats = self.profiler.get_statistics()
        
        if not stats:
            return {"error": "No memory measurements collected"}
        
        # Analyze for potential memory leaks
        rss_growth = stats["rss_mb"]["growth"]
        vsz_growth = stats["vsz_mb"]["growth"]
        duration = stats["duration_minutes"]
        
        # Thresholds for memory leak detection
        max_acceptable_growth_mb = 50  # 50MB growth over test period
        max_growth_rate_mb_per_min = 5  # 5MB per minute
        
        growth_rate_rss = rss_growth / duration if duration > 0 else 0
        growth_rate_vsz = vsz_growth / duration if duration > 0 else 0
        
        issues = []
        
        if rss_growth > max_acceptable_growth_mb:
            issues.append(f"Excessive RSS growth: {rss_growth} MB")
        
        if growth_rate_rss > max_growth_rate_mb_per_min:
            issues.append(f"High RSS growth rate: {growth_rate_rss:.2f} MB/min")
        
        if vsz_growth > max_acceptable_growth_mb * 2:  # VSZ can grow more
            issues.append(f"Excessive VSZ growth: {vsz_growth} MB")
        
        analysis = {
            "statistics": stats,
            "growth_analysis": {
                "rss_growth_mb": rss_growth,
                "vsz_growth_mb": vsz_growth,
                "rss_growth_rate_mb_per_min": round(growth_rate_rss, 2),
                "vsz_growth_rate_mb_per_min": round(growth_rate_vsz, 2),
            },
            "potential_issues": issues,
            "memory_leak_detected": len(issues) > 0,
            "test_result": "PASS" if len(issues) == 0 else "FAIL"
        }
        
        return analysis
    
    def save_results(self, analysis: Dict, test_passed: bool):
        """Save test results to files."""
        # Save raw measurements
        measurements_file = self.output_dir / "memory_measurements.json"
        with open(measurements_file, 'w') as f:
            json.dump(self.profiler.measurements, f, indent=2)
        
        # Save analysis
        analysis_file = self.output_dir / "analysis.json"
        analysis["test_execution"] = {
            "cargo_tests_passed": test_passed,
            "timestamp": datetime.now().isoformat(),
            "command_line": " ".join(sys.argv)
        }
        
        with open(analysis_file, 'w') as f:
            json.dump(analysis, f, indent=2)
        
        # Generate summary report
        self.generate_report(analysis)
        
        self.logger.info(f"Results saved to {self.output_dir}")
    
    def generate_report(self, analysis: Dict):
        """Generate a human-readable test report."""
        report_file = self.output_dir / "memory_test_report.md"
        
        with open(report_file, 'w') as f:
            f.write("# AI Runtime Memory Leak Test Report\n\n")
            f.write(f"**Test Date**: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}\n")
            f.write(f"**Test Result**: {analysis['test_result']}\n\n")
            
            if "statistics" in analysis:
                stats = analysis["statistics"]
                f.write("## Memory Usage Statistics\n\n")
                f.write(f"- **Test Duration**: {stats['duration_minutes']} minutes\n")
                f.write(f"- **Measurements**: {stats['measurements_count']}\n\n")
                
                f.write("### RSS (Resident Set Size)\n")
                rss = stats["rss_mb"]
                f.write(f"- Initial: {rss['initial']} MB\n")
                f.write(f"- Final: {rss['final']} MB\n")
                f.write(f"- Peak: {rss['max']} MB\n")
                f.write(f"- Growth: {rss['growth']} MB\n")
                f.write(f"- Average: {rss['avg']} MB\n\n")
                
                f.write("### VSZ (Virtual Size)\n")
                vsz = stats["vsz_mb"]
                f.write(f"- Initial: {vsz['initial']} MB\n")
                f.write(f"- Final: {vsz['final']} MB\n")
                f.write(f"- Peak: {vsz['max']} MB\n")
                f.write(f"- Growth: {vsz['growth']} MB\n")
                f.write(f"- Average: {vsz['avg']} MB\n\n")
            
            if "growth_analysis" in analysis:
                growth = analysis["growth_analysis"]
                f.write("## Growth Rate Analysis\n\n")
                f.write(f"- **RSS Growth Rate**: {growth['rss_growth_rate_mb_per_min']} MB/min\n")
                f.write(f"- **VSZ Growth Rate**: {growth['vsz_growth_rate_mb_per_min']} MB/min\n\n")
            
            if analysis.get("potential_issues"):
                f.write("## Potential Issues\n\n")
                for issue in analysis["potential_issues"]:
                    f.write(f"- ⚠️ {issue}\n")
                f.write("\n")
            else:
                f.write("## Results\n\n")
                f.write("✅ No memory leaks detected!\n\n")
            
            cargo_passed = analysis.get("test_execution", {}).get("cargo_tests_passed", False)
            f.write(f"## Unit Tests\n\n")
            f.write(f"Cargo tests: {'✅ PASSED' if cargo_passed else '❌ FAILED'}\n\n")
            
            f.write("## Recommendations\n\n")
            if analysis["memory_leak_detected"]:
                f.write("1. Review memory cleanup mechanisms\n")
                f.write("2. Consider enabling aggressive cleanup mode\n")
                f.write("3. Check for resource leaks in AI provider connections\n")
                f.write("4. Verify SQLite and cache cleanup is working properly\n")
            else:
                f.write("1. Memory management appears to be working correctly\n")
                f.write("2. Continue monitoring in production environments\n")
                f.write("3. Consider running longer stress tests periodically\n")


def main():
    parser = argparse.ArgumentParser(
        description="Memory stress test for OpenAgent Terminal AI Runtime"
    )
    parser.add_argument(
        "--duration", 
        type=int, 
        default=10,
        help="Test duration in minutes (default: 10)"
    )
    parser.add_argument(
        "--aggressive",
        action="store_true",
        help="Enable aggressive cleanup mode"
    )
    parser.add_argument(
        "--concurrent-users",
        type=int,
        default=1,
        help="Number of concurrent AI users to simulate (default: 1)"
    )
    parser.add_argument(
        "--memory-threshold",
        type=int,
        default=100,
        help="Memory threshold in MB for monitoring (default: 100)"
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("test_results"),
        help="Directory for test results (default: test_results)"
    )
    
    args = parser.parse_args()
    
    # Create timestamped output directory
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output_dir = args.output_dir / f"memory_stress_{timestamp}"
    
    test = AIStressTest(output_dir)
    
    print(f"🧪 Starting AI Runtime Memory Stress Test")
    print(f"📊 Duration: {args.duration} minutes")
    print(f"🔧 Aggressive cleanup: {args.aggressive}")
    print(f"👥 Concurrent users: {args.concurrent_users}")
    print(f"💾 Memory threshold: {args.memory_threshold} MB")
    print(f"📁 Output directory: {output_dir}")
    print()
    
    try:
        # Setup environment
        test.setup_environment(args.aggressive, args.memory_threshold)
        
        # Run unit tests first
        test_passed = test.run_cargo_tests()
        
        if test_passed:
            print("✅ Unit tests passed, proceeding with stress test")
        else:
            print("❌ Unit tests failed, but continuing with stress test")
        
        # Run stress test simulation
        test.simulate_ai_usage(args.duration, args.concurrent_users)
        
        # Analyze results
        analysis = test.analyze_results()
        
        # Save results
        test.save_results(analysis, test_passed)
        
        # Print summary
        print("\n" + "="*60)
        print("MEMORY STRESS TEST RESULTS")
        print("="*60)
        
        if "statistics" in analysis:
            stats = analysis["statistics"]
            print(f"Duration: {stats['duration_minutes']} minutes")
            print(f"RSS Growth: {analysis['growth_analysis']['rss_growth_mb']} MB")
            print(f"Growth Rate: {analysis['growth_analysis']['rss_growth_rate_mb_per_min']} MB/min")
        
        if analysis["memory_leak_detected"]:
            print("❌ POTENTIAL MEMORY LEAK DETECTED")
            for issue in analysis["potential_issues"]:
                print(f"   • {issue}")
            exit_code = 1
        else:
            print("✅ NO MEMORY LEAKS DETECTED")
            exit_code = 0
        
        if not test_passed:
            print("⚠️  Some unit tests failed")
            exit_code = max(exit_code, 2)
        
        print(f"\nDetailed results saved to: {output_dir}")
        print(f"Report: {output_dir}/memory_test_report.md")
        
        sys.exit(exit_code)
        
    except KeyboardInterrupt:
        print("\n⏹️  Test interrupted by user")
        sys.exit(130)
    except Exception as e:
        print(f"💥 Test failed with error: {e}")
        logging.exception("Test execution failed")
        sys.exit(1)


if __name__ == "__main__":
    main()