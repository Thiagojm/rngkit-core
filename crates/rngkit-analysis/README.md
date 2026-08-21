# rngkit-analysis

Incremental and batch descriptive cumulative statistics for RngKit sessions.

For total evaluated bits `N` and cumulative one-count `C`:

```text
p_hat = C / N
delta = p_hat - 0.5
Z     = (2*C - N) / sqrt(N)
```

Integer totals stay in checked integers and convert to `f64` only for
proportion, deviation, and Z. Positive Z means excess ones; negative Z means
excess zeroes.

The standard-normal reading of Z assumes i.i.d. Bernoulli bits with `P(1) =
0.5`. This crate does not establish that assumption. Cumulative Z is a
correlated descriptive trajectory, not a sequential hypothesis decision.
Sessions stop manually, so a point crossing `±1.96` is not reported as
significance. The crate does not expose p-values, confidence intervals,
pass/fail states, entropy certification, or causal claims.
