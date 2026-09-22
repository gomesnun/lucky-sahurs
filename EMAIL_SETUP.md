# Configurar o envio de emails (código de verificação) — sem cartão, sem chaves no GitHub

O jogo manda um código de 6 dígitos por email quando alguém cria conta. Para isso funcionar
sem a chave da Brevo ir dentro do jogo (o que já aconteceu uma vez e não queremos repetir),
usamos um pequeno "relay" gratuito no Google Apps Script: só ele conhece a chave da Brevo,
o jogo (em qualquer PC, teu ou de amigos) só fala com ele.

Demora uns 10 minutos, só precisas de uma conta Google. **Sem cartão de crédito.**

## 1. Cria o Web App no Apps Script

1. Vai a **script.google.com** (com a tua conta Google normal) → **Novo projeto**.
2. Dá-lhe um nome, por exemplo "Lucky Verities Mailer" (canto superior esquerdo, onde diz
   "Projeto sem título").
3. Apaga o conteúdo do ficheiro `Code.gs` que já lá está e cola lá dentro **todo** o conteúdo
   do ficheiro `backend/apps_script/Code.gs` deste projeto.
4. Grava (ícone da disquete ou `Ctrl+S`).

## 2. Guarda a chave da Brevo em segredo (nunca no código)

1. No menu da esquerda, clica na **roda dentada** ("Definições do projeto" / "Project Settings").
2. Desce até **"Propriedades do script" / "Script properties"** → **Adicionar propriedade
   do script**.
3. Nome (Property): `BREVO_API_KEY`
   Valor (Value): a tua chave nova da Brevo (Brevo → Settings → SMTP & API → API Keys).
   *(Se a chave antiga ainda estiver ativa nalgum lado, revoga-a lá e cria uma nova — a que
   vazou já foi desativada pela própria Brevo.)*
4. Grava.

Esta chave fica só aqui dentro, na tua conta Google. Nunca aparece em código nenhum.

## 3. Publica como Web App

1. Canto superior direito → **Implementar / Deploy** → **Nova implementação / New deployment**.
2. No ícone da roda dentada ao lado de "Selecionar tipo", escolhe **App da Web / Web app**.
3. Configura:
   - **Executar como / Execute as:** A tua conta (Me / Eu)
   - **Quem tem acesso / Who has access:** Qualquer pessoa / Anyone
     *(Isto não é um risco de segurança — o script não faz mais nada além de mandar o
     código, e tem um limite diário embutido no `Code.gs` para não ser abusado.)*
4. **Implementar / Deploy**. Pode pedir para autorizares o script (é a tua própria conta a
   pedir permissão para mandar pedidos de rede — aceita).
5. Copia o **URL do Web App** que aparece (algo como
   `https://script.google.com/macros/s/AKfycb.../exec`).

## 4. Liga o jogo ao relay

Abre `online/mailer.py` e cola o URL em `GAS_ENDPOINT`:

```python
GAS_ENDPOINT = "https://script.google.com/macros/s/AKfycb.../exec"
```

Este URL **não é secreto** — pode ir para o GitHub sem problema, é só um endereço, não uma
chave. A chave continua só no Apps Script (passo 2).

## 5. Testa

Antes de ligar ao jogo, confirma que o relay está mesmo a funcionar. No terminal:

```bash
curl -X POST "COLA_AQUI_O_TEU_URL" -H "Content-Type: application/json" \
     -d "{\"email\":\"o-teu-email@exemplo.com\",\"code\":\"123456\"}"
```

Deve responder `{"ok":true}` e chegar-te um email a sério com o código 123456. Se dizer
`{"ok":false,"error":"..."}`, a mensagem de erro diz o que falta (chave errada, script não
publicado, etc.).

## E se eu quiser mudar o limite diário?

Abre o `Code.gs` no script.google.com, muda o número em `MAX_PER_DAY` (linha perto do topo),
grava, e faz **Implementar → Gerir implementações → editar (lápis) → Nova versão → Implementar**
outra vez (editar o código sozinho não atualiza a versão publicada).

## Resumo do que fica onde

| Onde                                         | O quê                          | Vai para o GitHub? |
|-----------------------------------------------|---------------------------------|---------------------|
| `online/mailer.py` (`GAS_ENDPOINT`)           | URL do relay                    | Sim, sem problema   |
| `backend/apps_script/Code.gs`                 | Código do relay                 | Sim, sem problema   |
| Script Properties do Apps Script (na Google)  | `BREVO_API_KEY` (a chave a sério)| **Nunca**          |
