mod common;

use dbsaas_api::services::tls::TlsService;
use std::path::Path;

#[tokio::test]
async fn test_tls_ca_generation() {
    let tmp_dir = std::env::temp_dir().join("dbsaas_test_tls");
    let _ = std::fs::remove_dir_all(&tmp_dir);

    let tls = TlsService::new(tmp_dir.to_str().unwrap().to_string());
    tls.init_ca().unwrap();

    assert!(Path::new(&tmp_dir).join("ca.crt").exists());
    assert!(Path::new(&tmp_dir).join("ca.key").exists());

    // Generate server cert
    let cert = tls.generate_server_cert("localhost", 10000).unwrap();
    assert!(cert.cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(cert.key_pem.contains("BEGIN PRIVATE KEY"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmp_dir);
}
