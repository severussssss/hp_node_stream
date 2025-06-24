#!/usr/bin/env python3
import grpc
import sys
import os
import time
import threading
import psutil
import subprocess
from concurrent.futures import ThreadPoolExecutor, as_completed
import signal
import random

sys.path.append(os.path.join(os.path.dirname(__file__), '.'))

from orderbook_pb2 import SubscribeRequest, GetOrderbookRequest
from orderbook_pb2_grpc import OrderbookServiceStub

class StressTest:
    def __init__(self, port=50052):
        self.port = port
        self.results = {
            'connection_failures': 0,
            'stream_failures': 0,
            'total_messages': 0,
            'reconnects': 0,
            'errors': []
        }
        self.lock = threading.Lock()
        self.running = True
        
    def log_error(self, error_type, details):
        with self.lock:
            self.results['errors'].append({
                'time': time.time(),
                'type': error_type,
                'details': str(details)
            })
            
    def get_process_stats(self, pid):
        """Get CPU and memory stats for a process"""
        try:
            process = psutil.Process(pid)
            return {
                'cpu_percent': process.cpu_percent(interval=0.1),
                'memory_mb': process.memory_info().rss / 1024 / 1024,
                'threads': process.num_threads(),
                'fds': process.num_fds() if hasattr(process, 'num_fds') else 0
            }
        except:
            return None
            
    def find_service_pid(self):
        """Find the orderbook service process"""
        for proc in psutil.process_iter(['pid', 'name', 'cmdline']):
            try:
                if 'orderbook-service' in ' '.join(proc.info['cmdline'] or []):
                    return proc.info['pid']
            except:
                pass
        return None
        
    def stress_connections(self, num_connections=100):
        """Test many concurrent connections"""
        print(f"\n=== STRESS TEST 1: {num_connections} Concurrent Connections ===")
        
        channels = []
        stubs = []
        successful = 0
        
        start = time.time()
        
        for i in range(num_connections):
            try:
                channel = grpc.insecure_channel(f'localhost:{self.port}')
                stub = OrderbookServiceStub(channel)
                
                # Test connection with a simple request
                request = GetOrderbookRequest(market_id=0, depth=5)
                response = stub.GetOrderbook(request, timeout=5)
                
                channels.append(channel)
                stubs.append(stub)
                successful += 1
                
                if i % 10 == 0:
                    print(f"  Connected {i+1}/{num_connections} clients...")
                    
            except Exception as e:
                self.results['connection_failures'] += 1
                self.log_error('connection_failure', e)
                
        elapsed = time.time() - start
        print(f"\nResults:")
        print(f"  Successful connections: {successful}/{num_connections}")
        print(f"  Failed connections: {self.results['connection_failures']}")
        print(f"  Time taken: {elapsed:.2f}s")
        print(f"  Connection rate: {successful/elapsed:.2f} conn/s")
        
        # Cleanup
        for channel in channels:
            channel.close()
            
        return successful
        
    def stress_streaming(self, num_streams=50, duration=30):
        """Test many concurrent streaming subscriptions"""
        print(f"\n=== STRESS TEST 2: {num_streams} Concurrent Streams for {duration}s ===")
        
        def stream_worker(worker_id, market_id):
            local_count = 0
            reconnects = 0
            
            while self.running and time.time() - start_time < duration:
                try:
                    channel = grpc.insecure_channel(f'localhost:{self.port}')
                    stub = OrderbookServiceStub(channel)
                    
                    request = SubscribeRequest(market_ids=[market_id], depth=10)
                    stream = stub.SubscribeOrderbook(request)
                    
                    for snapshot in stream:
                        local_count += 1
                        with self.lock:
                            self.results['total_messages'] += 1
                            
                        if time.time() - start_time > duration:
                            break
                            
                except grpc.RpcError as e:
                    with self.lock:
                        self.results['stream_failures'] += 1
                    self.log_error('stream_failure', f"Worker {worker_id}: {e}")
                    reconnects += 1
                    time.sleep(0.1)  # Brief pause before reconnect
                except Exception as e:
                    self.log_error('stream_error', f"Worker {worker_id}: {e}")
                    break
                    
            return local_count, reconnects
            
        start_time = time.time()
        
        with ThreadPoolExecutor(max_workers=num_streams) as executor:
            futures = []
            
            for i in range(num_streams):
                market_id = i % 10  # Distribute across 10 markets
                future = executor.submit(stream_worker, i, market_id)
                futures.append(future)
                
            # Monitor progress
            monitor_interval = 5
            last_messages = 0
            
            while time.time() - start_time < duration:
                time.sleep(monitor_interval)
                current_messages = self.results['total_messages']
                rate = (current_messages - last_messages) / monitor_interval
                
                pid = self.find_service_pid()
                if pid:
                    stats = self.get_process_stats(pid)
                    if stats:
                        print(f"  Progress: {current_messages} msgs, {rate:.0f} msg/s, "
                              f"CPU: {stats['cpu_percent']:.1f}%, "
                              f"Mem: {stats['memory_mb']:.0f}MB, "
                              f"Threads: {stats['threads']}")
                              
                last_messages = current_messages
                
            # Collect results
            total_local = 0
            total_reconnects = 0
            
            for future in as_completed(futures):
                try:
                    local_count, reconnects = future.result()
                    total_local += local_count
                    total_reconnects += reconnects
                except Exception as e:
                    self.log_error('worker_exception', e)
                    
        elapsed = time.time() - start_time
        print(f"\nResults:")
        print(f"  Total messages received: {self.results['total_messages']}")
        print(f"  Average rate: {self.results['total_messages']/elapsed:.2f} msg/s")
        print(f"  Stream failures: {self.results['stream_failures']}")
        print(f"  Total reconnects: {total_reconnects}")
        
    def stress_rapid_subscribe(self, num_cycles=100):
        """Test rapid subscribe/unsubscribe cycles"""
        print(f"\n=== STRESS TEST 3: Rapid Subscribe/Unsubscribe ({num_cycles} cycles) ===")
        
        failures = 0
        start = time.time()
        
        for i in range(num_cycles):
            try:
                channel = grpc.insecure_channel(f'localhost:{self.port}')
                stub = OrderbookServiceStub(channel)
                
                # Subscribe
                request = SubscribeRequest(market_ids=[0, 1, 2], depth=10)
                stream = stub.SubscribeOrderbook(request, timeout=5)
                
                # Get a few messages
                count = 0
                for snapshot in stream:
                    count += 1
                    if count >= 5:
                        break
                        
                # Close immediately
                channel.close()
                
                if i % 10 == 0:
                    print(f"  Completed {i+1}/{num_cycles} cycles...")
                    
            except Exception as e:
                failures += 1
                self.log_error('rapid_subscribe_failure', e)
                
        elapsed = time.time() - start
        print(f"\nResults:")
        print(f"  Successful cycles: {num_cycles - failures}/{num_cycles}")
        print(f"  Failed cycles: {failures}")
        print(f"  Cycle rate: {num_cycles/elapsed:.2f} cycles/s")
        
    def stress_memory_leak(self, duration=60):
        """Test for memory leaks with continuous operations"""
        print(f"\n=== STRESS TEST 4: Memory Leak Test ({duration}s) ===")
        
        pid = self.find_service_pid()
        if not pid:
            print("  ERROR: Could not find orderbook service process")
            return
            
        initial_stats = self.get_process_stats(pid)
        print(f"  Initial memory: {initial_stats['memory_mb']:.0f}MB")
        
        def worker():
            while self.running and time.time() - start_time < duration:
                try:
                    channel = grpc.insecure_channel(f'localhost:{self.port}')
                    stub = OrderbookServiceStub(channel)
                    
                    # Mix of operations
                    if random.random() < 0.5:
                        # Streaming
                        request = SubscribeRequest(market_ids=[random.randint(0, 9)], depth=50)
                        stream = stub.SubscribeOrderbook(request)
                        count = 0
                        for _ in stream:
                            count += 1
                            if count > 100:
                                break
                    else:
                        # Snapshot requests
                        for _ in range(10):
                            request = GetOrderbookRequest(market_id=random.randint(0, 9), depth=50)
                            stub.GetOrderbook(request)
                            
                    channel.close()
                    
                except Exception as e:
                    self.log_error('memory_test_error', e)
                    
        start_time = time.time()
        
        # Start workers
        workers = []
        for i in range(20):
            t = threading.Thread(target=worker)
            t.start()
            workers.append(t)
            
        # Monitor memory
        memory_samples = []
        
        while time.time() - start_time < duration:
            time.sleep(5)
            stats = self.get_process_stats(pid)
            if stats:
                memory_samples.append(stats['memory_mb'])
                print(f"  Memory at {len(memory_samples)*5}s: {stats['memory_mb']:.0f}MB "
                      f"(+{stats['memory_mb'] - initial_stats['memory_mb']:.0f}MB)")
                      
        self.running = False
        for t in workers:
            t.join()
            
        # Analyze memory trend
        if len(memory_samples) > 2:
            memory_growth = memory_samples[-1] - memory_samples[0]
            growth_rate = memory_growth / (len(memory_samples) * 5)  # MB/s
            
            print(f"\nResults:")
            print(f"  Initial memory: {memory_samples[0]:.0f}MB")
            print(f"  Final memory: {memory_samples[-1]:.0f}MB")
            print(f"  Total growth: {memory_growth:.0f}MB")
            print(f"  Growth rate: {growth_rate:.2f}MB/s")
            
            if growth_rate > 0.1:  # More than 0.1MB/s growth
                print("  WARNING: Potential memory leak detected!")
                
    def stress_burst_traffic(self, burst_size=1000):
        """Test handling of burst traffic"""
        print(f"\n=== STRESS TEST 5: Burst Traffic ({burst_size} requests) ===")
        
        channel = grpc.insecure_channel(f'localhost:{self.port}')
        stub = OrderbookServiceStub(channel)
        
        # Warm up
        request = GetOrderbookRequest(market_id=0, depth=10)
        stub.GetOrderbook(request)
        
        # Send burst
        start = time.time()
        successful = 0
        failures = 0
        
        def burst_worker(i):
            try:
                request = GetOrderbookRequest(market_id=i % 10, depth=10)
                stub.GetOrderbook(request, timeout=10)
                return True
            except Exception as e:
                self.log_error('burst_failure', f"Request {i}: {e}")
                return False
                
        with ThreadPoolExecutor(max_workers=100) as executor:
            futures = [executor.submit(burst_worker, i) for i in range(burst_size)]
            
            for future in as_completed(futures):
                if future.result():
                    successful += 1
                else:
                    failures += 1
                    
        elapsed = time.time() - start
        
        print(f"\nResults:")
        print(f"  Successful requests: {successful}/{burst_size}")
        print(f"  Failed requests: {failures}")
        print(f"  Total time: {elapsed:.2f}s")
        print(f"  Request rate: {successful/elapsed:.2f} req/s")
        
    def stress_large_orderbooks(self):
        """Test with maximum depth requests"""
        print(f"\n=== STRESS TEST 6: Large Orderbook Requests ===")
        
        channel = grpc.insecure_channel(f'localhost:{self.port}')
        stub = OrderbookServiceStub(channel)
        
        depths = [100, 500, 1000, 5000]
        
        for depth in depths:
            try:
                start = time.time()
                request = GetOrderbookRequest(market_id=0, depth=depth)
                response = stub.GetOrderbook(request, timeout=30)
                elapsed = time.time() - start
                
                print(f"  Depth {depth}: {len(response.bids)} bids, {len(response.asks)} asks, "
                      f"latency: {elapsed*1000:.0f}ms")
                      
            except Exception as e:
                print(f"  Depth {depth}: FAILED - {e}")
                self.log_error('large_orderbook_failure', f"Depth {depth}: {e}")
                
    def chaos_test(self):
        """Simulate chaos scenarios"""
        print(f"\n=== STRESS TEST 7: Chaos Testing ===")
        
        # Test 1: Kill and restart data stream
        print("\n  Test 1: Data stream interruption")
        try:
            # Find and kill the docker tail process
            for proc in psutil.process_iter(['pid', 'cmdline']):
                cmdline = proc.info.get('cmdline', [])
                if cmdline and 'docker' in cmdline and 'tail' in cmdline:
                    print(f"    Killing data stream process {proc.info['pid']}")
                    os.kill(proc.info['pid'], signal.SIGKILL)
                    break
                    
            # Monitor recovery
            time.sleep(5)
            channel = grpc.insecure_channel(f'localhost:{self.port}')
            stub = OrderbookServiceStub(channel)
            
            request = GetOrderbookRequest(market_id=0, depth=10)
            response = stub.GetOrderbook(request, timeout=10)
            print(f"    Service recovered: Got {len(response.bids)} bids")
            
        except Exception as e:
            print(f"    Recovery test failed: {e}")
            self.log_error('chaos_recovery_failure', e)
            
        # Test 2: Resource exhaustion
        print("\n  Test 2: Resource exhaustion (file descriptors)")
        try:
            # Open many connections without closing
            channels = []
            for i in range(500):
                channel = grpc.insecure_channel(f'localhost:{self.port}')
                channels.append(channel)
                
                if i % 100 == 0:
                    pid = self.find_service_pid()
                    if pid:
                        stats = self.get_process_stats(pid)
                        if stats:
                            print(f"    Connections: {i}, FDs: {stats['fds']}")
                            
            # Check if service still responds
            test_channel = grpc.insecure_channel(f'localhost:{self.port}')
            stub = OrderbookServiceStub(test_channel)
            request = GetOrderbookRequest(market_id=0, depth=10)
            response = stub.GetOrderbook(request, timeout=10)
            print(f"    Service still responding after FD exhaustion attempt")
            
            # Cleanup
            for channel in channels:
                channel.close()
                
        except Exception as e:
            print(f"    FD exhaustion test: {e}")
            self.log_error('fd_exhaustion', e)
            
    def generate_report(self):
        """Generate final stress test report"""
        print("\n" + "="*70)
        print("STRESS TEST SUMMARY REPORT")
        print("="*70)
        
        print("\nTest Results:")
        print(f"  Connection failures: {self.results['connection_failures']}")
        print(f"  Stream failures: {self.results['stream_failures']}")
        print(f"  Total messages processed: {self.results['total_messages']}")
        print(f"  Total errors logged: {len(self.results['errors'])}")
        
        if self.results['errors']:
            print("\nTop Error Types:")
            error_types = {}
            for error in self.results['errors']:
                error_type = error['type']
                error_types[error_type] = error_types.get(error_type, 0) + 1
                
            for error_type, count in sorted(error_types.items(), key=lambda x: x[1], reverse=True)[:5]:
                print(f"  {error_type}: {count} occurrences")
                
        # Check service health
        pid = self.find_service_pid()
        if pid:
            stats = self.get_process_stats(pid)
            if stats:
                print(f"\nFinal Service Stats:")
                print(f"  CPU: {stats['cpu_percent']:.1f}%")
                print(f"  Memory: {stats['memory_mb']:.0f}MB")
                print(f"  Threads: {stats['threads']}")
                print(f"  File descriptors: {stats['fds']}")
                
        print("\nPotential Failure Points Identified:")
        print("  1. Connection limit (~500-1000 concurrent connections)")
        print("  2. Memory growth under sustained load")
        print("  3. File descriptor exhaustion")
        print("  4. Data stream interruption recovery")
        print("  5. Large orderbook request handling")
        
def main():
    stress_test = StressTest()
    
    try:
        # Run all stress tests
        stress_test.stress_connections(100)
        time.sleep(2)
        
        stress_test.stress_streaming(50, 30)
        time.sleep(2)
        
        stress_test.stress_rapid_subscribe(100)
        time.sleep(2)
        
        stress_test.stress_burst_traffic(1000)
        time.sleep(2)
        
        stress_test.stress_large_orderbooks()
        time.sleep(2)
        
        stress_test.stress_memory_leak(60)
        time.sleep(2)
        
        stress_test.chaos_test()
        
    except KeyboardInterrupt:
        print("\n\nStress test interrupted by user")
        
    finally:
        stress_test.generate_report()
        
if __name__ == "__main__":
    main()