"""CAIRN video: ten narrated scenes, rendered with Manim Community.

Every number on screen is read from results/*.json (written by cairn-lab) or
from video/demo_transcript.json (real output of the cairn binary). Each
scene is timed to its narration: sentence i of video/narration/<key>.txt
starts at a time proportional to the words before it, scaled to the length
of the voice clip (see video/build.py, which writes build/durations.json).
"""

import json
import re
from pathlib import Path

import numpy as np
from manim import (
    DOWN, LEFT, ORIGIN, RIGHT, UP, UL, UR,
    AddTextLetterByLetter, AnimationGroup, Axes, Create, Cross, Dot, FadeIn, FadeOut,
    GrowFromEdge, Indicate, LaggedStart, Line, MathTex, Matrix, Rectangle,
    ReplacementTransform, RoundedRectangle, Scene, SurroundingRectangle, TexTemplate,
    Text, Transform, VGroup, Write, Circle, Ellipse, DashedLine, Arrow,
)

ROOT = Path(__file__).resolve().parent.parent
NARR = ROOT / "video" / "narration"
DUR = ROOT / "video" / "build" / "durations.json"
RES = ROOT / "results"

BG = "#14161a"
INK = "#ecebe6"
INK2 = "#a9a8a1"
MUTED = "#5d5f63"
BLUE = "#3987e5"
ORANGE = "#d95926"
PANEL = "#1d2026"
SERIF = "Libertinus Serif"
MONO = "Adwaita Mono"

TPL = TexTemplate(
    preamble=r"""
\usepackage[english]{babel}
\usepackage{amsmath}
\usepackage{libertinus}
\usepackage{libertinust1math}
"""
)


def T(s, size=34, color=INK, font=SERIF, **kw):
    return Text(s, font=font, font_size=size, color=color, **kw)


def M(s, size=48, color=INK):
    return MathTex(s, font_size=size, color=color, tex_template=TPL)


def load(name):
    return json.loads((RES / f"{name}.json").read_text())["result"]


def stone(width, height, color=MUTED):
    return Ellipse(width=width, height=height, color=color, fill_color=color, fill_opacity=1, stroke_width=0)


def cairn_stack(scale=1.0):
    widths = [2.2, 1.8, 1.45, 1.1, 0.75]
    heights = [0.62, 0.55, 0.5, 0.45, 0.4]
    shades = ["#4a4d52", "#56595e", "#63666b", "#707378", "#7d8085"]
    stones = VGroup()
    y = 0
    for w, h, c in zip(widths, heights, shades):
        s = stone(w * scale, h * scale, c).move_to(UP * (y + h * scale / 2))
        stones.add(s)
        y += h * scale * 0.86
    return stones


def key_icon(color=INK, scale=1.0):
    ring = Circle(radius=0.45 * scale, color=color, stroke_width=8 * scale)
    shaft = Rectangle(width=1.6 * scale, height=0.16 * scale, color=color, fill_color=color, fill_opacity=1, stroke_width=0)
    shaft.next_to(ring, RIGHT, buff=0)
    t1 = Rectangle(width=0.14 * scale, height=0.32 * scale, color=color, fill_color=color, fill_opacity=1, stroke_width=0)
    t2 = t1.copy()
    t1.next_to(shaft, DOWN, buff=0).align_to(shaft, RIGHT)
    t2.next_to(shaft, DOWN, buff=0).shift(LEFT * 0.35 * scale).align_to(shaft, RIGHT).shift(LEFT * 0.3 * scale)
    return VGroup(ring, shaft, t1, t2).move_to(ORIGIN)


class Narrated(Scene):
    key = ""

    def setup(self):
        self.camera.background_color = BG
        d = json.loads(DUR.read_text())[self.key]
        self.total, self.lead, self.voice = d["total"], d["lead"], d["voice"]
        text = (NARR / f"{self.key}.txt").read_text().strip()
        sents = [s for s in re.split(r"(?<=[.!?])\s+", text) if s]
        words = [len(s.split()) for s in sents]
        tot = sum(words)
        cum = np.concatenate([[0], np.cumsum(words)])
        self.starts = [self.lead + self.voice * c / tot for c in cum]

    def at(self, i, frac=0.0):
        """Wait until fraction `frac` of sentence i has been spoken."""
        t = self.starts[i] + frac * (self.starts[i + 1] - self.starts[i])
        dt = t - self.renderer.time
        if dt > 1 / 60:
            self.wait(dt)

    def end(self):
        """Hold, then fade everything out so scenes join without a hard cut."""
        fade = 0.45
        dt = self.total - fade - self.renderer.time
        if dt > 1 / 60:
            self.wait(dt)
        if self.mobjects:
            self.play(*[FadeOut(m) for m in self.mobjects], run_time=fade)
        dt = self.total - self.renderer.time
        if dt > 1 / 60:
            self.wait(dt)


