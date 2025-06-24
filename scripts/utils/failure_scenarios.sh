#!/bin/bash

echo "=== Orderbook Service Failure Scenario Testing ==="
echo "This script will test various failure scenarios"
echo

# Function to check if service is running
check_service() {
    if pgrep -f "orderbook-service.*realtime" > /dev/null; then
        echo "✓ Service is running"
        return 0
    else
        echo "✗ Service is NOT running"
        return 1
    fi
}

# Function to get service PID
get_service_pid() {
    pgrep -f "orderbook-service.*realtime" | head -1
}

# Test 1: Kill data source
test_data_source_failure() {
    echo -e "\n--- Test 1: Data Source Failure ---"
    echo "Killing docker tail process..."
    
    # Find and kill docker tail
    TAIL_PID=$(pgrep -f "docker.*tail.*order_statuses" | head -1)
    if [ -n "$TAIL_PID" ]; then
        kill -9 $TAIL_PID
        echo "Killed data source process $TAIL_PID"
        
        sleep 5
        check_service
        
        # Test if service still responds
        echo "Testing service response..."
        timeout 5 grpcurl -plaintext localhost:50052 orderbook.OrderbookService/GetMarkets
    else
        echo "No data source process found"
    fi
}

# Test 2: Memory pressure
test_memory_pressure() {
    echo -e "\n--- Test 2: Memory Pressure ---"
    echo "Creating memory pressure..."
    
    # Allocate 1GB of memory
    stress-ng --vm 1 --vm-bytes 1G --timeout 30s &
    STRESS_PID=$!
    
    sleep 10
    check_service
    
    # Check memory usage
    PID=$(get_service_pid)
    if [ -n "$PID" ]; then
        MEM_MB=$(ps -o rss= -p $PID | awk '{print $1/1024}')
        echo "Service memory usage: ${MEM_MB}MB"
    fi
    
    kill $STRESS_PID 2>/dev/null
    wait $STRESS_PID 2>/dev/null
}

# Test 3: CPU stress
test_cpu_stress() {
    echo -e "\n--- Test 3: CPU Stress ---"
    echo "Creating CPU stress..."
    
    # Use all CPU cores
    stress-ng --cpu $(nproc) --timeout 30s &
    STRESS_PID=$!
    
    sleep 10
    check_service
    
    # Check CPU usage
    PID=$(get_service_pid)
    if [ -n "$PID" ]; then
        CPU=$(ps -o %cpu= -p $PID)
        echo "Service CPU usage: ${CPU}%"
    fi
    
    kill $STRESS_PID 2>/dev/null
    wait $STRESS_PID 2>/dev/null
}

# Test 4: Network disruption
test_network_disruption() {
    echo -e "\n--- Test 4: Network Disruption ---"
    echo "Simulating network issues..."
    
    # Add packet loss (requires sudo)
    if command -v tc &> /dev/null; then
        echo "Adding 10% packet loss on loopback..."
        sudo tc qdisc add dev lo root netem loss 10% 2>/dev/null || echo "Failed to add packet loss (need sudo)"
        
        sleep 10
        check_service
        
        # Remove packet loss
        sudo tc qdisc del dev lo root 2>/dev/null
    else
        echo "tc command not found, skipping network test"
    fi
}

# Test 5: File descriptor exhaustion
test_fd_exhaustion() {
    echo -e "\n--- Test 5: File Descriptor Exhaustion ---"
    echo "Opening many files..."
    
    # Create a program that opens many files
    cat > /tmp/fd_exhaust.py << 'EOF'
import time
files = []
try:
    for i in range(1000):
        f = open(f'/tmp/test_fd_{i}', 'w')
        files.append(f)
    print(f"Opened {len(files)} files")
    time.sleep(30)
except Exception as e:
    print(f"Failed after {len(files)} files: {e}")
finally:
    for f in files:
        f.close()
    import os
    for i in range(1000):
        try:
            os.remove(f'/tmp/test_fd_{i}')
        except:
            pass
EOF
    
    python3 /tmp/fd_exhaust.py &
    FD_PID=$!
    
    sleep 5
    check_service
    
    # Check FD usage
    PID=$(get_service_pid)
    if [ -n "$PID" ]; then
        FD_COUNT=$(ls /proc/$PID/fd 2>/dev/null | wc -l)
        echo "Service file descriptors: $FD_COUNT"
    fi
    
    kill $FD_PID 2>/dev/null
    wait $FD_PID 2>/dev/null
    rm -f /tmp/fd_exhaust.py
}

# Test 6: Disk I/O stress
test_disk_io_stress() {
    echo -e "\n--- Test 6: Disk I/O Stress ---"
    echo "Creating disk I/O stress..."
    
    # Write large file
    dd if=/dev/zero of=/tmp/stress_test bs=1M count=1000 oflag=direct &
    DD_PID=$!
    
    sleep 10
    check_service
    
    # Check I/O wait
    IOWAIT=$(iostat -c 1 2 | tail -1 | awk '{print $4}')
    echo "System I/O wait: ${IOWAIT}%"
    
    kill $DD_PID 2>/dev/null
    wait $DD_PID 2>/dev/null
    rm -f /tmp/stress_test
}

# Test 7: Signal handling
test_signal_handling() {
    echo -e "\n--- Test 7: Signal Handling ---"
    
    PID=$(get_service_pid)
    if [ -n "$PID" ]; then
        echo "Sending SIGHUP to service..."
        kill -HUP $PID
        sleep 2
        check_service
        
        echo "Sending SIGUSR1 to service..."
        kill -USR1 $PID
        sleep 2
        check_service
    else
        echo "Service not running, skipping signal test"
    fi
}

# Main execution
main() {
    echo "Starting failure scenario tests..."
    echo "Make sure the orderbook service is running before starting"
    echo
    
    check_service || exit 1
    
    # Install stress-ng if not available
    if ! command -v stress-ng &> /dev/null; then
        echo "Installing stress-ng..."
        sudo apt-get update && sudo apt-get install -y stress-ng
    fi
    
    # Run tests
    test_data_source_failure
    test_memory_pressure
    test_cpu_stress
    test_network_disruption
    test_fd_exhaustion
    test_disk_io_stress
    test_signal_handling
    
    echo -e "\n=== Test Summary ==="
    check_service
    
    # Final resource check
    PID=$(get_service_pid)
    if [ -n "$PID" ]; then
        echo -e "\nFinal service stats:"
        ps -o pid,vsz,rss,pcpu,pmem,nlwp,cmd -p $PID
    fi
}

main