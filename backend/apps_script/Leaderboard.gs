/*
 * LUCKY VERITIES - leaderboard partilhada (uma fotografia para toda a gente)
 * ===========================================================================
 * Isto NÃO corre no teu PC nem vai dentro do jogo: é um projeto NOVO no script.google.com
 * (à parte do Code.gs do email), feito com a MESMA conta Google que é dona do projeto Firebase.
 * Passo-a-passo em backend/LEADERBOARD_SETUP.md.
 *
 * Antes: cada jogador corria 4 queries (money, playtime, rolls, rebirths, até 50 cada) =
 * até 200 leituras do Firestore POR JOGADOR, a cada período.
 * Agora: o jogo lê primeiro o documento /public/leaderboard (1 leitura). Se essa fotografia já
 * for do período atual, acabou. Se estiver velha, o jogo chama este script, que tira a fotografia
 * UMA vez (as mesmas ~200 leituras), guarda-a no documento e devolve-a. Quem vier a seguir, no
 * mesmo período, só lê o documento. Sem ninguém a jogar, isto não gasta nada (não há trigger por tempo).
 *
 * Porque é que só este script pode escrever o documento: as regras do Firestore dizem
 * "allow write: if false" para /public/leaderboard, mas este script entra com a TUA conta Google
 * (dona do projeto), e essa conta passa por cima das regras. Os jogadores não conseguem.
 *
 * O URL do Web App (depois do deploy) NÃO É SECRETO: vai em LEADERBOARD_ENDPOINT, em online/firebase.py.
 */

var PROJECT_ID = "lucky-sahurs";           // Firebase: Project settings -> General -> "Project ID"
var PERIOD_SECONDS = 30 * 60;              // de quanto em quanto tempo há uma fotografia nova (igual a
                                           // LEADERBOARD_SNAPSHOT_PERIOD em online/firebase.py)
var FETCH_DELAY_SECONDS = 60;              // igual a LEADERBOARD_FETCH_DELAY: dá tempo a toda a gente de publicar
var SIZE = 50;                             // igual a LEADERBOARD_SIZE
var TABLES = [                             // [chave no jogo, campo em /leaderboard/{uid}, mínimo (ou null)]
  ["money", "coins", null],
  ["playtime", "playtime", null],
  ["rolls", "rolls", null],
  ["rebirths", "rebirths", 1],             // só entra quem já fez pelo menos 1 rebirth
];

var DOCS = "https://firestore.googleapis.com/v1/projects/" + PROJECT_ID + "/databases/(default)/documents";
var SNAPSHOT_PATH = "/public/leaderboard";

function doPost(e) {
  return jsonResponse(handle_());
}

// Também responde a GET: dá para abrir o URL no browser e ver se está tudo a funcionar.
function doGet(e) {
  return jsonResponse(handle_());
}

function handle_() {
  try {
    return { ok: true, snapshot: ensureSnapshot_() };
  } catch (err) {
    return { ok: false, error: String(err) };
  }
}

function currentPeriod_() {
  return Math.floor((Date.now() / 1000 - FETCH_DELAY_SECONDS) / PERIOD_SECONDS);
}

function ensureSnapshot_() {
  var period = currentPeriod_();
  var snap = readSnapshot_();
  if (snap && snap.period >= period) {
    return snap;                                   // já há uma fotografia deste período: 1 leitura só
  }
  // Dois jogadores a chegar ao mesmo tempo não podem tirar duas fotografias: o 2.º espera pelo 1.º
  // e depois já encontra a fotografia feita.
  var lock = LockService.getScriptLock();
  lock.waitLock(25000);
  try {
    snap = readSnapshot_();
    if (snap && snap.period >= period) {
      return snap;
    }
    snap = { period: period, fetched_at: Date.now() / 1000 };
    for (var i = 0; i < TABLES.length; i++) {
      snap[TABLES[i][0]] = topEntries_(TABLES[i][1], TABLES[i][2]);
    }
    // v3.0 (versão Rust): o Prestígio conta primeiro na tabela dos Rebirths. Junta-se o top por Prestígio
    // ao top por Rebirths e ordena-se por (prestígio, rebirths), para quem fez Prestígio (e voltou a 0
    // rebirths) continuar na tabela.
    snap.rebirths = mergeRebirths_(snap.rebirths, topEntries_("prestige", 1));
    writeSnapshot_(snap);
    return snap;
  } finally {
    lock.releaseLock();
  }
}