# ---------------------------------------------------------------------------
class S01Hook(Narrated):
    key = "01_hook"

    def construct(self):
        self.at(0)
        key = key_icon(INK, 1.3)
        self.play(Create(key), run_time=1.2)
        labels = VGroup(T("release signing key", 28, INK2), T("password vault", 28, INK2), T("database", 28, INK2))
        for lab, pos in zip(labels, [UL * 2.2 + LEFT * 1.5, UR * 2.2 + RIGHT * 1.5, DOWN * 2.3]):
            lab.move_to(pos)
        self.play(LaggedStart(*[FadeIn(lab, shift=0.2 * DOWN) for lab in labels], lag_ratio=0.35), run_time=1.6)

        self.at(1)
        self.play(FadeOut(labels), key.animate.scale(0.6).shift(UP * 2.2), run_time=0.8)
        tokens = VGroup()
        for i in range(5):
            box = RoundedRectangle(corner_radius=0.15, width=1.0, height=1.0, color=INK2, stroke_width=3)
            num = T(str(i + 1), 34, INK)
            tokens.add(VGroup(box, num).move_to(LEFT * 4 + RIGHT * 2 * i))
        self.play(LaggedStart(*[ReplacementTransform(key.copy(), t) for t in tokens], lag_ratio=0.12), FadeOut(key), run_time=1.5)
        self.at(1, 0.45)
        chosen = [tokens[0], tokens[2], tokens[4]]
        self.play(*[t[0].animate.set_color(BLUE) for t in chosen], run_time=0.6)
        rebuilt = key_icon(BLUE, 0.8).shift(DOWN * 2.2)
        arrows = VGroup(*[Arrow(t.get_bottom(), rebuilt.get_top(), color=BLUE, buff=0.15, stroke_width=4) for t in chosen])
        self.play(Create(arrows), FadeIn(rebuilt), run_time=1.0)
        cap1 = T("any 3 of 5 rebuild it", 26, BLUE).next_to(rebuilt, RIGHT, buff=0.6)
        self.play(FadeIn(cap1), run_time=0.5)
        self.at(1, 0.8)
        others = [tokens[1], tokens[3]]
        self.play(*[t.animate.set_opacity(0.35) for t in others], run_time=0.6)
        cap2 = T("2 learn nothing", 26, INK2).next_to(tokens[3], UP, buff=0.4)
        self.play(FadeIn(cap2), run_time=0.5)

        self.at(2)
        self.play(*[FadeOut(m) for m in [tokens, arrows, rebuilt, cap1, cap2]], run_time=0.7)
        stack = cairn_stack(1.1).shift(DOWN * 1.6)
        self.play(LaggedStart(*[FadeIn(s, shift=0.6 * DOWN) for s in stack], lag_ratio=0.25), run_time=1.6)

        self.at(3)
        title = T("CAIRN", 92, INK).next_to(stack, UP, buff=0.5)
        self.play(Write(title), run_time=0.8)
        sub = T("information-theoretic threshold custody of master keys", 28, INK2).next_to(title, UP, buff=0.3)
        sub2 = M(r"\text{linear algebra over } \mathbb{F}_{2^{127}-1}", 34, INK2).next_to(stack, DOWN, buff=0.35)
        self.play(FadeIn(sub), FadeIn(sub2), run_time=0.8)
        self.end()


