"""Ecrã de Amigos: procurar, pedidos, lista, stats de um amigo e escolha da foto de perfil."""

import time
import pygame

from config import TOPBAR_H, VIRTUAL_H
from core.formatting import format_number, format_playtime
from core.pets import MUTATIONS, RARITIES
from i18n import tr, tr_short
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK, GOOD, GREY, GREY_DIM,
    OUTLINE, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
)
from ui.avatar import avatar_ring_color, avatar_surface
from ui.drawing import bake, dim_overlay, draw_panel
from ui.fonts import fit_text, wrap_text


PANEL_W = 680
ROW_H = 56
ROW_GAP = 6
AVATAR_ROW = 40
AVATAR_BIG = 96
PICKER_CELL = 84          # célula da grelha das fotos: foto + nome do verity por baixo
PICKER_GAP = 10

FRIEND_TABS = (("friends", "Friends"), ("requests", "Requests"), ("add", "Add friend"))


class FriendsPanelMixin:
    """Ecrã de Amigos (a lógica e as chamadas à cloud estão em online/friends.py)."""

    # ================================================================ desenho
    def draw_friends(self, mouse_pos):
        self.refresh_friends()
        self.canvas.blit(dim_overlay(self.vw, VIRTUAL_H, 170), (0, 0))

        top = TOPBAR_H + 12
        panel_w = min(PANEL_W, self.vw - 40)
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, top, panel_w, VIRTUAL_H - top - 14)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)      # clicar dentro da página não a fecha

        title = self.font_big.render(tr("Friends"), True, WHITE)
        self.canvas.blit(bake(title, PANEL), (rect.x + 26, rect.y + 16))
        close_rect = pygame.Rect(rect.right - 48, rect.y + 18, 30, 30)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_friends, radius=8)

        # cartão do próprio jogador: foto + nome + "Change photo"
        me_rect = pygame.Rect(rect.x + 22, rect.y + 16 + title.get_height() + 10, panel_w - 44, 64)
        self.draw_friends_me(me_rect, mouse_pos)

        body_top = me_rect.bottom + 12
        if not self.friends_ready():
            self.draw_friends_message(pygame.Rect(rect.x + 22, body_top, panel_w - 44,
                                                  rect.bottom - 22 - body_top),
                                      tr("Log in from the main menu to add friends."))
            return

        if self.avatar_picker:
            self.draw_avatar_picker(pygame.Rect(rect.x + 22, body_top, panel_w - 44,
                                                rect.bottom - 22 - body_top), mouse_pos)
            return
        if self.chat_uid:
            self.draw_chat(pygame.Rect(rect.x + 22, body_top, panel_w - 44,
                                       rect.bottom - 22 - body_top), mouse_pos)
            return
        if self.friend_view:
            self.draw_friend_details(pygame.Rect(rect.x + 22, body_top, panel_w - 44,
                                                 rect.bottom - 22 - body_top), mouse_pos)
            return

        # separadores Friends / Requests / Add friend
        tab_gap = 10
        tab_w = (panel_w - 44 - tab_gap * (len(FRIEND_TABS) - 1)) // len(FRIEND_TABS)
        counts = {"friends": len(self.friends_list), "requests": len(self.friends_incoming)}
        for i, (key, label) in enumerate(FRIEND_TABS):
            text = tr_short(label)
            n = counts.get(key, 0)
            if n:
                text = "%s (%d)" % (text, n)
            trect = pygame.Rect(rect.x + 22 + i * (tab_w + tab_gap), body_top, tab_w, 40)
            active = self.friends_tab == key
            font = self.font_med if self.font_med.size(text)[0] <= trect.width - 16 else self.font_small_b
            self.button(trect, text, font, mouse_pos,
                        ACCENT if active else PANEL_LIGHT, ACCENT_HOVER if active else PANEL_LIGHTER,
                        BLACK if active else WHITE, callback=lambda k=key: self.set_friends_tab(k), radius=10)
            if key == "requests" and n and not active:
                self.draw_friends_badge(trect.topright, n)

        content_top = body_top + 40 + 12
        foot_h = self.draw_friends_footer(rect, mouse_pos)
        list_rect = pygame.Rect(rect.x + 22, content_top, panel_w - 44,
                                rect.bottom - 22 - foot_h - content_top)
        self.friends_list_rect = list_rect

        if self.friends_tab == "add":
            self.draw_friends_add(list_rect, mouse_pos)
        elif self.friends_tab == "requests":
            self.draw_friends_requests(list_rect, mouse_pos)
        else:
            self.draw_friends_friends(list_rect, mouse_pos)

    # ---------------------------------------------------------------- peças partilhadas
    def draw_friends_me(self, rect, mouse_pos):
        pygame.draw.rect(self.canvas, PANEL_LIGHT, rect, border_radius=12)
        pygame.draw.rect(self.canvas, OUTLINE, rect, width=2, border_radius=12)
        pet, mut = self.avatar_pair()
        img = avatar_surface(pet, mut, 48)
        self.canvas.blit(img, img.get_rect(midleft=(rect.x + 12, rect.centery)))
        name = self.account["username"] if self.account else tr("Not logged in")
        nt = self.font_med.render(fit_text(self.font_med, name, rect.width - 220), True, WHITE)
        self.canvas.blit(nt, (rect.x + 72, rect.centery - nt.get_height() - 1))
        sub = tr("%d friends", len(self.friends_list)) if self.friends_ready() else tr("Offline")
        self.canvas.blit(self.font_tiny.render(sub, True, GREY_DIM), (rect.x + 72, rect.centery + 3))
        if self.friends_ready():
            self.button(pygame.Rect(rect.right - 152, rect.y + 12, 140, rect.height - 24),
                        tr("Change photo"), self.font_small_b, mouse_pos, PANEL_LIGHTER, ACCENT_HOVER, WHITE,
                        callback=self.open_avatar_picker, radius=10)

    def open_avatar_picker(self):
        self.avatar_picker = True
        self.friends_scroll = 0.0
        self.friends_msg = None
        self.set_friends_focus(False)

    def draw_friends_badge(self, topright, count):
        """Bolinha vermelha com o número de pedidos por responder."""
        r = 11
        center = (topright[0] - 4, topright[1] + 2)
        pygame.draw.circle(self.canvas, BAD, center, r)
        pygame.draw.circle(self.canvas, OUTLINE, center, r, 2)
        t = self.font_tiny_b.render(str(min(count, 99)), True, WHITE)
        self.canvas.blit(t, t.get_rect(center=center))

    def draw_friends_message(self, rect, text, color=GREY):
        self.friends_max_scroll = 0.0
        y = rect.centery - 16
        for line in wrap_text(text, self.font_med, rect.width - 60):
            t = self.font_med.render(line, True, color)
            self.canvas.blit(t, t.get_rect(center=(rect.centerx, y)))
            y += 26

    def draw_friends_footer(self, rect, mouse_pos):
        """Rodapé: atualizar + estado. Devolve a altura que ocupa (o que sobra é a lista)."""
        h = 40
        y = rect.bottom - 22 - h
        self.button(pygame.Rect(rect.x + 22, y, 130, h), tr("Refresh"), self.font_small_b, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=lambda: self.refresh_friends(force=True), radius=10)
        if self.friends_loading:
            text, color = tr("Updating..."), GREY_DIM
        elif self.friends_error:
            text, color = self.friends_error, BAD
        elif self.friends_msg:
            text, color = self.friends_msg
        else:
            text, color = "", GREY
        if text:
            t = self.font_small.render(fit_text(self.font_small, text, rect.width - 200), True, color)
            self.canvas.blit(t, t.get_rect(midright=(rect.right - 26, y + h // 2)))
        return h + 12

    def friend_avatar(self, entry, size):
        return avatar_surface(entry.get("avatar_pet"), entry.get("avatar_mut", "normal"), size)

    def draw_friend_card(self, rect, entry, mouse_pos, buttons=(), on_click=None):
        """Uma linha da lista: foto + nome (+ botões à direita). 'buttons' = [(texto, cor, callback)]."""
        hovering = on_click is not None and rect.collidepoint(mouse_pos) and self.clip_allows(rect)
        pygame.draw.rect(self.canvas, PANEL_LIGHTER if hovering else PANEL_LIGHT, rect, border_radius=12)
        pygame.draw.rect(self.canvas, OUTLINE, rect, width=2, border_radius=12)
        img = self.friend_avatar(entry, AVATAR_ROW)
        self.canvas.blit(img, img.get_rect(midleft=(rect.x + 10, rect.centery)))
        if self.chat_unread(entry.get("uid")):
            # ponto vermelho: mensagem nova que ainda não foi vista
            pygame.draw.circle(self.canvas, BAD, (rect.x + 10 + AVATAR_ROW - 2, rect.centery - AVATAR_ROW // 2 + 4), 6)
            pygame.draw.circle(self.canvas, PANEL, (rect.x + 10 + AVATAR_ROW - 2, rect.centery - AVATAR_ROW // 2 + 4), 6, width=2)

        bx = rect.right - 10
        busy = self.friends_action == entry.get("uid")
        for label, color, callback in reversed(buttons):
            bw = max(84, self.font_small_b.size(label)[0] + 26)
            brect = pygame.Rect(bx - bw, rect.centery - 16, bw, 32)
            hover = tuple(min(255, c + 30) for c in color)
            text_color = BLACK if color in (ACCENT, GOOD) else WHITE
            self.button(brect, label, self.font_small_b, mouse_pos, color, hover, text_color,
                        callback=callback, radius=8, enabled=not busy)
            bx = brect.x - 8

        name_w = max(40, bx - (rect.x + 60))
        nt = self.font_med.render(fit_text(self.font_med, entry.get("username", "?"), name_w), True, WHITE)
        self.canvas.blit(nt, (rect.x + 60, rect.centery - nt.get_height() // 2))
        if on_click is not None:
            self.register_button(rect, on_click, "click")
        return hovering

    def draw_friends_rows(self, rect, rows, mouse_pos):
        """Desenha a lista com scroll. 'rows' = [(altura, função(rect))] - assim cabem cabeçalhos
        de secção e cartões na mesma lista."""
        scroll = self.friends_scroll
        self.push_clip(rect)
        y = rect.top - scroll
        for height, draw in rows:
            rrect = pygame.Rect(rect.x, y, rect.width - 16, height)
            if rrect.bottom >= rect.top - 4 and rrect.top <= rect.bottom + 4:
                draw(rrect)
            y += height + ROW_GAP
        content_h = sum(h for h, _d in rows) + ROW_GAP * max(0, len(rows) - 1)
        self.pop_clip()
        self.friends_max_scroll = max(0.0, content_h - rect.height)
        self.friends_scroll = max(0.0, min(self.friends_max_scroll, self.friends_scroll))
        self.draw_scrollbar(rect, scroll, content_h, key="friends", mouse_pos=mouse_pos)

    def draw_friends_section(self, rect, text):
        t = self.font_small_b.render(text, True, GREY_DIM)
        self.canvas.blit(t, (rect.x + 4, rect.bottom - t.get_height() - 4))
        pygame.draw.line(self.canvas, PANEL_LIGHT, (rect.x + 4, rect.bottom), (rect.right - 4, rect.bottom), 2)

    # ---------------------------------------------------------------- separador: amigos
    def draw_friends_friends(self, rect, mouse_pos):
        if not self.friends_list:
            if self.friends_loading and not self.friends_loaded:
                self.draw_friends_message(rect, tr("Loading friends..."))
            else:
                self.draw_friends_message(rect, tr("No friends yet - use \"Add friend\" to find someone."))
            return
        rows = []
        for entry in self.friends_list:
            def draw(rrect, entry=entry):
                self.draw_friend_card(
                    rrect, entry, mouse_pos,
                    buttons=[(tr("Chat"), ACCENT, lambda e=entry: self.open_chat(e["uid"], e["username"])),
                             (tr("Stats"), PANEL_LIGHTER, lambda e=entry: self.open_friend(e["uid"]))],
                    on_click=lambda e=entry: self.open_friend(e["uid"]))
            rows.append((ROW_H, draw))
        self.draw_friends_rows(rect, rows, mouse_pos)

    # ---------------------------------------------------------------- separador: pedidos
    def draw_friends_requests(self, rect, mouse_pos):
        if not self.friends_incoming and not self.friends_outgoing:
            if self.friends_loading and not self.friends_loaded:
                self.draw_friends_message(rect, tr("Loading requests..."))
            else:
                self.draw_friends_message(rect, tr("No friend requests right now."))
            return
        rows = []
        if self.friends_incoming:
            rows.append((22, lambda rrect: self.draw_friends_section(rrect, tr("Received"))))
            for req in self.friends_incoming:
                def draw(rrect, req=req):
                    self.draw_friend_card(rrect, req, mouse_pos, buttons=[
                        (tr("Accept"), GOOD, lambda r=req: self.accept_friend(r)),
                        (tr("Decline"), BAD, lambda r=req: self.decline_friend(r)),
                    ])
                rows.append((ROW_H, draw))
        if self.friends_outgoing:
            rows.append((28, lambda rrect: self.draw_friends_section(rrect, tr("Sent"))))
            for req in self.friends_outgoing:
                def draw(rrect, req=req):
                    self.draw_friend_card(rrect, req, mouse_pos, buttons=[
                        (tr("Cancel"), PANEL_LIGHTER, lambda r=req: self.cancel_friend_request(r)),
                    ])
                rows.append((ROW_H, draw))
        self.draw_friends_rows(rect, rows, mouse_pos)

    # ---------------------------------------------------------------- separador: adicionar
    def draw_friends_add(self, rect, mouse_pos):
        self.friends_max_scroll = 0.0
        field = pygame.Rect(rect.x, rect.y, rect.width - 130, 46)
        self.draw_friends_field(field)
        self.button(pygame.Rect(field.right + 10, field.y, 120, 46), tr("Search"), self.font_med, mouse_pos,
                    ACCENT, ACCENT_HOVER, BLACK, callback=self.submit_friend_search, radius=10,
                    enabled=not self.friends_search_busy)
        hint = self.font_tiny.render(tr("Type the exact username of the player you want to add."), True, GREY_DIM)
        self.canvas.blit(hint, (rect.x + 4, field.bottom + 8))

        card = pygame.Rect(rect.x, field.bottom + 34, rect.width, 76)
        if self.friends_search_busy:
            t = self.font_med.render(tr("Searching..."), True, GREY)
            self.canvas.blit(t, t.get_rect(center=(card.centerx, card.centery)))
            return
        if self.friends_result in (None, "none"):
            return
        profile = self.friends_result
        rel = self.friend_relation(profile["uid"])
        if rel == "friend":
            buttons = [(tr("Friends"), PANEL_LIGHT, None)]
        elif rel == "sent":
            buttons = [(tr("Request sent"), PANEL_LIGHT, None)]
        elif rel == "incoming":
            buttons = [(tr("Accept"), GOOD,
                        lambda: self.accept_friend({"uid": profile["uid"], "username": profile["username"]}))]
        else:
            buttons = [(tr("Add"), ACCENT,
                        lambda: self.send_friend_request(profile["uid"], profile["username"]))]
        self.draw_friend_card(card, profile, mouse_pos, buttons=buttons)

    def draw_friends_field(self, rect):
        """Campo de procura (mesmo aspeto do ecrã de conta: caixa escura, contorno ACCENT com foco)."""
        self.draw_text_field(rect, self.friends_search, self.friends_search.text, tr("username"),
                             self.friends_focus, lambda: self.set_friends_focus(True))

    # ---------------------------------------------------------------- stats de um amigo
    def draw_friend_details(self, rect, mouse_pos):
        self.friends_max_scroll = 0.0
        entry = next((f for f in self.friends_list if f["uid"] == self.friend_view), None)
        if entry is None:
            self.close_friend_view()
            return

        img = avatar_surface(entry.get("avatar_pet"), entry.get("avatar_mut", "normal"), AVATAR_BIG)
        self.canvas.blit(img, img.get_rect(midtop=(rect.centerx, rect.y + 6)))
        name = self.font_big.render(fit_text(self.font_big, entry["username"], rect.width - 40), True, WHITE)
        self.canvas.blit(bake(name, PANEL), name.get_rect(midtop=(rect.centerx, rect.y + AVATAR_BIG + 14)))

        pet = entry.get("avatar_pet")
        if isinstance(pet, int) and 0 <= pet < len(RARITIES):
            mut = MUTATIONS.get(entry.get("avatar_mut", "normal"), {}).get("label", "")
            label = ("%s %s" % (tr(mut), RARITIES[pet]["pet"])).strip()
            t = self.font_small.render(label, True, avatar_ring_color(entry.get("avatar_mut", "normal")))
            self.canvas.blit(bake(t, PANEL), t.get_rect(midtop=(rect.centerx, rect.y + AVATAR_BIG + 14 +
                                                                name.get_height() + 4)))

        stats = self.friend_stats.get(entry["uid"])
        y = rect.y + AVATAR_BIG + 82
        if stats is None:
            t = self.font_med.render(tr("Loading stats..."), True, GREY)
            self.canvas.blit(t, t.get_rect(center=(rect.centerx, y + 40)))
        elif stats == "none":
            t = self.font_med.render(tr("This player has no public stats yet."), True, GREY)
            self.canvas.blit(t, t.get_rect(center=(rect.centerx, y + 40)))
        else:
            lines = [
                (tr("Total Coins Earned"), "$" + format_number(stats["coins"])),
                (tr("Playtime"), format_playtime(stats["playtime"])),
                (tr("Total Rolls"), format_number(stats["rolls"])),
                (tr("Rebirths"), format_number(stats["rebirths"])),
            ]
            row_h = 36
            for label, value in lines:
                lt = self.font_med.render(label, True, GREY)
                vt = self.font_med.render(value, True, WHITE)
                self.canvas.blit(bake(lt, PANEL), (rect.x + 20, y))
                self.canvas.blit(bake(vt, PANEL), (rect.right - 20 - vt.get_width(), y))
                pygame.draw.line(self.canvas, PANEL_LIGHT, (rect.x + 20, y + row_h - 8),
                                 (rect.right - 20, y + row_h - 8), 1)
                y += row_h
            if stats.get("updated_at"):
                t = self.font_tiny.render(tr("Stats update every %d minutes.", 10), True, GREY_DIM)
                self.canvas.blit(t, (rect.x + 20, y + 4))

        self.button(pygame.Rect(rect.x, rect.bottom - 40, 140, 40), tr("Back"), self.font_small_b, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.close_friend_view, radius=10)
        self.button(pygame.Rect(rect.centerx - 75, rect.bottom - 40, 150, 40), tr("Message"),
                    self.font_small_b, mouse_pos, ACCENT, ACCENT_HOVER, BLACK,
                    callback=lambda: self.open_chat(entry["uid"], entry["username"]), radius=10)
        self.button(pygame.Rect(rect.right - 170, rect.bottom - 40, 170, 40), tr("Remove friend"),
                    self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=lambda: self.remove_friend(entry["uid"]), radius=10,
                    enabled=self.friends_action != entry["uid"])

    # ---------------------------------------------------------------- escolher a foto
    def draw_avatar_picker(self, rect, mouse_pos):
        head = self.font_med.render(tr("Choose a photo"), True, WHITE)
        self.canvas.blit(bake(head, PANEL), (rect.x + 4, rect.y))
        sub = self.font_tiny.render(tr("Any verity you own can be your photo - Golden and Diamond "
                                       "keep their coloured ring."), True, GREY_DIM)
        self.canvas.blit(sub, (rect.x + 4, rect.y + head.get_height() + 2))

        foot_y = rect.bottom - 40
        self.button(pygame.Rect(rect.x, foot_y, 140, 40), tr("Back"), self.font_small_b, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=lambda: setattr(self, "avatar_picker", False), radius=10)
        if self.avatar_pair()[0] is not None:
            self.button(pygame.Rect(rect.right - 160, foot_y, 160, 40), tr("No photo"), self.font_small_b,
                        mouse_pos, PANEL_LIGHT, BAD, WHITE, callback=self.clear_avatar, radius=10)

        grid = pygame.Rect(rect.x, rect.y + head.get_height() + 24, rect.width,
                           foot_y - 12 - (rect.y + head.get_height() + 24))
        options = self.avatar_options()
        if not options:
            self.draw_friends_message(grid, tr("Roll a verity first - then it can be your photo."))
            return

        per_row = max(1, (grid.width - 16) // (PICKER_CELL + PICKER_GAP))
        current = tuple(self.state.avatar) if self.state.avatar else None
        scroll = self.friends_scroll
        self.push_clip(grid)
        for i, (idx, mut) in enumerate(options):
            col, row = i % per_row, i // per_row
            cell = pygame.Rect(grid.x + col * (PICKER_CELL + PICKER_GAP),
                               grid.y + row * (PICKER_CELL + PICKER_GAP) - scroll,
                               PICKER_CELL, PICKER_CELL)
            if cell.bottom < grid.top - 4 or cell.top > grid.bottom + 4:
                continue
            chosen = current == (idx, mut)
            hovering = cell.collidepoint(mouse_pos) and self.clip_allows(cell)
            back = ACCENT if chosen else (PANEL_LIGHTER if hovering else PANEL_LIGHT)
            pygame.draw.rect(self.canvas, back, cell, border_radius=12)
            pygame.draw.rect(self.canvas, OUTLINE, cell, width=2, border_radius=12)
            img = avatar_surface(idx, mut, PICKER_CELL - 32)
            self.canvas.blit(img, img.get_rect(midtop=(cell.centerx, cell.y + 6)))
            label = fit_text(self.font_tiny, RARITIES[idx]["pet"], cell.width - 8)
            lt = self.font_tiny.render(label, True, BLACK if chosen else WHITE)
            self.canvas.blit(lt, lt.get_rect(midbottom=(cell.centerx, cell.bottom - 5)))
            self.register_button(cell, lambda i=idx, m=mut: self.set_avatar(i, m), "click")
        self.pop_clip()
        rows = (len(options) + per_row - 1) // per_row
        content_h = rows * (PICKER_CELL + PICKER_GAP) - PICKER_GAP
        self.friends_max_scroll = max(0.0, content_h - grid.height)
        self.friends_scroll = max(0.0, min(self.friends_max_scroll, self.friends_scroll))
        self.draw_scrollbar(grid, scroll, content_h, key="friends", mouse_pos=mouse_pos)
