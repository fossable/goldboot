//! Authentication endpoints: login and logout.

use crate::auth::{AppState, AuthenticatedUser, issue_token, revoke_token};
use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::StatusCode,
    response::IntoResponse,
};
use goldboot::registry::protocol::{LoginRequest, LoginResponse};
use std::net::SocketAddr;
use zeroize::Zeroize;

/// `POST /v1/auth/login`
///
/// Verifies username + password via argon2id and issues a fresh bearer
/// token. Always returns a generic 401 on failure so the response does
/// not distinguish "wrong password" from "no such user".
pub async fn login(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(mut req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    let ttl = state.config.server.token_ttl_secs;
    let result = issue_token(
        &state.config,
        &state.tokens,
        &state.limiter,
        addr.ip(),
        &req.username,
        &req.password,
        ttl,
    );
    // Clear the plaintext password from memory ASAP, regardless of outcome.
    req.password.zeroize();

    let (token, record) = result?;
    Ok(Json(LoginResponse {
        token,
        expires_at: record.expires_at,
        permissions: record.permissions,
    }))
}

/// `POST /v1/auth/logout`
///
/// Revokes the bearer token used to make the request. The extractor has
/// already validated it; we just need to scrub it from the store.
pub async fn logout(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Some(value) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
    {
        revoke_token(&state.tokens, value.trim());
    }
    StatusCode::NO_CONTENT
}
