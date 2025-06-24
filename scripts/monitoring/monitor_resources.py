#!/usr/bin/env python3
import psutil
import time
import sys
import matplotlib.pyplot as plt
from datetime import datetime
import json

class ResourceMonitor:
    def __init__(self):
        self.data = {
            'timestamps': [],
            'cpu': [],
            'memory': [],
            'threads': [],
            'fds': [],
            'connections': [],
            'disk_read_mb': [],
            'disk_write_mb': []
        }
        self.start_time = time.time()
        
    def find_service_pid(self):
        """Find the orderbook service process"""
        for proc in psutil.process_iter(['pid', 'name', 'cmdline']):
            try:
                cmdline = ' '.join(proc.info.get('cmdline', []))
                if 'orderbook-service' in cmdline and 'realtime' in cmdline:
                    return proc.info['pid']
            except:
                pass
        return None
        
    def get_network_connections(self, pid):
        """Count network connections for a process"""
        try:
            proc = psutil.Process(pid)
            connections = proc.connections(kind='tcp')
            established = sum(1 for c in connections if c.status == 'ESTABLISHED')
            return len(connections), established
        except:
            return 0, 0
            
    def monitor(self, duration=300, interval=1):
        """Monitor resources for specified duration"""
        print(f"Monitoring orderbook service for {duration} seconds...")
        print("Press Ctrl+C to stop early")
        
        pid = self.find_service_pid()
        if not pid:
            print("ERROR: Could not find orderbook service process")
            return
            
        print(f"Found service PID: {pid}")
        
        try:
            process = psutil.Process(pid)
            
            # Get initial disk I/O counters
            initial_io = process.io_counters()
            
            while time.time() - self.start_time < duration:
                try:
                    # CPU and Memory
                    cpu_percent = process.cpu_percent(interval=0.1)
                    mem_info = process.memory_info()
                    memory_mb = mem_info.rss / 1024 / 1024
                    
                    # Threads and FDs
                    threads = process.num_threads()
                    try:
                        fds = process.num_fds()
                    except:
                        fds = 0
                        
                    # Network connections
                    total_conn, established = self.get_network_connections(pid)
                    
                    # Disk I/O
                    current_io = process.io_counters()
                    disk_read_mb = (current_io.read_bytes - initial_io.read_bytes) / 1024 / 1024
                    disk_write_mb = (current_io.write_bytes - initial_io.write_bytes) / 1024 / 1024
                    
                    # Store data
                    elapsed = time.time() - self.start_time
                    self.data['timestamps'].append(elapsed)
                    self.data['cpu'].append(cpu_percent)
                    self.data['memory'].append(memory_mb)
                    self.data['threads'].append(threads)
                    self.data['fds'].append(fds)
                    self.data['connections'].append(established)
                    self.data['disk_read_mb'].append(disk_read_mb)
                    self.data['disk_write_mb'].append(disk_write_mb)
                    
                    # Print current stats
                    if int(elapsed) % 10 == 0:
                        print(f"\n[{elapsed:.0f}s] CPU: {cpu_percent:.1f}%, "
                              f"Mem: {memory_mb:.0f}MB, "
                              f"Threads: {threads}, "
                              f"FDs: {fds}, "
                              f"Connections: {established}")
                              
                except psutil.NoSuchProcess:
                    print("\nERROR: Process terminated!")
                    break
                except Exception as e:
                    print(f"\nError monitoring: {e}")
                    
                time.sleep(interval)
                
        except KeyboardInterrupt:
            print("\nMonitoring stopped by user")
            
    def save_data(self):
        """Save monitoring data to file"""
        filename = f"resource_monitor_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        with open(filename, 'w') as f:
            json.dump(self.data, f, indent=2)
        print(f"\nData saved to {filename}")
        
    def plot_results(self):
        """Generate plots of resource usage"""
        if not self.data['timestamps']:
            print("No data to plot")
            return
            
        fig, axes = plt.subplots(3, 2, figsize=(12, 10))
        fig.suptitle('Orderbook Service Resource Usage', fontsize=16)
        
        # CPU Usage
        axes[0, 0].plot(self.data['timestamps'], self.data['cpu'])
        axes[0, 0].set_title('CPU Usage (%)')
        axes[0, 0].set_xlabel('Time (s)')
        axes[0, 0].grid(True)
        
        # Memory Usage
        axes[0, 1].plot(self.data['timestamps'], self.data['memory'])
        axes[0, 1].set_title('Memory Usage (MB)')
        axes[0, 1].set_xlabel('Time (s)')
        axes[0, 1].grid(True)
        
        # Threads
        axes[1, 0].plot(self.data['timestamps'], self.data['threads'])
        axes[1, 0].set_title('Thread Count')
        axes[1, 0].set_xlabel('Time (s)')
        axes[1, 0].grid(True)
        
        # File Descriptors
        axes[1, 1].plot(self.data['timestamps'], self.data['fds'])
        axes[1, 1].set_title('File Descriptors')
        axes[1, 1].set_xlabel('Time (s)')
        axes[1, 1].grid(True)
        
        # Network Connections
        axes[2, 0].plot(self.data['timestamps'], self.data['connections'])
        axes[2, 0].set_title('Established Connections')
        axes[2, 0].set_xlabel('Time (s)')
        axes[2, 0].grid(True)
        
        # Disk I/O
        axes[2, 1].plot(self.data['timestamps'], self.data['disk_read_mb'], label='Read')
        axes[2, 1].plot(self.data['timestamps'], self.data['disk_write_mb'], label='Write')
        axes[2, 1].set_title('Cumulative Disk I/O (MB)')
        axes[2, 1].set_xlabel('Time (s)')
        axes[2, 1].legend()
        axes[2, 1].grid(True)
        
        plt.tight_layout()
        
        filename = f"resource_plot_{datetime.now().strftime('%Y%m%d_%H%M%S')}.png"
        plt.savefig(filename)
        print(f"\nPlot saved to {filename}")
        
        # Also generate summary statistics
        print("\nResource Usage Summary:")
        print(f"  CPU - Avg: {sum(self.data['cpu'])/len(self.data['cpu']):.1f}%, "
              f"Max: {max(self.data['cpu']):.1f}%")
        print(f"  Memory - Avg: {sum(self.data['memory'])/len(self.data['memory']):.0f}MB, "
              f"Max: {max(self.data['memory']):.0f}MB")
        print(f"  Threads - Max: {max(self.data['threads'])}")
        print(f"  File Descriptors - Max: {max(self.data['fds'])}")
        print(f"  Connections - Max: {max(self.data['connections'])}")
        
        # Check for memory leak
        if len(self.data['memory']) > 10:
            early_avg = sum(self.data['memory'][:10]) / 10
            late_avg = sum(self.data['memory'][-10:]) / 10
            growth = late_avg - early_avg
            growth_rate = growth / (self.data['timestamps'][-1] / 60)  # MB per minute
            
            print(f"\nMemory Analysis:")
            print(f"  Early average: {early_avg:.0f}MB")
            print(f"  Late average: {late_avg:.0f}MB")
            print(f"  Growth: {growth:.0f}MB ({growth_rate:.2f}MB/min)")
            
            if growth_rate > 1:
                print("  WARNING: Significant memory growth detected!")
                
def main():
    monitor = ResourceMonitor()
    
    # Check if running alongside stress test
    duration = 300  # 5 minutes default
    if len(sys.argv) > 1:
        duration = int(sys.argv[1])
        
    try:
        monitor.monitor(duration=duration)
    finally:
        monitor.save_data()
        try:
            monitor.plot_results()
        except Exception as e:
            print(f"Could not generate plots: {e}")
            
if __name__ == "__main__":
    main()