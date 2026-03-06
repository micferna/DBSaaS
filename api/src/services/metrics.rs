use lazy_static::lazy_static;
use prometheus::{
    Encoder, GaugeVec, IntCounterVec, IntGauge, HistogramVec, Registry, TextEncoder,
    opts, histogram_opts,
};
use sqlx::PgPool;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();

    // Platform gauges
    pub static ref TOTAL_USERS: IntGauge =
        IntGauge::new("dbsaas_total_users", "Total number of users").unwrap();
    pub static ref TOTAL_DATABASES: IntGauge =
        IntGauge::new("dbsaas_total_databases", "Total number of databases").unwrap();
    pub static ref DATABASES_BY_STATUS: GaugeVec =
        GaugeVec::new(opts!("dbsaas_databases_by_status", "Databases by status"), &["status"]).unwrap();
    pub static ref DATABASES_BY_TYPE: GaugeVec =
        GaugeVec::new(opts!("dbsaas_databases_by_type", "Databases by type"), &["db_type"]).unwrap();

    // Server gauges
    pub static ref SERVER_CONTAINERS: GaugeVec =
        GaugeVec::new(opts!("dbsaas_server_containers", "Containers per server"), &["server_id", "name"]).unwrap();
    pub static ref SERVER_CPU_PERCENT: GaugeVec =
        GaugeVec::new(opts!("dbsaas_server_cpu_percent", "Server CPU usage %"), &["server_id", "name"]).unwrap();
    pub static ref SERVER_MEMORY_PERCENT: GaugeVec =
        GaugeVec::new(opts!("dbsaas_server_memory_percent", "Server memory usage %"), &["server_id", "name"]).unwrap();

    // Revenue
    pub static ref MONTHLY_REVENUE_CENTS: IntGauge =
        IntGauge::new("dbsaas_monthly_revenue_cents", "Revenue for current month (cents)").unwrap();
    pub static ref PENDING_REVENUE_CENTS: IntGauge =
        IntGauge::new("dbsaas_pending_revenue_cents", "Pending unbilled revenue (cents)").unwrap();

    // HTTP metrics
    pub static ref HTTP_REQUEST_COUNT: IntCounterVec =
        IntCounterVec::new(opts!("dbsaas_http_requests_total", "Total HTTP requests"), &["method", "path", "status"]).unwrap();
    pub static ref HTTP_REQUEST_DURATION: HistogramVec =
        HistogramVec::new(
            histogram_opts!("dbsaas_http_request_duration_seconds", "HTTP request duration"),
            &["method", "path"]
        ).unwrap();

    // Container stats
    pub static ref CONTAINER_CPU_PERCENT: GaugeVec =
        GaugeVec::new(opts!("dbsaas_container_cpu_percent", "Container CPU usage %"), &["db_id", "name", "db_type"]).unwrap();
    pub static ref CONTAINER_MEMORY_BYTES: GaugeVec =
        GaugeVec::new(opts!("dbsaas_container_memory_bytes", "Container memory usage bytes"), &["db_id", "name", "db_type"]).unwrap();
}

pub fn init_metrics() {
    REGISTRY.register(Box::new(TOTAL_USERS.clone())).ok();
    REGISTRY.register(Box::new(TOTAL_DATABASES.clone())).ok();
    REGISTRY.register(Box::new(DATABASES_BY_STATUS.clone())).ok();
    REGISTRY.register(Box::new(DATABASES_BY_TYPE.clone())).ok();
    REGISTRY.register(Box::new(SERVER_CONTAINERS.clone())).ok();
    REGISTRY.register(Box::new(SERVER_CPU_PERCENT.clone())).ok();
    REGISTRY.register(Box::new(SERVER_MEMORY_PERCENT.clone())).ok();
    REGISTRY.register(Box::new(MONTHLY_REVENUE_CENTS.clone())).ok();
    REGISTRY.register(Box::new(PENDING_REVENUE_CENTS.clone())).ok();
    REGISTRY.register(Box::new(HTTP_REQUEST_COUNT.clone())).ok();
    REGISTRY.register(Box::new(HTTP_REQUEST_DURATION.clone())).ok();
    REGISTRY.register(Box::new(CONTAINER_CPU_PERCENT.clone())).ok();
    REGISTRY.register(Box::new(CONTAINER_MEMORY_BYTES.clone())).ok();
}

pub fn render_metrics() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap_or_default()
}

pub async fn refresh_platform_metrics(pool: &PgPool) {
    // Total users
    if let Ok((count,)) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM users")
        .fetch_one(pool).await
    {
        TOTAL_USERS.set(count);
    }

    // Total databases
    if let Ok((count,)) = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM database_instances")
        .fetch_one(pool).await
    {
        TOTAL_DATABASES.set(count);
    }

    // By status
    if let Ok(rows) = sqlx::query_as::<_, (String, i64)>(
        "SELECT status::TEXT, COUNT(*) FROM database_instances GROUP BY status"
    ).fetch_all(pool).await {
        // Reset before setting
        DATABASES_BY_STATUS.reset();
        for (status, count) in rows {
            DATABASES_BY_STATUS.with_label_values(&[&status]).set(count as f64);
        }
    }

    // By type
    if let Ok(rows) = sqlx::query_as::<_, (String, i64)>(
        "SELECT db_type::TEXT, COUNT(*) FROM database_instances GROUP BY db_type"
    ).fetch_all(pool).await {
        DATABASES_BY_TYPE.reset();
        for (db_type, count) in rows {
            DATABASES_BY_TYPE.with_label_values(&[&db_type]).set(count as f64);
        }
    }

    // Monthly revenue
    if let Ok(result) = sqlx::query_as::<_, (Option<i64>,)>(
        "SELECT SUM(total_cents) FROM billing_periods WHERE status = 'paid' AND period_start >= date_trunc('month', NOW())"
    ).fetch_one(pool).await {
        MONTHLY_REVENUE_CENTS.set(result.0.unwrap_or(0));
    }

    // Pending revenue
    if let Ok(result) = sqlx::query_as::<_, (Option<i64>,)>(
        "SELECT SUM(total_cents) FROM billing_periods WHERE status = 'pending'"
    ).fetch_one(pool).await {
        PENDING_REVENUE_CENTS.set(result.0.unwrap_or(0));
    }
}
