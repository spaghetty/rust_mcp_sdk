#!/bin/bash

# Test script for MCP Client Connection Debugging
# This script demonstrates various client connection debugging scenarios

set -e

echo "🔍 MCP Client Connection Debugging Test"
echo "======================================="
echo

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Function to check if a command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Check prerequisites
log_info "Checking prerequisites..."
if ! command_exists cargo; then
    log_error "Cargo not found. Please install Rust."
    exit 1
fi

if ! command_exists netstat; then
    log_warning "netstat not found. Some network checks will be skipped."
fi

log_success "Prerequisites check passed"
echo

# Build the debug client
log_info "Building debug client..."
cd examples/debug_client
if cargo build --quiet; then
    log_success "Debug client built successfully"
else
    log_error "Failed to build debug client"
    exit 1
fi
cd ../..
echo

# Test 1: Connection timeout (should pass)
log_info "Test 1: Connection timeout scenario"
if cargo run --manifest-path examples/debug_client/Cargo.toml --quiet -- timeout 2>/dev/null; then
    log_success "Timeout test passed"
else
    log_error "Timeout test failed"
fi
echo

# Test 2: Try to connect to a non-existent server (should fail gracefully)
log_info "Test 2: Connection to non-existent server"
if cargo run --manifest-path examples/debug_client/Cargo.toml --quiet -- tcp-ndjson --address 127.0.0.1:9999 2>/dev/null; then
    log_warning "Unexpected success connecting to non-existent server"
else
    log_success "Correctly failed to connect to non-existent server"
fi
echo

# Test 3: Start a simple server and test connection
log_info "Test 3: Real server connection test"
log_info "Starting simple server in background..."

# Start the unsafe SQL server in the background
cd examples/unsafe_sql_server
cargo build --quiet
./target/debug/unsafe_sql_server &
SERVER_PID=$!
cd ../..

# Give server time to start
sleep 2

# Check if server is running
if kill -0 $SERVER_PID 2>/dev/null; then
    log_success "Server started (PID: $SERVER_PID)"
    
    # Test connection to real server
    log_info "Testing connection to real server..."
    if cargo run --manifest-path examples/debug_client/Cargo.toml --quiet -- tcp-ndjson --address 127.0.0.1:8080 2>/dev/null; then
        log_success "Successfully connected to real server"
    else
        log_error "Failed to connect to real server"
    fi
    
    # Clean up server
    log_info "Stopping server..."
    kill $SERVER_PID 2>/dev/null || true
    wait $SERVER_PID 2>/dev/null || true
    log_success "Server stopped"
else
    log_error "Failed to start server"
fi
echo

# Test 4: Run comprehensive debugging
log_info "Test 4: Comprehensive debugging scenarios"
if cargo run --manifest-path examples/debug_client/Cargo.toml --quiet -- all 2>/dev/null; then
    log_success "All debugging scenarios completed"
else
    log_error "Some debugging scenarios failed"
fi
echo

# Test 5: Log analysis (if log file exists)
if [ -f "logs/server.log.2025-06-16" ]; then
    log_info "Test 5: Log analysis"
    if cargo run --manifest-path examples/debug_client/Cargo.toml --quiet -- analyze-logs --log-file logs/server.log.2025-06-16 2>/dev/null; then
        log_success "Log analysis completed"
    else
        log_error "Log analysis failed"
    fi
else
    log_warning "No log file found for analysis"
fi
echo

# Network diagnostic information
log_info "Network diagnostic information:"
echo
if command_exists netstat; then
    echo "Active connections on common MCP ports:"
    netstat -an 2>/dev/null | grep -E ':(8080|8081|8082)' || echo "No connections found on ports 8080-8082"
else
    log_warning "netstat not available for network diagnostics"
fi
echo

if command_exists lsof; then
    echo "Processes listening on common MCP ports:"
    lsof -i :8080 2>/dev/null || echo "No process listening on port 8080"
    lsof -i :8081 2>/dev/null || echo "No process listening on port 8081"
else
    log_warning "lsof not available for process diagnostics"
fi
echo

# Summary
log_success "MCP Client Connection Debugging Test Completed!"
echo
echo "📋 Available debug commands:"
echo "  cargo run --manifest-path examples/debug_client/Cargo.toml -- all"
echo "  cargo run --manifest-path examples/debug_client/Cargo.toml -- tcp-ndjson --address 127.0.0.1:8080"
echo "  cargo run --manifest-path examples/debug_client/Cargo.toml -- timeout"
echo "  cargo run --manifest-path examples/debug_client/Cargo.toml -- multiple"
echo "  cargo run --manifest-path examples/debug_client/Cargo.toml -- analyze-logs --log-file path/to/log"
echo
echo "📖 For detailed debugging information, see: DEBUG_CLIENT_CONNECTIONS.md"
echo

