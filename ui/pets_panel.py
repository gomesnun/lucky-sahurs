"""Painéis dos pets: Index (chances) e Bag (Equipados / Inventory)."""

import pygame

from core.formatting import format_number, format_one_in
from core.pets import MUTATIONS, MUT_ORDER, PET_ORDER, RARITIES, RARITY_TIERS, base_pet_chance
from i18n import tr
from theme import (
    ACCENT, ACCENT_HOVER, BAD, BLACK,
    GOOD, GREY, GREY_DIM, PANEL, PANEL_LIGHT,
    PANEL_LIGHTER, WHITE,
)
from ui.drawing import bake, draw_panel
from ui.fonts import wrap_text


# Modos de ordenação do Inventory (o primeiro é o de sempre: do que dá mais dinheiro para o que dá menos)
INV_SORTS = [("money", "Money"), ("rarity", "Rarity"), ("mutation", "Mutation"), ("quantity", "Quantity")]
INV_MUT_FILTERS = [("all", "All"), ("normal", "Normal"), ("golden", "Golden"), ("diamond", "Diamond")]


class PetsPanelMixin:
    """Painéis dos pets: Index (chances) e Bag (Equipados / Inventory)."""

    # ---------------------------------------------------------------- painel: index
    def draw_index_panel(self, rect, mouse_pos):
        self.panel_header(rect, tr("Pet Index"), mouse_pos, self.right_panel.close)

        tabs = [("normal", tr("Normal")), ("golden", tr("Golden")), ("diamond", tr("Diamond"))]
        pad = 20
        tab_w = (rect.width - pad * 2 - 16) // 3
        tx = rect.x + pad
        ty = rect.y + 58
        for key, label in tabs:
            trect = pygame.Rect(tx, ty, tab_w, 30)
            active = self.index_tab == key
            self.button(trect, label, self.font_small_b, mouse_pos,
                        ACCENT if active else PANEL_LIGHT, ACCENT_HOVER if not active else ACCENT,
                        BLACK if active else WHITE,
                        callback=(lambda k=key: setattr(self, "index_tab", k)), radius=8)
            tx += tab_w + 8

        mutation = self.index_tab
        locked_mut = ((mutation == "golden" and self.state.upgrade_level("golden_unlock") < 1) or
                      (mutation == "diamond" and self.state.upgrade_level("diamond_unlock") < 1))

        content_rect = pygame.Rect(rect.x, ty + 40, rect.width, rect.bottom - (ty + 40))
        scroll = self.right_panel.get_scroll()
        self.push_clip(content_rect)

        top_y = content_rect.top + 6 - scroll
        if locked_mut:
            for line in wrap_text(tr("Mutation not unlocked yet. Buy the upgrade in Upgrades."),
                                  self.font_small, rect.width - pad * 2):
                self.canvas.blit(self.font_small.render(line, True, BAD), (rect.x + pad, top_y))
                top_y += 18
            top_y += 8

        cols = 2
        gap = 12
        card = (rect.width - pad * 2 - gap - 6) // cols   # quadrados

        live_weights = self.state.roll_weights(with_luck=True)     # calculado 1x por frame, não por cartão
        for pos, i in enumerate(PET_ORDER):        # ordenado por raridade (os pets novos ficam com a sua raridade)
            rarity = RARITIES[i]
            col = pos % cols
            row = pos // cols
            crect = pygame.Rect(rect.x + pad + col * (card + gap), top_y + row * (card + gap), card, card)
            if crect.bottom < content_rect.top - 4 or crect.top > content_rect.bottom + 4:
                continue

            owned = self.state.count_owned(i, mutation)
            # Só chances: em cima a chance ATUAL de este pet sair com esta mutação (com a tua sorte e as
            # tuas chances de mutação de agora) e em baixo, entre parênteses, a chance BASE de quem
            # acabou de começar (sem luck nenhuma). Diferente no Normal, Golden e Diamond.
            real_chance = self.state.combined_chance(i, mutation, with_luck=True, weights=live_weights)
            base_line = "(%s)" % format_one_in(base_pet_chance(i, mutation))
            income_line = tr("+%s/sec", format_number(self.state.pet_income(i, mutation)))     # dinheiro/seg deste pet

            plates = [[(tr("Income"), income_line)], [(tr("Chance"), format_one_in(real_chance), base_line)]]
            locked = locked_mut or owned <= 0
            card_surf = self.render_pet_card(rarity, mutation, card, card, plates=plates, locked=locked)
            self.canvas.blit(bake(card_surf, PANEL), crect.topleft)

        rows = (len(RARITIES) + cols - 1) // cols
        content_h = (top_y + scroll - content_rect.top) + rows * (card + gap) + 10
        self.right_panel.set_max_scroll(max(0, content_h - content_rect.height))
        self.pop_clip()
        self.draw_scrollbar(content_rect, scroll, content_h, key="right", mouse_pos=mouse_pos)

    # ---------------------------------------------------------------- painel: mochila
    def toggle_bag_view(self):
        """Interruptor da Bag: pets equipados <-> Inventory (todos os pets que tens)."""
        self.bag_view = "equipped" if self.bag_view == "inventory" else "inventory"
        self.left_panel.scroll["bag"] = 0.0

    def draw_bag_panel(self, rect, mouse_pos):
        self.panel_header(rect, tr("Bag"), mouse_pos, self.left_panel.close)

        sub = self.font_small.render(
            tr("%d/%d slots  ·  %s $/sec", len(self.state.equipped), self.state.max_slots(),
               format_number(self.state.income_per_second())), True, GREY)
        self.canvas.blit(bake(sub, PANEL), (rect.x + 22, rect.y + 50))

        # ---- Inventory / Equip Best / Auto Equip Best ----
        # O Auto Equip Best compra-se na Upgrade Tree (categoria Misc). Aqui só aparece o
        # interruptor quando já está desbloqueado; os botões ocupam a largura toda para o texto caber.
        pad = 20
        btn_row_h = 34
        btn_y = rect.y + 74
        btn_w = rect.width - pad * 2

        # interruptor (em cima): nos pets equipados mostra "Inventory"; no Inventory mostra "Equipped Pets"
        in_inventory = self.bag_view == "inventory"
        view_rect = pygame.Rect(rect.x + pad, btn_y, btn_w, btn_row_h)
        self.button(view_rect, tr("Equipped Pets") if in_inventory else tr("Inventory"),
                    self.font_small_b, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.toggle_bag_view, radius=9)

        def do_equip_best():
            self.state.equip_best()
            self.show_toast(tr("Equipped your best money-makers!"))

        equip_rect = pygame.Rect(rect.x + pad, view_rect.bottom + 8, btn_w, btn_row_h)
        self.button(equip_rect, tr("Equip Best"),
                    self.font_small_b, mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=do_equip_best, radius=9, sfx="equip")
        btn_bottom = equip_rect.bottom

        if self.state.auto_equip_unlocked():
            on = self.state.auto_equip_best_on

            def toggle_auto_equip():
                self.state.auto_equip_best_on = not self.state.auto_equip_best_on
                if self.state.auto_equip_best_on:
                    self.state.equip_best()

            auto_rect = pygame.Rect(rect.x + pad, btn_bottom + 8, btn_w, btn_row_h)
            self.button(auto_rect, tr("Auto Equip Best: %s", tr("ON") if on else tr("OFF")),
                        self.font_small_b, mouse_pos,
                        (52, 120, 80) if on else PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                        callback=toggle_auto_equip, radius=9)
            btn_bottom = auto_rect.bottom
        else:
            hint_y = btn_bottom + 8
            for line in wrap_text(tr("Auto Equip Best is unlocked in Upgrades (Misc)."),
                                  self.font_tiny, btn_w):
                self.canvas.blit(bake(self.font_tiny.render(line, True, GREY_DIM), PANEL), (rect.x + pad, hint_y))
                hint_y += 16
            btn_bottom = hint_y - 8

        if in_inventory:
            btn_bottom = self.draw_inventory_controls(rect, btn_bottom, pad, mouse_pos)

        content_top = btn_bottom + 14
        content_rect = pygame.Rect(rect.x, content_top, rect.width, rect.bottom - content_top)
        scroll = self.left_panel.get_scroll()
        self.push_clip(content_rect)
        if in_inventory:
            content_h = self.draw_bag_inventory(rect, content_rect, scroll, mouse_pos)
        else:
            content_h = self.draw_bag_equipped(rect, content_rect, scroll, mouse_pos, pad)
        self.left_panel.set_max_scroll(max(0, content_h - content_rect.height))
        self.pop_clip()
        self.draw_scrollbar(content_rect, scroll, content_h, key="left", mouse_pos=mouse_pos)

    def draw_bag_equipped(self, rect, content_rect, scroll, mouse_pos, pad):
        """Vista normal da Bag: os slots com os pets equipados (2 por fila). Devolve a altura total."""
        cols = 2
        gap = 12
        card = (rect.width - pad * 2 - gap - 6) // cols
        n_slots = self.state.max_slots()
        top_y = content_rect.top + 6 - scroll

        # mochila organizada do que dá mais dinheiro (topo) para o que dá menos (fundo);
        # a lista em si (self.state.equipped) mantém a sua ordem, só a exibição é ordenada.
        order = sorted(range(len(self.state.equipped)),
                       key=lambda idx: -self.state.pet_income(*self.state.equipped[idx]))

        for i in range(n_slots):
            col = i % cols
            row = i // cols
            crect = pygame.Rect(rect.x + pad + col * (card + gap), top_y + row * (card + gap), card, card)
            if crect.bottom < content_rect.top - 4 or crect.top > content_rect.bottom + 4:
                continue

            if i < len(order):
                actual_idx = order[i]
                r_idx, mutation = self.state.equipped[actual_idx]
                rarity = RARITIES[r_idx]
                income = self.state.pet_income(r_idx, mutation)
                plates = [[(tr("Income"), tr("+%s/sec", format_number(income)))]]
                surf = self.render_pet_card(rarity, mutation, card, card, plates=plates)
                self.canvas.blit(surf, crect.topleft)
                if crect.collidepoint(mouse_pos) and content_rect.collidepoint(mouse_pos):
                    pygame.draw.rect(self.canvas, WHITE, crect, width=3, border_radius=12)

                def make_cb(idx=actual_idx):
                    return lambda: self.state.remove_slot_at(idx)

                self.register_button(crect, make_cb(), "equip")
            else:
                draw_panel(self.canvas, crect, PANEL_LIGHT, radius=12, shadow=False)
                t = self.font_small.render(tr("empty"), True, GREY_DIM)
                self.canvas.blit(bake(t, PANEL_LIGHT), t.get_rect(center=crect.center))

        rows = (n_slots + cols - 1) // cols
        info_y = top_y + rows * (card + gap) + 4
        for line in wrap_text(tr("Click a pet to remove it. Open the Inventory to equip more."),
                              self.font_tiny, rect.width - pad * 2):
            self.canvas.blit(bake(self.font_tiny.render(line, True, GREY), PANEL), (rect.x + pad, info_y))
            info_y += 16

        return (info_y + scroll - content_rect.top) + 10

    # ---- ordenar / filtrar o Inventory ----
    def inventory_filter_active(self):
        return self.inv_mut != "all" or self.inv_tier is not None

    def _inventory_scroll_top(self):
        self.left_panel.scroll["bag"] = 0.0

    def cycle_inv_sort(self):
        keys = [k for k, _label in INV_SORTS]
        self.inv_sort = keys[(keys.index(self.inv_sort) + 1) % len(keys)]
        self._inventory_scroll_top()

    def toggle_inv_order(self):
        self.inv_high_first = not self.inv_high_first
        self._inventory_scroll_top()

    def set_inv_mut(self, key):
        self.inv_mut = key
        self._inventory_scroll_top()

    def step_inv_tier(self, step):
        """Filtro de raridade: All -> Common -> ... -> Transcendent -> All (step = +1 / -1)."""
        options = [None] + list(range(len(RARITY_TIERS)))
        pos = options.index(self.inv_tier)
        self.inv_tier = options[(pos + step) % len(options)]
        self._inventory_scroll_top()

    def draw_inventory_controls(self, rect, top, pad, mouse_pos):
        """Linhas de ordenar / filtrar por cima do Inventory. Devolve o y do fundo delas."""
        w = rect.width - pad * 2
        h = 28
        gap = 6

        def font_for(label, width):
            return self.font_small_b if self.font_small_b.size(label)[0] <= width - 10 else self.font_tiny_b

        # linha 1: ordenar por (clica para mudar) + ordem
        y = top + 8
        half = (w - gap) // 2
        sort_label = tr("Sort: %s", tr(dict(INV_SORTS)[self.inv_sort]))
        self.button(pygame.Rect(rect.x + pad, y, half, h), sort_label, font_for(sort_label, half),
                    mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=self.cycle_inv_sort, radius=8)
        order_label = tr("Highest first") if self.inv_high_first else tr("Lowest first")
        order_w = w - half - gap
        self.button(pygame.Rect(rect.x + pad + half + gap, y, order_w, h), order_label,
                    font_for(order_label, order_w), mouse_pos, PANEL_LIGHT, PANEL_LIGHTER, WHITE,
                    callback=self.toggle_inv_order, radius=8)

        # linha 2: filtro por mutação
        y += h + gap
        tab_w = (w - gap * 3) // 4
        for n, (key, label) in enumerate(INV_MUT_FILTERS):
            active = self.inv_mut == key
            label = tr(label)
            self.button(pygame.Rect(rect.x + pad + n * (tab_w + gap), y, tab_w, h), label,
                        font_for(label, tab_w), mouse_pos,
                        ACCENT if active else PANEL_LIGHT, ACCENT_HOVER if not active else ACCENT,
                        BLACK if active else WHITE, callback=(lambda k=key: self.set_inv_mut(k)), radius=8)

        # linha 3: filtro por raridade (< label >)
        y += h + gap
        arrow_w = 34
        self.button(pygame.Rect(rect.x + pad, y, arrow_w, h), "<", self.font_small_b, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=lambda: self.step_inv_tier(-1), radius=8)
        self.button(pygame.Rect(rect.right - pad - arrow_w, y, arrow_w, h), ">", self.font_small_b, mouse_pos,
                    PANEL_LIGHT, PANEL_LIGHTER, WHITE, callback=lambda: self.step_inv_tier(1), radius=8)
        mid = pygame.Rect(rect.x + pad + arrow_w + gap, y, w - 2 * (arrow_w + gap), h)
        if self.inv_tier is None:
            label, base, text_color = tr("Rarity: All"), PANEL_LIGHT, WHITE
        else:
            tier = RARITY_TIERS[self.inv_tier]
            label, base, text_color = tr("Rarity: %s", tr(tier["name"])), tuple(tier["color"]), tier["text"]
        self.button(mid, label, font_for(label, mid.width), mouse_pos, base,
                    PANEL_LIGHTER if self.inv_tier is None else base, text_color,
                    callback=lambda: self.step_inv_tier(1), radius=8)
        return y + h

    def inventory_entries(self):
        """Os pets que tens (cada pet + mutação) que passam nos filtros do Inventory, pela ordem escolhida
        (por defeito: do que dá mais dinheiro para o que dá menos)."""
        st = self.state
        entries = []
        for key, n in st.owned.items():
            try:
                idx_s, mut = key.split("_", 1)
                idx = int(idx_s)
                if int(n) > 0 and 0 <= idx < len(RARITIES) and mut in MUTATIONS:
                    if self.inv_mut != "all" and mut != self.inv_mut:
                        continue
                    if self.inv_tier is not None and RARITIES[idx]["tier"] != self.inv_tier:
                        continue
                    entries.append((idx, mut))
            except (ValueError, TypeError):
                continue

        rank = {pet: pos for pos, pet in enumerate(PET_ORDER)}     # posição por raridade (o 2.º pet da raridade vem depois)

        # a ordem não depende dos multiplicadores globais de dinheiro, por isso basta o rendimento-base
        def money(e):
            return RARITIES[e[0]]["income"] * MUTATIONS[e[1]]["mult"]

        def primary(e):
            if self.inv_sort == "rarity":
                return rank[e[0]]
            if self.inv_sort == "mutation":
                return MUT_ORDER.index(e[1])
            if self.inv_sort == "quantity":
                return st.count_owned(e[0], e[1])
            return money(e)

        # "Highest first": maior primeiro; desempates pelo que dá mais dinheiro, pela raridade e pela mutação
        entries.sort(key=lambda e: (-primary(e), -money(e), -rank[e[0]], -MUT_ORDER.index(e[1])))
        if not self.inv_high_first:
            entries.reverse()
        return entries

    def draw_bag_inventory(self, rect, content_rect, scroll, mouse_pos):
        """Vista Inventory da Bag: todos os pets que tens (todas as mutações juntas), 3 por fila, com
        + / - para equipar e desequipar. Textos mais pequenos para caberem. Devolve a altura total."""
        st = self.state
        entries = self.inventory_entries()
        ipad = 14
        gap = 8
        cols = 3
        card_w = (rect.width - ipad * 2 - gap * (cols - 1) - 6) // cols
        card_h = 196          # um pouco mais alto (era 178) para a imagem do verity caber por cima do nome
        top_y = content_rect.top + 6 - scroll

        if not entries:
            msg = (tr("No pets match these filters.") if self.inventory_filter_active() and st.owned
                   else tr("You don't have any pets yet. Roll some!"))
            for k, line in enumerate(wrap_text(msg, self.font_small, rect.width - ipad * 2)):
                self.canvas.blit(self.font_small.render(line, True, GREY), (rect.x + ipad, top_y + k * 20))
            return 80

        for n, (idx, mut) in enumerate(entries):
            col = n % cols
            row = n // cols
            crect = pygame.Rect(rect.x + ipad + col * (card_w + gap), top_y + row * (card_h + gap),
                                card_w, card_h)
            if crect.bottom < content_rect.top - 4 or crect.top > content_rect.bottom + 4:
                continue

            owned = st.count_owned(idx, mut)
            eq_count = st.equipped_count(idx, mut)
            income = st.pet_income(idx, mut)
            plates = [[(tr("Income"), tr("+%s/sec", format_number(income)))],
                      [(tr("Have"), format_number(owned)), (tr("Equipped"), str(eq_count))]]
            surf = self.render_pet_card(RARITIES[idx], mut, card_w, card_h, plates=plates, footer_h=30)
            self.canvas.blit(surf, crect.topleft)

            bw = (card_w - 18) // 2
            minus_rect = pygame.Rect(crect.x + 6, crect.bottom - 28, bw, 22)
            plus_rect = pygame.Rect(crect.right - 6 - bw, crect.bottom - 28, bw, 22)
            can_minus = eq_count > 0
            can_plus = eq_count < owned and len(st.equipped) < st.max_slots()

            def make_minus(i=idx, m=mut):
                return lambda: st.equip_remove_one(i, m)

            def make_plus(i=idx, m=mut):
                return lambda: st.equip_add(i, m)

            self.button(minus_rect, "-", self.font_small_b, mouse_pos, (38, 40, 52), BAD, WHITE,
                        callback=make_minus() if can_minus else None, enabled=can_minus, radius=7,
                        sfx="equip")
            self.button(plus_rect, "+", self.font_small_b, mouse_pos, (38, 40, 52), GOOD, WHITE,
                        callback=make_plus() if can_plus else None, enabled=can_plus, radius=7,
                        sfx="equip")

        rows = (len(entries) + cols - 1) // cols
        return (top_y + scroll - content_rect.top) + rows * (card_h + gap) + 10
