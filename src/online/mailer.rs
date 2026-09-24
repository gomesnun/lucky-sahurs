//! Sends the 6-digit verification code by email through the Apps Script relay (online/mailer.py).

use crate::config::save_dir;
use serde_json::{Value, json};
use std::time::Duration;

pub const GAS_ENDPOINT: &str = "https://script.google.com/macros/s/AKfycbwi1MfcFdY2k5D_KN6W2fs-8t87iypr04QkHBevQVgsWz6FeNL2T3Km2HDeH7xFuG8f/exec";
const BREVO_ENDPOINT: &str = "https://api.brevo.com/v3/smtp/email";
const MAIL_TIMEOUT: f64 = 12.0;

fn local_brevo_key() -> String {
    if let Ok(k) = std::env::var("BREVO_API_KEY") {
        if !k.trim().is_empty() {
            return k.trim().to_string();
        }
    }
    let p = save_dir().join("brevo_key.json");
    if let Ok(t) = std::fs::read_to_string(p) {
        if let Ok(Value::Object(d)) = serde_json::from_str::<Value>(&t) {
            return d.get("brevo_api_key").map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())).unwrap_or_default().trim().to_string();
        }
    }
    String::new()
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs_f64(MAIL_TIMEOUT))).http_status_as_error(false).build().into()
}

pub fn send_verification_code(to_email: &str, code: &str) -> Result<(), String> {
    if !GAS_ENDPOINT.trim().is_empty() {
        send_via_relay(to_email, code)
    } else {
        let key = local_brevo_key();
        if !key.is_empty() {
            send_via_brevo_direct(to_email, code, &key)
        } else {
            Err("email sending isn't configured (no GAS_ENDPOINT / local BREVO key)".into())
        }
    }
}

fn send_via_relay(to_email: &str, code: &str) -> Result<(), String> {
    let data = serde_json::to_vec(&json!({"email": to_email, "code": code})).unwrap_or_default();
    let mut resp = agent().post(GAS_ENDPOINT).header("Content-Type", "application/json").send(&data[..]).map_err(|e| e.to_string())?;
    let status = resp.status();
    if status.as_u16() >= 400 {
        return Err(format!("relay HTTP {}: {}", status.as_u16(), status.canonical_reason().unwrap_or("")));
    }
    let text = resp.body_mut().read_to_string().map_err(|e| e.to_string())?;
    let payload: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !payload.get("ok").map(crate::storage::value_truthy).unwrap_or(false) {
        return Err(payload.get("error").map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())).unwrap_or_else(|| "the relay refused the request".into()));
    }
    Ok(())
}

fn send_via_brevo_direct(to_email: &str, code: &str, api_key: &str) -> Result<(), String> {
    let subject = format!("O teu código Lucky Verities: {}", code);
    let html = format!(
        "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:16px;color:#1a1a1a;max-width:420px;margin:0 auto\">\
<p>O teu código de verificação do <b>Lucky Verities</b> é:</p>\
<p style=\"font-size:34px;font-weight:bold;letter-spacing:8px;text-align:center;background:#f2f2f7;border-radius:10px;padding:16px 0;margin:18px 0\">{}</p>\
<p>Introduz este código no jogo para confirmares o teu email. O código expira em 10 minutos.</p>\
<p style=\"color:#777;font-size:13px\">Se não pediste isto, ignora este email — a tua conta continua segura.</p></div>",
        code
    );
    let text = format!(
        "O teu código de verificação do Lucky Verities é: {}\nIntroduz este código no jogo para confirmares o teu email. Expira em 10 minutos.\nSe não pediste isto, ignora este email.",
        code
    );
    let body = json!({
        "sender": {"name": "Lucky Verities", "email": "guilherme.can.gomes@gmail.com"},
        "to": [{"email": to_email}],
        "subject": subject,
        "htmlContent": html,
        "textContent": text,
    });
    let data = serde_json::to_vec(&body).unwrap_or_default();
    let mut resp = agent()
        .post(BREVO_ENDPOINT)
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .header("api-key", api_key)
        .send(&data[..])
        .map_err(|e| e.to_string())?;
    if resp.status().as_u16() >= 400 {
        let t = resp.body_mut().read_to_string().unwrap_or_default();
        let msg = serde_json::from_str::<Value>(&t).ok().and_then(|v| v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string())).unwrap_or(t);
        return Err(msg);
    }
    Ok(())
}