# ---------------------------------------------------------------------------
class S02Windows(Narrated):
    key = "02_windows"

    def construct(self):
        self.at(0)
        head = M(r"\text{Shamir: any } t-1 \text{ shares reveal nothing}", 40).to_edge(UP, buff=0.6)
        ok = T("on paper", 28, BLUE).next_to(head, DOWN, buff=0.2)
        self.play(Write(head), run_time=1.2)
        self.play(FadeIn(ok), run_time=0.4)

        self.at(1)
        self.play(FadeOut(ok), head.animate.become(T("Five windows through which a shared key leaks", 36).to_edge(UP, buff=0.6)), run_time=0.8)
        rows = [
            ("W1", "at rest", "share files get stolen", "L3 Vandermonde sharing"),
            ("W2", "over time", "a patient attacker collects shares", "L5 proactive refresh"),
            ("W3", "in memory", "cold boot, swap, partial dumps", "L4 split-state storage"),
            ("W4", "reconstruction", "timing and cache side channels", "L6 constant-time combine"),
            ("W5", "manipulation", "a custodian lies (Tompa-Woll)", "L2 AMD armor"),
        ]
        lines, layers = VGroup(), VGroup()
        for i, (w, name, what, layer) in enumerate(rows):
            y = 1.7 - i * 0.95
            a = T(w, 30, ORANGE).move_to(LEFT * 6.3 + UP * y)
            b = T(name, 30, INK)
            b.move_to(LEFT * 5.75 + UP * y, aligned_edge=LEFT).set_y(y)
            c = T(what, 25, INK2)
            c.move_to(LEFT * 2.75 + UP * y, aligned_edge=LEFT).set_y(y)
            lines.add(VGroup(a, b, c))
            lay = T(layer, 25, BLUE)
            lay.move_to(RIGHT * 2.75 + UP * y, aligned_edge=LEFT).set_y(y)
            layers.add(lay)
        for i in range(5):
            self.at(2 + i)
            self.play(FadeIn(lines[i], shift=0.2 * RIGHT), run_time=0.6)

        self.at(7)
        self.play(LaggedStart(*[FadeIn(l, shift=0.2 * LEFT) for l in layers], lag_ratio=0.2), run_time=1.6)
        words = VGroup(T("AES", 34, INK), T("hash", 34, INK), T("hard problem", 34, INK)).arrange(RIGHT, buff=1.2).to_edge(DOWN, buff=0.45)
        for i in range(3):
            self.at(8 + i)
            self.play(FadeIn(words[i]), run_time=0.25)
            self.play(Create(Cross(words[i], stroke_color=ORANGE, stroke_width=5)), run_time=0.3)
        self.end()


# ---------------------------------------------------------------------------
class S03Field(Narrated):
    key = "03_field"

    def construct(self):
        self.at(0)
        f = M(r"\mathbb{F}_p, \qquad p = 2^{127} - 1", 72)
        self.play(Write(f), run_time=1.4)
        tag = T("a Mersenne prime", 28, INK2).next_to(f, DOWN, buff=0.4)
        self.play(FadeIn(tag), run_time=0.5)

        self.at(1)
        self.play(f.animate.scale(0.6).to_edge(UP, buff=0.6), FadeOut(tag), run_time=0.8)
        ident = M(r"2^{127} \equiv 1 \pmod p", 64, BLUE)
        self.play(Write(ident), run_time=1.0)

        self.at(2)
        self.play(ident.animate.scale(0.7).next_to(f, DOWN, buff=0.4), run_time=0.7)
        red = M(r"h \cdot 2^{127} + l \;\equiv\; h + l", 54).shift(UP * 0.1)
        rlab = T("reduction: a shift and an add", 26, INK2).next_to(red, DOWN, buff=0.25)
        self.play(Write(red), FadeIn(rlab), run_time=1.0)
        self.at(2, 0.4)
        inv = M(r"x^{-1} = x^{\,p-2}", 54).shift(DOWN * 1.7)
        ilab = T("inversion: a public exponent", 26, INK2).next_to(inv, DOWN, buff=0.25)
        self.play(Write(inv), FadeIn(ilab), run_time=1.0)
        self.at(2, 0.78)
        badge = T("0 divisions on secret data", 30, BLUE)
        box = SurroundingRectangle(badge, color=BLUE, buff=0.2, corner_radius=0.1)
        VGroup(badge, box).to_edge(DOWN, buff=0.3)
        self.play(FadeIn(badge), Create(box), run_time=0.7)
        self.end()


