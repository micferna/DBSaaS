use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
    KeyUsagePurpose, SanType,
};
use std::path::Path;

use crate::error::{AppError, AppResult};

pub struct TlsService {
    ca_dir: String,
}

#[derive(Debug)]
pub struct CertPair {
    pub cert_pem: String,
    pub key_pem: String,
}

impl TlsService {
    pub fn new(ca_dir: String) -> Self {
        Self { ca_dir }
    }

    pub fn init_ca(&self) -> AppResult<()> {
        let cert_path = Path::new(&self.ca_dir).join("ca.crt");
        let key_path = Path::new(&self.ca_dir).join("ca.key");

        if cert_path.exists() && key_path.exists() {
            tracing::info!("CA certificate already exists");
            return Ok(());
        }

        std::fs::create_dir_all(&self.ca_dir)
            .map_err(|e| AppError::Internal(format!("Failed to create CA dir: {e}")))?;

        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "DBSaaS Internal CA");
        dn.push(DnType::OrganizationName, "DBSaaS Platform");
        params.distinguished_name = dn;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];

        let key_pair = KeyPair::generate()
            .map_err(|e| AppError::Internal(format!("Failed to generate CA key: {e}")))?;
        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| AppError::Internal(format!("Failed to self-sign CA: {e}")))?;

        std::fs::write(&cert_path, cert.pem())
            .map_err(|e| AppError::Internal(format!("Failed to write CA cert: {e}")))?;
        std::fs::write(&key_path, key_pair.serialize_pem())
            .map_err(|e| AppError::Internal(format!("Failed to write CA key: {e}")))?;

        tracing::info!("CA certificate generated");
        Ok(())
    }

    pub fn generate_server_cert(&self, hostname: &str, port: u16) -> AppResult<CertPair> {
        let ca_cert_pem = std::fs::read_to_string(Path::new(&self.ca_dir).join("ca.crt"))
            .map_err(|e| AppError::Internal(format!("Failed to read CA cert: {e}")))?;
        let ca_key_pem = std::fs::read_to_string(Path::new(&self.ca_dir).join("ca.key"))
            .map_err(|e| AppError::Internal(format!("Failed to read CA key: {e}")))?;

        let ca_key_pair = KeyPair::from_pem(&ca_key_pem)
            .map_err(|e| AppError::Internal(format!("Failed to parse CA key: {e}")))?;

        let issuer = Issuer::from_ca_cert_pem(&ca_cert_pem, ca_key_pair)
            .map_err(|e| AppError::Internal(format!("Failed to build CA issuer: {e}")))?;

        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, hostname);
        params.distinguished_name = dn;
        params.subject_alt_names = vec![
            SanType::DnsName(hostname.try_into().map_err(|e| {
                AppError::Internal(format!("Invalid DNS name: {e}"))
            })?),
            SanType::DnsName(
                format!("{hostname}-{port}")
                    .try_into()
                    .map_err(|e| AppError::Internal(format!("Invalid DNS name: {e}")))?,
            ),
        ];

        let server_key = KeyPair::generate()
            .map_err(|e| AppError::Internal(format!("Failed to generate server key: {e}")))?;
        let server_cert = params
            .signed_by(&server_key, &issuer)
            .map_err(|e| AppError::Internal(format!("Failed to sign server cert: {e}")))?;

        Ok(CertPair {
            cert_pem: server_cert.pem(),
            key_pem: server_key.serialize_pem(),
        })
    }

    /// Generate a TLS certificate for a subdomain FQDN (SNI routing).
    /// SAN = the full FQDN (e.g. `mydb-a1b2c3d4.db.example.com`)
    pub fn generate_cert_for_subdomain(&self, subdomain_fqdn: &str) -> AppResult<CertPair> {
        let ca_cert_pem = std::fs::read_to_string(Path::new(&self.ca_dir).join("ca.crt"))
            .map_err(|e| AppError::Internal(format!("Failed to read CA cert: {e}")))?;
        let ca_key_pem = std::fs::read_to_string(Path::new(&self.ca_dir).join("ca.key"))
            .map_err(|e| AppError::Internal(format!("Failed to read CA key: {e}")))?;

        let ca_key_pair = KeyPair::from_pem(&ca_key_pem)
            .map_err(|e| AppError::Internal(format!("Failed to parse CA key: {e}")))?;

        let issuer = Issuer::from_ca_cert_pem(&ca_cert_pem, ca_key_pair)
            .map_err(|e| AppError::Internal(format!("Failed to build CA issuer: {e}")))?;

        let mut params = CertificateParams::default();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, subdomain_fqdn);
        params.distinguished_name = dn;
        params.subject_alt_names = vec![
            SanType::DnsName(subdomain_fqdn.try_into().map_err(|e| {
                AppError::Internal(format!("Invalid DNS name: {e}"))
            })?),
        ];

        let server_key = KeyPair::generate()
            .map_err(|e| AppError::Internal(format!("Failed to generate server key: {e}")))?;
        let server_cert = params
            .signed_by(&server_key, &issuer)
            .map_err(|e| AppError::Internal(format!("Failed to sign server cert: {e}")))?;

        Ok(CertPair {
            cert_pem: server_cert.pem(),
            key_pem: server_key.serialize_pem(),
        })
    }

    pub fn get_ca_cert(&self) -> AppResult<String> {
        std::fs::read_to_string(Path::new(&self.ca_dir).join("ca.crt"))
            .map_err(|e| AppError::Internal(format!("Failed to read CA cert: {e}")))
    }
}
