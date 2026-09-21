# Lucky Sahurs

Jogo feito em Python + pygame. Funciona em **Windows, Linux e macOS**.

## Jogar (qualquer sistema, sem criar executável)

Precisas do Python 3 e do pygame:

- **Windows:** `pip install pygame-ce` e depois `python main.py`
- **Linux / macOS:** abre um terminal nesta pasta e corre `./run_linux_mac.sh`
  (na 1.ª vez instala o pygame-ce sozinho, num `.venv` dentro da pasta). No Ubuntu/Debian pode ser preciso
  `sudo apt install python3-venv`. Se der "permissão negada": `chmod +x run_linux_mac.sh build_linux_mac.sh`

## Criar o executável (para mandar aos amigos)

O PyInstaller só cria o executável do sistema onde corre — não dá para fazer o `.exe` num Mac, nem o contrário.

| Sistema | Como | Resultado |
|---|---|---|
| Windows | duplo clique em `build_exe.bat` | `dist\Lucky Sahurs.exe` (com o ícone do Tung) |
| macOS   | `./build_linux_mac.sh` | `dist/Lucky Sahurs.app` (com o ícone do Tung) |
| Linux   | `./build_linux_mac.sh` | `dist/Lucky Sahurs` |

macOS: ao abrir uma app não assinada, o sistema pode avisar que o programador não é verificado
(botão direito na app → Abrir). Uma app feita num Mac Apple Silicon pode não abrir em Macs Intel (e vice-versa);
nesse caso o amigo usa `./run_linux_mac.sh`.

## Executáveis automáticos no GitHub (Windows, Linux e macOS)

O ficheiro `.github/workflows/build.yml` cria os executáveis todos sozinho, nos servidores do GitHub:

1. Envia esta pasta para um repositório **público** do GitHub (com a pasta `.github` e o `.gitignore`).
2. Cria uma versão: `git tag v1.0.0` e `git push origin v1.0.0` (ou GitHub → Releases → Draft a new release).
3. Em 5-10 minutos aparecem na página **Releases** os ficheiros para Windows (`.exe`), Linux (`.tar.gz`) e macOS (`.zip`, Apple Silicon e Intel).
4. Os amigos descarregam o do sistema deles. Só isso.

Se o repositório do código for **privado**, podes publicar as Releases noutro repositório público (só com os
executáveis) para os amigos descarregarem sem conta: ver as instruções no topo de `.github/workflows/build.yml`
(variável `RELEASES_REPO` e segredo `RELEASES_TOKEN`). Em repositórios privados a quota grátis é de 2000 minutos
por mês (macOS conta 10x, Windows 2x); um build completo gasta cerca de 120.

Para só testar o build sem publicar: separador **Actions** → *Build Lucky Sahurs* → *Run workflow*.

Avisos normais de programas não assinados: no Windows, "Mais informações → Executar mesmo assim"; no macOS, botão direito
na app → Abrir (ou `xattr -dr com.apple.quarantine "Lucky Sahurs.app"`); no Linux, `tar xzf Lucky-Sahurs-Linux.tar.gz` e `./LuckySahurs`.

## Atualizações automáticas

Os executáveis criados pelo GitHub Actions verificam sozinhos se há uma versão nova:

- Ao abrir o jogo (e de 30 em 30 minutos) ele pergunta ao GitHub qual é a última Release (`UPDATE_REPO` em `config.py`).
- Se o número for **maior** que o do jogo, aparece o ecrã **Nova versão disponível** com **Atualizar agora** (descarrega,
  troca o programa e reabre sozinho) e **Não (fecha o jogo)**. O jogo tem de estar sempre atualizado.
- **Nunca bloqueia sem certeza:** sem internet, GitHub em baixo, ou Release ainda sem o ficheiro do teu sistema → o jogo abre normalmente.
- O número da versão vem da **tag** (`v1.0.1`): o workflow escreve-o dentro do jogo (`build_version.py`, que não vai para o
  repositório). **Usa sempre tags no formato `vX.Y.Z`** (v1.0.1, v1.1.0, v2.0.0...), cada uma MAIOR que a anterior.
  Tags com outro formato (`v1.0.0-rc1`, `final`...) geram um programa "dev" que não verifica atualizações.
- `python main.py`, o `build_exe.bat` (build local) e os "Run workflow" de teste **nunca pedem atualização** (versão "dev").
- Os saves ficam em `%APPDATA%\LuckySahurs` (ou o equivalente), por isso uma atualização não mexe neles.
- Para desligar (testes): variável de ambiente `LUCKY_SAHURS_NO_UPDATE=1`.

Como a atualização é obrigatória, **testa o executável antes de publicar**: corre *Actions → Build Lucky Sahurs → Run workflow*,
descarrega o ficheiro de "Artifacts", experimenta, e só depois cria a tag. Uma Release estragada seria imposta a todos.

## Idioma

**Options → Language** alterna entre English e Português e a mudança é imediata (fica guardada nas definições).
Os textos estão em `i18n_pt.py` (dicionário "texto em inglês" → "tradução"). Para acrescentar outro idioma, ver
as instruções no topo de `i18n.py`. Texto novo que ainda não esteja traduzido aparece em inglês (nunca dá erro).

## Onde ficam os saves

Sempre na pasta de dados do sistema (nunca ao lado do jogo):

- Windows: `%APPDATA%\LuckySahurs` (Win+R e escrever isso)
- macOS: `~/Library/Application Support/LuckySahurs`
- Linux: `~/.local/share/LuckySahurs`

Lá dentro ficam os saves (`savegame_slot*.json`), as definições, a sessão da conta e a cópia da cloud.
Quem tinha saves ao lado do jogo (versões antigas) ganha uma cópia automática na primeira vez; os ficheiros
antigos não são apagados (podes apagá-los depois de confirmares que está tudo bem).
Para usar outra pasta (testes, pen drive) define a variável de ambiente `LUCKY_SAHURS_SAVE_DIR`.

## Ecrã inteiro

`F11` (no macOS também `Cmd+F`, porque o F11 costuma estar ocupado pelo sistema). `Ctrl+F` também funciona.

## O que está nesta pasta

`main.py`, `config.py`, `storage.py`, `theme.py`, `i18n.py`, `i18n_pt.py` · `core/` (regras) · `ui/` (ecrãs) ·
`online/` (contas e cloud) · `icons/` (imagens; `tung.ico` / `tung.icns` são os ícones do executável) ·
`sounds/` (sons e música, com `CREDITS.txt`) · scripts de build.
 
