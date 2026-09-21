"""Ecrãs do título e dos saves (slots)."""

import math
import time
import pygame

from config import FULLSCREEN_HINT, GAME_TITLE, SAVE_SLOTS, VERSION, VIRTUAL_H
from core.formatting import format_number, format_playtime
from core.game_state import GameState
from core.offline import offline_message
from core.pets import RARITIES, TIER_FIRST_PET
from i18n import tr
from storage import delete_slot, peek_slot
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK,
    BORDER_W_SMALL, GOOD, GREY, GREY_DIM,
    OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER,
    WHITE,
)
from ui.drawing import blit_smooth_y, draw_panel, draw_rarity_bg, draw_state_border
from ui.fonts import fit_text, wrap_text
from ui.icons import load_icon


class TitleSavesMixin:
    """Ecrãs do título e dos saves (slots)."""

    # ---------------------------------------------------------------- navegação (menu / saves / jogo)
    def refresh_slot_info(self):
        self.slot_info = {i: peek_slot(i) for i in range(1, SAVE_SLOTS + 1)}

    def open_saves(self):
        self.refresh_slot_info()
        self.delete_confirm_slot = None
        self.screen_mode = "saves"
        if self.account and not self.cloud_slots_loaded:
            self.load_cloud_slots()

    def back_to_title(self):
        self.delete_confirm_slot = None
        self.screen_mode = "title"

    def request_delete(self, slot):
        if self.delete_confirm_slot == slot:
            delete_slot(slot)
            self.delete_confirm_slot = None
            self.refresh_slot_info()
            self.play("click")
            self.show_toast(tr("Slot %d deleted.", slot))
        else:
            self.delete_confirm_slot = slot
            self.delete_confirm_timer = 3.0

    def request_delete_account(self, slot):
        """Como request_delete, mas para um slot da CONTA (apaga só na cloud/cache; o
        ficheiro local, se existir, nunca é tocado)."""
        if self.delete_confirm_slot == slot:
            self.delete_confirm_slot = None
            self.play("click")
            self.delete_account_slot(slot)
        else:
            self.delete_confirm_slot = slot
            self.delete_confirm_timer = 3.0

    def start_slot(self, slot):
        st = GameState()
        st.slot = slot
        st.load()
        gain, away = st.claim_offline_earnings()      # dinheiro ganho enquanto o jogo esteve fechado
        st.save()                      # cria logo o ficheiro do slot (se for um jogo novo)
        self.state = st
        self.reset_ui()
        self.options_open = False
        self.stats_open = False
        self.autosave_timer = 0.0
        self.screen_mode = "game"
        if gain > 0:
            self.show_toast("%s\n%s" % (tr("Slot %d loaded", slot), offline_message(gain, away)), 6.0)
        else:
            self.show_toast(tr("Slot %d loaded", slot))

    def go_to_menu(self):
        self.state.save()
        self.state = GameState()
        self.reset_ui()
        self.options_open = False
        self.stats_open = False
        self.screen_mode = "title"

    # ---------------------------------------------------------------- menu principal
    def draw_title(self, mouse_pos):
        cx = self.vw // 2
        t = time.time()

        # fila de cartõezinhos das raridades a "flutuar" por baixo do título
        tier_pets = [RARITIES[j] for j in TIER_FIRST_PET]      # uma carta por raridade
        n = len(tier_pets)
        size, gap = 64, 16
        total = n * size + (n - 1) * gap
        x0 = cx - total // 2
        def build_card(r=None):
            card = pygame.Surface((size, size), pygame.SRCALPHA)
            draw_rarity_bg(card, pygame.Rect(0, 0, size, size), r, radius=10)
            pygame.draw.rect(card, OUTLINE, card.get_rect(), width=BORDER_W_SMALL, border_radius=10)
            return card

        for i, r in enumerate(tier_pets):
            bob = math.sin(t * 1.6 + i * 0.7) * 8 if self.animations else 0
            # posição vertical com fração de píxel (blit_smooth_y): sem isto o sobe-e-desce vê-se aos saltinhos
            blit_smooth_y(self.canvas, ("title_card", r["key"], size), lambda r=r: build_card(r),
                          x0 + i * (size + gap), 250 + bob)

        # título com sombra
        title = self.font_title.render(GAME_TITLE, True, WHITE)
        shadow = title.copy()
        shadow.fill((0, 0, 0, 110), special_flags=pygame.BLEND_RGBA_MULT)   # silhueta escura = sombra
        self.canvas.blit(shadow, shadow.get_rect(center=(cx + 3, 128 + 9)))
        self.canvas.blit(title, title.get_rect(center=(cx, 128)))
        sub = self.font_med.render(tr("Roll pets, equip them and fill your pockets"), True, GREY)
        self.canvas.blit(sub, sub.get_rect(center=(cx, 212)))

        self.draw_account_chip(mouse_pos)

        bw, bh = 340, 62
        y = 360
        stack_top = y
        self.button(pygame.Rect(cx - bw // 2, y, bw, bh), tr("Saves"), self.font_big, mouse_pos,
                    ACCENT, ACCENT_HOVER, BLACK, callback=self.open_saves, radius=14, icon="saves")
        y += bh + 14
        self.nav_mode = True      # continua clicável com o Leaderboard / as Options / os Credits abertos (para os fechar)
        self.button(pygame.Rect(cx - bw // 2, y, bw, bh), tr("Leaderboard"), self.font_big, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, ACCENT, callback=self.toggle_leaderboard, radius=14,
                    icon="leaderboard")
        y += bh + 14
        self.button(pygame.Rect(cx - bw // 2, y, bw, bh), tr("Options"), self.font_big, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.toggle_options, radius=14,
                    icon="options")
        y += bh + 14
        half = (bw - 14) // 2          # Credits e Quit lado a lado na última linha
        self.button(pygame.Rect(cx - bw // 2, y, half, bh), tr("Credits"), self.font_big, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.toggle_credits, radius=14)
        self.nav_mode = False
        self.button(pygame.Rect(cx - bw // 2 + half + 14, y, half, bh), tr("Quit"), self.font_big, mouse_pos,
                    (150, 60, 60), BAD, WHITE, callback=self.quit_game, radius=14)

        # mascote: centrado no retângulo livre à direita dos botões (do fim dos botões até à borda do ecrã),
        # na vertical alinhado com a pilha de botões
        stack_cy = (stack_top + y + bh) // 2
        self.draw_mascot((cx + bw // 2 + self.vw) // 2, stack_cy + 115, size=230)

        log_rect = pygame.Rect(20, VIRTUAL_H - 20 - 44, 190, 44)
        self.nav_mode = True      # continua clicável com o painel aberto (para o fechar)
        self.button(log_rect, tr("Update Log"), self.font_small_b, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.toggle_update_log, radius=10, icon="updatelog")
        self.nav_mode = False

        foot = self.font_small.render(tr("%s   ·   %s: fullscreen / windowed", VERSION, FULLSCREEN_HINT),
                                      True, GREY_DIM)
        self.canvas.blit(foot, foot.get_rect(center=(cx, VIRTUAL_H - 20)))

    def draw_mascot(self, cx, ground_y, size=230):
        """Tung Tung Tung Sahur (icons/tung.png) no menu principal, com os pés em (cx, ground_y).
        Balança-se de um lado para o outro e "respira" (cresce um pouco) sempre com o pivô nos pés.
        Tudo isto é feito com um único rotozoom (que interpola os píxeis), por isso o movimento é
        contínuo, sem saltinhos. Com as animações desligadas fica quieto. Sem o ficheiro, não aparece."""
        img = load_icon("tung", size)
        if img is None:
            return
        cache = getattr(self, "_mascot_pad", None)
        if cache is None or cache[0] is not img:
            # superfície com o dobro da altura: o Tung na metade de cima, por isso o CENTRO dela (que é
            # o pivô da rotação / do zoom) fica exatamente nos pés
            padded = pygame.Surface((size, size * 2), pygame.SRCALPHA)
            padded.blit(img, (0, 0))
            cache = (img, padded)
            self._mascot_pad = cache
        padded = cache[1]
        t = time.time()
        angle = math.sin(t * 1.3) * 3.5 if self.animations else 0.0
        scale = 1.0 + 0.025 * math.sin(t * 2.0) if self.animations else 1.0
        # sombra no chão (acompanha o "respirar")
        shadow = pygame.Surface((int(150 * scale), 26), pygame.SRCALPHA)
        pygame.draw.ellipse(shadow, (0, 0, 0, 55), shadow.get_rect())
        self.canvas.blit(shadow, shadow.get_rect(center=(cx, ground_y - 2)))
        frame = pygame.transform.rotozoom(padded, angle, scale) if self.animations else padded
        self.canvas.blit(frame, frame.get_rect(center=(cx, ground_y)))

    def draw_save_card_body(self, rect, info):
        """Dinheiro + linhas de Rolls/Playtime no meio do cartão de um slot (save local ou de conta)."""
        coins_txt = self.font_big.render("$ %s" % format_number(info["coins"]), True, GOOD)
        self.canvas.blit(coins_txt, coins_txt.get_rect(center=(rect.centerx, rect.y + 98)))
        rows = [(tr("Rolls"), format_number(info["total_rolls"])),
                (tr("Playtime"), format_playtime(info["playtime"]))]
        ry = rect.y + 152
        for label, value in rows:
            l_txt = self.font_small.render(label, True, GREY)
            v_txt = self.font_small_b.render(value, True, WHITE)
            self.canvas.blit(l_txt, (rect.x + 28, ry))
            self.canvas.blit(v_txt, (rect.right - 28 - v_txt.get_width(), ry))
            ry += 30

    def draw_saves(self, mouse_pos):
        cx = self.vw // 2
        title = self.font_huge.render(tr("Saves"), True, WHITE)
        self.canvas.blit(title, title.get_rect(center=(cx, 70)))
        if self.account:
            sub_text = tr("%s's saves - synced to your account (up to %d slots).",
                          self.account["username"], SAVE_SLOTS)
        else:
            sub_text = tr("Pick a slot to play (up to %d saves).", SAVE_SLOTS)
        sub = self.font_small.render(fit_text(self.font_small, sub_text, self.vw - 340), True, GREY)
        self.canvas.blit(sub, sub.get_rect(center=(cx, 114)))
        self.draw_account_chip(mouse_pos)

        gap = 32
        card_w = min(320, (self.vw - 160 - gap * 2) // SAVE_SLOTS)
        card_h = 380
        total_w = card_w * SAVE_SLOTS + gap * (SAVE_SLOTS - 1)
        x0 = cx - total_w // 2
        y0 = 160
        busy = self.slot_starting or self.import_busy

        for i in range(SAVE_SLOTS):
            slot = i + 1
            rect = pygame.Rect(x0 + i * (card_w + gap), y0, card_w, card_h)
            draw_panel(self.canvas, rect, PANEL, radius=16)
            head = self.font_big.render(tr("Slot %d", slot), True, WHITE)
            self.canvas.blit(head, head.get_rect(center=(rect.centerx, rect.y + 38)))
            play_rect = pygame.Rect(rect.x + 24, rect.bottom - 118, rect.width - 48, 50)

            if not self.account:
                info = self.slot_info.get(slot)
                if info:
                    draw_state_border(self.canvas, rect, ACCENT, 16)
                    self.draw_save_card_body(rect, info)
                    self.button(play_rect, tr("Play"), self.font_med, mouse_pos,
                                ACCENT, ACCENT_HOVER, BLACK, callback=lambda s=slot: self.start_slot(s),
                                radius=12)
                    confirming = self.delete_confirm_slot == slot
                    self.button(pygame.Rect(rect.x + 24, rect.bottom - 56, rect.width - 48, 38),
                                tr("Click again to delete") if confirming else tr("Delete save"),
                                self.font_small_b, mouse_pos,
                                (170, 60, 60) if confirming else PANEL_LIGHT,
                                BAD, WHITE, callback=lambda s=slot: self.request_delete(s), radius=10, sfx=None)
                else:
                    empty = self.font_med.render(tr("Empty"), True, GREY_DIM)
                    self.canvas.blit(empty, empty.get_rect(center=(rect.centerx, rect.centery - 20)))
                    self.button(play_rect, tr("New Game"), self.font_med, mouse_pos,
                                ACCENT, ACCENT_HOVER, BLACK, callback=lambda s=slot: self.start_slot(s),
                                radius=12)
                continue

            # ---- sessão iniciada: mostra os saves DESTA CONTA (cloud / cache / importável) ----
            kind, info, label = self.slot_view(slot)
            if label:
                badge_color = GOOD if label == "Cloud" else GREY_DIM
                badge = self.font_tiny.render(tr(label), True, badge_color)
                self.canvas.blit(badge, (rect.right - 14 - badge.get_width(), rect.y + 15))

            if kind == "unknown":
                failed = bool(self.cloud_slots_error) and not self.cloud_slots_loading
                msg = self.font_med.render(tr("Couldn't load") if failed else tr("Loading..."), True,
                                           BAD if failed else GREY_DIM)
                self.canvas.blit(msg, msg.get_rect(center=(rect.centerx, rect.centery)))
            elif kind == "empty":
                empty = self.font_med.render(tr("Empty"), True, GREY_DIM)
                self.canvas.blit(empty, empty.get_rect(center=(rect.centerx, rect.centery - 20)))
                self.button(play_rect, tr("New Game"), self.font_med, mouse_pos,
                            ACCENT, ACCENT_HOVER, BLACK, callback=lambda s=slot: self.start_cloud_slot(s),
                            radius=12, enabled=not busy)
            elif kind == "importable":
                draw_state_border(self.canvas, rect, PANEL_LIGHTER, 16)
                self.draw_save_card_body(rect, info)
                self.button(play_rect, tr("Import to account"), self.font_small_b, mouse_pos,
                            ACCENT, ACCENT_HOVER, BLACK, callback=lambda s=slot: self.import_local_slot(s),
                            radius=12, enabled=not self.import_busy)
                ny = rect.bottom - 60
                for line in wrap_text(tr("Local save on this PC. Importing copies it here - the local "
                                         "file is kept."), self.font_tiny, rect.width - 32)[:2]:
                    t = self.font_tiny.render(line, True, GREY_DIM)
                    self.canvas.blit(t, (rect.x + 16, ny))
                    ny += 15
            else:   # "save": há progresso desta conta neste slot (cloud, cache offline ou por enviar)
                draw_state_border(self.canvas, rect, ACCENT, 16)
                self.draw_save_card_body(rect, info)
                self.button(play_rect, tr("Play"), self.font_med, mouse_pos,
                            ACCENT, ACCENT_HOVER, BLACK, callback=lambda s=slot: self.start_cloud_slot(s),
                            radius=12, enabled=not busy)
                confirming = self.delete_confirm_slot == slot
                self.button(pygame.Rect(rect.x + 24, rect.bottom - 56, rect.width - 48, 38),
                            tr("Click again to delete") if confirming else tr("Delete save"),
                            self.font_small_b, mouse_pos,
                            (170, 60, 60) if confirming else PANEL_LIGHT,
                            BAD, WHITE, callback=lambda s=slot: self.request_delete_account(s),
                            radius=10, sfx=None)

        foot_y = y0 + card_h + 40
        self.button(pygame.Rect(cx - 110, foot_y, 220, 48), tr("Back"), self.font_med, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.back_to_title, radius=12)

        if self.account and self.cloud_slots_error and not self.cloud_slots_loading:
            if self.cloud_slots_loaded:
                note = tr("Couldn't refresh your cloud saves - showing the last known copy.")
            else:
                note = self.cloud_slots_error_msg or tr("Couldn't reach your cloud saves.")
            note = fit_text(self.font_tiny, note, max(200, self.vw - (cx + 130) - 20))
            t = self.font_tiny.render(note, True, BAD)
            self.canvas.blit(t, (cx + 130, foot_y + 8))
            self.button(pygame.Rect(cx + 130, foot_y + 26, 90, 32), tr("Retry"), self.font_small_b, mouse_pos,
                        PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.load_cloud_slots, radius=8)
