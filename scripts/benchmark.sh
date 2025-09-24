#!/bin/bash
set -euo pipefail

# Benchmark Runner Script for OpenAgent Terminal
# Provides comprehensive performance testing with CI/CD integration

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BENCHMARK_OUTPUT_DIR="${PROJECT_ROOT}/target/criterion"
RESULTS_DIR="${PROJECT_ROOT}/benchmark-results"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Default options
FEATURES="ai wgpu"
PROFILE="bench"
OUTPUT_FORMAT="html"
COMPARE_BASELINE=""
SAVE_BASELINE=""
VERBOSE=false
QUICK_MODE=false

show_help() {
    cat << EOF
OpenAgent Terminal Benchmark Runner

Usage: $0 [OPTIONS] [BENCHMARK_SUITE]

BENCHMARK_SUITE:
    all             Run all benchmark suites (default)
    core            Core terminal functionality
    ai              AI agent performance (requires 'ai' feature)
    startup         Terminal startup performance
    render          Rendering performance (requires 'wgpu' feature)

OPTIONS:
    -f, --features FEATURES   Cargo features to enable (default: "$FEATURES")
    -p, --profile PROFILE     Build profile to use (default: "$PROFILE")
    -o, --output FORMAT       Output format: html, json, csv (default: "$OUTPUT_FORMAT")
    -b, --baseline NAME       Compare against saved baseline
    -s, --save-baseline NAME  Save results as baseline
    -q, --quick              Quick mode with reduced sample sizes
    -v, --verbose            Verbose output
    -h, --help              Show this help

Examples:
    $0                                    # Run all benchmarks
    $0 core                              # Run only core benchmarks
    $0 ai -f "ai wgpu"                   # Run AI benchmarks with specific features
    $0 --save-baseline main              # Save results as 'main' baseline
    $0 --baseline main --quick           # Compare against 'main' baseline in quick mode

Environment Variables:
    CARGO_TARGET_DIR         Override target directory
    BENCHMARK_THREADS        Number of threads for benchmarking
    CI                       If set, enables CI-friendly output
EOF
}

log() {
    echo -e "${BLUE}[$(date +'%Y-%m-%d %H:%M:%S')] $*${NC}"
}

log_success() {
    echo -e "${GREEN}[$(date +'%Y-%m-%d %H:%M:%S')] $*${NC}"
}

log_warning() {
    echo -e "${YELLOW}[$(date +'%Y-%m-%d %H:%M:%S')] $*${NC}"
}

log_error() {
    echo -e "${RED}[$(date +'%Y-%m-%d %H:%M:%S')] $*${NC}" >&2
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -f|--features)
            FEATURES="$2"
            shift 2
            ;;
        -p|--profile)
            PROFILE="$2"
            shift 2
            ;;
        -o|--output)
            OUTPUT_FORMAT="$2"
            shift 2
            ;;
        -b|--baseline)
            COMPARE_BASELINE="$2"
            shift 2
            ;;
        -s|--save-baseline)
            SAVE_BASELINE="$2"
            shift 2
            ;;
        -q|--quick)
            QUICK_MODE=true
            shift
            ;;
        -v|--verbose)
            VERBOSE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        -*)
            log_error "Unknown option: $1"
            exit 1
            ;;
        *)
            BENCHMARK_SUITE="$1"
            shift
            ;;
    esac
done

# Set default benchmark suite if not specified
BENCHMARK_SUITE="${BENCHMARK_SUITE:-all}"

# Check dependencies
check_dependencies() {
    log "Checking dependencies..."
    
    if ! command -v cargo >/dev/null 2>&1; then
        log_error "cargo is required but not installed"
        exit 1
    fi
    
    if ! command -v gnuplot >/dev/null 2>&1 && [[ "$OUTPUT_FORMAT" == "html" ]]; then
        log_warning "gnuplot not found - HTML plots will be unavailable"
    fi
    
    log_success "Dependencies check completed"
}

