//! CA certificate management for the interceptor proxy.
//!
//! Uses rcgen (via hudsucker re-export) to generate a self-signed CA certificate,
//! stored persistently so users only need to trust it once.

use anyhow::{Context, Result};
use hudsucker::{
    certificate_authority::RcgenAuthority,
    rcgen::{
        BasicConstraints, CertificateParams, DistinguishedName, DnType, IsCa, Issuer, KeyPair,
        KeyUsagePurpose,
    },
    rustls::crypto::aws_lc_rs,
};
use std::path::{Path, PathBuf};

/// Wrapper around hudsucker's RcgenAuthority with persistent storage paths.
pub struct CertAuthority {
    pub authority: RcgenAuthority,
    pub cert_pem_path: PathBuf,
    pub key_pem_path: PathBuf,
}

/// Ensure CA certificate exists. Load or generate as needed.
pub fn ensure_ca(cert_dir: &Path) -> Result<CertAuthority> {
    let cert_pem_path = cert_dir.join("echovault-ca.crt");
    let key_pem_path = cert_dir.join("echovault-ca.key");

    if cert_pem_path.exists() && key_pem_path.exists() {
        tracing::info!(
            "[interceptor] Loading existing CA from {}",
            cert_dir.display()
        );
        load_ca(&cert_pem_path, &key_pem_path)
    } else {
        tracing::info!("[interceptor] Generating new CA in {}", cert_dir.display());
        generate_ca(&cert_pem_path, &key_pem_path)
    }
}

/// Load existing CA from PEM files.
fn load_ca(cert_path: &Path, key_path: &Path) -> Result<CertAuthority> {
    let cert_pem = std::fs::read_to_string(cert_path).context("Failed to read CA certificate")?;
    let key_pem = std::fs::read_to_string(key_path).context("Failed to read CA private key")?;

    let key_pair = KeyPair::from_pem(&key_pem).context("Failed to parse CA private key")?;
    let issuer =
        Issuer::from_ca_cert_pem(&cert_pem, key_pair).context("Failed to parse CA certificate")?;

    let authority = RcgenAuthority::new(issuer, 1_000, aws_lc_rs::default_provider());

    Ok(CertAuthority {
        authority,
        cert_pem_path: cert_path.to_path_buf(),
        key_pem_path: key_path.to_path_buf(),
    })
}

/// Generate new CA certificate and save to disk.
fn generate_ca(cert_path: &Path, key_path: &Path) -> Result<CertAuthority> {
    let mut params = CertificateParams::default();

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, "EchoVault Interceptor CA");
    dn.push(DnType::OrganizationName, "EchoVault");
    params.distinguished_name = dn;

    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![
        KeyUsagePurpose::KeyCertSign,
        KeyUsagePurpose::CrlSign,
        KeyUsagePurpose::DigitalSignature,
    ];

    // Default validity (rcgen defaults) is sufficient for dev CA

    let key_pair = KeyPair::generate().context("Failed to generate CA key pair")?;
    let cert = params
        .self_signed(&key_pair)
        .context("Failed to self-sign CA certificate")?;

    // Save as PEM
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    std::fs::write(cert_path, &cert_pem).context("Failed to write CA certificate")?;
    std::fs::write(key_path, &key_pem).context("Failed to write CA private key")?;

    // Restrict key file permissions on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))?;
    }

    tracing::info!("[interceptor] CA saved to {}", cert_path.display());

    // Reload from generated PEM to get RcgenAuthority
    let key_pair = KeyPair::from_pem(&key_pem).context("Failed to re-parse generated key")?;
    let issuer = Issuer::from_ca_cert_pem(&cert_pem, key_pair)
        .context("Failed to re-parse generated certificate")?;

    let authority = RcgenAuthority::new(issuer, 1_000, aws_lc_rs::default_provider());

    Ok(CertAuthority {
        authority,
        cert_pem_path: cert_path.to_path_buf(),
        key_pem_path: key_path.to_path_buf(),
    })
}
