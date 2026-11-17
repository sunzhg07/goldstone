//! A curated corpus of many-body equations (MBPT, CC, EOM/Λ, IMSRG,
//! VS-IMSRG, coupled forms) and what "Diagram from LaTeX" does with each.
//! See docs/PARSER_PLAN.md.
//!
//! `Draws(n)`: draws n diagrams with no warnings.
//! `Gap(phase)`: not supported yet; the phase of the plan that adds it.
//! When a gap is closed this test fails on purpose: update the entry to
//! `Draws`, so the corpus always states what works.

use goldstone_core::expr;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Expect {
    Draws(usize),
    Gap(&'static str),
}
use Expect::*;

const CORPUS: &[(&str, &str, Expect)] = &[
    // ---- MBPT -------------------------------------------------------------
    (
        "MBPT2 via amplitudes",
        r"E^{(2)} = \frac{1}{4} \sum_{ijab} \langle ij || ab \rangle t_{ij}^{ab}",
        Draws(1),
    ),
    (
        "MBPT2 with denominator",
        r"E^{(2)} = \frac{1}{4} \sum_{ijab} \frac{\langle ij || ab \rangle \langle ab || ij \rangle}{\epsilon_i + \epsilon_j - \epsilon_a - \epsilon_b}",
        Gap("B: denominators"),
    ),
    (
        "MBPT3 particle ladder",
        r"E^{(3)} = \frac{1}{8} \sum_{ijabcd} \frac{\langle ij||ab\rangle \langle ab||cd\rangle \langle cd||ij\rangle}{D_{ij}^{ab} D_{ij}^{cd}}",
        Gap("B: denominators"),
    ),
    // ---- CC ---------------------------------------------------------------
    (
        "CCSD energy",
        r"E_{\rm CC} = \sum_{ia} f_{ia} t_i^a + \frac{1}{4} \sum_{ijab} \langle ij||ab\rangle t_{ij}^{ab} + \frac{1}{2} \sum_{ijab} \langle ij||ab\rangle t_i^a t_j^b",
        Draws(3),
    ),
    (
        "CCD linear",
        r"0 = \langle ab||ij\rangle + P(ab) \sum_c f_{bc} t_{ij}^{ac} - P(ij) \sum_k f_{kj} t_{ik}^{ab} + \frac{1}{2} \sum_{cd} \langle ab||cd\rangle t_{ij}^{cd} + \frac{1}{2} \sum_{kl} \langle kl||ij\rangle t_{kl}^{ab} + P(ij)P(ab) \sum_{kc} \langle kb||cj\rangle t_{ik}^{ac}",
        Draws(6),
    ),
    (
        "CCD quadratic",
        r"0 = \frac{1}{4} \sum_{klcd} \langle kl||cd\rangle t_{ij}^{cd} t_{kl}^{ab} + P(ij) \sum_{klcd} \langle kl||cd\rangle t_{ik}^{ac} t_{jl}^{bd}",
        Draws(2),
    ),
    (
        "CCSD T1 (Crawford)",
        r"0 = f_{ai} + \sum_c f_{ac} t_i^c - \sum_k f_{ki} t_k^a + \sum_{kc} \langle ka||ci\rangle t_k^c + \sum_{kc} f_{kc} t_{ik}^{ac} + \frac{1}{2} \sum_{kcd} \langle ka||cd\rangle t_{ki}^{cd} - \frac{1}{2} \sum_{klc} \langle kl||ci\rangle t_{kl}^{ca}",
        Draws(7),
    ),
    (
        "CCSD T2 (Stanton-Gauss)",
        r"\begin{aligned} 0 &= \langle ij || ab \rangle + P(ab) \sum_e t_{ij}^{ae} \left( \mathcal{F}_b^e - \frac{1}{2} \sum_m t_m^b \mathcal{F}_m^e \right) \\ &\quad + \frac{1}{2} \sum_{mn} \tau_{mn}^{ab} \mathcal{W}_{mn}^{ij} \end{aligned}",
        Draws(4),
    ),
    (
        "generated CCSDT term",
        r"z3^{a b c }_{i j k } &+=& - V ^{a m }_{e f } t^{e b }_{i j } t^{f c }_{m k }",
        Draws(1),
    ),
    (
        "three-body interaction",
        r"E = \frac{1}{36} \sum_{ijkabc} W_{ijkabc} T_{abcijk}",
        Draws(1),
    ),
    (
        "rank subscripts T_2",
        r"E = \frac{1}{4} \sum_{ijab} V_{ijab} (T_2)_{abij}",
        Gap("A: operator labels"),
    ),
    (
        "operator form",
        r"E = \langle \Phi_0 | \bar{H} | \Phi_0 \rangle",
        Gap("F: operator algebra"),
    ),
    // ---- EOM / Λ ----------------------------------------------------------
    (
        "EOM-CCSD sigma 1h1p",
        r"\sigma_i^a = \sum_e \bar{H}_{ae} r_i^e - \sum_m \bar{H}_{mi} r_m^a + \sum_{me} \bar{H}_{maei} r_m^e",
        Draws(3),
    ),
    (
        "Lambda CCSD (one-body)",
        r"0 = \bar{H}_{ia} + \sum_e \lambda_i^e \bar{H}_{ea} - \sum_m \lambda_m^a \bar{H}_{im}",
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
        r"\frac{d f_{12}}{ds} = \sum_a (1 + P_{12}) \eta_{1a} f_{a2}",
        Draws(1),
    ),
    (
        "White generator",
        r"\eta_{ai} = \frac{f_{ai}}{f_{aa} - f_{ii} + \Gamma_{aiai}}",
        Gap("B: denominators"),
    ),
    (
        "commutator",
        r"\frac{d}{ds} \chi_{ij} = [\eta_{\rm 2b}, H_{\rm 2b}]_{ij}",
        Gap("F: operator algebra"),
    ),
    (
        "Magnus / BCH",
        r"H(s) = e^{\Omega(s)} H e^{-\Omega(s)} = \sum_{k=0}^{\infty} \frac{1}{k!} [\Omega, H]^{(k)}",
        Gap("F: operator algebra"),
    ),
    // ---- VS-IMSRG ---------------------------------------------------------
    (
        "VS-IMSRG valence generator",
        r"\eta_{vq} = \frac{f_{vq}}{f_{vv} - f_{qq}}",
        Gap("D: index classes"),
    ),
    // ---- coupled forms -----------------------------------------------------
    (
        "J-coupled term",
        r"Z^{J}_{ijkl} = \sum_{ab} \hat{J}^2 (-1)^{j_a + j_b} \chi^{J}_{ai} \Gamma^{J}_{ajkl}",
        Draws(1),
    ),
];

fn status(src: &str) -> Expect {
    match expr::from_latex(src) {
        Ok(d) if d.warnings.is_empty() => Draws(d.doc.groups.len().max(1)),
        _ => Gap("?"),
    }
}

#[test]
fn corpus() {
    let mut report = Vec::new();
    for (name, src, want) in CORPUS {
        let got = status(src);
        let fine = match (want, got) {
            (Draws(n), Draws(m)) => *n == m,
            (Gap(_), Gap(_)) => true,
            _ => false,
        };
        if !fine {
            let detail = expr::from_latex(src).map_or_else(|e| e, |d| d.warnings.join("; "));
            report.push(format!("{name}: expected {want:?}, got {got:?} {detail}"));
        }
    }
    assert!(report.is_empty(), "corpus changed:\n{}", report.join("\n"));
}