# Setup benchmark environment
setup_environment() {
    log "Setting up benchmark environment..."
    
    # Create results directory
    mkdir -p "$RESULTS_DIR"
    
    # Set environment variables for consistent benchmarking
    export CARGO_PROFILE_BENCH_DEBUG=false
    export CARGO_PROFILE_BENCH_LTO=fat
    export CARGO_PROFILE_BENCH_CODEGEN_UNITS=1
    
    # Use number of CPU cores for parallel benchmarking if not set
    if [[ -z "${BENCHMARK_THREADS:-}" ]]; then
        if command -v nproc >/dev/null 2>&1; then
            export BENCHMARK_THREADS=$(nproc)
        elif [[ -f /proc/cpuinfo ]]; then
            export BENCHMARK_THREADS=$(grep -c processor /proc/cpuinfo)
        else
            export BENCHMARK_THREADS=4
        fi
    fi
    
    log "Using $BENCHMARK_THREADS threads for benchmarking"
    
    # Quick mode adjustments
    if [[ "$QUICK_MODE" == "true" ]]; then
        export CRITERION_SAMPLE_SIZE=10
        export CRITERION_MEASUREMENT_TIME=5
        log_warning "Quick mode enabled - reduced sample size and measurement time"
    fi
    
    log_success "Environment setup completed"
}

# Build benchmarks
build_benchmarks() {
    log "Building benchmarks with features: $FEATURES"
    
    cd "$PROJECT_ROOT"
    
    local build_args=(
        "build"
        "--profile" "$PROFILE"
        "--benches"
        "--package" "openagent-terminal"
    )
    
    if [[ -n "$FEATURES" ]]; then
        build_args+=("--features" "$FEATURES")
    fi
    
    if [[ "$VERBOSE" == "true" ]]; then
        build_args+=("--verbose")
    fi
    
    log "Running: cargo ${build_args[*]}"
    
    if ! cargo "${build_args[@]}"; then
        log_error "Failed to build benchmarks"
        exit 1
    fi
    
    log_success "Benchmark build completed"
}

# Run specific benchmark suite
run_benchmark() {
    local bench_name="$1"
    local bench_binary="$2"
    
    log "Running $bench_name benchmarks..."
    
    local bench_args=(
        "bench"
        "--profile" "$PROFILE"
        "--package" "openagent-terminal"
        "--bin" "$bench_binary"
    )
    
    if [[ -n "$FEATURES" ]]; then
        bench_args+=("--features" "$FEATURES")
    fi
    
    # Add baseline comparison if specified
    if [[ -n "$COMPARE_BASELINE" ]]; then
        bench_args+=("--" "--baseline" "$COMPARE_BASELINE")
        log "Comparing against baseline: $COMPARE_BASELINE"
    fi
    
    # Save baseline if specified
    if [[ -n "$SAVE_BASELINE" ]]; then
        bench_args+=("--" "--save-baseline" "$SAVE_BASELINE")
        log "Saving baseline as: $SAVE_BASELINE"
    fi
    
    if [[ "$VERBOSE" == "true" ]]; then
        bench_args+=("--verbose")
    fi
    
    log "Running: cargo ${bench_args[*]}"
    
    if ! cargo "${bench_args[@]}"; then
        log_error "Failed to run $bench_name benchmarks"
        return 1
    fi
    
    log_success "$bench_name benchmarks completed"
}

