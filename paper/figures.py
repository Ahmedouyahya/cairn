"""Build the paper's figures from the lab's JSON results (results/*.json).

Run: make figures   (uv run --with matplotlib --with numpy python paper/figures.py)
Every number drawn here comes from a file written by `cairn-lab`.
"""

import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.colors import LinearSegmentedColormap  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
RES = ROOT / "results"
OUT = ROOT / "paper" / "figures"
OUT.mkdir(exist_ok=True)

# Validated reference palette, slots 1 and 2; text in neutral ink.
CAIRN = "#2a78d6"
NAIVE = "#eb6834"
INK = "#0b0b0b"
INK2 = "#52514e"
GRID = "#e4e3df"
BLUES = LinearSegmentedColormap.from_list("blues", ["#f3f7fd", "#9cc3ef", "#2a78d6", "#0d3b73"])

plt.rcParams.update({
    "font.family": "serif",
    "font.size": 8.5,
    "axes.edgecolor": INK2,
    "axes.labelcolor": INK,
    "axes.linewidth": 0.6,
    "xtick.color": INK2,
    "ytick.color": INK2,
    "axes.spines.top": False,
    "axes.spines.right": False,
    "savefig.bbox": "tight",
    "savefig.pad_inches": 0.02,
})


def load(name):
    return json.loads((RES / f"{name}.json").read_text())["result"]


def dudect():
    rows = load("dudect")["rows"]
    ops = [
        ("multiplication", "Fp127 multiplication", "double-and-add multiplication"),
        ("inversion", "Fp127 inversion (Fermat)", "extended Euclid inversion"),
        ("reconstruction", "reconstruct + AMD verify (3-of-5, ell=8)", "plain Shamir reconstruct"),
    ]
    t = {r["operation"]: r["campaign"]["max_abs_t"] for r in rows}
    fig, ax = plt.subplots(figsize=(3.4, 1.9))
    y = np.arange(len(ops))
    h = 0.36
    for k, (label, c_op, n_op) in enumerate(ops):
        for off, val, col in ((-h / 2, t[c_op], CAIRN), (h / 2, t[n_op], NAIVE)):
            ax.barh(y[k] + off, val, height=h - 0.04, color=col, edgecolor="white", linewidth=1)
            ax.text(val * 1.15, y[k] + off, f"{val:,.1f}", va="center", fontsize=7, color=INK)
    ax.axvline(4.5, color=INK, lw=0.8, ls=(0, (3, 2)))
    ax.text(4.5 * 1.12, -0.62, "threshold 4.5", fontsize=6.5, color=INK2, va="bottom")
    ax.set_xscale("log")
    ax.set_xlim(0.3, 2e5)
    ax.set_yticks(y, [o[0] for o in ops])
    ax.set_xlabel(r"max $|t|$ (Welch, fixed vs random, log scale)")
    ax.grid(axis="x", color=GRID, lw=0.5)
    ax.set_axisbelow(True)
    from matplotlib.patches import Patch
    ax.set_ylim(len(ops) - 0.45, -0.75)
    ax.legend(handles=[Patch(color=CAIRN, label="CAIRN"), Patch(color=NAIVE, label="naive baseline")],
              loc="lower center", bbox_to_anchor=(0.5, 1.0), ncol=2, frameon=False, fontsize=7)
    fig.savefig(OUT / "dudect.pdf")


