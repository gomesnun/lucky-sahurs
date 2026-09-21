"""Formatação de números, chances e tempo."""

from i18n import tr


def format_number(n):
    n = float(n)
    sign = "-" if n < 0 else ""
    n = abs(n)
    if n < 1000:
        if n < 10 and n != int(n):
            return "%s%.1f" % (sign, n)
        return "%s%.0f" % (sign, n)
    suffixes = ["", "K", "M", "B", "T", "Qa", "Qi", "Sx", "Sp", "Oc", "No", "Dc"]
    idx = 0
    while n >= 1000 and idx < len(suffixes) - 1:
        n /= 1000.0
        idx += 1
    return "%s%.2f%s" % (sign, n, suffixes[idx])


def format_one_in(chance):
    if chance <= 0:
        return "-"
    return tr("1 in %s", format_number(round(1.0 / chance)))


def format_playtime(seconds):
    seconds = int(max(0, seconds))
    d, rem = divmod(seconds, 86400)
    h, rem = divmod(rem, 3600)
    m, s = divmod(rem, 60)
    if d > 0:
        return "%dd %02dh" % (d, h)
    if h > 0:
        return "%dh %02dm" % (h, m)
    if m > 0:
        return "%dm %02ds" % (m, s)
    return "%ds" % s
