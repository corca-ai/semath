import type { PromotionSeedSuite } from "./synthetic";

interface Seed {
  alternate: { formula: string; roles: Readonly<Record<string, string>> };
  formula: string;
  lawId: string;
  refusals: readonly string[];
  roles: Readonly<Record<string, string>>;
  separator: "=" | "\\approx" | "\\in" | "\\le";
}

function law(seed: Seed): PromotionSeedSuite["laws"][number] {
  const separator = ` ${seed.separator} `;
  const split = seed.formula.indexOf(separator);
  if (split < 1) throw new Error(`${seed.lawId}: missing ${separator}`);
  const left = seed.formula.slice(0, split);
  const right = seed.formula.slice(split + separator.length);
  const positives = [
    { formula: seed.formula, roles: seed.roles },
    { formula: `{${left}} ${seed.separator} {${right}}`, roles: seed.roles },
    { formula: `\\displaystyle ${left} ${seed.separator} ${right}`, roles: seed.roles },
    { formula: `\\left(${left}\\right) ${seed.separator} \\left(${right}\\right)`, roles: seed.roles },
    seed.alternate,
  ];
  const categories = [
    "wrong-operator",
    "role-mismatch",
    "missing-role-evidence",
    "wrong-sign",
    "wrong-discretization",
  ] as const;
  return {
    lawId: seed.lawId,
    positives: positives.map(({ formula, roles }, index) => [
      `${seed.lawId}-positive-${index + 1}`,
      `The reviewed formula uses explicit local numerical-analysis roles. $${formula}$`,
      formula,
      roles,
    ]),
    refusals: seed.refusals.map((formula, index) => [
      `${seed.lawId}-refusal-${index + 1}`,
      `The altered expression does not establish the reviewed numerical relation. $${formula}$`,
      formula,
      categories[index % categories.length]!,
    ]),
  };
}

