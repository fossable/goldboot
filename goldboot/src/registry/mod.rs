pub mod api;
pub mod client;
pub mod protocol;

pub use client::{Client, registry_root};

use anyhow::{Result, bail};

/// Parse a docker-style image reference: `[scheme://]host/name[:tag]`.
/// Returns `(host, name, tag)`. The scheme, if present, stays attached to
/// the host so the client picks it up via `registry_root`.
pub fn parse_image_ref(reference: &str) -> Result<(String, String, String)> {
    // Detect and split off an optional scheme so the rest of the parser can
    // operate on a plain `host/name[:tag]` slice.
    let (scheme_prefix, rest) = if let Some(rest) = reference.strip_prefix("http://") {
        ("http://", rest)
    } else if let Some(rest) = reference.strip_prefix("https://") {
        ("https://", rest)
    } else {
        ("", reference)
    };

    // Split off the tag first
    let (ref_no_tag, tag) = if let Some(pos) = rest.rfind(':') {
        // Make sure the colon isn't part of a host:port before a slash
        let after_colon = &rest[pos + 1..];
        if after_colon.contains('/') {
            // colon was in host:port, no tag
            (rest, "latest")
        } else {
            (&rest[..pos], after_colon)
        }
    } else {
        (rest, "latest")
    };

    // Split host from name at the first slash (host must contain a dot or colon to distinguish)
    let (host, name) = if let Some(slash) = ref_no_tag.find('/') {
        let potential_host = &ref_no_tag[..slash];
        if potential_host.contains('.') || potential_host.contains(':') {
            (potential_host, &ref_no_tag[slash + 1..])
        } else {
            bail!(
                "No registry host found in reference '{}'. Use [scheme://]host/name[:tag]",
                reference
            );
        }
    } else {
        bail!(
            "No registry host found in reference '{}'. Use [scheme://]host/name[:tag]",
            reference
        );
    };

    Ok((format!("{scheme_prefix}{host}"), name.to_string(), tag.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_image_ref ──────────────────────────────────────────────────────

    #[test]
    fn test_parse_ref_host_name_tag() {
        let (host, name, tag) = parse_image_ref("registry.example.com/archlinux:v1").unwrap();
        assert_eq!(host, "registry.example.com");
        assert_eq!(name, "archlinux");
        assert_eq!(tag, "v1");
    }

    #[test]
    fn test_parse_ref_host_name_no_tag() {
        let (host, name, tag) = parse_image_ref("registry.example.com/archlinux").unwrap();
        assert_eq!(host, "registry.example.com");
        assert_eq!(name, "archlinux");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_parse_ref_host_port_name_tag() {
        let (host, name, tag) = parse_image_ref("localhost:5000/myimage:beta").unwrap();
        assert_eq!(host, "localhost:5000");
        assert_eq!(name, "myimage");
        assert_eq!(tag, "beta");
    }

    #[test]
    fn test_parse_ref_host_port_name_no_tag() {
        let (host, name, tag) = parse_image_ref("localhost:5000/myimage").unwrap();
        assert_eq!(host, "localhost:5000");
        assert_eq!(name, "myimage");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_parse_ref_subpath_name() {
        // name can contain slashes (e.g. org/repo style)
        let (host, name, tag) =
            parse_image_ref("registry.example.com/myorg/myimage:latest").unwrap();
        assert_eq!(host, "registry.example.com");
        assert_eq!(name, "myorg/myimage");
        assert_eq!(tag, "latest");
    }

    #[test]
    fn test_parse_ref_no_host_rejected() {
        assert!(parse_image_ref("archlinux:latest").is_err());
        assert!(parse_image_ref("archlinux").is_err());
    }

    #[test]
    fn test_parse_ref_no_dot_no_colon_host_rejected() {
        // "local" has no dot or colon → not a registry host
        assert!(parse_image_ref("local/archlinux:latest").is_err());
    }
}