def refresh():
    rows = [r for r in load("refresh") if r["method"].startswith("B")]
    configs = []
    for r in rows:
        c = (r["p"], r["ell"])
        if c not in configs:
            configs.append(c)
    get = lambda c, s: next(r for r in rows if (r["p"], r["ell"]) == c and r["refresh"] == s)  # noqa: E731
    fig, ax = plt.subplots(figsize=(3.4, 1.9))
    x = np.arange(len(configs))
    w = 0.36
    two = [get(c, "2-step")["tv_old_R_new_L_nonzero_values"] for c in configs]
    three = [get(c, "3-step")["tv_old_R_new_L_nonzero_values"] for c in configs]
    ax.bar(x - w / 2, two, w - 0.04, color=NAIVE, label="2-step refresh", edgecolor="white")
    ax.bar(x + w / 2, three, w - 0.04, color=CAIRN, label="3-step refresh (CAIRN)", edgecolor="white")
    for i, c in enumerate(configs):
        pred = c[0] ** (-(c[1] - 1))
        ax.plot([x[i] + w / 2 - w / 2, x[i] + w / 2 + w / 2], [pred, pred], color=INK, lw=0.9)
        ax.text(x[i] + w / 2, pred * 1.25, f"{three[i]:.3f}", ha="center", va="bottom", fontsize=6.5, color=INK)
        ax.text(x[i] - w / 2, two[i] * 1.08, "1", ha="center", va="bottom", fontsize=6.5, color=INK)
    ax.set_yscale("log")
    ax.set_ylim(0.01, 2)
    ax.set_xticks(x, [f"p={p}, $\\ell$={e}" for p, e in configs])
    ax.set_ylabel("TV(old R, new L)")
    ax.grid(axis="y", color=GRID, lw=0.5)
    ax.set_axisbelow(True)
    ax.plot([], [], color=INK, lw=0.9, label=r"predicted $p^{-(\ell-1)}$")
    ax.set_ylim(0.01, 3)
    ax.legend(frameon=False, fontsize=6.5, loc="lower center", ncol=3, bbox_to_anchor=(0.5, 1.0),
              handlelength=1.2, columnspacing=0.8)
    fig.savefig(OUT / "refresh.pdf")


def leakage():
    res = load("leakage")
    models = {m["model"]: m["advantage"] for m in res["toy_field"]["models"]}
    ell = res["toy_field"]["ell"]
    grid = np.array([[models[f"coords({a},{b})"] for b in range(ell + 1)] for a in range(ell + 1)])
    fig, ax = plt.subplots(figsize=(2.3, 2.1))
    im = ax.imshow(np.log10(np.maximum(grid, 1e-4)), cmap=BLUES, vmin=-4, vmax=0, origin="lower")
    for a in range(ell + 1):
        for b in range(ell + 1):
            v = grid[a, b]
            txt = "0" if v == 0 else (f"{v:.3f}" if v < 1 else "1")
            ax.text(b, a, txt, ha="center", va="center", fontsize=6.5,
                    color="white" if v >= 0.04 else INK)
    ax.set_xticks(range(ell + 1))
    ax.set_yticks(range(ell + 1))
    ax.set_xlabel("coordinates of R read in clear")
    ax.set_ylabel("coordinates of L read in clear")
    for s in ax.spines.values():
        s.set_visible(False)
    ax.tick_params(length=0)
    fig.savefig(OUT / "leakage.pdf")

    tab = res["production_budgets"]
    fig, ax = plt.subplots(figsize=(2.6, 1.8))
    ells = [r["ell"] for r in tab]
    bud = [r["budget_bits_eps_2^-128"] for r in tab]
    half = [r["half_size_bits"] for r in tab]
    ax.bar(range(len(ells)), bud, 0.6, color=CAIRN, edgecolor="white")
    for i, (b, hs) in enumerate(zip(bud, half)):
        ax.text(i, b + 30, f"{b:.0f}", ha="center", fontsize=7, color=INK)
    ax.set_xticks(range(len(ells)), [str(e) for e in ells])
    ax.set_xlabel(r"storage dimension $\ell$")
    ax.set_ylabel(r"leakage budget (bits), $\varepsilon=2^{-128}$")
    ax.set_ylim(0, 1600)
    ax.grid(axis="y", color=GRID, lw=0.5)
    ax.set_axisbelow(True)
    fig.savefig(OUT / "budget.pdf")


if __name__ == "__main__":
    dudect()
    refresh()
    leakage()
    print(f"figures written to {OUT}")
