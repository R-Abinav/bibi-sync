#!/usr/bin/env python3
"""
Cross-language test: Verify Python bindings work correctly
and can share data with the same semantics as Rust/C++
"""

import bibi_sync
import struct

def test_basic_pubsub():
    print("=== Test 1: Basic Pub/Sub ===")
    registry = bibi_sync.PyBibiRegistry()
    topic = registry.get_byte_topic("/test/basic", 8)
    
    topic.publish(bytes([1, 2, 3, 4, 5]))
    result = topic.try_receive()
    
    assert result is not None, "Should receive data"
    data, epoch = result
    assert list(data) == [1, 2, 3, 4, 5], f"Data mismatch: {list(data)}"
    assert epoch == 1, f"Epoch should be 1, got {epoch}"
    print("✅ Basic pub/sub works")

def test_shared_topic():
    print("\n=== Test 2: Shared Topic ===")
    registry = bibi_sync.PyBibiRegistry()
    
    topic1 = registry.get_byte_topic("/shared", 8)
    topic2 = registry.get_byte_topic("/shared", 8)
    
    topic1.publish(bytes([0xAB, 0xCD]))
    
    result = topic2.try_receive()
    assert result is not None, "topic2 should see topic1's data"
    data, _ = result
    assert list(data) == [0xAB, 0xCD], "Shared topic data mismatch"
    print("✅ Topics with same name share buffer")

def test_imu_struct():
    print("\n=== Test 3: IMU Struct (like C++ would send) ===")
    registry = bibi_sync.PyBibiRegistry()
    topic = registry.get_byte_topic("/imu/data", 16)
    
    #simulate IMU struct: 3 floats (accel_x, accel_y, accel_z)
    accel_x, accel_y, accel_z = 1.5, -2.3, 9.81
    imu_bytes = struct.pack('fff', accel_x, accel_y, accel_z)
    
    epoch = topic.publish(imu_bytes)
    print(f"Published IMU data, epoch={epoch}, size={len(imu_bytes)} bytes")
    
    result = topic.try_receive()
    assert result is not None
    data, _ = result
    
    rx_x, rx_y, rx_z = struct.unpack('fff', bytes(data))
    assert abs(rx_x - accel_x) < 0.001, f"accel_x mismatch: {rx_x}"
    assert abs(rx_y - accel_y) < 0.001, f"accel_y mismatch: {rx_y}"
    assert abs(rx_z - accel_z) < 0.001, f"accel_z mismatch: {rx_z}"
    print(f"✅ Received IMU: ({rx_x}, {rx_y}, {rx_z})")

def test_peek_latest():
    print("\n=== Test 4: Peek Latest (for control loops) ===")
    registry = bibi_sync.PyBibiRegistry()
    topic = registry.get_byte_topic("/sensor", 8)
    
    topic.publish(bytes([1]))
    topic.publish(bytes([2]))
    topic.publish(bytes([3]))
    
    result = topic.peek_latest()
    assert result is not None
    data, epoch = result
    assert list(data) == [3], f"Should peek latest [3], got {list(data)}"
    assert epoch == 3, f"Latest epoch should be 3, got {epoch}"
    
    #peek doesn't consume
    assert topic.len() == 3, "Peek should not consume"
    print("✅ Peek latest works (doesn't consume)")

def test_overflow_freshness_bias():
    print("\n=== Test 5: Overflow (Freshness Bias) ===")
    registry = bibi_sync.PyBibiRegistry()
    topic = registry.get_byte_topic("/overflow", 3)
    
    topic.publish(bytes([1]))
    topic.publish(bytes([2]))
    topic.publish(bytes([3]))
    topic.publish(bytes([4]))
    topic.publish(bytes([5]))
    
    #should get newest data (4, 5 - with 3 potentially there too)
    values = []
    while True:
        result = topic.try_receive()
        if result is None:
            break
        data, _ = result
        values.append(list(data)[0])
    
    print(f"Received after overflow: {values}")
    assert 5 in values, "Should have newest value 5"
    assert 4 in values, "Should have value 4"
    assert 1 not in values, "Old value 1 should be overwritten"
    print("✅ Freshness bias works (old data discarded)")

def test_multi_topic():
    print("\n=== Test 6: Multiple Topics ===")
    registry = bibi_sync.PyBibiRegistry()
    
    imu = registry.get_byte_topic("/imu", 8)
    gps = registry.get_byte_topic("/gps", 8)
    cam = registry.get_byte_topic("/camera", 16)
    
    imu.publish(bytes([1, 1, 1]))
    gps.publish(bytes([2, 2]))
    cam.publish(bytes([3, 3, 3, 3]))
    
    assert imu.len() == 1
    assert gps.len() == 1
    assert cam.len() == 1
    
    imu_data, _ = imu.try_receive()
    gps_data, _ = gps.try_receive()
    cam_data, _ = cam.try_receive()
    
    assert list(imu_data) == [1, 1, 1]
    assert list(gps_data) == [2, 2]
    assert list(cam_data) == [3, 3, 3, 3]
    print("✅ Multiple independent topics work")

def test_empty_topic():
    print("\n=== Test 7: Empty Topic ===")
    registry = bibi_sync.PyBibiRegistry()
    topic = registry.get_byte_topic("/empty", 8)
    
    assert topic.is_empty() == True
    assert topic.len() == 0
    assert topic.try_receive() is None
    assert topic.peek_latest() is None
    print("✅ Empty topic behaves correctly")

if __name__ == "__main__":
    print("🔬 BiBi-Sync Cross-Language Test Suite\n")
    
    test_basic_pubsub()
    test_shared_topic()
    test_imu_struct()
    test_peek_latest()
    test_overflow_freshness_bias()
    test_multi_topic()
    test_empty_topic()
    
    print("\n" + "="*50)
    print("🎉 All cross-language tests passed!")
    print("="*50)