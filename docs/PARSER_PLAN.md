# Diagram from LaTeX: test corpus and plan

How well "Diagram from LaTeX" (`goldstone_core::expr`) handles the
equations of coupled cluster, IMSRG and VS-IMSRG work, and what to add.

## Drawing a whole document

`goldstone paper.tex` (or `goldstone tex paper.tex --out-dir figs -f png`)
draws every equation into `paper-diagrams/`: one file per equation
(`eq0003-L37.pdf` = equation 3, line 37), overview pages with 20
equations each, `all.gold` with everything (editable), and `report.txt`
saying what was drawn, what was left out, and why.

## How it is tested

1. **Curated corpus**: `crates/goldstone-core/tests/corpus.rs`.
   Standard equations from each family, each marked `Draws(n)` or
   `Gap(phase)`. The test fails when a result changes, so the file always
   states what works; when a phase lands, its entries move to `Draws`.
2. **Real-world sweep**: every equation in a set of `.tex` files:

   ```sh
   cargo run --release -p goldstone-core --example corpus -- ~/work/test/*.tex ~/work/mbpt_diag/*.tex
   ```

   It extracts `equation`, `align`, `eqnarray`, `multline`, `gather`,
   `\[ \]`, `$$ $$`, runs parser and layout, and groups failures by cause.
3. **Visual spot checks**: render random passing equations
   (`goldstone latex '…' -o x.png`) and look at them. Passing the parser
   only means the contraction is consistent and laid out without errors.

## Results (2026-11-17, local files)

| Source | Equations | Drawn | Failed |
|---|---|---|---|
| CCSDT / CCSDTQ generated equations (`~/work/test`) | 4646 | **4646** | 0 |
| IMSRG / Magnus notes (`~/work/mbpt_diag`) | 169 | 0 | 169 |
| MR-CC notes (`~/work/mr_cc`) | 57 | 0 | 57 |
| IMSRG(3) / VS code notes (`~/work/imsrg/learn`) | 341 | 0 | 341 |
| Angular-momentum-coupled terms (`~/work/amc/output`) | 9 | 0 | 9 |
| **Total** | 5222 | 4646 | 576 |

Every CC equation draws (including ⟨pq‖rs⟩, `\mathcal{F}`/`\mathcal{W}`
intermediates, mixed index conventions, parentheses, `P(ij)`, aligned
multi-line equations, `&+=&`, `V ^{ij}_{ab}`). Everything IMSRG-like fails.

## What fails, by cause (the 576)

| Cause | Count | Example |
|---|---|---|
| Operator-level notation (no indices): `e^{\Omega}`, `U(s)`, `\frac{d}{ds}`, `[A,B]`, `\langle\Phi|…|\Phi\rangle`, cases | ~250 | `H(s) = e^{\Omega} H e^{-\Omega}` |
| Occupation factors `n_a`, `\bar n_a`, `(n_a - n_b)`, `(1 - n_a)` | ~110 | `\sum_{abcd} \bar n_a \bar n_b n_c n_d \Gamma_{…}` |
| General indices whose particle/hole character is not fixed by the letter | (inside the above) | `\eta_{ab} f_{ba}` in IMSRG notation |
| Labels that are not indices: `\eta_{\rm 2b}`, `T_2`, `\Gamma^{J}`, `W^{(1)}`, `O_0` | ~90 | `\chi^{J}_{ai}` |
| Coupled-scheme factors: `\hat J^2`, `(-1)^{j_a+j_b}`, 6j symbols, `\delta_{j_a j_b}` | ~70 | `\hat J^2 (-1)^{J_0} \sixj{…}` |
| Antisymmetrizers `(1 - \hat P_{ij})` | ~25 | `(1-\hat P_{ij}) \chi_{ai} \Gamma_{ajkl}` |
| Delimiters `\Big( \bigl\{ \Bigg[` | ~30 | `\Big( \bar n_a … \Big)` |
| Denominators `\frac{\langle…\rangle}{\epsilon…}` | ~15 | `\bar\Omega = \frac{\bar W^{\rm od}}{\bar\Delta}` |
| Composite indices `m_i`, `(a,j)`, `ij,kl` | ~45 | `C_{i m_i j m_j}` |
| Prose / code in math (`\mathrm{Eta.GetJRank()}`) | few | — |

Some of these are not diagrams at all (definitions, BCH series, code
mappings). The parser should say "this is not a tensor contraction"
instead of a confusing message, and draw nothing.

## Plan

Phases are ordered by value per effort. Each adds corpus entries.

**A. Notation cleanup (small).**
`\Big( \bigl\{ \Bigg[` as brackets; superscripts and subscripts that are
labels, not indices (`{\rm 2b}`, `(1)`, `J`, `T_2`, `O_0`), kept as part of
the operator's name; `\hat P_{ij}`, `(1-\hat P_{ij})` treated like `P(ij)`;
index lists with commas (`ij,kl`); a clear "not a tensor contraction"
message for operator-level equations.