# Generate reports
generate_reports() {
    log "Generating benchmark reports..."
    
    local report_dir="$RESULTS_DIR/$(date +'%Y%m%d_%H%M%S')"
    mkdir -p "$report_dir"
    
    # Copy Criterion reports
    if [[ -d "$BENCHMARK_OUTPUT_DIR" ]]; then
        cp -r "$BENCHMARK_OUTPUT_DIR"/* "$report_dir/" || true
        log "Criterion reports copied to $report_dir"
    fi
    
    # Generate summary report
    local summary_file="$report_dir/summary.md"
    generate_summary_report "$summary_file"
    
    # Generate JSON report for CI
    if [[ -n "${CI:-}" ]]; then
        generate_ci_report "$report_dir/ci-report.json"
    fi
    
    log_success "Reports generated in $report_dir"
}

# Generate summary report
generate_summary_report() {
    local output_file="$1"
    
    cat > "$output_file" << EOF
# OpenAgent Terminal Benchmark Report

Generated: $(date +'%Y-%m-%d %H:%M:%S')
Features: $FEATURES
Profile: $PROFILE
Benchmark Suite: $BENCHMARK_SUITE

## System Information

- OS: $(uname -s) $(uname -r)
- Architecture: $(uname -m)
- CPU Cores: $BENCHMARK_THREADS
- Rust Version: $(rustc --version)
- Cargo Version: $(cargo --version)

## Benchmark Configuration

- Quick Mode: $QUICK_MODE
- Output Format: $OUTPUT_FORMAT
- Baseline Comparison: ${COMPARE_BASELINE:-none}
- Baseline Save: ${SAVE_BASELINE:-none}

## Results

Detailed results are available in the Criterion HTML reports.

EOF

    if [[ -f "$BENCHMARK_OUTPUT_DIR/report/index.html" ]]; then
        echo "- [HTML Report]($BENCHMARK_OUTPUT_DIR/report/index.html)" >> "$output_file"
    fi
    
    log "Summary report generated: $output_file"
}

# Generate CI-compatible report
generate_ci_report() {
    local output_file="$1"
    
    cat > "$output_file" << EOF
{
  "timestamp": "$(date -Iseconds)",
  "suite": "$BENCHMARK_SUITE",
  "features": "$FEATURES",
  "profile": "$PROFILE",
  "quick_mode": $QUICK_MODE,
  "system": {
    "os": "$(uname -s)",
    "architecture": "$(uname -m)",
    "cpu_cores": $BENCHMARK_THREADS,
    "rust_version": "$(rustc --version)",
    "cargo_version": "$(cargo --version)"
  },
  "results_path": "$BENCHMARK_OUTPUT_DIR"
}
EOF

    log "CI report generated: $output_file"
}

# Main execution
main() {
    log "Starting OpenAgent Terminal benchmark run"
    log "Suite: $BENCHMARK_SUITE, Features: $FEATURES, Profile: $PROFILE"
    
    check_dependencies
    setup_environment
    build_benchmarks
    
    case "$BENCHMARK_SUITE" in
        "core"|"terminal")
            run_benchmark "Core Terminal" "core_performance"
            ;;
        "ai"|"agents")
            if [[ "$FEATURES" == *"ai"* ]]; then
                run_benchmark "AI Agents" "ai_agents"
            else
                log_error "AI benchmarks require 'ai' feature. Add --features ai"
                exit 1
            fi
            ;;
        "startup")
            run_benchmark "Terminal Startup" "terminal_startup"
            ;;
        "render"|"rendering")
            if [[ "$FEATURES" == *"wgpu"* ]]; then
                run_benchmark "Rendering Performance" "render_performance"
            else
                log_error "Rendering benchmarks require 'wgpu' feature. Add --features wgpu"
                exit 1
            fi
            ;;
        "all")
            log "Running all available benchmark suites..."
            run_benchmark "Core Terminal" "core_performance"
            
            if [[ "$FEATURES" == *"ai"* ]]; then
                run_benchmark "AI Agents" "ai_agents"
            else
                log_warning "Skipping AI benchmarks - 'ai' feature not enabled"
            fi
            
            if [[ "$FEATURES" == *"wgpu"* ]]; then
                run_benchmark "Rendering Performance" "render_performance"
            else
                log_warning "Skipping rendering benchmarks - 'wgpu' feature not enabled"
            fi
            
            run_benchmark "Terminal Startup" "terminal_startup"
            ;;
        *)
            log_error "Unknown benchmark suite: $BENCHMARK_SUITE"
            log "Available suites: all, core, ai, startup, render"
            exit 1
            ;;
    esac
    
    generate_reports
    
    log_success "Benchmark run completed successfully!"
    log "Results available in: $RESULTS_DIR"
    
    if [[ -f "$BENCHMARK_OUTPUT_DIR/report/index.html" ]]; then
        log "HTML report: $BENCHMARK_OUTPUT_DIR/report/index.html"
    fi
}

# Execute main function
main "$@"