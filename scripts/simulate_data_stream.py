#!/usr/bin/env python3
import sys
import socket
import struct
import json
import time
import pyarrow as pa
import pyarrow.ipc as ipc
import numpy as np
from datetime import datetime

class NEDTrajectoryStreamer:
    def __init__(self, host='127.0.0.1', port=9999, update_rate_hz=10):
        self.host = host
        self.port = port
        self.update_rate_hz = update_rate_hz
        self.update_interval = 1.0 / update_rate_hz
        
        # Start time in microseconds
        self.start_time_us = int(time.time() * 1_000_000)
        
        # Track last sent timestamp to send only new data
        self.last_sent_time_us = self.start_time_us
        
        # Socket will be created when needed
        self.sock = None
        
        # Parameters and version info
        self.parameters = {
            "trajectory_scale": 50.0,
            "spiral_turns": 3,
            "figure8_size": 40.0,
            "rollercoaster_amplitude": 30.0
        }
        self.version_info = {"sw_version": "v1.0.0-ned-trajectories"}
        
    def get_current_time_us(self):
        """Get current time in microseconds since epoch"""
        return int(time.time() * 1_000_000)
    
    def get_elapsed_time_us(self):
        """Get elapsed time since start in microseconds"""
        return self.get_current_time_us() - self.start_time_us
    
    def generate_spiral_trajectory(self, t):
        """
        Ascending spiral trajectory
        t: time in seconds (continuously extending)
        Returns: (north, east, down) in meters
        """
        t = np.asarray(t)
        # 3 full rotations per 10 seconds
        angle = 2 * np.pi * 0.3 * t
        # Radius oscillates between 15-30m
        radius = 22.5 + 7.5 * np.sin(0.2 * np.pi * t)
        
        north = radius * np.cos(angle)
        east = radius * np.sin(angle)
        down = -5 * t  # Continuously ascending at 5 m/s
        
        return north, east, down
    
    def generate_figure8_trajectory(self, t):
        """
        Figure-8 (lemniscate) trajectory that extends forward over time
        t: time in seconds (continuously extending)
        Returns: (north, east, down) in meters
        """
        t = np.asarray(t)
        # Move forward while tracing figure-8
        forward_speed = 8.0  # m/s
        north = forward_speed * t
        
        # Figure-8 lateral pattern (period = 10s)
        period = 10.0
        phase = 2 * np.pi * (t / period)
        east = 25 * np.sin(phase) * np.cos(phase)
        
        # Gentle altitude variation
        down = -30 + 5 * np.sin(2 * np.pi * t / period)
        
        return north, east, down
    
    def generate_rollercoaster_trajectory(self, t):
        """
        Rollercoaster path that extends forward with time
        t: time in seconds (continuously extending)
        Returns: (north, east, down) in meters
        """
        t = np.asarray(t)
        # Continuous forward motion
        forward_speed = 10.0  # m/s
        north = forward_speed * t
        
        # S-curve lateral movement (wavelength = 50m)
        wavelength = 50.0
        east = 20 * np.sin(2 * np.pi * north / wavelength)
        
        # Rollercoaster altitude profile with multiple hills
        down = -40 + 20 * np.sin(2 * np.pi * north / 60) + 8 * np.sin(2 * np.pi * north / 25)
        
        return north, east, down
    
    def generate_helix_trajectory(self, t):
        """
        Helical trajectory that extends forward over time
        t: time in seconds (continuously extending)
        Returns: (north, east, down) in meters
        """
        t = np.asarray(t)
        # Forward motion
        forward_speed = 6.0  # m/s
        north = forward_speed * t
        
        # Circular lateral pattern (radius = 20m, period = 8s)
        period = 8.0
        angle = 2 * np.pi * (t / period)
        radius = 20.0
        east = radius * np.sin(angle)
        
        # Descending at 3 m/s
        down = -3 * t
        
        return north, east, down
    
    def generate_cloverleaf_trajectory(self, t):
        """
        Cloverleaf pattern that extends forward over time
        t: time in seconds (continuously extending)
        Returns: (north, east, down) in meters
        """
        t = np.asarray(t)
        # Forward motion
        forward_speed = 7.0  # m/s
        north = forward_speed * t
        
        # Rose curve lateral pattern (4 petals per 60m)
        wavelength = 60.0
        phase = 2 * np.pi * north / wavelength
        amplitude = 25.0
        east = amplitude * np.abs(np.sin(2 * phase)) * np.sin(phase)
        
        # Altitude follows the pattern
        down = -35 - 8 * np.sin(4 * phase)
        
        return north, east, down
    
    def generate_wave_trajectory(self, t):
        """
        Wave pattern that extends forward over time
        t: time in seconds (continuously extending)
        Returns: (north, east, down) in meters
        """
        t = np.asarray(t)
        # Continuous forward motion
        forward_speed = 12.0  # m/s
        north = forward_speed * t
        
        # Sinusoidal lateral movement (wavelength = 40m, 3 waves)
        wavelength = 40.0
        east = 18 * np.sin(2 * np.pi * north / wavelength)
        
        # Altitude wave (wavelength = 50m)
        down = -35 + 12 * np.sin(2 * np.pi * north / 50)
        
        return north, east, down
    
    def generate_orbit_trajectory(self, t):
        """
        Circular orbit that drifts forward over time
        t: time in seconds (continuously extending)
        Returns: (north, east, down) in meters
        """
        t = np.asarray(t)
        # Slow forward drift
        forward_speed = 3.0  # m/s
        north_drift = forward_speed * t
        
        # Circular orbit (radius = 35m, period = 12s)
        period = 12.0
        angle = 2 * np.pi * (t / period)
        radius = 35.0
        
        north = north_drift + radius * np.cos(angle)
        east = radius * np.sin(angle)
        down = -45 * np.ones_like(t)  # Constant altitude
        
        return north, east, down
    
    def compute_orientation(self, vn, ve, vd):
        """
        Compute roll, pitch, yaw from velocity vectors.
        Simulates a vehicle oriented in the direction of travel.
        
        Returns: (roll, pitch, yaw) in radians
        """
        # Yaw: direction of horizontal velocity (heading)
        yaw = np.arctan2(ve, vn)  # arctan2(east, north)
        
        # Pitch: angle from horizontal based on vertical velocity
        horizontal_speed = np.sqrt(vn**2 + ve**2)
        pitch = -np.arctan2(vd, horizontal_speed)  # Negative because down is positive in NED
        
        # Roll: bank angle during turns (simplified model)
        # Use rate of change of yaw as a proxy for turn rate
        yaw_rate = np.gradient(yaw)
        # Bank angle proportional to turn rate and speed
        speed = np.sqrt(vn**2 + ve**2 + vd**2)
        # Limit roll to reasonable values (±45 degrees = ±0.785 rad)
        roll = np.clip(yaw_rate * speed * 2.0, -0.785, 0.785)
        
        return roll, pitch, yaw
    
    def generate_realtime_trajectories(self, start_time_us, end_time_us):
        """
        Generate trajectory data between start_time_us and end_time_us.
        """
        duration_us = end_time_us - start_time_us
        
        # Ensure minimum duration to prevent timestamp issues
        if duration_us < 1000:  # Less than 1ms
            duration_us = 1000
            end_time_us = start_time_us + duration_us
        
        # Calculate number of samples for this interval
        # Ensure at least 2 samples for velocity calculation
        num_samples = max(2, int(duration_us / (self.update_interval * 1_000_000)))
        
        # Generate timestamps using arange to ensure proper bounds
        step_us = duration_us / num_samples
        timestamps = np.arange(
            start_time_us,
            end_time_us,
            step_us,
            dtype='float64'
        ).astype('int64')
        
        # Ensure we have exactly the right range
        if len(timestamps) < 2:
            timestamps = np.array([start_time_us, end_time_us], dtype='int64')
        else:
            # Add end timestamp if needed
            if timestamps[-1] < end_time_us:
                timestamps = np.append(timestamps, end_time_us)
        
        # Clip to ensure bounds (paranoid check)
        timestamps = np.clip(timestamps, start_time_us, end_time_us)
        
        # Remove any duplicates that might have occurred
        timestamps = np.unique(timestamps)
        
        # Ensure at least 2 points
        if len(timestamps) < 2:
            timestamps = np.array([start_time_us, end_time_us], dtype='int64')
        
        # Convert to elapsed time in seconds
        elapsed_seconds = (timestamps - self.start_time_us) / 1_000_000.0
        
        # Use absolute time for continuously extending trajectories
        t = elapsed_seconds
        
        # Generate all trajectories
        spiral_n, spiral_e, spiral_d = self.generate_spiral_trajectory(t)
        fig8_n, fig8_e, fig8_d = self.generate_figure8_trajectory(t)
        coaster_n, coaster_e, coaster_d = self.generate_rollercoaster_trajectory(t)
        helix_n, helix_e, helix_d = self.generate_helix_trajectory(t)
        clover_n, clover_e, clover_d = self.generate_cloverleaf_trajectory(t)
        wave_n, wave_e, wave_d = self.generate_wave_trajectory(t)
        orbit_n, orbit_e, orbit_d = self.generate_orbit_trajectory(t)
        
        # Compute velocities (numerical derivative)
        def compute_velocity(pos):
            if len(pos) < 2:
                return np.zeros_like(pos)
            vel = np.gradient(pos, elapsed_seconds)
            return vel
        
        # Compute velocities and orientations for spiral
        spiral_vn = compute_velocity(spiral_n)
        spiral_ve = compute_velocity(spiral_e)
        spiral_vd = compute_velocity(spiral_d)
        spiral_roll, spiral_pitch, spiral_yaw = self.compute_orientation(spiral_vn, spiral_ve, spiral_vd)
        
        fig8_vn = compute_velocity(fig8_n)
        fig8_ve = compute_velocity(fig8_e)
        fig8_vd = compute_velocity(fig8_d)
        fig8_roll, fig8_pitch, fig8_yaw = self.compute_orientation(fig8_vn, fig8_ve, fig8_vd)
        
        coaster_vn = compute_velocity(coaster_n)
        coaster_ve = compute_velocity(coaster_e)
        coaster_vd = compute_velocity(coaster_d)
        coaster_roll, coaster_pitch, coaster_yaw = self.compute_orientation(coaster_vn, coaster_ve, coaster_vd)
        
        helix_vn = compute_velocity(helix_n)
        helix_ve = compute_velocity(helix_e)
        helix_vd = compute_velocity(helix_d)
        helix_roll, helix_pitch, helix_yaw = self.compute_orientation(helix_vn, helix_ve, helix_vd)
        
        clover_vn = compute_velocity(clover_n)
        clover_ve = compute_velocity(clover_e)
        clover_vd = compute_velocity(clover_d)
        clover_roll, clover_pitch, clover_yaw = self.compute_orientation(clover_vn, clover_ve, clover_vd)
        
        wave_vn = compute_velocity(wave_n)
        wave_ve = compute_velocity(wave_e)
        wave_vd = compute_velocity(wave_d)
        wave_roll, wave_pitch, wave_yaw = self.compute_orientation(wave_vn, wave_ve, wave_vd)
        
        orbit_vn = compute_velocity(orbit_n)
        orbit_ve = compute_velocity(orbit_e)
        orbit_vd = compute_velocity(orbit_d)
        orbit_roll, orbit_pitch, orbit_yaw = self.compute_orientation(orbit_vn, orbit_ve, orbit_vd)
        
        # Create Arrow table for each trajectory
        tables = {}
        
        # Spiral trajectory (with velocities)
        tables['spiral'] = pa.Table.from_arrays([
            pa.array(timestamps),
            pa.array(spiral_n),
            pa.array(spiral_e),
            pa.array(spiral_d),
            pa.array(spiral_vn),
            pa.array(spiral_ve),
            pa.array(spiral_vd),
            pa.array(spiral_roll),
            pa.array(spiral_pitch),
            pa.array(spiral_yaw),
        ], names=['timestamp', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'roll', 'pitch', 'yaw'])
        
        # Figure-8 trajectory
        tables['figure8'] = pa.Table.from_arrays([
            pa.array(timestamps),
            pa.array(fig8_n),
            pa.array(fig8_e),
            pa.array(fig8_d),
            pa.array(fig8_vn),
            pa.array(fig8_ve),
            pa.array(fig8_vd),
            pa.array(fig8_roll),
            pa.array(fig8_pitch),
            pa.array(fig8_yaw),
        ], names=['timestamp', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'roll', 'pitch', 'yaw'])
        
        # Rollercoaster trajectory
        tables['rollercoaster'] = pa.Table.from_arrays([
            pa.array(timestamps),
            pa.array(coaster_n),
            pa.array(coaster_e),
            pa.array(coaster_d),
            pa.array(coaster_vn),
            pa.array(coaster_ve),
            pa.array(coaster_vd),
            pa.array(coaster_roll),
            pa.array(coaster_pitch),
            pa.array(coaster_yaw),
        ], names=['timestamp', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'roll', 'pitch', 'yaw'])
        
        # Helix trajectory
        tables['helix'] = pa.Table.from_arrays([
            pa.array(timestamps),
            pa.array(helix_n),
            pa.array(helix_e),
            pa.array(helix_d),
            pa.array(helix_vn),
            pa.array(helix_ve),
            pa.array(helix_vd),
            pa.array(helix_roll),
            pa.array(helix_pitch),
            pa.array(helix_yaw),
        ], names=['timestamp', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'roll', 'pitch', 'yaw'])
        
        # Cloverleaf trajectory
        tables['cloverleaf'] = pa.Table.from_arrays([
            pa.array(timestamps),
            pa.array(clover_n),
            pa.array(clover_e),
            pa.array(clover_d),
            pa.array(clover_vn),
            pa.array(clover_ve),
            pa.array(clover_vd),
            pa.array(clover_roll),
            pa.array(clover_pitch),
            pa.array(clover_yaw),
        ], names=['timestamp', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'roll', 'pitch', 'yaw'])
        
        # Wave trajectory
        tables['wave'] = pa.Table.from_arrays([
            pa.array(timestamps),
            pa.array(wave_n),
            pa.array(wave_e),
            pa.array(wave_d),
            pa.array(wave_vn),
            pa.array(wave_ve),
            pa.array(wave_vd),
            pa.array(wave_roll),
            pa.array(wave_pitch),
            pa.array(wave_yaw),
        ], names=['timestamp', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'roll', 'pitch', 'yaw'])
        
        # Orbit trajectory
        tables['orbit'] = pa.Table.from_arrays([
            pa.array(timestamps),
            pa.array(orbit_n),
            pa.array(orbit_e),
            pa.array(orbit_d),
            pa.array(orbit_vn),
            pa.array(orbit_ve),
            pa.array(orbit_vd),
            pa.array(orbit_roll),
            pa.array(orbit_pitch),
            pa.array(orbit_yaw),
        ], names=['timestamp', 'x', 'y', 'z', 'vx', 'vy', 'vz', 'roll', 'pitch', 'yaw'])
        
        return tables
    
    def create_metadata(self, min_ts, max_ts, table_names):
        """Create metadata with current timeline range"""
        return {
            'parameters': self.parameters,
            'version_info': self.version_info,
            'table_count': len(table_names),
            'table_names': table_names,
            'timeline_range': {
                'min_timestamp': int(min_ts),
                'max_timestamp': int(max_ts)
            },
            'coordinate_system': 'NED',
            'units': {
                'position': 'meters',
                'velocity': 'meters/second',
                'orientation': 'radians',
                'timestamp': 'microseconds'
            }
        }
    
    def send_update(self):
        """Generate and send incremental data update"""
        if not self.sock:
            raise ConnectionError("Not connected")
        
        current_time_us = self.get_current_time_us()
        current_time = datetime.now().strftime('%H:%M:%S.%f')[:-3]
        
        # Skip update if too soon (prevents timestamp issues)
        time_delta_us = current_time_us - self.last_sent_time_us
        if time_delta_us < 1000:  # Less than 1ms
            return
        
        # Generate only new data since last update
        tables = self.generate_realtime_trajectories(self.last_sent_time_us, current_time_us)
        
        # Get timeline range from actual data
        test_table = list(tables.values())[0]
        ts_array = test_table.column('timestamp').to_numpy()
        min_ts = int(np.min(ts_array))
        max_ts = int(np.max(ts_array))
        
        # Sanity check: min should never be before start_time_us
        if min_ts < self.start_time_us:
            print(f"WARNING: min_ts ({min_ts}) < start_time_us ({self.start_time_us}), clamping")
            min_ts = self.start_time_us
        
        # Create metadata
        metadata = self.create_metadata(min_ts, max_ts, list(tables.keys()))
        
        # Send metadata
        metadata_json = json.dumps(metadata).encode('utf-8')
        metadata_len = struct.pack('<I', len(metadata_json))
        self.sock.sendall(metadata_len + metadata_json)
        
        # Send tables
        total_rows = 0
        for table_name, table in tables.items():
            # Send table name
            name_bytes = table_name.encode('utf-8')
            name_len = struct.pack('<I', len(name_bytes))
            self.sock.sendall(name_len + name_bytes)
            
            # Serialize Arrow table
            sink = pa.BufferOutputStream()
            with ipc.new_stream(sink, table.schema) as writer:
                writer.write_table(table)
            arrow_buffer = sink.getvalue()
            
            # Send table size and data
            table_size = struct.pack('<Q', len(arrow_buffer))
            self.sock.sendall(table_size)
            self.sock.sendall(arrow_buffer)
            
            total_rows += table.num_rows
        
        # Update last sent time
        self.last_sent_time_us = current_time_us
        
        elapsed_sec = (current_time_us - self.start_time_us) / 1_000_000.0
        print(f"[{current_time}] Sent {len(tables)} trajectories: {total_rows:,} total rows, "
              f"elapsed: {elapsed_sec:.2f}s, Δt: {time_delta_us/1000:.1f}ms")
    
    def connect(self):
        """Establish connection to receiver with retry logic"""
        max_retries = 5
        retry_delay = 2.0
        
        for attempt in range(max_retries):
            try:
                print(f"Connecting to {self.host}:{self.port}... (attempt {attempt + 1}/{max_retries})")
                self.sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
                self.sock.connect((self.host, self.port))
                print("Connected successfully!")
                if attempt == 0:
                    print(f"Start time: {self.start_time_us} μs")
                    print(f"Update rate: {self.update_rate_hz} Hz")
                    print(f"Trajectories: spiral, figure8, rollercoaster, helix, cloverleaf, wave, orbit")
                    print()
                return True
            except (ConnectionRefusedError, OSError) as e:
                if self.sock:
                    self.sock.close()
                    self.sock = None
                if attempt < max_retries - 1:
                    print(f"Connection failed: {e}. Retrying in {retry_delay}s...")
                    time.sleep(retry_delay)
                else:
                    print(f"Failed to connect after {max_retries} attempts.")
                    return False
        return False
    
    def disconnect(self):
        """Close the socket connection"""
        if self.sock:
            try:
                self.sock.close()
            except:
                pass
            self.sock = None
    
    def run(self):
        """Main streaming loop with auto-reconnect"""
        try:
            if not self.connect():
                print("Could not establish initial connection. Exiting.")
                sys.exit(1)
            
            consecutive_errors = 0
            max_consecutive_errors = 3
            
            while True:
                loop_start = time.time()
                
                try:
                    self.send_update()
                    consecutive_errors = 0
                    
                except (BrokenPipeError, ConnectionResetError, OSError) as e:
                    consecutive_errors += 1
                    print(f"\nConnection error: {e}")
                    print(f"Consecutive errors: {consecutive_errors}/{max_consecutive_errors}")
                    
                    self.disconnect()
                    
                    if consecutive_errors >= max_consecutive_errors:
                        print("Too many consecutive errors. Exiting.")
                        sys.exit(1)
                    
                    print("Attempting to reconnect...")
                    if not self.connect():
                        print("Reconnection failed. Will retry on next cycle...")
                        time.sleep(2.0)
                        continue
                    
                    print("Reconnected! Resuming stream...\n")
                    consecutive_errors = 0
                    continue
                
                # Sleep to maintain update rate
                elapsed = time.time() - loop_start
                sleep_time = max(0, self.update_interval - elapsed)
                time.sleep(sleep_time)
                
        except KeyboardInterrupt:
            print("\n\nStopping streamer...")
        except Exception as e:
            print(f"Unexpected error: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)
        finally:
            self.disconnect()
            print("Connection closed.")

def main():
    host = '127.0.0.1'
    port = 9999
    update_rate = 10  # Hz
    
    if len(sys.argv) > 1:
        try:
            update_rate = float(sys.argv[1])
            if update_rate <= 0:
                raise ValueError
        except ValueError:
            print(f"Error: Invalid update rate '{sys.argv[1]}'. Use a positive number (Hz).")
            sys.exit(1)
    
    streamer = NEDTrajectoryStreamer(host, port, update_rate)
    streamer.run()

if __name__ == "__main__":
    main()