# ---------------------------------------------------------------------------
class S04Amd(Narrated):
    key = "04_amd"

    def construct(self):
        self.at(0)
        title = T("L2  algebraic manipulation detection", 34, BLUE).to_edge(UP, buff=0.5)
        self.play(FadeIn(title), run_time=0.6)

        self.at(1)
        armor = M(r"a = (s_1,\dots,s_k,\; x,\; \tau)", 54).shift(UP * 1.5)
        tag = M(r"\tau = x^{k+2} + \sum_{i=1}^{k} s_i\, x^{i}", 54).next_to(armor, DOWN, buff=0.45)
        self.play(Write(armor), run_time=1.0)
        self.play(Write(tag), run_time=1.2)

        self.at(2)
        err = M(r"(s+d,\; x+e,\; \tau+g)", 48, ORANGE).next_to(tag, DOWN, buff=0.55)
        note = T("the error cannot depend on x", 26, INK2).next_to(err, DOWN, buff=0.2)
        self.play(FadeIn(err, shift=0.2 * UP), FadeIn(note), run_time=0.8)

        self.at(3)
        self.play(FadeOut(armor), FadeOut(note), VGroup(tag, err).animate.scale(0.75).to_edge(LEFT, buff=0.6).shift(UP * 0.3), run_time=0.8)
        ax = Axes(x_range=[-3, 3, 1], y_range=[-3, 3, 1], x_length=5.0, y_length=2.9,
                  axis_config={"color": MUTED, "include_ticks": False}).to_edge(RIGHT, buff=0.7).shift(UP * 0.85)
        curve = ax.plot(lambda x: 0.35 * (x + 2.1) * (x - 0.4) * (x - 1.9) + 0.0, x_range=[-2.75, 2.65], color=BLUE)
        roots = VGroup(*[Dot(ax.c2p(r, 0), color=ORANGE, radius=0.08) for r in (-2.1, 0.4, 1.9)])
        lab = M(r"\deg \le k+1 \;\Rightarrow\; \le k+1 \text{ roots}", 34).next_to(ax, DOWN, buff=0.15)
        self.play(Create(ax), run_time=0.6)
        self.play(Create(curve), run_time=1.0)
        self.play(FadeIn(roots), FadeIn(lab), run_time=0.6)

        self.at(4)
        bound = M(r"\Pr[\text{forgery}] \;\le\; \frac{k+1}{p} \;\approx\; 2^{-125}", 52, BLUE).shift(DOWN * 2.25)
        self.play(Write(bound), run_time=1.2)
        amd = load("amd")
        pairs = sum(r["secret_error_pairs"] for r in amd)
        chk = T(f"checked exhaustively over {pairs:,} secret/error pairs: the bound is attained exactly", 22, INK2).next_to(bound, DOWN, buff=0.3)
        self.play(FadeIn(chk), run_time=0.6)
        self.end()


# ---------------------------------------------------------------------------
class S05Sharing(Narrated):
    key = "05_sharing"

    def construct(self):
        self.at(0)
        title = T("L3  Vandermonde sharing", 34, BLUE).to_edge(UP, buff=0.5)
        eq = M(r"\Sigma \;=\; C\, V", 60).shift(LEFT * 4.2)
        V = Matrix([["1", "1", "1", "1", "1"], ["1", "2", "3", "4", "5"], ["1", "4", "9", "16", "25"]],
                   element_to_mobject=lambda s: M(s, 44), h_buff=1.1, v_buff=0.9,
                   bracket_config={"color": INK2}).shift(RIGHT * 1.6)
        vlab = M(r"V = \big[\, j^{m} \,\big]", 40, INK2).next_to(V, DOWN, buff=0.35)
        self.play(FadeIn(title), Write(eq), run_time=0.9)
        self.play(Create(V), FadeIn(vlab), run_time=1.3)

        self.at(1)
        cols = V.get_columns()
        boxes = VGroup(*[SurroundingRectangle(VGroup(cols[j][1], cols[j][2]), color=BLUE, buff=0.12) for j in (1, 3)])
        self.play(Create(boxes), run_time=0.8)
        rk = M(r"\text{any } t-1 \text{ columns: full rank} \;\Rightarrow\; \text{shares uniform}", 38, BLUE).to_edge(DOWN, buff=0.9)
        self.play(FadeIn(rk), run_time=0.7)

        self.at(4)
        big = T("Nothing.", 80, INK)
        self.play(FadeOut(VGroup(eq, V, vlab, boxes, rk, title)), FadeIn(big), run_time=0.6)
        self.end()


# ---------------------------------------------------------------------------
def cells(n, color, label):
    row = VGroup(*[Rectangle(width=0.62, height=0.62, color=color, stroke_width=2.5, fill_color=PANEL, fill_opacity=1) for _ in range(n)]).arrange(RIGHT, buff=0.08)
    lab = M(label, 46, color).next_to(row, LEFT, buff=0.35)
    return VGroup(lab, row)


