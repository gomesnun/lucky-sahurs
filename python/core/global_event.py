"""Multiplicadores do evento global a decorrer (ver online/events.py).

O evento vem da cloud e vale para toda a gente ao mesmo tempo, mas os calculos da sorte e do
dinheiro estao no GameState, que nao conhece a parte online. Por isso o valor vive aqui, num
sitio so: online/events.py escreve, core/pets.py e core/economy.py leem."""

_MULTS = {"luck": 1.0, "money": 1.0, "speed": 1.0}


def set_event_mults(luck=1.0, money=1.0, speed=1.0):
    _MULTS["luck"] = max(1.0, float(luck))
    _MULTS["money"] = max(1.0, float(money))
    _MULTS["speed"] = max(1.0, float(speed))


def event_mult(kind):
    return _MULTS.get(kind, 1.0)
