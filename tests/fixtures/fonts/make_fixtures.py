"""Regenerate the small font fixtures used by the font-format tests.

    python tests/fixtures/fonts/make_fixtures.py

Needs fontTools and the source fonts below (Debian/Ubuntu packages
``fonts-dejavu-core`` and ``lmodern``). The fixtures are committed so the
tests do not depend on the fonts installed on the machine.

- ``ferro-collection.ttc``: a TrueType collection of two faces, ASCII
  subsets of DejaVu Sans (face 0, family "Ferro TTC First") and DejaVu Serif
  (face 1, family "Ferro TTC Second").
- ``ferro-cff.otf``: a CFF-flavoured OpenType font, an ASCII subset of
  Latin Modern Roman 10 with its ligatures (family "Ferro CFF").

Faces are renamed so no system font can stand in for them (and as the GUST
Font License asks of modified Latin Modern fonts).
"""

from pathlib import Path

from fontTools import subset
from fontTools.ttLib import TTCollection, TTFont

HERE = Path(__file__).parent
ASCII = "".join(chr(c) for c in range(0x20, 0x7F))
DEJAVU = Path("/usr/share/fonts/truetype/dejavu")
LMODERN = Path("/usr/share/texmf/fonts/opentype/public/lm/lmroman10-regular.otf")


def subset_font(path: Path, text: str, features: list[str]) -> TTFont:
    font = TTFont(path)
    options = subset.Options()
    options.layout_features = features
    options.name_IDs = ["*"]
    options.notdef_outline = True
    options.hinting = False
    options.drop_tables += ["FFTM"]
    sub = subset.Subsetter(options)
    sub.populate(text=text)
    sub.subset(font)
    return font


def rename(font: TTFont, family: str) -> TTFont:
    ps = family.replace(" ", "") + "-Regular"
    names = {1: family, 2: "Regular", 3: ps, 4: f"{family} Regular", 6: ps, 16: family, 17: "Regular"}
    table = font["name"]
    table.names = [n for n in table.names if n.nameID not in names]
    for name_id, value in names.items():
        table.setName(value, name_id, 3, 1, 0x409)
        table.setName(value, name_id, 1, 0, 0)
    if "CFF " in font:
        cff = font["CFF "].cff
        cff.fontNames = [ps]
        top = cff.topDictIndex[0]
        top.FullName = f"{family} Regular"
        top.FamilyName = family
    return font


def main() -> None:
    first = rename(subset_font(DEJAVU / "DejaVuSans.ttf", ASCII, []), "Ferro TTC First")
    second = rename(subset_font(DEJAVU / "DejaVuSerif.ttf", ASCII, []), "Ferro TTC Second")
    collection = TTCollection()
    collection.fonts = [first, second]
    collection.save(HERE / "ferro-collection.ttc")

    cff = rename(subset_font(LMODERN, ASCII + "ﬀﬁﬂﬃﬄ", ["liga"]), "Ferro CFF")
    cff.save(HERE / "ferro-cff.otf")


if __name__ == "__main__":
    main()
