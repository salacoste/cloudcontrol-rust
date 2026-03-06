mod common;

use common::{create_temp_db, make_device_json};
use serde_json::json;

#[tokio::test]
async fn test_full_device_lifecycle() {
    let (_tmp, db) = create_temp_db().await;

    // 1. Insert device
    let data = make_device_json("lifecycle-dev", true, false);
    db.upsert("lifecycle-dev", &data).await.unwrap();

    // 2. Verify it exists
    let found = db.find_by_udid("lifecycle-dev").await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap()["model"], "TestPhone");

    // 3. Update a field
    db.update("lifecycle-dev", &json!({"present": false}))
        .await
        .unwrap();
    let updated = db.find_by_udid("lifecycle-dev").await.unwrap().unwrap();
    assert_eq!(updated["present"], false);

    // 4. Delete all
    db.delete_all_devices().await.unwrap();
    let gone = db.find_by_udid("lifecycle-dev").await.unwrap();
    assert!(gone.is_none());
}

#[tokio::test]
async fn test_device_field_roundtrip() {
    let (_tmp, db) = create_temp_db().await;

    let data = json!({
        "udid": "roundtrip-dev",
        "serial": "SER123",
        "ip": "10.0.0.5",
        "port": 7912,
        "present": true,
        "ready": false,
        "using": true,
        "is_server": false,
        "is_mock": true,
        "model": "Galaxy S23",
        "brand": "Samsung",
        "version": "13",
        "sdk": 33,
        "agentVersion": "0.10.3",
        "hwaddr": "AA:BB:CC:DD:EE:FF",
        "createdAt": "2024-01-01 00:00:00",
        "updatedAt": "2024-06-15 12:30:00",
        "memory": {"total": 12288, "free": 6144},
        "cpu": {"cores": 8, "freq": 2800},
        "battery": {"level": 92, "charging": true},
        "display": {"width": 1440, "height": 3200},
    });
    db.upsert("roundtrip-dev", &data).await.unwrap();

    let result = db.find_by_udid("roundtrip-dev").await.unwrap().unwrap();

    assert_eq!(result["udid"], "roundtrip-dev");
    assert_eq!(result["serial"], "SER123");
    assert_eq!(result["ip"], "10.0.0.5");
    assert_eq!(result["port"], 7912);
    assert_eq!(result["present"], true);
    assert_eq!(result["ready"], false);
    assert_eq!(result["using"], true);
    assert_eq!(result["is_server"], false);
    assert_eq!(result["is_mock"], true);
    assert_eq!(result["model"], "Galaxy S23");
    assert_eq!(result["brand"], "Samsung");
    assert_eq!(result["version"], "13");
    assert_eq!(result["sdk"], 33);
    assert_eq!(result["agentVersion"], "0.10.3");
    assert_eq!(result["hwaddr"], "AA:BB:CC:DD:EE:FF");
    assert_eq!(result["createdAt"], "2024-01-01 00:00:00");
    assert_eq!(result["updatedAt"], "2024-06-15 12:30:00");
}

#[tokio::test]
async fn test_json_field_persistence() {
    let (_tmp, db) = create_temp_db().await;

    let data = json!({
        "udid": "json-dev",
        "present": true,
        "memory": {"total": 8192, "free": 4096, "buffers": 512},
        "cpu": {"cores": 8, "model": "Snapdragon 888"},
        "battery": {"level": 85, "charging": false, "temperature": 28.5},
        "display": {"width": 1080, "height": 1920, "density": 480},
    });
    db.upsert("json-dev", &data).await.unwrap();

    let result = db.find_by_udid("json-dev").await.unwrap().unwrap();
    assert_eq!(result["memory"]["total"], 8192);
    assert_eq!(result["memory"]["free"], 4096);
    assert_eq!(result["memory"]["buffers"], 512);
    assert_eq!(result["cpu"]["cores"], 8);
    assert_eq!(result["cpu"]["model"], "Snapdragon 888");
    assert_eq!(result["battery"]["level"], 85);
    assert_eq!(result["battery"]["charging"], false);
    assert_eq!(result["display"]["density"], 480);
}

#[tokio::test]
async fn test_bool_field_mapping() {
    let (_tmp, db) = create_temp_db().await;

    // Insert with booleans
    let data = json!({
        "udid": "bool-dev",
        "present": true,
        "ready": false,
        "using": true,
        "is_server": false,
        "is_mock": true,
    });
    db.upsert("bool-dev", &data).await.unwrap();

    let result = db.find_by_udid("bool-dev").await.unwrap().unwrap();
    assert_eq!(result["present"], true);
    assert_eq!(result["ready"], false);
    assert_eq!(result["using"], true);
    assert_eq!(result["is_server"], false);
    assert_eq!(result["is_mock"], true);

    // Toggle all booleans
    let update = json!({
        "present": false,
        "ready": true,
        "using": false,
        "is_server": true,
        "is_mock": false,
    });
    db.update("bool-dev", &update).await.unwrap();

    let result2 = db.find_by_udid("bool-dev").await.unwrap().unwrap();
    assert_eq!(result2["present"], false);
    assert_eq!(result2["ready"], true);
    assert_eq!(result2["using"], false);
    assert_eq!(result2["is_server"], true);
    assert_eq!(result2["is_mock"], false);
}

