//! A curated corpus of many-body equations (MBPT, CC, EOM/Λ, IMSRG,
//! coupled forms) and what "Diagram from LaTeX" does with each.
//! See docs/PARSER_PLAN.md.
//!
//! The syntax is strict: an operator is `X^{label}_{indices}`, an index is
//! `a` or `a_1`; a–g particles, h–n holes, other letters general.
//!
//! `Draws(n)`: draws n diagrams with no warnings.
//! `Rejected(text)`: not accepted; the error message contains `text`.
//! `Gap(phase)`: valid, but not supported yet (the plan's phase).

use goldstone_core::expr;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Expect {
    Draws(usize),
    Rejected(&'static str),
    Gap(&'static str),
}
use Expect::*;

const CORPUS: &[(&str, &str, Expect)] = &[
    // ---- MBPT -------------------------------------------------------------
    (
        "MBPT2 via amplitudes",
        r"E^{(2)} = \frac{1}{4} \sum_{ijab} V_{ijab} t_{abij}",
        Draws(1),
    ),
    (
        "MBPT2 with denominator",
        r"E^{(2)} = \frac{1}{4} \sum_{ijab} \frac{V_{ijab} V_{abij}}{\epsilon_{i} + \epsilon_{j} - \epsilon_{a} - \epsilon_{b}}",
        Gap("B: denominators"),
    ),
    // ---- CC ---------------------------------------------------------------
    (
        "CCSD energy",
        r"E_{\rm CC} = \sum_{ia} f_{ia} t_{ai} + \frac{1}{4} \sum_{ijab} V_{ijab} t_{abij} + \frac{1}{2} \sum_{ijab} V_{ijab} t_{ai} t_{bj}",
        Draws(3),
    ),
    (
        "CCD linear",
        r"0 = V_{abij} + P(ab) \sum_{c} f_{bc} t_{acij} - P(ij) \sum_{k} f_{kj} t_{abik} + \frac{1}{2} \sum_{cd} V_{abcd} t_{cdij} + \frac{1}{2} \sum_{kl} V_{klij} t_{abkl} + P(ij)P(ab) \sum_{kc} V_{kbcj} t_{acik}",
        Draws(6),
    ),
    (
        "CCD quadratic",
        r"0 = \frac{1}{4} \sum_{klcd} V_{klcd} t_{cdij} t_{abkl} + P(ij) \sum_{klcd} V_{klcd} t_{acik} t_{bdjl}",
        Draws(2),
    ),
    (
        "CCSD T1 (Crawford)",
        r"0 = f_{ai} + \sum_{c} f_{ac} t_{ci} - \sum_{k} f_{ki} t_{ak} + \sum_{kc} V_{kaci} t_{ck} + \sum_{kc} f_{kc} t_{acik} + \frac{1}{2} \sum_{kcd} V_{kacd} t_{cdki} - \frac{1}{2} \sum_{klc} V_{klci} t_{cakl}",
        Draws(7),
    ),
    (
        "CCSD T2, rows and parentheses",
        r"\begin{aligned} 0 &= V_{abij} + P(ab) \sum_{e} T_{aeij} \left( \mathcal{F}_{be} - \frac{1}{2} \sum_{m} T_{bm} \mathcal{F}_{me} \right) \\ &\quad + \frac{1}{2} \sum_{mn} \tau_{abmn} \mathcal{W}_{mnij} \end{aligned}",
        Draws(4),
    ),
    (
        "three-body interaction",
        r"E = \frac{1}{36} \sum_{ijkabc} W_{ijkabc} T_{abcijk}",
        Draws(1),
    ),
    (
        "numbered indices",
        r"Z_{a_1 a_2 h_1 h_2} = \frac{1}{2} \sum_{b_1 b_2} V_{a_1 a_2 b_1 b_2} T_{b_1 b_2 h_1 h_2}",
        Draws(1),
    ),
    (
        "labelled operators",
        r"E = \frac{1}{4} \sum_{ijab} V^{1a}_{ijab} T^{(2)}_{abij}",
        Draws(1),
    ),
    (
        "operator form",
        r"E = \langle \Phi_0 | \bar{H} | \Phi_0 \rangle",
        Rejected(r"\langle"),
    ),
    // ---- EOM / Λ ----------------------------------------------------------
    (
        "EOM-CCSD sigma 1h1p",
        r"\sigma_{ai} = \sum_{e} \bar{H}_{ae} r_{ei} - \sum_{m} \bar{H}_{mi} r_{am} + \sum_{me} \bar{H}_{maei} r_{em}",
        Draws(3),
    ),
    (
        "Lambda CCSD (one-body)",
        r"0 = \bar{H}_{ia} + \sum_{e} \lambda_{ie} \bar{H}_{ea} - \sum_{m} \lambda_{ma} \bar{H}_{im}",
        Draws(3),
    ),
    // ---- IMSRG ------------------------------------------------------------
    (
        "IMSRG(2) energy flow",
        r"\frac{dE}{ds} = \sum_{ab} (n_a - n_b) \eta_{ab} f_{ba} + \frac{1}{4} \sum_{abcd} \eta_{abcd} \Gamma_{cdab} n_a n_b \bar{n}_c \bar{n}_d",
        Draws(3),
    ),
    (
        "IMSRG one-body flow",
        r"\frac{d f_{pq}}{ds} = \sum_{r} (1 + P_{pq}) \eta_{pr} f_{rq}",
        Draws(1),
    ),
    (
        "Gamma with general and hole lines",
        r"Z_{abcd} = \sum_{ph} \Gamma_{ahcp} T_{pbhd}",
        Draws(1),
    ),
    (
        "White generator",
        r"\eta_{ai} = \frac{f_{ai}}{f_{aa} - f_{ii} + \Gamma_{aiai}}",
        Gap("B: denominators"),
    ),
    // ---- coupled forms -----------------------------------------------------
    (
        "J-coupled term",
        r"Z^{J}_{ijkl} = \sum_{ab} \hat{J}^2 (-1)^{j_a + j_b} \chi^{J}_{ia} \Gamma^{J}_{ajkl}",
        Draws(1),
    ),
    // ---- rejected forms ------------------------------------------------------
    ("shorthand", r"E = V_ijab T_abij", Rejected("braces")),
    (
        "bra-ket",
        r"E = \frac{1}{4} \langle ij||ab \rangle t_{abij}",
        Rejected(r"\langle"),
    ),
    (
        "indices in the superscript",
        r"E = V_{ijab} t^{ab}_{ij}",
        Rejected("superscript"),
    ),
    (
        "one-character script",
        r"E = f_{ia} t_i^a",
        Rejected("braces"),
    ),
    (
        "digit orbitals",
        r"Z_{12} = \eta_{1a} f_{a2}",
        Rejected("index"),
    ),
    (
        "index with a digit",
        r"E = V_{i1ab} T_{abi1}",
        Rejected("index"),
    ),
    (
        "sum without backslash",
        r"E = sum_{ijab} V_{ijab} T_{abij}",
        Rejected(r"\sum"),
    ),
];

fn status(src: &str) -> Result<usize, String> {
    let d = expr::from_latex(src)?;
    if d.warnings.is_empty() {
        Ok(d.doc.groups.len().max(1))
    } else {
        Err(d.warnings.join("; "))
    }
}

#[test]
fn corpus() {
    let mut report = Vec::new();
    for (name, src, want) in CORPUS {
        let got = status(src);
        let fine = match (want, &got) {
            (Draws(n), Ok(m)) => n == m,
            (Rejected(text), Err(e)) => e.contains(text),
            (Gap(_), Err(_)) => true,
            _ => false,
        };
        if !fine {
            report.push(format!("{name}: expected {want:?}, got {got:?}"));
        }
    }
    assert!(report.is_empty(), "corpus changed:\n{}", report.join("\n"));
}
