"""Contexto HTTPS com os certificados do pacote 'certifi' (vão dentro do executável / do APK).

Porquê: um executável feito com o PyInstaller nem sempre encontra os certificados do sistema (no macOS e em
algumas distribuições de Linux dá "CERTIFICATE_VERIFY_FAILED" e o online / as atualizações não funcionam).
No Android é pior: o Python do python-for-android não traz CA nenhum, por isso sem o certifi TUDO o que é
online falha. Se o certifi não estiver disponível, tenta-se a loja de certificados do próprio Android e só
depois os do sistema. A verificação nunca é desligada."""

import os
import ssl

# onde o Android guarda os certificados das autoridades (ficheiros com nome <hash>.0, como o OpenSSL quer)
ANDROID_CA_DIR = "/system/etc/security/cacerts"

_ctx = None


def ssl_context():
    global _ctx
    if _ctx is None:
        _ctx = _build_context()
    return _ctx


def _build_context():
    try:
        import certifi
        return ssl.create_default_context(cafile=certifi.where())
    except Exception:
        pass
    if os.path.isdir(ANDROID_CA_DIR):
        try:
            ctx = ssl.create_default_context()
            ctx.load_verify_locations(capath=ANDROID_CA_DIR)
            return ctx
        except Exception:
            pass
    return ssl.create_default_context()
