"""Página de Credits: quem fez os sons e a música do jogo (licenças Creative Commons)."""

import webbrowser
import pygame

from config import VIRTUAL_H
from i18n import tr
from theme import ACCENT, BAD, GREY, GREY_DIM, PANEL, PANEL_LIGHT, PANEL_LIGHTER, WHITE
from ui.drawing import draw_panel, draw_state_border
from ui.fonts import fit_text, wrap_text

# (título, autor, licença, link, onde é usado no jogo)
# CC BY = é obrigatório dar o crédito ao autor (é isto que esta página faz); CC0 = domínio público.
CREDITS = (
    ("Level Up 01", "shinephoenixstormcrow  (original by rhodesmas)", "CC BY 3.0",
     "https://freesound.org/s/337049/", "buying upgrades"),
    ("Level Up 01", "mokasza", "CC BY 4.0",
     "https://freesound.org/s/810753/", "milestones"),
    ("Holy Protection Skill Buff", "EminYILDIRIM", "CC BY 4.0",
     "https://freesound.org/s/621206/", "rebirth (cut to the first 1.5 seconds)"),
    ("UI Button Click", "benzix2", "CC0",
     "https://freesound.org/s/467951/", "button clicks"),
    ("Lofi Hiphop Melody Loop 99 BPM", "holizna", "CC0",
     "https://freesound.org/s/629155/", "background music"),
)

CREDITS_NOTE = ("Click an entry to open its page on Freesound.org. The sounds were converted to .ogg. "
                "CC BY = Creative Commons Attribution; CC0 = public domain (no credit needed).")


class CreditsPanelMixin:
    """Página de Credits (abre-se no menu principal)."""

    # Como Stats / Options / Leaderboard: só uma página aberta de cada vez.
    def open_credits(self):
        self.close_overlays()
        self.credits_open = True

    def close_credits(self):
        self.credits_open = False

    def toggle_credits(self):
        if self.credits_open:
            self.close_credits()
        else:
            self.open_credits()

    def open_url(self, url):
        try:
            webbrowser.open(url)
        except Exception:
            self.show_toast(tr("Couldn't open the browser."))

    def draw_credits(self, mouse_pos):
        overlay = pygame.Surface((self.vw, VIRTUAL_H), pygame.SRCALPHA)
        overlay.fill((0, 0, 0, 170))
        self.canvas.blit(overlay, (0, 0))

        panel_w = 660
        row_h, gap = 76, 8
        note_lines = wrap_text(tr(CREDITS_NOTE), self.font_tiny, panel_w - 48)
        head_h = 86
        panel_h = head_h + len(CREDITS) * (row_h + gap) + 10 + 16 * len(note_lines) + 22
        rect = pygame.Rect(self.vw // 2 - panel_w // 2, VIRTUAL_H // 2 - panel_h // 2, panel_w, panel_h)
        draw_panel(self.canvas, rect, PANEL, radius=16)
        self.register_button(rect, lambda: None, None)     # clicar dentro da página não fecha

        title = self.font_big.render(tr("Credits"), True, WHITE)
        self.canvas.blit(title, (rect.x + 24, rect.y + 18))
        close_rect = pygame.Rect(rect.right - 46, rect.y + 20, 28, 28)
        self.button(close_rect, "X", self.font_small_b, mouse_pos, PANEL_LIGHT, BAD, WHITE,
                    callback=self.close_credits, radius=8)
        sub = self.font_small.render(tr("Sound effects and music, from Freesound.org"), True, GREY)
        self.canvas.blit(sub, (rect.x + 26, rect.y + 18 + title.get_height() + 2))

        y = rect.y + head_h
        for name, author, license_name, url, used_for in CREDITS:
            row = pygame.Rect(rect.x + 22, y, panel_w - 44, row_h)
            hovering = row.collidepoint(mouse_pos)
            pygame.draw.rect(self.canvas, PANEL_LIGHTER if hovering else PANEL_LIGHT, row, border_radius=10)
            if hovering:
                draw_state_border(self.canvas, row, ACCENT, 10)

            lic = self.font_small_b.render(license_name, True, ACCENT)
            self.canvas.blit(lic, (row.right - 16 - lic.get_width(), row.y + 10))
            title_txt = fit_text(self.font_med, name, row.width - 32 - lic.get_width() - 12)
            self.canvas.blit(self.font_med.render(title_txt, True, WHITE), (row.x + 16, row.y + 8))
            by = fit_text(self.font_small, tr("by %s", author), row.width - 32)
            self.canvas.blit(self.font_small.render(by, True, GREY), (row.x + 16, row.y + 34))
            short_url = url.replace("https://", "")
            line3 = fit_text(self.font_tiny, tr("Used for: %s   -   %s", tr(used_for), short_url), row.width - 32)
            self.canvas.blit(self.font_tiny.render(line3, True, GREY_DIM), (row.x + 16, row.y + 54))

            self.register_button(row, lambda u=url: self.open_url(u))
            y += row_h + gap

        ny = y + 2
        for line in note_lines:
            self.canvas.blit(self.font_tiny.render(line, True, GREY), (rect.x + 26, ny))
            ny += 16