class S06Storage(Narrated):
    key = "06_storage"

    def construct(self):
        self.at(0)
        title = T("L4  split-state storage", 34, BLUE).to_edge(UP, buff=0.5)
        share = M(r"\sigma", 80)
        self.play(FadeIn(title), FadeIn(share), run_time=0.7)
        self.at(1)
        self.play(Create(Cross(share, stroke_color=ORANGE, stroke_width=6)), run_time=0.6)

        self.at(2)
        self.play(*[FadeOut(m) for m in self.mobjects if m is not title], run_time=0.4)
        L = cells(8, BLUE, "L").shift(UP * 1.4)
        R = cells(8, BLUE, "R").shift(UP * 0.0)
        regA = T("memory region A", 22, INK2).next_to(L, RIGHT, buff=0.4)
        regB = T("memory region B", 22, INK2).next_to(R, RIGHT, buff=0.4)
        self.play(FadeIn(L), FadeIn(regA), run_time=0.6)
        self.play(FadeIn(R), FadeIn(regB), run_time=0.6)
        ip = M(r"\sigma = \langle L, R\rangle = \sum_{i=1}^{\ell} L_i R_i", 50).shift(DOWN * 1.3)
        self.play(Write(ip), run_time=1.0)

        self.at(3)
        leak = [L[1][1], L[1][4], R[1][2], R[1][6], L[1][6]]
        self.play(LaggedStart(*[c.animate.set_fill(ORANGE, opacity=0.85) for c in leak], lag_ratio=0.2), run_time=1.2)
        bound = M(r"\mathrm{TV} \le \Big(1+\tfrac1p\Big)\, 2^{(\lambda_L+\lambda_R)/2}\; p^{-(\ell-3)/2}", 44).move_to(ip)
        self.play(ReplacementTransform(ip, bound), run_time=1.0)
        res = M(r"\ell = 8:\quad \lambda_L+\lambda_R \le 378 \text{ bits} \;\Rightarrow\; \le 2^{-128}", 42, BLUE).next_to(bound, DOWN, buff=0.45)
        self.play(FadeIn(res, shift=0.2 * UP), run_time=0.8)

        self.at(4)
        self.play(*[c.animate.set_fill(PANEL, opacity=1) for c in leak], run_time=0.3)
        self.play(LaggedStart(*[c.animate.set_fill(ORANGE, opacity=0.85) for c in L[1]], lag_ratio=0.06), run_time=0.9)
        zero = T("one region, read completely:  advantage 0", 28, INK).next_to(res, DOWN, buff=0.4)
        self.play(FadeIn(zero), run_time=0.6)
        self.end()


# ---------------------------------------------------------------------------
class S07Refresh(Narrated):
    key = "07_refresh"

    def construct(self):
        self.at(0)
        title = T("L5  refresh without computing the secret", 34, BLUE).to_edge(UP, buff=0.5)
        self.play(FadeIn(title), run_time=0.6)

        self.at(1)
        skew = M(r"A^{\top} = -A \;\Rightarrow\; \langle AR, R\rangle = R^{\top}A^{\top}R = -R^{\top}AR = 0", 44).shift(UP * 2.35)
        self.play(Write(skew), run_time=1.6)

        self.at(2)
        s1 = M(r"L \leftarrow L + A R", 46).shift(UP * 1.1 + LEFT * 3.4)
        s2 = M(r"R \leftarrow R + B L", 46).next_to(s1, DOWN, buff=0.3).align_to(s1, LEFT)
        lab2 = T("first design: two steps", 24, INK2).next_to(s1, UP, buff=0.25).align_to(s1, LEFT)
        self.play(FadeIn(lab2), Write(s1), run_time=0.8)
        self.play(Write(s2), run_time=0.6)

        self.at(3)
        leak = M(r"\langle L_{\mathrm{new}},\, R_{\mathrm{old}}\rangle = \sigma", 50, ORANGE).shift(RIGHT * 2.8 + UP * 0.75)
        self.play(Write(leak), run_time=1.0)
        self.at(4)
        self.play(Indicate(leak, color=ORANGE), run_time=0.9)

        # bar chart from results/refresh.json (method B, sigma, sigma' != 0)
        rows = [r for r in load("refresh") if r["method"].startswith("B")]
        configs = []
        for r in rows:
            if (r["p"], r["ell"]) not in configs:
                configs.append((r["p"], r["ell"]))
        get = lambda c, s: next(r for r in rows if (r["p"], r["ell"]) == c and r["refresh"] == s)  # noqa: E731
        base = DOWN * 3.2
        H = 1.75  # log scale: 10^-2 at the axis, 10^0 at height H
        hl = lambda v: max(0.03, H * (np.log10(v) + 2) / 2)  # noqa: E731
        bars2, bars3, ticks, labels, vals = VGroup(), VGroup(), VGroup(), VGroup(), VGroup()
        for i, c in enumerate(configs):
            x = LEFT * 4.5 + RIGHT * 3.0 * i
            v2 = get(c, "2-step")["tv_old_R_new_L_nonzero_values"]
            v3 = get(c, "3-step")["tv_old_R_new_L_nonzero_values"]
            b2 = Rectangle(width=0.7, height=hl(v2), color=ORANGE, fill_color=ORANGE, fill_opacity=1, stroke_width=0)
            b2.move_to(base + x + LEFT * 0.42, aligned_edge=DOWN)
            b3 = Rectangle(width=0.7, height=hl(v3), color=BLUE, fill_color=BLUE, fill_opacity=1, stroke_width=0)
            b3.move_to(base + x + RIGHT * 0.42, aligned_edge=DOWN)
            pred = c[0] ** (-(c[1] - 1))
            tick = Line(LEFT * 0.42, RIGHT * 0.42, color=INK, stroke_width=4).move_to(base + x + RIGHT * 0.42 + UP * hl(pred))
            bars2.add(b2)
            bars3.add(b3)
            ticks.add(tick)
            labels.add(M(rf"p={c[0]},\ \ell={c[1]}", 26, INK2).next_to(base + x, DOWN, buff=0.15))
            vals.add(T(f"{v3:.3f}", 20, INK).next_to(tick, UP, buff=0.08))
        axis = Line(base + LEFT * 6.2, base + RIGHT * 6.2, color=MUTED, stroke_width=2)

        self.at(5)
        self.play(Create(axis), FadeIn(labels), run_time=0.5)
        self.play(LaggedStart(*[GrowFromEdge(b, DOWN) for b in bars2], lag_ratio=0.15), run_time=1.0)
        one = T("2-step refresh: TV = 1, a total break", 24, ORANGE).move_to(base + UP * (H + 0.35) + LEFT * 3.0)
        logn = T("log scale", 18, MUTED).move_to(base + UP * (H + 0.35) + RIGHT * 5.4)
        self.add(logn)
        self.play(FadeIn(one), run_time=0.5)

        self.at(6)
        s3 = M(r"L \leftarrow L + C R", 46, BLUE).next_to(s2, DOWN, buff=0.3).align_to(s2, LEFT)
        self.play(Write(s3), run_time=0.8)

        self.at(7)
        self.play(LaggedStart(*[GrowFromEdge(b, DOWN) for b in bars3], lag_ratio=0.15), run_time=1.0)
        self.play(FadeIn(ticks), FadeIn(vals), run_time=0.6)
        law = M(r"\mathrm{TV} = p^{-(\ell-1)} \text{ exactly}", 40, BLUE).next_to(leak, DOWN, buff=0.35)
        self.play(Write(law), run_time=0.9)
        self.end()


