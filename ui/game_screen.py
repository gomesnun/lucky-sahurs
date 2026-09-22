"""Ecrã principal: barra de topo, botões laterais, carta principal e Stats."""

import math
import time
import pygame

from config import HINT_H, TOPBAR_H, VIRTUAL_H
from core.formatting import format_number, format_one_in, format_playtime
from core.milestones import MILESTONE_DEFS
from core.pets import INDEX_ENTRIES, RARITIES
from i18n import tr
from theme import (
    BAD, BORDER_W, BORDER_W_SMALL, DIAMOND_BORDER,
    GOLD_BORDER, GOOD, GREY, OUTLINE,
    PANEL, PANEL_LIGHT, PANEL_LIGHTER, PAUSED_RED, WHITE,
)
from ui.drawing import (bake, bake_scene, dim_overlay,
    bar_fill_surface, draw_panel, draw_rainbow_border, draw_state_border,
    rainbow_glow_surface, rarity_glow,
)
from ui.fonts import is_light
from ui.icons import load_icon


class GameScreenMixin:
    """Ecrã principal: barra de topo, botões laterais, carta principal e Stats."""

    def open_stats(self):
        if self.options_open:
            self.close_options()
        self.traits_open = False
        self.rebirth_open = False
        self.rebirth_confirm = False
        self.leaderboard_open = False
        self.credits_open = False
        self.stats_open = True

    def close_stats(self):
        self.stats_open = False

    def toggle_stats(self):
        if self.stats_open:
            self.close_stats()
        else:
            self.open_stats()

    # ---------------------------------------------------------------- desenho
    def main_center_x(self):
        """Centro da zona livre entre os painéis (o conteúdo acompanha o slide)."""
        left = self.left_panel.shown_width(self.left_w) if self.left_panel.visible else 0
        right = self.vw - (self.right_panel.shown_width(self.right_w) if self.right_panel.visible else 0)
        return (left + right) // 2

    def draw_game_screen(self, mouse_pos):
        self.draw_topbar(mouse_pos)
        self.draw_main(mouse_pos)

        if self.animations:
            for p in self.particles:
                p.draw(self.canvas)

        # painéis por cima (entram a deslizar)
        self.right_rect = pygame.Rect(0, 0, 0, 0)
        self.left_rect = pygame.Rect(0, 0, 0, 0)

        panel_top = TOPBAR_H
        panel_h = VIRTUAL_H - TOPBAR_H - HINT_H

        if self.left_panel.visible:
            shown = self.left_panel.shown_width(self.left_w)
            rect = pygame.Rect(shown - self.left_w, panel_top, self.left_w, panel_h)
            self.left_rect = rect
            self.draw_bag_panel(rect, mouse_pos)

        if self.right_panel.visible:
            shown = self.right_panel.shown_width(self.right_w)
            rect = pygame.Rect(self.vw - shown, panel_top, self.right_w, panel_h)
            self.right_rect = rect
            if self.right_panel.content == "tree":
                self.draw_tree_panel(rect, mouse_pos)
            elif self.right_panel.content == "milestones":
                self.draw_milestones_panel(rect, mouse_pos)
            elif self.right_panel.content == "daily":
                self.draw_daily_panel(rect, mouse_pos)
            else:
                self.draw_index_panel(rect, mouse_pos)

        # botões laterais: desenhados antes das páginas por cima (Traits / Rebirth / Stats / Options),
        # por isso ficam escurecidos por elas, mas continuam clicáveis
        self.draw_side_buttons(mouse_pos)

        if self.traits_open:
            self.begin_modal()
            self.draw_traits_page(mouse_pos)
        if self.rebirth_open:
            self.begin_modal()
            self.draw_rebirth_page(mouse_pos)
        self.draw_toast()

    # ---------------------------------------------------------------- topbar
    def draw_topbar(self, mouse_pos):
        rect = pygame.Rect(0, 0, self.vw, TOPBAR_H)
        pygame.draw.rect(self.canvas, PANEL, rect)
        pygame.draw.line(self.canvas, OUTLINE, (0, TOPBAR_H), (self.vw, TOPBAR_H), 2 * BORDER_W - 3)

        cash = load_icon("cash", 54)              # icons/cash.png (opcional): pilha de dinheiro antes do valor
        off = 0
        if cash is not None:
            # a topbar e um rectangulo liso de PANEL: tudo o que la vai por cima pode ser
            # misturado com essa cor uma vez e depois copiado sem alfa (ver bake() em ui/drawing.py)
            self.canvas.blit(bake(cash, PANEL), (12, (TOPBAR_H - 54) // 2))
            off = 58
        coins_txt = self.font_big.render("$ %s" % format_number(self.state.coins), True, GOOD)
        self.canvas.blit(bake(coins_txt, PANEL), (22 + off, 11))
        dps_txt = self.font_small.render(tr("%s / sec", format_number(self.state.income_per_second())), True, GREY)
        self.canvas.blit(bake(dps_txt, PANEL), (25 + off, 44))
        pets_txt = self.font_small.render(tr("·   %d / %d pets equipped",
                                             len(self.state.equipped), self.state.max_slots()), True, GREY)
        self.canvas.blit(bake(pets_txt, PANEL), (25 + off + dps_txt.get_width() + 14, 44))

        self.nav_mode = True      # Friends / Stats / Options: clicáveis com qualquer página aberta
        friends_rect = pygame.Rect(self.vw - 472, 15, 146, 40)
        self.button(friends_rect, tr("Friends"), self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.toggle_friends, icon="friends")
        pedidos = self.friends_pending_count()
        if pedidos:
            self.draw_friends_badge(friends_rect.topright, pedidos)

        stats_rect = pygame.Rect(self.vw - 320, 15, 146, 40)
        self.button(stats_rect, tr("Stats"), self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.toggle_stats, icon="stats")

        opt_rect = pygame.Rect(self.vw - 168, 15, 146, 40)
        self.button(opt_rect, tr("Options"), self.font_med, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.toggle_options, icon="options")
        self.nav_mode = False
        self.draw_event_banner()        # evento global a decorrer (ver ui/events_panel.py)

    def draw_stats(self, mouse_pos):
        # Só os botões de navegação (Stats/Options no topo, Bag/Traits/Index/Tree/Milestones
        # ao lado) continuam clicáveis por baixo; o resto fica bloqueado (ver begin_modal).
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))

        st = self.state
        total_ms = sum(len(d["tiers"]) for d in MILESTONE_DEFS.values())
        lines = [
            (tr("Save"), tr("Slot %d", st.slot or 0)),
            (tr("Coins"), "$" + format_number(st.coins)),
            (tr("Income"), tr("%s / sec", format_number(st.income_per_second()))),
            (tr("Total Coins Earned"), "$" + format_number(st.total_coins_earned)),
            (tr("Total Rolls"), format_number(st.total_rolls)),
            (tr("Equipped Slots"), "%d/%d" % (len(st.equipped), st.max_slots())),
            (tr("Playtime"), format_playtime(st.playtime)),
            (tr("Indexed Pets"), "%d/%d" % (st.indexed_pets_count(), INDEX_ENTRIES)),
            (tr("Traits Rolled"), format_number(st.total_traits_rolled)),
            (tr("Rebirths"), format_number(st.rebirths)),
            (tr("Milestones Claimed"), "%d/%d" % (len(st.milestones_claimed), total_ms)),
        ]
        row_h = 34
        panel_w = 440
        panel_h = 84 + row_h * len(lines) + 16
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, VIRTUAL_H // 2 - panel_h // 2, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        title = self.font_big.render(tr("Stats"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 24, rect.y + 20))
        close_rect = pygame.Rect(rect.right - 46, rect.y + 20, 28, 28)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_stats, radius=8)

        y = rect.y + 72
        for label, value in lines:
            ltxt = self.font_med.render(label, True, GREY)
            vtxt = self.font_med.render(value, True, WHITE)
            self.canvas.blit(bake(ltxt, PANEL), (rect.x + 24, y))
            self.canvas.blit(bake(vtxt, PANEL), (rect.right - 24 - vtxt.get_width(), y))
            pygame.draw.line(self.canvas, PANEL_LIGHT, (rect.x + 24, y + row_h - 6),
                             (rect.right - 24, y + row_h - 6), 1)
            y += row_h

    def open_right_panel(self, content):
        """Abre um painel do lado direito (Index / Árvore / Metas) e fecha a
        página de Traits / Stats / Options, se estiver aberta."""
        self.close_overlays()
        if content == "milestones":
            self.milestones_selected_category = None
        elif content == "tree":
            self.tree_selected_category = None
        self.right_panel.toggle(content)

    def open_left_panel(self, content):
        self.close_overlays()
        self.left_panel.toggle(content)

    def draw_side_buttons(self, mouse_pos):
        size = 62
        # O texto de cada botão vai POR BAIXO do ícone (ver side_button), por isso cada botão precisa de
        # umas dezenas de pixéis de altura extra. Assim, com um painel aberto de cada lado, os textos
        # (Upgrades, Milestones, Rebirth...) já não ficam em cima do Auto Roller nem uns colados aos outros.
        step = size + 38
        lbl_index, lbl_upgrades, lbl_milestones, lbl_daily = tr("INDEX"), tr("UPGRADES"), tr("MILESTONES"), tr("DAILY")
        lbl_bag, lbl_rebirth, lbl_traits = tr("BAG"), tr("REBIRTH"), tr("TRAITS")
        labels = (lbl_index, lbl_upgrades, lbl_milestones, lbl_daily, lbl_bag, lbl_rebirth, lbl_traits)
        lf = self.font_small_b                       # se o texto mais comprido não couber na largura da coluna,
        if max(lf.render(t, True, WHITE).get_width() for t in labels) > size + 24:   # usa a fonte mais pequena
            lf = self.font_tiny_b                    # (a mesma em todos, para ficarem iguais)
        self.nav_mode = True     # sempre clicáveis, mesmo com Traits / Rebirth / Stats / Options abertos
        # --- direita: index + árvore + metas + missões diárias (acompanham o painel quando ele abre) ---
        shown_r = self.right_panel.shown_width(self.right_w) if self.right_panel.visible else 0
        x = self.vw - shown_r - size - 18
        # cada coluna fica centrada na vertical (no meio do ecrã, como o cartão): altura do bloco = botões + o texto por baixo
        label_h = 26

        def column_top(n_buttons):
            return VIRTUAL_H // 2 - ((n_buttons - 1) * step + size + label_h) // 2

        y = column_top(4)
        self.side_button(pygame.Rect(x, y, size, size), lbl_index, "index", mouse_pos,
                         self.right_panel.is_open and self.right_panel.content == "index",
                         lambda: self.open_right_panel("index"), lf)
        self.side_button(pygame.Rect(x, y + step, size, size), lbl_upgrades, "tree", mouse_pos,
                         self.right_panel.is_open and self.right_panel.content == "tree",
                         lambda: self.open_right_panel("tree"), lf,
                         badge=self.state.affordable_upgrades_count())
        self.side_button(pygame.Rect(x, y + 2 * step, size, size), lbl_milestones, "milestones", mouse_pos,
                         self.right_panel.is_open and self.right_panel.content == "milestones",
                         lambda: self.open_right_panel("milestones"), lf)
        self.side_button(pygame.Rect(x, y + 3 * step, size, size), lbl_daily, "daily", mouse_pos,
                         self.right_panel.is_open and self.right_panel.content == "daily",
                         lambda: self.open_right_panel("daily"), lf)

        # --- esquerda: mochila + rebirth + traits (Rebirth e Traits abrem uma página central, não um slide) ---
        shown_l = self.left_panel.shown_width(self.left_w) if self.left_panel.visible else 0
        lx = shown_l + 18
        y = column_top(3)               # a coluna da esquerda tem 3 botões (a da direita tem 4)
        self.side_button(pygame.Rect(lx, y, size, size), lbl_bag, "bag", mouse_pos,
                         self.left_panel.is_open,
                         lambda: self.open_left_panel("bag"), lf)
        # '!!' vermelho no canto do botão quando já dá para fazer um Rebirth
        self.side_button(pygame.Rect(lx, y + step, size, size), lbl_rebirth, "rebirth", mouse_pos,
                         self.rebirth_open,
                         self.toggle_rebirth, lf,
                         alert=self.state.rebirth_available())
        self.side_button(pygame.Rect(lx, y + 2 * step, size, size), lbl_traits, "trait", mouse_pos,
                         self.traits_open,
                         self.toggle_traits, lf)
        self.nav_mode = False

    # ---------------------------------------------------------------- ecrã principal
    def main_card_rect(self):
        """Cartão do último pet: fica EXATAMENTE no centro do ecrã (na vertical, o meio da janela;
        na horizontal, o meio da zona livre entre os painéis, que acompanha o slide)."""
        card_w, card_h = 236, 236          # quase quadrado: não ocupa o ecrã todo
        cx = self.main_center_x()
        cy = VIRTUAL_H // 2
        return pygame.Rect(cx - card_w // 2, cy - card_h // 2, card_w, card_h)

    def active_roll_luck(self):
        """Sorte total dos bónus Golden / Diamond / Rainbow Roll que estão ativos (as sortes somam,
        como no roll()). Conta um bónus quando está pronto ou quando a barra está sempre cheia (roll
        muito rápido) e ele não está pausado. Devolve (sorte_total, bónus_mais_raro_ativo)."""
        st = self.state
        total, top = 0.0, None
        for key, unlocked, ready, mult in (
                ("golden", True, st.cyclic_bonus_ready, st.golden_roll_mult()),
                ("diamond", st.diamond_roll_unlocked(), st.diamond_bonus_ready, st.diamond_roll_mult()),
                ("rainbow", st.rainbow_roll_unlocked(), st.rainbow_bonus_ready, st.rainbow_roll_mult())):
            if not unlocked or st.is_cycle_paused(key):
                continue
            if ready or self.bar_continuous.get(key, False):
                total += mult
                top = key
        return total, top

    def draw_main(self, mouse_pos):
        center_x = self.main_center_x()
        card_rect = self.main_card_rect()
        card_w, card_h = card_rect.size

        # --- último pet rolado, no meio ---
        if self.state.last_roll is not None:
            r_idx, mutation = self.state.last_roll
            rarity = RARITIES[r_idx]
            income = self.state.pet_income(r_idx, mutation)
            chance = self.state.combined_chance(r_idx, mutation)      # chance atual de sair este pet (com esta mutação)
            plates = [[(tr("Income"), tr("+%s/sec", format_number(income)))],
                      [(tr("Roll Chance"), format_one_in(chance))]]
            card_surf = self.render_pet_card(rarity, mutation, card_w, card_h, plates=plates)
            glow_color = tuple(rarity["color"]) if rarity["color"][2] > 60 or is_light(rarity["color"]) else (90, 90, 110)
            glow = rarity_glow((card_w, card_h), glow_color)
            glow_rect = glow.get_rect(center=card_rect.center)
            zoom = None
            if self.animations:
                elapsed = time.time() - self.roll_anim_start
                dur = 0.18
                if elapsed < dur:
                    zoom = 0.72 + 0.28 * (elapsed / dur)      # o cartão cresce um instante ao sair um pet novo
            if zoom is None:
                # O brilho e o cartão são as duas maiores imagens com alfa do ecrã (juntos, ~34 ms por
                # frame no telemóvel). Enquanto o pet não muda são sempre iguais: misturam-se uma vez
                # com o fundo e passam a ser um blit opaco. Durante o crescimento não vale a pena
                # (o tamanho muda a cada frame), e aí faz-se o desenho normal.
                scene = bake_scene(getattr(self, "bg_surface", None), glow_rect,
                                   (id(glow), id(card_surf), card_rect.center),
                                   lambda flat: (flat.blit(glow, (0, 0)),
                                                 flat.blit(card_surf, card_surf.get_rect(
                                                     center=(glow_rect.width // 2, glow_rect.height // 2)))))
                if scene is not None:
                    self.canvas.blit(scene, glow_rect.topleft)
                    card_surf = None
            if card_surf is not None:
                self.canvas.blit(glow, glow_rect)
                if zoom is not None:
                    card_surf = pygame.transform.smoothscale(
                        card_surf, (max(1, int(card_w * zoom)), max(1, int(card_h * zoom))))
                self.canvas.blit(card_surf, card_surf.get_rect(center=card_rect.center))
        else:
            draw_panel(self.canvas, card_rect, PANEL_LIGHT, radius=12)
            txt = self.font_med.render(tr("Click ROLL to start!"), True, GREY)
            self.canvas.blit(bake(txt, PANEL_LIGHT), txt.get_rect(center=card_rect.center))

        # --- Auto Roller: em cima do cartão (com os rolls/seg ainda mais acima) ---
        if self.state.auto_unlocked():
            rps = self.state.auto_rolls_per_second()
            on = self.state.auto_on
            auto_rect = pygame.Rect(center_x - 130, card_rect.top - 24 - 40, 260, 40)

            def toggle_auto():
                self.state.auto_on = not self.state.auto_on
                self.auto_accum = 0.0

            self.button(auto_rect,
                        tr("Auto Roller: %s", tr("ON") if on else tr("OFF")),
                        self.font_small_b, mouse_pos,
                        (52, 120, 80) if on else PANEL_LIGHT, PANEL_LIGHTER,
                        WHITE, callback=toggle_auto, radius=10)
            sub = self.font_small.render(tr("%.2f rolls / sec", rps), True, GREY)
            self.canvas.blit(sub, sub.get_rect(center=(center_x, auto_rect.top - 14)))

        # --- botão ROLL: em baixo do cartão ---
        # A cor do brilho e do contorno é a do bónus MAIS raro que está pronto (e não está pausado):
        # Rainbow (arco-íris) > Diamond (azul) > Golden (dourado). Pausado = não vai disparar, por isso não brilha.
        st = self.state
        bonus_kind = None
        if st.rainbow_roll_unlocked() and st.rainbow_bonus_ready and not st.is_cycle_paused("rainbow"):
            bonus_kind = "rainbow"
        elif st.diamond_roll_unlocked() and st.diamond_bonus_ready and not st.is_cycle_paused("diamond"):
            bonus_kind = "diamond"
        elif st.cyclic_bonus_ready and not st.is_cycle_paused("golden"):
            bonus_kind = "golden"

        roll_rect = pygame.Rect(center_x - 130, card_rect.bottom + 24, 260, 60)
        if bonus_kind:
            pulse = 0.6 + 0.4 * math.sin(time.time() * 6.0)
            # O brilho pulsa: se a transparência mudasse a cada frame, cada mistura seria diferente e
            # nenhuma serviria duas vezes. Arredondado a 8 degraus, repete-se - e aí pode ficar
            # misturado com o fundo (um blit com alfa deste tamanho custava ~5 ms no telemóvel).
            step = max(1, min(8, int(round(pulse * 8))))
            pulse = step / 8.0
            glow_w, glow_h = roll_rect.width + 20, roll_rect.height + 20
            glow_pos = (roll_rect.x - 10, roll_rect.y - 10)
            if bonus_kind == "rainbow":
                glow = rainbow_glow_surface(glow_w, glow_h, 20)
                glow.set_alpha(int(120 * pulse))
            else:
                glow_color, glow_alpha = (DIAMOND_BORDER, 130) if bonus_kind == "diamond" else (GOLD_BORDER, 90)
                glow = pygame.Surface((glow_w, glow_h), pygame.SRCALPHA)
                pygame.draw.rect(glow, (*glow_color, int(glow_alpha * pulse)), glow.get_rect(), border_radius=20)
            scene = bake_scene(getattr(self, "bg_surface", None),
                               pygame.Rect(glow_pos, (glow_w, glow_h)),
                               ("roll_glow", bonus_kind, step, roll_rect.topleft),
                               lambda flat: flat.blit(glow, (0, 0)))
            self.canvas.blit(scene if scene is not None else glow, glow_pos)
        # sorte total dos bónus ativos (Golden + Diamond + Rainbow somados), à direita da palavra ROLL
        luck_total, luck_top = self.active_roll_luck()
        luck_parts = None                     # (etiqueta SORTE, número ×N, x do centro do bloco)
        if luck_total > 0:
            luck_color = {"rainbow": (255, 140, 220), "diamond": DIAMOND_BORDER}.get(luck_top, GOLD_BORDER)
            num_txt = self.font_med.render("\u00d7%s" % format_number(luck_total), True, luck_color)
            if num_txt.get_width() > 60:
                num_txt = self.font_small_b.render("\u00d7%s" % format_number(luck_total), True, luck_color)
            lab_txt = self.font_tiny.render(tr("LUCK"), True, WHITE)
            luck_parts = (lab_txt, num_txt, roll_rect.right - 42)

        # O texto do botão fica centrado, mas nunca em cima do bloco "SORTE ×N": em Português "ROLAR" é mais
        # comprido que "ROLL" e, quando a sorte aparece, encostava a ela. Se preciso, desloca-se para a esquerda.
        roll_txt = self.font_huge.render(tr("ROLL"), True, WHITE)
        roll_cx = roll_rect.centerx
        if luck_parts:
            block_left = luck_parts[2] - max(luck_parts[0].get_width(), luck_parts[1].get_width()) // 2
            limit = block_left - 18
            if roll_cx + roll_txt.get_width() // 2 > limit:
                roll_cx = limit - roll_txt.get_width() // 2
                if roll_cx - roll_txt.get_width() // 2 < roll_rect.x + 10:       # nem deslocado cabe: letra mais pequena
                    roll_txt = self.font_big.render(tr("ROLL"), True, WHITE)
                    roll_cx = min(roll_rect.centerx, limit - roll_txt.get_width() // 2)
        self.button(roll_rect, "", self.font_huge, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.do_roll, radius=14, sfx=None)
        roll_fill = PANEL_LIGHTER if roll_rect.collidepoint(mouse_pos) else PANEL_LIGHT
        self.canvas.blit(bake(roll_txt, roll_fill), roll_txt.get_rect(center=(roll_cx, roll_rect.centery)))
        if bonus_kind == "rainbow":
            draw_rainbow_border(self.canvas, roll_rect.inflate(-9, -9), 10, width=3)
        elif bonus_kind == "diamond":
            draw_state_border(self.canvas, roll_rect, DIAMOND_BORDER, 14, width=3)
        elif bonus_kind == "golden":
            draw_state_border(self.canvas, roll_rect, GOLD_BORDER, 14, width=3)

        if luck_parts:
            lab_txt, num_txt, bx = luck_parts
            gap = -2
            block_h = lab_txt.get_height() + gap + num_txt.get_height()
            by = roll_rect.centery - block_h // 2
            self.canvas.blit(lab_txt, lab_txt.get_rect(midtop=(bx, by)))
            self.canvas.blit(num_txt, num_txt.get_rect(midtop=(bx, by + lab_txt.get_height() + gap)))

        # --- Golden / Diamond / Rainbow Roll (por baixo do ROLL, cada um na sua linha) ---
        y = roll_rect.bottom + 16

        def draw_cycle_line(key, y, ready, label, mult, left, every, color):
            pill = pygame.Rect(center_x - 150, y, 300, 34)
            paused = self.state.is_cycle_paused(key)      # clicar na barra pausa / retoma este ciclo

            # roll tão rápido que o ciclo passa várias vezes por segundo -> barra sempre cheia
            cycles_per_sec = self.roll_rate / float(max(1, every))
            cont = self.bar_continuous.get(key, False)
            if paused:
                cont = False              # pausado o ciclo não anda, por isso a barra não fica "sempre cheia"
            elif cont and cycles_per_sec < 1.2:
                cont = False
            elif not cont and cycles_per_sec >= 2.0:
                cont = True
            self.bar_continuous[key] = cont

            target = 1.0 if (ready or cont) else max(0.0, 1.0 - left / float(max(1, every)))
            shown = self.bar_display.get(key, target)
            if target < shown - 0.02:          # o ciclo reiniciou -> volta ao início
                shown = target
            else:                               # senão enche suavemente
                shown += (target - shown) * min(1.0, self.frame_dt * 16.0)
            self.bar_display[key] = shown

            pygame.draw.rect(self.canvas, PANEL, pill, border_radius=17)
            inner = pill.inflate(-10, -10)
            pygame.draw.rect(self.canvas, (20, 20, 23), inner, border_radius=inner.height // 2)
            fill_w = int(inner.width * max(0.0, min(1.0, shown)))
            if fill_w > 0:
                fill = bar_fill_surface(key, color, inner.width, inner.height)
                self.canvas.blit(fill, inner.topleft, area=pygame.Rect(0, 0, fill_w, inner.height))
            # contorno: preto normal, cinzento claro com o rato em cima, VERMELHO quando pausada
            outline_color = PAUSED_RED if paused else (GREY if pill.collidepoint(mouse_pos) else OUTLINE)
            pygame.draw.rect(self.canvas, outline_color, pill, width=BORDER_W_SMALL, border_radius=17)
            self.register_button(pill, lambda k=key: self.state.toggle_cycle_pause(k), "click")

            name = tr(label)              # "Golden Roll" / "Diamond Roll" / "Rainbow Roll" no idioma atual
            if ready:
                text = tr("%s READY!  x%g", name.upper(), mult)
            elif cont:
                text = "%s  x%g" % (name.upper(), mult)
            else:
                text = tr("%s in %d roll", name, left) if left == 1 else tr("%s in %d rolls", name, left)
            txt = self.font_small_b.render(text, True, WHITE)
            # o texto da barra fica por cima do enchimento escuro do meio da pilula
            self.canvas.blit(bake(txt, (20, 20, 23)), txt.get_rect(center=pill.center))
            return y + pill.height + 8

        y = draw_cycle_line("golden", y, self.state.cyclic_bonus_ready, "Golden Roll",
                            self.state.golden_roll_mult(), self.state.rolls_until_golden_roll(),
                            self.state.golden_roll_every(), GOLD_BORDER)

        if self.state.diamond_roll_unlocked():
            y = draw_cycle_line("diamond", y, self.state.diamond_bonus_ready, "Diamond Roll",
                                self.state.diamond_roll_mult(), self.state.rolls_until_diamond_roll(),
                                self.state.diamond_roll_every(), DIAMOND_BORDER)

        if self.state.rainbow_roll_unlocked():
            y = draw_cycle_line("rainbow", y, self.state.rainbow_bonus_ready, "Rainbow Roll",
                                self.state.rainbow_roll_mult(), self.state.rolls_until_rainbow_roll(),
                                self.state.rainbow_roll_every(), (255, 140, 220))