function readSnapshot_() {
  var resp = firestore_("get", SNAPSHOT_PATH, null);
  if (resp.code === 404) return null;
  if (resp.code >= 300) throw new Error("read snapshot: " + resp.code + " " + resp.text);
  var f = (resp.json.fields || {});
  if (!f.json || !f.json.stringValue) return null;
  try {
    return JSON.parse(f.json.stringValue);
  } catch (err) {
    return null;
  }
}

function writeSnapshot_(snap) {
  // A fotografia vai inteira num só campo de texto (JSON): o jogo lê-a com um GET e um json.loads.
  var body = { fields: {
    period: { integerValue: String(snap.period) },
    json: { stringValue: JSON.stringify(snap) },
  } };
  var resp = firestore_("patch", SNAPSHOT_PATH, body);
  if (resp.code >= 300) throw new Error("write snapshot: " + resp.code + " " + resp.text);
}

function topEntries_(field, minValue) {
  var query = {
    from: [{ collectionId: "leaderboard" }],
    orderBy: [{ field: { fieldPath: field }, direction: "DESCENDING" }],
    limit: SIZE,
  };
  if (minValue !== null) {
    query.where = { fieldFilter: { field: { fieldPath: field }, op: "GREATER_THAN_OR_EQUAL",
                                   value: { integerValue: String(minValue) } } };
  }
  var resp = firestore_("post", ":runQuery", { structuredQuery: query });
  if (resp.code >= 300) throw new Error("query " + field + ": " + resp.code + " " + resp.text);
  var out = [];
  var rows = resp.json || [];
  for (var i = 0; i < rows.length; i++) {
    var doc = rows[i].document;
    if (!doc) continue;
    var f = doc.fields || {};
    var row = { username: f.username ? String(f.username.stringValue) : "?", value: numberOf_(f[field]),
                rebirths: numberOf_(f.rebirths) };
    var prestige = numberOf_(f.prestige);
    if (prestige > 0) row.prestige = prestige;
    out.push(row);
  }
  return out;
}

function mergeRebirths_(a, b) {
  var seen = {};
  var out = [];
  var all = a.concat(b);
  for (var i = 0; i < all.length; i++) {
    if (seen[all[i].username]) continue;
    seen[all[i].username] = true;
    all[i].value = all[i].rebirths;      // a tabela mostra os rebirths (o prestígio vai ao lado)
    out.push(all[i]);
  }
  out.sort(function (x, y) {
    return ((y.prestige || 0) - (x.prestige || 0)) || (y.rebirths - x.rebirths);
  });
  return out.slice(0, SIZE);
}

function numberOf_(v) {
  if (!v) return 0;
  if (v.doubleValue !== undefined) return Number(v.doubleValue);
  if (v.integerValue !== undefined) return Number(v.integerValue);
  return 0;
}

function firestore_(method, suffix, body) {
  // O token é o da conta Google dona deste script (e do projeto Firebase): passa por cima das regras.
  var options = {
    method: method,
    contentType: "application/json",
    headers: { Authorization: "Bearer " + ScriptApp.getOAuthToken() },
    muteHttpExceptions: true,
  };
  if (body !== null) options.payload = JSON.stringify(body);
  var resp = UrlFetchApp.fetch(DOCS + suffix, options);
  var text = resp.getContentText();
  var json = null;
  try { json = JSON.parse(text); } catch (err) { json = null; }
  return { code: resp.getResponseCode(), text: text.slice(0, 300), json: json };
}

function jsonResponse(obj) {
  return ContentService.createTextOutput(JSON.stringify(obj))
    .setMimeType(ContentService.MimeType.JSON);
}
