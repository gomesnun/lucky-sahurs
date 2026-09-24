"""Envio do email com o código de verificação (6 dígitos) usado para confirmar a conta.

A CHAVE DA BREVO NUNCA FICA AQUI NEM EM LADO NENHUM QUE VÁ PARA O GITHUB. Em vez de o jogo
falar diretamente com a Brevo, fala com um pequeno "relay" que tu (e só tu) controlas — um
Google Apps Script gratuito, sem cartão de crédito. É esse relay que guarda a chave da Brevo
em segredo e manda o email; o jogo só lhe pede "manda este código a este email".

    jogo (todos os PCs) --(email + código, sem chave nenhuma)--> o teu Apps Script
                                                                        |  (chave da Brevo,
                                                                        |   guardada só aqui)
                                                                        v
                                                                     Brevo -> email

Configuração: ver EMAIL_SETUP.md na raiz do projeto (passo-a-passo, ~10 min, sem cartão).
Depois de fazeres o deploy do backend/apps_script/Code.gs, cola o URL do Web App em GAS_ENDPOINT
aqui em baixo. Esse URL NÃO É SECRETO — pode ficar à vista no GitHub sem problema; a única coisa
que protege a tua conta Brevo de abuso é o limite diário configurado dentro do próprio script.
Com o GAS_ENDPOINT vazio, o jogo funciona à mesma, só que sem mandar emails.

(Modo alternativo, só para testares no TEU PC sem montares o relay: define a variável de
ambiente BREVO_API_KEY, ou cria um ficheiro brevo_key.json — {"brevo_api_key": "..."} — na
pasta dos saves. Nesse caso o jogo fala com a Brevo diretamente, SÓ nesse PC. Não faças isto
num jogo que vais distribuir aos teus amigos — nesse caso usa sempre o relay acima.)
"""

import json
import os
import urllib.error
import urllib.request

from config import SAVE_DIR
from online.tls import ssl_context

# Cola aqui o URL do teu Web App depois do deploy (ver EMAIL_SETUP.md). Fica vazio até lá.
GAS_ENDPOINT = "https://script.google.com/macros/s/AKfycbwi1MfcFdY2k5D_KN6W2fs-8t87iypr04QkHBevQVgsWz6FeNL2T3Km2HDeH7xFuG8f/exec"     # ex.: "https://script.google.com/macros/s/AKfycb.../exec"

# Só para testes locais no teu próprio PC — ver aviso no docstring acima.
BREVO_ENDPOINT = "https://api.brevo.com/v3/smtp/email"
SENDER = {"name": "Lucky Verities", "email": "guilherme.can.gomes@gmail.com"}
LOCAL_KEY_PATH = os.path.join(SAVE_DIR, "brevo_key.json")   # {"brevo_api_key": "..."}

MAIL_TIMEOUT = 12.0


class MailError(Exception):
    """Falha a mandar o email (rede, relay em baixo, chave inválida, remetente não verificado...)."""


def _local_brevo_key():
    key = os.environ.get("BREVO_API_KEY", "").strip()
    if key:
        return key
    if os.path.exists(LOCAL_KEY_PATH):
        try:
            with open(LOCAL_KEY_PATH, "r", encoding="utf-8") as f:
                return str(json.load(f).get("brevo_api_key", "")).strip()
        except (OSError, ValueError, AttributeError):
            pass
    return ""


def mailer_ready():
    return bool(GAS_ENDPOINT.strip()) or bool(_local_brevo_key())


def send_verification_code(to_email, code):
    """Manda o código de 6 dígitos por email. Deve correr numa thread do Worker (é uma chamada de
    rede bloqueante), nunca no loop principal do jogo. Usa o relay (GAS_ENDPOINT) sempre que
    estiver configurado; só recorre à Brevo diretamente no modo de teste local (ver docstring)."""
    if GAS_ENDPOINT.strip():
        _send_via_relay(to_email, code)
    elif _local_brevo_key():
        _send_via_brevo_direct(to_email, code, _local_brevo_key())
    else:
        raise MailError("email sending isn't configured (no GAS_ENDPOINT / local BREVO key)")


def _send_via_relay(to_email, code):
    data = json.dumps({"email": to_email, "code": code}).encode("utf-8")
    req = urllib.request.Request(GAS_ENDPOINT, data=data, method="POST",
                                 headers={"Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=MAIL_TIMEOUT, context=ssl_context()) as resp:
            payload = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as e:
        raise MailError("relay HTTP %d: %s" % (e.code, e.reason))
    except (urllib.error.URLError, OSError, ValueError) as e:
        raise MailError(str(e))
    if not payload.get("ok"):
        raise MailError(payload.get("error", "the relay refused the request"))


def _send_via_brevo_direct(to_email, code, api_key):
    subject = "O teu código Lucky Verities: %s" % code
    html = (
        "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:16px;color:#1a1a1a;"
        "max-width:420px;margin:0 auto\">"
        "<p>O teu código de verificação do <b>Lucky Verities</b> é:</p>"
        "<p style=\"font-size:34px;font-weight:bold;letter-spacing:8px;text-align:center;"
        "background:#f2f2f7;border-radius:10px;padding:16px 0;margin:18px 0\">%s</p>"
        "<p>Introduz este código no jogo para confirmares o teu email. O código expira em 10 minutos.</p>"
        "<p style=\"color:#777;font-size:13px\">Se não pediste isto, ignora este email — a tua conta "
        "continua segura.</p>"
        "</div>" % code
    )
    text = ("O teu código de verificação do Lucky Verities é: %s\n"
            "Introduz este código no jogo para confirmares o teu email. Expira em 10 minutos.\n"
            "Se não pediste isto, ignora este email." % code)
    body = {
        "sender": SENDER,
        "to": [{"email": to_email}],
        "subject": subject,
        "htmlContent": html,
        "textContent": text,
    }
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(BREVO_ENDPOINT, data=data, method="POST", headers={
        "accept": "application/json",
        "content-type": "application/json",
        "api-key": api_key,
    })
    try:
        with urllib.request.urlopen(req, timeout=MAIL_TIMEOUT, context=ssl_context()) as resp:
            resp.read()
    except urllib.error.HTTPError as e:
        try:
            msg = json.loads(e.read().decode("utf-8")).get("message", str(e))
        except (ValueError, AttributeError):
            msg = str(e)
        raise MailError(msg)
    except (urllib.error.URLError, OSError, ValueError) as e:
        raise MailError(str(e))
