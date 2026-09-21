"""Contexto HTTPS com os certificados do pacote 'certifi' (vão dentro do executável).

Porquê: um executável feito com o PyInstaller nem sempre encontra os certificados do sistema (no macOS e em
algumas distribuições de Linux dá "CERTIFICATE_VERIFY_FAILED" e o online / as atualizações não funcionam).
Com o certifi funciona em todo o lado. Se o certifi não estiver instalado, usa os certificados do sistema."""

import ssl

_ctx = None


def ssl_context():
    global _ctx
    if _ctx is None:
        try:
            import certifi
            _ctx = ssl.create_default_context(cafile=certifi.where())
        except Exception:
            _ctx = ssl.create_default_context()
    return _ctx