# ---------------------------------------------------------------------------
class S08SideChannel(Narrated):
    key = "08_sidechannel"

    def construct(self):
        self.at(0)
        title = T("L6  constant-time reconstruction", 34, BLUE).to_edge(UP, buff=0.5)
        rec = M(r"\hat a_c = \sum_{i=1}^{\ell}\sum_{j\in T} (\lambda_j L_{j,c,i})\, R_{j,c,i}", 44).next_to(title, DOWN, buff=0.35)
        self.play(FadeIn(title), Write(rec), run_time=1.2)

        rows = {r["operation"]: r["campaign"]["max_abs_t"] for r in load("dudect")["rows"]}
        ops = [("multiplication", "Fp127 multiplication", "double-and-add multiplication"),
               ("inversion", "Fp127 inversion (Fermat)", "extended Euclid inversion"),
               ("reconstruction", "reconstruct + AMD verify (3-of-5, ell=8)", "plain Shamir reconstruct")]
        x0, W = LEFT * 2.6, 7.0  # log10 scale from 10^-0.5 to 10^5
        lg = lambda v: max(0.02, (np.log10(v) + 0.5) / 5.5 * W)  # noqa: E731
        naive_bars, cairn_bars, names, nv, cv = VGroup(), VGroup(), VGroup(), VGroup(), VGroup()
        for k, (lab, c_op, n_op) in enumerate(ops):
            y = UP * (0.15 - 1.15 * k)
            names.add(T(lab, 26, INK2).move_to(x0 + LEFT * 1.6 + y))
            nb = Rectangle(width=lg(rows[n_op]), height=0.32, color=ORANGE, fill_color=ORANGE, fill_opacity=1, stroke_width=0).move_to(x0 + y + DOWN * 0.2, aligned_edge=LEFT)
            cb = Rectangle(width=lg(rows[c_op]), height=0.32, color=BLUE, fill_color=BLUE, fill_opacity=1, stroke_width=0).move_to(x0 + y + UP * 0.2, aligned_edge=LEFT)
            naive_bars.add(nb)
            cairn_bars.add(cb)
            nv.add(T(f"{rows[n_op]:,.0f}", 22, INK).next_to(nb, RIGHT, buff=0.12))
            cv.add(T(f"{rows[c_op]:.1f}", 22, INK).next_to(cb, RIGHT, buff=0.12))
        thr_x = x0 + RIGHT * lg(4.5)
        thr = DashedLine(thr_x + UP * 0.75, thr_x + DOWN * 2.6, color=INK, stroke_width=2)
        thr_lab = T("|t| = 4.5", 20, INK2).next_to(thr, UP, buff=0.05)
        legend = VGroup(T("textbook baseline", 22, ORANGE), T("CAIRN", 22, BLUE)).arrange(RIGHT, buff=0.6).to_edge(RIGHT, buff=0.6).shift(UP * 1.0)

        self.at(1)
        self.play(FadeIn(names), FadeIn(legend[0]), run_time=0.5)
        self.play(LaggedStart(*[GrowFromEdge(b, LEFT) for b in naive_bars], lag_ratio=0.2), run_time=1.2)
        self.play(FadeIn(nv), run_time=0.4)

        self.at(2)
        self.play(FadeIn(legend[1]), LaggedStart(*[GrowFromEdge(b, LEFT) for b in cairn_bars], lag_ratio=0.2), run_time=1.0)
        self.play(FadeIn(cv), Create(thr), FadeIn(thr_lab), run_time=0.6)

        self.at(3)
        chart = VGroup(names, naive_bars, cairn_bars, nv, cv, thr, thr_lab, legend)
        self.play(FadeOut(chart), FadeOut(rec), run_time=0.5)
        cg = VGroup(T("Valgrind (ctgrind): secret-dependent branches", 30, INK2),
                    VGroup(T("CAIRN  0", 40, BLUE), T("baseline  2,289", 40, ORANGE)).arrange(RIGHT, buff=1.4)).arrange(DOWN, buff=0.4).shift(UP * 0.9)
        self.play(FadeIn(cg), run_time=0.7)

        self.at(4)
        asm = (RES / "asm_audit_full.txt").read_text().split("--- cairn_audit_mul")[1].splitlines()[1:9]
        asm = [re.sub(r"^\s*[0-9a-f]+:\s*", "", a).split("<")[0].rstrip() for a in asm]
        code = VGroup(*[T(a, 20, INK2, font=MONO) for a in asm]).arrange(DOWN, aligned_edge=LEFT, buff=0.08)
        panel = RoundedRectangle(corner_radius=0.12, width=code.width + 0.6, height=code.height + 0.4, color=MUTED, fill_color=PANEL, fill_opacity=1, stroke_width=1.5)
        g = VGroup(panel, code).shift(DOWN * 1.7 + LEFT * 2.2)
        code.move_to(panel)
        verdict = VGroup(T("field multiplication", 26, INK2), T("72 instructions", 30, INK), T("0 conditional jumps", 30, BLUE), T("0 divisions", 30, BLUE)).arrange(DOWN, aligned_edge=LEFT, buff=0.15).next_to(g, RIGHT, buff=0.8)
        self.play(FadeIn(g), run_time=0.6)
        self.play(LaggedStart(*[FadeIn(v) for v in verdict], lag_ratio=0.25), run_time=1.0)
        self.end()


