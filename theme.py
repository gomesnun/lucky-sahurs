"""Cores e espessuras do estilo "cartoon" usado em toda a interface."""


WHITE = (255, 255, 255)
BLACK = (12, 12, 14)
# --- estilo "cartoon": fundo creme, painéis escuros com contorno preto grosso ---
BG_TOP = (246, 243, 236)
BG_BOTTOM = (226, 222, 212)
OUTLINE = (8, 8, 10)           # contorno de painéis, botões e texto
PANEL = (34, 34, 37)
PANEL_LIGHT = (52, 52, 57)
PANEL_LIGHTER = (76, 76, 83)
ACCENT = (255, 184, 48)        # dourado (troca aqui a cor de destaque do jogo todo)
ACCENT_HOVER = (255, 212, 110)
GREY = (200, 200, 208)
GREY_DIM = (152, 152, 160)
BORDER_W = 4                   # espessura do contorno dos painéis grandes
BORDER_W_SMALL = 3             # botões, cartões e linhas
GOOD = (100, 225, 130)
BAD = (235, 95, 95)
GOLD_BORDER = (255, 205, 60)
DIAMOND_BORDER = (150, 235, 255)
PAUSED_RED = (225, 45, 45)     # contorno das barras Golden/Diamond/Rainbow Roll quando estão pausadas

# a primeira que existir no PC é usada: Windows (segoeui, verdana, arial), macOS (helveticaneue, helvetica, arial)
# e Linux (liberationsans, dejavusans, notosans...). Se nenhuma existir, o pygame usa a fonte que traz consigo.
FONT_NAMES = "segoeui,verdana,arial,helveticaneue,helvetica,liberationsans,dejavusans,notosans,freesans"
