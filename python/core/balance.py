"""BALANCE: todos os números que decidem a velocidade a que o jogo avança, num só sítio.

Para o jogo ficar mais fácil / difícil, muda só os valores aqui (os textos das upgrades, as milestones, as
traits e a página de Rebirth já leem daqui, por isso mostram sempre os números certos).
Nos ficheiros do core faz-se "from core import balance as B" e usa-se B.NOME, por isso mexer aqui chega.
"""

# ---------------------------------------------------------------- upgrades (efeito de cada nível)
LUCK_PER_LEVEL = 0.08            # Luck: +x% de peso em pets Raros ou melhores
LUCK_PRISM_PER_LEVEL = 0.07      # Prismatic Luck (acumula 3 vezes: Epic+, Mythic+ e Secret+)
LUCK_COSMIC_PER_LEVEL = 0.16     # Exotic Luck
LUCK_DIVINE_PER_LEVEL = 0.25     # Divine Luck
MONEY_PER_LEVEL = 0.07           # Money: +x% dinheiro/seg
MONEY_PRISM_PER_LEVEL = 0.16     # Superior Money: +x% dinheiro/seg (multiplica com o Money normal)
INCOME_SCALE = 0.15              # multiplica TODO o dinheiro/seg dos pets (o botão mais direto para "ganhar dinheiro é fácil demais")
UPGRADE_COST_GROWTH = 1.02       # todas as upgrades encarecem isto a mais por nível (1.0 = sem extra)

# ---------------------------------------------------------------- Auto Roller
AUTO_UNLOCK_REBIRTHS = 2         # o Auto Roller só se pode comprar a partir deste nº de rebirths
AUTO_BASE_RPS = 1.0              # rolls/seg quando compras o Auto Roller (sem traits, metas nem rebirths)
AUTO_SPEED_PER_LEVEL = 0.22      # Auto Speed: +x% de velocidade por nível
AUTO_TURBO_PER_LEVEL = 0.28      # Auto Turbo: +x% de velocidade por nível (multiplica)

# ---------------------------------------------------------------- Golden Roll (o ciclo que já vem desde o início)
GOLDEN_ROLL_EVERY = 14           # de quantos em quantos rolls a próxima roll vem com sorte extra
GOLDEN_ROLL_MIN = 10             # mínimo de rolls no ciclo (depois das upgrades Short Cycle)
GOLDEN_ROLL_MULT = 4.0           # multiplicador de sorte dessa roll

# ---------------------------------------------------------------- traits
TRAIT_CHARGE_ONE_IN = 250        # 1 carga de trait em média a cada X rolls
TRAIT_BUFF_SCALE = {"money": 0.6, "luck": 0.6, "auto_speed": 1.0}    # multiplica os buffs das traits

# ---------------------------------------------------------------- milestones
MILESTONE_SCALE = {"money": 0.6, "luck": 0.6, "auto_speed": 1.0}     # multiplica as recompensas (por tipo)

# ---------------------------------------------------------------- rebirths
REBIRTH_BASE_COST = 500000
REBIRTH_COST_MULT = 3.2
REBIRTH_MONEY_PER = 0.07         # +x% dinheiro/seg por rebirth
REBIRTH_LUCK_PER = 0.04          # +x% de peso em pets Raros ou melhores por rebirth
REBIRTH_REWARD_SCALE = {"money": 0.6, "luck": 0.6, "auto_speed": 1.0}    # multiplica as recompensas da lista

# ---------------------------------------------------------------- anti auto-clicker
MAX_MANUAL_CPS = 20              # máximo de rolls por clique no botão ROLL (20 cliques/seg = 1 a cada 0.05 s)
