use hmac::Mac;

pub fn verify_incoming_hmac(
    frames: &[Vec<u8>],
    config_key: &str,
    config_signature_scheme: &str,
    delim_index: usize,
) -> anyhow::Result<()> {
    if config_key.is_empty() {
        println!("Empty config key, skipping HMAC check");
        Ok(())
    } else {
        let incoming_sig = std::str::from_utf8(&frames[delim_index + 1]).unwrap_or("invalid");
        // Recompute signature over received header/parent/metadata/content
        let header_bytes = &frames[delim_index + 2];
        let parent_bytes = &frames[delim_index + 3];
        let metadata_bytes = &frames[delim_index + 4];
        let content_bytes = &frames[delim_index + 5];
        let expected_sig = sign_message(
            config_key,
            config_signature_scheme,
            header_bytes,
            parent_bytes,
            metadata_bytes,
            content_bytes,
        );
        println!("Incoming HMAC was: {incoming_sig}");
        if incoming_sig != expected_sig {
            return Err(anyhow::anyhow!("Warning: incoming HMAC mismatch"));
        }
        Ok(())
    }
}

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

pub fn sign_message(
    key: &str,
    signature_scheme: &str,
    header: &[u8],
    parent_header: &[u8],
    metadata: &[u8],
    content: &[u8],
) -> String {
    if key.is_empty() {
        println!("Empty key, skipping HMAC validation");
        return String::new();
    }
    // TODO: Is this check right?
    if signature_scheme != "hmac-sha256" {
        eprintln!("wrong signature schema: {signature_scheme}")
    }

    let mut mac: HmacSha256 = HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC key error");
    mac.update(header);
    mac.update(parent_header);
    mac.update(metadata);
    mac.update(content);
    hex::encode(mac.finalize().into_bytes())
}
