/*
 * LUCKY VERITIES - relay do email de verificação
 * ================================================
 * Isto NÃO corre no teu PC nem vai dentro do jogo: cola isto no site script.google.com
 * (Google Apps Script), num projeto novo, com a TUA conta Google. Ver EMAIL_SETUP.md
 * na raiz do projeto para o passo-a-passo completo.
 *
 * O que faz: recebe um pedido do jogo (email + código de 6 dígitos), pede à Brevo para
 * mandar o email, e responde "ok" ou "erro". A chave da Brevo fica guardada aqui dentro
 * (em "Project Settings -> Script Properties"), NUNCA no código - por isso este ficheiro
 * pode ir para o GitHub sem problema nenhum, mesmo depois de colares a chave na consola.
 *
 * O URL deste Web App (depois do deploy) NÃO É SECRETO - é suposto ir dentro do mailer.py
 * do jogo. A única coisa que protege a tua conta Brevo de abuso é o limite diário (MAX_PER_DAY,
 * em baixo) e a validação do pedido. Ajusta o limite ao número de amigos que tens.
 */

var MAX_PER_DAY = 80;           // não deixa mandar mais do que isto por dia (protege a tua conta Brevo/Gmail)
var BREVO_ENDPOINT = "https://api.brevo.com/v3/smtp/email";
var SENDER = { name: "Lucky Verities", email: "guilherme.can.gomes@gmail.com" };

function doPost(e) {
  try {
    var body = JSON.parse(e.postData.contents);
    var email = String(body.email || "").trim();
    var code = String(body.code || "").trim();

    if (!isValidEmail(email)) {
      return jsonResponse({ ok: false, error: "invalid email" });
    }
    if (!/^\d{6}$/.test(code)) {
      return jsonResponse({ ok: false, error: "invalid code" });
    }
    if (!withinDailyLimit_()) {
      return jsonResponse({ ok: false, error: "daily limit reached, try again tomorrow" });
    }

    sendViaBrevo_(email, code);
    return jsonResponse({ ok: true });

  } catch (err) {
    return jsonResponse({ ok: false, error: String(err) });
  }
}

// Só para conseguires testar o deploy a abrir o URL no browser (dá sempre erro "method" -
// serve só para confirmares que o Web App está mesmo no ar antes de ligares o jogo).
function doGet(e) {
  return jsonResponse({ ok: false, error: "use POST, not GET - this only replies to the game" });
}

function isValidEmail(email) {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

// Guarda um contador por dia (UTC) nas Script Properties. Reseta sozinho porque a chave
// muda todos os dias ("count_2026-09-22", depois "count_2026-09-23", ...).
function withinDailyLimit_() {
  var props = PropertiesService.getScriptProperties();
  var key = "count_" + Utilities.formatDate(new Date(), "UTC", "yyyy-MM-dd");
  var count = parseInt(props.getProperty(key) || "0", 10);
  if (count >= MAX_PER_DAY) return false;
  props.setProperty(key, String(count + 1));
  return true;
}

function sendViaBrevo_(email, code) {
  var apiKey = PropertiesService.getScriptProperties().getProperty("BREVO_API_KEY");
  if (!apiKey) {
    throw new Error("BREVO_API_KEY not set - Project Settings > Script Properties");
  }

  var html =
    '<div style="font-family:Arial,Helvetica,sans-serif;font-size:16px;color:#1a1a1a;' +
    'max-width:420px;margin:0 auto">' +
    '<p>O teu c\u00f3digo de verifica\u00e7\u00e3o do <b>Lucky Verities</b> \u00e9:</p>' +
    '<p style="font-size:34px;font-weight:bold;letter-spacing:8px;text-align:center;' +
    'background:#f2f2f7;border-radius:10px;padding:16px 0;margin:18px 0">' + code + '</p>' +
    '<p>Introduz este c\u00f3digo no jogo para confirmares o teu email. O c\u00f3digo expira em 10 minutos.</p>' +
    '<p style="color:#777;font-size:13px">Se n\u00e3o pediste isto, ignora este email - a tua conta ' +
    'continua segura.</p></div>';
  var text =
    "O teu codigo de verificacao do Lucky Verities e: " + code + "\n" +
    "Introduz este codigo no jogo para confirmares o teu email. Expira em 10 minutos.\n" +
    "Se nao pediste isto, ignora este email.";

  var payload = {
    sender: SENDER,
    to: [{ email: email }],
    subject: "O teu c\u00f3digo Lucky Verities: " + code,
    htmlContent: html,
    textContent: text,
  };
  var options = {
    method: "post",
    contentType: "application/json",
    headers: { "api-key": apiKey },
    payload: JSON.stringify(payload),
    muteHttpExceptions: true,
  };
  var resp = UrlFetchApp.fetch(BREVO_ENDPOINT, options);
  if (resp.getResponseCode() >= 300) {
    throw new Error("Brevo error " + resp.getResponseCode() + ": " + resp.getContentText());
  }
}

function jsonResponse(obj) {
  // Um Web App do Apps Script responde SEMPRE com o código HTTP 200; o jogo sabe disto e
  // olha para o campo "ok" dentro do JSON, não para o código HTTP.
  return ContentService.createTextOutput(JSON.stringify(obj))
    .setMimeType(ContentService.MimeType.JSON);
}
