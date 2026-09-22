"""Instalar um APK novo no Android (a parte do online/updater.py que só existe no telemóvel).

No computador o jogo troca-se a si próprio e volta a abrir. No Android isso é proibido: quem instala
aplicações é o sistema. O que dá para fazer é:

  1. descarregar o APK novo (isso é o updater.py, igual a todos os sistemas);
  2. pôr o ficheiro num sítio que o instalador do Android consiga ler (a pasta Transferências, através
     do MediaStore; em Android 9 e anteriores, a pasta Transferências normal);
  3. abrir o instalador do sistema a apontar para esse ficheiro.

O utilizador ainda tem de carregar em "Instalar" (o Android nunca deixa instalar sem ele saber) e, da
primeira vez, tem de dar autorização a este jogo para instalar aplicações - e é por isso que existe o
ensure_can_install(): abre a página dessa autorização quando ainda não foi dada.

Tudo isto usa o pyjnius (chamar Java a partir do Python). Se faltar alguma coisa, levanta InstallError e
quem chama abre a página da Release no browser, que é o caminho manual e funciona sempre.
"""

import os

from config import GAME_TITLE

APK_MIME = "application/vnd.android.package-archive"


class InstallError(Exception):
    """code: "permission" (falta autorizar a instalação) | "other"."""

    def __init__(self, code, detail=""):
        Exception.__init__(self, "%s: %s" % (code, detail))
        self.code = code
        self.detail = detail


def _jnius():
    try:
        from jnius import autoclass, cast
    except ImportError as e:        # APK feito sem o pyjnius
        raise InstallError("other", str(e))
    return autoclass, cast


def _activity(autoclass):
    for name in ("org.kivy.android.PythonActivity", "org.renpy.android.PythonActivity"):
        try:
            act = autoclass(name).mActivity
        except Exception:
            continue
        if act is not None:
            return act
    raise InstallError("other", "sem PythonActivity")


def _sdk_int(autoclass):
    return autoclass("android.os.Build$VERSION").SDK_INT


def ensure_can_install():
    """True se o jogo já pode instalar aplicações. Se ainda não pode, abre a página onde se autoriza
    ("Instalar apps desconhecidas") e devolve False - a seguir o utilizador volta ao jogo e tenta outra vez."""
    autoclass, _cast = _jnius()
    activity = _activity(autoclass)
    if _sdk_int(autoclass) < 26:
        return True             # antes do Android 8 a autorização era geral, não por aplicação
    try:
        if activity.getPackageManager().canRequestPackageInstalls():
            return True
    except Exception as e:
        raise InstallError("other", str(e))
    Intent = autoclass("android.content.Intent")
    Settings = autoclass("android.provider.Settings")
    Uri = autoclass("android.net.Uri")
    intent = Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    Uri.parse("package:" + activity.getPackageName()))
    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try:
        activity.startActivity(intent)
    except Exception as e:
        raise InstallError("other", str(e))
    return False


def _publish_mediastore(autoclass, cast, activity, path, name):
    """Copia o APK para a pasta Transferências pelo MediaStore (Android 10+) e devolve o content:// dele."""
    ContentValues = autoclass("android.content.ContentValues")
    MediaStore = autoclass("android.provider.MediaStore$Downloads")
    MediaColumns = autoclass("android.provider.MediaStore$MediaColumns")
    resolver = activity.getContentResolver()
    collection = MediaStore.EXTERNAL_CONTENT_URI

    # Uma atualização anterior deixou um ficheiro com este nome: sem isto o Android guardava
    # "Lucky-Sahurs-Android (1).apk" e a pasta enchia-se de cópias.
    try:
        resolver.delete(collection, MediaColumns.DISPLAY_NAME + "=?", [name])
    except Exception:
        pass

    values = ContentValues()
    values.put(MediaColumns.DISPLAY_NAME, name)
    values.put(MediaColumns.MIME_TYPE, APK_MIME)
    values.put(MediaColumns.IS_PENDING, 1)      # "ainda a escrever": ninguém lê o ficheiro a meio
    uri = resolver.insert(collection, values)
    if uri is None:
        raise InstallError("other", "o MediaStore recusou o ficheiro")
    try:
        stream = resolver.openOutputStream(uri)
        try:
            with open(path, "rb") as f:
                while True:
                    chunk = f.read(1024 * 1024)
                    if not chunk:
                        break
                    stream.write(chunk)
        finally:
            stream.close()
        done = ContentValues()
        done.put(MediaColumns.IS_PENDING, 0)
        resolver.update(uri, done, None, None)
    except BaseException:
        try:
            resolver.delete(uri, None, None)
        except Exception:
            pass
        raise
    return uri


def _publish_file(autoclass, path, name):
    """Android 9 e anteriores: o APK vai para a pasta Transferências e abre-se por file://."""
    Environment = autoclass("android.os.Environment")
    File = autoclass("java.io.File")
    Uri = autoclass("android.net.Uri")
    folder = Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS)
    folder.mkdirs()
    dest = File(folder, name)
    with open(path, "rb") as src, open(dest.getAbsolutePath(), "wb") as dst:
        while True:
            chunk = src.read(1024 * 1024)
            if not chunk:
                break
            dst.write(chunk)
    # Nestas versões o Android proíbe passar um file:// a outra aplicação a não ser que se desligue
    # esta verificação (o FileProvider, que é a maneira normal, precisa de mexer no AndroidManifest,
    # que o buildozer não deixa acrescentar dentro de <application>).
    try:
        StrictMode = autoclass("android.os.StrictMode")
        builder = autoclass("android.os.StrictMode$VmPolicy$Builder")()
        StrictMode.setVmPolicy(builder.build())
    except Exception:
        pass
    return Uri.fromFile(dest)


def install_apk(path):
    """Abre o instalador do Android com o APK que está em 'path'. Levanta InstallError."""
    autoclass, cast = _jnius()
    activity = _activity(autoclass)
    name = os.path.basename(path) or (GAME_TITLE.replace(" ", "-") + ".apk")
    try:
        if _sdk_int(autoclass) >= 29:
            uri = _publish_mediastore(autoclass, cast, activity, path, name)
        else:
            uri = _publish_file(autoclass, path, name)
    except InstallError:
        raise
    except Exception as e:
        raise InstallError("other", str(e))

    Intent = autoclass("android.content.Intent")
    intent = Intent(Intent.ACTION_VIEW)
    intent.setDataAndType(uri, APK_MIME)
    intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    try:
        activity.startActivity(intent)
    except Exception as e:
        raise InstallError("other", str(e))
