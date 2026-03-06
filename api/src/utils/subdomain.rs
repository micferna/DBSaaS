use uuid::Uuid;

/// Generate a subdomain from a database name and UUID.
/// Format: `{sanitized_name}-{id8}.db`
/// The sanitized name is lowercase, alphanumeric + hyphens, max 48 chars.
pub fn generate_subdomain(name: &str, id: Uuid) -> String {
    let sanitized: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();

    // Remove leading/trailing hyphens and collapse multiple hyphens
    let sanitized: String = sanitized
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    // Truncate to max 48 chars
    let truncated = if sanitized.len() > 48 {
        &sanitized[..48]
    } else {
        &sanitized
    };

    // Take first 8 hex chars of UUID (no dashes)
    let id_hex: String = id.to_string().replace('-', "");
    let id8 = &id_hex[..8];

    format!("{}-{}.db", truncated, id8)
}

/// Build the FQDN for a subdomain given the platform domain.
/// Returns `{subdomain}.{platform_domain}` (e.g. `mydb-a1b2c3d4.db.example.com`)
pub fn subdomain_fqdn(subdomain: &str, platform_domain: &str) -> String {
    format!("{}.{}", subdomain, platform_domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_subdomain() {
        let id = Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap();
        let sub = generate_subdomain("My_Test DB", id);
        assert_eq!(sub, "my-test-db-a1b2c3d4.db");
    }

    #[test]
    fn test_subdomain_fqdn() {
        let fqdn = subdomain_fqdn("mydb-a1b2c3d4.db", "example.com");
        assert_eq!(fqdn, "mydb-a1b2c3d4.db.example.com");
    }

    #[test]
    fn test_long_name_truncated() {
        let id = Uuid::parse_str("a1b2c3d4-e5f6-7890-abcd-ef1234567890").unwrap();
        let long_name = "a".repeat(100);
        let sub = generate_subdomain(&long_name, id);
        // 48 chars + "-" + 8 chars + ".db" = 60 chars
        assert!(sub.len() <= 60);
    }
}
