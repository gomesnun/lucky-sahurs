"""Idiomas do jogo (English / Português) — tudo o que muda de língua passa por aqui.

COMO FUNCIONA
-------------
No código, todos os textos visíveis continuam a ser escritos em INGLÊS e passam por tr():

    tr("Save now")                       ->  "Guardar agora" (em Português) / "Save now" (em English)
    tr("Slot %d deleted.", slot)         ->  a tradução JÁ com o número metido (usa %, como o resto do jogo)

A tradução procura o texto inglês no dicionário do idioma atual (i18n_pt.py). Se não encontrar
(texto novo que ainda não foi traduzido), mostra o inglês — nunca dá erro nem fica em branco.

TEXTOS COM NÚMEROS (dados do jogo, ex.: descrições das upgrades)
----------------------------------------------------------------
L("+%g%% chance ...", 0.25) devolve o texto inglês já formatado (é uma str normal, dá para usar em
qualquer lado) mas que "sabe" o seu molde, por isso tr() consegue traduzi-lo com os mesmos números.

PARA ACRESCENTAR UM IDIOMA
--------------------------
1. Cria i18n_xx.py com um dicionário  XX = {"texto em inglês": "tradução", ...}  (copia o i18n_pt.py).
2. Acrescenta-o a LANGUAGES e a _CATALOGS aqui em baixo.  O botão "Language" das Options passa a
   percorrer todos os idiomas por ordem.
Regra: a tradução tem de ter os MESMOS marcadores (%d, %s, %g...) e pela mesma ordem que o original.
"""

from i18n_pt import PT, PT_SHORT

DEFAULT_LANGUAGE = "en"

# (código, nome mostrado — sempre no próprio idioma, para se reconhecer mesmo que não se perceba o resto)
LANGUAGES = (("en", "English"), ("pt", "Português"))
LANGUAGE_CODES = tuple(code for code, _name in LANGUAGES)

_CATALOGS = {"pt": PT}
_SHORT = {"pt": PT_SHORT}       # formas curtas, só para sítios apertados (ver tr_short)
_current = DEFAULT_LANGUAGE


class TStr(str):
    """Texto inglês já formatado que também guarda o molde e os números (ver L())."""

    def __new__(cls, template, args=()):
        obj = super().__new__(cls, (template % args) if args else template)
        obj.template = template
        obj.args = tuple(args)
        return obj


def L(template, *args):
    """Como '%' mas o resultado sabe traduzir-se: L("Lv %d", 3) -> str "Lv 3" (que tr() traduz)."""
    return TStr(template, args)


def set_language(code):
    """Muda o idioma atual. Códigos desconhecidos voltam ao inglês."""
    global _current
    _current = code if code in LANGUAGE_CODES else DEFAULT_LANGUAGE
    return _current


def get_language():
    return _current


def language_name(code=None):
    code = code or _current
    for c, name in LANGUAGES:
        if c == code:
            return name
    return code


def next_language(code=None):
    """O idioma a seguir ao dado (ao atual, por omissão), a rodar pela lista LANGUAGES."""
    code = code or _current
    codes = list(LANGUAGE_CODES)
    if code not in codes:
        return codes[0]
    return codes[(codes.index(code) + 1) % len(codes)]


def tr_short(text, *args):
    """Como tr(), mas usa a forma CURTA (se o idioma tiver uma) em sítios apertados, como as abas do ranking.
    Sem forma curta, é igual a tr()."""
    if _current != DEFAULT_LANGUAGE:
        curta = _SHORT.get(_current, {}).get(text)
        if curta is not None:
            return curta % args if args else curta
    return tr(text, *args)


def tr(text, *args):
    """Traduz 'text' (em inglês) para o idioma atual e, se houver 'args', mete-os no texto com '%'.
    Nunca falha: sem tradução usa o inglês; se a tradução tiver marcadores errados, também."""
    template, targs = text, args
    if isinstance(text, TStr) and not args:
        template, targs = text.template, text.args
    if _current != DEFAULT_LANGUAGE:
        template = _CATALOGS.get(_current, {}).get(template, template)
    if not targs:
        return template
    try:
        return template % targs
    except (TypeError, ValueError):
        pass
    try:                                  # tradução estragada: usa o inglês
        return str(text) if isinstance(text, TStr) else (text % args)
    except (TypeError, ValueError):
        return str(text)