#[tokio::test]
async fn test_extra_data_persistence() {
    let (_tmp, db) = create_temp_db().await;

    // "custom_field" is not in FIELD_MAPPING, should go into extra_data
    let data = json!({
        "udid": "extra-dev",
        "present": true,
        "custom_field": "custom_value",
        "nested_custom": {"key": "value"},
    });
    db.upsert("extra-dev", &data).await.unwrap();

    let result = db.find_by_udid("extra-dev").await.unwrap().unwrap();
    assert_eq!(result["udid"], "extra-dev");
    // extra_data should be stored and retrievable
    assert!(result.get("extra_data").is_some() || result.get("custom_field").is_some());
}

#[tokio::test]
async fn test_concurrent_upserts() {
    let (_tmp, db) = create_temp_db().await;

    let mut handles = Vec::new();
    for i in 0..10 {
        let db_clone = db.clone();
        let handle = tokio::spawn(async move {
            let data = json!({
                "udid": "concurrent-dev",
                "present": true,
                "model": format!("Model-{}", i),
            });
            db_clone.upsert("concurrent-dev", &data).await
        });
        handles.push(handle);
    }

    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent upsert should not fail");
    }

    // Device should exist with the last update
    let result = db.find_by_udid("concurrent-dev").await.unwrap();
    assert!(result.is_some());
}

#[tokio::test]
async fn test_file_crud_lifecycle() {
    let (_tmp, db) = create_temp_db().await;

    // Save
    db.save_install_file("group1", "app.apk", Some(1024), "2024-01-01", "admin", None)
        .await
        .unwrap();

    // Query
    let files = db.query_install_file("group1", 0, 10).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["filename"], "app.apk");

    // Count
    let count = db.query_all_install_file().await.unwrap();
    assert_eq!(count, 1);

    // Delete
    db.delete_install_file("group1", "app.apk").await.unwrap();
    let files_after = db.query_install_file("group1", 0, 10).await.unwrap();
    assert_eq!(files_after.len(), 0);
}

#[tokio::test]
async fn test_file_pagination() {
    let (_tmp, db) = create_temp_db().await;

    // Insert 12 files
    for i in 0..12 {
        db.save_install_file(
            "0",
            &format!("file_{}.apk", i),
            Some(i * 100),
            "2024-01-01",
            "admin",
            None,
        )
        .await
        .unwrap();
    }

    // Total should be 12
    let total = db.query_all_install_file().await.unwrap();
    assert_eq!(total, 12);

    // Page 1: offset 0, limit 5
    let page1 = db.query_install_file("0", 0, 5).await.unwrap();
    assert_eq!(page1.len(), 5);

    // Page 2: offset 5, limit 5
    let page2 = db.query_install_file("0", 5, 5).await.unwrap();
    assert_eq!(page2.len(), 5);

    // Page 3: offset 10, limit 5 (only 2 remaining)
    let page3 = db.query_install_file("0", 10, 5).await.unwrap();
    assert_eq!(page3.len(), 2);
}

#[tokio::test]
async fn test_corrupted_database_recovery() {
    use cloudcontrol::db::Database;
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let db_dir = tmp.path().to_str().unwrap();
    let db_name = "test_corrupted.db";
    let db_path = tmp.path().join(db_name);

    // Create a valid database first
    {
        let db = Database::new(db_dir, db_name).await.unwrap();

        // Add some data
        let data = json!({"udid": "test-device", "present": true, "model": "Test"});
        db.upsert("test-device", &data).await.unwrap();
    } // Close connection (drop db)

    // Corrupt the database by writing garbage
    let garbage = b"CORRUPTED_DATABASE_DATA_########";
    fs::write(&db_path, garbage).unwrap();

    // Database::new should recover and create a fresh database
    let recovered_db = Database::new(db_dir, db_name).await.unwrap();

    // Fresh database should be empty
    let devices = recovered_db.find_device_list().await.unwrap();
    assert_eq!(devices.len(), 0, "Recovered database should be empty");

    // Should be able to insert new data
    let new_data = json!({"udid": "recovered-device", "present": true});
    recovered_db.upsert("recovered-device", &new_data).await.unwrap();
    let found = recovered_db.find_by_udid("recovered-device").await.unwrap();
    assert!(found.is_some());
}