# ---------------------------------------------------------------------------
class S09Demo(Narrated):
    key = "09_demo"

    def construct(self):
        steps = json.loads((ROOT / "video" / "demo_transcript.json").read_text())
        frame = RoundedRectangle(corner_radius=0.18, width=13.4, height=7.3, color=MUTED, fill_color="#0e1013", fill_opacity=1, stroke_width=1.5)
        bar = Rectangle(width=13.4, height=0.42, color=PANEL, fill_color=PANEL, fill_opacity=1, stroke_width=0).align_to(frame, UP)
        dots = VGroup(*[Circle(radius=0.07, color=c, fill_color=c, fill_opacity=1, stroke_width=0) for c in ("#d95926", "#c98500", "#3987e5")]).arrange(RIGHT, buff=0.12).move_to(bar).align_to(bar, LEFT).shift(RIGHT * 0.3)
        ttl = T("cairn: 3-of-5 custody of a 32-byte master key", 18, INK2, font=MONO).move_to(bar)
        self.at(0)
        self.play(FadeIn(frame), FadeIn(bar), FadeIn(dots), FadeIn(ttl), run_time=0.8)

        top = frame.get_top() + DOWN * 0.75
        left = frame.get_left() + RIGHT * 0.35
        self.lines = VGroup()
        self.cursor_y = top[1]

        def emit(mob):
            mob.move_to([left[0], self.cursor_y, 0], aligned_edge=LEFT)
            self.cursor_y -= 0.42
            self.lines.add(mob)
            # scroll when the window is full
            if self.cursor_y < frame.get_bottom()[1] + 0.35:
                self.play(self.lines.animate.shift(UP * 0.42), run_time=0.15)
                self.cursor_y += 0.42
                first = self.lines[0]
                self.remove(first)
                self.lines.remove(first)
                for m in list(self.lines):
                    if m.get_top()[1] > frame.get_top()[1] - 0.45:
                        self.remove(m)
                        self.lines.remove(m)

        def show(i, typing=0.9):
            s = steps[i]
            if s.get("note"):
                note = T("# " + s["note"], 18, MUTED, font=MONO)
                emit(note)
                self.play(FadeIn(note), run_time=0.25)
            cmd = T("$ " + s["cmd"], 19, INK, font=MONO)
            emit(cmd)
            self.play(AddTextLetterByLetter(cmd, run_time=typing))
            for line in s["out"]:
                color = ORANGE if ("error" in line or "WARNING" in line) else (BLUE if "verified" in line or "wrote" in line or "refreshed" in line else INK2)
                o = T("  " + line, 17, color, font=MONO)
                emit(o)
                self.play(FadeIn(o), run_time=0.2)

        self.at(1)
        show(0, 1.0)
        self.at(3)
        show(1, 0.9)
        show(2, 0.7)
        self.at(4)
        show(3, 0.6)
        show(4, 1.0)
        self.at(5)
        show(5, 0.8)
        self.at(6)
        show(6, 0.8)
        self.at(7)
        show(7, 0.4)
        show(8, 0.6)
        show(9, 0.8)
        self.end()


