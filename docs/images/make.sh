#!/bin/sh
# Regenerates the README images with Goldstone itself.
#   cargo build --release && sh docs/images/make.sh
set -e
cd "$(dirname "$0")/../.."
G=${GOLDSTONE:-target/release/goldstone}
O=docs/images
BG="--background white"

$G cc-latex 'E^{(2)} = \frac{1}{4} \sum_{ijab} V_{ijab} t_{abij}' -o $O/mbpt2.svg $BG
$G cc-latex 'Z_{abic} = \sum_{ef} V_{abef} S_{efic}' -o $O/open.svg $BG
$G cc-latex '0 = \frac{1}{2} \sum_{cd} V_{abcd} t_{cdij} + \frac{1}{2} \sum_{kl} V_{klij} t_{abkl} + P(ij) P(ab) \sum_{kc} V_{kbcj} t_{acik} + \frac{1}{4} \sum_{klcd} V_{klcd} t_{cdij} t_{abkl}' -o $O/ccd.svg $BG
$G cc-latex '\begin{aligned}
Z_{a_1 a_2 h_1 h_2} &= \frac{1}{2} \sum_{b_1 b_2} V_{a_1 a_2 b_1 b_2} T_{b_1 b_2 h_1 h_2} + P(h_1 h_2) \sum_{i_1 b_1} V_{i_1 a_2 b_1 h_2} T_{a_1 b_1 h_1 i_1} \\
&\quad + \sum_{e} T_{a_1 e h_1 h_2} \left( F_{a_2 e} - \frac{1}{2} \sum_{m} T_{a_2 m} F_{m e} \right)
\end{aligned}' -o $O/ccsd.svg $BG
$G srg-latex '\frac{dE}{ds} = \sum_{ab} (n_a - n_b) \eta_{ab} f_{ba} + \frac{1}{4} \sum_{abcd} \eta_{abcd} \Gamma_{cdab} n_a n_b \bar{n}_c \bar{n}_d' -o $O/imsrg.svg $BG
$G srg-latex '\chi_{ijkl} = \sum_{ab} \left( \bar{n}_a \bar{n}_b n_l + n_a n_b \bar{n}_l \right) \Gamma_{ijab} \Omega_{abkl}' -o $O/imsrg2.svg $BG
$G plot 'sin(x); cos(x); \frac{1}{1+x^2}' --x=-2pi:2pi -o $O/plot.svg $BG
$G render examples/mbpt2.gold -o $O/handmade.svg $BG
$G render examples/gallery.gold -o $O/gallery.svg $BG
$G render examples/double.gold -o $O/double.svg $BG
$G render examples/ticks.gold -o $O/ticks.svg $BG
echo "done"
