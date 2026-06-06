//! Image endpoints: list, manifest, clusters (range-supported), push.

use crate::{cmd::start::ServerConfig, storage::Storage};
use anyhow::Result;
use axum::{
    Json,
    body::Body,
    extract::{Path, Request},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::TryStreamExt;
use goldboot::registry::protocol::{ImageListResponse, MANIFEST_CONTENT_TYPE, RegistryImageEntry};
use goldboot_image::ImageHandle;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio_util::io::ReaderStream;
use tracing::{info, warn};

/// `GET /v1/images`
pub async fn list(
    storage: axum::extract::Extension<Arc<Storage>>,
) -> Result<Json<ImageListResponse>, StatusCode> {
    let storage = storage.0.clone();
    let images = tokio::task::spawn_blocking(move || -> Result<Vec<RegistryImageEntry>> {
        let mut out = Vec::new();
        for (name, tag) in storage.list()? {
            let path = storage.image_path(&name, &tag)?;
            match ImageHandle::open(&path) {
                Ok(h) => out.push(RegistryImageEntry {
                    name,
                    tag,
                    size: h.primary_header.size,
                    arch: h.primary_header.arch,
                    timestamp: h.primary_header.timestamp,
                    // `h.id` is the content_id hex string (cluster-region SHA256).
                    id: h.id,
                }),
                Err(e) => warn!(error = ?e, path = %path.display(), "skipping unreadable image"),
            }
        }
        Ok(out)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|e| {
        warn!(error = ?e, "list failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    Ok(Json(ImageListResponse { images }))
}

/// `GET /v1/images/:name/tags/:tag/manifest`
pub async fn manifest(
    storage: axum::extract::Extension<Arc<Storage>>,
    Path((name, tag)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    let storage = storage.0.clone();
    let blob_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let path = storage.image_path(&name, &tag)?;
        let mut handle = ImageHandle::open(&path)?;
        if handle.directory.is_none() {
            handle.load(None).map_err(|_| {
                anyhow::anyhow!("encrypted images are not yet supported by the registry")
            })?;
        }
        let blob = handle.read_manifest_blob()?;
        Ok(blob.write_to())
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|e| {
        warn!(error = ?e, "manifest failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(MANIFEST_CONTENT_TYPE),
        )],
        blob_bytes,
    )
        .into_response())
}

/// `GET /v1/images/:name/tags/:tag/clusters` — streams the cluster region.
/// Supports `Range:` for resume.
pub async fn clusters(
    storage: axum::extract::Extension<Arc<Storage>>,
    Path((name, tag)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let storage = storage.0.clone();
    let (file_path, cluster_start, cluster_end) =
        tokio::task::spawn_blocking(move || -> Result<(std::path::PathBuf, u64, u64)> {
            let path = storage.image_path(&name, &tag)?;
            let mut handle = ImageHandle::open(&path)?;
            if handle.directory.is_none() {
                handle.load(None).ok();
            }
            let (s, e) = handle.cluster_region_bounds()?;
            Ok((path, s, e))
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map_err(|e| {
            warn!(error = ?e, "clusters lookup failed");
            StatusCode::NOT_FOUND
        })?;

    let total_len = cluster_end - cluster_start;
    let (range_start, range_end_inclusive) =
        parse_range_header(&headers, total_len).map_err(|_| StatusCode::RANGE_NOT_SATISFIABLE)?;

    let absolute_start = cluster_start + range_start;
    let absolute_len = range_end_inclusive - range_start + 1;

    let mut file = tokio::fs::File::open(&file_path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    use tokio::io::AsyncSeekExt;
    file.seek(std::io::SeekFrom::Start(absolute_start))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let limited = file.take(absolute_len);
    let stream = ReaderStream::new(limited);
    let body = Body::from_stream(stream.map_err(std::io::Error::from));

    let mut resp = Response::new(body);
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    resp.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&absolute_len.to_string()).unwrap(),
    );
    resp.headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if headers.contains_key(header::RANGE) {
        *resp.status_mut() = StatusCode::PARTIAL_CONTENT;
        resp.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!(
                "bytes {}-{}/{}",
                range_start, range_end_inclusive, total_len
            ))
            .unwrap(),
        );
    }
    Ok(resp)
}

/// Parse a single-range `Range: bytes=N-M` header against `total_len`.
/// Returns `(start, end_inclusive)` relative to the cluster region.
fn parse_range_header(headers: &HeaderMap, total_len: u64) -> Result<(u64, u64)> {
    let Some(value) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) else {
        if total_len == 0 {
            return Ok((0, 0));
        }
        return Ok((0, total_len - 1));
    };
    let body = value
        .strip_prefix("bytes=")
        .ok_or_else(|| anyhow::anyhow!("not a byte range"))?;
    let (s, e) = body
        .split_once('-')
        .ok_or_else(|| anyhow::anyhow!("malformed"))?;
    let start: u64 = s.parse()?;
    let end: u64 = if e.is_empty() {
        total_len - 1
    } else {
        e.parse()?
    };
    if start > end || end >= total_len {
        anyhow::bail!("out of bounds");
    }
    Ok((start, end))
}

/// `PUT /v1/images/:name/tags/:tag` — upload an image. Body size is capped
/// by the global RequestBodyLimitLayer applied during router construction.
pub async fn push(
    storage: axum::extract::Extension<Arc<Storage>>,
    server_config: axum::extract::Extension<ServerConfig>,
    Path((name, tag)): Path<(String, String)>,
    req: Request,
) -> Result<StatusCode, StatusCode> {
    let cap = server_config.0.max_upload_size;
    let body = req.into_body();
    let body_stream = body.into_data_stream();
    let async_read =
        tokio_util::io::StreamReader::new(body_stream.map_err(|e| std::io::Error::other(e)));

    // Hop to a blocking task to write the file with the sync API.
    let storage_clone = storage.0.clone();
    let name_c = name.clone();
    let tag_c = tag.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<u64> {
        use std::io::{Cursor, Read};
        let mut sync_reader = tokio_util::io::SyncIoBridge::new(async_read);
        let mut capped = (&mut sync_reader).take(cap + 1);
        let mut buf = Vec::new();
        capped.read_to_end(&mut buf)?;
        if buf.len() as u64 > cap {
            anyhow::bail!("upload exceeds max_upload_size");
        }

        // Parse the PrimaryHeader from the in-memory buffer and verify
        // that its `name` / `tag` match the URL path. Refuse mismatches —
        // the image carries its own identity now, so divergence means the
        // client is misconfigured.
        let header = goldboot_image::PrimaryHeader::read_from_bytes(&buf)
            .map_err(|e| anyhow::anyhow!("invalid image header: {e}"))?;
        let header_name = header.name_str();
        let header_tag = header.tag_str();
        if header_name != name_c {
            anyhow::bail!("URL name '{name_c}' does not match image header name '{header_name}'");
        }
        if header_tag != tag_c {
            anyhow::bail!("URL tag '{tag_c}' does not match image header tag '{header_tag}'");
        }

        storage_clone.put(&name_c, &tag_c, Cursor::new(buf), None)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match result {
        Ok(written) => {
            info!(
                event = "push.ok",
                image = %name,
                tag = %tag,
                bytes = written
            );
            Ok(StatusCode::CREATED)
        }
        Err(e) => {
            warn!(error = ?e, "push failed");
            Err(StatusCode::BAD_REQUEST)
        }
    }
}
