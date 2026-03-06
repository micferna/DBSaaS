mod common;

#[tokio::test]
async fn test_port_pool_allocation() {
    let pool = dbsaas_api::utils::port_pool::PortPool::new(10000, 10005);

    let p1 = pool.allocate().unwrap();
    let p2 = pool.allocate().unwrap();
    assert_ne!(p1, p2);
    assert!((10000..=10005).contains(&p1));

    pool.release(p1);
    let p3 = pool.allocate().unwrap();
    assert_eq!(p3, p1); // Should reuse released port
}

#[tokio::test]
async fn test_port_pool_exhaustion() {
    let pool = dbsaas_api::utils::port_pool::PortPool::new(10000, 10001);

    pool.allocate().unwrap();
    pool.allocate().unwrap();
    assert!(pool.allocate().is_err());
}