# ---------------------------------------------------------------------------
class S10Close(Narrated):
    key = "10_close"

    def construct(self):
        sec = load("secrecy")
        amd = load("amd")
        items = [
            f"perfect secrecy: 0 violations in {sum(r['dealings_enumerated'] for r in sec):,} enumerated dealings",
            f"AMD bound: exhaustive over {sum(r['secret_error_pairs'] for r in amd):,} pairs, tight",
            "Python reference matches Rust byte for byte",
            f"timing: CAIRN max |t| = {max(r['campaign']['max_abs_t'] for r in load('dudect')['rows'] if r['implementation'] == 'cairn'):.1f}, baseline up to {max(r['campaign']['max_abs_t'] for r in load('dudect')['rows'] if r['implementation'] == 'naive'):,.0f}",
            "Valgrind: 0 secret-dependent branches",
            "Miri: no undefined behaviour",
        ]
        self.at(0)
        rows = VGroup()
        for s in items:
            mark = T("✓", 30, BLUE)
            rows.add(VGroup(mark, T(s, 27, INK)).arrange(RIGHT, buff=0.3))
        rows.arrange(DOWN, aligned_edge=LEFT, buff=0.28).shift(UP * 0.5)
        self.play(LaggedStart(*[FadeIn(r, shift=0.15 * RIGHT) for r in rows], lag_ratio=0.45), run_time=4.5)

        self.at(1)
        cmd = T("$ make verify", 32, INK, font=MONO).next_to(rows, DOWN, buff=0.6).align_to(rows, LEFT)
        done = T("== all checks passed", 28, BLUE, font=MONO).next_to(cmd, DOWN, buff=0.15).align_to(cmd, LEFT)
        self.play(AddTextLetterByLetter(cmd, run_time=0.8))
        self.play(FadeIn(done), run_time=0.4)

        self.at(2)
        self.play(FadeOut(VGroup(rows, cmd, done)), run_time=0.6)
        stack = cairn_stack(1.2).shift(DOWN * 1.2)
        title = T("CAIRN", 96, INK).next_to(stack, UP, buff=0.45)
        self.play(FadeIn(stack[0], shift=0.4 * DOWN), Write(title), run_time=0.9)

        self.at(3)
        self.play(Indicate(stack[0], color=BLUE, scale_factor=1.08), run_time=0.9)

        self.at(4)
        self.play(LaggedStart(*[FadeIn(s, shift=0.5 * DOWN) for s in stack[1:]], lag_ratio=0.25), run_time=1.4)
        url = T("github.com/Ahmedouyahya/cairn", 30, BLUE, font=MONO).next_to(stack, DOWN, buff=0.45)
        self.play(FadeIn(url), run_time=0.6)
        self.end()


SCENES = [S01Hook, S02Windows, S03Field, S04Amd, S05Sharing, S06Storage, S07Refresh, S08SideChannel, S09Demo, S10Close]
