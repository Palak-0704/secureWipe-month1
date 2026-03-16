use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::errors::AppError;
use super::types::CertificateSignature;

const CERTIFICATE_ALGORITHM: &str = "ed25519";

pub fn sign_certificate_payload(certificate: &Value) -> Result<CertificateSignature, AppError> {
    let canonical_json = canonicalize_json(certificate)?;
    let payload_sha256 = sha256_hex(&canonical_json);
    let (signing_key, signer_source) = signing_key_from_env()?;
    let verifying_key = signing_key.verifying_key();
    let signature = signing_key.sign(canonical_json.as_bytes());

    Ok(CertificateSignature {
        algorithm: CERTIFICATE_ALGORITHM.to_string(),
        signer_source,
        public_key_base64: STANDARD.encode(verifying_key.to_bytes()),
        signature_base64: STANDARD.encode(signature.to_bytes()),
        payload_sha256,
    })
}

pub fn verify_certificate_payload(
    certificate: &Value,
    signature_base64: &str,
    public_key_base64: &str,
) -> Result<bool, AppError> {
    let canonical_json = canonicalize_json(certificate)?;
    let public_key_bytes = decode_fixed_bytes::<32>(public_key_base64, "certificate_public_key_invalid")?;
    let signature_bytes = decode_fixed_bytes::<64>(signature_base64, "certificate_signature_invalid")?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| {
        AppError::bad_request(
            "certificate_public_key_invalid",
            "Certificate public key is not a valid Ed25519 verifying key.",
        )
    })?;
    let signature = Signature::from_bytes(&signature_bytes);

    Ok(verifying_key
        .verify(canonical_json.as_bytes(), &signature)
        .is_ok())
}

pub fn payload_sha256(certificate: &Value) -> Result<String, AppError> {
    Ok(sha256_hex(&canonicalize_json(certificate)?))
}

fn signing_key_from_env() -> Result<(SigningKey, String), AppError> {
    let raw_seed = std::env::var("SECUREWIPE_CERT_SIGNING_SEED").map_err(|_| {
        AppError::service_unavailable(
            "certificate_signing_seed_missing",
            "SECUREWIPE_CERT_SIGNING_SEED is required and must contain a 32-byte hex/base64 seed.",
        )
    })?;

    let seed = decode_seed(raw_seed.trim()).ok_or_else(|| {
        AppError::bad_request(
            "certificate_signing_seed_invalid",
            "SECUREWIPE_CERT_SIGNING_SEED must be a 32-byte value encoded as hex or base64.",
        )
    })?;

    Ok((SigningKey::from_bytes(&seed), "environment_seed".to_string()))
}

fn decode_seed(raw_seed: &str) -> Option<[u8; 32]> {
    if let Some(bytes) = decode_hex(raw_seed) {
        return bytes.try_into().ok();
    }

    STANDARD.decode(raw_seed).ok()?.try_into().ok()
}

fn decode_hex(raw: &str) -> Option<Vec<u8>> {
    if raw.len() != 64 || !raw.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }

    let mut bytes = Vec::with_capacity(32);
    for chunk in raw.as_bytes().chunks(2) {
        let value = std::str::from_utf8(chunk).ok()?;
        let byte = u8::from_str_radix(value, 16).ok()?;
        bytes.push(byte);
    }

    Some(bytes)
}

fn decode_fixed_bytes<const N: usize>(raw: &str, code: &'static str) -> Result<[u8; N], AppError> {
    let decoded = STANDARD.decode(raw).map_err(|_| {
        AppError::bad_request(
            code,
            "Certificate signature material must be valid base64.",
        )
    })?;

    decoded.try_into().map_err(|_| {
        AppError::bad_request(
            code,
            format!("Certificate signature material must decode to exactly {} bytes.", N),
        )
    })
}

fn canonicalize_json(value: &Value) -> Result<String, AppError> {
    serde_json::to_string(value).map_err(|_| {
        AppError::internal_server_error(
            "certificate_serialization_failed",
            "Failed to serialize certificate payload for signing.",
        )
    })
}

fn sha256_hex(payload: &str) -> String {
    let digest = Sha256::digest(payload.as_bytes());
    format!("{:x}", digest)
}