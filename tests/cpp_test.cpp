#include <cstdio>
#include <cstring>
#include <cassert>
#include "../include/bibi_sync.h"

struct ImuData{
    float accel_x;
    float accel_y;
    float accel_z;
};

int main(){
    printf("🔬 BiBi-Sync C++ Test\n\n");
    
    //create registry
    BibiRegistry* registry = bibi_registry_new();
    assert(registry != nullptr);
    printf("✅ Registry created\n");
    
    //create topic
    BibiByteTopic* imu_topic = bibi_registry_get_byte_topic(registry, "/imu", 8);
    assert(imu_topic != nullptr);
    printf("✅ Topic created: /imu\n");
    
    //publish
    ImuData imu = {1.5f, -2.3f, 9.81f};
    uint64_t epoch = bibi_byte_topic_publish(
        imu_topic, 
        (const uint8_t*)&imu, 
        sizeof(ImuData)
    );
    printf("✅ Published IMU data, epoch=%llu\n", epoch);
    
    //receive
    uint8_t buffer[256];
    size_t len;
    int result = bibi_byte_topic_try_receive(imu_topic, buffer, &len, 256);
    assert(result == 1);
    assert(len == sizeof(ImuData));
    
    ImuData* received = (ImuData*)buffer;
    printf("✅ Received: accel=(%.2f, %.2f, %.2f)\n", 
        received->accel_x, received->accel_y, received->accel_z);
    
    //cleanup
    bibi_byte_topic_free(imu_topic);
    bibi_registry_free(registry);
    
    printf("\n🎉 C++ test passed!\n");
    return 0;
}