#!/bin/sh
# Regenerates the README images with Goldstone itself.
#   cargo build --release && sh docs/images/make.sh
set -e
cd "$(dirname "$0")/../.."
G=${GOLDSTONE:-target/release/goldstone}
O=docs/images
BG="--background white"

$G latex 'E^{(2)} = \frac{1}{4} \sum_{ijab} \langle ij || ab \rangle t_{ij}^{ab}' -o $O/mbpt2.svg $BG
$G latex 'Z_{abic} = \sum_{ef} V_{abef} S_{efic}' -o $O/open.svg $BG
$G latex '0 = \frac{1}{2} \sum_{cd} \langle ab||cd \rangle t_{ij}^{cd} + \frac{1}{2} \sum_{kl} \langle kl||ij \rangle t_{kl}^{ab} + P(ij) P(ab) \sum_{kc} \langle kb||cj \rangle t_{ik}^{ac} + \frac{1}{4} \sum_{klcd} \langle kl||cd \rangle t_{ij}^{cd} t_{kl}^{ab}' -o $O/ccd.svg $BG
$G latex '\begin{aligned}
0 &= \langle ij || ab \rangle + P(ab) \sum_e t_{ij}^{ae} \left( \mathcal{F}_b^e - \frac{1}{2} \sum_m t_m^b \mathcal{F}_m^e \right) - P(ij) \sum_m t_{im}^{ab} \left( \mathcal{F}_m^j + \frac{1}{2} \sum_e t_j^e \mathcal{F}_m^e \right) \\
&\quad + \frac{1}{2} \sum_{mn} \tau_{mn}^{ab} \mathcal{W}_{mn}^{ij} + \frac{1}{2} \sum_{ef} \tau_{ij}^{ef} \mathcal{W}_{ab}^{ef} + P(ij) P(ab) \sum_{me} \left( t_{im}^{ae} \mathcal{W}_{mb}^{ej} - t_i^e t_m^a \langle mb || ej \rangle \right) \\
&\quad + P(ij) \sum_e t_i^e \langle ab || ej \rangle - P(ab) \sum_m t_m^a \langle mb || ij \rangle
\end{aligned}' -o $O/ccsd.svg $BG
$G latex '\frac{dE}{ds} = \sum_{ab} (n_a - n_b) \eta_{ab} f_{ba} + \frac{1}{4} \sum_{abcd} \eta_{abcd} \Gamma_{cdab} n_a n_b \bar{n}_c \bar{n}_d' -o $O/imsrg.svg $BG
$G latex '\chi_{ijkl} = \sum_{ab} \left( \bar{n}_a \bar{n}_b n_l + n_a n_b \bar{n}_l \right) \Gamma_{ijab} \Omega_{abkl}' -o $O/imsrg2.svg $BG
$G render examples/mbpt2.gold -o $O/handmade.svg $BG
$G render examples/gallery.gold -o $O/gallery.svg $BG
$G render examples/double.gold -o $O/double.svg $BG
$G render examples/ticks.gold -o $O/ticks.svg $BG
echo "done"
