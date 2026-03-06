mod common;

#[tokio::test]
async fn test_password_hashing() {
    let hash = dbsaas_api::utils::crypto::hash_password("test_password_123").unwrap();
    assert!(dbsaas_api::utils::crypto::verify_password("test_password_123", &hash).unwrap());
    assert!(!dbsaas_api::utils::crypto::verify_password("wrong_password", &hash).unwrap());
}

#[tokio::test]
async fn test_encryption_roundtrip() {
    let key = "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2";
    let plaintext = "super_secret_password_123";
    let encrypted = dbsaas_api::utils::crypto::encrypt_string(plaintext, key).unwrap();
    let decrypted = dbsaas_api::utils::crypto::decrypt_string(&encrypted, key).unwrap();
    assert_eq!(plaintext, decrypted);
}

#[tokio::test]
async fn test_api_key_generation() {
    let key = dbsaas_api::utils::crypto::generate_api_key();
    assert!(key.starts_with("sbk_"));
    assert_eq!(key.len(), 4 + 48); // "sbk_" + 48 chars
}

#[tokio::test]
async fn test_random_string_generation() {
    let s = dbsaas_api::utils::crypto::generate_random_string(32);
    assert_eq!(s.len(), 32);
    assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
}
