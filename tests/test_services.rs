mod common;

use cloudcontrol::services::file_service::FileService;
use cloudcontrol::services::phone_service::PhoneService;
use common::{create_temp_db, make_device_json};
use serde_json::json;

#[tokio::test]
async fn test_phone_service_update_field() {
    let (_tmp, db) = create_temp_db().await;
    let svc = PhoneService::new(db);

    let data = make_device_json("ps-dev-1", true, false);
    svc.update_field("ps-dev-1", &data).await.unwrap();

    let result = svc.query_info_by_udid("ps-dev-1").await.unwrap();
    assert!(result.is_some());
    let device = result.unwrap();
    assert_eq!(device["udid"], "ps-dev-1");
    assert_eq!(device["model"], "TestPhone");
    assert_eq!(device["present"], true);
}

#[tokio::test]
async fn test_phone_service_online_filter() {
    let (_tmp, db) = create_temp_db().await;
    let svc = PhoneService::new(db);

    let data1 = make_device_json("dev-online", true, false);
    let data2 = make_device_json("dev-offline", false, false);
    svc.update_field("dev-online", &data1).await.unwrap();
    svc.update_field("dev-offline", &data2).await.unwrap();

    let list = svc.query_device_list_by_present().await.unwrap();
    assert_eq!(list.len(), 1);
    let udids: Vec<&str> = list
        .iter()
        .map(|d| d["udid"].as_str().unwrap())
        .collect();
    assert!(udids.contains(&"dev-online"));
    assert!(!udids.contains(&"dev-offline"));
}

#[tokio::test]
async fn test_phone_service_delete_all() {
    let (_tmp, db) = create_temp_db().await;
    let svc = PhoneService::new(db);

    svc.update_field("del-dev-1", &make_device_json("del-dev-1", true, false))
        .await
        .unwrap();
    svc.update_field("del-dev-2", &make_device_json("del-dev-2", false, false))
        .await
        .unwrap();

    svc.delete_devices().await.unwrap();

    let list = svc.query_device_list().await.unwrap();
    assert_eq!(list.len(), 0);
}

// ── Device State Persistence Tests (Story 1B-3) ──

#[tokio::test]
async fn test_restore_devices_loads_persisted_devices() {
    let (_tmp, db) = create_temp_db().await;
    let svc = PhoneService::new(db);

    // Setup: Insert devices that simulate a previous session
    let device1 = json!({
        "udid": "persisted-device-1",
        "ip": "192.168.1.100",
        "port": 9008,
        "model": "Persisted Phone",
        "present": true,
        "ready": true,
    });
    let device2 = json!({
        "udid": "persisted-device-2",
        "ip": "192.168.1.101",
        "port": 9008,
        "model": "Another Phone",
        "present": true,
        "ready": false,
    });
    svc.update_field("persisted-device-1", &device1).await.unwrap();
    svc.update_field("persisted-device-2", &device2).await.unwrap();

    // Verify devices exist
    let list = svc.query_device_list_by_present().await.unwrap();
    assert_eq!(list.len(), 2);

    // Simulate restart: restore_devices should mark all as offline initially
    svc.restore_devices().await.unwrap();

    // All devices should still exist but marked as offline (present=false)
    let device1_info = svc.query_info_by_udid("persisted-device-1").await.unwrap().unwrap();
    let device2_info = svc.query_info_by_udid("persisted-device-2").await.unwrap().unwrap();

    // Devices exist but marked offline (will be updated by discovery)
    assert_eq!(device1_info["present"], false);
    assert_eq!(device2_info["present"], false);
    // Metadata preserved
    assert_eq!(device1_info["model"], "Persisted Phone");
    assert_eq!(device2_info["ip"], "192.168.1.101");
}

#[tokio::test]
async fn test_restore_devices_empty_db() {
    let (_tmp, db) = create_temp_db().await;
    let svc = PhoneService::new(db);

    // Empty database - restore should succeed without error
    svc.restore_devices().await.unwrap();

    // Still empty
    let list = svc.query_device_list_by_present().await.unwrap();
    assert_eq!(list.len(), 0);
}

#[tokio::test]
async fn test_persistence_survives_simulated_restart() {
    let (_tmp, db) = create_temp_db().await;
    let svc = PhoneService::new(db);

    // Setup: Add device with metadata
    let device = json!({
        "udid": "survivor-device",
        "ip": "192.168.1.200",
        "port": 7912,
        "model": "Survivor Model",
        "brand": "SurvivorBrand",
        "version": "13",
        "present": true,
    });
    svc.update_field("survivor-device", &device).await.unwrap();

    // Simulate restart: DON'T delete, just restore
    // (This tests new behavior vs delete_devices)
    svc.restore_devices().await.unwrap();

    // Device still exists with all metadata preserved
    let restored = svc.query_info_by_udid("survivor-device").await.unwrap().unwrap();
    assert_eq!(restored["model"], "Survivor Model");
    assert_eq!(restored["brand"], "SurvivorBrand");
    assert_eq!(restored["version"], "13");
    assert_eq!(restored["ip"], "192.168.1.200");
    // Marked offline initially (will be updated by discovery)
    assert_eq!(restored["present"], false);
}

// ── FileService Tests ──

#[tokio::test]
async fn test_file_service_save() {
    let (_tmp, db) = create_temp_db().await;
    let svc = FileService::new(db);

    for i in 0..5 {
        let file_data = json!({
            "group": "0",
            "filename": format!("file_{}.apk", i),
            "filesize": 1000 + i,
            "upload_time": "2024-01-01",
            "who": "user",
        });
        svc.save_install_file(&file_data).await.unwrap();
    }
}

#[tokio::test]
async fn test_file_service_query() {
    let (_tmp, db) = create_temp_db().await;
    let svc = FileService::new(db);

    for i in 0..5 {
        let file_data = json!({
            "group": "0",
            "filename": format!("file_{}.apk", i),
            "filesize": 1000 + i,
            "upload_time": "2024-01-01",
            "who": "user",
        });
        svc.save_install_file(&file_data).await.unwrap();
    }

    let count = svc.query_all_install_file().await.unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn test_file_service_delete() {
    let (_tmp, db) = create_temp_db().await;
    let svc = FileService::new(db);

    let file_data = json!({
        "group": "0",
        "filename": "to_delete.apk",
        "filesize": 512,
        "upload_time": "2024-01-01",
        "who": "admin",
    });
    svc.save_install_file(&file_data).await.unwrap();

    svc.delete_install_file("0", "to_delete.apk").await.unwrap();

    let files = svc.query_install_file("0", 0, 10, "").await.unwrap();
    assert_eq!(files.len(), 0);
}