const seeds: readonly Seed[] = [
  {
    lawId: "approximate-value-relation",
    formula: "u_h \\approx u",
    separator: "\\approx",
    roles: { "approximate-value": "u_h", "exact-value": "u" },
    alternate: { formula: "v_k \\approx v", roles: { "approximate-value": "v_k", "exact-value": "v" } },
    refusals: ["u_h=u", "u=u_h", "u_h\\ne u", "u_h\\le u", "u_h\\approx v"],
  },
  {
    lawId: "absolute-error-definition",
    formula: "e = \\operatorname{norm}(u-u_h)",
    separator: "=",
    roles: { "absolute-error": "e", "exact-value": "u", "approximate-value": "u_h" },
    alternate: { formula: "d=\\operatorname{norm}(v-v_k)", roles: { "absolute-error": "d", "exact-value": "v", "approximate-value": "v_k" } },
    refusals: ["e=u-u_h", "e=\\operatorname{norm}(u_h-u)", "e=\\operatorname{norm}(v-u_h)", "e=-\\operatorname{norm}(u-u_h)", "e=\\operatorname{norm}(u-u_h)+r"],
  },
  {
    lawId: "relative-error-definition",
    formula: "r = e/\\lVert u\\rVert",
    separator: "=",
    roles: { "relative-error": "r", "absolute-error": "e", "exact-value": "u" },
    alternate: { formula: "q=d/\\lVert v\\rVert", roles: { "relative-error": "q", "absolute-error": "d", "exact-value": "v" } },
    refusals: ["r=e", "r=e/\\lVert v\\rVert", "r=\\lVert u\\rVert/e", "r=-e/\\lVert u\\rVert", "r=e/(\\lVert u\\rVert+s)"],
  },
  {
    lawId: "truncation-error-bound",
    formula: "\\tau \\le C h^p",
    separator: "\\le",
    roles: { "truncation-error": "tau", coefficient: "C", step: "h", order: "p" },
    alternate: { formula: "\\rho \\le D k^q", roles: { "truncation-error": "rho", coefficient: "D", step: "k", order: "q" } },
    refusals: ["\\tau=C h^p", "\\tau\\le C k^p", "\\tau\\ge C h^p", "\\tau\\le-C h^p", "\\tau\\le C h^{p+1}"],
  },
  {
    lawId: "discretization-error-bound",
    formula: "\\delta \\le C h^p",
    separator: "\\le",
    roles: { "discretization-error": "delta", coefficient: "C", step: "h", order: "p" },
    alternate: { formula: "\\zeta \\le D k^q", roles: { "discretization-error": "zeta", coefficient: "D", step: "k", order: "q" } },
    refusals: ["\\delta=C h^p", "\\delta\\le C k^p", "\\delta\\ge C h^p", "\\delta\\le-C h^p", "\\delta\\le C h^{p+1}"],
  },
  {
    lawId: "asymptotic-order-membership",
    formula: "\\tau \\in O(h^p)",
    separator: "\\in",
    roles: { "truncation-error": "tau", step: "h", order: "p" },
    alternate: { formula: "\\rho \\in O(k^q)", roles: { "truncation-error": "rho", step: "k", order: "q" } },
    refusals: ["\\tau=O(h^p)", "\\tau\\in O(k^p)", "\\tau\\notin O(h^p)", "\\tau\\in O(-h^p)", "\\tau\\in O(h^{p+1})"],
  },
  {
    lawId: "linear-system-residual",
    formula: "r = b-Ax_h",
    separator: "=",
    roles: { residual: "r", "right-hand-side": "b", operator: "A", "approximate-solution": "x_h" },
    alternate: { formula: "s=c-By_k", roles: { residual: "s", "right-hand-side": "c", operator: "B", "approximate-solution": "y_k" } },
    refusals: ["r=b+Ax_h", "r=c-Ax_h", "r=b-x_h A", "r=Ax_h-b", "r=b-Ax_h+q"],
  },
  {
    lawId: "residual-stopping-condition",
    formula: "\\lVert r\\rVert \\le \\epsilon",
    separator: "\\le",
    roles: { residual: "r", tolerance: "epsilon" },
    alternate: { formula: "\\lVert s\\rVert \\le \\tau", roles: { residual: "s", tolerance: "tau" } },
    refusals: ["r\\le\\epsilon", "\\lVert q\\rVert\\le\\epsilon", "\\lVert r\\rVert\\ge\\epsilon", "-\\lVert r\\rVert\\le\\epsilon", "\\lVert r\\rVert\\le\\epsilon+d"],
  },
  {
    lawId: "convergence-envelope",
    formula: "\\operatorname{norm}(x_k-z) \\le e_k",
    separator: "\\le",
    roles: { iterate: "x_k", limit: "z", "error-bound": "e_k" },
    alternate: { formula: "\\operatorname{norm}(y_j-w) \\le d_j", roles: { iterate: "y_j", limit: "w", "error-bound": "d_j" } },
    refusals: ["x_k=z", "\\operatorname{norm}(x_k-w)\\le e_k", "\\operatorname{norm}(x_k-z)\\ge e_k", "-\\operatorname{norm}(x_k-z)\\le e_k", "\\operatorname{norm}(x_k-z)\\le e_k+r"],
  },
  {
    lawId: "linear-convergence-step",
    formula: "e_{k+1} \\le q e_k",
    separator: "\\le",
    roles: { "next-error": "e_{k+1}", rate: "q", "current-error": "e_k" },
    alternate: { formula: "d_{j+1} \\le \\rho d_j", roles: { "next-error": "d_{j+1}", rate: "rho", "current-error": "d_j" } },
    refusals: ["e_{k+1}=q e_k", "e_{k+1}\\le q d_k", "e_{k+1}\\ge q e_k", "e_{k+1}\\le-q e_k", "e_{k+1}\\le q e_k+r"],
  },
  {
    lawId: "newton-root-update",
    formula: "x_{k+1} = x_k-f_k/d_k",
    separator: "=",
    roles: { "next-iterate": "x_{k+1}", "current-iterate": "x_k", "function-value": "f_k", "derivative-value": "d_k" },
    alternate: { formula: "y_{j+1}=y_j-g_j/c_j", roles: { "next-iterate": "y_{j+1}", "current-iterate": "y_j", "function-value": "g_j", "derivative-value": "c_j" } },
    refusals: ["x_{k+1}=x_k-f_k", "x_{k+1}=y_k-f_k/d_k", "x_{k+1}=x_k+f_k/d_k", "x_{k+1}=x_k-f_k/(-d_k)", "x_{k+1}=x_k-f_k/d_k+r"],
  },
  {
    lawId: "trapezoidal-quadrature",
    formula: "Q = h\\cdot(f_a+f_b)/2",
    separator: "=",
    roles: { "integral-approximation": "Q", step: "h", "left-value": "f_a", "right-value": "f_b" },
    alternate: { formula: "T=k\\cdot(g_c+g_d)/2", roles: { "integral-approximation": "T", step: "k", "left-value": "g_c", "right-value": "g_d" } },
    refusals: ["Q=h(f_a+f_b)", "Q=h(f_a+g_b)/2", "Q=h(f_a-f_b)/2", "Q=-h(f_a+f_b)/2", "Q=h(f_a+f_b)/3"],
  },
  {
    lawId: "forward-difference-derivative",
    formula: "D = (f_1-f_0)/h",
    separator: "=",
    roles: { "derivative-approximation": "D", "next-value": "f_1", "current-value": "f_0", step: "h" },
    alternate: { formula: "G=(g_2-g_1)/k", roles: { "derivative-approximation": "G", "next-value": "g_2", "current-value": "g_1", step: "k" } },
    refusals: ["D=f_1-f_0", "D=(g_1-f_0)/h", "D=(f_1+f_0)/h", "D=-(f_1-f_0)/h", "D=(f_1-f_0)/(2h)"],
  },
  {
    lawId: "central-difference-derivative",
    formula: "D = (f_R-f_L)/(2h)",
    separator: "=",
    roles: { "derivative-approximation": "D", "right-value": "f_R", "left-value": "f_L", step: "h" },
    alternate: { formula: "G=(g_R-g_L)/(2k)", roles: { "derivative-approximation": "G", "right-value": "g_R", "left-value": "g_L", step: "k" } },
    refusals: ["D=(f_R-f_L)/h", "D=(g_R-f_L)/(2h)", "D=(f_R+f_L)/(2h)", "D=-(f_R-f_L)/(2h)", "D=(f_R-f_L)/(3h)"],
  },
  {
    lawId: "explicit-euler-step",
    formula: "y_{n+1} = y_n+h f_n",
    separator: "=",
    roles: { "next-state": "y_{n+1}", "current-state": "y_n", step: "h", source: "f_n" },
    alternate: { formula: "z_{m+1}=z_m+k g_m", roles: { "next-state": "z_{m+1}", "current-state": "z_m", step: "k", source: "g_m" } },
    refusals: ["y_{n+1}=y_n+f_n", "y_{n+1}=z_n+h f_n", "y_{n+1}=y_n-h f_n", "y_{n+1}=y_n+(-h)f_n", "y_{n+1}=y_n+h f_{n+1}"],
  },
  {
    lawId: "implicit-euler-step",
    formula: "y_{n+1} = y_n+h f_{n+1}",
    separator: "=",
    roles: { "next-state": "y_{n+1}", "current-state": "y_n", step: "h", "next-source": "f_{n+1}" },
    alternate: { formula: "z_{m+1}=z_m+k g_{m+1}", roles: { "next-state": "z_{m+1}", "current-state": "z_m", step: "k", "next-source": "g_{m+1}" } },
    refusals: ["y_{n+1}=y_n+f_{n+1}", "y_{n+1}=z_n+h f_{n+1}", "y_{n+1}=y_n-h f_{n+1}", "y_{n+1}=y_n+(-h)f_{n+1}", "y_{n+1}=y_n+h f_n"],
  },
  {
    lawId: "relaxed-residual-iteration",
    formula: "x_{k+1} = x_k+\\omega r_k",
    separator: "=",
    roles: { "next-iterate": "x_{k+1}", "current-iterate": "x_k", relaxation: "omega", residual: "r_k" },
    alternate: { formula: "y_{j+1}=y_j+\\alpha s_j", roles: { "next-iterate": "y_{j+1}", "current-iterate": "y_j", relaxation: "alpha", residual: "s_j" } },
    refusals: ["x_{k+1}=x_k+r_k", "x_{k+1}=y_k+\\omega r_k", "x_{k+1}=x_k-\\omega r_k", "x_{k+1}=x_k+(-\\omega)r_k", "x_{k+1}=x_k+\\omega r_{k+1}"],
  },
  {
    lawId: "least-squares-approximation",
    formula: "b \\approx A x",
    separator: "\\approx",
    roles: { observation: "b", design: "A", parameter: "x" },
    alternate: { formula: "c \\approx B y", roles: { observation: "c", design: "B", parameter: "y" } },
    refusals: ["b=Ax", "b\\approx xA", "b\\ne Ax", "b\\approx-Ax", "b\\approx Ax+r"],
  },
  {
    lawId: "linear-interpolation",
    formula: "p = \\alpha f_L+\\beta f_R",
    separator: "=",
    roles: { interpolant: "p", "left-weight": "alpha", "left-value": "f_L", "right-weight": "beta", "right-value": "f_R" },
    alternate: { formula: "q=\\gamma g_L+\\delta g_R", roles: { interpolant: "q", "left-weight": "gamma", "left-value": "g_L", "right-weight": "delta", "right-value": "g_R" } },
    refusals: ["p=f_L+f_R", "p=\\alpha g_L+\\beta f_R", "p=\\alpha f_L-\\beta f_R", "p=-\\alpha f_L+\\beta f_R", "p=\\alpha f_L+\\beta f_R+s"],
  },
  {
    lawId: "condition-number-definition",
    formula: "\\kappa = \\lVert A\\rVert\\lVert B\\rVert",
    separator: "=",
    roles: { "condition-number": "kappa", operator: "A", inverse: "B" },
    alternate: { formula: "\\chi=\\lVert M\\rVert\\lVert N\\rVert", roles: { "condition-number": "chi", operator: "M", inverse: "N" } },
    refusals: ["\\kappa=AB", "\\kappa=\\lVert C\\rVert\\lVert B\\rVert", "\\kappa=\\lVert A\\rVert+\\lVert B\\rVert", "\\kappa=-\\lVert A\\rVert\\lVert B\\rVert", "\\kappa=\\lVert A\\rVert\\lVert B\\rVert+s"],
  },
  {
    lawId: "perturbation-stability-bound",
    formula: "u \\le \\kappa v",
    separator: "\\le",
    roles: { "output-error": "u", "condition-number": "kappa", "input-error": "v" },
    alternate: { formula: "w \\le \\chi z", roles: { "output-error": "w", "condition-number": "chi", "input-error": "z" } },
    refusals: ["u=\\kappa v", "u\\le\\chi v", "u\\ge\\kappa v", "u\\le-\\kappa v", "u\\le\\kappa v+s"],
  },
  {
    lawId: "discrete-model-equation",
    formula: "L_h u_h = f",
    separator: "=",
    roles: { "discrete-operator": "L_h", "approximate-field": "u_h", source: "f" },
    alternate: { formula: "M_k v_k=g", roles: { "discrete-operator": "M_k", "approximate-field": "v_k", source: "g" } },
    refusals: ["L_h+u_h=f", "L_h v_h=f", "u_h L_h=f", "L_h u_h=-f", "L_h u_h=f+r"],
  },
];

export const numericalAnalysisFoundationSuite: PromotionSeedSuite = {
  id: "numerical-analysis-foundation-probe",
  laws: seeds.map(law),
  packId: "numerical-analysis",
};