**B. Denominators.**
`\frac{numerator}{denominator}`: draw the numerator; the denominator
(energies, `D_{ij}^{ab}`, `\Delta`) becomes part of the prefactor text or is
dropped, with an option to draw Goldstone denominator cuts (dotted
horizontal lines between vertex levels).

**C. General indices and occupation factors (IMSRG) — done.**
Decided: in (VS-)IMSRG notation every index is a general orbital; `n_a`
makes line *a* a hole, `\bar n_a` or `(1-n_a)` a particle; external lines
are general, drawn without arrows, to keep diagrams light. Occupation
factors are multiplied out into definite particle–hole diagrams
(`(n_a-n_b)` → two diagrams, `(1-n_a-n_b)` → `\bar n_a\bar n_b - n_a n_b`,
`n_a\bar n_a` = 0, equal terms added, cancelling ones dropped). IMSRG
notation is recognised by occupation factors; otherwise the letters decide
(a–g particles, h–n holes, other letters and digits general), never the
operator's name. Digit orbitals (`f_{12}`, `\eta_{1p}`) work. General external lines go to
the nearest edge (down from the bottom operator, up from the top one).

Original plan for C:
In IMSRG papers `a, b, c, d` (or `p, q, r, s`) are general orbitals and
the factors `n_a`, `\bar n_a` say whether a line is a hole or a particle.
Plan: an index-convention switch; occupation factors decide each line's
character, and sums like `(n_a - n_b)` or `n_a n_b \bar n_c \bar n_d - …`
are multiplied out into the particle–hole diagrams they stand for
(`(n_a - n_b) η_{ab} f_{ba}` → two diagrams). Lines of undetermined
character are drawn without an arrow.

**D. Index classes for VS-IMSRG.**
Configurable classes beyond particle/hole: core, valence, excluded
(`q`-space), with their own letters and line styles (e.g. valence lines
double or coloured). Needs your notation (see questions).

**E. Coupled-scheme equations — done.**
`\hat J`, `(-1)^{…}`, 6j/9j symbols, `\delta_{…}`, `\sqrt{…}`,
`\frac{1}{2j+1}` and coupling labels (`^{J}`, `^{0}`) are not part of the
diagram and are left out (decided: "the diagram is the diagram").
Operators written in either index order (`\chi_{ai} \Gamma_{ajkl}`) are
read the way that makes the contraction consistent. All equations in
`~/work/amc/output` draw.

**F. Operator algebra (large, separate feature).**
`[A, B]`, nested commutators, `\langle\Phi_0|\bar H|\Phi_0\rangle`,
`e^{-T} H e^{T}`: generate all contractions (Wick) and draw each diagram.
This is a diagram *generator*, like `~/work/tenpi-master` and
`~/work/cc_gen`; best done by reading their output (`z… &+=& …` lines
already work) rather than reimplementing them.

## Layout quality

`goldstone_core::quality` measures a drawn diagram: fermion lines crossing,
lines through amplitude bars or (internal lines) through interaction
lines, external lines over interaction lines (normal in Goldstone
diagrams, counted apart), lines within 5 pt of a vertex, labels
overlapping labels or lines, slanted internal lines, size.

```sh
cargo run --release -p goldstone-core --example layout_report -- FILES.tex --worst 20 --dump DIR
```

prints these per diagram type and lists the worst equations.

Over 4593 equations (all local files), before → after this round:

| | before | after |
|---|---|---|
| diagrams with a problem (score ≥ 2) | 87 % | 45 % |
| mean score | 11.6 | 2.7 |
| internal lines through operators | 1.29 | 0.00 |
| lines near a vertex | 0.01 | 0.00 |
| label × line | 0.21 | 0.05 |
| label × label | 0.08 | 0.01 |
| crossings | 0.10 | 0.07 |

What changed: candidate layouts (the best column arrangements under
several column groupings) are drawn and the one that measures best is
kept; external lines count in the column choice; one-body operators have
their real extent (---X, bar) when routing external lines; a line that
would run through a vertex in its column becomes an arc around it; the
space for the equation text uses the drawn extent (arcs, labels); general
external lines go to the nearest edge.

Remaining, by design or topology: external lines crossing an interaction
line (1.2 per diagram: amplitudes under an interaction with open lines
that must reach the top); about 0.45 crossings in T₃T₃V, T₂T₄V, T₁T₂T₃V
terms, where both interaction vertices connect to both amplitudes, so one
crossing is unavoidable.

## Questions

1. VS-IMSRG: which letters for core, valence and excluded orbitals, and
   how should valence lines look?
